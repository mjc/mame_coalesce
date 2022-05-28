use std::fs;

use camino::Utf8Path;

use super::game::Game;
use super::header::Header;

use crate::{hashes, MameResult};

#[derive(Debug, Deserialize)]
pub struct DataFile {
    file_name: Option<String>,
    #[serde(default)]
    build: Option<String>,
    #[serde(default)]
    debug: Option<String>, // bool
    header: Header,
    sha1: Option<Vec<u8>>,
    #[serde(rename = "game", default)]
    games: Vec<Game>,
}
impl DataFile {
    pub fn from_path(path: &Utf8Path) -> MameResult<Self> {
        let mmap = hashes::mmap_path(path)?;
        let sha1 = hashes::stream_sha1(&mmap);

        let contents = fs::read_to_string(path)?;

        let mut data_file: Self = quick_xml::de::from_str(&contents)?;
        {
            let full_path = path.canonicalize().ok();
            data_file.file_name = full_path.map(|p| p.to_string_lossy().into_owned());
        }
        data_file.sha1 = Some(sha1);
        Ok(data_file)
    }

    /// Get a reference to the data file's header.
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Get a reference to the data file's games.
    pub fn games(&self) -> &[Game] {
        self.games.as_ref()
    }

    /// Get a reference to the data file's sha1.
    pub const fn sha1(&self) -> Option<&Vec<u8>> {
        self.sha1.as_ref()
    }

    /// Get a reference to the data file's file name.
    pub const fn file_name(&self) -> Option<&String> {
        self.file_name.as_ref()
    }

    /// Set the data file's file name.
    pub fn set_file_name(&mut self, file_name: Option<String>) {
        self.file_name = file_name;
    }

    /// Get a reference to the data file's build.
    pub const fn build(&self) -> Option<&String> {
        self.build.as_ref()
    }

    /// Get a reference to the data file's debug.
    pub const fn debug(&self) -> Option<&String> {
        self.debug.as_ref()
    }
}
