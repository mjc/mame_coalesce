use sha1::{Digest, Sha1};

use super::game::Game;
use super::header::Header;

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
    pub fn from_str(contents: &str) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(contents);
        let mut data_file: DataFile =
            serde_xml_rs::from_str(contents).expect("Can't read Logiqx datafile.");
        // this should probably happen before we bother parsing, at the call site for this
        data_file.sha1 = Some(hasher.finalize().to_vec());
        data_file
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
}
