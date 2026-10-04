use std::{borrow::Cow, str::Chars};

use quick_xml::{
    events::{BytesStart, Event},
    name::ResolveResult,
};

use crate::{
    Error, Result,
    diagnostics::{ByteRange, ExcerptView, SourceExcerpt},
    logiqx::RecordLocation,
    xml_reader::{self, DeclaredText, PositionMap, SourcePosition, XmlReader},
};

use super::model::{
    ArchiveClone, ArchiveDescription, DatabaseDigest, DatabaseGame, DatabaseHeader,
    DatabaseRelease, DumpSource, EnvelopeKind, HeaderField, HeaderFieldKind,
    NoIntroDatabaseDocument, NoIntroDatabaseMode, RecoveryWarning, ReleaseDetails, ReleaseFile,
    ReleaseSerials, SourceDetails, SourceFile, SourceOrRelease, SourceSerials,
};

#[cfg(test)]
mod tests;

const MAX_FIELDS_PER_ELEMENT: usize = 128;

/// Caller state returned only after the XML document reaches valid EOF.
///
/// Construction is private so this value proves that framing, all game
/// records, the final closing tags, and trailing document content were parsed.
#[must_use = "the EOF proof should be retained until publication is complete"]
pub struct ValidatedNoIntroDatabase<'a, S> {
    inner: S,
    recovery_source: Option<RecoverySource<'a>>,
}

impl<'a, S> ValidatedNoIntroDatabase<'a, S> {
    const fn after_eof(inner: S, recovery_source: Option<RecoverySource<'a>>) -> Self {
        Self {
            inner,
            recovery_source,
        }
    }

    /// Return the state produced by `begin` and `consume` after valid EOF.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.inner
    }

    /// Iterate all recovered U+0000 locations without owning a warning collection.
    #[must_use]
    pub fn recovery_warnings(&self) -> RecoveryWarnings<'_> {
        self.recovery_source
            .as_ref()
            .map_or_else(RecoveryWarnings::empty, RecoverySource::warnings)
    }

    /// Return the EOF-validated state and lazy recovery source for persistence.
    #[must_use]
    pub(crate) fn into_parts(self) -> (S, Option<RecoverySource<'a>>) {
        (self.inner, self.recovery_source)
    }
}

/// Retained decoded source plus its exact pre-recovery byte representation.
pub struct RecoverySource<'a> {
    text: Cow<'a, str>,
    encoded: Option<Cow<'a, [u8]>>,
    view: ExcerptView,
    initial_offset: usize,
}

impl RecoverySource<'_> {
    pub(crate) fn warnings(&self) -> RecoveryWarnings<'_> {
        let content = if self.encoded.is_none() {
            self.text.strip_prefix('\u{feff}').unwrap_or(&self.text)
        } else {
            &self.text
        };
        RecoveryWarnings {
            characters: content.chars(),
            position: SourcePosition::new(),
            bytes: self
                .encoded
                .as_deref()
                .unwrap_or_else(|| self.text.as_bytes()),
            view: self.view,
            utf16: self.encoded.is_some(),
            encoded_offset: self.initial_offset,
        }
    }
}

/// Original coordinates of recovered NULs, generated in source order.
pub struct RecoveryWarnings<'a> {
    characters: Chars<'a>,
    position: SourcePosition,
    bytes: &'a [u8],
    view: ExcerptView,
    utf16: bool,
    encoded_offset: usize,
}

impl RecoveryWarnings<'_> {
    fn empty() -> Self {
        Self {
            characters: "".chars(),
            position: SourcePosition::new(),
            bytes: &[],
            view: ExcerptView::RetainedOriginalBytes,
            utf16: false,
            encoded_offset: 0,
        }
    }
}

impl Iterator for RecoveryWarnings<'_> {
    type Item = RecoveryWarning;

    fn next(&mut self) -> Option<Self::Item> {
        for character in self.characters.by_ref() {
            let location = self.position.location();
            self.position.advance(character);
            let width = if self.utf16 {
                character.len_utf16() * 2
            } else {
                character.len_utf8()
            };
            if character == '\0' {
                let excerpt = self
                    .encoded_offset
                    .checked_add(width)
                    .and_then(|end| ByteRange::new(self.encoded_offset, end))
                    .and_then(|problem| {
                        SourceExcerpt::capture(self.bytes, self.view, problem, Some(problem))
                    });
                self.encoded_offset += width;
                return Some(RecoveryWarning {
                    replaced: '\0',
                    replacement: '\u{fffd}',
                    location,
                    excerpt,
                });
            }
            self.encoded_offset += width;
        }
        None
    }
}

impl std::iter::FusedIterator for RecoveryWarnings<'_> {}

