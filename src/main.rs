extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;
extern crate walkdir;

use rayon::prelude::*;

use sha1::{Digest, Sha1};

use std::path::PathBuf;
use std::{env, fs, io};

use walkdir::{DirEntry, WalkDir};

mod logiqx {
    #[derive(Debug, Deserialize)]
    pub struct Datafile {
        #[serde(default)]
        build: String,
        #[serde(default)]
        debug: String, // bool
        header: Header,
        #[serde(rename = "game", default)]
        games: Vec<Game>,
    }
    #[derive(Debug, Deserialize)]
    pub struct Header {
        name: String,
        description: String,
        version: String,
        author: String,
        homepage: String,
        url: String,
    }
    #[derive(Debug, Deserialize)]
    pub struct Game {
        name: String,
        #[serde(default)]
        sourcefile: String,
        #[serde(default)]
        isbios: String, // bool
        #[serde(default)]
        cloneof: String,
        #[serde(default)]
        romof: String,
        #[serde(default)]
        sampleof: String,
        #[serde(default)]
        board: String,
        #[serde(default)]
        rebuildto: String,
        #[serde(default)]
        year: String, // should probably be a DateTime
        #[serde(default)]
        manufacturer: String,
        #[serde(rename = "rom", default)]
        roms: Vec<Rom>,
    }
    #[derive(Debug, Deserialize)]
    pub struct Rom {
        name: String,
        size: String,
        md5: String,
        sha1: String,
        crc: String,
        #[serde(default)]
        merge: String,
        #[serde(default)]
        status: String, // baddump|nodump|good|verified
        #[serde(default)]
        serial: String,
        #[serde(default)]
        date: String, // should probably be DateTime
    }
}

#[derive(Debug)]
struct File {
    path: PathBuf,
    sha1: Option<String>,
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let datfile = &args[1];
    let path = &args[2];
    println!("Using datfile: {}", datfile);
    println!("Looking in path: {}", path);

    let data = load_datafile(datfile);

    let mut files = list_files(path);
    println!("Files to check: {}", files.len());

    compute_all_sha1(&mut files);

    println!(
        "sha1 of last file: {:?}",
        files.last().unwrap().sha1.as_ref().unwrap()
    );
}
