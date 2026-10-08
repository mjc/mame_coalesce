//! Shared bounded XML decoding and borrowing event-reader setup.

use std::{borrow::Cow, collections::HashSet};

use quick_xml::{
    events::{BytesDecl, BytesRef, BytesStart, Event, attributes::Attribute},
    name::{Namespace, NamespaceResolver, QName, ResolveResult},
    reader::Reader,
};

use crate::{
    Result,
    diagnostics::{ByteRange, ExcerptView, SourceExcerpt},
    document_input,
    error::Error,
    logiqx::RecordLocation,
};

pub const MAX_XML_DEPTH: usize = 256;
pub const MAX_XML_NODES: usize = 200_000;
pub const MAX_MAME_XML_NODES: usize = 6_000_000;
const MAX_NAME_BYTES: usize = 4096;
const MAX_ATTRIBUTES: usize = 1024;
const MAX_ATTRIBUTE_BYTES: usize = 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

const fn utf8_bom_length(bytes: &[u8]) -> usize {
    if matches!(bytes, [0xef, 0xbb, 0xbf, ..]) {
        3
    } else {
        0
    }
}

mod attributes;
pub use attributes::XmlAttributes;
pub(crate) use attributes::attribute_fields;

/// The first character of an attribute `QName`, never its owner's tag position.
/// Columns count Unicode scalars; tabs count as one column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributeLocation {
    pub line: i64,
    pub column: i64,
}

/// Position of a recognized XML attribute, without another owned value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttributePosition<Field> {
    pub field: Field,
    pub source_order: usize,
    pub location: AttributeLocation,
}

/// A present scalar, including explicit empty text and its source position.
///
/// Child fields use the zero-based ordinal among all direct child elements of
/// their owner (including vendor elements, excluding comments/text). Attribute
/// fields use the zero-based lexical attribute ordinal of the opening tag,
/// including vendor attributes and namespace declarations. These are separate
/// domains; their position tables must not share an ordinal uniqueness key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredText {
    pub value: String,
    pub source_order: usize,
    pub location: RecordLocation,
}

impl DeclaredText {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.value.as_str()
    }
}

/// Borrowing event reader with XML-normalized namespace bindings. The upstream
/// namespace reader validates reserved bindings before decoding references.
pub struct XmlReader<'a> {
    reader: Reader<&'a [u8]>,
    namespaces: NamespaceResolver,
    pending_pop: bool,
    last_event_span: Option<(u64, u64)>,
}

impl<'a> XmlReader<'a> {
    fn new(input: &'a [u8]) -> Self {
        let mut reader = Reader::from_reader(input);
        reader.config_mut().check_comments = true;
        Self {
            reader,
            namespaces: NamespaceResolver::default(),
            pending_pop: false,
            last_event_span: None,
        }
    }

    pub const fn buffer_position(&self) -> u64 {
        self.reader.buffer_position()
    }

    pub const fn resolver(&self) -> &NamespaceResolver {
        &self.namespaces
    }

    /// The most recently consumed event's half-open range in the parse buffer.
    #[must_use]
    pub const fn last_event_span(&self) -> Option<(u64, u64)> {
        self.last_event_span
    }

