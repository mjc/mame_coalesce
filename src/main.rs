extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;
extern crate walkdir;
extern crate zip;

use rayon::prelude::*;

use sha1::{Digest, Sha1};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use walkdir::{DirEntry, WalkDir};

use zip::write::{FileOptions, ZipWriter};

mod logiqx {
    #[derive(Debug, Deserialize)]
    pub struct Datafile {
        #[serde(default)]
        pub build: String,
        #[serde(default)]
        pub debug: String, // bool
        pub header: Header,
        #[serde(rename = "game", default)]
        pub games: Vec<Game>,
    }
    #[derive(Debug, Deserialize)]
    pub struct Header {
        pub name: String,
        pub description: String,
        pub version: String,
        pub author: String,
        pub homepage: String,
        pub url: String,
    }
    #[derive(Debug, Deserialize)]
    pub struct Game {
        pub name: String,
        #[serde(default)]
        pub sourcefile: String,
        #[serde(default)]
        pub isbios: String, // bool
        #[serde(default)]
        pub cloneof: String,
        #[serde(default)]
        pub romof: String,
        #[serde(default)]
        pub sampleof: String,
        #[serde(default)]
        pub board: String,
        #[serde(default)]
        pub rebuildto: String,
        #[serde(default)]
        pub year: String, // should probably be a DateTime
        #[serde(default)]
        pub manufacturer: String,
        #[serde(rename = "rom", default)]
        pub roms: Vec<Rom>,
    }
    #[derive(Debug, Deserialize)]
    pub struct Rom {
        pub name: String,
        pub size: String,
        pub md5: String,
        pub sha1: String,
        pub crc: String,
        #[serde(default)]
        pub merge: String,
        #[serde(default)]
        pub status: String, // baddump|nodump|good|verified
        #[serde(default)]
        pub serial: String,
        #[serde(default)]
        pub date: String, // should probably be DateTime
    }
}

#[derive(Debug, Clone)]
struct File {
    path: PathBuf,
    sha1: Option<String>,
}

#[derive(Debug)]
struct Bundle {
    name: String,                            // 7z name
    files: HashMap<String, String>,          // sha1 key, rom file name
    matches: Vec<(String, String, PathBuf)>, // sha1, destination, File for matching files
}

fn load_datafile(name: &String) -> logiqx::Datafile {
    let datafile_contents =
        fs::read_to_string(name).expect("Something went wrong reading the datfile");
    serde_xml_rs::from_str(&datafile_contents).unwrap()
}

fn is_relevant(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with("."))
        .unwrap_or(false)
}

fn list_files(dir: &String) -> Vec<File> {
    let mut paths = vec![];
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| is_relevant(e))
        .filter_map(|v| v.ok())
    {
        if entry.file_type().is_file() {
            let file = File {
                // sha1: Some(compute_sha1(entry.path().to_path_buf())),
                sha1: None,
                path: entry.path().to_path_buf(),
            };
            paths.push(file);
        }
    }

    paths
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

fn get_sha_and_destination_name(rom: &logiqx::Rom) -> (String, String) {
    (rom.sha1.to_string().to_lowercase(), rom.name.to_string())
}

fn get_bundle_files(roms: &Vec<logiqx::Rom>) -> HashMap<String, String> {
    roms.iter()
        .map(|rom| get_sha_and_destination_name(rom))
        .collect()
}

fn bundle_from_game(game: &logiqx::Game) -> Bundle {
    Bundle {
        name: game.name.to_string(),
        files: get_bundle_files(&game.roms),
        matches: Vec::<(String, String, PathBuf)>::new(),
    }
}

fn game_bundles(datafile: &logiqx::Datafile) -> Vec<Bundle> {
    datafile
        .games
        .iter()
        .map(|game| bundle_from_game(game))
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

fn write_zip(bundle: &Bundle, zip_dest: &str) {
    let output_file_name = format!("{}.zip", bundle.name);
    println!("Writing {}", output_file_name);
    let path: PathBuf = [zip_dest, output_file_name.as_str()].iter().collect();
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

fn write_all_zip(bundles: Vec<Bundle>, zip_dest: &str) {
    bundles
        .iter()
        .for_each(|bundle| write_zip(bundle, zip_dest));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let datfile = &args[1];
    let path = &args[2];
    let destination = &args[3];
    println!("Using datfile: {}", datfile);
    println!("Looking in path: {}", path);

    let data = load_datafile(datfile);
    let mut bundles = game_bundles(&data);

    let mut files = list_files(path);
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

    write_all_zip(bundles, destination);
}
