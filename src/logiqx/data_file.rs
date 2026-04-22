use std::io::Read;

use camino::Utf8Path;
use fmmap::MmapFileExt;
use serde::Deserialize;

use super::game::Game;
use super::header::Header;

use crate::hashes;

#[derive(Debug, Deserialize)]
pub struct DataFile {
    file_name: Option<String>,
    #[serde(rename = "@build", default)]
    build: Option<String>,
    #[serde(rename = "@debug", default)]
    debug: Option<String>, // bool
    header: Header,
    sha1: Option<Vec<u8>>,
    #[serde(rename = "game", default)]
    games: Vec<Game>,
}
impl DataFile {
    pub fn from_reader<R: Read>(reader: R) -> crate::Result<Self> {
        let data_file: Self = serde_xml_rs::from_reader(reader)?;
        Ok(data_file)
    }

    pub fn from_path(path: &Utf8Path) -> crate::Result<Self> {
        let mmap = hashes::mmap_path(path)?;
        let sha1 = hashes::stream_sha1(&mmap);
        let reader = mmap
            .reader(0)
            .map_err(|e| crate::Error::Mmap(e.to_string()))?;

        let mut data_file = Self::from_reader(reader)?;
        data_file.file_name = path
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        data_file.sha1 = Some(sha1);
        Ok(data_file)
    }

    /// Get a reference to the data file's header.
    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Get a reference to the data file's games.
    #[must_use]
    pub fn games(&self) -> &[Game] {
        self.games.as_ref()
    }

    /// Get a reference to the data file's sha1.
    #[must_use]
    pub const fn sha1(&self) -> Option<&Vec<u8>> {
        self.sha1.as_ref()
    }

    /// Get a reference to the data file's file name.
    #[must_use]
    pub const fn file_name(&self) -> Option<&String> {
        self.file_name.as_ref()
    }

    /// Set the data file's file name.
    pub fn set_file_name(&mut self, file_name: Option<String>) {
        self.file_name = file_name;
    }

    /// Get a reference to the data file's build.
    #[must_use]
    pub const fn build(&self) -> Option<&String> {
        self.build.as_ref()
    }

    /// Get a reference to the data file's debug.
    #[must_use]
    pub const fn debug(&self) -> Option<&String> {
        self.debug.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    const SIMPLE_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Test Set</name>
    <description>Test Description</description>
    <version>1.0</version>
    <author>Tester</author>
  </header>
  <game name="pong" sourcefile="pong.c">
    <description>Pong</description>
    <year>1972</year>
    <manufacturer>Atari</manufacturer>
    <rom name="pong.rom" size="4096" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#;

    const CLONE_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Clone Set</name>
    <description>Test with clones</description>
    <version>1.0</version>
    <author>Tester</author>
  </header>
  <game name="parent" sourcefile="parent.c">
    <description>Parent Game</description>
    <year>1980</year>
    <manufacturer>Acme</manufacturer>
    <rom name="parent.rom" size="8192" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="aabbccdd"/>
  </game>
  <game name="clone1" cloneof="parent" sourcefile="clone.c">
    <description>Clone Game</description>
    <year>1981</year>
    <manufacturer>Acme</manufacturer>
    <rom name="clone.rom" size="8192" sha1="84983e441c3bd26ebaae4aa1f575527d004816f2" md5="f96b697d7cb7938d525a2f31aaf161d0" crc="bbccddee"/>
  </game>
</datafile>"#;

    #[test]
    fn parse_simple_dat() -> Result<(), Box<dyn std::error::Error>> {
        let df = DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
        assert_eq!(df.header().name(), "Test Set");
        assert_eq!(
            df.header().description().map(std::string::String::as_str),
            Some("Test Description")
        );
        assert_eq!(
            df.header().version().map(std::string::String::as_str),
            Some("1.0")
        );
        assert_eq!(
            df.header().author().map(std::string::String::as_str),
            Some("Tester")
        );
        assert_eq!(df.games().len(), 1);
        Ok(())
    }

    #[test]
    fn parse_game_fields() -> Result<(), Box<dyn std::error::Error>> {
        let df = DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
        let game = df
            .games()
            .first()
            .ok_or_else(|| io::Error::other("missing game"))?;
        assert_eq!(game.name(), "pong");
        assert_eq!(game.sourcefile(), "pong.c");
        assert_eq!(game.year(), "1972");
        assert_eq!(game.manufacturer(), "Atari");
        assert!(game.cloneof().is_none());
        Ok(())
    }

    #[test]
    fn parse_rom_hashes() -> Result<(), Box<dyn std::error::Error>> {
        let df = DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
        let game = df
            .games()
            .first()
            .ok_or_else(|| io::Error::other("missing game"))?;
        let rom = game
            .roms()
            .first()
            .ok_or_else(|| io::Error::other("missing rom"))?;
        assert_eq!(rom.name(), "pong.rom");
        assert_eq!(*rom.size(), 4096);
        assert_eq!(
            hex::encode(rom.sha1()),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex::encode(rom.md5()), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex::encode(rom.crc()), "12345678");
        Ok(())
    }

    #[test]
    fn parse_parent_and_clone() -> Result<(), Box<dyn std::error::Error>> {
        let df = DataFile::from_reader(CLONE_DAT.as_bytes())?;
        assert_eq!(df.games().len(), 2);
        let parent = df
            .games()
            .iter()
            .find(|g| g.name() == "parent")
            .ok_or_else(|| io::Error::other("missing parent"))?;
        let clone = df
            .games()
            .iter()
            .find(|g| g.name() == "clone1")
            .ok_or_else(|| io::Error::other("missing clone"))?;
        assert!(parent.cloneof().is_none());
        assert_eq!(
            clone.cloneof().map(std::string::String::as_str),
            Some("parent")
        );
        Ok(())
    }

    #[test]
    fn parse_optional_header_fields() -> Result<(), Box<dyn std::error::Error>> {
        let minimal = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Minimal</name>
  </header>
</datafile>"#;
        let df = DataFile::from_reader(minimal.as_bytes())?;
        assert_eq!(df.header().name(), "Minimal");
        assert!(df.header().description().is_none());
        assert!(df.header().version().is_none());
        assert!(df.header().author().is_none());
        assert!(df.header().homepage().is_none());
        assert!(df.header().url().is_none());
        assert_eq!(df.games().len(), 0);
        Ok(())
    }

    #[test]
    fn parse_fixture_sega_dat() -> Result<(), Box<dyn std::error::Error>> {
        let path = camino::Utf8Path::new(
            "fixtures/Sega - Master System - Mark III Parent-Clone (20160331-213351).dat",
        );
        if path.exists() {
            let df = DataFile::from_path(path)?;
            assert_eq!(
                df.header().name(),
                "Sega - Master System - Mark III Parent-Clone"
            );
            assert!(!df.games().is_empty());
        }
        Ok(())
    }
}