    fn read_event(&mut self) -> Result<Event<'a>> {
        if self.pending_pop {
            self.namespaces.pop();
            self.pending_pop = false;
        }
        let start = self.reader.buffer_position();
        let event = self
            .reader
            .read_event()
            .map_err(|error| Error::XmlValidation(error.to_string()))?;
        self.last_event_span = Some((start, self.reader.buffer_position()));
        match &event {
            Event::Start(start) | Event::Empty(start) => {
                self.push_namespaces(start)?;
                self.pending_pop = matches!(event, Event::Empty(_));
            }
            Event::End(_) => self.pending_pop = true,
            _ => {}
        }
        Ok(event)
    }

    fn push_namespaces(&mut self, start: &BytesStart<'_>) -> Result<()> {
        // Push only the scope; add declarations after normalization, retaining
        // the original borrowed tag and lexical attribute positions.
        self.namespaces
            .push(&BytesStart::new(""))
            .map_err(|error| Error::XmlValidation(error.to_string()))?;
        for (index, attribute) in start.attributes().enumerate() {
            let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
            if index >= MAX_ATTRIBUTES || attribute.value.len() > MAX_ATTRIBUTE_BYTES {
                return Err(Error::XmlValidation("XML attribute limit exceeded".into()));
            }
            if let Some(prefix) = attribute.key.as_namespace_binding() {
                let namespace = normalize_namespace(&attribute.value)?;
                validate_namespace_binding(attribute.key.as_ref(), &namespace)?;
                self.namespaces
                    .add(prefix, Namespace(&namespace))
                    .map_err(|error| Error::XmlValidation(error.to_string()))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Element {
    // Match the canonical key order previously produced through serde_json::Value.
    pub attributes: XmlAttributes,
    pub content: Vec<ElementContent>,
    pub name: String,
    #[serde(skip)]
    pub(crate) content_kind: ElementContentKind,
    #[serde(skip)]
    pub(crate) has_namespace_declarations: bool,
    #[serde(skip)]
    pub location: RecordLocation,
}

/// Exact half-open byte interval in the UTF-8 transport-decoded XML view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceExtent {
    pub start: usize,
    pub end: usize,
}

/// Coarse XML content classification retained without saving markup strings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ElementContentKind {
    #[default]
    Empty,
    ElementOnly,
    CharacterData,
}

impl ElementContentKind {
    fn include(&mut self, kind: Self) {
        *self = (*self).max(kind);
    }
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

    pub(crate) fn child_text_preserved(&self, name: &str) -> Result<Option<String>> {
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
        Ok(Some(child.direct_text()))
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

    pub(crate) fn include(&mut self, depth: usize) -> Result<()> {
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
    position: SourcePosition,
    source_byte_view: Option<SourceByteView>,
}

/// Identifies the complete byte sequence against which source extents are measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceByteView {
    /// The retained input bytes are the XML byte sequence.
    RetainedOriginal { byte_length: usize },
    /// A transport wrapper was decoded; the XML byte sequence is this long.
    TransportDecodedXml { byte_length: usize },
}

/// Provenance of a leading U+FEFF in the decoded reader buffer. quick-xml
/// removes that prefix in either case; only a transport BOM is column-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BomOrigin {
    Transport,
    DecodedScalar,
}

impl BomOrigin {
    fn content(self, text: &str) -> &str {
        match self {
            Self::Transport => text.strip_prefix('\u{feff}').unwrap_or(text),
            Self::DecodedScalar => text,
        }
    }
}

/// Original decoded-document coordinates, following XML's line-ending rules.
pub struct SourcePosition {
    location: RecordLocation,
    previous_was_carriage_return: bool,
}

impl Default for SourcePosition {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePosition {
    pub const fn new() -> Self {
        Self {
            location: RecordLocation { line: 1, column: 1 },
            previous_was_carriage_return: false,
        }
    }

    pub const fn location(&self) -> RecordLocation {
        self.location
    }

    pub const fn advance(&mut self, character: char) {
        match character {
            '\r' => {
                self.location.line = self.location.line.saturating_add(1);
                self.location.column = 1;
                self.previous_was_carriage_return = true;
            }
            '\n' => {
                if !self.previous_was_carriage_return {
                    self.location.line = self.location.line.saturating_add(1);
                }
                self.location.column = 1;
                self.previous_was_carriage_return = false;
            }
            _ => {
                self.location.column = self.location.column.saturating_add(1);
                self.previous_was_carriage_return = false;
            }
        }
    }
}

impl<'a> PositionMap<'a> {
    /// Locate a borrowed opening tag without allocating an extension tree.
    pub(crate) fn start_location(&mut self, start: &BytesStart<'_>) -> Result<RecordLocation> {
        let offset = self
            .checked_slice_offset(start.as_ref().as_bytes())?
            .checked_sub(1)
            .ok_or_else(|| Error::XmlValidation("XML opening tag has no source prefix".into()))?;
        self.source_location(offset)
    }

    /// Locate an attribute at the first byte of its qualified name.
    pub(crate) fn attribute_location(
        &mut self,
        attribute: &Attribute<'_>,
    ) -> Result<RecordLocation> {
        let name = attribute.key.as_ref();
        let offset = self.checked_slice_offset(name.as_bytes())?;
        self.source_location(offset)
    }

    fn checked_slice_offset(&self, slice: &[u8]) -> Result<usize> {
        let base = self.bytes.as_ptr() as usize;
        let start = (slice.as_ptr() as usize).checked_sub(base).ok_or_else(|| {
            Error::XmlValidation("XML borrowed source is outside the input".into())
        })?;
        let end = start
            .checked_add(slice.len())
            .ok_or_else(|| Error::XmlValidation("XML borrowed source offset overflow".into()))?;
        if slice.is_empty() || end > self.bytes.len() {
            return Err(Error::XmlValidation(
                "XML borrowed source is outside the input".into(),
            ));
        }
        Ok(start)
    }

    /// Locate a boundary in the retained parse buffer, including its BOM bytes.
    pub(crate) fn source_location(&mut self, offset: usize) -> Result<RecordLocation> {
        if offset < self.cursor {
            return Err(Error::XmlValidation(
                "XML source location precedes the current source position".into(),
            ));
        }
        let text = self
            .bytes
            .get(self.cursor..offset)
            .ok_or_else(|| Error::XmlValidation("XML source offset is outside the input".into()))?;
        let text = std::str::from_utf8(text).map_err(|error| {
            Error::XmlValidation(format!("invalid UTF-8 source position: {error}"))
        })?;
        for character in text.chars() {
            self.position.advance(character);
        }
        self.cursor = offset;
        Ok(self.position.location())
    }

    #[cfg(test)]
    const fn new(bytes: &'a [u8], bom_origin: BomOrigin) -> Self {
        Self::with_source_byte_mapping(
            bytes,
            bom_origin,
            Some(SourceByteView::RetainedOriginal {
                byte_length: bytes.len(),
            }),
        )
    }

    const fn with_source_byte_mapping(
        bytes: &'a [u8],
        bom_origin: BomOrigin,
        source_byte_view: Option<SourceByteView>,
    ) -> Self {
        let prefix_length = utf8_bom_length(bytes);
        let mut position = SourcePosition::new();
        if prefix_length != 0 && matches!(bom_origin, BomOrigin::DecodedScalar) {
            position.advance('\u{feff}');
        }
        Self {
            bytes,
            // Borrowed slices still address the original parse buffer. Match
            // the reader-skipped byte prefix without erasing a decoded scalar.
            cursor: prefix_length,
            position,
            source_byte_view,
        }
    }

    /// Describe the complete mapped byte view, if event offsets refer to it.
    #[must_use]
    pub const fn source_byte_view(&self) -> Option<SourceByteView> {
        self.source_byte_view
    }

    /// Translate quick-xml's BOM-excluding offset to the retained parse buffer.
    fn event_source_offset(&self, offset: u64) -> Result<usize> {
        usize::try_from(offset)
            .ok()
            .and_then(|offset| offset.checked_add(utf8_bom_length(self.bytes)))
            .filter(|&offset| offset <= self.bytes.len())
            .ok_or_else(|| Error::XmlValidation("invalid XML event source offset".into()))
    }

    /// Map an event boundary to transport-decoded UTF-8 bytes when available.
    pub(crate) fn event_extent_start(&self, offset: u64) -> Result<Option<usize>> {
        self.source_byte_view
            .is_some()
            .then(|| self.event_source_offset(offset))
            .transpose()
    }

    /// Locate a quick-xml event boundary; borrowed-slice offsets use `source_location`.
    pub(crate) fn event_location(&mut self, offset: u64) -> Result<RecordLocation> {
        self.source_location(self.event_source_offset(offset)?)
    }

    /// Map reader event boundaries to source bytes when parsing did not
    /// transcode the XML encoding.
    pub(crate) fn event_extent(&self, start: u64, end: u64) -> Result<Option<SourceExtent>> {
        if self.source_byte_view.is_none() {
            return Ok(None);
        }
        let start = self.event_source_offset(start)?;
        let end = self.event_source_offset(end)?;
        if end <= start {
            return Err(Error::XmlValidation(
                "XML event extent is empty or reversed".into(),
            ));
        }
        Ok(Some(SourceExtent { start, end }))
    }
}

/// Run a format adapter against a namespace-aware reader whose events borrow
/// from the decoded document. This is the only XML traversal for an import.
pub fn with_reader<T, E: From<Error>>(
    bytes: &[u8],
    parse: impl FnOnce(&mut XmlReader<'_>, &mut PositionMap<'_>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    let source = document_input::decode_xml(bytes)?;
    let view = input_view(bytes);
    let source_bytes_mappable = utf16_encoding(&source).is_none();
    let source_byte_view = source_bytes_mappable.then(|| match view {
        ExcerptView::RetainedOriginalBytes => SourceByteView::RetainedOriginal {
            byte_length: source.len(),
        },
        ExcerptView::TransportDecodedXmlBytes => SourceByteView::TransportDecodedXml {
            byte_length: source.len(),
        },
    });
    let xml = decode_text(&source, view)?;
    let bom_origin = if utf16_encoding(&source).is_some() {
        BomOrigin::DecodedScalar
    } else {
        BomOrigin::Transport
    };
    validate_xml10_characters(&xml, Some((&source, view)), bom_origin)?;
    parse_decoded(&xml, bom_origin, source_byte_view, parse)
}

/// Read text decoded once by an adapter with a format-specific recovery policy.
pub fn with_decoded_reader<T, E: From<Error>>(
    xml: &str,
    bom_origin: BomOrigin,
    parse: impl FnOnce(&mut XmlReader<'_>, &mut PositionMap<'_>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    validate_xml10_characters(xml, None, bom_origin)?;
    parse_decoded(xml, bom_origin, None, parse)
}

fn parse_decoded<T, E: From<Error>>(
    xml: &str,
    bom_origin: BomOrigin,
    source_byte_view: Option<SourceByteView>,
    parse: impl FnOnce(&mut XmlReader<'_>, &mut PositionMap<'_>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    let mut positions =
        PositionMap::with_source_byte_mapping(xml.as_bytes(), bom_origin, source_byte_view);
    let mut reader = XmlReader::new(xml.as_bytes());
    parse(&mut reader, &mut positions)
}

fn validate_xml10_characters(
    xml: &str,
    source: Option<(&[u8], ExcerptView)>,
    bom_origin: BomOrigin,
) -> Result<()> {
    let mut position = SourcePosition::new();
    let utf16 = source.and_then(|(bytes, _)| utf16_encoding(bytes));
    let mut source_offset = utf16.map_or_else(|| utf8_bom_length(xml.as_bytes()), |(_, skip)| skip);
    let xml = bom_origin.content(xml);

    for character in xml.chars() {
        let codepoint = u32::from(character);
        if !is_xml10_character(character) {
            let location = position.location();
            return Err(Error::CatalogParse {
                message: format!("XML 1.0 forbids U+{codepoint:04X}"),
                record_kind: Some("document".into()),
                record_name: None,
                line: Some(location.line),
                column: Some(location.column),
                excerpt: source.and_then(|(bytes, view)| {
                    character_excerpt(
                        bytes,
                        view,
                        source_offset,
                        encoded_width(character, utf16.is_some()),
                    )
                }),
                coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
            });
        }

        position.advance(character);
        source_offset += encoded_width(character, utf16.is_some());
    }
    Ok(())
}

/// Decoded text plus the byte source needed by format-specific recovery.
/// UTF-8 bytes are the text itself; UTF-16 keeps its encoded input separately.
pub enum DecodedRecoveryInput<'a> {
    Utf8 {
        text: Cow<'a, str>,
        view: ExcerptView,
    },
    Utf16 {
        text: String,
        encoded: Cow<'a, [u8]>,
        view: ExcerptView,
        skip: usize,
    },
}

impl<'a> DecodedRecoveryInput<'a> {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Utf8 { text, .. } => text,
            Self::Utf16 { text, .. } => text,
        }
    }

    /// Decoded characters excluding only a UTF-8 transport BOM. UTF-16 decoding
    /// already removed the encoded transport BOM.
    pub(crate) fn content(&self) -> &str {
        self.bom_origin().content(self.text())
    }

    pub(crate) const fn bom_origin(&self) -> BomOrigin {
        match self {
            Self::Utf8 { .. } => BomOrigin::Transport,
            Self::Utf16 { .. } => BomOrigin::DecodedScalar,
        }
    }

    pub(crate) fn initial_offset(&self) -> usize {
        match self {
            Self::Utf8 { text, .. } => utf8_bom_length(text.as_bytes()),
            Self::Utf16 { skip, .. } => *skip,
        }
    }

    pub(crate) const fn encoded_width(&self, character: char) -> usize {
        match self {
            Self::Utf8 { .. } => character.len_utf8(),
            Self::Utf16 { .. } => character.len_utf16() * 2,
        }
    }

    pub(crate) fn excerpt(&self, offset: usize, width: usize) -> Option<Box<SourceExcerpt>> {
        let (bytes, view) = match self {
            Self::Utf8 { text, view } => (text.as_bytes(), *view),
            Self::Utf16 { encoded, view, .. } => (encoded.as_ref(), *view),
        };
        character_excerpt(bytes, view, offset, width)
    }

    pub(crate) fn discard_encoded_source(&mut self) {
        if let Self::Utf16 { encoded, .. } = self {
            *encoded = Cow::Borrowed(&[]);
        }
    }

    pub(crate) fn into_parts(self) -> (Cow<'a, str>, Option<Cow<'a, [u8]>>, ExcerptView, usize) {
        let initial_offset = self.initial_offset();
        match self {
            Self::Utf8 { text, view } => (text, None, view, initial_offset),
            Self::Utf16 {
                text,
                encoded,
                view,
                skip,
            } => (Cow::Owned(text), Some(encoded), view, skip),
        }
    }
}

/// Decode an XML source once while retaining the correct exact-byte view for
/// recovery diagnostics. Owned gzip UTF-8 is converted into a String in place.
pub fn decode_recovery_input(bytes: &[u8]) -> Result<DecodedRecoveryInput<'_>> {
    let view = input_view(bytes);
    let encoded = document_input::decode_xml(bytes)?;
    if let Some((_, skip)) = utf16_encoding(&encoded) {
        let text = decode_text(&encoded, view)?.into_owned();
        return Ok(DecodedRecoveryInput::Utf16 {
            text,
            encoded,
            view,
            skip,
        });
    }

    let text = match encoded {
        Cow::Borrowed(bytes) => decode_text(bytes, view)?,
        Cow::Owned(bytes) => {
            let text = String::from_utf8(bytes).map_err(|error| {
                let invalid = error.utf8_error();
                let bytes = error.as_bytes();
                encoding_error(
                    format!("XML is not valid UTF-8: {invalid}"),
                    character_excerpt(
                        bytes,
                        view,
                        invalid.valid_up_to(),
                        invalid
                            .error_len()
                            .unwrap_or_else(|| bytes.len() - invalid.valid_up_to()),
                    ),
                )
            })?;
            validate_declared_encoding(&text, TextEncoding::Utf8)?;
            Cow::Owned(text)
        }
    };
    Ok(DecodedRecoveryInput::Utf8 { text, view })
}

const fn input_view(bytes: &[u8]) -> ExcerptView {
    if matches!(bytes, [0x1f, 0x8b, ..]) {
        ExcerptView::TransportDecodedXmlBytes
    } else {
        ExcerptView::RetainedOriginalBytes
    }
}

const fn encoded_width(character: char, utf16: bool) -> usize {
    if utf16 {
        character.len_utf16() * 2
    } else {
        character.len_utf8()
    }
}

fn character_excerpt(
    bytes: &[u8],
    view: ExcerptView,
    offset: usize,
    width: usize,
) -> Option<Box<SourceExcerpt>> {
    let problem = ByteRange::new(offset, offset.checked_add(width)?)?;
    SourceExcerpt::capture(bytes, view, problem, Some(problem)).map(Box::new)
}

fn encoding_error(message: String, excerpt: Option<Box<SourceExcerpt>>) -> Error {
    Error::CatalogParse {
        message,
        record_kind: Some("document".into()),
        record_name: None,
        line: None,
        column: None,
        excerpt,
        coordinates: None,
    }
}

fn decode_text(bytes: &[u8], view: ExcerptView) -> Result<Cow<'_, str>> {
    if let Some((little_endian, skip)) = utf16_encoding(bytes) {
        let text = decode_utf16(bytes, little_endian, skip, view)?;
        validate_declared_encoding(
            &text,
            if little_endian {
                TextEncoding::Utf16LittleEndian
            } else {
                TextEncoding::Utf16BigEndian
            },
        )?;
        return Ok(Cow::Owned(text));
    }
    let text = std::str::from_utf8(bytes).map_err(|error| {
        encoding_error(
            format!("XML is not valid UTF-8: {error}"),
            character_excerpt(
                bytes,
                view,
                error.valid_up_to(),
                error
                    .error_len()
                    .unwrap_or_else(|| bytes.len() - error.valid_up_to()),
            ),
        )
    })?;
    validate_declared_encoding(text, TextEncoding::Utf8)?;
    Ok(Cow::Borrowed(text))
}

