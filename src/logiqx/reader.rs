use quick_xml::events::Event;

use super::LogiqxMode;
use super::{
    AttributePosition, DocumentAttribute, DocumentMetadata, Game, Header, RecordLocation,
    UnsupportedAttribute,
    data_file::{self, XmlSourceMap},
    dtd15,
};
use crate::xml_reader::{self, Element, NodeBudget};

/// Root declarations available before any game is consumed.
#[derive(Clone, Copy, Debug)]
pub struct DocumentAttributes<'a> {
    pub build: Option<&'a str>,
    pub debug: Option<&'a str>,
    pub attribute_positions: &'a [AttributePosition<DocumentAttribute>],
}

/// One completed game and its actual record positions.
#[derive(Debug)]
pub struct LocatedGame {
    pub game: Game,
    pub location: RecordLocation,
    pub rom_locations: Vec<RecordLocation>,
    pub device_ref_locations: Vec<RecordLocation>,
    pub unsupported_attributes: Vec<UnsupportedAttribute>,
}

/// Reader-owned proof that the root closed and the complete input reached valid EOF.
#[derive(Debug)]
#[must_use]
pub struct ValidatedLogiqx<S> {
    metadata: DocumentMetadata,
    sink: S,
    document_diagnostics: Vec<UnsupportedAttribute>,
}

impl<S> ValidatedLogiqx<S> {
    /// Consume EOF proof together with its final document facts and sink.
    #[must_use]
    pub fn into_parts(self) -> (DocumentMetadata, S) {
        (self.metadata, self.sink)
    }

    pub(super) fn into_collected_parts(self) -> (DocumentMetadata, S, Vec<UnsupportedAttribute>) {
        (self.metadata, self.sink, self.document_diagnostics)
    }
}

/// Stream completed Logiqx games through one XML pass. Final document metadata
/// becomes available only after EOF; a compatible header may follow games.
pub fn read_with<S, E: From<crate::Error>>(
    bytes: &[u8],
    start: impl FnOnce(DocumentAttributes<'_>) -> Result<S, E>,
    consume: impl FnMut(&mut S, LocatedGame) -> Result<(), E>,
) -> Result<ValidatedLogiqx<S>, E> {
    read_with_mode(bytes, LogiqxMode::ObservedCompatible, start, consume)
}

/// Stream a Logiqx document using the selected interpretation.
pub fn read_with_mode<S, E: From<crate::Error>>(
    bytes: &[u8],
    mode: LogiqxMode,
    start: impl FnOnce(DocumentAttributes<'_>) -> Result<S, E>,
    consume: impl FnMut(&mut S, LocatedGame) -> Result<(), E>,
) -> Result<ValidatedLogiqx<S>, E> {
    read_with_diagnostics_mode(bytes, false, mode, start, consume)
}

pub(super) fn read_with_diagnostics_mode<S, E: From<crate::Error>>(
    bytes: &[u8],
    retain_diagnostics: bool,
    mode: LogiqxMode,
    start: impl FnOnce(DocumentAttributes<'_>) -> Result<S, E>,
    mut consume: impl FnMut(&mut S, LocatedGame) -> Result<(), E>,
) -> Result<ValidatedLogiqx<S>, E> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let mut budget = NodeBudget::default();
        let strict_dtd15 = mode == LogiqxMode::StrictDtd15;
        let (mut root, empty, declaration_policy) =
            data_file::read_datafile_root(reader, positions, &mut budget, strict_dtd15)?;
        let grammar_policy = strict_dtd15.then_some(declaration_policy);
        let mut dtd15 = if strict_dtd15 {
            Some(dtd15::Validator::new(&mut root, declaration_policy)?)
        } else {
            None
        };
        if dtd15.is_none()
            && root
                .attributes
                .get("debug")
                .is_some_and(|value| !["yes", "no"].contains(&value.as_str()))
        {
            return Err(
                crate::Error::XmlValidation("invalid debug value on <datafile>".into()).into(),
            );
        }
        let mut document_map = XmlSourceMap::default();
        if retain_diagnostics {
            data_file::collect_unsupported_attributes(&root, None, &mut document_map);
        }
        let mut metadata = DocumentMetadata {
            build: root.attributes.get("build").cloned(),
            debug: root.attributes.get("debug").cloned(),
            attribute_positions: root
                .attributes
                .positions(DocumentAttribute::from_name)
                .collect(),
            ..DocumentMetadata::default()
        };
        drop(root);
        let mut sink = start(DocumentAttributes {
            build: metadata.build(),
            debug: metadata.debug(),
            attribute_positions: metadata.attribute_positions(),
        })?;
        if !empty {
            loop {
                let mut node =
                    match read_root_child(reader, positions, &mut budget, grammar_policy)? {
                        RootEvent::Child(node) => node,
                        RootEvent::Misc => continue,
                        RootEvent::RootEnd => break,
                    };
                if let Some(validator) = &mut dtd15 {
                    validator.accept_root_child(&mut node)?;
                }
                match data_file::local_name(&node.name) {
                    "header" => {
                        data_file::set_once(
                            &mut metadata.header,
                            Header::from_xml(&node)?,
                            "header",
                        )?;
                        if retain_diagnostics {
                            data_file::collect_subtree_attributes(&node, None, &mut document_map);
                        }
                    }
                    "game" => {
                        let game = located_game(&node, retain_diagnostics)?;
                        drop(node);
                        consume(&mut sink, game)?;
                    }
                    "file_name" => data_file::set_once(
                        &mut metadata.file_name,
                        node.direct_text(),
                        "file_name",
                    )?,
                    "sha1" => data_file::set_once(
                        &mut metadata.sha1,
                        data_file::parse_datafile_sha1(&node.direct_text())?,
                        "sha1",
                    )?,
                    _ => {}
                }
            }
        }
        data_file::finish_document(reader, positions, strict_dtd15)?;
        if let Some(validator) = dtd15 {
            validator.finish()?;
        }
        Ok(ValidatedLogiqx {
            metadata,
            sink,
            document_diagnostics: document_map.unsupported_attributes,
        })
    })
}

