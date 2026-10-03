use std::io::Read;

use camino::Utf8Path;
use fmmap::MmapFileExt;
use quick_xml::events::Event;

use super::game::Game;
use super::header::{ClrMameProOptions, Header, RomCenterOptions};
use super::{
    AttributePosition, BiosSetAttribute, ClrMameProAttribute, DiskAttribute, DocumentAttribute,
    GameAttribute, NameAttribute, ReleaseAttribute, RomAttribute, RomCenterAttribute,
};

use crate::{
    document_input, hashes,
    xml_reader::{self, Element, NodeBudget},
};

#[derive(Debug)]
pub struct DataFile {
    file_name: Option<String>,
    build: Option<String>,
    debug: Option<String>, // bool
    header: Option<Header>,
    sha1: Option<Vec<u8>>,
    games: Vec<Game>,
    attribute_positions: Vec<AttributePosition<DocumentAttribute>>,
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
    pub device_ref_locations: Vec<Vec<RecordLocation>>,
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
        let (mut data_file, _) = Self::parse_bytes(raw)?;
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
        Self::parse_bytes(raw).map(|_| ())
    }

    fn parse_bytes(raw: &[u8]) -> crate::Result<(Self, XmlSourceMap)> {
        xml_reader::with_reader(raw, |reader, positions| {
            let mut budget = NodeBudget::default();
            let (root, empty) = read_datafile_root(reader, positions, &mut budget)?;
            let mut source_map = XmlSourceMap::default();
            collect_unsupported_attributes(&root, None, &mut source_map);
            let build = root.attributes.get("build").cloned();
            let debug = root.attributes.get("debug").cloned();
            if root
                .attributes
                .get("debug")
                .is_some_and(|value| !["yes", "no"].contains(&value.as_str()))
            {
                return Err(crate::Error::XmlValidation(
                    "invalid debug value on <datafile>".into(),
                ));
            }
            let mut header = None;
            let mut file_name = None;
            let mut sha1 = None;
            let mut games = Vec::new();
            if !empty {
                loop {
                    let (namespace, event) = xml_reader::next(reader, positions)?;
                    let node = match event {
                        Event::Start(start) => Some(xml_reader::read_element(
                            reader,
                            namespace,
                            &start,
                            &mut budget,
                            1,
                            positions,
                        )?),
                        Event::Empty(start) => Some(xml_reader::element_from_start(
                            reader,
                            namespace,
                            &start,
                            &mut budget,
                            1,
                            positions,
                        )?),
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(crate::Error::XmlValidation(
                                "unexpected end of input inside <datafile>".into(),
                            ));
                        }
                        _ => None,
                    };
                    let Some(node) = node else { continue };
                    match local_name(&node.name) {
                        "header" => {
                            let parsed_header = Header::from_xml(&node)?;
                            if header.replace(parsed_header).is_some() {
                                return Err(crate::Error::XmlValidation(
                                    "duplicate <header> in <datafile>".into(),
                                ));
                            }
                            collect_subtree_attributes(&node, None, &mut source_map);
                        }
                        "game" => {
                            let game_index = games.len();
                            source_map.game_locations.push(node.location);
                            source_map.rom_locations.push(Vec::new());
                            source_map.device_ref_locations.push(Vec::new());
                            collect_game_source_map(&node, game_index, &mut source_map);
                            games.push(Game::from_xml(&node)?);
                        }
                        "file_name" => set_once(&mut file_name, node.direct_text(), "file_name")?,
                        "sha1" => {
                            set_once(&mut sha1, parse_datafile_sha1(&node.direct_text())?, "sha1")?;
                        }
                        _ => {}
                    }
                }
            }
            finish_document(reader, positions)?;
            Ok((
                Self {
                    file_name,
                    build,
                    debug,
                    header,
                    sha1,
                    games,
                    attribute_positions: root
                        .attributes
                        .positions(DocumentAttribute::from_name)
                        .collect(),
                },
                source_map,
            ))
        })
    }

    /// Get a reference to the optional data file header.
    #[must_use]
    pub const fn header_opt(&self) -> Option<&Header> {
        self.header.as_ref()
    }

    /// Get a reference to the data file's header.
    pub fn header(&self) -> crate::Result<&Header> {
        self.header.as_ref().ok_or_else(|| {
            crate::Error::XmlValidation("Logiqx data file is missing its header".to_owned())
        })
    }

    #[must_use]
    pub const fn clrmamepro_options_opt(&self) -> Option<&ClrMameProOptions> {
        match &self.header {
            Some(header) => header.clrmamepro_options(),
            None => None,
        }
    }

    #[must_use]
    pub const fn romcenter_options_opt(&self) -> Option<&RomCenterOptions> {
        match &self.header {
            Some(header) => header.romcenter_options(),
            None => None,
        }
    }

    /// Return the DTD-effective debug value.
    #[must_use]
    pub fn debug_effective(&self) -> &str {
        self.debug.as_deref().unwrap_or("no")
    }

    #[must_use]
    pub const fn debug_was_explicit(&self) -> bool {
        self.debug.is_some()
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

    #[must_use]
    pub fn attribute_positions(&self) -> &[AttributePosition<DocumentAttribute>] {
        &self.attribute_positions
    }
}

