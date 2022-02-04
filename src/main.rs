#![deny(elided_lifetimes_in_paths)]

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

use fmmap::{MmapFile, MmapFileExt};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use log::{info, warn, LevelFilter};
use models::NewRomFile;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use simplelog::{CombinedLogger, TermLogger};
use walkdir::{DirEntry, WalkDir};

use std::{error, fs::File, io::BufReader, path::Path, result::Result};

mod logiqx;

mod db;
mod hashes;
mod models;
mod operations;
mod schema;

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

    // TODO: these .unwrap()s need to actually handle errors
    match cli.command {
        Command::AddDataFile { path } => {
            parse_and_insert_datfile(&path, &pool).unwrap();
        }
        Command::ScanSource { jobs, path } => {
            scan_source(&path, &bar_style, jobs, &pool).unwrap();
        }

        Command::Rename {
            dry_run,
            data_file,
            source: _,
            destination,
        } => {
            // TODO: respect source argument
            operations::rename_roms(&pool, &data_file, &bar_style, dry_run, &destination).unwrap();
        }
    }
}

// TODO: this should return a Result
fn scan_source(
    path: &Utf8Path,
    bar_style: &ProgressStyle,
    jobs: usize,
    pool: &DbPool,
) -> MameResult<Utf8PathBuf> {
    info!("Looking in path: {}", path);
    let file_list = walk_for_files(path)?;
    let bar = ProgressBar::new(file_list.len() as u64);
    bar.set_style(bar_style.clone());
    let new_rom_files = get_all_rom_files_par(file_list, jobs, bar)?;

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
fn parse_and_insert_datfile(path: &Utf8Path, pool: &DbPool) -> MameResult<i32> {
    info!("Using datafile: {}", &path);
    logiqx::DataFile::from_path(path)
        .and_then(|datafile| db::traverse_and_insert_data_file(pool, datafile))
}

fn get_all_rom_files_par(
    file_list: Vec<Utf8PathBuf>,
    jobs: usize,
    bar: ProgressBar,
) -> MameResult<Vec<NewRomFile>> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build_global()
        .unwrap();
    Ok(file_list
        .par_iter()
        .progress_with(bar)
        .filter_map(|p| build_newrom_vec(p))
        .flatten_iter()
        .collect())
}

fn build_newrom_vec(path: &Utf8Path) -> Option<Vec<NewRomFile>> {
    let single_rom = || NewRomFile::from_path(path).map(|nrf| vec![nrf]);
    let mmap = hashes::mmap_path(path).ok()?;
    infer::get_from_path(path)
        .ok()
        .flatten()
        .map_or_else(single_rom, |t| match t.mime_type() {
            "application/zip" => scan_zip(mmap).ok(),
            "application/x-7z-compressed" => scan_7z(path).ok(),
            _ => single_rom(),
        })
}

fn scan_zip(mmap: MmapFile) -> MameResult<Vec<NewRomFile>> {
    let path = Utf8Path::from_path(mmap.path()).unwrap();
    let reader = mmap.reader(0)?;
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut rom_files = Vec::new();

    let mut sha1hasher = Sha1::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let name = file.enclosed_name().unwrap();
        let mut nrf = NewRomFile::from_archive(path, name, Vec::new()).unwrap();

        std::io::copy(&mut file, &mut sha1hasher)?;
        let sha1 = sha1hasher.finalize_reset().to_vec();
        nrf.set_sha1(sha1);
        rom_files.push(nrf);
    }

    Ok(rom_files)
}

fn scan_7z(path: &Utf8Path) -> MameResult<Vec<NewRomFile>> {
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    // let mmap = mmap_path(path)?;
    // let chunks = mmap.chunks(16_384);
    let mut rom_files: Vec<NewRomFile> = Vec::new();
    let iter = ArchiveIterator::from_read(reader)?;

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
            let filename = Path::new(&name);
            if let Some(nrf) = NewRomFile::from_archive(path, filename, sha1) {
                rom_files.push(nrf)
            }
        }
        ArchiveContents::Err(e) => {
            warn!("couldn't read {} from {:?}: {:?}", name, path, e)
        }
    });

    Ok(rom_files)
}

fn walk_for_files(dir: &Utf8Path) -> MameResult<Vec<Utf8PathBuf>> {
    let v = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .collect();
    let optimized = optimize_file_order(v);
    Ok(optimized
        .iter()
        .filter_map(|direntry| Utf8PathBuf::from_path_buf(direntry.path().to_path_buf()).ok())
        .collect())
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
