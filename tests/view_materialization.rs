use std::{fs, io::Read};

use camino::{Utf8Path, Utf8PathBuf};
use mame_coalesce::{
    app::{ViewMaterializationRequest, materialize_view},
    build::{
        mame_layout::{
            AssetRequirementIdentity, MameLayoutDiagnostic, MameSetLayoutPolicy,
            ResolvedMachineAsset, ResolvedMachineSet,
        },
        view_manifest::{TargetViewRequest, ViewManifest, plan_view},
    },
    domain::{
        CatalogKey, CatalogScope, DocumentKey, EvidenceProvenance, EvidenceScope, ExpectedEvidence,
        LogicalEntry, LogicalPath, MatchingPolicy, ObservedContent, OutputContainer,
        ParserInterpretationKey, RequirementKey, SelectionProvenance, SetKey, SetName, SnapshotKey,
        SourceFile, SourceLocation, SourcePhysicalPath, SourceRoot, ZipCompression,
    },
    hashes::{sha1_bytes, xxhash3_bytes},
    machine_dependencies::{DependencyDiagnostic, MachineDependencyCatalog, MachineSet},
    mount::MountProjection,
    resolution::MatchStrength,
    serving::{ByteLength, MaterializationBudget},
};
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    source_root: Utf8PathBuf,
    source: Utf8PathBuf,
    contents: Vec<u8>,
}

impl Fixture {
    fn new(contents: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = utf8_path(temp.path())?;
        let source_root = root.join("roms");
        fs::create_dir_all(&source_root)?;
        let source = source_root.join("nes.rom");
        fs::write(&source, contents)?;
        Ok(Self {
            temp,
            source_root,
            source,
            contents: contents.to_vec(),
        })
    }

    fn manifest(&self, snapshot: &SnapshotKey, include_set: bool) -> ViewManifest {
        let root = SetName::new("nes");
        let closure =
            MachineDependencyCatalog::new(snapshot.clone(), vec![MachineSet::new(root.as_str())])
                .resolve(&root);
        let closures = [closure];
        let sets = if include_set {
            vec![self.resolved_set(snapshot, &root)]
        } else {
            Vec::new()
        };
        plan_view(TargetViewRequest::mame_0289(
            snapshot,
            std::slice::from_ref(&root),
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        ))
    }

    fn resolved_set(&self, snapshot: &SnapshotKey, root: &SetName) -> ResolvedMachineSet {
        let catalog = CatalogKey::new("mame");
        let sha1 = sha1_bytes(&self.contents);
        let location = SourceLocation::BareFile {
            path: self.source.to_string(),
        };
        let source = SourceFile {
            source_root: SourceRoot::new(self.source_root.to_string()),
            physical_path: SourcePhysicalPath::from_location(&location),
            location,
            observed: ObservedContent {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::Computed,
                size: Some(self.contents.len() as u64),
                crc: None,
                md5: None,
                sha1: Some(sha1),
                xxh3: xxhash3_bytes(&self.contents),
            },
            fingerprint: None,
            scan_run: None,
            scan_provenance: None,
            bare_file_cache_stamp: None,
        };
        let requirement =
            RequirementKey::new(SetKey::new(catalog.clone(), root.as_str()), "nes.rom");
        let entry = LogicalEntry {
            path: LogicalPath::new("nes.rom"),
            source,
            requirement,
            expected: ExpectedEvidence {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::SourceDeclared,
                size: Some(self.contents.len() as u64),
                sha1: Some(sha1),
                ..ExpectedEvidence::default()
            },
            selection: SelectionProvenance {
                policy: MatchingPolicy::Sha1Compatibility,
                strength: MatchStrength::Sha1,
                assessments: Vec::new(),
                omitted_assessments: 0,
            },
        };
        let mut set = ResolvedMachineSet::new(snapshot.clone(), catalog, root.clone());
        set.entries.push(ResolvedMachineAsset::new(
            AssetRequirementIdentity::new(snapshot.clone(), root.clone(), "nes.rom", 0),
            entry,
        ));
        set
    }
}

fn snapshot(name: &str) -> SnapshotKey {
    let catalog = CatalogKey::new("mame");
    let document = DocumentKey::from_bytes(name.as_bytes());
    let interpretation =
        ParserInterpretationKey::for_format("mame-machine-xml", &CatalogScope::Complete);
    SnapshotKey::new(&catalog, &document, &interpretation)
}

