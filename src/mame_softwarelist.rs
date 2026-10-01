use std::collections::HashSet;

pub use crate::domain::media::SourceLoadInstruction as LoadInstruction;

use crate::{
    logiqx::RecordLocation,
    mame::{ExtensionValue, XmlExtension},
    xml_reader::{self, Element, NodeBudget},
};
use quick_xml::events::Event;

macro_rules! string_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            const fn new(value: String) -> Self {
                Self(value)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_identity!(SoftwareListName);
string_identity!(SoftwareItemName);
string_identity!(PartName);
string_identity!(AreaName);
string_identity!(ComponentName);
string_identity!(DipSwitchName);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SupportedStatus {
    #[default]
    Yes,
    Partial,
    No,
}

impl SupportedStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::Partial => "partial",
            Self::No => "no",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedValue {
    pub name: String,
    pub value: Option<String>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListCatalog {
    pub build: Option<String>,
    pub lists: Vec<SoftwareList>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareList {
    pub name: SoftwareListName,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub location: RecordLocation,
    pub items: Vec<SoftwareItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareItem {
    pub name: SoftwareItemName,
    pub clone_of: Option<SoftwareItemName>,
    pub supported: Option<SupportedStatus>,
    pub description: String,
    pub year: String,
    pub publisher: String,
    pub notes: Option<String>,
    pub info: Vec<NamedValue>,
    pub shared_features: Vec<NamedValue>,
    pub parts: Vec<SoftwarePart>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwarePart {
    pub name: PartName,
    pub interface: String,
    pub features: Vec<NamedValue>,
    pub dipswitches: Vec<SoftwareDipSwitch>,
    pub areas: Vec<SoftwareArea>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipSwitch {
    pub name: DipSwitchName,
    pub tag: String,
    pub mask: String,
    pub values: Vec<SoftwareDipValue>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipValue {
    pub name: String,
    pub value: String,
    pub is_default: bool,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AreaKind {
    Data,
    Disk,
}

impl AreaKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Disk => "disk",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Little => "little",
            Self::Big => "big",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareArea {
    pub name: AreaName,
    pub kind: AreaKind,
    pub declared_size: Option<u64>,
    pub width: Option<u8>,
    pub endianness: Option<Endianness>,
    pub components: Vec<SoftwareComponent>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareComponent {
    Rom(SoftwareRom),
    Disk(SoftwareDisk),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareRom {
    pub name: Option<ComponentName>,
    pub size: Option<u64>,
    pub crc: Option<[u8; 4]>,
    pub sha1: Option<[u8; 20]>,
    pub offset: Option<u64>,
    pub value: Option<String>,
    pub status: Option<DumpStatus>,
    pub load: Option<LoadInstruction>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDisk {
    pub requirement: crate::disk::DiskRequirement,
    pub status: Option<DumpStatus>,
    pub writeable: Option<bool>,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DumpStatus {
    BadDump,
    NoDump,
    #[default]
    Good,
}

impl DumpStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BadDump => "baddump",
            Self::NoDump => "nodump",
            Self::Good => "good",
        }
    }
}

impl SoftwareListCatalog {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        xml_reader::with_reader(bytes, |reader, positions| {
            let mut budget = NodeBudget::with_limit(xml_reader::MAX_MAME_XML_NODES);
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
                Event::Start(start) if start.local_name().as_ref() == "softwarelist" => {
                    let mut extensions = Vec::new();
                    let (list, build) = parse_list_events(
                        reader,
                        positions,
                        namespace,
                        &start,
                        &mut budget,
                        &mut extensions,
                    )?;
                    finish_softwarelist_document(reader, positions)?;
                    return Ok(Self {
                        build,
                        lists: vec![list],
                        extensions,
                    });
                }
                Event::Empty(start) if start.local_name().as_ref() == "softwarelist" => {
                    let node = xml_reader::element_from_start(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        0,
                        positions,
                    )?;
                    let mut extensions = Vec::new();
                    let _ = parse_empty_list(&node, &mut extensions)?;
                    unreachable!("empty software lists have no items")
                }
                Event::Start(start) => (
                    xml_reader::element_from_start(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        0,
                        positions,
                    )?,
                    false,
                ),
                Event::Empty(start) => (
                    xml_reader::element_from_start(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        0,
                        positions,
                    )?,
                    true,
                ),
                _ => unreachable!(),
            };
            parse_softwarelists(reader, positions, &root, empty, &mut budget)
        })
    }
}

fn parse_softwarelists(
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
    root: &Element,
    empty: bool,
    budget: &mut NodeBudget,
) -> crate::Result<SoftwareListCatalog> {
    if root.name != "softwarelists" {
        return Err(crate::Error::XmlValidation(format!(
            "expected <softwarelist> or <softwarelists>, found <{}>",
            root.name
        )));
    }
    let build = root.attributes.get("build").cloned();
    let mut extensions = Vec::new();
    retain_unknown_attributes(root, &["build"], "document", None, &mut extensions);
    let mut lists = Vec::new();
    let mut names = HashSet::new();
    if !empty {
        loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::Start(start) if start.local_name().as_ref() == "softwarelist" => {
                    let (list, _) = parse_list_events(
                        reader,
                        positions,
                        namespace,
                        &start,
                        budget,
                        &mut extensions,
                    )?;
                    insert_list(list, &mut names, &mut lists)?;
                }
                Event::Empty(start) if start.local_name().as_ref() == "softwarelist" => {
                    let node = xml_reader::element_from_start(
                        reader, namespace, &start, budget, 1, positions,
                    )?;
                    insert_list(
                        parse_empty_list(&node, &mut extensions)?,
                        &mut names,
                        &mut lists,
                    )?;
                }
                Event::Start(start) => {
                    let node =
                        xml_reader::read_element(reader, namespace, &start, budget, 1, positions)?;
                    extensions.push(extension("document", None, &node)?);
                }
                Event::Empty(start) => {
                    let node = xml_reader::element_from_start(
                        reader, namespace, &start, budget, 1, positions,
                    )?;
                    extensions.push(extension("document", None, &node)?);
                }
                Event::End(_) => break,
                Event::Eof => {
                    return Err(crate::Error::XmlValidation(
                        "unexpected end of input inside <softwarelists>".into(),
                    ));
                }
                _ => {}
            }
        }
    }
    finish_softwarelist_document(reader, positions)?;
    Ok(SoftwareListCatalog {
        build,
        lists,
        extensions,
    })
}

fn insert_list(
    list: SoftwareList,
    names: &mut HashSet<String>,
    lists: &mut Vec<SoftwareList>,
) -> crate::Result<()> {
    if !names.insert(list.name.as_str().to_owned()) {
        return Err(crate::Error::XmlValidation(format!(
            "duplicate software-list name {:?}",
            list.name.as_str()
        )));
    }
    lists.push(list);
    Ok(())
}

fn parse_empty_list(
    node: &Element,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareList> {
    let name = SoftwareListName::new(required(node, "name")?);
    retain_unknown_attributes(
        node,
        &["name", "description"],
        "software_list",
        Some(name.as_str()),
        extensions,
    );
    parse_list_parts(node, name, Vec::new(), None)
}

fn parse_list_events(
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
    namespace: Option<String>,
    start: &quick_xml::events::BytesStart<'_>,
    budget: &mut NodeBudget,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<(SoftwareList, Option<String>)> {
    let node = xml_reader::element_from_start(reader, namespace, start, budget, 1, positions)?;
    let name = SoftwareListName::new(required(&node, "name")?);
    retain_unknown_attributes(
        &node,
        &["name", "description"],
        "software_list",
        Some(name.as_str()),
        extensions,
    );
    let mut items = Vec::new();
    let mut notes = None;
    let mut item_names = HashSet::<String>::new();
    loop {
        let (namespace, event) = xml_reader::next(reader, positions)?;
        let item_node = match event {
            Event::Start(start) if start.local_name().as_ref() == "notes" => {
                let notes_node =
                    xml_reader::read_element(reader, namespace, &start, budget, 2, positions)?;
                retain_unknown_node_content(
                    &notes_node,
                    &[],
                    "software_list_notes",
                    Some(name.as_str()),
                    extensions,
                )?;
                if notes.replace(notes_node.direct_text()).is_some() {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate software-list notes for {:?}",
                        name.as_str()
                    )));
                }
                continue;
            }
            Event::Empty(start) if start.local_name().as_ref() == "notes" => {
                let notes_node = xml_reader::element_from_start(
                    reader, namespace, &start, budget, 2, positions,
                )?;
                retain_unknown_node_content(
                    &notes_node,
                    &[],
                    "software_list_notes",
                    Some(name.as_str()),
                    extensions,
                )?;
                if notes.replace(notes_node.direct_text()).is_some() {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate software-list notes for {:?}",
                        name.as_str()
                    )));
                }
                continue;
            }
            Event::Start(start) if start.local_name().as_ref() == "software" => Some(
                xml_reader::read_element(reader, namespace, &start, budget, 2, positions)?,
            ),
            Event::Empty(start) if start.local_name().as_ref() == "software" => Some(
                xml_reader::element_from_start(reader, namespace, &start, budget, 2, positions)?,
            ),
            Event::Start(start) => {
                let unknown =
                    xml_reader::read_element(reader, namespace, &start, budget, 2, positions)?;
                extensions.push(extension("software_list", Some(name.as_str()), &unknown)?);
                continue;
            }
            Event::Empty(start) => {
                let unknown = xml_reader::element_from_start(
                    reader, namespace, &start, budget, 2, positions,
                )?;
                extensions.push(extension("software_list", Some(name.as_str()), &unknown)?);
                continue;
            }
            Event::End(_) => break,
            Event::Eof => {
                return Err(crate::Error::XmlValidation(
                    "unexpected end of input inside <softwarelist>".into(),
                ));
            }
            _ => continue,
        };
        if let Some(item_node) = item_node {
            let item = parse_item(&item_node, &name, extensions)?;
            if !item_names.insert(item.name.as_str().to_owned()) {
                return Err(crate::Error::XmlValidation(format!(
                    "duplicate software item {:?} in list {:?}",
                    item.name.as_str(),
                    name.as_str()
                )));
            }
            items.push(item);
        }
    }
    let build = node.attributes.get("build").cloned();
    Ok((parse_list_parts(&node, name, items, notes)?, build))
}

fn finish_softwarelist_document(
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

fn parse_list_parts(
    node: &Element,
    name: SoftwareListName,
    items: Vec<SoftwareItem>,
    notes: Option<String>,
) -> crate::Result<SoftwareList> {
    if items.is_empty() {
        return Err(crate::Error::XmlValidation(format!(
            "software list {:?} has no software items",
            name.as_str()
        )));
    }
    Ok(SoftwareList {
        name,
        description: node.attributes.get("description").cloned(),
        notes,
        location: node.location,
        items,
    })
}

fn parse_item(
    node: &Element,
    list: &SoftwareListName,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareItem> {
    let name = SoftwareItemName::new(required(node, "name")?);
    let record = format!("{}:{}", list.as_str(), name.as_str());
    retain_unknown_attributes(
        node,
        &["name", "cloneof", "supported"],
        "software_item",
        Some(&record),
        extensions,
    );
    let clone_of = node
        .attributes
        .get("cloneof")
        .cloned()
        .map(SoftwareItemName::new);
    let supported = match node.attributes.get("supported").map(String::as_str) {
        None | Some("yes") => Some(SupportedStatus::Yes),
        Some("partial") => Some(SupportedStatus::Partial),
        Some("no") => Some(SupportedStatus::No),
        Some(other) => {
            return Err(invalid_value(node, &record, "supported", other));
        }
    };
    let mut description = None;
    let mut year = None;
    let mut publisher = None;
    let mut notes = None;
    let mut info = Vec::new();
    let mut shared_features = Vec::new();
    let mut parts = Vec::new();
    let mut part_names = HashSet::<String>::new();
    for child in node.children() {
        match child.name.as_str() {
            "description" => set_item_text(&mut description, child, &name, &record, extensions)?,
            "year" => set_item_text(&mut year, child, &name, &record, extensions)?,
            "publisher" => set_item_text(&mut publisher, child, &name, &record, extensions)?,
            "notes" => {
                set_once(&mut notes, child, &name)?;
                retain_unknown_node_content(
                    child,
                    &[],
                    "software_item_notes",
                    Some(&record),
                    extensions,
                )?;
            }
            "info" => {
                info.push(named_value(child)?);
                retain_unknown_attributes(
                    child,
                    &["name", "value"],
                    "software_info",
                    Some(&record),
                    extensions,
                );
                retain_child_elements(child, "software_info", Some(&record), extensions)?;
            }
            "sharedfeat" => {
                shared_features.push(named_value(child)?);
                retain_unknown_attributes(
                    child,
                    &["name", "value"],
                    "software_shared_feature",
                    Some(&record),
                    extensions,
                );
                retain_child_elements(child, "software_shared_feature", Some(&record), extensions)?;
            }
            "part" => {
                let part = parse_part(child, &record, extensions)?;
                if !part_names.insert(part.name.as_str().to_owned()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate part {:?} for item {:?}",
                        part.name.as_str(),
                        name.as_str()
                    )));
                }
                parts.push(part);
            }
            _ => extensions.push(extension("software_item", Some(&record), child)?),
        }
    }
    Ok(SoftwareItem {
        name,
        clone_of,
        supported,
        description: required_text(description, "description", &record)?,
        year: required_text(year, "year", &record)?,
        publisher: required_text(publisher, "publisher", &record)?,
        notes,
        info,
        shared_features,
        parts,
        location: node.location,
    })
}

