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
use log::{error, info, warn, LevelFilter};
use models::{NewRomFile, RomFile};
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, TerminalMode};
use walkdir::{DirEntry, WalkDir};

use std::{
    error::Error,
    fs::{create_dir_all, File},
    io::BufReader,
    os::linux::fs::MetadataExt,
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

fn main() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();
    let cli = Cli::parse();

    let pool: db::DbPool = db::create_db_pool(&cli.database_path);

    let bar_style = ProgressStyle::default_bar()
        .template("[{elapsed}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} ETA: {eta}");

    match cli.command {
        Command::AddDataFile { path } => parse_and_insert_datfile(&path, &pool),
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
) -> Result<Utf8PathBuf, Box<dyn Error>> {
    info!("Looking in path: {}", path);
    let file_list = walk_for_files(&path)?;
    let bar = ProgressBar::new(file_list.len() as u64);
    bar.set_style(bar_style.clone());
    let new_rom_files = if parallel {
        get_all_rom_files_parallel(&file_list, bar)?
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
fn parse_and_insert_datfile(path: &Utf8Path, pool: &DbPool) {
    info!("Using datafile: {}", &path);
    match logiqx::load_datafile(&path) {
        Ok(data_file) => {
            db::traverse_and_insert_data_file(pool, data_file);
        }
        Err(e) => {
            error!("Unable to load data file: {:#?}, error: {}", path, e);
        }
    }
}

fn get_all_rom_files_parallel(
    file_list: &[DirEntry],
    bar: ProgressBar,
) -> Result<Vec<NewRomFile>, Box<dyn Error>> {
    let new_rom_files = file_list
        .par_iter()
        .progress_with(bar)
        .fold(
            Vec::<NewRomFile>::new,
            |mut v: Vec<NewRomFile>, e: &DirEntry| {
                if let Some(path) = Utf8Path::from_path(e.path()) {
                    v.append(&mut build_newrom_vec(path));
                }
                v
            },
        )
        .reduce(
            Vec::<NewRomFile>::new,
            |mut dest: Vec<NewRomFile>, mut source: Vec<NewRomFile>| {
                dest.append(&mut source);
                dest
            },
        );
    Ok(new_rom_files)
}

fn get_all_rom_files(
    file_list: &[DirEntry],
    bar: ProgressBar,
) -> Result<Vec<NewRomFile>, Box<dyn Error>> {
    let new_rom_files = file_list.iter().progress_with(bar).fold(
        Vec::<NewRomFile>::new(),
        |mut v: Vec<NewRomFile>, e: &DirEntry| {
            if let Some(path) = Utf8Path::from_path(e.path()) {
                v.append(&mut build_newrom_vec(path));
            }
            v
        },
    );
    Ok(new_rom_files)
}

fn build_newrom_vec(path: &Utf8Path) -> Vec<NewRomFile> {
    match RomFile::is_archive(path) {
        false => {
            if let Some(nrf) = NewRomFile::from_path(path) {
                vec![nrf]
            } else {
                Vec::new()
            }
        }
        true => get_rom_files_for_archive(path),
    }
}

fn get_rom_files_for_archive(path: &Utf8Path) -> Vec<NewRomFile> {
    let f = File::open(path).unwrap();
    let buf = BufReader::new(f); // TODO: mmap?
    let mut rom_files: Vec<NewRomFile> = Vec::new();
    if let Ok(mut iter) = ArchiveIterator::from_read(buf) {
        let mut name = String::new();
        let mut sha1hasher = Sha1::new();

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
                    let crc = Vec::new();
                    let md5 = Vec::new();
                    if let Some(rom_file) = NewRomFile::from_archive(path, &name, crc, sha1, md5) {
                        rom_files.push(rom_file);
                    }
                }
                ArchiveContents::Err(e) => {
                    warn!("couldn't read {} from {:?}: {:?}", name, path, e)
                }
            }
        }
    }

    rom_files
}

fn walk_for_files(dir: &Utf8Path) -> Result<Vec<DirEntry>, Box<dyn Error>> {
    let v: Vec<DirEntry> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .filter_map(|v| v.ok())
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
