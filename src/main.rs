#![deny(elided_lifetimes_in_paths, clippy::all)]
#![warn(clippy::pedantic)]
#![warn(
    clippy::nursery,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::str_to_string,
    clippy::string_to_string,
    clippy::unneeded_field_pattern,
    clippy::unwrap_used
)]

extern crate indicatif;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate walkdir;
extern crate zip;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use clap::StructOpt;

use log::warn;

use std::{error, result::Result};

mod logiqx;

mod db;
mod hashes;
mod logger;
mod models;
mod operations;
mod progress;
mod schema;

mod opts;
use opts::{Cli, Command};

type MameResult<T> = Result<T, Box<dyn error::Error>>;

fn main() {
    logger::setup_logger();

    let cli = Cli::parse();

    let pool = match db::create_db_pool(cli.database_path()) {
        Ok(pool) => pool,
        Err(err) => panic!("Couldn't create db pool: {err:?}"),
    };

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
}
