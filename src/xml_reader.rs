//! Shared bounded XML decoding and borrowing event-reader setup.

use std::{borrow::Cow, collections::BTreeMap};

use quick_xml::{
    events::{BytesRef, BytesStart, Event},
    name::ResolveResult,
    reader::NsReader,
};

use crate::{Result, document_input, error::Error, logiqx::RecordLocation};

pub const MAX_XML_DEPTH: usize = 256;
pub const MAX_XML_NODES: usize = 200_000;
pub const MAX_MAME_XML_NODES: usize = 6_000_000;
const MAX_NAME_BYTES: usize = 4096;
const MAX_ATTRIBUTES: usize = 1024;
const MAX_ATTRIBUTE_BYTES: usize = 1024 * 1024;
const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Element {
    // Match the canonical key order previously produced through serde_json::Value.
    pub attributes: BTreeMap<String, String>,
    pub content: Vec<ElementContent>,
    pub name: String,
    #[serde(skip)]
    pub location: RecordLocation,
}

impl Element {
    pub fn children(&self) -> impl Iterator<Item = &Self> {
        self.content.iter().filter_map(|content| match content {
            ElementContent::Element(child) => Some(child),
            ElementContent::Text(_) => None,
        })
    }

    #[must_use]
    pub fn direct_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|content| match content {
                ElementContent::Text(text) => Some(text.as_str()),
                ElementContent::Element(_) => None,
            })
            .collect()
    }

    pub(crate) fn child_text(&self, name: &str) -> Result<Option<String>> {
        let mut matches = self.children().filter(|child| child.name == name);
        let Some(child) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Err(Error::XmlValidation(format!(
                "duplicate <{name}> field inside <{}>",
                self.name
            )));
        }
        Ok(Some(child.direct_text().trim().to_owned()))
    }

    pub(crate) fn required_attribute(&self, name: &str) -> Result<String> {
        self.attributes.get(name).cloned().ok_or_else(|| {
            Error::XmlValidation(format!(
                "missing required {name:?} attribute on <{}>",
                self.name
            ))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ElementContent {
    Text(String),
    Element(Element),
}

pub struct NodeBudget {
    count: usize,
    limit: usize,
}

impl Default for NodeBudget {
    fn default() -> Self {
        Self::with_limit(MAX_XML_NODES)
    }
}

impl NodeBudget {
    pub(crate) const fn with_limit(limit: usize) -> Self {
        Self { count: 0, limit }
    }

    fn include(&mut self, depth: usize) -> Result<()> {
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::XmlValidation("XML element count overflow".into()))?;
        if self.count > self.limit {
            return Err(Error::XmlValidation(format!(
                "XML element count exceeds {} nodes",
                self.limit
            )));
        }
        if depth > MAX_XML_DEPTH {
            return Err(Error::XmlValidation(format!(
                "XML element nesting exceeds {MAX_XML_DEPTH} levels"
            )));
        }
        Ok(())
    }
}

pub struct PositionMap<'a> {
    bytes: &'a [u8],
    cursor: usize,
    line: i64,
    column: i64,
}

impl<'a> PositionMap<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            cursor: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn at(&mut self, offset: u64) -> (i64, i64) {
        let end = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(self.bytes.len());
        if end < self.cursor {
            return (self.line, self.column);
        }
        let Ok(text) = std::str::from_utf8(&self.bytes[self.cursor..end]) else {
            return (self.line, self.column);
        };
        for character in text.chars() {
            if character == '\n' {
                self.line = self.line.saturating_add(1);
                self.column = 1;
            } else {
                self.column = self.column.saturating_add(1);
            }
        }
        self.cursor = end;
        (self.line, self.column)
    }
}

