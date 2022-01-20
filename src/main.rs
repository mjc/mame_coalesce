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

use camino::{Utf8Path, Utf8PathBuf};
use clap::StructOpt;
use compress_tools::*;
use db::DbPool;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressIterator, ProgressStyle};
use log::{info, warn, LevelFilter};
use models::NewRomFile;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use simplelog::{CombinedLogger, TermLogger};
use walkdir::{DirEntry, WalkDir};

use std::{
    convert::TryInto,
    error,
    fs::{create_dir_all, File},
    io::BufReader,
    result::Result,
};

pub mod logiqx;

mod db;
mod hashes;
pub mod models;
pub mod schema;

mod destination;
mod opts;
use opts::{Cli, Command};

type MameResult<T> = Result<T, Box<dyn error::Error>>;

fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Never,
    )])
    .unwrap();
    let cli = Cli::parse();

    let pool: db::DbPool = db::create_db_pool(&cli.database_path);

    let bar_style = ProgressStyle::default_bar()
        .template("[{elapsed}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} ETA: {eta}");

    match cli.command {
        Command::AddDataFile { path } => {
            parse_and_insert_datfile(&path, &pool);
            ()
        }
        Command::ScanSource { parallel, path } => {
            scan_source(&path, &bar_style, parallel, &pool);
            ()
        }

        Command::Rename {
            dry_run,
            data_file,
            source: _,
            destination,
        } => {
            // TODO: respect source argument
            rename_roms(&pool, &data_file, &bar_style, dry_run, &destination);
        }
    }
}

fn rename_roms(
    pool: &DbPool,
    data_file: &Utf8Path,
    bar_style: &ProgressStyle,
    dry_run: bool,
    destination: &Utf8Path,
) {
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
    zip_bar.set_style(bar_style.clone());
    if dry_run {
        info!("Dry run enabled, not writing zips!");
    } else {
        info!("Saving zips to path: {}", &destination);

        create_dir_all(&destination).expect("Couldn't create destination directory");
        destination::write_all_zips(games, &destination, &zip_bar);
    }
}

// TODO: this should return a Result
fn scan_source(
    path: &Utf8Path,
    bar_style: &ProgressStyle,
    parallel: bool,
    pool: &DbPool,
) -> MameResult<Utf8PathBuf> {
    info!("Looking in path: {}", path);
    let file_list = walk_for_files(&path)?;
    let bar = ProgressBar::new(file_list.len() as u64);
    bar.set_style(bar_style.clone());
    let new_rom_files = if parallel {
        get_all_rom_files_par(&file_list, bar)?
    } else {
        get_all_rom_files(&file_list, bar)?
    };
    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    db::import_rom_files(pool, &new_rom_files)?;
    // TODO: warning if nothing associated
    // TODO: pick datafile to scan for
    Ok(path.to_path_buf())
}

// TODO: this should return a Result
fn parse_and_insert_datfile(path: &Utf8Path, pool: &DbPool) -> Result<i32, serde_xml_rs::Error> {
    info!("Using datafile: {}", &path);
    logiqx::DataFile::from_path(&path)
        .map(|datafile| db::traverse_and_insert_data_file(pool, datafile).unwrap())
}

fn get_all_rom_files_par(file_list: &[DirEntry], bar: ProgressBar) -> MameResult<Vec<NewRomFile>> {
    Ok(file_list
        .par_iter()
        .progress_with(bar)
        .filter_map(|e| build_newrom_vec(e.path().try_into().ok()?))
        .flatten_iter()
        .collect())
}

fn get_all_rom_files(file_list: &[DirEntry], bar: ProgressBar) -> MameResult<Vec<NewRomFile>> {
    Ok(file_list
        .iter()
        .progress_with(bar)
        .filter_map(|e| build_newrom_vec(e.path().try_into().ok()?))
        .flatten()
        .collect())
}

fn build_newrom_vec(path: &Utf8Path) -> Option<Vec<NewRomFile>> {
    infer::get_from_path(path)
        .ok()
        .flatten()
        .and_then(|t| match t.mime_type() {
            "application/zip" | "application/x-7z-compressed" => scan_archive(path).ok(),
            _ => NewRomFile::from_path(path).map(|nrf| vec![nrf]),
        })
}

fn scan_archive(path: &Utf8Path) -> MameResult<Vec<NewRomFile>> {
    let f = File::open(path).unwrap();
    let buf = BufReader::new(f); // TODO: mmap?
    let mut rom_files: Vec<NewRomFile> = Vec::new();
    let iter = ArchiveIterator::from_read(buf)?;

    let mut name = String::new();
    let mut sha1hasher = Sha1::new();

    iter.for_each(|content| match content {
        ArchiveContents::StartOfEntry(s) => {
            name = s;
            sha1hasher.reset();
        }
        ArchiveContents::DataChunk(v) => {
            sha1hasher.update(&v);
        }
        ArchiveContents::EndOfEntry => {
            let sha1 = sha1hasher.finalize_reset().to_vec();
            NewRomFile::from_archive(path, &name, sha1).map(|nrf| rom_files.push(nrf));
        }
        ArchiveContents::Err(e) => {
            warn!("couldn't read {} from {:?}: {:?}", name, path, e)
        }
    });

    Ok(rom_files)
}

fn walk_for_files(dir: &Utf8Path) -> MameResult<Vec<DirEntry>> {
    let v = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .collect();
    Ok(optimize_file_order(v))
}

#[cfg(target_os = "linux")]
fn optimize_file_order(mut dirs: Vec<DirEntry>) -> Vec<DirEntry> {
    // TODO: figure out fiemap

    use walkdir::DirEntryExt;
    dirs.sort_by(|a, b| {
        let a_inode = a.ino();
        let b_inode = b.ino();
        a_inode.cmp(&b_inode)
    });
    dirs
}

#[cfg(not(target_os = "linux"))]
fn optimize_file_order(mut dirs: Vec<DirEntry>) -> Vec<DirEntry> {
    dirs
}

fn entry_is_relevant(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with('.'))
        .unwrap_or(false)
}
