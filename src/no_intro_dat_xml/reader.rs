use std::collections::{HashMap, HashSet};

use quick_xml::{
    events::{BytesStart, Event},
    name::{QName, ResolveResult},
};

use crate::{
    Error, Result,
    logiqx::RecordLocation,
    no_intro_dat_xml::{
        ClrMameProOptions, Document, Game, Header, NoIntroDatMode, Release, Rom, RomCenterOptions,
    },
    xml_reader::{self, DeclaredText, NodeBudget, PositionMap, XmlReader},
};

const XML_SCHEMA: &str = "http://www.w3.org/2001/XMLSchema";
const XSI_SCHEMA_LOCATION: &str = "{http://www.w3.org/2001/XMLSchema-instance}schemaLocation";
const XSI_NO_NAMESPACE_SCHEMA_LOCATION: &str =
    "{http://www.w3.org/2001/XMLSchema-instance}noNamespaceSchemaLocation";
const XSI_NIL: &str = "{http://www.w3.org/2001/XMLSchema-instance}nil";
const XSI_TYPE: &str = "{http://www.w3.org/2001/XMLSchema-instance}type";

#[cfg(test)]
mod tests;

pub struct ValidatedNoIntroDat<S> {
    inner: S,
}

impl<S> ValidatedNoIntroDat<S> {
    const fn after_eof(inner: S) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> S {
        self.inner
    }
}

pub fn read_with<S, E: From<Error>>(
    bytes: &[u8],
    mode: NoIntroDatMode,
    begin: impl FnOnce(Document) -> std::result::Result<S, E>,
    mut consume: impl FnMut(&mut S, Game) -> std::result::Result<(), E>,
) -> std::result::Result<ValidatedNoIntroDat<S>, E> {
    xml_reader::with_reader::<_, E>(bytes, |reader, positions| -> std::result::Result<_, E> {
        let mut parser = Parser::new(mode);
        let mut state = None;
        let mut saw_root = false;
        let mut begin = Some(begin);

        loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::Start(start) => {
                    if saw_root {
                        return Err(xml_error("more than one document element").into());
                    }
                    saw_root = true;
                    let root = parser.start_info(reader, namespace, &start, 0, 0, positions)?;
                    if !root.is_unqualified("datafile") {
                        return Err(
                            invalid(&root, "document", "expected unqualified <datafile>").into(),
                        );
                    }
                    let begin = begin
                        .take()
                        .ok_or_else(|| xml_error("document callback was already used"))?;
                    let document_state =
                        parse_datafile(&mut parser, reader, positions, &root, begin, &mut consume)?;
                    state = Some(document_state);
                }
                Event::Empty(start) => {
                    if saw_root {
                        return Err(xml_error("more than one document element").into());
                    }
                    let root = parser.start_info(reader, namespace, &start, 0, 0, positions)?;
                    if !root.is_unqualified("datafile") {
                        return Err(
                            invalid(&root, "document", "expected unqualified <datafile>").into(),
                        );
                    }
                    return Err(invalid(&root, "datafile", "required <header> is missing").into());
                }
                Event::Text(text) => {
                    if !xml_whitespace_only(&text.xml10_content()) {
                        return Err(xml_error("text outside the document element").into());
                    }
                }
                Event::CData(_) => {
                    return Err(xml_error("CDATA outside the document element").into());
                }
                Event::DocType(_) => return Err(Error::XmlEntityNotAllowed.into()),
                Event::Eof => {
                    let Some(state) = state else {
                        return Err(xml_error("document element <datafile> is missing").into());
                    };
                    parser.finish()?;
                    return Ok(ValidatedNoIntroDat::after_eof(state));
                }
                Event::End(_) | Event::GeneralRef(_) => {
                    return Err(xml_error("unexpected content outside the document element").into());
                }
                Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
            }
        }
    })
}

struct Parser {
    mode: NoIntroDatMode,
    nodes: NodeBudget,
    ids: HashSet<String>,
    idrefs: HashMap<String, RecordLocation>,
}

impl Parser {
    fn new(mode: NoIntroDatMode) -> Self {
        Self {
            mode,
            nodes: NodeBudget::with_limit(xml_reader::MAX_MAME_XML_NODES),
            ids: HashSet::new(),
            idrefs: HashMap::new(),
        }
    }

    const fn strict(&self) -> bool {
        self.mode.is_strict()
    }

    fn finish(&self) -> Result<()> {
        if let Some((reference, location)) = self.idrefs.iter().next() {
            return Err(Error::CatalogParse {
                message: format!("xs:IDREF {reference:?} has no matching xs:ID"),
                record_kind: Some("IDREF".to_owned()),
                record_name: None,
                line: Some(location.line),
                column: Some(location.column),
            });
        }
        Ok(())
    }

