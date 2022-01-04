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

use compress_tools::*;
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, LevelFilter};
use models::{NewRomFile, RomFile};
use pretty_env_logger::env_logger::Builder;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use walkdir::{DirEntry, WalkDir};

use std::{env, fs, io::BufReader};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

pub mod logiqx;

mod db;
mod hashes;
pub mod models;
pub mod schema;

mod opts;
use opts::{Opt, StructOpt};

fn main() {
    dotenv().ok();
    let mut builder = Builder::from_default_env();

    builder.filter(None, LevelFilter::Info).init();
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

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool: db::DbPool = db::create_db_pool(&database_url);

    let file_name = Path::new(&opt.datafile)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    info!("Loading data file: {:?}", &file_name);
    let data_file_id = db::traverse_and_insert_data_file(&pool, data_file, file_name);

    let file_list = walk_for_files(&opt.path);

    let bar = ProgressBar::new(file_list.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar().template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}",
        ),
    );

    // this can probably be done during the walkdir
    let new_rom_files = get_all_rom_files_parallel(&file_list, &bar);
    bar.finish();
    // this should happen during get_all_rom_files_parallel
    // that way, we can skip extracting archives that we've already checked
    // and not load things all over again
    db::import_rom_files(&pool, &new_rom_files);

    let games = db::load_games(&pool, &data_file_id);
    info!("Processing {} games with matching rom files", &games.len());
    debug!("{:?}", &games);
}

fn get_all_rom_files_parallel(file_list: &Vec<DirEntry>, bar: &ProgressBar) -> Vec<NewRomFile> {
    file_list
        .par_iter()
        .fold(
            || Vec::<NewRomFile>::new(),
            |mut v: Vec<NewRomFile>, e: &DirEntry| {
                let path = e.path().to_path_buf();
                bar.inc(1);

                match RomFile::is_archive(e.path()) {
                    false => {
                        let r = NewRomFile::from_path(path);
                        v.push(r);
                        v
                    }
                    true => {
                        let mut internal = get_rom_files_for_archive(&path);
                        v.append(&mut internal);
                        v
                    }
                }
            },
        )
        .reduce(
            || Vec::<NewRomFile>::default(),
            |mut dest: Vec<NewRomFile>, mut source: Vec<NewRomFile>| {
                dest.append(&mut source);
                dest
            },
        )
}

fn get_rom_files_for_archive(path: &PathBuf) -> Vec<NewRomFile> {
    let f = File::open(path).unwrap();
    let buf = BufReader::new(f); // TODO: mmap?
    let mut name = String::default();
    let mut iter = ArchiveIterator::from_read(buf).unwrap();
    let mut sha1hasher = Sha1::default();

    let mut rom_files: Vec<NewRomFile> = Vec::default();

    for content in &mut iter {
        match content {
            ArchiveContents::StartOfEntry(s) => {
                name = s;
                sha1hasher.reset();
            }
            ArchiveContents::DataChunk(v) => {
                sha1hasher.update(&v);
            }
            ArchiveContents::EndOfEntry => {
                let sha1 = sha1hasher.finalize_reset().to_vec();
                let crc = Vec::default();
                let md5 = Vec::default();

                rom_files.push(NewRomFile::from_archive(path, &name, crc, sha1, md5));
            }
            ArchiveContents::Err(e) => {
                panic!("{:?}", e)
            }
        }
    }

    rom_files
}

fn walk_for_files(dir: &PathBuf) -> Vec<DirEntry> {
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