/// Run a format adapter against a namespace-aware reader whose events borrow
/// from the decoded document. This is the only XML traversal for an import.
pub fn with_reader<T, E: From<Error>>(
    bytes: &[u8],
    parse: impl FnOnce(&mut NsReader<&[u8]>, &mut PositionMap<'_>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    let xml = decode(bytes)?;
    validate_xml10_characters(&xml)?;
    let mut positions = PositionMap::new(xml.as_bytes());
    let mut reader = NsReader::from_reader(xml.as_bytes());
    reader.config_mut().check_comments = true;
    parse(&mut reader, &mut positions)
}

fn validate_xml10_characters(xml: &str) -> Result<()> {
    let mut line = 1_i64;
    let mut column = 1_i64;
    let mut previous_was_carriage_return = false;

    for character in xml.chars() {
        let codepoint = u32::from(character);
        let valid = matches!(
            codepoint,
            0x09 | 0x0a | 0x0d | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x0010_ffff
        );
        if !valid {
            return Err(Error::CatalogParse {
                message: format!("XML 1.0 forbids U+{codepoint:04X}"),
                record_kind: Some("document".into()),
                record_name: None,
                line: Some(line),
                column: Some(column),
            });
        }

        match character {
            '\r' => {
                line = line.saturating_add(1);
                column = 1;
                previous_was_carriage_return = true;
            }
            '\n' => {
                if !previous_was_carriage_return {
                    line = line.saturating_add(1);
                }
                column = 1;
                previous_was_carriage_return = false;
            }
            _ => {
                column = column.saturating_add(1);
                previous_was_carriage_return = false;
            }
        }
    }
    Ok(())
}

/// Decode gzip and UTF-16 inputs while borrowing ordinary UTF-8 documents.
pub fn decode(bytes: &[u8]) -> Result<Cow<'_, str>> {
    match document_input::decode_xml(bytes)? {
        Cow::Borrowed(bytes) => decode_text(bytes),
        Cow::Owned(bytes) => decode_text(&bytes).map(Cow::into_owned).map(Cow::Owned),
    }
}

fn decode_text(bytes: &[u8]) -> Result<Cow<'_, str>> {
    if let Some((little_endian, skip)) = utf16_encoding(bytes) {
        return decode_utf16(bytes, little_endian, skip).map(Cow::Owned);
    }
    std::str::from_utf8(bytes)
        .map(Cow::Borrowed)
        .map_err(|error| Error::XmlValidation(format!("XML is not valid UTF-8: {error}")))
}

const fn utf16_encoding(bytes: &[u8]) -> Option<(bool, usize)> {
    match bytes {
        [0xff, 0xfe, ..] => Some((true, 2)),
        [0xfe, 0xff, ..] => Some((false, 2)),
        [0x3c, 0x00, _, 0x00, ..] => Some((true, 0)),
        [0x00, 0x3c, 0x00, _, ..] => Some((false, 0)),
        _ => None,
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool, skip: usize) -> Result<String> {
    let remaining = bytes
        .get(skip..)
        .filter(|remaining| remaining.len() % 2 == 0)
        .ok_or_else(|| Error::XmlValidation("malformed UTF-16 encoding".into()))?;
    let units = remaining.as_chunks::<2>().0.iter().map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let mut output = String::with_capacity(bytes.len().saturating_sub(skip));
    for scalar in char::decode_utf16(units) {
        output.push(scalar.map_err(|_| Error::XmlValidation("malformed UTF-16 encoding".into()))?);
        if output.len() > document_input::MAX_DECOMPRESSED_DOCUMENT_BYTES {
            return Err(Error::DocumentTooLarge {
                limit: document_input::MAX_DECOMPRESSED_DOCUMENT_BYTES,
            });
        }
    }
    Ok(output)
}

/// Read an event while enforcing the established no-custom-entities policy.
pub fn next<'a>(
    reader: &mut NsReader<&'a [u8]>,
    positions: &mut PositionMap<'_>,
) -> Result<(Option<String>, Event<'a>)> {
    let (namespace, event) = match reader.read_resolved_event() {
        Ok(resolved) => resolved,
        Err(error) => {
            let offset = reader.error_position();
            let (line, column) = positions.at(offset);
            return Err(Error::CatalogParse {
                message: error.to_string(),
                record_kind: Some("document".into()),
                record_name: None,
                line: Some(line),
                column: Some(column),
            });
        }
    };
    match &event {
        Event::DocType(declaration) if declaration.as_ref().contains("<!ENTITY") => {
            return Err(Error::XmlEntityNotAllowed);
        }
        Event::GeneralRef(reference) if !allowed_reference(reference) => {
            return Err(Error::XmlEntityNotAllowed);
        }
        Event::Start(start) | Event::Empty(start) => validate_start(start)?,
        Event::Text(text) if text.as_ref().len() > MAX_TEXT_BYTES => {
            return Err(Error::XmlValidation(format!(
                "XML text exceeds {MAX_TEXT_BYTES} bytes"
            )));
        }
        Event::CData(text) if text.as_ref().len() > MAX_TEXT_BYTES => {
            return Err(Error::XmlValidation(format!(
                "XML text exceeds {MAX_TEXT_BYTES} bytes"
            )));
        }
        _ => {}
    }
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.as_ref().to_owned()),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(prefix) => {
            return Err(Error::XmlValidation(format!(
                "unbound XML namespace prefix {prefix:?}"
            )));
        }
    };
    Ok((namespace, event))
}

