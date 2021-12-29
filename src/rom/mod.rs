use crate::indicatif::ParallelProgressIterator;

use crate::logiqx;
use crate::walkdir::{DirEntry, WalkDir};
use dpc_pariter::IteratorExt;
use rayon::prelude::*;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{fs, io};

pub mod zip;

pub fn files(dir: PathBuf) -> Vec<File> {
    let list = file_list(&dir);
    let result: Vec<File> = list
        .par_iter()
        .progress_count(list.len() as u64)
        .map(|file| {
            let mut file = file.clone();
            file.sha1 = compute_sha1(&file.path);
            file
        })
        .collect();
    result
}

fn file_list(dir: &PathBuf) -> Vec<File> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| File::entry_is_relevant(e))
        .filter_map(|v| v.ok())
        .filter(|entry| entry.file_type().is_file())
        .parallel_map(|entry| File::new(&entry))
        .collect()
}

fn compute_sha1(path: &PathBuf) -> String {
    let mut file = fs::File::open(path).unwrap();
    let mut hasher = Sha1::new();
    let _n = io::copy(&mut file, &mut hasher);
    format!("{:x}", hasher.finalize())
}

pub fn files_by_sha1(files: &[File]) -> HashMap<String, File> {
    files
        .iter()
        .map(|file| (file.sha1().to_string(), file.clone()))
        .collect()
}

#[derive(Debug, Clone)]
pub struct File {
    path: PathBuf,
    sha1: String,
}

impl File {
    pub fn new(entry: &DirEntry) -> Self {
        let path: PathBuf = entry.path().into();
        File {
            sha1: compute_sha1(&path),
            path: path,
        }
    }
    pub fn entry_is_relevant(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| entry.depth() == 0 || !s.starts_with('.'))
            .unwrap_or(false)
    }

    /// Get a reference to the file's sha1.
    pub fn sha1(&self) -> &str {
        self.sha1.as_ref()
    }
}

#[derive(Debug)]
pub struct Bundle {
    pub name: String,                            // 7z name
    pub files: HashMap<String, String>,          // sha1 key, rom file name
    pub matches: Vec<(String, String, PathBuf)>, // sha1, destination, File for matching files
}

impl Bundle {
    pub fn new(game: &logiqx::Game, match_map: &HashMap<String, File>) -> Self {
        let files = Self::load_files_from_roms(&game.roms);
        let matches = Self::get_matches(&files, match_map);
        Bundle {
            name: game.name.to_string(),
            files,
            matches,
        }
    }

    pub fn from_datafile(datafile: &logiqx::Datafile, files: &[File]) -> Vec<Bundle> {
        let file_map = files_by_sha1(&files);
        datafile
            .games
            .iter()
            .map(|game| Bundle::new(game, &file_map))
            .collect()
    }

    pub fn load_files_from_roms(roms: &[logiqx::Rom]) -> HashMap<String, String> {
        roms.iter()
            .map(|rom| Self::get_sha_and_destination_name(rom))
            .collect()
    }
    pub fn get_sha_and_destination_name(rom: &logiqx::Rom) -> (String, String) {
        (rom.sha1.to_string().to_lowercase(), rom.name.to_string())
    }

    fn get_matches(
        files: &HashMap<String, String>,
        match_map: &HashMap<String, File>,
    ) -> Vec<(String, String, PathBuf)> {
        files
            .iter()
            .filter_map(|(sha, name)| match match_map.get(sha) {
                Some(file) => Some((sha.to_string(), name.to_string(), file.path.to_path_buf())),
                None => None,
            })
            .collect()
    }
}
