use std::{
    fs::File,
    io::{Cursor, Read},
};

use camino::Utf8Path;
use serde::Deserialize;
use xml::reader::{ParserConfig, XmlEvent};

use super::game::Game;
use super::header::Header;

use crate::{document_input, hashes};

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
        let raw = document_input::read_bounded(reader, document_input::MAX_DOCUMENT_BYTES)?;
        Self::from_bytes(&raw)
    }

    pub fn from_path(path: &Utf8Path) -> crate::Result<Self> {
        let raw =
            document_input::read_bounded(File::open(path)?, document_input::MAX_DOCUMENT_BYTES)?;
        let mut data_file = Self::from_bytes(&raw)?;
        data_file.file_name = path
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        data_file.sha1 = Some(hashes::sha1_bytes(&raw).to_vec());
        Ok(data_file)
    }

    fn from_bytes(raw: &[u8]) -> crate::Result<Self> {
        let xml = document_input::decode_xml(raw)?;
        validate_xml(&xml)?;
        Ok(serde_xml_rs::SerdeXml::new()
            .parser(
                ParserConfig::new()
                    .max_entity_expansion_length(1024)
                    .max_entity_expansion_depth(4)
                    .max_name_length(4096)
                    .max_attributes(1024)
                    .max_attribute_length(1024 * 1024)
                    .max_data_length(1024 * 1024)
                    .allow_multiple_root_elements(false),
            )
            .from_reader(Cursor::new(xml.as_ref()))?)
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
    pub fn sha1(&self) -> Option<&[u8]> {
        self.sha1.as_deref()
    }

    /// Get a reference to the data file's file name.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.file_name.as_deref()
    }

    /// Get a reference to the data file's build.
    #[must_use]
    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }

    /// Get a reference to the data file's debug.
    #[must_use]
    pub fn debug(&self) -> Option<&str> {
        self.debug.as_deref()
    }
}