#[derive(Clone, Copy)]
enum TextEncoding {
    Utf8,
    Utf16LittleEndian,
    Utf16BigEndian,
}

fn validate_declared_encoding(text: &str, actual: TextEncoding) -> Result<()> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(declaration) = text.strip_prefix("<?xml") else {
        return Ok(());
    };
    // A different PI name (such as xml-stylesheet) is not a declaration.
    if !declaration.starts_with([' ', '\t', '\r', '\n']) {
        return Ok(());
    }
    let Some(end) = text.find("?>") else {
        return Ok(());
    };
    let declaration = BytesDecl::from_start(BytesStart::from_content(&text[2..end], 3));
    let Some(encoding) = declaration.encoding() else {
        return Ok(());
    };
    let encoding = encoding.map_err(|error| Error::XmlValidation(error.to_string()))?;
    let valid = match actual {
        TextEncoding::Utf8 => encoding.eq_ignore_ascii_case("UTF-8"),
        TextEncoding::Utf16LittleEndian => {
            encoding.eq_ignore_ascii_case("UTF-16") || encoding.eq_ignore_ascii_case("UTF-16LE")
        }
        TextEncoding::Utf16BigEndian => {
            encoding.eq_ignore_ascii_case("UTF-16") || encoding.eq_ignore_ascii_case("UTF-16BE")
        }
    };
    if !valid {
        return Err(Error::XmlValidation(format!(
            "unsupported or mismatched XML encoding {encoding:?}"
        )));
    }
    Ok(())
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

