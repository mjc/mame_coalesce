use std::{fs::File, io::BufReader};

use super::game::Game;
use super::header::Header;

use crate::hashes::MultiHash;

#[derive(Debug, Deserialize)]
pub struct DataFile {
    pub file_name: Option<String>,
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub debug: String, // bool
    header: Header,
    sha1: Option<Vec<u8>>,
    #[serde(rename = "game", default)]
    games: Vec<Game>,
}
impl DataFile {
    pub fn from_file(contents: &File) -> Result<Self, serde_xml_rs::Error> {
        let reader = BufReader::new(contents);
        // this should be BufReader or reader should be mmap, idk
        let (_crc, sha1) = contents.all_hashes();
        let mut data_file: DataFile = serde_xml_rs::from_reader(reader)?;
        data_file.sha1 = Some(sha1);
        Ok(data_file)
    }

    /// Get a reference to the data file's header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get a reference to the data file's games.
    pub fn games(&self) -> &[Game] {
        self.games.as_ref()
    }

    /// Get a reference to the data file's build.
    pub fn build(&self) -> &str {
        self.build.as_ref()
    }

    /// Get a reference to the data file's debug.
    pub fn debug(&self) -> &str {
        self.debug.as_ref()
    }

    /// Get a reference to the data file's sha1.
    pub fn sha1(&self) -> Option<&Vec<u8>> {
        self.sha1.as_ref()
    }

    /// Get a reference to the data file's file name.
    pub fn file_name(&self) -> Option<&String> {
        self.file_name.as_ref()
    }

    /// Set the data file's file name.
    pub fn set_file_name(&mut self, file_name: Option<String>) {
        self.file_name = file_name;
    }
}