/// Parse one database-export document and deliver each completed game.
///
/// The `begin` callback receives the framing, optional header, selected mode,
/// and source location. `consume` is called in catalog
/// order with one fully typed game at a time. An error through EOF returns no
/// [`ValidatedNoIntroDatabase`] value, even if earlier games were consumed;
/// callers should stage callback effects transactionally.
pub fn read_with<S, E: From<Error>>(
    bytes: &[u8],
    mode: NoIntroDatabaseMode,
    begin: impl FnOnce(NoIntroDatabaseDocument) -> std::result::Result<S, E>,
    consume: impl FnMut(&mut S, DatabaseGame) -> std::result::Result<(), E>,
) -> std::result::Result<ValidatedNoIntroDatabase<'_, S>, E> {
    let mut decoded = xml_reader::decode_recovery_input(bytes)?;
    let mut position = SourcePosition::new();
    let mut sanitized: Option<String> = None;
    let mut encoded_offset = decoded.initial_offset();

    {
        let decoded_text = decoded.text();
        let transport_prefix_length = decoded_text.len() - decoded.content().len();
        for (offset, character) in decoded_text.char_indices() {
            // Skip transport metadata in scalar/encoded advancement, but keep
            // it in any sanitized buffer so the reader cannot strip a second
            // leading U+FEFF as though it were the original transport BOM.
            if offset < transport_prefix_length {
                continue;
            }
            let encoded_width = decoded.encoded_width(character);
            if character == '\0' {
                let location = position.location();
                if mode == NoIntroDatabaseMode::ObservedCompatible {
                    let mut error =
                        parse_error("XML 1.0 forbids U+0000", "document", None, location);
                    if let Error::CatalogParse { excerpt, .. } = &mut error {
                        *excerpt = decoded.excerpt(encoded_offset, encoded_width);
                    }
                    return Err(error.into());
                }
                let prefix = decoded_text
                    .get(..offset)
                    .ok_or_else(|| xml_error("invalid recovery source boundary"))?;
                let view = sanitized.get_or_insert_with(|| {
                    let mut view = String::with_capacity(decoded_text.len());
                    view.push_str(prefix);
                    view
                });
                view.push('\u{fffd}');
            } else if let Some(view) = sanitized.as_mut() {
                view.push(character);
            }
            position.advance(character);
            encoded_offset += encoded_width;
        }
    }
    if sanitized.is_none() {
        decoded.discard_encoded_source();
    }
    let parse_xml = sanitized.as_deref().unwrap_or_else(|| decoded.text());
    let state = xml_reader::with_decoded_reader::<_, E>(
        parse_xml,
        decoded.bom_origin(),
        |reader, positions| parse_document(reader, positions, mode, begin, consume),
    )?;
    let recovery_source = if sanitized.is_some() {
        let (text, encoded, view, initial_offset) = decoded.into_parts();
        Some(RecoverySource {
            text,
            encoded,
            view,
            initial_offset,
        })
    } else {
        None
    };
    Ok(ValidatedNoIntroDatabase::after_eof(state, recovery_source))
}

fn parse_document<S, E, B, C>(
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    mode: NoIntroDatabaseMode,
    begin: B,
    consume: C,
) -> std::result::Result<S, E>
where
    E: From<Error>,
    B: FnOnce(NoIntroDatabaseDocument) -> std::result::Result<S, E>,
    C: FnMut(&mut S, DatabaseGame) -> std::result::Result<(), E>,
{
    let mut parser = Parser::new();
    let mut callbacks = Callbacks::new(begin, consume);
    let mut external_header = None;
    let mut envelope = None;
    let mut root_order = 0;
    let mut datafile_seen = false;

    loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        let empty_event = matches!(&event, Event::Empty(_));
        match event {
            Event::Decl(_) if !datafile_seen && envelope.is_none() => {}
            Event::Comment(_) | Event::PI(_) => {}
            Event::Text(text) if xml_whitespace_only(&text.xml10_content()) => {}
            Event::Start(start) | Event::Empty(start) => {
                let empty = empty_event;
                let info =
                    parser.start_info(reader, namespace, &start, 0, root_order, positions)?;
                root_order = root_order
                    .checked_add(1)
                    .ok_or_else(|| xml_error("root ordinal overflow"))?;
                if datafile_seen {
                    return Err(invalid(&info, "document", "unexpected second root element").into());
                }
                match info.local.as_str() {
                    "header"
                        if info.namespace.is_none()
                            && external_header.is_none()
                            && envelope.is_none() =>
                    {
                        external_header = Some(parse_header(
                            &mut parser,
                            reader,
                            positions,
                            &info,
                            empty,
                            1,
                        )?);
                    }
                    "datafile" if info.namespace.is_none() => {
                        datafile_seen = true;
                        let kind = if external_header.is_some() {
                            EnvelopeKind::SiblingHeaderDatafile
                        } else {
                            EnvelopeKind::SingleDatafile
                        };
                        envelope = Some(kind);
                        let context = DatafileContext {
                            envelope: kind,
                            mode,
                            location: info.location,
                        };
                        if let Some(header) = external_header.take() {
                            callbacks.start_if_needed(context, Some(header))?;
                        }
                        parse_datafile(
                            &mut parser,
                            reader,
                            positions,
                            &info,
                            empty,
                            context,
                            &mut callbacks,
                        )?;
                    }
                    _ => {
                        return Err(invalid(
                            &info,
                            "document",
                            "expected <header> or <datafile> root",
                        )
                        .into());
                    }
                }
            }
            Event::Eof => {
                if !datafile_seen {
                    return Err(xml_error("required <datafile> root is missing").into());
                }
                return callbacks
                    .state
                    .ok_or_else(|| xml_error("document callback was not started").into());
            }
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed.into()),
            Event::CData(_) => {
                return Err(xml_error("CDATA is not allowed in database-export XML").into());
            }
            Event::End(_) | Event::GeneralRef(_) => {
                return Err(xml_error("unexpected content outside the root elements").into());
            }
            Event::Text(_) => {
                return Err(xml_error("non-whitespace text outside root elements").into());
            }
            Event::Decl(_) => {
                return Err(xml_error("XML declaration must precede the root elements").into());
            }
        }
    }
}

