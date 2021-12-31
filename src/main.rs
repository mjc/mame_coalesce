extern crate indicatif;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate structopt;

extern crate walkdir;
extern crate zip;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use diesel::{prelude::*, SqliteConnection};
use diesel_logger::LoggingConnection;
use dotenv::dotenv;
use files::RomFile;
use indicatif::ProgressIterator;
use md5::Md5;
use memmap2::MmapOptions;
use sha1::{Digest, Sha1};
use walkdir::{DirEntry, WalkDir};

use std::{env, fs, io::BufReader};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

pub mod logiqx;

pub mod files;
pub mod models;
pub mod queries;
pub mod schema;

use queries::traverse_and_insert_data_file;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mame_coalesce",
    about = "A commandline app for merging ROMs for emulators like mame."
)]
struct Opt {
    datafile: String,
    #[structopt(parse(from_os_str))]
    path: PathBuf,
    #[structopt(parse(from_os_str))]
    destination: Option<PathBuf>,
}

impl Opt {
    pub fn default_destination(path: &PathBuf) -> PathBuf {
        [path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect()
    }
}

fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    let opt = Opt::from_args();

    let destination = match opt.destination {
        None => Opt::default_destination(&opt.path),
        Some(x) => x,
    };

    fs::create_dir_all(&destination).expect("Couldn't create destination directory");

    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());
    println!("Saving zips to path: {}", destination.to_str().unwrap());

    let data_file = logiqx::load_datafile(&opt.datafile).expect("Couldn't load datafile");

    let conn = establish_connection();

    let file_name = Path::new(&opt.datafile)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    traverse_and_insert_data_file(conn, data_file, file_name);
    let file_list = file_list(&opt.path);
    // this can probably be done during the walkdir
    let rom_files = file_list.iter().progress().fold(
        Vec::<RomFile>::new().as_mut(),
        |rf_vec: &mut Vec<RomFile>, dir_entry| {
            if RomFile::is_archive(dir_entry.path()) {
                rf_vec
            } else {
                rf_vec.push(RomFile::from_path(dir_entry.path(), false));
                rf_vec
            }
        },
    );
}

embed_migrations!("migrations");
pub fn establish_connection() -> LoggingConnection<SqliteConnection> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let connection = LoggingConnection::<SqliteConnection>::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url));
    let _migration_result = embedded_migrations::run(&connection);
    connection
}

fn file_list(dir: &PathBuf) -> Vec<DirEntry> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| entry_is_relevant(e))
        .filter_map(|v| v.ok())
        .filter_map(|entry| match entry.file_type().is_file() {
            true => Some(entry),
            false => None,
        })
        .collect::<Vec<DirEntry>>()
}

fn entry_is_relevant(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with('.'))
        .unwrap_or(false)
}