    fn register_identity(&mut self, kind: SimpleType, value: &str, info: &StartInfo) -> Result<()> {
        if !self.strict() {
            return Ok(());
        }
        let value = collapse_xml_whitespace(value);
        match kind {
            SimpleType::Id => {
                if !self.ids.insert(value.clone()) {
                    return Err(invalid(
                        info,
                        "simple element",
                        format!("duplicate xs:ID {value:?}"),
                    ));
                }
                self.idrefs.remove(&value);
            }
            SimpleType::IdRef if !self.ids.contains(&value) => {
                self.idrefs.entry(value).or_insert(info.location);
            }
            _ => {}
        }
        Ok(())
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
        let location = positions.start_location(start);
        let local = start.local_name().as_ref().to_owned();
        let mut attributes = Vec::new();

        for (ordinal, attribute) in start.attributes().enumerate() {
            let attribute = attribute.map_err(|error| Error::XmlValidation(error.to_string()))?;
            let raw_name = attribute.key.as_ref();
            let is_namespace_declaration = raw_name == "xmlns" || raw_name.starts_with("xmlns:");
            let name = if is_namespace_declaration {
                None
            } else {
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
            });
        }

        let info = StartInfo {
            namespace,
            local,
            source_order,
            location,
            attributes,
        };
        validate_schema_hints(self.mode, &info)?;
        Ok(info)
    }

    fn validate_attributes(
        &self,
        reader: &XmlReader<'_>,
        info: &StartInfo,
        allowed: &[&str],
        simple_type: Option<SimpleType>,
    ) -> Result<Option<SimpleType>> {
        let mut effective_type = simple_type;

        for attribute in &info.attributes {
            let Some(name) = attribute.name.as_deref() else {
                // Namespace declarations participate in the source ordinal but
                // are not schema attributes.
                continue;
            };
            match name {
                XSI_SCHEMA_LOCATION | XSI_NO_NAMESPACE_SCHEMA_LOCATION => {}
                XSI_NIL => {
                    let value = collapse_xml_whitespace(&attribute.value);
                    if self.strict() {
                        return Err(invalid(
                            info,
                            "element",
                            "xsi:nil is invalid for a non-nillable No-Intro declaration",
                        ));
                    }
                    match value.as_str() {
                        "false" | "0" => {}
                        "true" | "1" => {
                            return Err(invalid(
                                info,
                                "element",
                                "xsi:nil=true is invalid for a non-nillable No-Intro declaration",
                            ));
                        }
                        _ => {
                            return Err(invalid(
                                info,
                                "element",
                                "xsi:nil must have an XML Schema boolean value",
                            ));
                        }
                    }
                }
                XSI_TYPE => {
                    let Some(expected) = simple_type else {
                        return Err(invalid(
                            info,
                            "element",
                            "xsi:type cannot replace this anonymous No-Intro complex type",
                        ));
                    };
                    effective_type =
                        Some(resolve_narrowed_type(reader, info, attribute, expected)?);
                }
                _ if allowed.contains(&name) => {}
                _ if self.strict() => {
                    return Err(invalid(
                        info,
                        "element",
                        format!("attribute {name:?} is not declared by the selected schema"),
                    ));
                }
                _ => {}
            }
        }

        Ok(effective_type)
    }
}

#[derive(Clone, Debug)]
struct Attribute {
    name: Option<String>,
    value: String,
    ordinal: usize,
}

#[derive(Debug)]
struct StartInfo {
    namespace: Option<String>,
    local: String,
    source_order: usize,
    location: RecordLocation,
    attributes: Vec<Attribute>,
}

impl StartInfo {
    fn is_unqualified(&self, name: &str) -> bool {
        self.namespace.is_none() && self.local == name
    }

