use std::path::PathBuf;
use crate::walkdir::DirEntry;

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
