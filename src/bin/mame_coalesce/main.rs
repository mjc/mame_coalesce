use std::process::ExitCode;

use clap::Parser;

mod options;
use options::{Cli, Command, DatCommand, SourceCommand};

use mame_coalesce::{
    app::{self, BuildWorkflowRequest, DatImportRequest, RunWorkflowRequest, SourceScanRequest},
    db::create_db_pool,
    logger,
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
    let pool = create_db_pool(cli.database_path())?;

    match cli.command() {
        Command::Dat {
            command: DatCommand::Import { dat_path },
        } => {
            app::import_dat(
                &pool,
                &DatImportRequest {
                    dat_path: dat_path.clone(),
                },
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Source {
            command: SourceCommand::Scan { source_path, jobs },
        } => {
            app::scan_source(
                &pool,
                &SourceScanRequest {
                    source_path: source_path.clone(),
                    jobs: *jobs,
                },
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Build(args) => {
            let report = app::build(
                &pool,
                &BuildWorkflowRequest {
                    dat_path: args.dat.clone(),
                    source_path: args.source.clone(),
                    destination_path: args.out.clone(),
                    mode: args.mode.into(),
                    dry_run: args.dry_run,
                    strict: args.strict,
                },
            )?;
            Ok(exit_code(report.exit_code))
        }
        Command::Run(args) => {
            let report = app::run(
                &pool,
                &RunWorkflowRequest {
                    dat_path: args.dat.clone(),
                    source_path: args.source.clone(),
                    destination_path: args.out.clone(),
                    mode: args.mode.into(),
                    jobs: args.jobs,
                    dry_run: args.dry_run,
                    strict: args.strict,
                },
            )?;
            Ok(exit_code(report.exit_code))
        }
    }
}

fn exit_code(code: i32) -> ExitCode {
    match code {
        0 => ExitCode::SUCCESS,
        2 => ExitCode::from(2),
        _ => ExitCode::from(1),
    }
}
