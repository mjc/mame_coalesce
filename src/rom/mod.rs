use crate::logiqx;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::{fs, io};
use std::path::PathBuf;
use crate::walkdir::{WalkDir, DirEntry};

pub mod zip;

pub fn list_files(dir: PathBuf) -> Vec<File> {
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

pub fn compute_sha1(path: &PathBuf) -> Option<String> {
    let mut file = fs::File::open(path).unwrap();
    let mut hasher = Sha1::new();
    let _n = io::copy(&mut file, &mut hasher);
    Some(format!("{:x}", hasher.result()))
}

pub fn compute_all_sha1(files: &mut Vec<File>) {
    files
        .par_iter_mut()
        .for_each(|file| file.sha1 = compute_sha1(&file.path));
}

pub fn add_matches_to_bundles(bundles: &mut Vec<Bundle>, files: &HashMap<String, File>) {
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

fn get_key(file: &File) -> String {
    file.sha1.as_ref().unwrap().to_string()
}

pub fn files_by_sha1(files: &Vec<File>) -> HashMap<String, File> {
        files
        .iter()
        .map(|file| (get_key(file), file.clone()))
        .collect()
}


#[derive(Debug, Clone)]
pub struct File {
    pub path: PathBuf,
    pub sha1: Option<String>,
}

impl File {
    pub fn new(entry: &DirEntry) -> Self {
        File {
            sha1: None,
            path: entry.path().to_path_buf(),
        }
    }
    pub fn entry_is_relevant(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| entry.depth() == 0 || !s.starts_with("."))
            .unwrap_or(false)
    }
}

#[derive(Debug)]
pub struct Bundle {
    pub name: String,                            // 7z name
    pub files: HashMap<String, String>,          // sha1 key, rom file name
    pub matches: Vec<(String, String, PathBuf)>, // sha1, destination, File for matching files
}

impl Bundle {
    pub fn new(game: &logiqx::Game) -> Self {
        Bundle {
             name: game.name.to_string(),
             files: Self::load_files_from_roms(&game.roms),
             matches: Vec::<(String, String, PathBuf)>::new(),
        }
    }

    pub fn from_datafile(datafile: &logiqx::Datafile) -> Vec<Bundle> {
        datafile
            .games
            .iter()
            .map(|game| Bundle::new(game))
            .collect()
    }

    pub fn load_files_from_roms(roms: &Vec<logiqx::Rom>) -> HashMap<String, String> {
        roms.iter()
            .map(|rom| Self::get_sha_and_destination_name(rom))
            .collect()
    }
    pub fn get_sha_and_destination_name(rom: &logiqx::Rom) -> (String, String) {
        (rom.sha1.to_string().to_lowercase(), rom.name.to_string())
    }
}