fn parse_datafile<S, E, B, C>(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    root: &StartInfo,
    empty: bool,
    context: DatafileContext,
    callbacks: &mut Callbacks<S, B, C>,
) -> std::result::Result<(), E>
where
    E: From<Error>,
    B: FnOnce(NoIntroDatabaseDocument) -> std::result::Result<S, E>,
    C: FnMut(&mut S, DatabaseGame) -> std::result::Result<(), E>,
{
    Parser::validate_attributes(root, &[])?;
    if empty {
        callbacks.start_if_needed(context, None)?;
        return Ok(());
    }
    let mut child_order = 0;
    let mut game_order = 0;
    let mut header_seen = false;
    let mut saw_game = false;
    while let Some((child, child_empty)) = next_child(
        parser,
        reader,
        positions,
        root,
        "datafile",
        1,
        &mut child_order,
    )? {
        match child.local.as_str() {
            "header" if child.namespace.is_none() => {
                if context.envelope == EnvelopeKind::SiblingHeaderDatafile
                    || header_seen
                    || saw_game
                {
                    return Err(invalid(&child, "header", "duplicate or misplaced <header>").into());
                }
                header_seen = true;
                let header = parse_header(parser, reader, positions, &child, child_empty, 2)?;
                callbacks.start_if_needed(context, Some(header))?;
            }
            "game" if child.namespace.is_none() => {
                saw_game = true;
                callbacks.start_if_needed(context, None)?;
                let game = parse_game(
                    parser,
                    reader,
                    positions,
                    &child,
                    child_empty,
                    2,
                    game_order,
                )?;
                game_order = game_order
                    .checked_add(1)
                    .ok_or_else(|| invalid(&child, "game", "game ordinal overflow"))?;
                callbacks.consume_game(game)?;
            }
            _ => {
                return Err(invalid(
                    &child,
                    "datafile",
                    format!("unsupported child <{}>", child.local),
                )
                .into());
            }
        }
    }
    callbacks.start_if_needed(context, None)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct DatafileContext {
    envelope: EnvelopeKind,
    mode: NoIntroDatabaseMode,
    location: RecordLocation,
}

struct Callbacks<S, B, C> {
    state: Option<S>,
    begin: Option<B>,
    consume: C,
}

impl<S, B, C> Callbacks<S, B, C> {
    const fn new(begin: B, consume: C) -> Self {
        Self {
            state: None,
            begin: Some(begin),
            consume,
        }
    }

    fn start_if_needed<E>(
        &mut self,
        context: DatafileContext,
        header: Option<DatabaseHeader>,
    ) -> std::result::Result<(), E>
    where
        E: From<Error>,
        B: FnOnce(NoIntroDatabaseDocument) -> std::result::Result<S, E>,
    {
        if self.state.is_some() {
            return Ok(());
        }
        let callback = self
            .begin
            .take()
            .ok_or_else(|| xml_error("document callback was already used"))?;
        self.state = Some(callback(NoIntroDatabaseDocument {
            envelope: context.envelope,
            mode: context.mode,
            location: context.location,
            header,
        })?);
        Ok(())
    }

    fn consume_game<E>(&mut self, game: DatabaseGame) -> std::result::Result<(), E>
    where
        E: From<Error>,
        C: FnMut(&mut S, DatabaseGame) -> std::result::Result<(), E>,
    {
        let Some(state) = self.state.as_mut() else {
            return Err(xml_error("document callback did not initialize state").into());
        };
        (self.consume)(state, game)
    }
}

struct Parser {
    nodes: xml_reader::NodeBudget,
}

impl Parser {
    const fn new() -> Self {
        Self {
            nodes: xml_reader::NodeBudget::with_limit(xml_reader::MAX_MAME_XML_NODES),
        }
    }

    fn start_info(
        &mut self,
        reader: &XmlReader<'_>,
        namespace: Option<String>,
        start: &BytesStart<'_>,
        depth: usize,
        source_order: usize,
        positions: &mut PositionMap<'_>,
    ) -> Result<StartInfo> {
        self.nodes.include(depth)?;
        let location = positions.start_location(start)?;
        let mut attributes = Vec::new();
        for (ordinal, attribute) in start.attributes().enumerate() {
            let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
            let raw_name = attribute.key.as_ref();
            let name = if raw_name == "xmlns" || raw_name.starts_with("xmlns:") {
                None
            } else {
                let (namespace, local) = reader.resolver().resolve_attribute(attribute.key);
                let namespace = match namespace {
                    ResolveResult::Bound(value) => Some(value.as_ref().to_owned()),
                    ResolveResult::Unbound => None,
                    ResolveResult::Unknown(prefix) => {
                        return Err(Error::XmlValidation(format!(
                            "unbound XML namespace prefix {prefix:?}"
                        )));
                    }
                };
                Some(expanded_name(namespace.as_deref(), local.as_ref()))
            };
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|error| Error::XmlValidation(error.to_string()))?
                .into_owned();
            attributes.push(Attribute {
                name,
                value,
                ordinal,
                location: positions.attribute_location(&attribute)?,
            });
        }
        let opening_end = positions.event_location(reader.buffer_position())?;
        Ok(StartInfo {
            namespace,
            local: start.local_name().as_ref().to_owned(),
            source_order,
            location,
            opening_end,
            attributes,
        })
    }

    fn validate_attributes(info: &StartInfo, allowed: &[&str]) -> Result<()> {
        if info.attributes.len() > MAX_FIELDS_PER_ELEMENT {
            return Err(invalid(info, "element", "too many attributes"));
        }
        for attr in &info.attributes {
            let Some(name) = attr.name.as_deref() else {
                continue;
            };
            if !allowed.contains(&name) {
                return Err(invalid(
                    info,
                    "element",
                    format!("unsupported attribute {name:?}"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
struct Attribute {
    name: Option<String>,
    value: String,
    ordinal: usize,
    location: RecordLocation,
}
struct StartInfo {
    namespace: Option<String>,
    local: String,
    source_order: usize,
    location: RecordLocation,
    opening_end: RecordLocation,
    attributes: Vec<Attribute>,
}

impl StartInfo {
    fn attribute(&self, key: &str) -> Option<DeclaredText> {
        self.attributes
            .iter()
            .find(|attr| attr.name.as_deref() == Some(key))
            .map(|attr| DeclaredText {
                value: attr.value.clone(),
                source_order: attr.ordinal,
                location: attr.location,
            })
    }
}

fn parse_header(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<DatabaseHeader> {
    Parser::validate_attributes(info, &[])?;
    let mut header = DatabaseHeader {
        location: info.location,
        fields: Vec::new(),
    };
    if empty {
        return Ok(header);
    }
    let mut order = 0;
    while let Some((child, child_empty)) =
        next_child(parser, reader, positions, info, "header", depth, &mut order)?
    {
        let kind = match (child.namespace.as_deref(), child.local.as_str()) {
            (None, "author") => HeaderFieldKind::Author,
            (None, "piracy") => HeaderFieldKind::Piracy,
            (None, "trademarks") => HeaderFieldKind::Trademarks,
            (None, "url") => HeaderFieldKind::Url,
            (None, "version") => HeaderFieldKind::Version,
            _ => {
                return Err(invalid(
                    &child,
                    "header",
                    format!("unsupported header field <{}>", child.local),
                ));
            }
        };
        Parser::validate_attributes(&child, &[])?;
        let value = read_scalar(
            parser,
            reader,
            positions,
            &child,
            child_empty,
            depth + 1,
            "header field",
        )?;
        header.fields.push(HeaderField { kind, value });
    }
    Ok(header)
}

fn parse_game(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    list_order: usize,
) -> Result<DatabaseGame> {
    Parser::validate_attributes(info, &["name"])?;
    let name = info
        .attribute("name")
        .ok_or_else(|| invalid(info, "game", "required game name is missing"))?;
    let mut game = DatabaseGame {
        list_order,
        location: info.location,
        name,
        archives: Vec::new(),
        source_or_release: Vec::new(),
    };
    if empty {
        return Ok(game);
    }
    let mut order = 0;
    while let Some((child, child_empty)) =
        next_child(parser, reader, positions, info, "game", depth, &mut order)?
    {
        match (child.namespace.as_deref(), child.local.as_str()) {
            (None, "archive") => game.archives.push(parse_archive(
                parser,
                reader,
                positions,
                &child,
                child_empty,
                depth + 1,
            )?),
            (None, "source") => game
                .source_or_release
                .push(SourceOrRelease::Source(Box::new(parse_source(
                    parser,
                    reader,
                    positions,
                    &child,
                    child_empty,
                    depth + 1,
                )?))),
            (None, "release") => {
                game.source_or_release
                    .push(SourceOrRelease::Release(Box::new(parse_release(
                        parser,
                        reader,
                        positions,
                        &child,
                        child_empty,
                        depth + 1,
                    )?)));
            }
            _ => {
                return Err(invalid(
                    &child,
                    "game",
                    format!("unsupported game child <{}>", child.local),
                ));
            }
        }
    }
    Ok(game)
}

fn parse_archive(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<ArchiveDescription> {
    const FIELDS: &[&str] = &[
        "additional",
        "adult",
        "aftermarket",
        "alt",
        "bios",
        "categories",
        "complete",
        "dat",
        "datter_note",
        "description",
        "devstatus",
        "gameid1",
        "gameid2",
        "langchecked",
        "languages",
        "licensed",
        "listed",
        "mergename",
        "name",
        "name_alt",
        "number",
        "physical",
        "region",
        "regparent",
        "showlang",
        "special1",
        "special2",
        "sticky_note",
        "version1",
        "version2",
        "clone",
        "mergeof",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let clone = info.attribute("clone").map(|value| {
        if value.value == "P" {
            ArchiveClone::ParentMarker(value)
        } else {
            ArchiveClone::OtherValue(value)
        }
    });
    let archive = ArchiveDescription {
        source_order: info.source_order,
        location: info.location,
        additional: info.attribute("additional"),
        adult: info.attribute("adult"),
        aftermarket: info.attribute("aftermarket"),
        alt: info.attribute("alt"),
        bios: info.attribute("bios"),
        categories: info.attribute("categories"),
        complete: info.attribute("complete"),
        dat: info.attribute("dat"),
        datter_note: info.attribute("datter_note"),
        description: info.attribute("description"),
        devstatus: info.attribute("devstatus"),
        gameid1: info.attribute("gameid1"),
        gameid2: info.attribute("gameid2"),
        langchecked: info.attribute("langchecked"),
        languages: info.attribute("languages"),
        licensed: info.attribute("licensed"),
        listed: info.attribute("listed"),
        mergename: info.attribute("mergename"),
        name: info.attribute("name"),
        name_alt: info.attribute("name_alt"),
        number: info.attribute("number"),
        physical: info.attribute("physical"),
        region: info.attribute("region"),
        regparent: info.attribute("regparent"),
        showlang: info.attribute("showlang"),
        special1: info.attribute("special1"),
        special2: info.attribute("special2"),
        sticky_note: info.attribute("sticky_note"),
        version1: info.attribute("version1"),
        version2: info.attribute("version2"),
        clone,
        mergeof: info.attribute("mergeof"),
    };
    consume_empty(parser, reader, positions, info, empty, depth, "archive")?;
    Ok(archive)
}

fn parse_source(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<DumpSource> {
    Parser::validate_attributes(info, &[])?;
    let mut source = DumpSource {
        source_order: info.source_order,
        location: info.location,
        details: None,
        serials: None,
        files: Vec::new(),
    };
    parse_owner_children(
        parser,
        reader,
        positions,
        info,
        empty,
        depth,
        Owner::Source(&mut source),
    )?;
    Ok(source)
}

fn parse_release(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<DatabaseRelease> {
    Parser::validate_attributes(info, &[])?;
    let mut release = DatabaseRelease {
        source_order: info.source_order,
        location: info.location,
        details: None,
        serials: None,
        files: Vec::new(),
    };
    parse_owner_children(
        parser,
        reader,
        positions,
        info,
        empty,
        depth,
        Owner::Release(&mut release),
    )?;
    Ok(release)
}

enum Owner<'a> {
    Source(&'a mut DumpSource),
    Release(&'a mut DatabaseRelease),
}

fn parse_owner_children(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    mut owner: Owner<'_>,
) -> Result<()> {
    if empty {
        return Ok(());
    }
    let mut order = 0;
    while let Some((child, child_empty)) = next_child(
        parser,
        reader,
        positions,
        info,
        "source/release",
        depth,
        &mut order,
    )? {
        if child.namespace.is_some() {
            return Err(invalid(
                &child,
                "source/release",
                "namespaced child elements are unsupported",
            ));
        }
        match child.local.as_str() {
            "details" => match &mut owner {
                Owner::Source(source) if source.details.is_none() => {
                    source.details = Some(parse_source_details(
                        parser,
                        reader,
                        positions,
                        &child,
                        child_empty,
                        depth + 1,
                    )?);
                }
                Owner::Release(release) if release.details.is_none() => {
                    release.details = Some(parse_release_details(
                        parser,
                        reader,
                        positions,
                        &child,
                        child_empty,
                        depth + 1,
                    )?);
                }
                _ => return Err(invalid(&child, "details", "duplicate singleton <details>")),
            },
            "serials" => match &mut owner {
                Owner::Source(source) if source.serials.is_none() => {
                    source.serials = Some(parse_source_serials(
                        parser,
                        reader,
                        positions,
                        &child,
                        child_empty,
                        depth + 1,
                    )?);
                }
                Owner::Release(release) if release.serials.is_none() => {
                    release.serials = Some(parse_release_serials(
                        parser,
                        reader,
                        positions,
                        &child,
                        child_empty,
                        depth + 1,
                    )?);
                }
                _ => return Err(invalid(&child, "serials", "duplicate singleton <serials>")),
            },
            "file" => match &mut owner {
                Owner::Source(source) => source.files.push(parse_source_file(
                    parser,
                    reader,
                    positions,
                    &child,
                    child_empty,
                    depth + 1,
                )?),
                Owner::Release(release) => release.files.push(parse_release_file(
                    parser,
                    reader,
                    positions,
                    &child,
                    child_empty,
                    depth + 1,
                )?),
            },
            _ => {
                return Err(invalid(
                    &child,
                    "source/release",
                    format!("unsupported child <{}>", child.local),
                ));
            }
        }
    }
    Ok(())
}

macro_rules! text_attrs {
    ($info:expr; $($field:ident),+ $(,)?) => {
        ($( $info.attribute(stringify!($field)), )+)
    };
}

fn parse_source_details(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<SourceDetails> {
    const FIELDS: &[&str] = &[
        "comment1",
        "comment2",
        "d_date",
        "d_date_info",
        "dumper",
        "id",
        "link1",
        "link2",
        "link3",
        "media_title",
        "nodump",
        "origin",
        "originalformat",
        "project",
        "r_date",
        "r_date_info",
        "region",
        "rominfo",
        "section",
        "tool",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (
        comment1,
        comment2,
        d_date,
        d_date_info,
        dumper,
        id,
        link1,
        link2,
        link3,
        media_title,
        nodump,
        origin,
        originalformat,
        project,
        r_date,
        r_date_info,
        region,
        rominfo,
        section,
        tool,
    ) = text_attrs!(info; comment1, comment2, d_date, d_date_info, dumper, id, link1, link2, link3, media_title, nodump, origin, originalformat, project, r_date, r_date_info, region, rominfo, section, tool);
    consume_empty(parser, reader, positions, info, empty, depth, "details")?;
    Ok(SourceDetails {
        source_order: info.source_order,
        location: info.location,
        opening_end: info.opening_end,
        comment1,
        comment2,
        d_date,
        d_date_info,
        dumper,
        id,
        link1,
        link2,
        link3,
        media_title,
        nodump,
        origin,
        originalformat,
        project,
        r_date,
        r_date_info,
        region,
        rominfo,
        section,
        tool,
    })
}

fn parse_release_details(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<ReleaseDetails> {
    const FIELDS: &[&str] = &[
        "archivename",
        "category",
        "comment",
        "date",
        "dirname",
        "group",
        "id",
        "nfo_crc32",
        "nfo_size",
        "nfocrc",
        "nfoname",
        "nfosize",
        "origin",
        "originalformat",
        "region",
        "rominfo",
        "tool",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (
        archivename,
        category,
        comment,
        date,
        dirname,
        group,
        id,
        nfoname,
        origin,
        originalformat,
        region,
        rominfo,
        tool,
    ) = text_attrs!(info; archivename, category, comment, date, dirname, group, id, nfoname, origin, originalformat, region, rominfo, tool);
    consume_empty(parser, reader, positions, info, empty, depth, "details")?;
    Ok(ReleaseDetails {
        source_order: info.source_order,
        location: info.location,
        opening_end: info.opening_end,
        archivename,
        category,
        comment,
        date,
        dirname,
        group,
        id,
        nfo_crc32: digest(info, "nfo_crc32", 4),
        nfo_size: info.attribute("nfo_size"),
        nfocrc: digest(info, "nfocrc", 4),
        nfoname,
        nfosize: info.attribute("nfosize"),
        origin,
        originalformat,
        region,
        rominfo,
        tool,
    })
}

fn parse_source_serials(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<SourceSerials> {
    const FIELDS: &[&str] = &[
        "box_barcode",
        "box_serial",
        "chip_serial",
        "digital_serial1",
        "digital_serial2",
        "lockout_serial",
        "media_serial1",
        "media_serial2",
        "media_serial3",
        "mediastamp",
        "pcb_serial",
        "romchip_serial1",
        "romchip_serial2",
        "savechip_serial",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (
        box_barcode,
        box_serial,
        chip_serial,
        digital_serial1,
        digital_serial2,
        lockout_serial,
        media_serial1,
        media_serial2,
        media_serial3,
        mediastamp,
        pcb_serial,
        romchip_serial1,
        romchip_serial2,
        savechip_serial,
    ) = text_attrs!(info; box_barcode, box_serial, chip_serial, digital_serial1, digital_serial2, lockout_serial, media_serial1, media_serial2, media_serial3, mediastamp, pcb_serial, romchip_serial1, romchip_serial2, savechip_serial);
    consume_empty(parser, reader, positions, info, empty, depth, "serials")?;
    Ok(SourceSerials {
        source_order: info.source_order,
        location: info.location,
        box_barcode,
        box_serial,
        chip_serial,
        digital_serial1,
        digital_serial2,
        lockout_serial,
        media_serial1,
        media_serial2,
        media_serial3,
        mediastamp,
        pcb_serial,
        romchip_serial1,
        romchip_serial2,
        savechip_serial,
    })
}

fn parse_release_serials(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<ReleaseSerials> {
    const FIELDS: &[&str] = &[
        "box_barcode",
        "box_serial",
        "media_serial1",
        "mediastamp",
        "pcb_serial",
        "romchip_serial1",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (box_barcode, box_serial, media_serial1, mediastamp, pcb_serial, romchip_serial1) = text_attrs!(info; box_barcode, box_serial, media_serial1, mediastamp, pcb_serial, romchip_serial1);
    consume_empty(parser, reader, positions, info, empty, depth, "serials")?;
    Ok(ReleaseSerials {
        source_order: info.source_order,
        location: info.location,
        box_barcode,
        box_serial,
        media_serial1,
        mediastamp,
        pcb_serial,
        romchip_serial1,
    })
}

fn parse_source_file(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<SourceFile> {
    const FIELDS: &[&str] = &[
        "bad",
        "crc32",
        "date",
        "extension",
        "filter",
        "forcename",
        "forcescenename",
        "format",
        "header",
        "id",
        "item",
        "md5",
        "mia",
        "note",
        "origin_sha256",
        "origin_size",
        "serial",
        "sha1",
        "sha256",
        "size",
        "unique",
        "update_type",
        "version",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (
        bad,
        date,
        extension,
        filter,
        forcename,
        forcescenename,
        format,
        header,
        id,
        item,
        mia,
        note,
        origin_size,
        serial,
        size,
        unique,
        update_type,
        version,
    ) = text_attrs!(info; bad, date, extension, filter, forcename, forcescenename, format, header, id, item, mia, note, origin_size, serial, size, unique, update_type, version);
    let file = SourceFile {
        source_order: info.source_order,
        location: info.location,
        bad,
        crc32: digest(info, "crc32", 4),
        date,
        extension,
        filter,
        forcename,
        forcescenename,
        format,
        header,
        id,
        item,
        md5: digest(info, "md5", 16),
        mia,
        note,
        origin_sha256: digest(info, "origin_sha256", 32),
        origin_size,
        serial,
        sha1: digest(info, "sha1", 20),
        sha256: digest(info, "sha256", 32),
        size,
        unique,
        update_type,
        version,
    };
    consume_empty(parser, reader, positions, info, empty, depth, "file")?;
    Ok(file)
}

fn parse_release_file(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<ReleaseFile> {
    const FIELDS: &[&str] = &[
        "bad",
        "crc32",
        "extension",
        "forcename",
        "forcescenename",
        "format",
        "header",
        "id",
        "item",
        "md5",
        "note",
        "serial",
        "sha1",
        "sha256",
        "size",
        "update_type",
        "version",
    ];
    Parser::validate_attributes(info, FIELDS)?;
    let (
        bad,
        extension,
        forcename,
        forcescenename,
        format,
        header,
        id,
        item,
        note,
        serial,
        size,
        update_type,
        version,
    ) = text_attrs!(info; bad, extension, forcename, forcescenename, format, header, id, item, note, serial, size, update_type, version);
    let file = ReleaseFile {
        source_order: info.source_order,
        location: info.location,
        bad,
        crc32: digest(info, "crc32", 4),
        extension,
        forcename,
        forcescenename,
        format,
        header,
        id,
        item,
        md5: digest(info, "md5", 16),
        note,
        serial,
        sha1: digest(info, "sha1", 20),
        sha256: digest(info, "sha256", 32),
        size,
        update_type,
        version,
    };
    consume_empty(parser, reader, positions, info, empty, depth, "file")?;
    Ok(file)
}

fn digest(info: &StartInfo, name: &str, byte_len: usize) -> Option<DatabaseDigest> {
    info.attribute(name).map(|source| {
        let value = decode_hex(&source.value).filter(|value| value.len() == byte_len);
        DatabaseDigest { source, value }
    })
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in pairs {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

const fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn consume_empty(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    owner: &str,
) -> Result<()> {
    if empty {
        return Ok(());
    }
    loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        match event {
            Event::End(_) => return Ok(()),
            Event::Text(text) if xml_whitespace_only(&text.xml10_content()) => {}
            Event::Comment(_) | Event::PI(_) => {}
            Event::CData(_) => return Err(invalid(info, owner, "CDATA is not allowed")),
            Event::Start(start) | Event::Empty(start) => {
                parser.start_info(reader, namespace, &start, depth, 0, positions)?;
                return Err(invalid(
                    info,
                    owner,
                    "elements may not contain child elements",
                ));
            }
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            Event::Eof => return Err(invalid(info, owner, "unexpected end of XML")),
            Event::Decl(_) => {
                return Err(invalid(
                    info,
                    owner,
                    "XML declaration is not allowed inside an element",
                ));
            }
            Event::Text(_) | Event::GeneralRef(_) => {
                return Err(invalid(info, owner, "unexpected character content"));
            }
        }
    }
}

fn read_scalar(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    owner: &str,
) -> Result<DeclaredText> {
    if empty {
        return Ok(DeclaredText {
            value: String::new(),
            source_order: info.source_order,
            location: info.location,
        });
    }
    let mut value = String::new();
    loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        match event {
            Event::End(_) => {
                return Ok(DeclaredText {
                    value,
                    source_order: info.source_order,
                    location: info.location,
                });
            }
            Event::Text(text) => append_scalar(&mut value, &text.xml10_content(), info, owner)?,
            Event::GeneralRef(reference) => {
                append_scalar(&mut value, &resolve_reference(&reference)?, info, owner)?;
            }
            Event::Comment(_) | Event::PI(_) => {}
            Event::CData(text) => append_scalar(&mut value, &text.xml10_content(), info, owner)?,
            Event::Start(start) | Event::Empty(start) => {
                parser.start_info(reader, namespace, &start, depth, 0, positions)?;
                return Err(invalid(
                    info,
                    owner,
                    "scalar fields cannot contain child elements",
                ));
            }
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            Event::Eof => return Err(invalid(info, owner, "unexpected end of XML")),
            Event::Decl(_) => {
                return Err(invalid(
                    info,
                    owner,
                    "XML declaration is not allowed inside a scalar",
                ));
            }
        }
    }
}

fn append_scalar(value: &mut String, text: &str, info: &StartInfo, owner: &str) -> Result<()> {
    if value
        .len()
        .checked_add(text.len())
        .is_none_or(|length| length > xml_reader::MAX_TEXT_BYTES)
    {
        return Err(invalid(info, owner, "decoded scalar text limit exceeded"));
    }
    value.push_str(text);
    Ok(())
}

fn next_child(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    owner: &StartInfo,
    owner_name: &str,
    depth: usize,
    child_order: &mut usize,
) -> Result<Option<(StartInfo, bool)>> {
    loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        match event {
            Event::Start(start) => {
                let order = *child_order;
                *child_order = order
                    .checked_add(1)
                    .ok_or_else(|| invalid(owner, owner_name, "child element ordinal overflow"))?;
                return parser
                    .start_info(reader, namespace, &start, depth, order, positions)
                    .map(|info| Some((info, false)));
            }
            Event::Empty(start) => {
                let order = *child_order;
                *child_order = order
                    .checked_add(1)
                    .ok_or_else(|| invalid(owner, owner_name, "child element ordinal overflow"))?;
                return parser
                    .start_info(reader, namespace, &start, depth, order, positions)
                    .map(|info| Some((info, true)));
            }
            Event::Text(text) if xml_whitespace_only(&text.xml10_content()) => {}
            Event::Comment(_) | Event::PI(_) => {}
            Event::End(_) => return Ok(None),
            Event::CData(_) => return Err(invalid(owner, owner_name, "CDATA is not allowed")),
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            Event::Eof => return Err(invalid(owner, owner_name, "unexpected end of XML")),
            Event::Text(_) | Event::GeneralRef(_) => {
                return Err(invalid(
                    owner,
                    owner_name,
                    "mixed character content is not allowed",
                ));
            }
            Event::Decl(_) => {
                return Err(invalid(
                    owner,
                    owner_name,
                    "XML declaration is not allowed inside an element",
                ));
            }
        }
    }
}

fn resolve_reference(reference: &quick_xml::events::BytesRef<'_>) -> Result<String> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|error| Error::XmlValidation(error.to_string()))?
    {
        return Ok(character.to_string());
    }
    quick_xml::escape::resolve_predefined_entity(reference.as_ref())
        .map(str::to_owned)
        .ok_or(Error::XmlEntityNotAllowed)
}

fn parse_error(
    message: impl Into<String>,
    kind: &str,
    name: Option<String>,
    location: RecordLocation,
) -> Error {
    Error::CatalogParse {
        message: message.into(),
        record_kind: Some(kind.to_owned()),
        record_name: name,
        line: Some(location.line),
        column: Some(location.column),
        excerpt: None,
        coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
    }
}

fn invalid(info: &StartInfo, kind: &str, message: impl Into<String>) -> Error {
    parse_error(
        message,
        kind,
        info.attribute("name").map(|value| value.value),
        info.location,
    )
}

fn xml_error(message: impl Into<String>) -> Error {
    Error::CatalogParse {
        message: message.into(),
        record_kind: Some("document".to_owned()),
        record_name: None,
        line: None,
        column: None,
        excerpt: None,
        coordinates: None,
    }
}

fn expanded_name(namespace: Option<&str>, local: &str) -> String {
    namespace.map_or_else(
        || local.to_owned(),
        |namespace| format!("{{{namespace}}}{local}"),
    )
}

fn xml_whitespace_only(value: &str) -> bool {
    value
        .chars()
        .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n'))
}
