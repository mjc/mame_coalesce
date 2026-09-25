use std::{io::Write, process::ExitCode};

use camino::Utf8PathBuf;
use clap::Parser;

mod logger;
mod options;
mod render;
use options::{AuditArgs, AuditFormatArg, CacheCommand, Cli, Command};

use mame_coalesce::{
    app::{
        self, AuditRefresh, AuditRequest, BuildWorkflowRequest, DatImportRequest,
        RunWorkflowRequest, ScanCachePolicy, SourceRootSelection, SourceScanRequest,
    },
    database::Database,
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
            let report = app::run_with_roots_and_container_and_progress(
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
                args.options.output_container.into(),
                &callback,
            )?;
            progress.finish();
            render::build_report(&report);
            Ok(render::exit_code(&report))
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
            command: CacheCommand::Scan(args),
        } => cache_scan_command(&database, args),
        Command::Cache {
            command: CacheCommand::Build(args),
        } => {
            let report = app::build_with_roots_and_container(
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
                args.options.output_container.into(),
            )?;
            render::build_report(&report);
            Ok(render::exit_code(&report))
        }
    }
}

fn cache_scan_command(
    database: &Database,
    args: &options::CacheScanArgs,
) -> mame_coalesce::Result<ExitCode> {
    let progress = render::ScanProgressReporter::default();
    let callback = |event| progress.update(event);
    let reports = if args.reuse_unchanged {
        if !args.additional_source_roots.is_empty() {
            return Err(mame_coalesce::Error::InvalidPath(
                "bare-file reuse currently supports one source root per scan".to_owned(),
            ));
        }
        vec![app::scan_source_with_policy_and_progress(
            database,
            &SourceScanRequest {
                source_path: args.source.clone(),
                jobs: args.jobs,
            },
            ScanCachePolicy::ReuseUnchangedBareFiles {
                force_rehash: args.force_rehash.clone(),
            },
            &callback,
        )?]
    } else {
        app::scan_sources_with_progress(
            database,
            &SourceRootSelection {
                primary: args.source.clone(),
                additional: args.additional_source_roots.clone(),
            },
            args.jobs,
            &callback,
        )?
    };
    progress.finish();
    for report in &reports {
        render::scan_report(report);
    }
    Ok(ExitCode::SUCCESS)
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
