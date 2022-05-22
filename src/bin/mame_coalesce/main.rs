use clap::StructOpt;

mod options;
use options::{Cli, Command};

use mame_coalesce::{
    db::{create_db_pool, SyncPool},
    logger, operations, MameResult,
};

fn main() -> MameResult<()> {
    logger::setup_logger();

    let cli = Cli::parse();

    let pool = get_pool(&cli);

    match cli.command() {
        Command::AddDataFile { path } => {
            if let Err(e) = operations::parse_and_insert_datfile(path, &pool) {
                panic!("Couldn't insert data file: {e:?}");
            }
        }
        Command::ScanSource { jobs, path } => {
            if let Err(e) = operations::scan_source(path, *jobs, &pool) {
                panic!("Couldn't scan source: {e:?}");
            }
        }

        Command::Rename {
            dry_run,
            data_file,
            destination,
            ..
        } => {
            // TODO: respect source argument
            let result = operations::rename_roms(&pool, data_file, *dry_run, destination);

            if let Err(e) = result {
                panic!("Unable to rename roms: {e:?}")
            }
        }
    }
    Ok(())
}

pub fn get_pool(cli: &Cli) -> SyncPool {
    let pool = match create_db_pool(cli.database_path()) {
        Ok(pool) => pool,
        Err(err) => panic!("Couldn't create db pool: {err:?}"),
    };
    pool
}