fn parse_part(
    node: &Element,
    item_record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwarePart> {
    let name = PartName::new(required(node, "name")?);
    let record = format!("{item_record}:{}", name.as_str());
    let interface = required(node, "interface")?;
    retain_unknown_attributes(
        node,
        &["name", "interface"],
        "software_part",
        Some(&record),
        extensions,
    );
    let mut features = Vec::new();
    let mut dipswitches = Vec::new();
    let mut areas = Vec::new();
    for child in node.children() {
        match child.name.as_str() {
            "feature" => {
                features.push(named_value(child)?);
                retain_unknown_attributes(
                    child,
                    &["name", "value"],
                    "software_part_feature",
                    Some(&record),
                    extensions,
                );
                retain_child_elements(child, "software_part_feature", Some(&record), extensions)?;
            }
            "dataarea" | "diskarea" => {
                let area = parse_area(child, &record, extensions)?;
                areas.push(area);
            }
            "dipswitch" => dipswitches.push(parse_dipswitch(child, &record, extensions)?),
            _ => extensions.push(extension("software_part", Some(&record), child)?),
        }
    }
    Ok(SoftwarePart {
        name,
        interface,
        features,
        dipswitches,
        areas,
        location: node.location,
    })
}

fn parse_dipswitch(
    node: &Element,
    part_record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareDipSwitch> {
    let name = DipSwitchName::new(required(node, "name")?);
    let record = format!("{part_record}:{}", name.as_str());
    retain_unknown_attributes(
        node,
        &["name", "tag", "mask"],
        "software_dipswitch",
        Some(&record),
        extensions,
    );
    let mut values = Vec::new();
    for child in node.children() {
        if child.name == "dipvalue" {
            retain_unknown_attributes(
                child,
                &["name", "value", "default"],
                "software_dipvalue",
                Some(&record),
                extensions,
            );
            retain_child_elements(child, "software_dipvalue", Some(&record), extensions)?;
            let is_default = match child.attributes.get("default").map(String::as_str) {
                None | Some("no") => false,
                Some("yes") => true,
                Some(other) => return Err(invalid_value(child, &record, "default", other)),
            };
            values.push(SoftwareDipValue {
                name: required(child, "name")?,
                value: required(child, "value")?,
                is_default,
                location: child.location,
            });
        } else {
            extensions.push(extension("software_dipswitch", Some(&record), child)?);
        }
    }
    Ok(SoftwareDipSwitch {
        name,
        tag: required(node, "tag")?,
        mask: required(node, "mask")?,
        values,
        location: node.location,
    })
}

fn parse_area(
    node: &Element,
    part_record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareArea> {
    let kind = if node.name == "dataarea" {
        AreaKind::Data
    } else {
        AreaKind::Disk
    };
    let name = AreaName::new(required(node, "name")?);
    let record = format!("{part_record}:{}:{}", kind.as_str(), name.as_str());
    let known_attributes: &[&str] = match kind {
        AreaKind::Data => &["name", "size", "width", "endianness"],
        AreaKind::Disk => &["name"],
    };
    retain_unknown_attributes(
        node,
        known_attributes,
        "software_area",
        Some(&record),
        extensions,
    );
    let (declared_size, width, endianness, component_name) = match kind {
        AreaKind::Data => (
            Some(parse_number_at(
                node,
                &record,
                "size",
                &required(node, "size")?,
            )?),
            Some(
                node.attributes
                    .get("width")
                    .map(|value| parse_width(node, &record, value))
                    .transpose()?
                    .unwrap_or(8),
            ),
            Some(
                node.attributes
                    .get("endianness")
                    .map(|value| parse_endianness(node, &record, value))
                    .transpose()?
                    .unwrap_or(Endianness::Little),
            ),
            "rom",
        ),
        AreaKind::Disk => (None, None, None, "disk"),
    };
    let mut components = Vec::new();
    for child in node.children() {
        if child.name == component_name {
            let component = if kind == AreaKind::Data {
                SoftwareComponent::Rom(parse_rom(child, &record, extensions)?)
            } else {
                SoftwareComponent::Disk(parse_disk(child, &record, extensions)?)
            };
            components.push(component);
        } else {
            extensions.push(extension("software_area", Some(&record), child)?);
        }
    }
    Ok(SoftwareArea {
        name,
        kind,
        declared_size,
        width,
        endianness,
        components,
        location: node.location,
    })
}

fn parse_rom(
    node: &Element,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareRom> {
    retain_unknown_attributes(
        node,
        &[
            "name", "size", "crc", "sha1", "offset", "value", "status", "loadflag",
        ],
        "software_rom",
        Some(record),
        extensions,
    );
    for child in node.children() {
        extensions.push(extension("software_rom", Some(record), child)?);
    }
    Ok(SoftwareRom {
        name: node.attributes.get("name").cloned().map(ComponentName::new),
        size: parse_optional_number(node, record, "size")?,
        crc: parse_digest(node, record, "crc")?,
        sha1: parse_digest(node, record, "sha1")?,
        offset: parse_optional_number(node, record, "offset")?,
        value: node.attributes.get("value").cloned(),
        status: Some(parse_status(node, record)?.unwrap_or_default()),
        load: parse_load(node, record)?,
        location: node.location,
    })
}

fn parse_disk(
    node: &Element,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<SoftwareDisk> {
    retain_unknown_attributes(
        node,
        &["name", "sha1", "status", "writeable"],
        "software_disk",
        Some(record),
        extensions,
    );
    retain_child_elements(node, "software_disk", Some(record), extensions)?;
    let writeable = match node.attributes.get("writeable").map(String::as_str) {
        None => None,
        Some("no") => Some(false),
        Some("yes") => Some(true),
        Some(other) => return Err(invalid_value(node, record, "writeable", other)),
    };
    let name = required(node, "name")?;
    let sha1 = parse_digest(node, record, "sha1")?;
    Ok(SoftwareDisk {
        requirement: crate::disk::DiskRequirement::new(
            crate::disk::DiskName::new(name),
            sha1.map(crate::disk::DiskIdentitySha1::new),
            crate::disk::DiskDigestScope::ChdHeaderSha1,
        ),
        status: Some(parse_status(node, record)?.unwrap_or_default()),
        writeable: Some(writeable.unwrap_or(false)),
        location: node.location,
    })
}

fn parse_status(node: &Element, record: &str) -> crate::Result<Option<DumpStatus>> {
    match node.attributes.get("status").map(String::as_str) {
        None => Ok(None),
        Some("good") => Ok(Some(DumpStatus::Good)),
        Some("baddump") => Ok(Some(DumpStatus::BadDump)),
        Some("nodump") => Ok(Some(DumpStatus::NoDump)),
        Some(other) => Err(invalid_value(node, record, "status", other)),
    }
}

fn parse_load(node: &Element, record: &str) -> crate::Result<Option<LoadInstruction>> {
    node.attributes
        .get("loadflag")
        .map(|value| {
            let load = match value.as_str() {
                "load16_byte" => LoadInstruction::Load16Byte,
                "load16_word" => LoadInstruction::Load16Word,
                "load16_word_swap" => LoadInstruction::Load16WordSwap,
                "load32_byte" => LoadInstruction::Load32Byte,
                "load32_word" => LoadInstruction::Load32Word,
                "load32_word_swap" => LoadInstruction::Load32WordSwap,
                "load32_dword" => LoadInstruction::Load32Dword,
                "load64_word" => LoadInstruction::Load64Word,
                "load64_word_swap" => LoadInstruction::Load64WordSwap,
                "reload" => LoadInstruction::Reload,
                "fill" => LoadInstruction::Fill,
                "continue" => LoadInstruction::Continue,
                "reload_plain" => LoadInstruction::ReloadPlain,
                "ignore" => LoadInstruction::Ignore,
                other => return Err(invalid_value(node, record, "loadflag", other)),
            };
            Ok(load)
        })
        .transpose()
}

fn parse_width(node: &Element, record: &str, value: &str) -> crate::Result<u8> {
    let width = value
        .parse::<u8>()
        .map_err(|_| invalid_value(node, record, "width", value))?;
    if matches!(width, 8 | 16 | 32 | 64) {
        Ok(width)
    } else {
        Err(invalid_value(node, record, "width", value))
    }
}

fn parse_endianness(node: &Element, record: &str, value: &str) -> crate::Result<Endianness> {
    match value {
        "little" => Ok(Endianness::Little),
        "big" => Ok(Endianness::Big),
        other => Err(invalid_value(node, record, "endianness", other)),
    }
}

fn parse_optional_number(node: &Element, record: &str, name: &str) -> crate::Result<Option<u64>> {
    node.attributes
        .get(name)
        .map(|value| parse_number_at(node, record, name, value))
        .transpose()
}

fn parse_number_at(node: &Element, record: &str, field: &str, value: &str) -> crate::Result<u64> {
    parse_number(value).map_err(|_| invalid_value(node, record, field, value))
}

fn parse_number(value: &str) -> crate::Result<u64> {
    let parsed = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(hex, 16),
        None if value.len() > 1 && value.starts_with('0') => u64::from_str_radix(value, 8),
        None => value.parse(),
    }
    .map_err(|_| crate::Error::XmlValidation(format!("invalid unsigned integer {value:?}")))?;
    i64::try_from(parsed).map_err(|_| {
        crate::Error::XmlValidation(format!("integer exceeds supported storage range {value:?}"))
    })?;
    Ok(parsed)
}

fn parse_digest<const N: usize>(
    node: &Element,
    record: &str,
    field: &str,
) -> crate::Result<Option<[u8; N]>> {
    let Some(value) = node.attributes.get(field) else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    let bytes = hex::decode(value).map_err(|_| invalid_value(node, record, field, value))?;
    let digest: [u8; N] = bytes
        .try_into()
        .map_err(|_: Vec<u8>| invalid_value(node, record, field, value))?;
    Ok(Some(digest))
}

fn named_value(node: &Element) -> crate::Result<NamedValue> {
    Ok(NamedValue {
        name: required(node, "name")?,
        value: node.attributes.get("value").cloned(),
        location: node.location,
    })
}

fn set_once(
    field: &mut Option<String>,
    node: &Element,
    item: &SoftwareItemName,
) -> crate::Result<()> {
    if field.replace(node.direct_text()).is_some() {
        return Err(crate::Error::XmlValidation(format!(
            "duplicate {} field for software {:?}",
            node.name,
            item.as_str()
        )));
    }
    Ok(())
}

fn retain_child_elements(
    node: &Element,
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    for child in node.children() {
        extensions.push(extension(record_kind, record_name, child)?);
    }
    Ok(())
}

fn retain_unknown_node_content(
    node: &Element,
    known_attributes: &[&str],
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    retain_unknown_attributes(node, known_attributes, record_kind, record_name, extensions);
    retain_child_elements(node, record_kind, record_name, extensions)
}

fn set_item_text(
    field: &mut Option<String>,
    node: &Element,
    item: &SoftwareItemName,
    record: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    set_once(field, node, item)?;
    retain_unknown_node_content(node, &[], "software_item", Some(record), extensions)
}

fn required_text(value: Option<String>, field: &str, record: &str) -> crate::Result<String> {
    value.ok_or_else(|| {
        crate::Error::XmlValidation(format!("missing required {field} for software {record:?}"))
    })
}

fn required(node: &Element, field: &str) -> crate::Result<String> {
    node.attributes
        .get(field)
        .cloned()
        .ok_or_else(|| crate::Error::CatalogParse {
            message: format!("missing required {field} attribute on <{}>", node.name),
            record_kind: Some(node.name.clone()),
            record_name: node.attributes.get("name").cloned(),
            line: Some(node.location.line),
            column: Some(node.location.column),
        })
}

fn invalid_value(node: &Element, record: &str, field: &str, value: &str) -> crate::Error {
    crate::Error::CatalogParse {
        message: format!(
            "invalid {field} value {value:?} on <{}> {record:?}",
            node.name
        ),
        record_kind: Some(node.name.clone()),
        record_name: Some(record.to_owned()),
        line: Some(node.location.line),
        column: Some(node.location.column),
    }
}

fn extension(
    record_kind: &str,
    record_name: Option<&str>,
    node: &Element,
) -> crate::Result<XmlExtension> {
    let value = ExtensionValue::encode(node)?;
    Ok(XmlExtension {
        record_kind: record_kind.into(),
        record_name: record_name.map(str::to_owned),
        field_name: format!("element:{}", node.name),
        namespace_uri: None,
        value,
        location: node.location,
    })
}

fn retain_unknown_attributes(
    node: &Element,
    known: &[&str],
    record_kind: &str,
    record_name: Option<&str>,
    extensions: &mut Vec<XmlExtension>,
) {
    for (name, raw_value) in &node.attributes {
        if known.contains(&name.as_str()) {
            continue;
        }
        let (field_name, namespace_uri) = name
            .strip_prefix('{')
            .and_then(|name| name.split_once('}'))
            .map_or_else(
                || (format!("@{name}"), None),
                |(namespace, local_name)| (format!("@{local_name}"), Some(namespace.to_owned())),
            );
        extensions.push(XmlExtension {
            record_kind: record_kind.into(),
            record_name: record_name.map(str::to_owned),
            field_name,
            namespace_uri,
            value: serde_json::json!(raw_value).into(),
            location: node.location,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at<T>(values: &[T], index: usize) -> crate::Result<&T> {
        values.get(index).ok_or_else(|| {
            crate::Error::InvalidPath(format!("software-list fixture is missing item {index}"))
        })
    }

    fn as_rom(component: &SoftwareComponent) -> crate::Result<&SoftwareRom> {
        match component {
            SoftwareComponent::Rom(rom) => Ok(rom),
            SoftwareComponent::Disk(_) => Err(crate::Error::InvalidPath(
                "expected ROM component in software-list fixture".into(),
            )),
        }
    }

    fn disk(component: &SoftwareComponent) -> crate::Result<&SoftwareDisk> {
        match component {
            SoftwareComponent::Disk(disk) => Ok(disk),
            SoftwareComponent::Rom(_) => Err(crate::Error::InvalidPath(
                "expected disk component in software-list fixture".into(),
            )),
        }
    }

    #[test]
    fn fixture_preserves_list_scoped_ids_nested_order_loads_and_clone_relationships()
    -> crate::Result<()> {
        let bytes = include_bytes!("../fixtures/catalog/mame/software-list.xml");
        let catalog = SoftwareListCatalog::parse(bytes)?;
        assert_eq!(catalog.build.as_deref(), Some("0.289-synthetic"));
        assert_eq!(catalog.lists.len(), 2);
        let list = at(&catalog.lists, 0)?;
        assert_eq!(list.name.as_str(), "demo_cart");
        assert_eq!(list.items.len(), 2);
        let item = at(&list.items, 0)?;
        assert_eq!(item.name.as_str(), "demo_game");
        assert_eq!(
            item.clone_of.as_ref().map(SoftwareItemName::as_str),
            Some("demo_original")
        );
        assert_eq!(item.supported, Some(SupportedStatus::Partial));
        assert_eq!(
            item.info
                .iter()
                .filter(|value| value.name == "language")
                .count(),
            2
        );
        assert_eq!(at(&item.shared_features, 0)?.name, "compatibility");
        assert_eq!(item.parts.len(), 2);
        let cart = at(&item.parts, 0)?;
        assert_eq!(cart.interface, "demo_cart");
        assert_eq!(cart.areas.len(), 2);
        let program = at(&cart.areas, 0)?;
        assert_eq!(program.kind, AreaKind::Data);
        assert_eq!(program.name.as_str(), "program");
        assert_eq!(program.declared_size, Some(32));
        assert_eq!(program.width, Some(16));
        assert_eq!(program.endianness, Some(Endianness::Big));
        assert_eq!(program.components.len(), 2);
        let rom = as_rom(at(&program.components, 0)?)?;
        assert_eq!(
            rom.name.as_ref().map(ComponentName::as_str),
            Some("program.bin")
        );
        assert_eq!(rom.load, Some(LoadInstruction::Load16WordSwap));
        assert_eq!(rom.offset, Some(0));
        let no_dump = as_rom(at(&program.components, 1)?)?;
        assert_eq!(no_dump.status, Some(DumpStatus::NoDump));
        assert_eq!(no_dump.sha1, None);
        assert_eq!(no_dump.load, Some(LoadInstruction::Continue));
        let media = at(&cart.areas, 1)?;
        let disk = disk(at(&media.components, 0)?)?;
        assert_eq!(disk.writeable, Some(true));
        let manual_area = at(&at(&item.parts, 1)?.areas, 0)?;
        assert_eq!(manual_area.width, Some(8));
        assert_eq!(manual_area.endianness, Some(Endianness::Little));
        assert!(item.location.line < cart.location.line);
        assert!(cart.location.line < rom.location.line);
        let second_list = at(&catalog.lists, 1)?;
        assert_eq!(at(&second_list.items, 0)?.name.as_str(), "demo_game");
        assert_eq!(second_list.name.as_str(), "demo_flop");
        assert!(catalog.extensions.iter().any(|extension| {
            extension.field_name == "element:future-policy"
                && extension.record_name.as_deref() == Some("demo_cart:demo_game")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.field_name == "@future-flag"
                && extension.value == serde_json::json!("retained").into()
                && extension.record_name.as_deref() == Some("demo_cart:demo_game")
        }));
        Ok(())
    }

    #[test]
    fn parser_accepts_a_single_softwarelist_root_and_rejects_duplicate_item_names()
    -> crate::Result<()> {
        let single = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert_eq!(SoftwareListCatalog::parse(single)?.lists.len(), 1);

        let duplicate = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software><software name="game"><description>Game 2</description><year>2001</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(duplicate).is_err());
        Ok(())
    }

    #[test]
    fn parser_rejects_duplicate_notes_instead_of_dropping_the_first() {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><notes>First</notes><notes>Second</notes><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(xml).is_err());
    }

    #[test]
    fn parser_accepts_metadata_only_software_without_parts() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert!(at(&at(&catalog.lists, 0)?.items, 0)?.parts.is_empty());
        Ok(())
    }

    #[test]
    fn parser_accepts_empty_aggregate_but_not_empty_individual_lists() -> crate::Result<()> {
        let catalog = SoftwareListCatalog::parse(b"<softwarelists/>")?;
        assert!(catalog.lists.is_empty());
        assert!(SoftwareListCatalog::parse(b"<softwarelist name=\"empty\"/>").is_err());
        Ok(())
    }

    #[test]
    fn parser_retains_single_list_attributes_and_rom_extensions() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one" future-root="kept"><software name="game"><description future-text-attribute="retained">Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="game.bin"><future-component-claim value="kept"/></rom><rom loadflag="continue"/></dataarea><diskarea name="media"><disk name="game-disk"><future-disk-claim value="also-kept"/></disk></diskarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(
            catalog
                .extensions
                .iter()
                .filter(|extension| {
                    extension.field_name == "@future-root"
                        && extension.value == serde_json::json!("kept").into()
                })
                .count(),
            1
        );
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_list"
                && extension.field_name == "@future-root"
                && extension.value == serde_json::json!("kept").into()
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_item"
                && extension.field_name == "@future-text-attribute"
                && extension.value == serde_json::json!("retained").into()
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_rom"
                && extension.field_name == "element:future-component-claim"
                && serde_json::from_str::<serde_json::Value>(extension.value.as_str())
                    .is_ok_and(|value| value["attributes"]["value"] == "kept")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_disk"
                && extension.field_name == "element:future-disk-claim"
                && serde_json::from_str::<serde_json::Value>(extension.value.as_str())
                    .is_ok_and(|value| value["attributes"]["value"] == "also-kept")
        }));
        Ok(())
    }

    #[test]
    fn parser_preserves_spec_valid_empty_rom_records_and_rejects_out_of_range_numbers()
    -> crate::Result<()> {
        let empty_rom = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(empty_rom)?;
        let rom = as_rom(
            &at(
                &at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?.areas,
                0,
            )?
            .components[0],
        )?;
        assert_eq!(rom.name, None);
        assert_eq!(rom.status, Some(DumpStatus::Good));
        let empty_name = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name=""/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(empty_name)?;
        let rom = as_rom(
            &at(
                &at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?.areas,
                0,
            )?
            .components[0],
        )?;
        assert_eq!(rom.name.as_ref().map(ComponentName::as_str), Some(""));
        let unnamed_load = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom loadflag="continue"/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(unnamed_load).is_ok());
        assert!(parse_number("9223372036854775808").is_err());
        assert!(parse_number("0x8000000000000000").is_err());
        Ok(())
    }

    #[test]
    fn parser_preserves_unresolved_clone_and_optional_named_values() -> crate::Result<()> {
        let partial = br#"<softwarelist name="partial"><software name="clone" cloneof="omitted_parent" supported="partial"><description>Clone</description><year>2000</year><publisher>Pub</publisher><info name="language"/><sharedfeat name="compatibility"/><part name="cart" interface="cart"/></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(partial)?;
        let item = at(&at(&catalog.lists, 0)?.items, 0)?;

        assert_eq!(
            item.clone_of.as_ref().map(SoftwareItemName::as_str),
            Some("omitted_parent")
        );
        assert_eq!(at(&item.info, 0)?.value, None);
        assert_eq!(at(&item.shared_features, 0)?.value, None);
        Ok(())
    }

    #[test]
    fn parser_materializes_dtd_defaults_and_parses_dipswitches() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><notes>list notes</notes><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="first.bin"/></dataarea><dataarea name="rom" size="2"><rom name="second.bin"/></dataarea><diskarea name="media"><disk name="implicit"/><disk name="explicit" writeable="no"/></diskarea><dipswitch name="Difficulty" tag=":DSW" mask="0x03"><dipvalue name="Easy" value="0x01" default="yes"/><dipvalue name="Hard" value="0x02"/></dipswitch></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(catalog.lists[0].notes.as_deref(), Some("list notes"));
        let item = at(&at(&catalog.lists, 0)?.items, 0)?;
        assert_eq!(item.supported, Some(SupportedStatus::Yes));
        let part = at(&item.parts, 0)?;
        let areas = &part.areas;
        assert_eq!(areas.len(), 3);
        assert_eq!(areas[0].name, areas[1].name);
        assert_eq!(areas[0].declared_size, Some(1));
        assert_eq!(areas[1].declared_size, Some(2));
        assert_eq!(areas[0].width, Some(8));
        assert_eq!(areas[0].endianness, Some(Endianness::Little));
        assert_eq!(
            as_rom(at(&areas[0].components, 0)?)?.status,
            Some(DumpStatus::Good)
        );
        assert_eq!(
            as_rom(at(&areas[1].components, 0)?)?
                .name
                .as_ref()
                .map(ComponentName::as_str),
            Some("second.bin")
        );
        assert_eq!(disk(at(&areas[2].components, 0)?)?.writeable, Some(false));
        assert_eq!(disk(at(&areas[2].components, 1)?)?.writeable, Some(false));
        assert_eq!(
            disk(at(&areas[2].components, 0)?)?.status,
            Some(DumpStatus::Good)
        );
        let switch = at(&part.dipswitches, 0)?;
        assert_eq!(switch.name.as_str(), "Difficulty");
        assert_eq!(switch.tag, ":DSW");
        assert_eq!(switch.mask, "0x03");
        assert_eq!(switch.values[0].name, "Easy");
        assert_eq!(switch.values[0].value, "0x01");
        assert!(switch.values[0].is_default);
        assert!(!switch.values[1].is_default);

        let empty_text_fields = br#"<softwarelist name="empty-text"><software name="game"><description/><year/><publisher/></software></softwarelist>"#;
        let empty_text_catalog = SoftwareListCatalog::parse(empty_text_fields)?;
        let empty_text_item = &empty_text_catalog.lists[0].items[0];
        assert_eq!(empty_text_item.description, "");
        assert_eq!(empty_text_item.year, "");
        assert_eq!(empty_text_item.publisher, "");
        Ok(())
    }

    #[test]
    fn parser_uses_mame_number_bases_and_treats_empty_hashes_as_absent() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="010"><rom name="missing.bin" size="010" offset="010" status="nodump" crc="" sha1=""/></dataarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        let area = at(&at(&at(&catalog.lists, 0)?.items, 0)?.parts, 0)?;
        let area = at(&area.areas, 0)?;
        assert_eq!(area.declared_size, Some(8));
        let rom = as_rom(at(&area.components, 0)?)?;
        assert_eq!(rom.size, Some(8));
        assert_eq!(rom.offset, Some(8));
        assert_eq!(rom.crc, None);
        assert_eq!(rom.sha1, None);
        assert_eq!(rom.status, Some(DumpStatus::NoDump));
        assert_eq!(parse_number("0x10")?, 16);
        assert!(parse_number("08").is_err());
        Ok(())
    }

    #[test]
    fn parser_rejects_external_entity_declarations_and_invalid_hashes() {
        let entity = br#"<!DOCTYPE softwarelist [<!ENTITY external SYSTEM "file:///etc/passwd">]><softwarelist name="one"><software name="game"><description>&external;</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(matches!(
            SoftwareListCatalog::parse(entity),
            Err(crate::Error::XmlEntityNotAllowed)
        ));

        let invalid_hash = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="16"><rom name="game.bin" crc="not-hex"/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(invalid_hash).is_err());

        let encoded = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><!DOCTYPE softwarelist [<!ENTITY secret \"expanded\">]><softwarelist name=\"one\"><software name=\"game\"><description>&secret;</description><year>2000</year><publisher>Pub</publisher><part name=\"cart\" interface=\"cart\"/></software></softwarelist>";
        let mut utf16 = vec![0xff, 0xfe];
        for unit in encoded.encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(matches!(
            SoftwareListCatalog::parse(&utf16),
            Err(crate::Error::XmlEntityNotAllowed)
        ));

        let mut no_bom = Vec::new();
        for unit in
            "<!DOCTYPE softwarelist [<!ENTITY secret \"expanded\">]><softwarelist name=\"one\"/>"
                .encode_utf16()
        {
            no_bom.extend_from_slice(&unit.to_le_bytes());
        }
        assert!(matches!(
            SoftwareListCatalog::parse(&no_bom),
            Err(crate::Error::XmlEntityNotAllowed)
        ));
    }

    #[test]
    fn invalid_enumerated_values_report_the_record_and_source_location() {
        let source =
            String::from_utf8_lossy(include_bytes!("../fixtures/catalog/mame/software-list.xml"));
        for (original, replacement, expected_kind, expected_record) in [
            (
                "supported=\"partial\"",
                "supported=\"unsupported\"",
                "software",
                "demo_cart:demo_game",
            ),
            (
                "status=\"nodump\"",
                "status=\"unknown\"",
                "rom",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "loadflag=\"continue\"",
                "loadflag=\"unknown\"",
                "rom",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "writeable=\"yes\"",
                "writeable=\"unknown\"",
                "disk",
                "demo_cart:demo_game:cart:disk:media",
            ),
            (
                "width=\"16\"",
                "width=\"7\"",
                "dataarea",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "endianness=\"big\"",
                "endianness=\"middle\"",
                "dataarea",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "size=\"0x20\"",
                "size=\"invalid\"",
                "dataarea",
                "demo_cart:demo_game:cart:data:program",
            ),
            (
                "crc=\"12345678\"",
                "crc=\"invalid\"",
                "rom",
                "demo_cart:demo_game:cart:data:program",
            ),
        ] {
            let xml = source.replacen(original, replacement, 1);
            let result = SoftwareListCatalog::parse(xml.as_bytes());
            assert!(matches!(
                result,
                Err(crate::Error::CatalogParse {
                    record_kind: Some(kind),
                    record_name: Some(record),
                    line: Some(line),
                    column: Some(column),
                    ..
                }) if kind == expected_kind && record == expected_record && line > 0 && column > 0
            ));
        }
    }
}
