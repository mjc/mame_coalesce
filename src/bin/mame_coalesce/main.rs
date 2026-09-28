use std::{io::Write, process::ExitCode};

use camino::Utf8PathBuf;
use clap::Parser;

mod logger;
mod options;
mod render;
mod report;
use options::{AuditArgs, AuditFormatArg, CacheCommand, Cli, Command};

use mame_coalesce::{
    app::{
        self, AuditRefresh, AuditRequest, BuildWorkflowRequest, CatalogImportRequest,
        CatalogImportStatus, DatImportRequest, DiskAuditRequest, RunWorkflowRequest,
        SourceRootSelection,
    },
    database::Database,
    domain::{CatalogKey, PublishingSourceKey},
};

fn main() -> ExitCode {
    logger::setup();
    match run() {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> mame_coalesce::Result<ExitCode> {
    let cli = Cli::parse();
    let database = Database::open(&resolve_cache_path(cli.cache()))?;

    match cli.command() {
        Command::Build(args) => {
            let progress = render::ScanProgressReporter::default();
            let callback = |event| progress.update(event);
            let result = app::run_with_roots_and_progress(
                &database,
                &RunWorkflowRequest {
                    dat_path: args.dat.clone(),
                    source_path: args.source.clone(),
                    destination_path: args.out.clone(),
                    mode: args.options.layout.into(),
                    compression: args.options.compression.into(),
                    jobs: args.jobs,
                    dry_run: args.options.dry_run,
                    strict: args.options.missing.strict(),
                },
                &SourceRootSelection {
                    primary: args.source.clone(),
                    additional: args.additional_source_roots.clone(),
                },
                &callback,
            );
            progress.finish();
            render_build_result(result)
        }
        Command::Audit(args) => audit_command(&database, args),
        Command::Cache {
            command: CacheCommand::Import { dat },
        } => {
            let report = app::import_dat(
                &database,
                &DatImportRequest {
                    dat_path: dat.clone(),
                },
            )?;
            log::info!("imported DAT as cache data file {}", report.data_file_id);
            Ok(ExitCode::SUCCESS)
        }
        Command::Cache {
            command: CacheCommand::CatalogImport(args),
        } => import_catalog_command(&database, args),
        Command::Cache {
            command: CacheCommand::Scan(args),
        } => {
            let progress = render::ScanProgressReporter::default();
            let callback = |event| progress.update(event);
            let reports = app::scan_sources_with_progress(
                &database,
                &SourceRootSelection {
                    primary: args.source.clone(),
                    additional: args.additional_source_roots.clone(),
                },
                args.jobs,
                &callback,
            )?;
            progress.finish();
            for report in &reports {
                render::scan_report(report);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Cache {
            command: CacheCommand::Build(args),
        } => {
            let result = app::build_with_roots(
                &database,
                &BuildWorkflowRequest {
                    dat_path: args.dat.clone(),
                    source_path: args.source.clone(),
                    destination_path: args.out.clone(),
                    mode: args.options.layout.into(),
                    compression: args.options.compression.into(),
                    dry_run: args.options.dry_run,
                    strict: args.options.missing.strict(),
                },
                &SourceRootSelection {
                    primary: args.source.clone(),
                    additional: args.additional_source_roots.clone(),
                },
            );
            render_build_result(result)
        }
        Command::Cache {
            command: CacheCommand::Audit(args),
        } => run_disk_audit(&database, args),
    }
}

fn run_disk_audit(
    database: &Database,
    args: &options::DiskAuditArgs,
) -> mame_coalesce::Result<ExitCode> {
    let report = app::audit_disks(
        database,
        &DiskAuditRequest {
            catalog_key: args.catalog.clone(),
            source_path: args.source.clone(),
        },
    )?;
    report::write_disk_audit(&report, args.format)?;
    Ok(ExitCode::SUCCESS)
}

fn import_catalog_command(
    database: &Database,
    args: &options::CatalogImportArgs,
) -> mame_coalesce::Result<ExitCode> {
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: args.document.clone(),
            format: args.format.into(),
            source_key: PublishingSourceKey::new(args.source_key.clone()),
            source_display_name: args.source_name.clone(),
            catalog_key: CatalogKey::new(args.catalog_key.clone()),
            catalog_display_name: args.catalog_name.clone(),
            scope: args.scope.into(),
        },
    )?;
    if report.status == CatalogImportStatus::Failed {
        eprintln!(
            "catalog import failed ({} diagnostics)",
            report.diagnostic_count
        );
        Ok(ExitCode::FAILURE)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn render_build_result(
    result: mame_coalesce::Result<mame_coalesce::app::BuildWorkflowReport>,
) -> mame_coalesce::Result<ExitCode> {
    match result {
        Ok(report) => {
            render_workflow_scans(&report);
            render::build_report(&report);
            Ok(render::exit_code(&report))
        }
        Err(mame_coalesce::Error::BuildWorkflow { report, source }) => {
            render_workflow_scans(&report);
            render::build_report(&report);
            eprintln!("{source}");
            Ok(ExitCode::from(1))
        }
        Err(mame_coalesce::Error::RunWorkflow {
            scan_report,
            source,
        }) => {
            render::scan_report(&scan_report);
            eprintln!("{source}");
            Ok(ExitCode::from(1))
        }
        Err(mame_coalesce::Error::RunWorkflowWithRoots {
            scan_reports,
            source,
        }) => {
            for scan_report in &scan_reports {
                render::scan_report(scan_report);
            }
            eprintln!("{source}");
            Ok(ExitCode::from(1))
        }
        Err(error) => Err(error),
    }
}

fn render_workflow_scans(report: &mame_coalesce::app::BuildWorkflowReport) {
    if report.scan_reports.is_empty() {
        if let Some(scan_report) = &report.scan_report {
            render::scan_report(scan_report);
        }
    } else {
        for scan_report in &report.scan_reports {
            render::scan_report(scan_report);
        }
    }
}

fn audit_command(database: &Database, args: &AuditArgs) -> mame_coalesce::Result<ExitCode> {
    let progress = render::ScanProgressReporter::default();
    let callback = |event| progress.update(event);
    let report = app::audit_with_roots_and_progress(
        database,
        &AuditRequest {
            dat_path: args.dat.clone(),
            source_path: args.source.clone(),
            refresh: if args.refresh {
                AuditRefresh::Refresh
            } else {
                AuditRefresh::Cached
            },
            jobs: args.jobs,
            matching_policy: args.matching_policy.into(),
        },
        &SourceRootSelection {
            primary: args.source.clone(),
            additional: args.additional_source_roots.clone(),
        },
        &callback,
    )?;
    progress.finish();
    match args.format {
        AuditFormatArg::Human => print!("{}", render::audit_report(&report)),
        AuditFormatArg::Json => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(&report.to_json()?)?;
            stdout.write_all(b"\n")?;
        }
    }
    Ok(render::audit_exit_code(&report))
}

fn resolve_cache_path(cache: Option<&Utf8PathBuf>) -> Utf8PathBuf {
    cache.cloned().unwrap_or_else(default_cache_path)
}

fn default_cache_path() -> Utf8PathBuf {
    std::env::var("XDG_CACHE_HOME")
        .map(Utf8PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| Utf8PathBuf::from(home).join(".cache"))
        })
        .map_or_else(
            || Utf8PathBuf::from("coalesce.db"),
            |cache_root| cache_root.join("mame_coalesce").join("coalesce.db"),
        )
}