fn decode_utf16(
    bytes: &[u8],
    little_endian: bool,
    skip: usize,
    view: ExcerptView,
) -> Result<String> {
    let remaining = bytes
        .get(skip..)
        .filter(|remaining| remaining.len() % 2 == 0)
        .ok_or_else(|| {
            encoding_error(
                "malformed UTF-16 encoding".into(),
                character_excerpt(bytes, view, bytes.len().saturating_sub(1), 1),
            )
        })?;
    let units = remaining.as_chunks::<2>().0.iter().map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let mut output = String::with_capacity(bytes.len().saturating_sub(skip));
    let mut source_offset = skip;
    for scalar in char::decode_utf16(units) {
        let character = scalar.map_err(|_| {
            encoding_error(
                "malformed UTF-16 encoding".into(),
                character_excerpt(bytes, view, source_offset, 2),
            )
        })?;
        output.push(character);
        source_offset += character.len_utf16() * 2;
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
    reader: &mut XmlReader<'a>,
    positions: &mut PositionMap<'_>,
) -> Result<(Option<String>, Event<'a>)> {
    let event_offset = reader.buffer_position();
    let event = match reader.read_event() {
        Ok(event) => event,
        Err(error) => {
            let offset = reader.reader.error_position();
            // An error can point behind an already visited attribute. Keep the
            // actual parse error, but never invent a location from that cursor.
            let location = positions.event_location(offset).ok();
            return Err(Error::CatalogParse {
                message: error.to_string(),
                record_kind: Some("document".into()),
                record_name: None,
                line: location.map(|location| location.line),
                column: location.map(|location| location.column),
                excerpt: None,
                coordinates: location
                    .map(|_| crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
            });
        }
    };
    let namespace = match &event {
        Event::Start(start) | Event::Empty(start) => {
            reader.resolver().resolve_element(start.name()).0
        }
        Event::End(end) => reader.resolver().resolve_element(end.name()).0,
        _ => ResolveResult::Unbound,
    };
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.as_ref().to_owned()),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(prefix) => {
            return Err(Error::XmlValidation(format!(
                "unbound XML namespace prefix {prefix:?}"
            )));
        }
    };
    match &event {
        Event::Decl(declaration) => validate_declaration(declaration, event_offset)?,
        Event::DocType(declaration) => {
            validate_doctype_opening(positions, event_offset, reader.buffer_position())?;
            if declaration.as_ref().contains("<!ENTITY") {
                return Err(Error::XmlEntityNotAllowed);
            }
        }
        Event::GeneralRef(reference) if !allowed_reference(reference) => {
            return Err(Error::XmlEntityNotAllowed);
        }
        Event::Start(start) | Event::Empty(start) => validate_start(start, reader)?,
        Event::PI(instruction) => {
            let target = instruction.target();
            if target.eq_ignore_ascii_case("xml") || !is_ncname(target) {
                return Err(Error::XmlValidation(format!(
                    "invalid XML processing-instruction target {target:?}"
                )));
            }
        }
        Event::Text(text) if text.as_ref().contains("]]>") => {
            return Err(Error::XmlValidation(
                "literal ]]> is forbidden in character data".into(),
            ));
        }
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
    Ok((namespace, event))
}

fn validate_doctype_opening(positions: &PositionMap<'_>, start: u64, end: u64) -> Result<()> {
    // quick-xml excludes the transport BOM from both buffer offsets; the
    // retained decoded source keeps it for source-position reconstruction.
    let start = positions.event_source_offset(start)?;
    let end = positions.event_source_offset(end)?;
    let source = positions
        .bytes
        .get(start..end)
        .ok_or_else(|| Error::XmlValidation("invalid DOCTYPE source range".into()))?;
    if !source
        .strip_prefix(b"<!DOCTYPE")
        .and_then(|remainder| remainder.first())
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return Err(Error::XmlValidation(
            "DOCTYPE requires exact uppercase markup followed by XML whitespace".into(),
        ));
    }
    Ok(())
}

