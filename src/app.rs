use camino::Utf8PathBuf;
use log::{info, warn};

use crate::{
    build::{planner::plan_build, writer::write_plan_with_compression},
    db::Pool,
    domain::{BuildMode, BuildReport, BuildRequest, ZipCompression},
    operations,
    storage::repositories::{BuildRepository, DataFileSelector, SourceRepository},
};

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

pub fn import_dat(pool: &Pool, request: &DatImportRequest) -> crate::Result<DatImportReport> {
    operations::parse_and_insert_datfile(&request.dat_path, pool)
        .map(|data_file_id| DatImportReport { data_file_id })
}

pub fn scan_source(pool: &Pool, request: &SourceScanRequest) -> crate::Result<SourceScanReport> {
    operations::source(&request.source_path, request.jobs, pool)
        .map(|source_path| SourceScanReport { source_path })
}

pub fn build(pool: &Pool, request: &BuildWorkflowRequest) -> crate::Result<BuildWorkflowReport> {
    let dat_selector = resolve_dat_selector(&request.dat_path);
    let source_root = request.source_path.canonicalize_utf8()?;
    let dat_roms = BuildRepository::new(pool).load_dat_roms(dat_selector.repository_selector())?;
    let source_files = SourceRepository::new(pool).load_source_files()?;
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

pub fn run(pool: &Pool, request: &RunWorkflowRequest) -> crate::Result<BuildWorkflowReport> {
    import_dat(
        pool,
        &DatImportRequest {
            dat_path: request.dat_path.clone(),
        },
    )?;
    scan_source(pool, &source_scan_request_from_run(request))?;
    build(pool, &build_workflow_request_from_run(request))
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
                hex::encode(missing.sha1)
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