fn utf8_path(path: &std::path::Path) -> Result<&Utf8Path, std::io::Error> {
    Utf8Path::from_path(path).ok_or_else(|| std::io::Error::other("path is not UTF-8"))
}

#[test]
fn one_pinned_manifest_materializes_equal_zip_and_directory_content()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"same pinned ROM bytes")?;
    let pinned_snapshot = snapshot("published-1");
    let manifest = fixture.manifest(&pinned_snapshot, true);
    let serialized = manifest.to_json()?;
    let newer_fixture = Fixture::new(b"different bytes from a newer catalog snapshot")?;
    let newer_manifest = newer_fixture.manifest(&snapshot("published-2"), true);
    assert_ne!(newer_manifest.snapshot(), manifest.snapshot());
    assert_ne!(
        newer_manifest.layout().groups()[0].entries()[0]
            .expected
            .sha1,
        manifest.layout().groups()[0].entries()[0].expected.sha1
    );
    assert_eq!(manifest.snapshot(), &pinned_snapshot);
    assert_eq!(manifest.to_json()?, serialized);

    let root = utf8_path(fixture.temp.path())?;
    let zip_destination = root.join("zip-output");
    let directory_destination = root.join("directory-output");
    let zip_report = materialize_view(&ViewMaterializationRequest {
        manifest: &manifest,
        destination_path: &zip_destination,
        container: OutputContainer::Zip,
        compression: ZipCompression::Deflate,
    })?;
    let directory_report = materialize_view(&ViewMaterializationRequest {
        manifest: &manifest,
        destination_path: &directory_destination,
        container: OutputContainer::Directory,
        compression: ZipCompression::Store,
    })?;

    assert!(zip_report.layout_diagnostics.is_empty());
    assert!(zip_report.validation_issues.is_empty());
    assert_eq!(zip_report.written_paths.len(), 1);
    assert_eq!(directory_report.written_paths.len(), 1);
    let zip_path = zip_destination.join("nes.zip");
    let mut archive = zip::ZipArchive::new(fs::File::open(zip_path)?)?;
    let mut archived_contents = Vec::new();
    archive
        .by_name("nes.rom")?
        .read_to_end(&mut archived_contents)?;
    let directory_contents = fs::read(directory_destination.join("nes/nes.rom"))?;
    assert_eq!(archived_contents, fixture.contents);
    assert_eq!(directory_contents, fixture.contents);
    assert_eq!(archived_contents, directory_contents);
    Ok(())
}

#[test]
fn stale_source_does_not_replace_a_prior_valid_artifact() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new(b"planned bytes")?;
    let manifest = fixture.manifest(&snapshot("published"), true);
    let destination = utf8_path(fixture.temp.path())?.join("output");
    fs::create_dir_all(&destination)?;
    let artifact = destination.join("nes.zip");
    fs::write(&artifact, b"prior valid artifact")?;
    let previous = fs::read(&artifact)?;
    fs::write(&fixture.source, b"changed after planning")?;

    let report = materialize_view(&ViewMaterializationRequest {
        manifest: &manifest,
        destination_path: &destination,
        container: OutputContainer::Zip,
        compression: ZipCompression::Deflate,
    })?;
    let result = report
        .artifact_results
        .first()
        .ok_or_else(|| std::io::Error::other("expected an artifact result"))?;
    assert!(matches!(
        &result.outcome,
        mame_coalesce::domain::ArtifactOutcome::Failed { error }
            if error.contains("source changed since planning")
    ));
    assert_eq!(fs::read(&artifact)?, previous);
    assert_eq!(fs::read_dir(&destination)?.count(), 1);
    Ok(())
}

#[test]
fn unresolved_profile_dependencies_block_materialization_before_writing()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"unused")?;
    let manifest = fixture.manifest(&snapshot("published"), false);
    let destination = utf8_path(fixture.temp.path())?.join("output");
    fs::create_dir_all(&destination)?;
    let artifact = destination.join("nes.zip");
    fs::write(&artifact, b"prior valid artifact")?;
    let previous = fs::read(&artifact)?;

    let report = materialize_view(&ViewMaterializationRequest {
        manifest: &manifest,
        destination_path: &destination,
        container: OutputContainer::Zip,
        compression: ZipCompression::Deflate,
    })?;
    assert!(!report.layout_diagnostics.is_empty());
    assert!(report.artifact_results.is_empty());
    assert_eq!(fs::read(&artifact)?, previous);
    Ok(())
}