fn validate_start(start: &BytesStart<'_>) -> Result<()> {
    if start.name().as_ref().len() > MAX_NAME_BYTES {
        return Err(Error::XmlValidation(format!(
            "XML element name exceeds {MAX_NAME_BYTES} bytes"
        )));
    }
    let mut count = 0;
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
        count += 1;
        if count > MAX_ATTRIBUTES {
            return Err(Error::XmlValidation(format!(
                "XML element exceeds {MAX_ATTRIBUTES} attributes"
            )));
        }
        if attribute.key.as_ref().len() > MAX_NAME_BYTES {
            return Err(Error::XmlValidation(format!(
                "XML attribute name exceeds {MAX_NAME_BYTES} bytes"
            )));
        }
        if attribute.value.len() > MAX_ATTRIBUTE_BYTES {
            return Err(Error::XmlValidation(format!(
                "XML attribute value exceeds {MAX_ATTRIBUTE_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

/// Convert the current start/empty event into the extension tree.
pub fn element_from_start(
    reader: &NsReader<&[u8]>,
    namespace: Option<String>,
    start: &BytesStart<'_>,
    budget: &mut NodeBudget,
    depth: usize,
    positions: &mut PositionMap<'_>,
) -> Result<Element> {
    budget.include(depth)?;
    let name = expanded_name(namespace, start.local_name().as_ref());
    let mut attributes = BTreeMap::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
        let raw_name = attribute.key.as_ref();
        if raw_name == "xmlns" || raw_name.starts_with("xmlns:") {
            continue;
        }
        let (namespace, local) = reader.resolver().resolve_attribute(attribute.key);
        let namespace = match namespace {
            ResolveResult::Bound(namespace) => Some(namespace.as_ref().to_owned()),
            ResolveResult::Unbound => None,
            ResolveResult::Unknown(prefix) => {
                return Err(Error::XmlValidation(format!(
                    "unbound XML namespace prefix {prefix:?}"
                )));
            }
        };
        let local = local.as_ref().to_owned();
        let name = expanded_name(namespace, &local);
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|error| Error::XmlValidation(error.to_string()))?
            .into_owned();
        if attributes.insert(name.clone(), value).is_some() {
            return Err(Error::XmlValidation(format!(
                "duplicate XML attribute {name:?}"
            )));
        }
    }
    let start_offset = (start.as_ref().as_ptr() as usize)
        .saturating_sub(positions.bytes.as_ptr() as usize)
        .saturating_sub(1);
    let (line, column) = positions.at(u64::try_from(start_offset).unwrap_or(u64::MAX));
    Ok(Element {
        name,
        attributes,
        content: Vec::new(),
        location: RecordLocation { line, column },
    })
}

/// Consume one record subtree so adapters need not build a whole-document DOM.
pub fn read_element(
    reader: &mut NsReader<&[u8]>,
    namespace: Option<String>,
    start: &BytesStart<'_>,
    budget: &mut NodeBudget,
    depth: usize,
    positions: &mut PositionMap<'_>,
) -> Result<Element> {
    let mut element = element_from_start(reader, namespace, start, budget, depth, positions)?;
    loop {
        let (namespace, event) = next(reader, positions)?;
        match event {
            Event::Start(child) => element.content.push(ElementContent::Element(read_element(
                reader,
                namespace,
                &child,
                budget,
                depth.saturating_add(1),
                positions,
            )?)),
            Event::Empty(child) => {
                element
                    .content
                    .push(ElementContent::Element(element_from_start(
                        reader,
                        namespace,
                        &child,
                        budget,
                        depth.saturating_add(1),
                        positions,
                    )?));
            }
            Event::Text(text) => {
                let text = text.xml10_content();
                if !text.is_empty() {
                    element
                        .content
                        .push(ElementContent::Text(text.into_owned()));
                }
            }
            Event::CData(text) => element
                .content
                .push(ElementContent::Text(text.as_ref().to_owned())),
            Event::GeneralRef(reference) => {
                let text = match reference
                    .resolve_char_ref()
                    .map_err(|error| Error::XmlValidation(error.to_string()))?
                {
                    Some(character) => character.to_string(),
                    None => quick_xml::escape::resolve_predefined_entity(reference.as_ref())
                        .ok_or(Error::XmlEntityNotAllowed)?
                        .to_owned(),
                };
                element.content.push(ElementContent::Text(text));
            }
            Event::End(_) => return Ok(element),
            Event::Eof => {
                let record_kind = local_name(&element.name).to_owned();
                return Err(Error::CatalogParse {
                    message: format!("unexpected end of stream inside <{record_kind}>"),
                    record_name: element.attributes.get("name").cloned(),
                    record_kind: Some(record_kind),
                    line: Some(element.location.line),
                    column: Some(element.location.column),
                });
            }
            _ => {}
        }
    }
}

fn expanded_name(namespace: Option<String>, local: &str) -> String {
    namespace.map_or_else(
        || local.to_owned(),
        |namespace| format!("{{{namespace}}}{local}"),
    )
}

fn local_name(name: &str) -> &str {
    name.rsplit_once('}').map_or(name, |(_, local)| local)
}

fn allowed_reference(reference: &BytesRef<'_>) -> bool {
    reference.resolve_char_ref().ok().flatten().is_some()
        || matches!(reference.as_ref(), "amp" | "lt" | "gt" | "apos" | "quot")
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;

    use super::*;

    #[test]
    fn node_budget_uses_its_format_specific_limit() {
        let mut budget = NodeBudget::with_limit(1);
        assert!(budget.include(1).is_ok());
        assert!(matches!(
            budget.include(1),
            Err(Error::XmlValidation(message)) if message.contains("exceeds 1 nodes")
        ));
    }

    #[test]
    fn reader_borrows_utf8_events_resolves_namespaces_and_unescapes_attributes() -> Result<()> {
        let input = b"<root xmlns='urn:test'>\n  <item name='a&amp;b'/></root>";
        let (name, namespace, position, attribute) = with_reader(input, |reader, positions| {
            let (namespace, event) = next(reader, positions)?;
            let Event::Start(root) = event else {
                return Err(Error::XmlValidation("expected root".into()));
            };
            let name = root.local_name().as_ref().to_owned();
            let namespace = namespace.unwrap_or_default();
            let item = loop {
                match next(reader, positions)?.1 {
                    Event::Empty(item) => break item,
                    Event::Text(text) if text.xml10_content().trim().is_empty() => {}
                    _ => return Err(Error::XmlValidation("expected item".into())),
                }
            };
            let attribute = item
                .attributes()
                .next()
                .ok_or_else(|| Error::XmlValidation("missing attribute".into()))?
                .map_err(|error| Error::XmlValidation(error.to_string()))?
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| Error::XmlValidation(error.to_string()))?
                .into_owned();
            Ok((
                name,
                namespace,
                positions.at(reader.buffer_position()),
                attribute,
            ))
        })?;
        assert_eq!(name, "root");
        assert_eq!(namespace, "urn:test");
        assert_eq!(position, (2, 25));
        assert_eq!(attribute, "a&b");
        Ok(())
    }

    #[test]
    fn reader_decodes_utf16_and_streams_text() -> Result<()> {
        let text = "<?xml version='1.0' encoding='UTF-16'?><root>café</root>";
        let mut input = vec![0xff, 0xfe];
        input.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let content = with_reader(&input, |reader, positions| {
            let mut content = String::new();
            loop {
                match next(reader, positions)?.1 {
                    Event::Text(text) => content.push_str(&text.xml10_content()),
                    Event::Eof => break,
                    _ => {}
                }
            }
            Ok::<_, Error>(content)
        })?;
        assert_eq!(content, "café");
        Ok(())
    }

    #[test]
    fn reader_rejects_entity_declarations_and_custom_references() {
        assert!(matches!(
            with_reader(
                b"<!DOCTYPE root [<!ENTITY x 'bad'>]><root>&x;</root>",
                |reader, positions| {
                    while next(reader, positions)?.1 != Event::Eof {}
                    Ok(())
                }
            ),
            Err(Error::XmlEntityNotAllowed)
        ));
        assert!(with_reader(&[0xff, 0xfe, 0x3c], |_, _| Ok::<_, Error>(())).is_err());
    }

    #[test]
    fn reader_rejects_xml_10_forbidden_characters_in_text_and_attributes() {
        for xml in [
            "<root>\u{0000}</root>",
            "<root>\u{0001}</root>",
            "<root value='\u{000b}'/>",
            "<root>\u{fffe}</root>",
            "<root>\u{ffff}</root>",
        ] {
            let result = with_reader(xml.as_bytes(), |reader, positions| {
                while next(reader, positions)?.1 != Event::Eof {}
                Ok::<_, Error>(())
            });
            assert!(
                result.is_err(),
                "accepted forbidden XML character in {xml:?}"
            );
        }
    }

    #[test]
    fn invalid_character_diagnostic_uses_decoded_source_line_and_column() {
        let xml = "<root>\r\n \u{0000}</root>";
        let error = with_reader(xml.as_bytes(), |_, _| Ok::<_, Error>(()));
        assert!(matches!(
            error,
            Err(Error::CatalogParse {
                message,
                line: Some(2),
                column: Some(2),
                ..
            }) if message.contains("U+0000")
        ));

        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        let error = with_reader(&utf16, |_, _| Ok::<_, Error>(()));
        assert!(matches!(
            error,
            Err(Error::CatalogParse {
                line: Some(2),
                column: Some(2),
                ..
            })
        ));
    }
}