enum RootEvent {
    Child(Element),
    Misc,
    RootEnd,
}

fn read_root_child(
    reader: &mut xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
    budget: &mut NodeBudget,
    policy: Option<dtd15::DeclarationPolicy>,
) -> crate::Result<RootEvent> {
    let (namespace, event) = xml_reader::next(reader, positions)?;
    match event {
        Event::Start(start) => {
            xml_reader::read_element(reader, namespace, &start, budget, 1, positions)
                .map(RootEvent::Child)
        }
        Event::Empty(start) => {
            xml_reader::element_from_start(reader, namespace, &start, budget, 1, positions)
                .map(RootEvent::Child)
        }
        Event::End(_) => Ok(RootEvent::RootEnd),
        Event::Text(text) => {
            if let Some(policy) = policy {
                policy.validate_container_text(&text.xml10_content(), "datafile")?;
            }
            Ok(RootEvent::Misc)
        }
        Event::Comment(_) | Event::PI(_) => Ok(RootEvent::Misc),
        Event::CData(_) | Event::GeneralRef(_) => {
            if policy.is_some() {
                return Err(crate::Error::XmlValidation(
                    "CDATA and character references are not allowed in element-only <datafile>"
                        .into(),
                ));
            }
            Ok(RootEvent::Misc)
        }
        Event::Decl(_) | Event::DocType(_) => Err(crate::Error::XmlValidation(
            "declarations are not allowed inside the document root".into(),
        )),
        Event::Eof => Err(crate::Error::XmlValidation(
            "unexpected end of input inside <datafile>".into(),
        )),
    }
}

fn located_game(node: &Element, retain_diagnostics: bool) -> crate::Result<LocatedGame> {
    let mut source_map = XmlSourceMap::default();
    let mut rom_locations = Vec::new();
    let mut device_ref_locations = Vec::new();
    if retain_diagnostics {
        data_file::collect_unsupported_attributes(node, Some(0), &mut source_map);
    }
    for child in node.children() {
        match data_file::local_name(&child.name) {
            "rom" => rom_locations.push(child.location),
            "device_ref" => device_ref_locations.push(child.location),
            _ => {}
        }
        if retain_diagnostics {
            data_file::collect_subtree_attributes(child, Some(0), &mut source_map);
        }
    }
    Ok(LocatedGame {
        game: Game::from_xml(node)?,
        location: node.location,
        rom_locations,
        device_ref_locations,
        unsupported_attributes: source_map.unsupported_attributes,
    })
}
