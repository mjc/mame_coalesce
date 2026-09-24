use std::io::{Cursor, Read};

use camino::Utf8Path;
use fmmap::MmapFileExt;
use serde::Deserialize;
use xml::common::Position;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A one-based XML parser position immediately after an opening record tag.
pub struct RecordLocation {
    pub line: i64,
    pub column: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedAttribute {
    pub record_kind: String,
    pub record_name: Option<String>,
    pub field_name: String,
    pub namespace_uri: Option<String>,
    pub value: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct XmlSourceMap {
    pub game_locations: Vec<RecordLocation>,
    pub rom_locations: Vec<Vec<RecordLocation>>,
    pub unsupported_attributes: Vec<UnsupportedAttribute>,
}
impl DataFile {
    pub fn from_reader<R: Read>(reader: R) -> crate::Result<Self> {
        let raw = document_input::read_bounded(reader, document_input::MAX_DOCUMENT_BYTES)?;
        Self::parse_bytes(&raw).map(|(data_file, _)| data_file)
    }

    pub fn from_path(path: &Utf8Path) -> crate::Result<Self> {
        // Keep large path-based DATs out of the eager heap while preserving their
        // historical size behavior. `from_reader` remains deliberately bounded.
        let mmap = hashes::mmap_path(path)?;
        let raw = mmap.as_slice();
        let (mut data_file, _) = Self::parse_bytes(&raw)?;
        data_file.file_name = path
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        data_file.sha1 = Some(hashes::sha1_bytes(raw).to_vec());
        Ok(data_file)
    }

    pub(crate) fn from_reader_with_source_map<R: Read>(
        reader: R,
    ) -> crate::Result<(Self, XmlSourceMap)> {
        let raw = document_input::read_bounded(reader, document_input::MAX_DOCUMENT_BYTES)?;
        Self::parse_bytes(&raw)
    }

    pub(crate) fn validate_document_bytes(raw: &[u8]) -> crate::Result<()> {
        let xml = document_input::decode_xml(raw)?;
        validate_xml(&xml).map(|_| ())
    }

    fn parse_bytes(raw: &[u8]) -> crate::Result<(Self, XmlSourceMap)> {
        let xml = document_input::decode_xml(raw)?;
        let source_map = validate_xml(&xml)?;
        let data_file = serde_xml_rs::SerdeXml::new()
            .parser(
                ParserConfig::new()
                    .trim_whitespace(true)
                    .whitespace_to_characters(true)
                    .cdata_to_characters(true)
                    .ignore_comments(true)
                    .coalesce_characters(true)
                    .max_entity_expansion_length(1024)
                    .max_entity_expansion_depth(4)
                    .max_name_length(4096)
                    .max_attributes(1024)
                    .max_attribute_length(1024 * 1024)
                    .max_data_length(1024 * 1024)
                    .allow_multiple_root_elements(false),
            )
            .from_reader(Cursor::new(xml.as_ref()))?;
        Ok((data_file, source_map))
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

fn validate_xml(bytes: &[u8]) -> crate::Result<XmlSourceMap> {
    if contains_entity_declaration(bytes)
        .map_err(|()| crate::Error::XmlValidation("malformed UTF-16 encoding".into()))?
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
    let mut reader = config.create_reader(bytes);
    let mut source_map = XmlSourceMap::default();
    let mut current_game = None;
    loop {
        let event = reader.next();
        match event {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                let local_name = name.local_name;
                let position = reader.position();
                let location = RecordLocation {
                    line: i64::try_from(position.row.saturating_add(1)).unwrap_or(i64::MAX),
                    column: i64::try_from(position.column.saturating_add(1)).unwrap_or(i64::MAX),
                };
                let record_name = attributes
                    .iter()
                    .find(|attribute| attribute.name.local_name == "name")
                    .map(|attribute| attribute.value.clone());
                match local_name.as_str() {
                    "game" => {
                        current_game = Some(source_map.game_locations.len());
                        source_map.game_locations.push(location);
                        source_map.rom_locations.push(Vec::new());
                    }
                    "rom" => {
                        if let Some(index) = current_game {
                            source_map.rom_locations[index].push(location);
                        }
                    }
                    _ => {}
                }
                for attribute in &attributes {
                    if attribute.name.namespace.is_some()
                        || !known_attribute(&local_name, &attribute.name.local_name)
                    {
                        let record_kind = match local_name.as_str() {
                            "game" => "game",
                            "rom" => "rom",
                            _ => "document",
                        };
                        source_map
                            .unsupported_attributes
                            .push(UnsupportedAttribute {
                                record_kind: record_kind.to_owned(),
                                record_name: record_name.clone(),
                                field_name: attribute.name.local_name.clone(),
                                namespace_uri: attribute.name.namespace.clone(),
                                value: attribute.value.clone(),
                                location,
                            });
                    }
                }
            }
            Ok(XmlEvent::EndElement { name }) => {
                if name.local_name == "game" {
                    current_game = None;
                }
            }
            Ok(XmlEvent::EndDocument) => break,
            // xml-rs reports declarations without fetching external subsets.
            // Entity declarations were rejected above.
            Ok(XmlEvent::Doctype { .. } | _) => {}
            Err(error) => return Err(crate::Error::XmlValidation(error.to_string())),
        }
    }
    Ok(source_map)
}

fn known_attribute(element: &str, attribute: &str) -> bool {
    let known = match element {
        "datafile" => &["build", "debug"][..],
        "game" => &[
            "name",
            "sourcefile",
            "isbios",
            "cloneof",
            "romof",
            "sampleof",
            "board",
            "rebuildto",
        ][..],
        "rom" => &[
            "name", "size", "md5", "sha1", "crc", "merge", "status", "serial", "date",
        ][..],
        "device_ref" => &["name"][..],
        _ => &[][..],
    };
    known.contains(&attribute)
}

fn contains_entity_declaration(bytes: &[u8]) -> Result<bool, ()> {
    let decoded = utf16_inspection_bytes(bytes)?;
    let bytes = decoded.as_deref().unwrap_or(bytes);
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"<!--") {
            index = after_markup(bytes, index + b"<!--".len(), b"-->");
        } else if bytes[index..].starts_with(b"<![CDATA[") {
            index = after_markup(bytes, index + b"<![CDATA[".len(), b"]]>");
        } else if bytes[index..].starts_with(b"<?") {
            index = after_markup(bytes, index + b"<?".len(), b"?>");
        } else if bytes[index..].starts_with(b"<!DOCTYPE") {
            let (has_entity, end) =
                doctype_contains_entity_declaration(bytes, index + b"<!DOCTYPE".len());
            if has_entity {
                return Ok(true);
            }
            index = end;
        } else {
            index += 1;
        }
    }
    Ok(false)
}

/// Return a byte-oriented inspection view for UTF-16, preserving ASCII markup
/// while replacing non-ASCII characters with non-markup bytes.
fn utf16_inspection_bytes(bytes: &[u8]) -> Result<Option<Vec<u8>>, ()> {
    let (encoding, start) = if bytes.starts_with(&[0xFE, 0xFF]) {
        (Some(true), 2)
    } else if bytes.starts_with(&[0xFF, 0xFE]) {
        (Some(false), 2)
    } else if bytes.starts_with(b"<\0?\0") {
        (Some(false), 0)
    } else if bytes.starts_with(b"\0<\0?") {
        (Some(true), 0)
    } else {
        (None, 0)
    };
    let Some(big_endian) = encoding else {
        return Ok(None);
    };
    let encoded = bytes.get(start..).ok_or(())?;
    let (pairs, remainder) = encoded.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(());
    }
    let units = pairs.iter().map(|pair| {
        if big_endian {
            u16::from_be_bytes([pair[0], pair[1]])
        } else {
            u16::from_le_bytes([pair[0], pair[1]])
        }
    });
    let mut inspection = Vec::with_capacity(encoded.len() / 2);
    for character in char::decode_utf16(units) {
        let character = character.map_err(|_| ())?;
        inspection.push(if character.is_ascii() {
            character as u8
        } else {
            0x80
        });
    }
    Ok(Some(inspection))
}

