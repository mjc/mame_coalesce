use camino::Utf8PathBuf;
use diesel::{
    QueryableByName,
    prelude::*,
    sql_query,
    sql_types::{Binary, Nullable, Text},
};
use log::{info, warn};
use serde::Serialize;

use crate::{
    build::{planner::plan_build, writer::write_plan_with_compression},
    database::Database,
    disk::{
        self, DiskDigestScope, DiskIdentitySha1, DiskName, DiskObservation, DiskRequirement,
        DiskVerificationState, ParentDiskName,
    },
    domain::{
        BuildMode, BuildReport, BuildRequest, CatalogKey, CatalogScope, ImportRunKey,
        PublishingSourceKey, SnapshotKey, ZipCompression,
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
    pub build_report: BuildReport,
    pub exit_code: i32,
    pub mode: BuildMode,
    pub compression: ZipCompression,
    pub dry_run: bool,
    pub strict: bool,
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
    pub parent_disk: Option<String>,
    pub expected_logical_sha1: Option<String>,
    pub state: DiskAuditState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskAuditState {
    Missing,
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

#[derive(QueryableByName)]
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
}

/// Audit disk requirements from the latest published catalog snapshot against a scanned source.
/// Scanned SHA-1 values are deliberately ignored: they identify container bytes, not CHD identity.
pub fn audit_disks(
    database: &Database,
    request: &DiskAuditRequest,
) -> crate::Result<DiskAuditReport> {
    let source_path = request.source_path.canonicalize_utf8()?;
    let mut conn = database.pool().get()?;
    let snapshot = sql_query(
        "SELECT publication.snapshot_key FROM snapshot_publications AS publication \
         WHERE publication.catalog_key = ? \
         ORDER BY publication.published_at DESC, publication.snapshot_key DESC LIMIT 1",
    )
    .bind::<Text, _>(&request.catalog_key)
    .get_result::<PublishedDiskSnapshotRow>(&mut conn)?;
    let requirements = sql_query(
        "SELECT asset.set_name, asset.asset_name, asset.sha1, asset.evidence_scope, asset.merge_name \
         FROM asset_requirements AS asset \
         WHERE asset.snapshot_key = ? AND asset.role = 'disk' \
         ORDER BY asset.set_name, asset.component_order",
    )
    .bind::<Text, _>(&snapshot.snapshot_key)
    .load::<DiskRequirementRow>(&mut conn)?;
    drop(conn);

    let source_files = SourceRepository::new(database.pool()).load_source_files()?;
    let disks = requirements
        .into_iter()
        .map(|row| {
            let expected = row
                .sha1
                .map(|bytes| {
                    bytes.try_into().map_err(|bytes: Vec<u8>| {
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
            let matching_file = source_files
                .iter()
                .filter_map(|file| {
                    let crate::domain::SourceLocation::BareFile { path } = &file.location else {
                        return None;
                    };
                    let path = camino::Utf8Path::new(path);
                    if !path.starts_with(&source_path) {
                        return None;
                    }
                    let file_name = path.file_name()?;
                    let matches = file_name.eq_ignore_ascii_case(&row.asset_name)
                        || file_name.eq_ignore_ascii_case(&format!("{}.chd", row.asset_name));
                    matches.then_some((file, file_name.to_owned()))
                })
                .min_by_key(|(file, file_name)| {
                    (
                        u8::from(!file_name.to_ascii_lowercase().ends_with(".chd")),
                        file.location.path().to_owned(),
                    )
                })
                .map(|(file, _)| file);
            let observation = matching_file.map_or(DiskObservation::Missing, |file| {
                let file_name = camino::Utf8Path::new(file.location.path())
                    .file_name()
                    .unwrap_or_default();
                if file_name.to_ascii_lowercase().ends_with(".chd") {
                    DiskObservation::ContainerPresent { byte_sha1: None }
                } else {
                    DiskObservation::UnsupportedContainer
                }
            });
            let state = DiskAuditState::from(disk::audit_disk(&requirement, observation));
            Ok(DiskAuditEntry {
                set_name: row.set_name,
                disk_name: row.asset_name,
                parent_disk: requirement
                    .parent()
                    .map(|parent| parent.as_str().to_owned()),
                expected_logical_sha1: expected.map(hex::encode),
                state,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(DiskAuditReport {
        schema_version: 1,
        catalog_key: request.catalog_key.clone(),
        snapshot_key: snapshot.snapshot_key,
        source_path,
        disks,
    })
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
    operations::source(&request.source_path, request.jobs, database.pool())
        .map(|source_path| SourceScanReport { source_path })
}

pub fn build(
    database: &Database,
    request: &BuildWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    let dat_selector = resolve_dat_selector(&request.dat_path);
    let source_root = request.source_path.canonicalize_utf8()?;
    let dat_roms =
        BuildRepository::new(database.pool()).load_dat_roms(dat_selector.repository_selector())?;
    let source_files = SourceRepository::new(database.pool()).load_source_files()?;
    let plan = plan_build(
        &dat_roms,
        &source_files,
        &BuildRequest {
            dat_name: dat_selector.value().to_owned(),
            source_root: source_root.to_string(),
            mode: request.mode,
            dry_run: request.dry_run,
            strict: request.strict,
        },
    );
    report_build_outcome(&plan.report);
    let exit_code = plan.report.exit_code;
    let build_report = plan.report.clone();
    let written_paths =
        write_plan_with_compression(&plan, &request.destination_path, request.compression)?;

    Ok(BuildWorkflowReport {
        written_paths,
        build_report,
        exit_code,
        mode: request.mode,
        compression: request.compression,
        dry_run: request.dry_run,
        strict: request.strict,
    })
}

pub fn run(
    database: &Database,
    request: &RunWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    import_dat(
        database,
        &DatImportRequest {
            dat_path: request.dat_path.clone(),
        },
    )?;
    scan_source(database, &source_scan_request_from_run(request))?;
    build(database, &build_workflow_request_from_run(request))
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

fn report_build_outcome(report: &BuildReport) {
    info!("matched {} ROMs", report.matched_roms);

    if !report.missing_roms.is_empty() {
        warn!("{} ROMs are missing", report.missing_roms.len());
        for missing in &report.missing_roms {
            warn!(
                "missing ROM: game={} rom={} sha1={}",
                missing.game_name,
                missing.rom_name,
                missing
                    .sha1
                    .map_or_else(|| "not supplied".to_owned(), hex::encode)
            );
        }
    }

    if !report.duplicate_matches.is_empty() {
        warn!(
            "{} ROMs had duplicate source matches",
            report.duplicate_matches.len()
        );
        for duplicate in &report.duplicate_matches {
            warn!(
                "duplicate ROM match: rom={} selected={} candidates={}",
                duplicate.rom_name,
                duplicate.selected.display_name(),
                duplicate.candidates.len()
            );
        }
    }
}
