use camino::Utf8PathBuf;

use crate::{
    build::{planner::plan_build, writer::write_plan},
    db::Pool,
    domain::{BuildMode, BuildRequest},
    operations,
    storage::repositories::{BuildRepository, SourceRepository},
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
    pub dry_run: bool,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildWorkflowReport {
    pub written_paths: Vec<Utf8PathBuf>,
    pub exit_code: i32,
    pub mode: BuildMode,
    pub dry_run: bool,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunWorkflowRequest {
    pub dat_path: Utf8PathBuf,
    pub source_path: Utf8PathBuf,
    pub destination_path: Utf8PathBuf,
    pub mode: BuildMode,
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
    let dat_path = request.dat_path.canonicalize_utf8()?;
    let source_root = request.source_path.canonicalize_utf8()?;
    let dat_roms = BuildRepository::new(pool).load_dat_roms(&dat_path)?;
    let source_files = SourceRepository::new(pool).load_source_files()?;
    let plan = plan_build(
        &dat_roms,
        &source_files,
        &BuildRequest {
            dat_name: dat_path.to_string(),
            source_root: source_root.to_string(),
            mode: request.mode,
            dry_run: request.dry_run,
            strict: request.strict,
        },
    );
    let exit_code = plan.report.exit_code;
    let written_paths = write_plan(&plan, &request.destination_path)?;

    Ok(BuildWorkflowReport {
        written_paths,
        exit_code,
        mode: request.mode,
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
    scan_source(
        pool,
        &SourceScanRequest {
            source_path: request.source_path.clone(),
            jobs: request.jobs,
        },
    )?;
    build(
        pool,
        &BuildWorkflowRequest {
            dat_path: request.dat_path.clone(),
            source_path: request.source_path.clone(),
            destination_path: request.destination_path.clone(),
            mode: request.mode,
            dry_run: request.dry_run,
            strict: request.strict,
        },
    )
}