#[test]
fn mount_preflight_uses_the_manifest_source_and_rejects_stale_bytes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"pinned mount bytes")?;
    let manifest = fixture.manifest(&snapshot("mounted-snapshot"), true);
    let projection = MountProjection::compile(manifest)?;
    let spool_root = utf8_path(fixture.temp.path())?.join("mount-spool");
    fs::create_dir(&spool_root)?;
    let budget = MaterializationBudget::new_in(
        ByteLength::new(512 * 1024 * 1024),
        ByteLength::new(256 * 1024 * 1024),
        spool_root,
    );

    projection.preflight_sources(&budget)?;
    fs::write(&fixture.source, b"replaced after the manifest was pinned")?;
    assert!(matches!(
        projection.preflight_sources(&budget),
        Err(mame_coalesce::mount::MountPreflightError::Serving(
            mame_coalesce::serving::ServingError::SourceChanged { .. }
        ))
    ));
    Ok(())
}

#[test]
fn mount_preflight_rejects_mountpoints_that_hide_pinned_sources()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"pinned mount bytes")?;
    let manifest = fixture.manifest(&snapshot("mounted-snapshot"), true);
    let projection = MountProjection::compile(manifest)?;
    let spool_root = utf8_path(fixture.temp.path())?.join("mount-spool");
    fs::create_dir(&spool_root)?;

    assert!(matches!(
        projection
            .validate_spool_root(spool_root.as_std_path(), fixture.source_root.as_std_path(),),
        Err(mame_coalesce::mount::MountStartupError::MountOverlapsSource(_))
    ));
    Ok(())
}

#[test]
fn mount_startup_rejects_nonempty_mountpoints() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"pinned mount bytes")?;
    let manifest = fixture.manifest(&snapshot("mounted-snapshot"), true);
    let projection = MountProjection::compile(manifest)?;
    let root = utf8_path(fixture.temp.path())?;
    let spool_root = root.join("mount-spool");
    let mount_point = root.join("mount-point");
    fs::create_dir(&spool_root)?;
    fs::create_dir(&mount_point)?;
    fs::write(mount_point.join("existing-file"), b"keep me")?;

    assert!(matches!(
        projection.validate_spool_root(spool_root.as_std_path(), mount_point.as_std_path()),
        Err(mame_coalesce::mount::MountStartupError::MountPointNotEmpty(
            _
        ))
    ));
    assert_eq!(fs::read(mount_point.join("existing-file"))?, b"keep me");
    Ok(())
}

#[test]
fn unsupported_profile_relationships_block_materialization_before_writing()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(b"unused")?;
    let snapshot = snapshot("published");
    let root = SetName::new("nes");
    let mut machine = MachineSet::new(root.as_str());
    machine
        .unsupported_relationships
        .push(("requires_disk".to_owned(), SetName::new("disk")));
    let closure = MachineDependencyCatalog::new(snapshot.clone(), vec![machine]).resolve(&root);
    let closures = [closure];
    let sets = [fixture.resolved_set(&snapshot, &root)];
    let manifest = plan_view(TargetViewRequest::mame_0289(
        &snapshot,
        std::slice::from_ref(&root),
        MameSetLayoutPolicy::NonMerged,
        &closures,
        &sets,
    ));
    let destination = utf8_path(fixture.temp.path())?.join("output");
    fs::create_dir_all(&destination)?;
    let artifact = destination.join("nes.zip");
    fs::write(&artifact, b"prior valid artifact")?;
    let previous = fs::read(&artifact)?;

    let report = materialize_view(&ViewMaterializationRequest {
        manifest: &manifest,
        destination_path: &destination,
        container: OutputContainer::Zip,
        compression: ZipCompression::Deflate,
    })?;

    assert!(report.layout_diagnostics.iter().any(|diagnostic| matches!(
        diagnostic,
        MameLayoutDiagnostic::Dependency {
            diagnostic: DependencyDiagnostic::UnsupportedRelationship { .. },
            ..
        }
    )));
    assert!(report.artifact_results.is_empty());
    assert_eq!(fs::read(&artifact)?, previous);
    Ok(())
}
