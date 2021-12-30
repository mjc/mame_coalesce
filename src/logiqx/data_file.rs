use super::game::Game;
use super::header::Header;

#[derive(Debug, Deserialize)]
pub struct DataFile {
    #[serde(default)]
    build: String,
    #[serde(default)]
    debug: String, // bool
    header: Header,
    #[serde(rename = "game", default)]
    games: Vec<Game>,
}
impl DataFile {
    pub fn from_str(contents: &str) -> Self {
        serde_xml_rs::from_str(contents).expect("Can't read Logiqx datafile.")
    }

    /// Get a reference to the data file's build.
    pub fn build(&self) -> &str {
        self.build.as_ref()
    }

    /// Get a reference to the data file's debug.
    pub fn debug(&self) -> &str {
        self.debug.as_ref()
    }

    /// Get a reference to the data file's header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get a reference to the data file's games.
    pub fn games(&self) -> &[Game] {
        self.games.as_ref()
    }
}