    fn attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.as_deref() == Some(name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimpleType {
    String,
    NormalizedString,
    Token,
    Language,
    Name,
    NcName,
    NmToken,
    Id,
    IdRef,
    Entity,
    Int,
    Short,
    Byte,
}

fn parse_datafile<S, E: From<Error>>(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    root: &StartInfo,
    begin: impl FnOnce(Document) -> std::result::Result<S, E>,
    consume: &mut impl FnMut(&mut S, Game) -> std::result::Result<(), E>,
) -> std::result::Result<S, E> {
    let schema_location = schema_location(root);
    parser.validate_attributes(reader, root, &[], None)?;
    let mut root_child_order = 0;
    let mut list_order = 0;
    let mut saw_header = false;
    let mut saw_game = false;
    let mut strict_last_rank = None;
    let mut state = None;
    let mut begin = Some(begin);

    while let Some((child, empty)) = next_child(
        parser,
        reader,
        positions,
        root,
        "datafile",
        1,
        &mut root_child_order,
    )? {
        let root_rank = if child.is_unqualified("header") {
            Some(0)
        } else if child.is_unqualified("game") {
            Some(1)
        } else {
            None
        };
        if parser.strict() {
            let Some(rank) = root_rank else {
                return Err(invalid(
                    &child,
                    "datafile",
                    format!("unexpected element <{}>", child.local),
                )
                .into());
            };
            check_order(&mut strict_last_rank, rank, &child, "datafile")?;
        }

        if child.is_unqualified("header") {
            if saw_header {
                return Err(invalid(&child, "header", "duplicate <header>").into());
            }
            if saw_game {
                return Err(invalid(&child, "header", "<header> must precede every <game>").into());
            }
            saw_header = true;
            let begin = begin
                .take()
                .ok_or_else(|| invalid(&child, "header", "header callback was already used"))?;
            let header = parse_header(parser, reader, positions, &child, empty, 2)?;
            let document = Document {
                schema_location: schema_location.clone(),
                location: root.location,
                header,
            };
            state = Some(begin(document)?);
        } else if child.is_unqualified("game") {
            if !saw_header {
                return Err(
                    invalid(&child, "game", "required <header> must precede <game>").into(),
                );
            }
            saw_game = true;
            let game = parse_game(parser, reader, positions, &child, empty, 2, list_order)?;
            list_order = list_order
                .checked_add(1)
                .ok_or_else(|| xml_error("game occurrence ordinal overflow"))?;
            let Some(state) = state.as_mut() else {
                return Err(xml_error("header callback did not initialize parser state").into());
            };
            consume(state, game)?;
        } else {
            skip_unknown(parser, reader, positions, &child, empty, 1)?;
        }
    }

    state.ok_or_else(|| invalid(root, "datafile", "required <header> is missing").into())
}

fn schema_location(root: &StartInfo) -> Option<String> {
    root.attribute(XSI_SCHEMA_LOCATION)
        .or_else(|| root.attribute(XSI_NO_NAMESPACE_SCHEMA_LOCATION))
        .map(|attribute| attribute.value.clone())
}

fn validate_schema_hints(mode: NoIntroDatMode, info: &StartInfo) -> Result<()> {
    let schema_pairs = info
        .attribute(XSI_SCHEMA_LOCATION)
        .map(|attribute| attribute.value.as_str());
    let no_namespace = info
        .attribute(XSI_NO_NAMESPACE_SCHEMA_LOCATION)
        .map(|attribute| attribute.value.as_str());
    let mut references = Vec::new();

    if let Some(value) = schema_pairs {
        let tokens = xml_tokens(value);
        if !tokens.len().is_multiple_of(2) {
            return Err(invalid(
                info,
                "element",
                "xsi:schemaLocation must contain namespace/location pairs",
            ));
        }
        references.extend(tokens.into_iter().skip(1).step_by(2).map(str::to_owned));
    }
    if let Some(value) = no_namespace {
        let location = collapse_xml_whitespace(value);
        if !location.is_empty() {
            references.push(location);
        }
    }

    for reference in &references {
        if let Some(revision) = schema_revision(reference)
            && revision != mode.revision()
        {
            return Err(invalid(
                info,
                "element",
                format!(
                    "schema location references No-Intro v{revision}, but mode selects v{}",
                    mode.revision()
                ),
            ));
        }
    }

    Ok(())
}

fn schema_revision(location: &str) -> Option<u8> {
    let without_suffix = location.split(['?', '#']).next()?;
    let filename = without_suffix.rsplit(['/', '\\']).next()?;
    match filename {
        "schema_nointro_datfile_v3.xsd" => Some(3),
        "schema_nointro_datfile_v4.xsd" => Some(4),
        _ => None,
    }
}

fn parse_header(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<Header> {
    parser.validate_attributes(reader, info, &[], None)?;
    let mut header = Header {
        location: info.location,
        source_order: info.source_order,
        id: None,
        name: None,
        description: None,
        version: None,
        date: None,
        author: None,
        homepage: None,
        url: None,
        trademarks: None,
        piracy: None,
        subset: None,
        comment: None,
        clrmamepro: None,
        romcenter: None,
    };
    if empty {
        return Err(invalid(
            info,
            "header",
            "required header fields are missing",
        ));
    }

    let mut child_order = 0;
    let mut strict_last_rank = None;
    while let Some((child, child_empty)) = next_child(
        parser,
        reader,
        positions,
        info,
        "header",
        depth,
        &mut child_order,
    )? {
        let rank = header_rank(parser.mode, &child);
        if parser.strict() {
            let Some(rank) = rank else {
                return Err(invalid(
                    &child,
                    "header",
                    format!("unexpected element <{}>", child.local),
                ));
            };
            check_order(&mut strict_last_rank, rank, &child, "header")?;
        }

        if child.namespace.is_some() {
            skip_unknown(parser, reader, positions, &child, child_empty, depth)?;
            continue;
        }
        parse_header_child(
            parser,
            reader,
            positions,
            &mut header,
            &child,
            child_empty,
            depth,
        )?;
    }

    require_header_fields(parser, &header, info)?;
    Ok(header)
}

fn parse_header_child(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    header: &mut Header,
    child: &StartInfo,
    child_empty: bool,
    depth: usize,
) -> Result<()> {
    let field = match child.local.as_str() {
        "id" => Some((&mut header.id, SimpleType::Int)),
        "name" => Some((&mut header.name, SimpleType::String)),
        "description" => Some((&mut header.description, SimpleType::String)),
        "version" => Some((&mut header.version, SimpleType::String)),
        "date" => Some((&mut header.date, SimpleType::String)),
        "author" => Some((&mut header.author, SimpleType::String)),
        "homepage" => Some((&mut header.homepage, SimpleType::String)),
        "url" => Some((&mut header.url, SimpleType::String)),
        "trademarks" if parser.mode.revision() == 4 || !parser.strict() => {
            Some((&mut header.trademarks, SimpleType::String))
        }
        "piracy" if parser.mode.revision() == 4 || !parser.strict() => {
            Some((&mut header.piracy, SimpleType::String))
        }
        "subset" => Some((&mut header.subset, SimpleType::String)),
        "comment" if !parser.strict() => Some((&mut header.comment, SimpleType::String)),
        _ => None,
    };

    if let Some((target, simple_type)) = field {
        let value = parse_simple_child(
            parser,
            reader,
            positions,
            child,
            child_empty,
            depth + 1,
            simple_type,
        )?;
        return set_once(target, value, child, "header field");
    }

    match child.local.as_str() {
        "clrmamepro" => {
            let options =
                parse_clrmamepro(parser, reader, positions, child, child_empty, depth + 1)?;
            if header.clrmamepro.replace(options).is_some() {
                return Err(invalid(child, "header field", "duplicate <clrmamepro>"));
            }
        }
        "romcenter" => {
            let options =
                parse_romcenter(parser, reader, positions, child, child_empty, depth + 1)?;
            if header.romcenter.replace(options).is_some() {
                return Err(invalid(child, "header field", "duplicate <romcenter>"));
            }
        }
        _ => skip_unknown(parser, reader, positions, child, child_empty, depth)?,
    }
    Ok(())
}

fn header_rank(mode: NoIntroDatMode, child: &StartInfo) -> Option<usize> {
    if child.namespace.is_some() {
        return None;
    }
    let names_v3 = [
        "id",
        "name",
        "description",
        "version",
        "date",
        "author",
        "homepage",
        "url",
        "subset",
        "clrmamepro",
        "romcenter",
    ];
    let names_v4 = [
        "id",
        "name",
        "description",
        "version",
        "date",
        "author",
        "homepage",
        "url",
        "trademarks",
        "piracy",
        "subset",
        "clrmamepro",
        "romcenter",
    ];
    let names = if mode.revision() == 3 {
        &names_v3[..]
    } else {
        &names_v4[..]
    };
    names.iter().position(|name| *name == child.local)
}

fn require_header_fields(parser: &Parser, header: &Header, info: &StartInfo) -> Result<()> {
    let fields = [
        ("id", header.id.is_some()),
        ("name", header.name.is_some()),
        ("description", header.description.is_some()),
        ("version", header.version.is_some()),
    ];
    if let Some((name, _)) = fields.into_iter().find(|(_, present)| !present) {
        return Err(invalid(
            info,
            "header",
            format!("required <{name}> is missing"),
        ));
    }
    if parser.strict() && header.author.is_none() {
        return Err(invalid(info, "header", "required <author> is missing"));
    }
    Ok(())
}

fn parse_clrmamepro(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<ClrMameProOptions> {
    parser.validate_attributes(reader, info, &["forcenodump", "header"], None)?;
    let forcenodump = info
        .attribute("forcenodump")
        .map(|attribute| declared_attribute(attribute, info.location));
    if parser.strict()
        && let Some(value) = &forcenodump
        && !matches!(
            collapse_xml_whitespace(&value.value).as_str(),
            "obsolete" | "required" | "ignore"
        )
    {
        return Err(invalid(
            info,
            "clrmamepro",
            "forcenodump is outside its declared enumeration",
        ));
    }
    let header = info
        .attribute("header")
        .map(|attribute| declared_attribute(attribute, info.location));
    consume_empty_content(parser, reader, positions, info, empty, depth, "clrmamepro")?;
    Ok(ClrMameProOptions {
        source_order: info.source_order,
        location: info.location,
        forcenodump,
        header,
    })
}

fn parse_romcenter(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<RomCenterOptions> {
    parser.validate_attributes(reader, info, &["plugin"], None)?;
    let plugin = info
        .attribute("plugin")
        .map(|attribute| declared_attribute(attribute, info.location));
    consume_empty_content(parser, reader, positions, info, empty, depth, "romcenter")?;
    Ok(RomCenterOptions {
        source_order: info.source_order,
        location: info.location,
        plugin,
    })
}

fn parse_game(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    list_order: usize,
) -> Result<Game> {
    parser.validate_attributes(reader, info, &["name", "id", "cloneof", "cloneofid"], None)?;
    let name = required_attribute(info, "name", "game")?;
    let id = info
        .attribute("id")
        .map(|attribute| declared_attribute(attribute, info.location));
    let cloneof = info
        .attribute("cloneof")
        .map(|attribute| declared_attribute(attribute, info.location));
    let cloneofid = info
        .attribute("cloneofid")
        .map(|attribute| declared_attribute(attribute, info.location));

    let mut game = Game {
        list_order,
        source_order: info.source_order,
        location: info.location,
        name,
        id,
        cloneof,
        cloneofid,
        description: None,
        categories: Vec::new(),
        identifiers: Vec::new(),
        releases: Vec::new(),
        roms: Vec::new(),
    };
    if empty {
        return Err(invalid(
            info,
            "game",
            "required <description> and <rom> are missing",
        ));
    }

    let mut child_order = 0;
    let mut strict_last_rank = None;
    while let Some((child, child_empty)) = next_child(
        parser,
        reader,
        positions,
        info,
        "game",
        depth,
        &mut child_order,
    )? {
        let rank = game_rank(&child);
        if parser.strict() {
            let Some(rank) = rank else {
                return Err(invalid(
                    &child,
                    "game",
                    format!("unexpected element <{}>", child.local),
                ));
            };
            check_order(&mut strict_last_rank, rank, &child, "game")?;
        }
        parse_game_child(
            parser,
            reader,
            positions,
            &mut game,
            &child,
            child_empty,
            depth,
        )?;
    }

    if game.description.is_none() {
        return Err(invalid(info, "game", "required <description> is missing"));
    }
    if game.roms.is_empty() {
        return Err(invalid(info, "game", "required <rom> is missing"));
    }
    if parser.strict() && game.roms.len() != 1 {
        return Err(invalid(
            info,
            "game",
            "strict schema requires exactly one <rom>",
        ));
    }
    Ok(game)
}

fn parse_game_child(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    game: &mut Game,
    child: &StartInfo,
    child_empty: bool,
    depth: usize,
) -> Result<()> {
    if child.namespace.is_some() {
        return skip_unknown(parser, reader, positions, child, child_empty, depth);
    }

    match child.local.as_str() {
        "category" => game.categories.push(parse_simple_child(
            parser,
            reader,
            positions,
            child,
            child_empty,
            depth + 1,
            SimpleType::String,
        )?),
        "description" => {
            let description = parse_simple_child(
                parser,
                reader,
                positions,
                child,
                child_empty,
                depth + 1,
                SimpleType::String,
            )?;
            set_once(&mut game.description, description, child, "game field")?;
        }
        "game_id" if !parser.strict() => game.identifiers.push(parse_simple_child(
            parser,
            reader,
            positions,
            child,
            child_empty,
            depth + 1,
            SimpleType::String,
        )?),
        "rom" => game.roms.push(parse_rom(
            parser,
            reader,
            positions,
            child,
            child_empty,
            depth + 1,
        )?),
        "release" => game.releases.push(parse_release(
            parser,
            reader,
            positions,
            child,
            child_empty,
        )?),
        _ => skip_unknown(parser, reader, positions, child, child_empty, depth)?,
    }
    Ok(())
}

fn game_rank(info: &StartInfo) -> Option<usize> {
    if info.namespace.is_some() {
        return None;
    }
    match info.local.as_str() {
        "category" => Some(0),
        "description" => Some(1),
        "rom" => Some(2),
        "release" => Some(3),
        _ => None,
    }
}

fn parse_rom(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<Rom> {
    const ROM_ATTRIBUTES: [&str; 11] = [
        "name", "size", "crc", "md5", "sha1", "sha256", "status", "serial", "header", "date", "mia",
    ];
    let allowed = if parser.strict() {
        &ROM_ATTRIBUTES[..9]
    } else {
        &ROM_ATTRIBUTES[..]
    };
    parser.validate_attributes(reader, info, allowed, None)?;
    let name = required_attribute(info, "name", "rom")?;
    let size = info
        .attribute("size")
        .map(|attribute| declared_attribute(attribute, info.location));
    let crc = info
        .attribute("crc")
        .map(|attribute| declared_attribute(attribute, info.location));
    let md5 = info
        .attribute("md5")
        .map(|attribute| declared_attribute(attribute, info.location));
    let sha1 = info
        .attribute("sha1")
        .map(|attribute| declared_attribute(attribute, info.location));
    let sha256 = info
        .attribute("sha256")
        .map(|attribute| declared_attribute(attribute, info.location));
    let status = info
        .attribute("status")
        .map(|attribute| declared_attribute(attribute, info.location));
    let serial = info
        .attribute("serial")
        .map(|attribute| declared_attribute(attribute, info.location));
    let header = info
        .attribute("header")
        .map(|attribute| declared_attribute(attribute, info.location));
    let date = info
        .attribute("date")
        .map(|attribute| declared_attribute(attribute, info.location));
    let mia = info
        .attribute("mia")
        .map(|attribute| declared_attribute(attribute, info.location));

    if parser.strict() {
        for (name, present) in [
            ("size", size.is_some()),
            ("crc", crc.is_some()),
            ("md5", md5.is_some()),
            ("sha1", sha1.is_some()),
        ] {
            if !present {
                return Err(invalid(
                    info,
                    "rom",
                    format!("required {name:?} attribute is missing"),
                ));
            }
        }
        if let Some(size) = &size {
            validate_xs_unsigned_int(&size.value, None).map_err(|message| {
                invalid(
                    info,
                    "rom",
                    format!("invalid xs:unsignedInt size: {message}"),
                )
            })?;
        }
    }
    consume_empty_content(parser, reader, positions, info, empty, depth, "rom")?;
    Ok(Rom {
        source_order: info.source_order,
        location: info.location,
        name,
        size,
        crc,
        md5,
        sha1,
        sha256,
        status,
        serial,
        header,
        date,
        mia,
    })
}

fn parse_release(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
) -> Result<Release> {
    parser.validate_attributes(reader, info, &["name", "region"], None)?;
    let name = required_attribute(info, "name", "release")?;
    let region = required_attribute(info, "region", "release")?;
    consume_empty_content(parser, reader, positions, info, empty, 3, "release")?;
    Ok(Release {
        source_order: info.source_order,
        location: info.location,
        name,
        region,
    })
}

fn parse_simple_child(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    _depth: usize,
    expected_type: SimpleType,
) -> Result<DeclaredText> {
    let effective_type = parser
        .validate_attributes(reader, info, &[], Some(expected_type))?
        .unwrap_or(expected_type);
    let value = read_simple_content(reader, positions, info, empty)?;
    if parser.strict() {
        match effective_type {
            SimpleType::Int => validate_xs_int(&value, None),
            SimpleType::Short => {
                validate_xs_int(&value, Some((i64::from(i16::MIN), i64::from(i16::MAX))))
            }
            SimpleType::Byte => {
                validate_xs_int(&value, Some((i64::from(i8::MIN), i64::from(i8::MAX))))
            }
            SimpleType::String | SimpleType::NormalizedString | SimpleType::Token => Ok(()),
            SimpleType::Language => validate_xs_language(&value),
            SimpleType::Name => validate_xml_name(&value, true, true),
            SimpleType::NcName | SimpleType::Id | SimpleType::IdRef => {
                validate_xml_name(&value, false, true)
            }
            SimpleType::NmToken => validate_xml_name(&value, true, false),
            SimpleType::Entity => Err("xs:ENTITY requires a declared unparsed entity".to_owned()),
        }
        .map_err(|message| {
            invalid(
                info,
                "simple element",
                format!("value does not satisfy its selected XML Schema type: {message}"),
            )
        })?;
    }
    parser.register_identity(effective_type, &value, info)?;
    Ok(DeclaredText {
        value,
        source_order: info.source_order,
        location: info.location,
    })
}

fn read_simple_content(
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
) -> Result<String> {
    if empty {
        return Ok(String::new());
    }
    let mut value = String::new();
    loop {
        let (_, event) = xml_reader::next(reader, positions)?;
        match event {
            Event::Text(text) => value.push_str(&text.xml10_content()),
            Event::CData(text) => {
                value.push_str(&text.xml10_content());
            }
            Event::GeneralRef(reference) => value.push_str(&resolve_reference(&reference)?),
            Event::End(_) => return Ok(value),
            Event::Start(_) | Event::Empty(_) => {
                return Err(invalid(
                    info,
                    "simple element",
                    "nested elements are not allowed",
                ));
            }
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            Event::Eof => return Err(invalid(info, "simple element", "unexpected end of XML")),
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
        }
    }
}

fn consume_empty_content(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
    kind: &str,
) -> Result<()> {
    if empty {
        return Ok(());
    }
    if parser.strict() {
        loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::End(_) => return Ok(()),
                Event::Text(_) | Event::CData(_) | Event::GeneralRef(_) => {
                    return Err(invalid(
                        info,
                        kind,
                        format!("<{kind}> cannot contain character content"),
                    ));
                }
                Event::Start(start) | Event::Empty(start) => {
                    parser.start_info(reader, namespace, &start, depth, 0, positions)?;
                    return Err(invalid(
                        info,
                        kind,
                        format!("<{kind}> cannot contain child elements"),
                    ));
                }
                Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
                Event::Eof => return Err(invalid(info, kind, "unexpected end of XML")),
                Event::Decl(_) => {
                    return Err(invalid(info, kind, "XML declaration is not allowed here"));
                }
                Event::Comment(_) | Event::PI(_) => {}
            }
        }
    }
    let mut child_order = 0;
    if let Some((child, _)) = next_child(
        parser,
        reader,
        positions,
        info,
        kind,
        depth,
        &mut child_order,
    )? {
        return Err(invalid(
            &child,
            kind,
            format!("<{kind}> cannot contain child elements"),
        ));
    }
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
                *child_order = child_order
                    .checked_add(1)
                    .ok_or_else(|| invalid(owner, owner_name, "child element ordinal overflow"))?;
                return parser
                    .start_info(reader, namespace, &start, depth, order, positions)
                    .map(|info| Some((info, false)));
            }
            Event::Empty(start) => {
                let order = *child_order;
                *child_order = child_order
                    .checked_add(1)
                    .ok_or_else(|| invalid(owner, owner_name, "child element ordinal overflow"))?;
                return parser
                    .start_info(reader, namespace, &start, depth, order, positions)
                    .map(|info| Some((info, true)));
            }
            Event::Text(text) => {
                if !xml_whitespace_only(&text.xml10_content()) {
                    return Err(invalid(
                        owner,
                        owner_name,
                        "mixed character content is not allowed",
                    ));
                }
            }
            Event::CData(text) => {
                let text = text.xml10_content();
                if !xml_whitespace_only(&text) {
                    return Err(invalid(
                        owner,
                        owner_name,
                        "mixed CDATA content is not allowed",
                    ));
                }
            }
            Event::GeneralRef(reference) => {
                if !xml_whitespace_only(&resolve_reference(&reference)?) {
                    return Err(invalid(
                        owner,
                        owner_name,
                        "mixed character content is not allowed",
                    ));
                }
            }
            Event::End(_) => return Ok(None),
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            Event::Eof => return Err(invalid(owner, owner_name, "unexpected end of XML")),
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
        }
    }
}

fn skip_unknown(
    parser: &mut Parser,
    reader: &mut XmlReader<'_>,
    positions: &mut PositionMap<'_>,
    info: &StartInfo,
    empty: bool,
    depth: usize,
) -> Result<()> {
    if parser.strict() {
        return Err(invalid(
            info,
            "element",
            format!("unknown element <{}>", info.local),
        ));
    }
    if empty {
        return Ok(());
    }

    let mut open_elements = 1_usize;
    while open_elements > 0 {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        match event {
            Event::Start(start) => {
                parser.start_info(
                    reader,
                    namespace,
                    &start,
                    depth + open_elements,
                    0,
                    positions,
                )?;
                open_elements = open_elements
                    .checked_add(1)
                    .ok_or_else(|| invalid(info, "element", "vendor nesting counter overflow"))?;
            }
            Event::Empty(start) => {
                parser.start_info(
                    reader,
                    namespace,
                    &start,
                    depth + open_elements,
                    0,
                    positions,
                )?;
            }
            Event::End(_) => open_elements -= 1,
            Event::Eof => return Err(invalid(info, "element", "unexpected end in vendor element")),
            Event::DocType(_) => return Err(Error::XmlEntityNotAllowed),
            _ => {}
        }
    }
    Ok(())
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

fn resolve_narrowed_type(
    reader: &XmlReader<'_>,
    info: &StartInfo,
    attribute: &Attribute,
    expected: SimpleType,
) -> Result<SimpleType> {
    let lexical = collapse_xml_whitespace(&attribute.value);
    if lexical.is_empty() || lexical.chars().any(is_xml_whitespace_char) {
        return Err(invalid(info, "element", "xsi:type is not a single QName"));
    }
    xml_reader::validate_qname(&lexical)
        .map_err(|_| invalid(info, "element", "xsi:type is not a valid XML QName"))?;
    let (namespace, local) = reader.resolver().resolve(QName(lexical.as_str()), true);
    let namespace_matches = match namespace {
        ResolveResult::Bound(namespace) => namespace.as_ref() == XML_SCHEMA,
        ResolveResult::Unbound | ResolveResult::Unknown(_) => false,
    };
    if !namespace_matches {
        return Err(invalid(
            info,
            "element",
            "xsi:type must name a compatible XML Schema built-in type",
        ));
    }
    let local = local.as_ref();
    let narrowed = match (expected, local) {
        (SimpleType::String, "string") => SimpleType::String,
        (SimpleType::String, "normalizedString") => SimpleType::NormalizedString,
        (SimpleType::String, "token") => SimpleType::Token,
        (SimpleType::String, "language") => SimpleType::Language,
        (SimpleType::String, "Name") => SimpleType::Name,
        (SimpleType::String, "NCName") => SimpleType::NcName,
        (SimpleType::String, "NMTOKEN") => SimpleType::NmToken,
        (SimpleType::String, "ID") => SimpleType::Id,
        (SimpleType::String, "IDREF") => SimpleType::IdRef,
        (SimpleType::String, "ENTITY") => SimpleType::Entity,
        (SimpleType::Int, "int") => SimpleType::Int,
        (SimpleType::Int, "short") => SimpleType::Short,
        (SimpleType::Int, "byte") => SimpleType::Byte,
        _ => {
            return Err(invalid(
                info,
                "element",
                format!("xsi:type {lexical:?} is not derived from the declared type"),
            ));
        }
    };
    Ok(narrowed)
}

fn validate_xs_language(raw: &str) -> std::result::Result<(), String> {
    let lexical = collapse_xml_whitespace(raw);
    let mut subtags = lexical.split('-');
    let primary = subtags.next().map_or_else(|| "", |primary| primary);
    let valid_subtag = |subtag: &str, alpha_only: bool| {
        (1..=8).contains(&subtag.len())
            && subtag
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || (!alpha_only && byte.is_ascii_digit()))
    };
    if !valid_subtag(primary, true) || !subtags.all(|subtag| valid_subtag(subtag, false)) {
        return Err(format!("{raw:?} is not an xs:language lexical value"));
    }
    Ok(())
}

fn validate_xml_name(
    raw: &str,
    allow_colon: bool,
    require_start: bool,
) -> std::result::Result<(), String> {
    let lexical = collapse_xml_whitespace(raw);
    let mut characters = lexical.chars();
    let valid = if require_start {
        characters
            .next()
            .is_some_and(|character| xml_reader::xml_name_start_char(character, allow_colon))
            && characters.all(|character| xml_reader::xml_name_char(character, allow_colon))
    } else {
        !lexical.is_empty()
            && characters.all(|character| xml_reader::xml_name_char(character, allow_colon))
    };
    if !valid {
        return Err(format!("{raw:?} is not a valid XML name token"));
    }
    Ok(())
}

fn validate_xs_int(raw: &str, narrowed: Option<(i64, i64)>) -> std::result::Result<(), String> {
    let lexical = collapse_xml_whitespace(raw);
    let (negative, digits) = lexical.strip_prefix('-').map_or_else(
        || {
            (
                false,
                lexical
                    .strip_prefix('+')
                    .map_or(lexical.as_str(), |digits| digits),
            )
        },
        |digits| (true, digits),
    );
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{raw:?} is not an xs:int lexical value"));
    }
    let significant_digits = digits.trim_start_matches('0');
    let magnitude = if significant_digits.is_empty() {
        0
    } else {
        significant_digits
            .parse::<u64>()
            .map_err(|_| format!("{raw:?} is outside the integer value range"))?
    };
    let value = if negative {
        -(i128::from(magnitude))
    } else {
        i128::from(magnitude)
    };
    let (minimum, maximum) = narrowed.map_or_else(
        || (i128::from(i32::MIN), i128::from(i32::MAX)),
        |(minimum, maximum)| (i128::from(minimum), i128::from(maximum)),
    );
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{raw:?} is outside {minimum}..={maximum}"));
    }
    Ok(())
}

