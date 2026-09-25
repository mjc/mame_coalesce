use std::process::ExitCode;

use camino::Utf8PathBuf;
use clap::Parser;

mod logger;
mod options;
mod render;
use options::{CacheCommand, Cli, Command};

use mame_coalesce::{
    app::{self, BuildWorkflowRequest, DatImportRequest, RunWorkflowRequest, SourceScanRequest},
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
            let report = app::run_with_progress(
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
                &callback,
            )?;
            progress.finish();
            render::build_report(&report);
            Ok(render::exit_code(&report))
        }
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
            command: CacheCommand::Scan { source, jobs },
        } => {
            let progress = render::ScanProgressReporter::default();
            let callback = |event| progress.update(event);
            let report = app::scan_source_with_progress(
                &database,
                &SourceScanRequest {
                    source_path: source.clone(),
                    jobs: *jobs,
                },
                &callback,
            )?;
            progress.finish();
            render::scan_report(&report);
            Ok(ExitCode::SUCCESS)
        }
        Command::Cache {
            command: CacheCommand::Build(args),
        } => {
            let report = app::build(
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
            )?;
            render::build_report(&report);
            Ok(render::exit_code(&report))
        }
    }
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
