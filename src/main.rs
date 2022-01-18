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
use compress_tools::*;
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use log::{error, info, LevelFilter};
use models::{NewRomFile, RomFile};
use pretty_env_logger::env_logger::Builder;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use walkdir::{DirEntry, WalkDir};

use std::{
    fs::{create_dir_all, File},
    io::BufReader,
    path::Path,
};

pub mod logiqx;

mod db;
mod hashes;
pub mod models;
pub mod schema;

mod destination;
mod opts;
use opts::{Cli, Command};

fn main() {
    dotenv().ok();
    let mut builder = Builder::from_default_env();

    builder.filter(None, LevelFilter::Info).init();
    let cli = Cli::parse();

    let pool: db::DbPool = db::create_db_pool(&cli.database_path);

    let bar_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}");

    match cli.command {
        Command::AddDataFile { path } =>
        // TODO: this should be a wrapper that returns a Result
        {
            info!("Using datafile: {}", &path.to_str().unwrap());

            match logiqx::load_datafile(&path) {
                Ok(data_file) => {
                    db::traverse_and_insert_data_file(&pool, data_file);
                }
                Err(e) => {
                    error!("Unable to load data file: {:#?}, error: {}", path, e);
                }
            }
        }
        Command::ScanSource { parallel, path } => {
            info!("Looking in path: {}", &path.to_str().unwrap());

            // TODO: spinner for file walking
            let file_list = walk_for_files(&path);

            let bar = ProgressBar::new(file_list.len() as u64);
            bar.set_style(bar_style);

            // this can probably be done during the walkdir
            // need to experiment with SSD vs HDD
            // HDD request ordering should help too
            // detect ssd or hdd?
            let new_rom_files = if parallel {
                get_all_rom_files_parallel(&file_list, &bar)
            } else {
                get_all_rom_files(&file_list, &bar)
            };

            info!(
                "rom files found (unpacked and packed both): {}",
                new_rom_files.len()
            );

            // this should happen while loading files
            // that way, we can skip extracting archives that we've already checked
            // and not load things all over again
            db::import_rom_files(&pool, &new_rom_files);
            // TODO: warning if nothing associated
            // TODO: pick datafile to scan for
        }
        Command::Rename {
            dry_run,
            data_file,
            source: _,
            destination,
        } => {
            // TODO: respect source argument
            let games = db::load_parents(&pool, &data_file);
            info!(
                "Processing {} games with {} matching rom files",
                games.len(),
                games
                    .iter()
                    .map(|(_rom, rom_files)| { rom_files.len() as i32 })
                    .sum::<i32>()
            );

            let zip_bar = ProgressBar::new(games.len() as u64);
            zip_bar.set_style(bar_style);
            if dry_run {
                info!("Dry run enabled, not writing zips!");
            } else {
                info!("Saving zips to path: {}", &destination.to_str().unwrap());

                create_dir_all(&destination).expect("Couldn't create destination directory");
                destination::write_all_zips(games, &destination, &zip_bar);
            }
        }
    }
}

fn get_all_rom_files_parallel(file_list: &[DirEntry], bar: &ProgressBar) -> Vec<NewRomFile> {
    file_list
        .par_iter()
        .fold(
            Vec::<NewRomFile>::new,
            |v: Vec<NewRomFile>, e: &DirEntry| {
                bar.inc(1);

                build_newrom_vec(e, v)
            },
        )
        .reduce(
            Vec::<NewRomFile>::default,
            |mut dest: Vec<NewRomFile>, mut source: Vec<NewRomFile>| {
                dest.append(&mut source);
                dest
            },
        )
}

fn get_all_rom_files(file_list: &[DirEntry], bar: &ProgressBar) -> Vec<NewRomFile> {
    file_list.iter().fold(
        Vec::<NewRomFile>::new(),
        |v: Vec<NewRomFile>, e: &DirEntry| {
            bar.inc(1);

            build_newrom_vec(e, v)
        },
    )
}

fn build_newrom_vec(e: &DirEntry, mut v: Vec<NewRomFile>) -> Vec<NewRomFile> {
    let path = e.path().to_path_buf();
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
}

fn get_rom_files_for_archive(path: &Path) -> Vec<NewRomFile> {
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
                panic!("couldn't read {} from {:?}: {:?}", name, path, e)
            }
        }
    }
    rom_files
}

fn walk_for_files(dir: &Path) -> Vec<DirEntry> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .filter_map(|v| v.ok())
        .filter(|entry| entry.file_type().is_file())
        .collect::<Vec<DirEntry>>()
}

fn entry_is_relevant(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with('.'))
        .unwrap_or(false)
}