fn validate_xml(bytes: &[u8]) -> crate::Result<()> {
    if bytes
        .windows(b"<!ENTITY".len())
        .any(|marker| marker == b"<!ENTITY")
    {
        return Err(crate::Error::XmlEntityNotAllowed);
    }
    let config = ParserConfig::new()
        .max_entity_expansion_length(1024)
        .max_entity_expansion_depth(4)
        .max_name_length(4096)
        .max_attributes(1024)
        .max_attribute_length(1024 * 1024)
        .max_data_length(1024 * 1024)
        .allow_multiple_root_elements(false);
    for event in config.create_reader(bytes) {
        match event {
            // xml-rs reports the declaration without fetching external subsets. The
            // constrained parser config limits any document-local expansion.
            Ok(XmlEvent::Doctype { .. } | _) => {}
            Err(error) => return Err(crate::Error::XmlValidation(error.to_string())),
        }
    }
    Ok(())
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
    fn rejects_external_entity_expansion_without_rejecting_standard_logiqx_doctypes()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat = br#"<?xml version="1.0"?>
<!DOCTYPE datafile [<!ENTITY remote SYSTEM "http://127.0.0.1:9/catalog.dtd">]>
<datafile><header><name>&remote;</name></header></datafile>"#;
        assert!(DataFile::from_reader(dat.as_slice()).is_err());

        let standard = br#"<!DOCTYPE datafile PUBLIC "-//Logiqx//DTD ROM Management Datafile//EN" "http://www.logiqx.com/Dats/datafile.dtd"><datafile><header><name>Standard</name></header></datafile>"#;
        assert_eq!(
            DataFile::from_reader(standard.as_slice())?.header().name(),
            "Standard"
        );
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
        assert_eq!(game.sourcefile_opt(), Some("pong.c"));
        assert_eq!(game.year_opt(), Some("1972"));
        assert_eq!(game.manufacturer_opt(), Some("Atari"));
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
        assert_eq!(rom.size(), Some(4096));
        assert_eq!(
            hex::encode(rom.sha1().ok_or("missing SHA1")?),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex::encode(rom.md5().ok_or("missing MD5")?),
            "900150983cd24fb0d6963f7d28e17f72"
        );
        assert_eq!(hex::encode(rom.crc().ok_or("missing CRC")?), "12345678");
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
        assert_eq!(clone.cloneof(), Some("parent"));
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
    fn optional_logiqx_fields_use_expected_defaults() -> Result<(), Box<dyn std::error::Error>> {
        let minimal_game = r#"<?xml version="1.0"?>
<datafile build="2024-01-01" debug="no">
  <header>
    <name>Minimal Game</name>
  </header>
  <game name="empty">
  </game>
</datafile>"#;

        let df = DataFile::from_reader(minimal_game.as_bytes())?;
        let game = df
            .games()
            .first()
            .ok_or_else(|| io::Error::other("missing game"))?;

        assert_eq!(df.build(), Some("2024-01-01"));
        assert_eq!(df.debug(), Some("no"));
        assert!(df.file_name().is_none());
        assert!(df.sha1().is_none());
        assert_eq!(game.sourcefile_opt(), None);
        assert_eq!(game.isbios_opt(), None);
        assert_eq!(game.cloneof(), None);
        assert!(game.roms().is_empty());
        Ok(())
    }

    #[test]
    fn optional_rom_metadata_and_partial_hashes_are_preserved()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat = r#"<datafile><header><name>Optional</name></header>
<game name="partial"><rom name="large.bin" size="4294967296" crc="ABCDEF01"/></game>
</datafile>"#;
        let df = DataFile::from_reader(dat.as_bytes())?;
        let rom = &df.games()[0].roms()[0];
        assert_eq!(rom.size(), Some(4_294_967_296));
        assert_eq!(rom.crc(), Some(&[0xab, 0xcd, 0xef, 0x01][..]));
        assert_eq!(rom.md5(), None);
        assert_eq!(rom.sha1(), None);
        assert_eq!(rom.merge(), None);
        assert_eq!(rom.status(), None);
        assert_eq!(rom.serial(), None);
        assert_eq!(rom.date(), None);
        Ok(())
    }

    #[test]
    fn malformed_supplied_hashes_are_rejected() {
        for (attribute, value) in [("crc", "xyz"), ("md5", "xyz"), ("sha1", "xyz")] {
            let dat = format!(
                "<datafile><header><name>Bad</name></header><game name=\"g\"><rom name=\"r\" size=\"1\" {attribute}=\"{value}\"/></game></datafile>"
            );
            assert!(
                DataFile::from_reader(dat.as_bytes()).is_err(),
                "{attribute}"
            );
        }
    }

    #[test]
    fn game_metadata_is_optional_and_device_relationships_are_kept()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat = r#"<datafile><header><name>Game metadata</name></header>
<game name="machine" cloneof="parent" romof="bios" sampleof="samples" isbios="yes">
  <device_ref name="sound-chip"/>
</game><game name="minimal"/>
</datafile>"#;
        let df = DataFile::from_reader(dat.as_bytes())?;
        let game = &df.games()[0];
        assert_eq!(game.isbios_opt(), Some("yes"));
        assert_eq!(game.romof_opt(), Some("bios"));
        assert_eq!(game.sampleof_opt(), Some("samples"));
        assert_eq!(game.device_refs().collect::<Vec<_>>(), ["sound-chip"]);
        let minimal = &df.games()[1];
        assert_eq!(minimal.sourcefile_opt(), None);
        assert_eq!(minimal.isbios_opt(), None);
        assert_eq!(minimal.romof_opt(), None);
        assert_eq!(minimal.sampleof_opt(), None);
        assert_eq!(minimal.board_opt(), None);
        assert_eq!(minimal.rebuildto_opt(), None);
        assert_eq!(minimal.year_opt(), None);
        assert_eq!(minimal.manufacturer_opt(), None);
        Ok(())
    }

    #[test]
    fn invalid_hex_in_dat_xml_fails_parsing() {
        let invalid = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Invalid</name>
  </header>
  <game name="bad">
    <rom name="bad.rom" size="1" sha1="not-hex" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#;

        assert!(DataFile::from_reader(invalid.as_bytes()).is_err());
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
