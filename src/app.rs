use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName,
    prelude::*,
    sql_query,
    sql_types::{Binary, Nullable, Text},
};
use serde::Serialize;

use crate::{
    build::{planner::plan_build as build_plan, write_plan_with_compression},
    database::Database,
    disk::{
        self, DiskDigestScope, DiskIdentitySha1, DiskName, DiskObservation, DiskRequirement,
        DiskVerificationState, ParentDiskName,
    },
    domain::{
        ArtifactOutcome, ArtifactResult, BuildMode, BuildReport, BuildRequest, CatalogKey,
        CatalogScope, ImportRunKey, MatchingPolicy, MissingContentPolicy, PlanOutcome,
        PublishingSourceKey, ScanRunKey, SnapshotKey, SourceRoot, ZipCompression,
    },
    operations,
    storage::repositories::{BuildRepository, DataFileSelector, SourceRepository},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogImportRequest {
    pub document_path: Utf8PathBuf,
    pub format: CatalogDocumentFormat,
    pub source_key: PublishingSourceKey,
    pub source_display_name: String,
    pub catalog_key: CatalogKey,
    pub catalog_display_name: String,
    pub scope: CatalogScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogDocumentFormat {
    Logiqx,
    MameListXml,
    MameSoftwareListXml,
    ClrMamePro,
    NoIntroPcXml,
}

impl CatalogDocumentFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Logiqx => "logiqx",
            Self::MameListXml => "mame-listxml",
            Self::MameSoftwareListXml => "mame-softwarelist-xml",
            Self::ClrMamePro => "clrmamepro-dat",
            Self::NoIntroPcXml => "no-intro-pc-xml",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogImportStatus {
    Succeeded,
    Failed,
}

impl CatalogImportStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogImportReport {
    pub snapshot_key: Option<SnapshotKey>,
    pub run_key: ImportRunKey,
    pub status: CatalogImportStatus,
    pub diagnostic_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatImportRequest {
    pub dat_path: Utf8PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatImportReport {
    pub data_file_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceScanRequest {
    pub source_path: Utf8PathBuf,
    pub jobs: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceScanReport {
    pub source_path: Utf8PathBuf,
    pub scan_run: ScanRunKey,
    pub observation_count: usize,
    pub associated_rom_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanProgressEvent {
    Started { files: u64 },
    Advanced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildWorkflowRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub destination_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub compression: ZipCompression,
    pub dry_run: bool,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildWorkflowReport {
    pub written_paths: Vec<Utf8PathBuf>,
    pub artifact_results: Vec<ArtifactResult>,
    pub build_report: BuildReport,
    pub scan_report: Option<SourceScanReport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlanRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub missing_policy: MissingContentPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunWorkflowRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub destination_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub compression: ZipCompression,
    pub jobs: usize,
    pub dry_run: bool,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskAuditRequest {
    pub catalog_key: String,
    pub source_path: Utf8PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiskAuditReport {
    pub schema_version: u32,
    pub catalog_key: String,
    pub snapshot_key: String,
    pub source_path: Utf8PathBuf,
    pub disks: Vec<DiskAuditEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DiskAuditEntry {
    pub set_name: String,
    pub disk_name: String,
    pub list_name: Option<String>,
    pub item_name: Option<String>,
    pub part_name: Option<String>,
    pub parent_disk: Option<String>,
    pub expected_logical_sha1: Option<String>,
    pub state: DiskAuditState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskAuditState {
    Missing,
    AmbiguousLocation,
    UnknownDigestScope,
    IdentityNotDeclared,
    UnverifiedContainer,
    UnsupportedContainer,
    VerifiedLogicalIdentity,
    LogicalIdentityMismatch,
}

impl From<DiskVerificationState> for DiskAuditState {
    fn from(state: DiskVerificationState) -> Self {
        match state {
            DiskVerificationState::Missing => Self::Missing,
            DiskVerificationState::UnknownDigestScope => Self::UnknownDigestScope,
            DiskVerificationState::IdentityNotDeclared => Self::IdentityNotDeclared,
            DiskVerificationState::UnverifiedContainer => Self::UnverifiedContainer,
            DiskVerificationState::UnsupportedContainer => Self::UnsupportedContainer,
            DiskVerificationState::VerifiedLogicalIdentity => Self::VerifiedLogicalIdentity,
            DiskVerificationState::LogicalIdentityMismatch => Self::LogicalIdentityMismatch,
        }
    }
}

#[derive(QueryableByName)]
struct PublishedDiskSnapshotRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
}

#[derive(Clone, QueryableByName)]
struct DiskRequirementRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    asset_name: String,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    list_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    item_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    part_name: Option<String>,
}

type DiskParentMap = HashMap<(Option<String>, String), String>;
type DiskSourceIndex<'files> = HashMap<String, Vec<&'files camino::Utf8Path>>;
type DiskRequirementsByEntity<'rows> =
    HashMap<(Option<String>, String), Vec<&'rows DiskRequirementRow>>;

#[derive(QueryableByName)]
struct DiskParentRow {
    #[diesel(sql_type = Nullable<Text>)]
    list: Option<String>,
    #[diesel(sql_type = Text)]
    child: String,
    #[diesel(sql_type = Text)]
    parent: String,
}

/// Audit disk requirements against a source root without reading or hashing its CHD contents.
/// Container-byte hashes cannot establish logical CHD identity.
pub fn audit_disks(
    database: &Database,
    request: &DiskAuditRequest,
) -> crate::Result<DiskAuditReport> {
    let source_path = request.source_path.canonicalize_utf8()?;
    let (snapshot_key, requirements, parent_map) =
        load_published_disk_requirements(database, &request.catalog_key)?;
    let source_paths = operations::list_source_paths(&source_path, database.pool())?;
    let source_index = disk_source_index(&source_paths);
    let requirements_by_entity = disk_requirements_by_entity(&requirements);
    let disks = requirements
        .iter()
        .cloned()
        .map(|row| {
            audit_disk_requirement(
                row,
                &source_path,
                &source_index,
                &requirements_by_entity,
                &parent_map,
            )
        })
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(DiskAuditReport {
        schema_version: 1,
        catalog_key: request.catalog_key.clone(),
        snapshot_key,
        source_path,
        disks,
    })
}

fn load_published_disk_requirements(
    database: &Database,
    catalog_key: &str,
) -> crate::Result<(String, Vec<DiskRequirementRow>, DiskParentMap)> {
    let mut conn = database.pool().get()?;
    let snapshot = sql_query(
        "SELECT publication.snapshot_key FROM snapshot_publications AS publication \
         WHERE publication.catalog_key = ? \
         ORDER BY publication.rowid DESC LIMIT 1",
    )
    .bind::<Text, _>(catalog_key)
    .get_result::<PublishedDiskSnapshotRow>(&mut conn)
    .map_err(|error| match error {
        diesel::result::Error::NotFound => crate::Error::CatalogNotFound(catalog_key.to_owned()),
        error => error.into(),
    })?;
    let mut requirements = sql_query(
        "SELECT asset.set_name, asset.asset_name, asset.sha1, asset.evidence_scope, \
                asset.merge_name, NULL AS list_name, NULL AS item_name, NULL AS part_name \
         FROM asset_requirements AS asset \
         WHERE asset.snapshot_key = ? AND asset.role = 'disk' \
         ORDER BY asset.set_name, asset.component_order",
    )
    .bind::<Text, _>(&snapshot.snapshot_key)
    .load::<DiskRequirementRow>(&mut conn)?;
    requirements.extend(
        sql_query(
            "SELECT component.item_name AS set_name, component.component_name AS asset_name, \
                component.sha1, component.evidence_scope, NULL AS merge_name, \
                component.list_name, component.item_name, component.part_name \
         FROM software_components AS component \
         JOIN software_items AS item \
           ON item.snapshot_key = component.snapshot_key \
          AND item.list_name = component.list_name AND item.item_name = component.item_name \
         WHERE component.snapshot_key = ? AND component.component_kind = 'disk' \
         ORDER BY component.list_name, component.item_name, component.part_name, \
                  component.area_order, component.component_order",
        )
        .bind::<Text, _>(&snapshot.snapshot_key)
        .load::<DiskRequirementRow>(&mut conn)?,
    );
    let mut parent_rows = sql_query(
        "SELECT NULL AS list, set_name AS child, parent_name AS parent \
         FROM snapshot_sets WHERE snapshot_key = ? AND parent_name IS NOT NULL",
    )
    .bind::<Text, _>(&snapshot.snapshot_key)
    .load::<DiskParentRow>(&mut conn)?;
    parent_rows.extend(
        sql_query(
            "SELECT dependency.list_name AS list, dependency.item_name AS child, \
             dependency.target_item_name AS parent \
             FROM software_item_dependencies AS dependency \
             WHERE dependency.snapshot_key = ? AND dependency.dependency_kind = 'clone_of'",
        )
        .bind::<Text, _>(&snapshot.snapshot_key)
        .load::<DiskParentRow>(&mut conn)?,
    );
    let parent_map = parent_rows
        .into_iter()
        .map(|row| ((row.list, row.child), row.parent))
        .collect();
    Ok((snapshot.snapshot_key, requirements, parent_map))
}

fn audit_disk_requirement(
    row: DiskRequirementRow,
    source_path: &camino::Utf8Path,
    source_index: &DiskSourceIndex<'_>,
    requirements_by_entity: &DiskRequirementsByEntity<'_>,
    parent_map: &DiskParentMap,
) -> crate::Result<DiskAuditEntry> {
    let (requirement, expected_logical_sha1) = disk_requirement(&row)?;
    let (observation, ambiguous) = disk_observation(
        &row,
        source_path,
        source_index,
        requirements_by_entity,
        parent_map,
    );
    let state = if ambiguous {
        DiskAuditState::AmbiguousLocation
    } else {
        DiskAuditState::from(disk::audit_disk(&requirement, observation))
    };

    Ok(DiskAuditEntry {
        set_name: row.set_name,
        disk_name: row.asset_name,
        list_name: row.list_name,
        item_name: row.item_name,
        part_name: row.part_name,
        parent_disk: requirement
            .parent()
            .map(|parent| parent.as_str().to_owned()),
        expected_logical_sha1,
        state,
    })
}

fn disk_requirement(row: &DiskRequirementRow) -> crate::Result<(DiskRequirement, Option<String>)> {
    let expected = row
        .sha1
        .as_ref()
        .map(|bytes| {
            <[u8; 20]>::try_from(bytes.as_slice()).map_err(|_| {
                crate::Error::InvalidHash(format!(
                    "disk identity SHA-1 for {} has length {}; expected 20 bytes",
                    row.asset_name,
                    bytes.len()
                ))
            })
        })
        .transpose()?;
    let scope = match row.evidence_scope.as_str() {
        "chd_header_sha1" => DiskDigestScope::ChdHeaderSha1,
        _ => DiskDigestScope::Unknown,
    };
    let mut requirement = DiskRequirement::new(
        DiskName::new(row.asset_name.clone()),
        expected.map(DiskIdentitySha1::new),
        scope,
    );
    if let Some(parent) = row.merge_name.as_deref() {
        requirement = requirement.with_parent(ParentDiskName::new(parent));
    }
    Ok((requirement, expected.map(hex::encode)))
}

fn disk_observation(
    row: &DiskRequirementRow,
    source_path: &camino::Utf8Path,
    source_index: &DiskSourceIndex<'_>,
    requirements_by_entity: &DiskRequirementsByEntity<'_>,
    parent_map: &DiskParentMap,
) -> (DiskObservation, bool) {
    let candidate_locations = disk_locations(row, source_path, requirements_by_entity, parent_map);
    let matching_file = indexed_disk_candidates(&candidate_locations, source_index);
    matching_file.map_or((DiskObservation::Missing, false), |(file, assigned)| {
        if !assigned {
            return (DiskObservation::Missing, true);
        }
        let file_name = file.file_name().unwrap_or_default();
        if file_name.to_ascii_lowercase().ends_with(".chd") {
            (DiskObservation::ContainerPresent { byte_sha1: None }, false)
        } else {
            (DiskObservation::UnsupportedContainer, false)
        }
    })
}

fn disk_locations(
    row: &DiskRequirementRow,
    source_path: &camino::Utf8Path,
    requirements_by_entity: &DiskRequirementsByEntity<'_>,
    parent_map: &DiskParentMap,
) -> Vec<(Utf8PathBuf, Vec<String>)> {
    let mut locations = Vec::new();
    let initial_entity = (
        row.list_name.clone(),
        row.item_name
            .clone()
            .unwrap_or_else(|| row.set_name.clone()),
    );
    let direct_parent = parent_map
        .get(&initial_entity)
        .map(|parent| (initial_entity.0.clone(), parent.clone()));
    let mut entity = initial_entity;
    let mut ancestors = HashSet::new();
    while ancestors.insert(entity.clone()) {
        let mut names = vec![row.asset_name.clone()];
        if direct_parent.as_ref() == Some(&entity)
            && let Some(merge_name) = row.merge_name.as_deref()
        {
            names.push(merge_name.to_owned());
        }
        let entity_directories = std::iter::once(entity.0.as_deref().map_or_else(
            || source_path.join(&entity.1),
            |list| source_path.join(list).join(&entity.1),
        ))
        .chain(entity.0.is_some().then(|| source_path.join(&entity.1)));
        for parent_disk in requirements_by_entity
            .get(&entity)
            .into_iter()
            .flatten()
            .filter(|candidate| same_declared_disk_identity(row, candidate))
        {
            if !names.contains(&parent_disk.asset_name) {
                names.push(parent_disk.asset_name.clone());
            }
        }
        for directory in entity_directories {
            locations.push((directory, names.clone()));
        }
        let Some(parent) = parent_map.get(&entity).cloned() else {
            break;
        };
        entity = (entity.0, parent);
    }
    locations
}

fn disk_requirements_by_entity(
    requirements: &[DiskRequirementRow],
) -> DiskRequirementsByEntity<'_> {
    let mut index = DiskRequirementsByEntity::new();
    for requirement in requirements {
        let entity = (
            requirement.list_name.clone(),
            requirement
                .item_name
                .clone()
                .unwrap_or_else(|| requirement.set_name.clone()),
        );
        index.entry(entity).or_default().push(requirement);
    }
    index
}

fn disk_source_index(source_files: &[Utf8PathBuf]) -> DiskSourceIndex<'_> {
    let mut index = DiskSourceIndex::new();
    for file in source_files {
        if let Some(file_name) = file.file_name() {
            index
                .entry(file_name.to_owned())
                .or_default()
                .push(file.as_path());
            if file
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("chd"))
                && let Some(stem) = file.file_stem()
            {
                let normalized_name = format!("{stem}.chd");
                if normalized_name != file_name {
                    index
                        .entry(normalized_name)
                        .or_default()
                        .push(file.as_path());
                }
            }
        }
    }
    index
}

fn indexed_disk_candidates<'files>(
    locations: &[(Utf8PathBuf, Vec<String>)],
    source_index: &DiskSourceIndex<'files>,
) -> Option<(&'files camino::Utf8Path, bool)> {
    let mut candidates = Vec::new();
    for (directory, names) in locations {
        for name in names {
            for expected_name in [name.clone(), format!("{name}.chd")] {
                let Some(files) = source_index.get(&expected_name) else {
                    continue;
                };
                for file in files {
                    let assigned = file.parent() == Some(directory.as_path());
                    candidates.push((*file, expected_name.clone(), assigned));
                }
            }
        }
    }
    let matching_file = candidates
        .into_iter()
        .min_by_key(|(file, file_name, assigned)| {
            (
                u8::from(!assigned),
                u8::from(
                    !camino::Utf8Path::new(file_name)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("chd")),
                ),
                file.as_str().to_owned(),
            )
        });
    matching_file.map(|(file, _, assigned)| (file, assigned))
}

fn same_declared_disk_identity(left: &DiskRequirementRow, right: &DiskRequirementRow) -> bool {
    left.evidence_scope == "chd_header_sha1"
        && right.evidence_scope == left.evidence_scope
        && left.sha1.is_some()
        && left.sha1 == right.sha1
}

pub fn import_dat(
    database: &Database,
    request: &DatImportRequest,
) -> crate::Result<DatImportReport> {
    operations::parse_and_insert_datfile(&request.dat_path, database.pool())
        .map(|data_file_id| DatImportReport { data_file_id })
}

pub fn import_catalog(
    database: &Database,
    request: &CatalogImportRequest,
) -> crate::Result<CatalogImportReport> {
    crate::storage::catalog_import::import(database.pool(), request)
}

pub fn scan_source(
    database: &Database,
    request: &SourceScanRequest,
) -> crate::Result<SourceScanReport> {
    scan_source_with_progress(database, request, &|_| {})
}

pub fn scan_source_with_progress(
    database: &Database,
    request: &SourceScanRequest,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<SourceScanReport> {
    let excluded_paths = crate::storage::db::database_file_paths(database.pool())?;
    let completed_scan = operations::scan::source_with_progress(
        &request.source_path,
        request.jobs,
        &excluded_paths,
        &|event| {
            progress(match event {
                operations::scan::ScanProgress::Started { files } => {
                    ScanProgressEvent::Started { files }
                }
                operations::scan::ScanProgress::Advanced => ScanProgressEvent::Advanced,
            });
        },
    )?;
    let source_path = Utf8PathBuf::from(completed_scan.source_root().as_str());
    let scan_run = completed_scan.scan_run();
    let observation_count = completed_scan.observations().len();
    let associated_rom_count =
        SourceRepository::new(database.pool()).replace_completed_scan(&completed_scan)?;
    Ok(SourceScanReport {
        source_path,
        scan_run,
        observation_count,
        associated_rom_count,
    })
}

pub fn build(
    database: &Database,
    request: &BuildWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    let source_root = request.source_path.canonicalize_utf8()?;
    crate::build::validation::ensure_sources_disjoint_from_destination(
        &[source_root.as_path()],
        &request.destination_path,
    )?;
    let plan = plan_build(
        database,
        &BuildPlanRequest {
            dat_path: request.dat_path.clone(),
            source_path: request.source_path.clone(),
            mode: request.mode,
            missing_policy: if request.strict {
                MissingContentPolicy::RequireComplete
            } else {
                MissingContentPolicy::AllowPartial
            },
        },
    )?;
    let unattempted = || {
        plan.groups
            .iter()
            .map(|group| ArtifactResult {
                path: request
                    .destination_path
                    .join(format!("{}.zip", group.path.as_str()))
                    .to_string(),
                outcome: ArtifactOutcome::Unattempted,
            })
            .collect::<Vec<_>>()
    };
    let build_report = plan.report.clone();
    let execution = if request.dry_run || plan.report.outcome != PlanOutcome::Ready {
        Ok((Vec::new(), unattempted()))
    } else {
        (|| {
            let source_root = request.source_path.canonicalize_utf8()?;
            crate::build::validation::ensure_sources_disjoint_from_destination(
                &[source_root.as_path()],
                &request.destination_path,
            )?;
            let results =
                write_plan_with_compression(&plan, &request.destination_path, request.compression)?;
            let paths = results
                .iter()
                .filter(|result| result.outcome == ArtifactOutcome::Completed)
                .map(|result| Utf8PathBuf::from(&result.path))
                .collect::<Vec<_>>();
            Ok((paths, results))
        })()
    };
    let (written_paths, artifact_results) = match execution {
        Ok(execution) => execution,
        Err(source) => {
            return Err(crate::Error::BuildWorkflow {
                report: Box::new(BuildWorkflowReport {
                    written_paths: Vec::new(),
                    artifact_results: unattempted(),
                    build_report,
                    scan_report: None,
                }),
                source: Box::new(source),
            });
        }
    };

    Ok(BuildWorkflowReport {
        written_paths,
        artifact_results,
        build_report,
        scan_report: None,
    })
}

pub fn plan_build(
    database: &Database,
    request: &BuildPlanRequest,
) -> crate::Result<crate::domain::BuildPlan> {
    let dat_selector = resolve_dat_selector(&request.dat_path);
    let source_root = request.source_path.canonicalize_utf8()?;
    let dat_roms =
        BuildRepository::new(database.pool()).load_dat_roms(dat_selector.repository_selector())?;
    let source_files = SourceRepository::new(database.pool())
        .load_source_files_for_root(&SourceRoot::new(source_root.to_string()))?;
    Ok(build_plan(
        &dat_roms,
        &source_files,
        &BuildRequest {
            dat_name: dat_selector.value().to_owned(),
            source_root: SourceRoot::new(source_root.to_string()),
            mode: request.mode,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: request.missing_policy,
        },
    ))
}

pub fn run(
    database: &Database,
    request: &RunWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    run_with_progress(database, request, &|_| {})
}

pub fn run_with_progress(
    database: &Database,
    request: &RunWorkflowRequest,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<BuildWorkflowReport> {
    crate::build::validation::ensure_sources_disjoint_from_destination(
        &[request.source_path.as_path()],
        &request.destination_path,
    )?;
    import_dat(
        database,
        &DatImportRequest {
            dat_path: request.dat_path.clone(),
        },
    )?;
    let scan_report =
        scan_source_with_progress(database, &source_scan_request_from_run(request), progress)?;
    match build(database, &build_workflow_request_from_run(request)) {
        Ok(mut report) => {
            report.scan_report = Some(scan_report);
            Ok(report)
        }
        Err(crate::Error::BuildWorkflow { mut report, source }) => {
            report.scan_report = Some(scan_report);
            Err(crate::Error::BuildWorkflow { report, source })
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_reports_scans_and_plan_diagnostics_when_an_artifact_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
        let dat_path = root.join("catalog.dat");
        let source_path = root.join("roms");
        let destination_path = root.join("output-file");
        std::fs::create_dir(&source_path)?;
        std::fs::write(source_path.join("game.rom"), b"abc")?;
        std::fs::write(
            &dat_path,
            r#"<?xml version="1.0"?><datafile><header><name>Fixture</name></header><game name="game"><rom name="game.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game></datafile>"#,
        )?;
        std::fs::write(&destination_path, b"not a directory")?;

        let database = Database::in_memory()?;
        let report = match run(
            &database,
            &RunWorkflowRequest {
                dat_path: dat_path.clone(),
                source_path: source_path.clone(),
                destination_path,
                mode: BuildMode::PerGame,
                compression: ZipCompression::Deflate,
                jobs: 1,
                dry_run: false,
                strict: false,
            },
        ) {
            Ok(_) => return Err("writing beneath a regular file unexpectedly succeeded".into()),
            Err(crate::Error::BuildWorkflow { report, .. }) => *report,
            Err(error) => return Err(error.into()),
        };
        let scan = report
            .scan_report
            .as_ref()
            .ok_or("completed scan missing from artifact failure report")?;
        assert_eq!(scan.observation_count, 1);
        assert_eq!(scan.associated_rom_count, 1);
        assert_eq!(report.build_report.matched_roms, 1);
        assert_eq!(report.artifact_results.len(), 1);
        assert_eq!(
            report.artifact_results[0].outcome,
            ArtifactOutcome::Unattempted
        );

        std::fs::write(source_path.join("game.rom"), b"different")?;
        let report = run(
            &database,
            &RunWorkflowRequest {
                dat_path,
                source_path,
                destination_path: root.join("dry-run-output"),
                mode: BuildMode::PerGame,
                compression: ZipCompression::Deflate,
                jobs: 1,
                dry_run: true,
                strict: false,
            },
        )?;
        let scan = report
            .scan_report
            .ok_or("successful run missing its scan report")?;
        assert_eq!(scan.observation_count, 1);
        assert_eq!(scan.associated_rom_count, 0);
        Ok(())
    }

    #[test]
    fn planning_from_in_memory_cache_does_not_create_or_write_outputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
        let dat_path = root.join("catalog.dat");
        let source_path = root.join("roms");
        let destination_path = root.join("outputs");
        std::fs::create_dir(&source_path)?;
        std::fs::write(source_path.join("game.rom"), b"abc")?;
        std::fs::write(
            &dat_path,
            r#"<?xml version="1.0"?><datafile><header><name>Fixture</name></header><game name="game"><rom name="game.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game></datafile>"#,
        )?;
        let database = Database::in_memory()?;
        import_dat(
            &database,
            &DatImportRequest {
                dat_path: dat_path.clone(),
            },
        )?;
        let progress_events = std::sync::Mutex::new(Vec::new());
        scan_source_with_progress(
            &database,
            &SourceScanRequest {
                source_path: source_path.clone(),
                jobs: 1,
            },
            &|event| {
                progress_events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(event);
            },
        )?;
        let progress_events = progress_events
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            progress_events,
            vec![
                ScanProgressEvent::Started { files: 1 },
                ScanProgressEvent::Advanced
            ]
        );

        let plan = plan_build(
            &database,
            &BuildPlanRequest {
                dat_path,
                source_path,
                mode: BuildMode::PerGame,
                missing_policy: MissingContentPolicy::RequireComplete,
            },
        )?;

        assert_eq!(plan.report.outcome, PlanOutcome::Ready);
        assert_eq!(plan.report.matched_roms, 1);
        assert_eq!(plan.groups.len(), 1);
        assert!(!destination_path.exists());
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BuildDatSelector {
    FileName(String),
    Name(String),
}

impl BuildDatSelector {
    fn repository_selector(&self) -> DataFileSelector<'_> {
        match self {
            Self::FileName(value) => DataFileSelector::FileName(value),
            Self::Name(value) => DataFileSelector::Name(value),
        }
    }

    fn value(&self) -> &str {
        match self {
            Self::FileName(value) | Self::Name(value) => value,
        }
    }
}

fn resolve_dat_selector(dat_path: &Utf8PathBuf) -> BuildDatSelector {
    dat_path.canonicalize_utf8().map_or_else(
        |_| BuildDatSelector::Name(dat_path.to_string()),
        |path| BuildDatSelector::FileName(path.to_string()),
    )
}

fn source_scan_request_from_run(request: &RunWorkflowRequest) -> SourceScanRequest {
    SourceScanRequest {
        source_path: request.source_path.clone(),
        jobs: request.jobs,
    }
}

fn build_workflow_request_from_run(request: &RunWorkflowRequest) -> BuildWorkflowRequest {
    BuildWorkflowRequest {
        dat_path: request.dat_path.clone(),
        source_path: request.source_path.clone(),
        destination_path: request.destination_path.clone(),
        mode: request.mode,
        compression: request.compression,
        dry_run: request.dry_run,
        strict: request.strict,
    }
}