fn validate_start(start: &BytesStart<'_>, reader: &XmlReader<'_>) -> Result<()> {
    validate_qname(start.name().as_ref())?;
    if start.name().as_ref().starts_with("xmlns:") {
        return Err(Error::XmlValidation(
            "xmlns cannot be an element prefix".into(),
        ));
    }
    if start.name().as_ref().len() > MAX_NAME_BYTES {
        return Err(Error::XmlValidation(format!(
            "XML element name exceeds {MAX_NAME_BYTES} bytes"
        )));
    }
    let mut count = 0;
    let mut expanded_names = HashSet::new();
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
        validate_qname(attribute.key.as_ref())?;
        validate_attribute_spacing(start.as_ref(), &attribute)?;
        if attribute.value.contains('<') {
            return Err(Error::XmlValidation(
                "literal < is forbidden in attribute values".into(),
            ));
        }
        // Raw characters were checked before traversal. Numeric references can
        // nevertheless decode to XML-forbidden characters inside attributes.
        if attribute.value.contains('&') {
            let decoded = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| Error::XmlValidation(error.to_string()))?;
            if let Some(character) = decoded
                .chars()
                .find(|character| !is_xml10_character(*character))
            {
                return Err(Error::XmlValidation(format!(
                    "XML 1.0 forbids referenced U+{:04X}",
                    u32::from(character)
                )));
            }
        }
        let key = attribute.key.as_ref();
        if key != "xmlns" && !key.starts_with("xmlns:") {
            let (namespace, local) = reader.resolver().resolve_attribute(attribute.key);
            let namespace = match namespace {
                ResolveResult::Bound(namespace) => Some(namespace.as_ref().to_owned()),
                ResolveResult::Unbound => None,
                ResolveResult::Unknown(prefix) => {
                    return Err(Error::XmlValidation(format!(
                        "unbound XML attribute prefix {prefix:?}"
                    )));
                }
            };
            if !expanded_names.insert((namespace, local.into_inner())) {
                return Err(Error::XmlValidation(format!(
                    "duplicate expanded XML attribute {key:?}"
                )));
            }
        }
    }
    Ok(())
}

fn validate_attribute_spacing(content: &str, attribute: &Attribute<'_>) -> Result<()> {
    let offset = (attribute.key.as_ref().as_ptr() as usize)
        .checked_sub(content.as_ptr() as usize)
        .ok_or_else(|| Error::XmlValidation("invalid attribute source position".into()))?;
    if !content
        .get(..offset)
        .and_then(|prefix| prefix.bytes().last())
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return Err(Error::XmlValidation(
            "XML attributes must be separated by whitespace".into(),
        ));
    }
    Ok(())
}

/// Resolve the XML-normalized value of a borrowed namespace declaration.
pub fn normalize_namespace(namespace: &str) -> Result<Cow<'_, str>> {
    Attribute {
        key: QName("xmlns"),
        value: Cow::Borrowed(namespace),
    }
    .normalized_value(quick_xml::XmlVersion::Implicit1_0)
    .map_err(|error| Error::XmlValidation(error.to_string()))
}

fn validate_namespace_binding(key: &str, namespace: &str) -> Result<()> {
    const XML: &str = "http://www.w3.org/XML/1998/namespace";
    const XMLNS: &str = "http://www.w3.org/2000/xmlns/";
    let prefix = key.strip_prefix("xmlns:");
    if namespace == XMLNS
        || prefix == Some("xmlns")
        || (prefix == Some("xml") && namespace != XML)
        || (prefix != Some("xml") && namespace == XML)
        || (prefix.is_some() && namespace.is_empty())
    {
        return Err(Error::XmlValidation(format!(
            "invalid namespace binding {key:?}={namespace:?}"
        )));
    }
    Ok(())
}

pub fn validate_qname(name: &str) -> Result<()> {
    let valid = name.split_once(':').map_or_else(
        || is_ncname(name),
        |(prefix, local)| is_ncname(prefix) && is_ncname(local),
    );
    if valid {
        Ok(())
    } else {
        Err(Error::XmlValidation(format!("invalid XML QName {name:?}")))
    }
}

fn is_ncname(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| xml_name_start_char(character, false))
        && characters.all(|character| xml_name_char(character, false))
}

pub const fn xml_name_start_char(character: char, allow_colon: bool) -> bool {
    (allow_colon && character == ':')
        || matches!(character as u32,
        0x41..=0x5a | 0x5f | 0x61..=0x7a | 0xc0..=0xd6 | 0xd8..=0xf6 | 0xf8..=0x2ff
        | 0x370..=0x37d | 0x37f..=0x1fff | 0x200c..=0x200d | 0x2070..=0x218f
        | 0x2c00..=0x2fef | 0x3001..=0xd7ff | 0xf900..=0xfdcf | 0xfdf0..=0xfffd | 0x10000..=0xeffff)
}

pub const fn xml_name_char(character: char, allow_colon: bool) -> bool {
    xml_name_start_char(character, allow_colon)
        || matches!(character as u32, 0x2d..=0x2e | 0x30..=0x39 | 0xb7 | 0x300..=0x36f | 0x203f..=0x2040)
}

fn validate_declaration(declaration: &BytesDecl<'_>, offset: u64) -> Result<()> {
    if offset != 0 {
        return Err(Error::XmlValidation(
            "XML declaration must be the first document event".into(),
        ));
    }
    let attributes = BytesStart::from_content(declaration.as_ref(), 3);
    let mut previous_rank = None;
    for (index, attribute) in attributes.attributes().enumerate() {
        let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
        validate_attribute_spacing(declaration.as_ref(), &attribute)?;
        let name = attribute.key.as_ref();
        let value = attribute.value.as_ref();
        if index == 0 && name != "version" {
            return Err(Error::XmlValidation(
                "XML declaration requires version first".into(),
            ));
        }
        let (rank, valid) = match name {
            "version" => (0, value == "1.0"),
            "encoding" => (
                1,
                value
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphabetic)
                    && value.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    }),
            ),
            "standalone" => (2, matches!(value, "yes" | "no")),
            _ => {
                return Err(Error::XmlValidation(format!(
                    "unknown XML declaration field {name:?}"
                )));
            }
        };
        if !valid || previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(Error::XmlValidation(format!(
                "invalid XML declaration field {name:?}"
            )));
        }
        previous_rank = Some(rank);
    }
    if previous_rank.is_none() {
        return Err(Error::XmlValidation(
            "XML declaration requires a version".into(),
        ));
    }
    Ok(())
}

const fn is_xml10_character(character: char) -> bool {
    matches!(character as u32,
        0x09 | 0x0a | 0x0d | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x0010_ffff)
}

