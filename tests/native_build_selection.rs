#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{
        self, BuildPlanRequest, CatalogDocumentFormat, CatalogImportRequest, DatImportRequest,
        RunWorkflowRequest, SourceScanRequest,
    },
    database::Database,
    domain::{
        BuildMode, CatalogKey, CatalogScope, MatchingPolicy, MissingContentPolicy,
        PublishingSourceKey, SetName, SetSelection, ZipCompression,
    },
};

const ABC_ROM: &str = r#"<rom name="abc.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>"#;

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    dat_path: Utf8PathBuf,
    source_path: Utf8PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(directory.path().to_path_buf())?;
        let dat_path = root.join("catalog.dat");
        let source_path = root.join("roms");
        std::fs::create_dir(&source_path)?;
        let database = Database::open(&root.join("catalog.sqlite"))?;
        Ok(Self {
            directory,
            database,
            dat_path,
            source_path,
        })
    }

    fn write_catalog(path: &Utf8PathBuf, set_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(
            path,
            format!(
                "<datafile><header><name>{set_name}</name></header><game name=\"{set_name}\" sourcefile=\"src/{set_name}.cpp\">{ABC_ROM}</game></datafile>"
            ),
        )?;
        Ok(())
    }

    fn import_explicit_catalog(
        &self,
        document_path: &Utf8PathBuf,
        catalog_key: &str,
    ) -> mame_coalesce::Result<app::CatalogImportReport> {
        app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path: document_path.clone(),
                format: CatalogDocumentFormat::Logiqx,
                source_key: PublishingSourceKey::new(format!("publisher-{catalog_key}")),
                source_display_name: "Test publisher".to_owned(),
                catalog_key: CatalogKey::new(catalog_key),
                catalog_display_name: catalog_key.to_owned(),
                scope: CatalogScope::Complete,
            },
        )
    }

    fn plan_request(&self, dat_path: Utf8PathBuf) -> BuildPlanRequest {
        BuildPlanRequest {
            dat_path,
            source_path: self.source_path.clone(),
            mode: BuildMode::PerGame,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
            set_selection: SetSelection::All,
        }
    }
}

#[test]
fn one_shot_run_uses_reverted_snapshot_for_exact_set_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    std::fs::write(fixture.source_path.join("abc.rom"), b"abc")?;
    Fixture::write_catalog(&fixture.dat_path, "set-a")?;
    let first_a = app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;
    Fixture::write_catalog(&fixture.dat_path, "set-b")?;
    app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;
    Fixture::write_catalog(&fixture.dat_path, "set-a")?;
    let reverted_a = app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;
    assert_eq!(first_a.snapshot_key, reverted_a.snapshot_key);

    let report = app::run(
        &fixture.database,
        &RunWorkflowRequest {
            dat_path: fixture.dat_path.clone(),
            source_path: fixture.source_path.clone(),
            destination_path: fixture.directory.path().join("output").try_into()?,
            mode: BuildMode::PerGame,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: true,
            strict: false,
            set_selection: SetSelection::exact_names([SetName::new("set-a")]),
        },
    )?;
    assert!(report.build_report.set_selection_issues.is_empty());
    assert_eq!(report.build_report.matched_roms, 1);
    assert!(report.build_report.missing_roms.is_empty());
    Ok(())
}

#[test]
fn existing_catalog_source_path_yields_to_exact_catalog_key()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    Fixture::write_catalog(&fixture.dat_path, "automatic-set")?;
    app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;

    let explicit_document = fixture.directory.path().join("explicit.dat");
    let explicit_document = Utf8PathBuf::try_from(explicit_document)?;
    Fixture::write_catalog(&explicit_document, "explicit-set")?;
    fixture.import_explicit_catalog(&explicit_document, fixture.dat_path.as_str())?;

    let plan = app::plan_build(
        &fixture.database,
        &fixture.plan_request(fixture.dat_path.clone()),
    )?;
    assert_eq!(plan.report.missing_roms.len(), 1);
    assert_eq!(plan.report.missing_roms[0].game_name, "explicit-set");
    Ok(())
}

#[test]
fn cached_plan_loads_catalog_after_its_source_file_is_removed()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    Fixture::write_catalog(&fixture.dat_path, "cached-set")?;
    std::fs::write(fixture.source_path.join("abc.rom"), b"abc")?;
    app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;
    app::scan_source(
        &fixture.database,
        &SourceScanRequest {
            source_path: fixture.source_path.clone(),
            jobs: 1,
        },
    )?;
    std::fs::remove_file(&fixture.dat_path)?;
    assert!(fixture.source_path.is_dir());

    let plan = app::plan_build(
        &fixture.database,
        &fixture.plan_request(fixture.dat_path.clone()),
    )?;
    assert_eq!(plan.report.matched_roms, 1);
    assert!(plan.report.missing_roms.is_empty());
    Ok(())
}

#[test]
fn one_shot_run_keeps_its_snapshot_when_another_import_publishes_during_scan()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    Fixture::write_catalog(&fixture.dat_path, "set-a")?;
    std::fs::write(fixture.source_path.join("abc.rom"), b"abc")?;
    let publication = std::sync::Once::new();
    let report = app::run_with_progress(
        &fixture.database,
        &RunWorkflowRequest {
            dat_path: fixture.dat_path.clone(),
            source_path: fixture.source_path.clone(),
            destination_path: fixture.directory.path().join("output").try_into()?,
            mode: BuildMode::PerGame,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: true,
            strict: false,
            set_selection: SetSelection::exact_names([SetName::new("set-a")]),
        },
        &|_| {
            publication.call_once(|| {
                Fixture::write_catalog(&fixture.dat_path, "set-b")
                    .expect("write competing edition");
                app::import_dat(
                    &fixture.database,
                    &DatImportRequest {
                        dat_path: fixture.dat_path.clone(),
                    },
                )
                .expect("publish competing edition");
            });
        },
    )?;
    assert!(publication.is_completed());
    assert!(report.build_report.set_selection_issues.is_empty());
    assert_eq!(report.build_report.matched_roms, 1);
    assert_eq!(report.artifact_results.len(), 1);
    assert!(report.artifact_results[0].path.ends_with("/set-a.zip"));
    Ok(())
}

#[test]
fn refreshed_audit_uses_one_snapshot_for_selection_and_requirements()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    Fixture::write_catalog(&fixture.dat_path, "set-a")?;
    std::fs::write(fixture.source_path.join("abc.rom"), b"abc")?;
    app::import_dat(
        &fixture.database,
        &DatImportRequest {
            dat_path: fixture.dat_path.clone(),
        },
    )?;
    let publication = std::sync::Once::new();
    let audit = app::audit_with_progress(
        &fixture.database,
        &app::AuditRequest {
            dat_path: fixture.dat_path.clone(),
            source_path: fixture.source_path.clone(),
            refresh: app::AuditRefresh::Refresh,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            jobs: 1,
            set_selection: SetSelection::exact_names([SetName::new("set-a")]),
        },
        &|_| {
            publication.call_once(|| {
                Fixture::write_catalog(&fixture.dat_path, "set-b")
                    .expect("write competing edition");
                app::import_dat(
                    &fixture.database,
                    &DatImportRequest {
                        dat_path: fixture.dat_path.clone(),
                    },
                )
                .expect("publish competing edition");
            });
        },
    )?;
    assert!(publication.is_completed());
    assert!(audit.report().set_selection_issues.is_empty());
    assert_eq!(audit.report().matched_roms, 1);
    Ok(())
}
