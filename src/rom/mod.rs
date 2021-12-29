use crate::indicatif::ProgressIterator;
use crate::logiqx;
use crate::walkdir::{DirEntry, WalkDir};
use dpc_pariter::IteratorExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha1::{Digest, Sha1};
use std::path::PathBuf;
use std::{collections::HashMap, f32::consts::E};
use std::{fs, io};

pub mod zip;

pub fn files(dir: &PathBuf) -> Vec<File> {
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner} #{pos} at {per_sec} [{elapsed_precise}] {msg}"),
    );
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| File::entry_is_relevant(e))
        .progress_with(pb)
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
    mime: String,
    contents: HashMap<String, Option<String>>,
}

impl File {
    pub fn new(entry: &DirEntry) -> Self {
        let path: PathBuf = entry.path().into();
        let sha1 = compute_sha1(&path);
        let mime = tree_magic::from_filepath(&path);
        let contents = HashMap::new();
        File {
            sha1,
            mime,
            path,
            contents,
        }
    }

    pub fn populate_archive_contents(&mut self) {
        if !Self::is_archive(&self.mime) {
            let file = std::fs::File::open(&self.path)
                .expect(format!("can't open file: {:?}", &self.path).as_str());
            let files = compress_tools::list_archive_files(file)
                .expect(format!("can't parse: {:?}", &self.path).as_str());
            self.contents = files.iter().map(|file| (file.to_owned(), None)).collect();
        } else {
            ()
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

    /// Get a reference to the file's mime.
    pub fn mime(&self) -> &str {
        self.mime.as_ref()
    }

    pub fn is_archive(mime: &str) -> bool {
        match mime {
            "application/zip" => true,
            "application/x-7z-compressed" => true,
            _ => false,
        }
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
