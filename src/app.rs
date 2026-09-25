use camino::Utf8PathBuf;

use crate::{
    build::{planner::plan_build as build_plan, write_plan_with_container},
    database::Database,
    domain::{
        ArtifactOutcome, ArtifactResult, AuditReport, BuildMode, BuildReport, BuildRequest,
        CatalogKey, CatalogScope, ImportRunKey, MatchingPolicy, MissingContentPolicy,
        ObservationBasis, OutputContainer, PlanOutcome, PublishingSourceKey, ScanRunKey,
        SetSelection, SnapshotKey, SourceRoot, ZipCompression,
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
pub struct SourceScanReport {
    pub source_path: Utf8PathBuf,
    pub scan_run: ScanRunKey,
    pub observation_count: usize,
    pub associated_rom_count: usize,
    /// Number of bare-file observations reused under the explicit cache policy.
    pub reused_bare_files: usize,
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
    pub set_selection: SetSelection,
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
                .map(|path| path.canonicalize_utf8())
                .collect::<std::io::Result<std::collections::BTreeSet<_>>>()?;
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
    let (written_paths, artifact_results) =
        if request.dry_run || plan.report.outcome != PlanOutcome::Ready {
            (Vec::new(), unattempted())
        } else {
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
                        result.outcome,
                        ArtifactOutcome::Completed | ArtifactOutcome::CompletedWithWarning { .. }
                    )
                })
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
    let dat_roms =
        BuildRepository::new(database.pool()).load_dat_roms(dat_selector.repository_selector())?;
    let repository = SourceRepository::new(database.pool());
    let source_files = if let [source_root] = source_roots.as_slice() {
        repository.load_source_files_for_root(source_root)?
    } else {
        repository.load_source_files_for_roots(&source_roots)?
    };
    let plan = build_plan(
        &dat_roms,
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

pub fn run(
    database: &Database,
    request: &RunWorkflowRequest,
) -> crate::Result<BuildWorkflowReport> {
    run_with_progress(database, request, &|_| {})
}

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
    scan_sources_with_progress(database, selection, request.jobs, progress)?;
    build_with_roots_and_container(
        database,
        &build_workflow_request_from_run(request),
        selection,
        container,
    )
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