fn validate_xs_unsigned_int(
    raw: &str,
    narrowed: Option<(u64, u64)>,
) -> std::result::Result<(), String> {
    let lexical = collapse_xml_whitespace(raw);
    let (negative, digits) = lexical.strip_prefix('-').map_or_else(
        || {
            (
                false,
                lexical
                    .strip_prefix('+')
                    .map_or(lexical.as_str(), |digits| digits),
            )
        },
        |digits| (true, digits),
    );
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{raw:?} is not an xs:unsignedInt lexical value"));
    }
    let significant_digits = digits.trim_start_matches('0');
    let value = if significant_digits.is_empty() {
        0
    } else {
        significant_digits
            .parse::<u64>()
            .map_err(|_| format!("{raw:?} is outside the integer value range"))?
    };
    if negative && value != 0 {
        return Err(format!("{raw:?} is negative"));
    }
    let (minimum, maximum) = narrowed.unwrap_or_else(|| (0, u64::from(u32::MAX)));
    if value < minimum || value > maximum {
        return Err(format!("{raw:?} is outside {minimum}..={maximum}"));
    }
    Ok(())
}

fn check_order(
    previous: &mut Option<usize>,
    current: usize,
    info: &StartInfo,
    owner: &str,
) -> Result<()> {
    if previous.is_some_and(|previous| current < previous) {
        return Err(invalid(
            info,
            owner,
            "element is out of schema sequence order",
        ));
    }
    *previous = Some(current);
    Ok(())
}