/// Convert the current start/empty event into the extension tree.
pub fn element_from_start(
    reader: &XmlReader<'_>,
    namespace: Option<String>,
    start: &BytesStart<'_>,
    budget: &mut NodeBudget,
    depth: usize,
    positions: &mut PositionMap<'_>,
) -> Result<Element> {
    budget.include(depth)?;
    let name = expanded_name(namespace, start.local_name().as_ref());
    let location = positions.start_location(start)?;
    let mut attributes = XmlAttributes::default();
    let mut has_namespace_declarations = false;
    for (source_order, attribute) in start.attributes().enumerate() {
        let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
        let location = positions.attribute_location(&attribute)?;
        let raw_name = attribute.key.as_ref();
        if raw_name == "xmlns" || raw_name.starts_with("xmlns:") {
            has_namespace_declarations = true;
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
        if !attributes.push_declared(
            name.clone(),
            DeclaredText {
                value,
                source_order,
                location,
            },
        ) {
            return Err(Error::XmlValidation(format!(
                "duplicate XML attribute {name:?}"
            )));
        }
    }
    Ok(Element {
        name,
        attributes,
        content: Vec::new(),
        content_kind: ElementContentKind::Empty,
        has_namespace_declarations,
        location,
    })
}

/// Consume one record subtree so adapters need not build a whole-document DOM.
pub fn read_element(
    reader: &mut XmlReader<'_>,
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
            Event::Start(child) => {
                element
                    .content_kind
                    .include(ElementContentKind::ElementOnly);
                element.content.push(ElementContent::Element(read_element(
                    reader,
                    namespace,
                    &child,
                    budget,
                    depth.saturating_add(1),
                    positions,
                )?));
            }
            Event::Empty(child) => {
                element
                    .content_kind
                    .include(ElementContentKind::ElementOnly);
                let child = element_from_start(
                    reader,
                    namespace,
                    &child,
                    budget,
                    depth.saturating_add(1),
                    positions,
                )?;
                element.content.push(ElementContent::Element(child));
            }
            Event::Text(text) => {
                let text = text.xml10_content();
                if !text.is_empty() {
                    let kind = if text
                        .chars()
                        .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n'))
                    {
                        ElementContentKind::ElementOnly
                    } else {
                        ElementContentKind::CharacterData
                    };
                    element.content_kind.include(kind);
                    element
                        .content
                        .push(ElementContent::Text(text.into_owned()));
                }
            }
            Event::CData(text) => {
                element
                    .content_kind
                    .include(ElementContentKind::CharacterData);
                element
                    .content
                    .push(ElementContent::Text(text.as_ref().to_owned()));
            }
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
                element
                    .content_kind
                    .include(ElementContentKind::CharacterData);
                element.content.push(ElementContent::Text(text));
            }
            Event::Comment(_) | Event::PI(_) => {
                element
                    .content_kind
                    .include(ElementContentKind::ElementOnly);
            }
            Event::DocType(_) => {
                return Err(Error::XmlValidation(
                    "DOCTYPE is not allowed inside an element".into(),
                ));
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
                    excerpt: None,
                    coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
                });
            }
            Event::Decl(_) => {}
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
    reference
        .resolve_char_ref()
        .ok()
        .flatten()
        .is_some_and(is_xml10_character)
        || matches!(reference.as_ref(), "amp" | "lt" | "gt" | "apos" | "quot")
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;
    use std::io::Write;

    use super::*;

    fn validate_events(input: &[u8]) -> Result<()> {
        with_reader(input, |reader, positions| {
            while !matches!(next(reader, positions)?.1, Event::Eof) {}
            Ok(())
        })
    }

    #[test]
    fn source_byte_view_reports_complete_mappable_input_view_lengths() -> Result<()> {
        use flate2::{Compression, write::GzEncoder};

        let plain = b"<root/>";
        let plain_view =
            with_reader::<_, Error>(plain, |_, positions| Ok(positions.source_byte_view()))?;
        assert_eq!(
            plain_view,
            Some(SourceByteView::RetainedOriginal {
                byte_length: plain.len()
            })
        );

        let mut bom_xml = b"\xef\xbb\xbf".to_vec();
        bom_xml.extend_from_slice(plain);
        let bom_view =
            with_reader::<_, Error>(&bom_xml, |_, positions| Ok(positions.source_byte_view()))?;
        assert_eq!(
            bom_view,
            Some(SourceByteView::RetainedOriginal {
                byte_length: bom_xml.len()
            })
        );

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain)?;
        let compressed = encoder.finish()?;
        let gzip_view =
            with_reader::<_, Error>(&compressed, |_, positions| Ok(positions.source_byte_view()))?;
        assert_eq!(
            gzip_view,
            Some(SourceByteView::TransportDecodedXml {
                byte_length: plain.len()
            })
        );

        let utf16 = [
            0xff, 0xfe, b'<', 0, b'r', 0, b'o', 0, b'o', 0, b't', 0, b'/', 0, b'>', 0,
        ];
        let utf16_view =
            with_reader::<_, Error>(&utf16, |_, positions| Ok(positions.source_byte_view()))?;
        assert_eq!(utf16_view, None);

        let recovered_view =
            with_decoded_reader::<_, Error>("<root/>", BomOrigin::Transport, |_, positions| {
                Ok(positions.source_byte_view())
            })?;
        assert_eq!(recovered_view, None);

        let repaired_map = PositionMap::with_source_byte_mapping(plain, BomOrigin::Transport, None);
        assert_eq!(repaired_map.source_byte_view(), None);
        Ok(())
    }

    #[test]
    fn encoded_characters_must_belong_to_xml_10_in_text_and_attributes() {
        for xml in [
            "<root>&#x1;</root>",
            "<root name='&#1;'/>",
            "<root><vendor>&#xB;</vendor></root>",
            "<root name='&#xFFFE;'/>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_err(), "{xml}");
        }
        assert!(validate_events(b"<root name='&#9;'>&#xA;&#xD;&#x10000;</root>").is_ok());
    }

    #[test]
    fn declarations_are_validated_and_only_allowed_at_the_document_start() {
        for xml in [
            "<root/><?xml version='1.0'?>",
            " <?xml version='1.0'?><root/>",
            "<root><?xml version='1.0'?></root>",
            "<?xml?><root/>",
            "<?xml version='garbage'?><root/>",
            "<?xml version='1.0' standalone='maybe'?><root/>",
            "<?xml version='1.0' encoding='9bad'?><root/>",
            "<?xml version='1.0' standalone='yes' encoding='UTF-8'?><root/>",
            "<?xml version='1.0' extra='value'?><root/>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_err(), "{xml}");
        }
        assert!(
            validate_events(b"<?xml version='1.0' encoding='UTF-8' standalone='yes'?><root/>")
                .is_ok()
        );
        assert!(validate_events(b"\xef\xbb\xbf<?xml version='1.0'?><root/>").is_ok());
    }

    #[test]
    fn lexical_xml_rules_apply_to_known_and_skipped_elements() {
        for xml in [
            "<root name='a<b'/>",
            "<root>a]]>b</root>",
            "<root name='a'size='1'/>",
            "<root><?XML version='1.0'?></root>",
            "<?xml version='1.0'encoding='UTF-8'?><root/>",
            "<root><vendor name='a<b'/></root>",
            "<root><vendor>a]]>b</vendor></root>",
            "<root><vendor name='a'size='1'/></root>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_err(), "{xml}");
        }
        for xml in [
            "<root name='a&lt;b'>a]]&gt;b</root>",
            "<?xml-stylesheet href='style.xsl'?><root/>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_ok(), "{xml}");
        }
    }

    #[test]
    fn qnames_and_namespace_declarations_are_well_formed() {
        for xml in [
            "<1root/>",
            "<root a:b:c='1'/>",
            "<root xmlns:p='urn:p'><p:1bad/></root>",
            "<root><vendor bad!name='1'/></root>",
            "<root xmlns:xml='urn:wrong'/>",
            "<root xmlns:p='http://www.w3.org/XML/1998/namespace'/>",
            "<root xmlns='http://www.w3.org/XML/1998/namespace'/>",
            "<root xmlns:xmlns='urn:bad'/>",
            "<root xmlns:p='http://www.w3.org/2000/xmlns/'/>",
            "<root xmlns:p=''/>",
            "<root><vendor p:unbound='1'/></root>",
            "<root xmlns:p='urn:a&amp;b' xmlns:q='urn:a&#38;b' p:x='1' q:x='2'/>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_err(), "{xml}");
        }
        for xml in [
            "<root xmlns:xml='http://www.w3.org/XML/1998/namespace' xml:lang='en'/>",
            "<root xmlns='urn:outer'><child xmlns=''/></root>",
            "<échantillon xmlns:p='urn:p' p:名='1'/>",
        ] {
            assert!(validate_events(xml.as_bytes()).is_ok(), "{xml}");
        }
    }

    #[test]
    fn namespace_bindings_use_xml_attribute_normalization() -> Result<()> {
        let namespace = with_reader(b"<root xmlns='urn:a&amp;b'/>", |reader, positions| {
            Ok::<_, Error>(next(reader, positions)?.0)
        })?;
        assert_eq!(namespace.as_deref(), Some("urn:a&b"));
        Ok(())
    }

    #[test]
    fn extension_attributes_iterate_in_source_order() -> Result<()> {
        let attributes = with_reader(b"<root z='last' a='first'/>", |reader, positions| {
            let (namespace, event) = next(reader, positions)?;
            let Event::Empty(start) = event else {
                return Err(Error::XmlValidation("expected empty root".into()));
            };
            element_from_start(
                reader,
                namespace,
                &start,
                &mut NodeBudget::default(),
                0,
                positions,
            )
        })?;

        assert_eq!(
            attributes
                .attributes
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
        Ok(())
    }

    fn empty_element_attributes(xml: &[u8]) -> Result<XmlAttributes> {
        with_reader(xml, |reader, positions| {
            let (namespace, start) = loop {
                let (namespace, event) = next(reader, positions)?;
                match event {
                    Event::Decl(_) => {}
                    Event::Empty(start) => break (namespace, start),
                    _ => return Err(Error::XmlValidation("expected empty element".into())),
                }
            };
            Ok(element_from_start(
                reader,
                namespace,
                &start,
                &mut NodeBudget::default(),
                0,
                positions,
            )?
            .attributes)
        })
    }

    fn declared<'a>(attributes: &'a XmlAttributes, name: &str) -> Result<&'a DeclaredText> {
        attributes
            .get_declared(name)
            .ok_or_else(|| Error::XmlValidation(format!("missing test attribute {name:?}")))
    }

    #[test]
    fn attributes_keep_resolved_names_normalized_values_and_lexical_ordinals() -> Result<()> {
        let xml = "<root xmlns:p='urn:p' vendor='x' p:名='é\r\nz' value='a\tb&#x9;c'/>";
        let attributes = empty_element_attributes(xml.as_bytes())?;

        assert_eq!(declared(&attributes, "vendor")?.source_order, 1);
        let namespaced = declared(&attributes, "{urn:p}名")?;
        assert_eq!(namespaced.source_order, 2);
        assert_eq!(namespaced.value, "é z");
        let (prefix, _) = xml
            .split_once("p:名=")
            .ok_or_else(|| Error::XmlValidation("missing namespaced test QName".into()))?;
        let expected_column = i64::try_from(prefix.chars().count() + 1)
            .map_err(|error| Error::XmlValidation(error.to_string()))?;
        assert_eq!(
            (namespaced.location.line, namespaced.location.column),
            (1, expected_column),
            "the original prefix is part of the attribute token"
        );
        let value = declared(&attributes, "value")?;
        assert_eq!(value.source_order, 3);
        assert_eq!(value.value, "a b\tc");
        assert_eq!(
            attributes.keys().map(String::as_str).collect::<Vec<_>>(),
            ["vendor", "{urn:p}名", "value"]
        );
        Ok(())
    }

    #[test]
    fn attribute_locations_use_qname_start_and_unicode_scalar_columns() -> Result<()> {
        let attributes =
            empty_element_attributes("<root\r\n\tz='1'\t名='é'\r\n a='3'/>".as_bytes())?;

        for (name, expected) in [("z", (2, 2)), ("名", (2, 8)), ("a", (3, 2))] {
            let location = declared(&attributes, name)?.location;
            assert_eq!((location.line, location.column), expected, "{name}");
        }
        Ok(())
    }

    #[test]
    fn attribute_locations_use_decoded_coordinates_for_utf16() -> Result<()> {
        let xml = "<?xml version='1.0' encoding='UTF-16'?><root z='1' a='2'/>";
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        let attributes = empty_element_attributes(&utf16)?;

        let location = declared(&attributes, "a")?.location;
        let (prefix, _) = xml
            .split_once("a='2'")
            .ok_or_else(|| Error::XmlValidation("missing test QName".into()))?;
        let expected_column = i64::try_from(prefix.chars().count() + 1)
            .map_err(|error| Error::XmlValidation(error.to_string()))?;
        assert_eq!((location.line, location.column), (1, expected_column));
        Ok(())
    }

    #[test]
    fn attribute_locations_ignore_only_the_transport_bom() -> Result<()> {
        let text = "<root z='\u{feff}' a='2'/>";
        let input = format!("\u{feff}{text}");
        let attributes = empty_element_attributes(input.as_bytes())?;
        let location = declared(&attributes, "a")?.location;
        let (prefix, _) = text
            .split_once("a='2'")
            .ok_or_else(|| Error::XmlValidation("missing test QName".into()))?;
        let column = i64::try_from(prefix.chars().count() + 1)
            .map_err(|error| Error::XmlValidation(error.to_string()))?;
        assert_eq!((location.line, location.column), (1, column));
        Ok(())
    }

    #[test]
    fn attribute_serde_keeps_canonical_sorted_keys() -> Result<()> {
        let attributes = empty_element_attributes(b"<root z='last' a='first'/>")?;
        assert_eq!(
            serde_json::to_string(&attributes)?,
            r#"{"a":"first","z":"last"}"#
        );
        Ok(())
    }

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
                positions.event_location(reader.buffer_position())?,
                attribute,
            ))
        })?;
        assert_eq!(name, "root");
        assert_eq!(namespace, "urn:test");
        assert_eq!(
            position,
            RecordLocation {
                line: 2,
                column: 25
            }
        );
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

    #[test]
    fn source_positions_use_xml_line_end_rules_in_utf8_and_utf16() -> Result<()> {
        for ending in ["\r", "\n", "\r\n"] {
            let xml = format!("<root>{ending} <child/>{ending}</root>");
            let mut utf16 = vec![0xff, 0xfe];
            utf16.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
            for input in [xml.as_bytes(), utf16.as_slice()] {
                with_reader(input, |reader, positions| {
                    loop {
                        match next(reader, positions)?.1 {
                            Event::Empty(start) => {
                                let location = positions.start_location(&start)?;
                                assert_eq!((location.line, location.column), (2, 2), "{ending:?}");
                            }
                            Event::Eof => break,
                            _ => {}
                        }
                    }
                    Ok::<_, Error>(())
                })?;
            }
        }
        Ok(())
    }

    #[test]
    fn source_positions_keep_crlf_state_between_offsets_and_count_unicode_scalars() -> Result<()> {
        let mut positions = PositionMap::new("\r\né💿\rZ".as_bytes(), BomOrigin::Transport);
        for (offset, line, column) in [
            (1, 2, 1),
            (2, 2, 1),
            (4, 2, 2),
            (8, 2, 3),
            (9, 3, 1),
            (10, 3, 2),
        ] {
            assert_eq!(
                positions.source_location(offset)?,
                RecordLocation { line, column }
            );
        }
        Ok(())
    }

    #[test]
    fn source_boundaries_reject_backwards_out_of_buffer_and_split_utf8() -> Result<()> {
        let mut positions = PositionMap::new("\u{feff}é😀".as_bytes(), BomOrigin::Transport);
        for offset in [0, 2, 4, 6, 7, 8, 10, usize::MAX] {
            assert!(
                positions.source_location(offset).is_err(),
                "offset {offset}"
            );
        }
        assert_eq!(
            positions.source_location(5)?,
            RecordLocation { line: 1, column: 2 }
        );
        assert!(positions.source_location(3).is_err());
        assert_eq!(
            positions.source_location(9)?,
            RecordLocation { line: 1, column: 3 }
        );
        assert!(positions.event_location(u64::MAX).is_err());
        assert!(positions.event_location(7).is_err());
        Ok(())
    }

    #[test]
    fn event_boundaries_translate_bom_once_and_include_closing_tags() -> Result<()> {
        let text = "<root>é😀\r\n<child a='\u{feff}'/></root>";
        let bom_text = format!("\u{feff}{text}");
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        for input in [text.as_bytes(), bom_text.as_bytes(), utf16.as_slice()] {
            with_reader(input, |reader, positions| {
                let mut boundaries = Vec::new();
                loop {
                    match next(reader, positions)?.1 {
                        Event::Start(start) | Event::Empty(start) => {
                            let start_location = positions.start_location(&start)?;
                            let end = positions.event_location(reader.buffer_position())?;
                            boundaries.push((start_location, end));
                        }
                        Event::End(_) => {
                            let end = positions.event_location(reader.buffer_position())?;
                            boundaries.push((end, end));
                        }
                        Event::Eof => {
                            let end = positions.event_location(reader.buffer_position())?;
                            assert_eq!(
                                end,
                                RecordLocation {
                                    line: 2,
                                    column: 22
                                }
                            );
                            break;
                        }
                        _ => {}
                    }
                }
                assert_eq!(
                    boundaries,
                    vec![
                        (
                            RecordLocation { line: 1, column: 1 },
                            RecordLocation { line: 1, column: 7 }
                        ),
                        (
                            RecordLocation { line: 2, column: 1 },
                            RecordLocation {
                                line: 2,
                                column: 15
                            }
                        ),
                        (
                            RecordLocation {
                                line: 2,
                                column: 22
                            },
                            RecordLocation {
                                line: 2,
                                column: 22
                            }
                        ),
                    ]
                );
                Ok::<_, Error>(())
            })?;
        }
        Ok(())
    }

    #[test]
    fn transport_bom_does_not_shift_invalid_character_coordinates() -> Result<()> {
        let text = "<root>é😀\u{feff}\0</root>";
        let utf8 = format!("\u{feff}{text}");
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        for (input, offset, width) in [(utf8.as_bytes(), 18, 1), (utf16.as_slice(), 22, 2)] {
            let error = with_reader(input, |_, _| Ok::<_, Error>(()));
            let Err(Error::CatalogParse {
                line,
                column,
                excerpt: Some(excerpt),
                ..
            }) = error
            else {
                return Err(Error::XmlValidation(
                    "expected forbidden-character evidence".into(),
                ));
            };
            assert_eq!((line, column), (Some(1), Some(10)));
            assert_eq!(
                excerpt.source_problem(),
                ByteRange::new(offset, offset + width)
            );
            assert_eq!(excerpt.bytes(), vec![0; width]);
        }
        Ok(())
    }

    #[test]
    fn decoded_scalar_prefix_counts_at_opening_and_document_end() -> Result<()> {
        let text = "\u{feff}<root/>";
        for little_endian in [true, false] {
            let mut input = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in text.encode_utf16() {
                input.extend(if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            with_reader(&input, |reader, positions| {
                let Event::Empty(start) = next(reader, positions)?.1 else {
                    return Err(Error::XmlValidation("expected empty root".into()));
                };
                assert_eq!(
                    positions.start_location(&start)?,
                    RecordLocation { line: 1, column: 2 }
                );
                assert_eq!(
                    positions.event_location(reader.buffer_position())?,
                    RecordLocation { line: 1, column: 9 }
                );
                assert_eq!(next(reader, positions)?.1, Event::Eof);
                assert_eq!(
                    positions.event_location(reader.buffer_position())?,
                    RecordLocation { line: 1, column: 9 }
                );
                Ok::<_, Error>(())
            })?;
        }
        Ok(())
    }

    #[test]
    fn malformed_end_tag_coordinates_translate_reader_bom_offsets() -> Result<()> {
        let text = "<root>é😀</bad>";
        let bom_text = format!("\u{feff}{text}");
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        for input in [text.as_bytes(), bom_text.as_bytes(), utf16.as_slice()] {
            let error = with_reader(input, |reader, positions| {
                while next(reader, positions)?.1 != Event::Eof {}
                Ok::<_, Error>(())
            });
            let Err(Error::CatalogParse {
                line,
                column,
                coordinates,
                ..
            }) = error
            else {
                return Err(Error::XmlValidation("expected malformed end tag".into()));
            };
            assert_eq!((line, column), (Some(1), Some(9)));
            assert_eq!(
                coordinates,
                Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars)
            );
        }
        Ok(())
    }

    #[test]
    fn declared_encoding_must_be_supported_and_match_the_original_bytes() {
        for encoding in ["MadeUp", "UTF-16", "UTF-16LE", "UTF-16BE"] {
            let xml = format!("<?xml version='1.0' encoding='{encoding}'?><root/>");
            assert!(
                with_reader(xml.as_bytes(), |reader, positions| {
                    while next(reader, positions)?.1 != Event::Eof {}
                    Ok::<_, Error>(())
                })
                .is_err(),
                "UTF-8 bytes declared {encoding}"
            );
        }
        for (little_endian, encoding) in [(true, "UTF-8"), (true, "UTF-16BE"), (false, "UTF-16LE")]
        {
            let xml = format!("<?xml version='1.0' encoding='{encoding}'?><root/>");
            let mut bytes = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for unit in xml.encode_utf16() {
                bytes.extend(if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
            assert!(
                with_reader(&bytes, |reader, positions| {
                    while next(reader, positions)?.1 != Event::Eof {}
                    Ok::<_, Error>(())
                })
                .is_err(),
                "wrong UTF-16 byte order/declaration {encoding}"
            );
        }
    }
}
