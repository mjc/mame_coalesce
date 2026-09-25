use camino::Utf8PathBuf;

use crate::{
    build::{planner::plan_build as build_plan, write_plan_with_compression},
    database::Database,
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
    let (written_paths, artifact_results) =
        if request.dry_run || plan.report.outcome != PlanOutcome::Ready {
            (Vec::new(), unattempted())
        } else {
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
            (paths, results)
        };

    Ok(BuildWorkflowReport {
        written_paths,
        artifact_results,
        build_report,
    })
}

/// Resolve cached catalog and source evidence without writing outputs or rendering to a terminal.
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
    let plan = build_plan(
        &dat_roms,
        &source_files,
        &BuildRequest {
            dat_name: dat_selector.value().to_owned(),
            source_root: SourceRoot::new(source_root.to_string()),
            mode: request.mode,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: request.missing_policy,
        },
    );
    Ok(plan)
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
    scan_source_with_progress(database, &source_scan_request_from_run(request), progress)?;
    build(database, &build_workflow_request_from_run(request))
}

#[cfg(test)]
mod tests {
    use super::*;

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
