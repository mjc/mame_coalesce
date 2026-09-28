use std::collections::{HashMap, HashSet};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    QueryableByName,
    prelude::*,
    sql_query,
    sql_types::{Binary, Nullable, Text},
};
use serde::Serialize;

use crate::{
    build::write_plan_with_container,
    database::Database,
    disk::{
        self, DiskDigestScope, DiskIdentitySha1, DiskName, DiskObservation, DiskRequirement,
        DiskVerificationState, ParentDiskName,
    },
    domain::{
        ArtifactOutcome, ArtifactResult, AuditReport, BuildMode, BuildReport, BuildRequest,
        CatalogKey, CatalogScope, ImportRunKey, MatchingPolicy, MissingContentPolicy,
        ObservationBasis, OutputContainer, PlanOutcome, PublishingSourceKey,
        RelationshipAssertionKey, RelationshipClaim, RelationshipExplanation, RelationshipReview,
        ScanRunKey, SetSelection, SnapshotKey, SourceRoot, ZipCompression,
    },
    operations,
    storage::repositories::{BuildRepository, DataFileSelector, SourceRepository},
};

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs needed to import a catalog document into the local cache.
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
/// Format of a supported catalog document.
pub enum CatalogDocumentFormat {
    Logiqx,
    MameListXml,
    MameSoftwareListXml,
    ClrMamePro,
    NoIntroPcXml,
}

impl CatalogDocumentFormat {
    /// Return the stable identifier used when reporting this document format.
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
/// Result status for a catalog import attempt.
pub enum CatalogImportStatus {
    Succeeded,
    Failed,
}

impl CatalogImportStatus {
    /// Return the stable lowercase identifier for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Summary of a catalog import and its diagnostics.
pub struct CatalogImportReport {
    pub snapshot_key: Option<SnapshotKey>,
    pub run_key: ImportRunKey,
    pub status: CatalogImportStatus,
    pub diagnostic_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs needed to import a Logiqx DAT file.
pub struct DatImportRequest {
    pub dat_path: Utf8PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Identifies the imported DAT record in the cache.
pub struct DatImportReport {
    pub data_file_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs needed to scan a source directory or its supported archives.
pub struct SourceScanRequest {
    pub source_path: Utf8PathBuf,
    pub jobs: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Select whether a source scan rehashes all files or opts into bare-file cache reuse.
pub enum ScanCachePolicy {
    /// Read and hash all bare files; archives are always rescanned under either policy.
    #[default]
    RehashAll,
    /// Reuse unchanged bare files, except paths explicitly listed for a forced rehash.
    ReuseUnchangedBareFiles { force_rehash: Vec<Utf8PathBuf> },
}

/// Ordered root paths supplied by a caller. The first path retains the legacy positional-root
/// role; additional paths are considered in the order supplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRootSelection {
    pub primary: Utf8PathBuf,
    pub additional: Vec<Utf8PathBuf>,
}

impl SourceRootSelection {
    #[must_use]
    pub const fn single(primary: Utf8PathBuf) -> Self {
        Self {
            primary,
            additional: Vec::new(),
        }
    }