fn read_datafile_root(
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
    budget: &mut NodeBudget,
) -> crate::Result<(Element, bool)> {
    let (namespace, event) = loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        if matches!(event, Event::Start(_) | Event::Empty(_)) {
            break (namespace, event);
        }
        if event == Event::Eof {
            return Err(crate::Error::XmlValidation("missing document root".into()));
        }
    };
    let (root, empty) = match event {
        Event::Start(start) => (
            xml_reader::element_from_start(reader, namespace, &start, budget, 0, positions)?,
            false,
        ),
        Event::Empty(start) => (
            xml_reader::element_from_start(reader, namespace, &start, budget, 0, positions)?,
            true,
        ),
        _ => unreachable!(),
    };
    if local_name(&root.name) != "datafile" {
        return Err(crate::Error::XmlValidation(format!(
            "expected <datafile>, found <{}>",
            root.name
        )));
    }
    Ok((root, empty))
}

fn parse_datafile_sha1(value: &str) -> crate::Result<Vec<u8>> {
    let digest = hex::decode(value.trim())
        .map_err(|error| crate::Error::XmlValidation(format!("invalid datafile SHA1: {error}")))?;
    if digest.len() != 20 {
        return Err(crate::Error::XmlValidation(
            "datafile SHA1 must be 20 bytes".into(),
        ));
    }
    Ok(digest)
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> crate::Result<()> {
    if slot.replace(value).is_some() {
        return Err(crate::Error::XmlValidation(format!(
            "duplicate <{name}> in <datafile>"
        )));
    }
    Ok(())
}

fn finish_document(
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
) -> crate::Result<()> {
    loop {
        match xml_reader::next(reader, positions)?.1 {
            Event::Eof => return Ok(()),
            Event::Text(text) if text.xml10_content().trim().is_empty() => {}
            Event::Comment(_) | Event::PI(_) => {}
            _ => {
                return Err(crate::Error::XmlValidation(
                    "content after the document root".into(),
                ));
            }
        }
    }
}

fn local_name(name: &str) -> &str {
    name.rsplit_once('}').map_or(name, |(_, local)| local)
}

fn collect_game_source_map(element: &Element, game_index: usize, source_map: &mut XmlSourceMap) {
    collect_unsupported_attributes(element, Some(game_index), source_map);
    for child in element.children() {
        match local_name(&child.name) {
            "rom" => source_map.rom_locations[game_index].push(child.location),
            "device_ref" => source_map.device_ref_locations[game_index].push(child.location),
            _ => {}
        }
        collect_subtree_attributes(child, Some(game_index), source_map);
    }
}

fn collect_subtree_attributes(
    element: &Element,
    in_game: Option<usize>,
    source_map: &mut XmlSourceMap,
) {
    collect_unsupported_attributes(element, in_game, source_map);
    for child in element.children() {
        collect_subtree_attributes(child, in_game, source_map);
    }
}

fn collect_unsupported_attributes(
    element: &Element,
    in_game: Option<usize>,
    source_map: &mut XmlSourceMap,
) {
    let element_name = local_name(&element.name);
    let record_name = element.attributes.get("name").cloned();
    for (attribute_name, value) in &element.attributes {
        let (namespace_uri, field_name) = attribute_name
            .strip_prefix('{')
            .and_then(|name| name.split_once('}'))
            .map_or((None, attribute_name.as_str()), |(namespace, local)| {
                (Some(namespace.to_owned()), local)
            });
        if namespace_uri.is_some() || !known_attribute(element_name, field_name) {
            let record_kind = match element_name {
                "game" => "game",
                "rom" => "rom",
                _ if in_game.is_some() => element_name,
                _ => "document",
            };
            source_map
                .unsupported_attributes
                .push(UnsupportedAttribute {
                    record_kind: record_kind.to_owned(),
                    record_name: record_name.clone(),
                    field_name: field_name.to_owned(),
                    namespace_uri,
                    value: value.clone(),
                    location: element.location,
                });
        }
    }
}

fn known_attribute(element: &str, attribute: &str) -> bool {
    match element {
        "datafile" => DocumentAttribute::from_name(attribute).is_some(),
        "clrmamepro" => ClrMameProAttribute::from_name(attribute).is_some(),
        "romcenter" => RomCenterAttribute::from_name(attribute).is_some(),
        "game" => GameAttribute::from_name(attribute).is_some(),
        "release" => ReleaseAttribute::from_name(attribute).is_some(),
        "biosset" => BiosSetAttribute::from_name(attribute).is_some(),
        "rom" => RomAttribute::from_name(attribute).is_some(),
        "disk" => DiskAttribute::from_name(attribute).is_some(),
        "sample" | "archive" | "device_ref" => NameAttribute::from_name(attribute).is_some(),
        _ => false,
    }
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
        assert_eq!(df.header()?.name(), "Test Set");
        assert_eq!(
            df.header()?.description().map(std::string::String::as_str),
            Some("Test Description")
        );
        assert_eq!(
            df.header()?.version().map(std::string::String::as_str),
            Some("1.0")
        );
        assert_eq!(
            df.header()?.author().map(std::string::String::as_str),
            Some("Tester")
        );
        assert_eq!(df.games().len(), 1);
        Ok(())
    }

    #[test]
    fn missing_header_is_a_contextual_validation_error() -> Result<(), Box<dyn std::error::Error>> {
        let data_file = DataFile::from_reader(b"<datafile/>".as_slice())?;

        assert!(data_file.header_opt().is_none());
        assert!(matches!(
            data_file.header(),
            Err(crate::Error::XmlValidation(message)) if message.contains("header")
        ));
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
            DataFile::from_reader(standard.as_slice())?.header()?.name(),
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
        assert_eq!(parsed.header()?.name(), "Test & stuff <!ENTITY literal>");
        Ok(())
    }

    #[test]
    fn parser_source_map_retains_unknown_attributes_and_record_positions()
    -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<datafile>
  <header><name>Source map</name></header>
  <game name="set" future="preserve">
    <device_ref name="sound"/>
    <rom name="asset.bin" size="4" future-hash="unknown"/>
  </game>
</datafile>"#;
        let (data_file, source_map) = DataFile::from_reader_with_source_map(xml.as_slice())?;
        assert_eq!(data_file.games().len(), 1);
        assert_eq!(source_map.game_locations[0].line, 3);
        assert_eq!(source_map.device_ref_locations[0][0].line, 4);
        assert_eq!(source_map.device_ref_locations[0][0].column, 5);
        assert_eq!(source_map.rom_locations[0][0].line, 5);
        assert_eq!(source_map.unsupported_attributes.len(), 2);
        assert_eq!(source_map.unsupported_attributes[0].field_name, "future");
        assert_eq!(source_map.unsupported_attributes[1].value, "unknown");
        Ok(())
    }

    #[test]
    fn parser_source_map_ignores_nested_games_and_roms() -> Result<(), Box<dyn std::error::Error>> {
        let xml = br#"<datafile>
  <header>
    <name>Source map</name>
    <extension>
      <game name="ignored">
        <rom name="ignored.bin" size="1"/>
      </game>
    </extension>
  </header>
  <game name="real">
    <rom name="real.bin" size="1"/>
  </game>
</datafile>"#;

        let (data_file, source_map) = DataFile::from_reader_with_source_map(xml.as_slice())?;

        assert_eq!(data_file.games().len(), 1);
        assert_eq!(data_file.games()[0].name(), "real");
        assert_eq!(source_map.game_locations.len(), 1);
        assert_eq!(source_map.game_locations[0].line, 10);
        assert_eq!(source_map.rom_locations.len(), 1);
        assert_eq!(source_map.rom_locations[0].len(), 1);
        assert_eq!(source_map.rom_locations[0][0].line, 11);
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
            assert_eq!(parsed.header()?.name(), "Test Set");
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
    fn rejects_malformed_utf16_during_streaming_decode() {
        assert!(matches!(
            DataFile::from_reader([0xFF, 0xFE, b'<'].as_slice()),
            Err(crate::Error::CatalogParse { message, excerpt: Some(excerpt), .. })
                if message.contains("malformed UTF-16")
                    && excerpt.bytes() == b"<"
                    && excerpt.start_byte() == 2
                    && excerpt.problem().is_some_and(|range| range.start() == 0 && range.end() == 1)
        ));
    }

    #[test]
    fn malformed_repeated_doctype_prefixes_fail_during_streaming_parse() {
        let prefix = b"<!DOCTYPE";
        let mut malformed = Vec::with_capacity(prefix.len() * 4096);
        for _ in 0..4096 {
            malformed.extend_from_slice(prefix);
        }
        assert!(DataFile::from_reader(malformed.as_slice()).is_err());
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
        assert_eq!(df.header()?.name(), "Minimal");
        assert!(df.header()?.description().is_none());
        assert!(df.header()?.version().is_none());
        assert!(df.header()?.author().is_none());
        assert!(df.header()?.homepage().is_none());
        assert!(df.header()?.url().is_none());
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
    fn malformed_supplied_attributes_are_retained_without_usable_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let dat = r#"<datafile><header><name>Uninterpreted</name></header>
<game name="g"><rom name="r" size="not-a-size" crc="xyz" md5="" sha1="not-hex"/></game></datafile>"#;
        let df = DataFile::from_reader(dat.as_bytes())?;
        let rom = &df.games()[0].roms()[0];

        assert_eq!(rom.size_text(), Some("not-a-size"));
        assert_eq!(rom.size(), None);
        assert_eq!(rom.crc_text(), Some("xyz"));
        assert_eq!(rom.crc(), None);
        assert_eq!(rom.md5_text(), Some(""));
        assert_eq!(rom.md5(), None);
        assert_eq!(rom.sha1_text(), Some("not-hex"));
        assert_eq!(rom.sha1(), None);
        Ok(())
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
    fn invalid_hex_in_dat_xml_is_retained_without_usable_sha1()
    -> Result<(), Box<dyn std::error::Error>> {
        let invalid = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Uninterpreted SHA1</name>
  </header>
  <game name="bad">
    <rom name="bad.rom" size="1" sha1="zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#;

        let data_file = DataFile::from_reader(invalid.as_bytes())?;
        let rom = &data_file.games()[0].roms()[0];
        assert_eq!(
            rom.sha1_text(),
            Some("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
        );
        assert_eq!(rom.sha1(), None);
        assert_eq!(
            rom.md5(),
            Some(&hex::decode("900150983cd24fb0d6963f7d28e17f72")?[..])
        );
        assert_eq!(rom.crc(), Some(&[0x12, 0x34, 0x56, 0x78][..]));
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
                df.header()?.name(),
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
        assert_eq!(data_file.header()?.name(), "Large");
        assert!(data_file.sha1().is_some());
        Ok(())
    }
}
