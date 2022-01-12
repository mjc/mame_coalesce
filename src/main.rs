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
use zip::{write::FileOptions, ZipWriter};

use std::{
    env,
    fs::{create_dir_all, File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

pub mod logiqx;

mod db;
mod hashes;
pub mod models;
pub mod schema;

mod destination;
mod opts;
use opts::{Opt, StructOpt};

use crate::destination::DestinationBundle;

fn main() {
    dotenv().ok();
    let mut builder = Builder::from_default_env();

    builder.filter(None, LevelFilter::Info).init();
    let opt = Opt::from_args();

    create_dir_all(&opt.destination).expect("Couldn't create destination directory");

    info!("Using datafile: {}", opt.datafile);
    info!("Looking in path: {}", opt.path.to_str().unwrap());
    info!("Saving zips to path: {}", opt.destination.to_str().unwrap());

    let data_file = logiqx::load_datafile(&opt.datafile).expect("Couldn't load datafile");
    let destination = &opt.destination;

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
    let bar_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}");
    bar.set_style(bar_style.clone());

    // this can probably be done during the walkdir
    let new_rom_files = get_all_rom_files_parallel(&file_list, &bar);

    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    // this should happen during get_all_rom_files_parallel
    // that way, we can skip extracting archives that we've already checked
    // and not load things all over again
    db::import_rom_files(&pool, &new_rom_files);

    let games = db::load_parents(&pool, &data_file_id);
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

    // this is by far the ugliest code I've ever written in any language
    // I'm sorry
    // TODO: major refactor
    write_all_zips(games, destination, &zip_bar);
}

fn write_all_zips(
    games: std::collections::BTreeMap<
        models::Game,
        std::collections::HashSet<(models::Rom, RomFile)>,
    >,
    destination: &Path,
    zip_bar: &ProgressBar,
) {
    games.par_iter().for_each(|(game, rom_and_romfile_pair)| {
        let destination_bundles = rom_and_romfile_pair.iter().map(|(rom, rom_file)| {
            DestinationBundle::from_rom_and_rom_file(rom, rom_file, game.name())
        });

        let zip_file_path = DestinationBundle::zip_file_path(destination, game.name());
        debug!("Creating zip file: {:?}", zip_file_path.to_str().unwrap());

        let (mut zip_writer, zip_options) = DestinationBundle::open_destination_zip(zip_file_path);

        // TODO: zip_writer.raw_copy_file_rename/2 to skip recompressing for zip files

        for bundle in destination_bundles {
            // TODO: don't open the same file multiple times?
            // maybe group sha's or something?

            if bundle.in_archive() {
                debug!(
                    "Adding file {} from archive: {}",
                    bundle.source_name(),
                    bundle.archive_path()
                );
                copy_from_archive(
                    bundle.archive_path(),
                    bundle.source_name(),
                    &mut zip_writer,
                    bundle.destination_name(),
                    zip_options,
                );
            } else {
                debug!("Adding file not in archive: {:?}", bundle.source_name());
                copy_bare_file(
                    bundle.archive_path(),
                    &mut zip_writer,
                    bundle.destination_name(),
                    zip_options,
                );
            }
        }
        zip_writer.finish().unwrap();
        zip_bar.inc(1);
    });
}

fn copy_bare_file(
    source_path: &str,
    zip_writer: &mut ZipWriter<BufWriter<File>>,
    destination_name: &str,
    zip_options: FileOptions,
) {
    let input_file = File::open(source_path).unwrap();
    let input_reader = BufReader::new(input_file);
    zip_writer
        .start_file(destination_name, zip_options)
        .unwrap();
    input_reader.bytes().for_each(|b| {
        zip_writer.write_all(&[b.unwrap()]).unwrap();
    });
}

fn copy_from_archive(
    source_path: &str,
    source_name: &str,
    zip_writer: &mut ZipWriter<BufWriter<File>>,
    destination_name: &str,
    zip_options: FileOptions,
) {
    let input_file = File::open(source_path).unwrap();
    let input_reader = BufReader::new(input_file);
    let mut iter = ArchiveIterator::from_read(input_reader).unwrap();
    let mut current_name = String::default();
    for content in &mut iter {
        match content {
            ArchiveContents::StartOfEntry(name) => {
                current_name = name.to_string();
                if current_name == source_name {
                    debug!("Found file: {:?}", current_name);
                    zip_writer
                        .start_file(destination_name, zip_options)
                        .unwrap();
                }
            }
            ArchiveContents::DataChunk(chunk) => {
                if current_name == source_name {
                    zip_writer.write_all(&chunk).unwrap();
                }
            }
            ArchiveContents::EndOfEntry => {
                zip_writer.flush().unwrap();
            }
            ArchiveContents::Err(e) => {
                panic!("{:?}", e)
            }
        }
    }
}

fn get_all_rom_files_parallel(file_list: &[DirEntry], bar: &ProgressBar) -> Vec<NewRomFile> {
    file_list
        .par_iter()
        .fold(
            Vec::<NewRomFile>::new,
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
            Vec::<NewRomFile>::default,
            |mut dest: Vec<NewRomFile>, mut source: Vec<NewRomFile>| {
                dest.append(&mut source);
                dest
            },
        )
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
                panic!("{:?}", e)
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