    fn paths(&self) -> impl Iterator<Item = &Utf8PathBuf> {
        std::iter::once(&self.primary).chain(self.additional.iter())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Counts and identity for one completed source scan.
pub struct SourceScanReport {
    pub source_path: Utf8PathBuf,
    pub scan_run: ScanRunKey,
    pub observation_count: usize,
    pub associated_rom_count: usize,
    /// Number of bare-file observations reused under the explicit cache policy.
    pub reused_bare_files: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Progress notifications emitted while scanning source files.
pub enum ScanProgressEvent {
    Started { files: u64 },
    Advanced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs needed to plan and optionally write build artifacts.
pub struct BuildWorkflowRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub destination_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub compression: ZipCompression,
    pub dry_run: bool,
    pub strict: bool,
    pub set_selection: SetSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Build diagnostics, artifact outcomes, and any paths visibly written.
pub struct BuildWorkflowReport {
    pub written_paths: Vec<Utf8PathBuf>,
    pub artifact_results: Vec<ArtifactResult>,
    pub build_report: BuildReport,
    /// Scan results for every selected root. `scan_report` is retained as the legacy primary-root view.
    pub scan_reports: Vec<SourceScanReport>,
    pub scan_report: Option<SourceScanReport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs for planning a build from already-cached DAT and scan data.
pub struct BuildPlanRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub matching_policy: MatchingPolicy,
    pub missing_policy: MissingContentPolicy,
    pub set_selection: SetSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRefresh {
    Cached,
    Refresh,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub refresh: AuditRefresh,
    pub matching_policy: MatchingPolicy,
    pub jobs: usize,
    pub set_selection: SetSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs for the one-shot import, scan, plan, and build workflow.
pub struct RunWorkflowRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub destination_path: Utf8PathBuf,
    pub mode: BuildMode,
    pub compression: ZipCompression,
    pub jobs: usize,
    pub dry_run: bool,
    pub strict: bool,
    pub set_selection: SetSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Inputs for auditing declared disk requirements against a source tree.
pub struct DiskAuditRequest {
    pub catalog_key: String,
    pub source_path: Utf8PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
/// Serializable result of auditing all declared disks in a catalog snapshot.
pub struct DiskAuditReport {
    pub schema_version: u32,
    pub catalog_key: String,
    pub snapshot_key: String,
    pub source_path: Utf8PathBuf,
    pub disks: Vec<DiskAuditEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
/// Audit state and catalog identity for one declared disk requirement.
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
/// Verification state assigned to a disk requirement during an audit.
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

/// Parse a Logiqx DAT file and store its contents in the database cache.
pub fn import_dat(
    database: &Database,
    request: &DatImportRequest,
) -> crate::Result<DatImportReport> {
    operations::parse_and_insert_datfile(&request.dat_path, database.pool())
        .map(|data_file_id| DatImportReport { data_file_id })
}

/// Import a supported catalog document and report its snapshot and diagnostics.
pub fn import_catalog(
    database: &Database,
    request: &CatalogImportRequest,
) -> crate::Result<CatalogImportReport> {
    crate::storage::catalog_import::import(database.pool(), request)
}

/// Persist a derived candidate or user-authored relationship without conflating it with a source claim.
pub fn record_relationship(
    database: &Database,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    crate::storage::relationships::record_claim(database.pool(), claim)
}

/// Append a review event. Earlier reviews remain available as durable history.
pub fn review_relationship(
    database: &Database,
    assertion_key: &RelationshipAssertionKey,
    review: &RelationshipReview,
) -> crate::Result<()> {
    crate::storage::relationships::review_claim(database.pool(), assertion_key, review)
}

/// Explain all known source claims, derived candidates, and user conclusions in stable order.
pub fn explain_relationships(database: &Database) -> crate::Result<Vec<RelationshipExplanation>> {
    crate::storage::relationships::explain_all(database.pool())
}

/// Scan a source tree, hash discovered content, and persist the completed scan.
pub fn scan_source(
    database: &Database,
    request: &SourceScanRequest,
) -> crate::Result<SourceScanReport> {
    scan_source_with_progress(database, request, &|_| {})
}

/// Scan and persist a source tree while reporting file-count progress to the caller.
pub fn scan_source_with_progress(
    database: &Database,
    request: &SourceScanRequest,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<SourceScanReport> {
    scan_source_with_policy_and_progress(database, request, ScanCachePolicy::RehashAll, progress)
}

/// Scan one source root under an explicit cache freshness policy.
pub fn scan_source_with_policy(
    database: &Database,
    request: &SourceScanRequest,
    cache_policy: ScanCachePolicy,
) -> crate::Result<SourceScanReport> {
    scan_source_with_policy_and_progress(database, request, cache_policy, &|_| {})
}

/// Scan one source root under an explicit cache policy and report progress events.
pub fn scan_source_with_policy_and_progress(
    database: &Database,
    request: &SourceScanRequest,
    cache_policy: ScanCachePolicy,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<SourceScanReport> {
    let excluded_paths = crate::storage::db::database_file_paths(database.pool())?;
    let source_root_path = request.source_path.canonicalize_utf8()?;
    let source_root = SourceRoot::new(source_root_path.to_string());
    let repository = SourceRepository::new(database.pool());
    let (cached_files, forced_paths) = match cache_policy {
        ScanCachePolicy::RehashAll => (Vec::new(), std::collections::BTreeSet::new()),
        ScanCachePolicy::ReuseUnchangedBareFiles { force_rehash } => {
            let cached_files = repository.load_source_files_for_root(&source_root)?;
            let forced_paths = force_rehash
                .into_iter()
                .map(|path| {
                    let file_name = path.file_name().ok_or_else(|| {
                        crate::Error::InvalidPath(format!(
                            "forced rehash path is not a file: {path}"
                        ))
                    })?;
                    let parent = path
                        .parent()
                        .unwrap_or_else(|| Utf8Path::new("."))
                        .canonicalize_utf8()?;
                    Ok(parent.join(file_name))
                })
                .collect::<crate::Result<std::collections::BTreeSet<_>>>()?;
            if forced_paths
                .iter()
                .any(|path| !path.starts_with(&source_root_path))
            {
                return Err(crate::Error::InvalidPath(
                    "forced rehash path is outside the selected source root".to_owned(),
                ));
            }
            (cached_files, forced_paths)
        }
    };
    let completed_scan = operations::scan::source_with_cache_and_progress(
        &request.source_path,
        request.jobs,
        &excluded_paths,
        &cached_files,
        &forced_paths,
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
    let reused_bare_files = completed_scan
        .observations()
        .iter()
        .filter(|observation| {
            observation.scan_provenance == crate::domain::ScanProvenance::ReusedStatValidatedV1
        })
        .count();
    let associated_rom_count = repository.replace_completed_scan(&completed_scan)?;
    Ok(SourceScanReport {
        source_path,
        scan_run,
        observation_count,
        associated_rom_count,
        reused_bare_files,
    })
}

/// Plan a build from cached data and write artifacts unless dry-run or blocked.
///
/// Artifact-level failures are returned in the report. Once planning succeeds, output
/// setup and execution errors carry the partial report in [`crate::Error::BuildWorkflow`].
/// Scan every distinct canonical root before replacing any cached scope. If any scan fails,
/// none of the requested roots are refreshed. Successful scans commit together in one DB tx.
pub fn scan_sources_with_progress(
    database: &Database,
    selection: &SourceRootSelection,
    jobs: usize,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<Vec<SourceScanReport>> {
    let roots = canonical_roots(selection)?;
    let excluded_paths = crate::storage::db::database_file_paths(database.pool())?;
    let mut completed_scans = Vec::with_capacity(roots.len());
    for (path, _) in &roots {
        let scan = operations::scan::source_with_progress(path, jobs, &excluded_paths, &|event| {
            progress(match event {
                operations::scan::ScanProgress::Started { files } => {
                    ScanProgressEvent::Started { files }
                }
                operations::scan::ScanProgress::Advanced => ScanProgressEvent::Advanced,
            });
        })?;
        completed_scans.push(scan);
    }
    let associated =
        SourceRepository::new(database.pool()).replace_completed_scans(&completed_scans)?;
    Ok(completed_scans
        .iter()
        .zip(associated)
        .map(|(scan, associated_rom_count)| SourceScanReport {
            source_path: Utf8PathBuf::from(scan.source_root().as_str()),
            scan_run: scan.scan_run(),
            observation_count: scan.observations().len(),
            associated_rom_count,
            reused_bare_files: 0,
        })
        .collect())
}

pub fn scan_sources(
    database: &Database,
    selection: &SourceRootSelection,
    jobs: usize,
) -> crate::Result<Vec<SourceScanReport>> {
    scan_sources_with_progress(database, selection, jobs, &|_| {})
}
pub fn build(
    database: &Database,
    request: &BuildWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    build_with_roots(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
    )
}

pub fn build_with_container(
    database: &Database,
    request: &BuildWorkflowRequest,
    container: OutputContainer,
) -> crate::Result<BuildWorkflowReport> {
    build_with_roots_and_container(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
        container,
    )
}

pub fn build_with_roots(
    database: &Database,
    request: &BuildWorkflowRequest,
    selection: &SourceRootSelection,
) -> crate::Result<BuildWorkflowReport> {
    build_with_roots_and_container(database, request, selection, OutputContainer::Zip)
}

pub fn build_with_roots_and_container(
    database: &Database,
    request: &BuildWorkflowRequest,
    selection: &SourceRootSelection,
    container: OutputContainer,
) -> crate::Result<BuildWorkflowReport> {
    let roots = canonical_roots(selection)?;
    let canonical_paths = roots
        .iter()
        .map(|(path, _)| path.as_path())
        .collect::<Vec<_>>();
    crate::build::validation::ensure_sources_disjoint_from_destination(
        &canonical_paths,
        &request.destination_path,
    )?;
    let plan = plan_build_with_roots(
        database,
        &BuildPlanRequest {
            dat_path: request.dat_path.clone(),
            source_path: selection.primary.clone(),
            mode: request.mode,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: if request.strict {
                MissingContentPolicy::RequireComplete
            } else {
                MissingContentPolicy::AllowPartial
            },
            set_selection: request.set_selection.clone(),
        },
        selection,
    )?;
    let unattempted = || {
        plan.groups
            .iter()
            .map(|group| ArtifactResult {
                path: request
                    .destination_path
                    .join(match container {
                        OutputContainer::Zip => format!("{}.zip", group.path.as_str()),
                        OutputContainer::Directory => group.path.as_str().to_owned(),
                    })
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
            crate::build::validation::ensure_sources_disjoint_from_destination(
                &canonical_paths,
                &request.destination_path,
            )?;
            let results = write_plan_with_container(
                &plan,
                &request.destination_path,
                container,
                request.compression,
            )?;
            let paths = results
                .iter()
                .filter(|result| {
                    matches!(
                        &result.outcome,
                        ArtifactOutcome::Completed
                            | ArtifactOutcome::ReplacedButNotDurable { .. }
                            | ArtifactOutcome::CompletedWithWarning { .. }
                    )
                })
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
                    scan_reports: Vec::new(),
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
        scan_reports: Vec::new(),
        scan_report: None,
    })
}

/// Create a build plan from cached DAT and source-scan data without writing outputs.
pub fn plan_build(
    database: &Database,
    request: &BuildPlanRequest,
) -> crate::Result<crate::domain::BuildPlan> {
    plan_build_with_roots(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
    )
}

pub fn plan_build_with_roots(
    database: &Database,
    request: &BuildPlanRequest,
    selection: &SourceRootSelection,
) -> crate::Result<crate::domain::BuildPlan> {
    let dat_selector = resolve_dat_selector(&request.dat_path);
    let roots = canonical_roots(selection)?;
    let source_roots = roots
        .iter()
        .map(|(_, root)| root.clone())
        .collect::<Vec<_>>();
    let build_repository = BuildRepository::new(database.pool());
    let available_set_names =
        build_repository.load_set_names(dat_selector.repository_selector())?;
    let dat_roms = build_repository.load_dat_roms(dat_selector.repository_selector())?;
    let repository = SourceRepository::new(database.pool());
    let source_files = if let [source_root] = source_roots.as_slice() {
        repository.load_source_files_for_root(source_root)?
    } else {
        repository.load_source_files_for_roots(&source_roots)?
    };
    let plan = crate::build::planner::plan_build_with_set_names(
        &dat_roms,
        &available_set_names,
        &source_files,
        &BuildRequest {
            dat_name: dat_selector.value().to_owned(),
            source_roots,
            mode: request.mode,
            matching_policy: request.matching_policy,
            missing_policy: request.missing_policy,
            set_selection: request.set_selection.clone(),
        },
    );
    Ok(plan)
}

pub fn audit(database: &Database, request: &AuditRequest) -> crate::Result<AuditReport> {
    audit_with_progress(database, request, &|_| {})
}

pub fn audit_with_progress(
    database: &Database,
    request: &AuditRequest,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<AuditReport> {
    audit_with_roots_and_progress(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
        progress,
    )
}

pub fn audit_with_roots(
    database: &Database,
    request: &AuditRequest,
    selection: &SourceRootSelection,
) -> crate::Result<AuditReport> {
    audit_with_roots_and_progress(database, request, selection, &|_| {})
}

pub fn audit_with_roots_and_progress(
    database: &Database,
    request: &AuditRequest,
    selection: &SourceRootSelection,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<AuditReport> {
    let selection_issues =
        catalog_set_selection_issues(database, &request.dat_path, &request.set_selection)?;
    if !selection_issues.is_empty() {
        return Ok(AuditReport::new(
            ObservationBasis::Cached,
            invalid_set_selection_report(selection_issues),
        ));
    }

    let observation_basis = match request.refresh {
        AuditRefresh::Cached => ObservationBasis::Cached,
        AuditRefresh::Refresh => {
            let scans = scan_sources_with_progress(database, selection, request.jobs, progress)?;
            if let [scan] = scans.as_slice() {
                ObservationBasis::FreshScan {
                    scan_run: scan.scan_run,
                }
            } else {
                ObservationBasis::FreshScans {
                    scan_runs: scans
                        .into_iter()
                        .map(|scan| crate::domain::RootScanRun {
                            source_root: SourceRoot::new(scan.source_path.to_string()),
                            scan_run: scan.scan_run,
                        })
                        .collect(),
                }
            }
        }
    };
    let plan = plan_build_with_roots(
        database,
        &BuildPlanRequest {
            dat_path: request.dat_path.clone(),
            source_path: selection.primary.clone(),
            mode: BuildMode::ParentBundles,
            matching_policy: request.matching_policy,
            missing_policy: MissingContentPolicy::AllowPartial,
            set_selection: request.set_selection.clone(),
        },
        selection,
    )?;
    Ok(AuditReport::new(observation_basis, plan.report))
}

/// Import a DAT, scan its source, plan the build, and write eligible artifacts.
pub fn run(
    database: &Database,
    request: &RunWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    run_with_progress(database, request, &|_| {})
}

/// Run a complete workflow using the selected output container.
pub fn run_with_container(
    database: &Database,
    request: &RunWorkflowRequest,
    container: OutputContainer,
) -> crate::Result<BuildWorkflowReport> {
    run_with_roots_and_container_and_progress(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
        container,
        &|_| {},
    )
}

/// Run the complete workflow and report source-scan progress to the caller.
pub fn run_with_progress(
    database: &Database,
    request: &RunWorkflowRequest,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<BuildWorkflowReport> {
    run_with_roots_and_progress(
        database,
        request,
        &SourceRootSelection::single(request.source_path.clone()),
        progress,
    )
}

pub fn run_with_roots(
    database: &Database,
    request: &RunWorkflowRequest,
    selection: &SourceRootSelection,
) -> crate::Result<BuildWorkflowReport> {
    run_with_roots_and_progress(database, request, selection, &|_| {})
}

pub fn run_with_roots_and_progress(
    database: &Database,
    request: &RunWorkflowRequest,
    selection: &SourceRootSelection,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<BuildWorkflowReport> {
    run_with_roots_and_container_and_progress(
        database,
        request,
        selection,
        OutputContainer::Zip,
        progress,
    )
}

pub fn run_with_roots_and_container_and_progress(
    database: &Database,
    request: &RunWorkflowRequest,
    selection: &SourceRootSelection,
    container: OutputContainer,
    progress: &(impl Fn(ScanProgressEvent) + Sync),
) -> crate::Result<BuildWorkflowReport> {
    let roots = canonical_roots(selection)?;
    let canonical_paths = roots
        .iter()
        .map(|(path, _)| path.as_path())
        .collect::<Vec<_>>();
    crate::build::validation::ensure_sources_disjoint_from_destination(
        &canonical_paths,
        &request.destination_path,
    )?;
    import_dat(
        database,
        &DatImportRequest {
            dat_path: request.dat_path.clone(),
        },
    )?;
    let selection_issues =
        catalog_set_selection_issues(database, &request.dat_path, &request.set_selection)?;
    if !selection_issues.is_empty() {
        return Ok(BuildWorkflowReport {
            written_paths: Vec::new(),
            artifact_results: Vec::new(),
            build_report: invalid_set_selection_report(selection_issues),
            scan_reports: Vec::new(),
            scan_report: None,
        });
    }
    let scan_reports = scan_sources_with_progress(database, selection, request.jobs, progress)?;
    let scan_report = scan_reports.first().cloned().ok_or_else(|| {
        crate::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "at least one source root is required",
        ))
    })?;
    match build_with_roots_and_container(
        database,
        &build_workflow_request_from_run(request),
        selection,
        container,
    ) {
        Ok(mut report) => {
            report.scan_report = Some(scan_report);
            report.scan_reports = scan_reports;
            Ok(report)
        }
        Err(crate::Error::BuildWorkflow { mut report, source }) => {
            report.scan_report = Some(scan_report);
            report.scan_reports = scan_reports;
            Err(crate::Error::BuildWorkflow { report, source })
        }
        Err(source) if scan_reports.len() == 1 => Err(crate::Error::RunWorkflow {
            scan_report,
            source: Box::new(source),
        }),
        Err(source) => Err(crate::Error::RunWorkflowWithRoots {
            scan_reports,
            source: Box::new(source),
        }),
    }
}

fn catalog_set_selection_issues(
    database: &Database,
    dat_path: &Utf8PathBuf,
    selection: &SetSelection,
) -> crate::Result<Vec<crate::domain::SetSelectionIssue>> {
    if matches!(selection, SetSelection::All) {
        return Ok(Vec::new());
    }
    let dat_selector = resolve_dat_selector(dat_path);
    let available =
        BuildRepository::new(database.pool()).load_set_names(dat_selector.repository_selector())?;
    Ok(crate::build::planner::validate_set_selection(
        selection, &available,
    ))
}

fn invalid_set_selection_report(issues: Vec<crate::domain::SetSelectionIssue>) -> BuildReport {
    BuildReport {
        outcome: crate::domain::PlanOutcome::Blocked(
            crate::domain::PlanBlockReason::InvalidSetSelection,
        ),
        set_selection_issues: issues,
        ..BuildReport::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies one-shot runs retain scan and plan diagnostics when artifact output fails.
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
                set_selection: SetSelection::All,
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
                set_selection: SetSelection::All,
            },
        )?;
        let scan = report
            .scan_report
            .ok_or("successful run missing its scan report")?;
        assert_eq!(scan.observation_count, 1);
        assert_eq!(scan.associated_rom_count, 0);
        Ok(())
    }

    /// Verifies completed scan results survive build failures before planning completes.
    #[test]
    fn run_preserves_scan_report_when_build_fails_before_planning()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
        let dat_path = root.join("catalog.dat");
        let source_path = root.join("roms");
        std::fs::create_dir(&source_path)?;
        std::fs::write(source_path.join("game.rom"), b"abc")?;
        std::fs::write(
            &dat_path,
            r#"<?xml version="1.0"?><datafile><header><name>Fixture</name></header><game name="game"><rom name="game.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game></datafile>"#,
        )?;

        let database = Database::in_memory()?;
        let removed = std::sync::atomic::AtomicBool::new(false);
        let result = run_with_progress(
            &database,
            &RunWorkflowRequest {
                dat_path,
                source_path: source_path.clone(),
                destination_path: root.join("output"),
                mode: BuildMode::PerGame,
                compression: ZipCompression::Deflate,
                jobs: 1,
                dry_run: false,
                strict: false,
                set_selection: SetSelection::All,
            },
            &|event| {
                if matches!(event, ScanProgressEvent::Advanced) {
                    removed.store(
                        std::fs::remove_dir_all(&source_path).is_ok(),
                        std::sync::atomic::Ordering::Relaxed,
                    );
                }
            },
        );
        let Err(error) = result else {
            return Err("removing the source after scanning must fail planning".into());
        };
        assert!(removed.load(std::sync::atomic::Ordering::Relaxed));

        match error {
            crate::Error::RunWorkflow {
                scan_report,
                source,
            } => {
                assert_eq!(scan_report.observation_count, 1);
                assert_eq!(scan_report.associated_rom_count, 1);
                assert!(matches!(*source, crate::Error::Io(_)));
            }
            error => return Err(format!("unexpected error: {error}").into()),
        }
        Ok(())
    }

    /// Verifies the scan reports progress and in-memory planning creates no output files.
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
                matching_policy: MatchingPolicy::Sha1Compatibility,
                missing_policy: MissingContentPolicy::RequireComplete,
                set_selection: SetSelection::All,
            },
        )?;

        assert_eq!(plan.report.outcome, PlanOutcome::Ready);
        assert_eq!(plan.report.matched_roms, 1);
        assert_eq!(plan.groups.len(), 1);
        assert!(!destination_path.exists());
        Ok(())
    }

    #[test]
    fn set_selection_uses_empty_catalog_sets_and_rejects_typos_before_scanning()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
        let dat_path = root.join("catalog.dat");
        let source_path = root.join("roms");
        std::fs::create_dir(&source_path)?;
        std::fs::write(
            &dat_path,
            r#"<?xml version="1.0"?><datafile><header><name>Fixture</name></header><game name="empty-set"><description>Empty</description></game></datafile>"#,
        )?;
        let database = Database::in_memory()?;
        import_dat(
            &database,
            &DatImportRequest {
                dat_path: dat_path.clone(),
            },
        )?;

        let empty_set_plan = plan_build(
            &database,
            &BuildPlanRequest {
                dat_path: dat_path.clone(),
                source_path: source_path.clone(),
                mode: BuildMode::PerGame,
                matching_policy: MatchingPolicy::Sha1Compatibility,
                missing_policy: MissingContentPolicy::AllowPartial,
                set_selection: SetSelection::exact_names([crate::domain::SetName::new(
                    "empty-set",
                )]),
            },
        )?;
        assert_eq!(empty_set_plan.report.outcome, PlanOutcome::Ready);
        assert!(empty_set_plan.report.set_selection_issues.is_empty());

        let progress_events = std::sync::Mutex::new(Vec::new());
        let request = RunWorkflowRequest {
            dat_path: dat_path.clone(),
            source_path: source_path.clone(),
            destination_path: root.join("output"),
            mode: BuildMode::PerGame,
            compression: ZipCompression::Deflate,
            jobs: 1,
            dry_run: false,
            strict: false,
            set_selection: SetSelection::exact_names([crate::domain::SetName::new("typo")]),
        };
        let run_report = run_with_roots_and_progress(
            &database,
            &request,
            &SourceRootSelection::single(source_path.clone()),
            &|event| {
                progress_events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(event);
            },
        )?;
        assert_eq!(
            run_report.build_report.outcome,
            PlanOutcome::Blocked(crate::domain::PlanBlockReason::InvalidSetSelection)
        );
        assert!(
            progress_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );

        let audit_progress_events = std::sync::Mutex::new(Vec::new());
        let audit = audit_with_roots_and_progress(
            &database,
            &AuditRequest {
                dat_path,
                source_path: source_path.clone(),
                refresh: AuditRefresh::Refresh,
                matching_policy: MatchingPolicy::Sha1Compatibility,
                jobs: 1,
                set_selection: SetSelection::exact_names([crate::domain::SetName::new("typo")]),
            },
            &SourceRootSelection::single(source_path),
            &|event| {
                audit_progress_events
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(event);
            },
        )?;
        assert_eq!(
            audit.report().outcome,
            PlanOutcome::Blocked(crate::domain::PlanBlockReason::InvalidSetSelection)
        );
        assert_eq!(audit.report().set_selection_issues.len(), 1);
        assert!(
            audit_progress_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        );
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

fn build_workflow_request_from_run(request: &RunWorkflowRequest) -> BuildWorkflowRequest {
    BuildWorkflowRequest {
        dat_path: request.dat_path.clone(),
        source_path: request.source_path.clone(),
        destination_path: request.destination_path.clone(),
        mode: request.mode,
        compression: request.compression,
        dry_run: request.dry_run,
        strict: request.strict,
        set_selection: request.set_selection.clone(),
    }
}

fn canonical_roots(
    selection: &SourceRootSelection,
) -> crate::Result<Vec<(Utf8PathBuf, SourceRoot)>> {
    let mut roots = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in selection.paths() {
        let canonical_path = path.canonicalize_utf8()?;
        let root = SourceRoot::new(canonical_path.to_string());
        if seen.insert(root.clone()) {
            roots.push((canonical_path, root));
        }
    }
    if roots.is_empty() {
        return Err(crate::Error::InvalidPath(
            "at least one source root is required".to_owned(),
        ));
    }
    Ok(roots)
}
