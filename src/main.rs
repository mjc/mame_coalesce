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

use compress_tools::{uncompress_archive, Ownership};
use diesel::{prelude::*, SqliteConnection};
use diesel_logger::LoggingConnection;
use dotenv::dotenv;
use files::RomFile;
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use memmap2::MmapOptions;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use tempdir::TempDir;
use walkdir::{DirEntry, WalkDir};

use std::{
    convert::TryInto,
    env, fs,
    io::{self, BufReader},
};
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

use crate::queries::import_rom_file;

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

    traverse_and_insert_data_file(&conn, data_file, file_name);
    let file_list = file_list(&opt.path);
    let bar = ProgressBar::new(file_list.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar().template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}",
        ),
    );
    // this can probably be done during the walkdir
    let rom_files: Vec<RomFile> = get_all_rom_files_parallel(&file_list, &bar);
    // this should happen during get_all_rom_files_parallel
    // that way, we can skip extracting archives that we've already checked
    for rom_file in rom_files {
        import_rom_file(&conn, &rom_file);
    }
}

fn get_all_rom_files_parallel(file_list: &Vec<DirEntry>, bar: &ProgressBar) -> Vec<RomFile> {
    file_list
        .par_iter()
        .fold(
            || Vec::<RomFile>::new(),
            |mut v: Vec<RomFile>, e: &DirEntry| {
                let path = e.path().to_path_buf();
                bar.inc(1);

                match RomFile::is_archive(e.path()) {
                    false => {
                        let r = RomFile::from_path(path, false);
                        v.push(r);
                        v
                    }
                    true => {
                        let mut internal = get_rom_files_for_archive(path);
                        v.append(&mut internal);
                        v
                    }
                }
            },
        )
        .reduce(
            || Vec::<RomFile>::new(),
            |mut dest: Vec<RomFile>, mut source: Vec<RomFile>| {
                dest.append(&mut source);
                dest
            },
        )
}

fn get_rom_files_for_archive(path: PathBuf) -> Vec<RomFile> {
    let source = File::open(&path).unwrap();
    let file_name = path.file_name().unwrap().to_str().unwrap();
    let dest = TempDir::new(file_name).unwrap();
    uncompress_archive(source, &dest.path(), Ownership::Ignore).unwrap();
    walkdir::WalkDir::new(dest.path())
        .into_iter()
        .filter_entry(|e| entry_is_relevant(e))
        .filter_map(|v| v.ok())
        .filter_map(|entry| match entry.file_type().is_file() {
            true => Some(RomFile::from_path(entry.path().to_path_buf(), true)),
            false => None,
        })
        .collect()
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
