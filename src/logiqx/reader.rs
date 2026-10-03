use quick_xml::events::Event;

use super::{
    AttributePosition, DocumentAttribute, DocumentMetadata, Game, Header, RecordLocation,
    UnsupportedAttribute,
    data_file::{self, XmlSourceMap},
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
    read_with_diagnostics(bytes, false, start, consume)
}

pub(super) fn read_with_diagnostics<S, E: From<crate::Error>>(
    bytes: &[u8],
    retain_diagnostics: bool,
    start: impl FnOnce(DocumentAttributes<'_>) -> Result<S, E>,
    mut consume: impl FnMut(&mut S, LocatedGame) -> Result<(), E>,
) -> Result<ValidatedLogiqx<S>, E> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let mut budget = NodeBudget::default();
        let (root, empty) = data_file::read_datafile_root(reader, positions, &mut budget)?;
        if root
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
                let (namespace, event) = xml_reader::next(reader, positions)?;
                let node = match event {
                    Event::Start(start) => xml_reader::read_element(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        1,
                        positions,
                    )?,
                    Event::Empty(start) => xml_reader::element_from_start(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        1,
                        positions,
                    )?,
                    Event::End(_) => break,
                    Event::Eof => {
                        return Err(crate::Error::XmlValidation(
                            "unexpected end of input inside <datafile>".into(),
                        )
                        .into());
                    }
                    _ => continue,
                };
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
        data_file::finish_document(reader, positions)?;
        Ok(ValidatedLogiqx {
            metadata,
            sink,
            document_diagnostics: document_map.unsupported_attributes,
        })
    })
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
