extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate structopt;

extern crate walkdir;
extern crate zip;

use rayon::prelude::*;

use sha1::{Digest, Sha1};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use structopt::StructOpt;

use walkdir::{DirEntry, WalkDir};

use zip::write::{FileOptions, ZipWriter};

mod logiqx;

#[derive(Debug, Clone)]
struct File {
    path: PathBuf,
    sha1: Option<String>,
}

impl File {
    fn new(entry: &DirEntry) -> Self {
        File {
            sha1: None,
            path: entry.path().to_path_buf(),
        }
    }
    fn entry_is_relevant(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| entry.depth() == 0 || !s.starts_with("."))
            .unwrap_or(false)
    }
}

#[derive(Debug)]
struct Bundle {
    name: String,                            // 7z name
    files: HashMap<String, String>,          // sha1 key, rom file name
    matches: Vec<(String, String, PathBuf)>, // sha1, destination, File for matching files
}

impl Bundle {
    fn new(game: &logiqx::Game) -> Self {
        Bundle {
            name: game.name.to_string(),
            files: Self::load_files_from_roms(&game.roms),
            matches: Vec::<(String, String, PathBuf)>::new(),
        }
    }

    fn load_files_from_roms(roms: &Vec<logiqx::Rom>) -> HashMap<String, String> {
        roms.iter()
            .map(|rom| Self::get_sha_and_destination_name(rom))
            .collect()
    }
    fn get_sha_and_destination_name(rom: &logiqx::Rom) -> (String, String) {
        (rom.sha1.to_string().to_lowercase(), rom.name.to_string())
    }
}

fn load_datafile(name: String) -> logiqx::Datafile {
    let datafile_contents =
        fs::read_to_string(name).expect("Something went wrong reading the datfile");
    logiqx::Datafile::from_str(&datafile_contents)
}

fn list_files(dir: PathBuf) -> Vec<File> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| File::entry_is_relevant(e))
        .filter_map(|v| v.ok())
        .filter_map(|entry| match entry.file_type().is_file() {
            true => Some(File::new(&entry)),
            false => None,
        })
        .collect()
}

fn compute_sha1(path: &PathBuf) -> Option<String> {
    let mut file = fs::File::open(path).unwrap();
    let mut hasher = Sha1::new();
    let _n = io::copy(&mut file, &mut hasher);
    Some(format!("{:x}", hasher.result()))
}

fn compute_all_sha1(files: &mut Vec<File>) {
    files
        .par_iter_mut()
        .for_each(|file| file.sha1 = compute_sha1(&file.path));
}

fn get_key(file: &File) -> String {
    file.sha1.as_ref().unwrap().to_string()
}

fn game_bundles(datafile: &logiqx::Datafile) -> Vec<Bundle> {
    datafile
        .games
        .iter()
        .map(|game| Bundle::new(game))
        .collect()
}

fn add_matches_to_bundles(bundles: &mut Vec<Bundle>, files: &HashMap<String, File>) {
    for bundle in bundles.iter_mut() {
        for (sha, name) in bundle.files.iter() {
            match files.get(sha) {
                Some(file) => bundle.matches.push((
                    sha.to_string(),
                    name.to_string(),
                    file.path.to_path_buf(),
                )),
                None => (),
            }
        }
    }
}

fn write_zip(bundle: &Bundle, zip_dest: PathBuf) {
    let output_file_name = format!("{}.zip", bundle.name);
    println!("Writing {}", output_file_name);
    let path: PathBuf = [zip_dest.to_str().unwrap(), output_file_name.as_str()]
        .iter()
        .collect();
    let output = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(output);
    bundle.files.iter().for_each(|(sha, _file)| {
        match bundle
            .matches
            .iter()
            .find(|(sha1, _dest, _src)| sha == sha1)
        {
            Some((_sha1, dest, src)) => {
                let mut source = fs::File::open(Path::new(src)).unwrap();
                zip.start_file(dest, FileOptions::default()).unwrap();
                std::io::copy(&mut source, &mut zip).unwrap();
            }
            None => (),
        }
    });
    zip.finish().unwrap();
}

fn write_all_zip(bundles: Vec<Bundle>, zip_dest: &PathBuf) {
    bundles
        .par_iter()
        .for_each(|bundle| write_zip(bundle, zip_dest.to_path_buf()));
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mame_coalesce",
    about = "A commandline app for merging ROMs for emulators like mame."
)]
struct Opt {
    datfile: String,
    #[structopt(parse(from_os_str))]
    path: PathBuf,
    #[structopt(parse(from_os_str))]
    destination: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);
    let default_destination: PathBuf =
        [opt.path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect();
    let destination = match opt.destination {
        Some(x) => x,
        None => default_destination,
    };
    fs::create_dir_all(&destination).expect("Couldn't create destination directory");
    println!("Using datfile: {}", opt.datfile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());

    let data = load_datafile(opt.datfile);
    let mut bundles = game_bundles(&data);

    let mut files = list_files(opt.path);
    println!("Files to check: {}", files.len());

    compute_all_sha1(&mut files);

    let files_by_sha1: HashMap<String, File> = files
        .iter()
        .map(|file| (get_key(file), file.clone()))
        .collect();

    println!(
        "sha1 of last file: {:?}",
        files.last().unwrap().sha1.as_ref().unwrap()
    );

    add_matches_to_bundles(&mut bundles, &files_by_sha1);

    write_all_zip(bundles, &destination);
}