fn doctype_contains_entity_declaration(bytes: &[u8], mut index: usize) -> (bool, usize) {
    let mut subset_depth = 0_usize;
    let mut quote = None;
    while index < bytes.len() {
        if let Some(delimiter) = quote {
            if bytes[index] == delimiter {
                quote = None;
            }
            index += 1;
        } else if bytes[index..].starts_with(b"<!--") {
            index = after_markup(bytes, index + b"<!--".len(), b"-->");
        } else if bytes[index..].starts_with(b"<?") {
            index = after_markup(bytes, index + b"<?".len(), b"?>");
        } else if bytes[index..].starts_with(b"<!ENTITY") && subset_depth > 0 {
            return (true, index + b"<!ENTITY".len());
        } else {
            match bytes[index] {
                b'\'' | b'"' => quote = Some(bytes[index]),
                b'[' => subset_depth += 1,
                b']' => subset_depth = subset_depth.saturating_sub(1),
                b'>' if subset_depth == 0 => return (false, index + 1),
                _ => {}
            }
            index += 1;
        }
    }
    (false, bytes.len())
}

fn after_markup(bytes: &[u8], mut index: usize, terminator: &[u8]) -> usize {
    while index < bytes.len() {
        if bytes[index..].starts_with(terminator) {
            return index + terminator.len();
        }
        index += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

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
    fn preserves_mixed_text_and_cdata_and_ignores_entity_text_in_comments_and_cdata()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat = br#"<?note <!DOCTYPE datafile [<!ENTITY fake "value">]?>
<datafile><!-- <!ENTITY comment "ignored"> --><header><name>Test <![CDATA[& stuff <!ENTITY literal>]]></name></header></datafile>"#;
        let parsed = DataFile::from_reader(dat.as_slice())?;
        assert_eq!(parsed.header().name(), "Test & stuff <!ENTITY literal>");
        Ok(())
    }

    #[test]
    fn parser_source_map_retains_unknown_attributes_and_record_positions()
    -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<datafile>
  <header><name>Source map</name></header>
  <game name="set" future="preserve">
    <rom name="asset.bin" size="4" future-hash="unknown"/>
  </game>
</datafile>"#;
        let (data_file, source_map) = DataFile::from_reader_with_source_map(xml.as_slice())?;
        assert_eq!(data_file.games().len(), 1);
        assert_eq!(source_map.game_locations[0].line, 3);
        assert_eq!(source_map.rom_locations[0][0].line, 4);
        assert_eq!(source_map.unsupported_attributes.len(), 2);
        assert_eq!(source_map.unsupported_attributes[0].field_name, "future");
        assert_eq!(source_map.unsupported_attributes[1].value, "unknown");
        Ok(())
    }

    #[test]
    fn rejects_internal_entity_declarations() {
        let dat = br#"<!DOCTYPE datafile [<!ENTITY local "value">]><datafile><header><name>&local;</name></header></datafile>"#;
        assert!(matches!(
            DataFile::from_reader(dat.as_slice()),
            Err(crate::Error::XmlEntityNotAllowed)
        ));
    }

    #[test]
    fn detects_entities_in_bom_prefixed_utf16_and_parses_normal_documents()
    -> Result<(), Box<dyn std::error::Error>> {
        for big_endian in [false, true] {
            let entity = concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-16\"?>",
                "<!DOCTYPE datafile [<!ENTITY local \"value\">]>",
                "<datafile><header><name>&local;</name></header></datafile>"
            );
            let mut entity_bytes = utf16_bytes(entity, big_endian);
            entity_bytes.splice(
                0..0,
                if big_endian {
                    [0xFE, 0xFF]
                } else {
                    [0xFF, 0xFE]
                },
            );
            assert!(matches!(
                DataFile::from_reader(entity_bytes.as_slice()),
                Err(crate::Error::XmlEntityNotAllowed)
            ));

            let document = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-16\"?>{}",
                SIMPLE_DAT.replacen("<?xml version=\"1.0\"?>", "", 1)
            );
            let mut document_bytes = utf16_bytes(&document, big_endian);
            document_bytes.splice(
                0..0,
                if big_endian {
                    [0xFE, 0xFF]
                } else {
                    [0xFF, 0xFE]
                },
            );
            let parsed = DataFile::from_reader(document_bytes.as_slice())?;
            assert_eq!(parsed.header().name(), "Test Set");
        }
        Ok(())
    }

    #[test]
    fn detects_entities_with_utf16_xml_signature_without_bom() {
        for big_endian in [false, true] {
            let entity = concat!(
                "<?xml version=\"1.0\" encoding=\"UTF-16\"?>",
                "<!DOCTYPE datafile [<!ENTITY local \"value\">]>",
                "<datafile><header><name>&local;</name></header></datafile>"
            );
            let bytes = utf16_bytes(entity, big_endian);
            assert!(matches!(
                DataFile::from_reader(bytes.as_slice()),
                Err(crate::Error::XmlEntityNotAllowed)
            ));
        }
    }

    #[test]
    fn rejects_malformed_utf16_before_declaration_inspection_can_be_bypassed() {
        assert!(matches!(
            contains_entity_declaration(&[0xFF, 0xFE, b'<']),
            Err(())
        ));
        assert!(matches!(
            DataFile::from_reader([0xFF, 0xFE, b'<'].as_slice()),
            Err(crate::Error::XmlValidation(_))
        ));
    }

    #[test]
    fn malformed_repeated_doctype_prefixes_are_scanned_once_and_fail_xml_validation() {
        let prefix = b"<!DOCTYPE";
        let mut malformed = Vec::with_capacity(prefix.len() * 4096);
        for _ in 0..4096 {
            malformed.extend_from_slice(prefix);
        }
        let (_, end) = doctype_contains_entity_declaration(&malformed, prefix.len());
        assert_eq!(end, malformed.len());
        assert!(matches!(
            DataFile::from_reader(malformed.as_slice()),
            Err(crate::Error::XmlValidation(_))
        ));
    }

    fn utf16_bytes(value: &str, big_endian: bool) -> Vec<u8> {
        value
            .encode_utf16()
            .flat_map(|unit| {
                if big_endian {
                    unit.to_be_bytes()
                } else {
                    unit.to_le_bytes()
                }
            })
            .collect()
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

    #[test]
    fn from_path_accepts_valid_documents_larger_than_reader_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("large.dat");
        let mut file = std::fs::File::create(&path)?;
        file.write_all(b"<datafile><header><name>Large</name></header>")?;
        let padding = vec![b' '; 64 * 1024];
        let mut remaining = document_input::MAX_DOCUMENT_BYTES + 1;
        while remaining > 0 {
            let chunk_len = remaining.min(padding.len());
            file.write_all(b"<!--")?;
            file.write_all(&padding[..chunk_len])?;
            file.write_all(b"-->")?;
            remaining -= chunk_len;
        }
        file.write_all(b"</datafile>")?;
        drop(file);

        let path = Utf8Path::from_path(&path).ok_or("temporary path is not UTF-8")?;
        let data_file = DataFile::from_path(path)?;
        assert_eq!(data_file.header().name(), "Large");
        assert!(data_file.sha1().is_some());
        Ok(())
    }
}