fn set_once(
    target: &mut Option<DeclaredText>,
    value: DeclaredText,
    info: &StartInfo,
    kind: &str,
) -> Result<()> {
    if target.replace(value).is_some() {
        return Err(invalid(info, kind, format!("duplicate <{}>", info.local)));
    }
    Ok(())
}

fn required_attribute(info: &StartInfo, name: &str, kind: &str) -> Result<DeclaredText> {
    info.attribute(name)
        .map(|attribute| declared_attribute(attribute, info.location))
        .ok_or_else(|| {
            invalid(
                info,
                kind,
                format!("required {name:?} attribute is missing"),
            )
        })
}

fn declared_attribute(attribute: &Attribute, location: RecordLocation) -> DeclaredText {
    DeclaredText {
        value: attribute.value.clone(),
        source_order: attribute.ordinal,
        location,
    }
}

fn expanded_name(namespace: Option<&str>, local: &str) -> String {
    namespace.map_or_else(
        || local.to_owned(),
        |namespace| format!("{{{namespace}}}{local}"),
    )
}

fn xml_tokens(value: &str) -> Vec<&str> {
    value
        .split(is_xml_whitespace_char)
        .filter(|token| !token.is_empty())
        .collect()
}

fn collapse_xml_whitespace(value: &str) -> String {
    let mut collapsed = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if is_xml_whitespace_char(character) {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(character);
        }
    }
    collapsed
}

fn xml_whitespace_only(value: &str) -> bool {
    value.chars().all(is_xml_whitespace_char)
}

const fn is_xml_whitespace_char(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\r' | ' ')
}

fn invalid(info: &StartInfo, kind: &str, message: impl Into<String>) -> Error {
    Error::CatalogParse {
        message: message.into(),
        record_kind: Some(kind.to_owned()),
        record_name: info
            .attribute("name")
            .map(|attribute| attribute.value.clone()),
        line: Some(info.location.line),
        column: Some(info.location.column),
    }
}

fn xml_error(message: impl Into<String>) -> Error {
    Error::CatalogParse {
        message: message.into(),
        record_kind: Some("document".to_owned()),
        record_name: None,
        line: None,
        column: None,
    }
}
