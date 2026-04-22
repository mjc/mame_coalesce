use clap::Parser;

mod options;
use options::{Cli, Command};

use mame_coalesce::{db::create_db_pool, logger, operations};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logger::setup();

    let cli = Cli::parse();
    let pool = create_db_pool(cli.database_path())?;

    match cli.command() {
        Command::AddDataFile { path } => {
            operations::parse_and_insert_datfile(path.as_path(), &pool)?;
        }
        Command::ScanSource { jobs, path } => {
            operations::source(path.as_path(), *jobs, &pool)?;
        }
        Command::Rename {
            dry_run,
            data_file,
            destination,
            ..
        } => {
            // TODO: respect source argument
            operations::rename_roms(&pool, data_file.as_path(), *dry_run, destination.as_path())?;
        }
    }
    Ok(())
}
