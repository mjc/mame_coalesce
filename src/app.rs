use camino::Utf8PathBuf;

use crate::{db::Pool, domain::BuildMode, operations};

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

pub fn import_dat(pool: &Pool, request: &DatImportRequest) -> crate::Result<DatImportReport> {
    operations::parse_and_insert_datfile(&request.dat_path, pool)
        .map(|data_file_id| DatImportReport { data_file_id })
}

pub fn scan_source(pool: &Pool, request: &SourceScanRequest) -> crate::Result<SourceScanReport> {
    operations::source(&request.source_path, request.jobs, pool)
        .map(|source_path| SourceScanReport { source_path })
}

pub fn build(pool: &Pool, request: &BuildWorkflowRequest) -> crate::Result<BuildWorkflowReport> {
    let written_paths = operations::rename_roms(
        pool,
        &request.dat_path,
        request.dry_run,
        &request.destination_path,
    )?;

    Ok(BuildWorkflowReport {
        written_paths,
        exit_code: 0,
        mode: request.mode,
        dry_run: request.dry_run,
        strict: request.strict,
    })
}
