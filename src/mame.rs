use std::collections::BTreeMap;

use quick_xml::events::Event;

use crate::{
    disk::{DiskDigestScope, DiskIdentitySha1, DiskName, DiskRequirement, ParentDiskName},
    domain::AssetRole,
    logiqx::RecordLocation,
    xml_reader::{self, NodeBudget},
};

use crate::xml_reader::Element;

mod specification;
pub use specification::*;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(test)]
pub struct MameCatalog {
    pub build: Option<String>,
    pub debug: bool,
    pub config_version: Option<String>,
    pub machines: Vec<Machine>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub parent: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub location: RecordLocation,
    pub facts: MachineFacts,
    pub assets: Vec<MachineAsset>,
    pub device_refs: Vec<DeviceReference>,
    pub switches: Vec<MachineSwitch>,
    pub bios_sets: Vec<MachineBiosSet>,
    pub specification: Vec<MachineSpecificationElement>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineFacts {
    pub source_file: Option<String>,
    pub description: String,
    pub description_location: RecordLocation,
    pub year: Option<String>,
    pub year_location: Option<RecordLocation>,
    pub manufacturer: Option<String>,
    pub manufacturer_location: Option<RecordLocation>,
    pub flags: MachineFlags,
    pub attributes_location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineFlags(u8);

impl MachineFlags {
    const DEVICE: u8 = 1 << 0;
    const RUNNABLE: u8 = 1 << 1;
    const BIOS: u8 = 1 << 2;
    const MECHANICAL: u8 = 1 << 3;
    const CONSUMABLE: u8 = 1 << 4;

    fn parse(attributes: &BTreeMap<String, String>) -> crate::Result<Self> {
        let mut flags = 0;
        for (attribute, flag, default) in [
            ("isdevice", Self::DEVICE, false),
            ("runnable", Self::RUNNABLE, true),
            ("isbios", Self::BIOS, false),
            ("ismechanical", Self::MECHANICAL, false),
            ("isconsumable", Self::CONSUMABLE, false),
        ] {
            if parse_mame_boolean_with_default(attributes.get(attribute), default, attribute)? {
                flags |= flag;
            }
        }
        Ok(Self(flags))
    }

    #[must_use]
    pub const fn is_device(self) -> bool {
        self.0 & Self::DEVICE != 0
    }
    #[must_use]
    pub const fn is_runnable(self) -> bool {
        self.0 & Self::RUNNABLE != 0
    }
    #[must_use]
    pub const fn is_bios(self) -> bool {
        self.0 & Self::BIOS != 0
    }
    #[must_use]
    pub const fn is_mechanical(self) -> bool {
        self.0 & Self::MECHANICAL != 0
    }
    #[must_use]
    pub const fn is_consumable(self) -> bool {
        self.0 & Self::CONSUMABLE != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineBiosSet {
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub location: RecordLocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineSwitchKind {
    DipSwitch,
    Configuration,
}

impl MachineSwitchKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DipSwitch => "dipswitch",
            Self::Configuration => "configuration",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitch {
    pub kind: MachineSwitchKind,
    pub name: String,
    pub tag: String,
    pub mask: u64,
    pub location: RecordLocation,
    pub condition: Option<MachineCondition>,
    pub locations: Vec<MachineSwitchLocation>,
    pub values: Vec<MachineSwitchValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchLocation {
    pub name: String,
    pub number: String,
    pub inverted: bool,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchValue {
    pub name: String,
    pub value: u64,
    pub default: bool,
    pub condition: Option<MachineCondition>,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceReference {
    pub tag: String,
    pub name: String,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineAsset {
    pub name: String,
    pub role: AssetRole,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub merge_name: Option<String>,
    pub dump_status: MameDumpStatus,
    pub disk_requirement: Option<DiskRequirement>,
    pub location: RecordLocation,
    pub attributes: MameAssetAttributes,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MameDumpStatus {
    Good,
    BadDump,
    NoDump,
}

impl MameDumpStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::BadDump => "baddump",
            Self::NoDump => "nodump",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MameOffset(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MameBoolean {
    #[default]
    No,
    Yes,
}

impl MameBoolean {
    pub(crate) const fn as_bool(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MameAssetAttributes {
    pub region: Option<String>,
    pub bios: Option<String>,
    pub offset: Option<MameOffset>,
    pub optional: MameBoolean,
    pub sound_only: Option<MameBoolean>,
    pub dispose: Option<MameBoolean>,
    pub load_flag: Option<String>,
    pub value: Option<String>,
    pub inverted: Option<MameBoolean>,
    pub ovha: Option<String>,
    pub no_thread: Option<MameBoolean>,
    pub disk_index: Option<String>,
    pub writable: Option<MameBoolean>,
    pub writeable: Option<MameBoolean>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlExtension {
    pub record_kind: String,
    pub record_name: Option<String>,
    pub field_name: String,
    pub namespace_uri: Option<String>,
    pub value: ExtensionValue,
    pub location: RecordLocation,
}

/// Valid JSON encoded once, without retaining a second tree of JSON objects.
/// The private representation prevents arbitrary text or double encoding at storage boundaries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionValue(String);

impl ExtensionValue {
    pub(crate) fn encode(value: &impl serde::Serialize) -> crate::Result<Self> {
        serde_json::to_string(value).map(Self).map_err(Into::into)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<serde_json::Value> for ExtensionValue {
    fn from(value: serde_json::Value) -> Self {
        Self(value.to_string())
    }
}

pub struct MameHeader {
    pub build: Option<String>,
    pub debug: bool,
    pub config_version: Option<String>,
    pub location: RecordLocation,
    pub extensions: Vec<XmlExtension>,
}

pub enum MameRecord {
    Machine(Box<Machine>),
    Extension(XmlExtension),
}

/// Only a successful traversal through the closing root and EOF produces this state.
pub struct ValidatedMame<S>(S);

impl<S> ValidatedMame<S> {
    pub(crate) fn into_inner(self) -> S {
        self.0
    }
}

/// Consume each record before reading the next. Only a complete document returns
/// a validated sink. Parser errors convert through `E::from`; consumers can use
/// a separate error variant for persistence failures.
pub fn read_with<S, E: From<crate::Error>>(
    bytes: &[u8],
    start: impl FnOnce(MameHeader) -> Result<S, E>,
    mut consume: impl FnMut(&mut S, MameRecord) -> Result<(), E>,
) -> Result<ValidatedMame<S>, E> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let (namespace, root_start, empty) = loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::Start(start) => break (namespace, start, false),
                Event::Empty(start) => break (namespace, start, true),
                Event::Text(text)
                    if text
                        .xml10_content()
                        .bytes()
                        .all(quick_xml::utils::is_whitespace) => {}
                Event::Decl(_) | Event::DocType(_) | Event::Comment(_) | Event::PI(_) => {}
                Event::Eof => {
                    return Err(crate::Error::XmlValidation("missing document root".into()).into());
                }
                _ => {
                    return Err(crate::Error::XmlValidation(
                        "content before the document root".into(),
                    )
                    .into());
                }
            }
        };
        let mut budget = NodeBudget::with_limit(xml_reader::MAX_MAME_XML_NODES);
        let root = xml_reader::element_from_start(
            reader,
            namespace,
            &root_start,
            &mut budget,
            0,
            positions,
        )?;
        if root.name != "mame" {
            return Err(crate::Error::XmlValidation(format!(
                "expected <mame>, found <{}>",
                root.name
            ))
            .into());
        }
        let build = root.attributes.get("build").cloned();
        let debug = parse_mame_boolean(root.attributes.get("debug"), "debug")?;
        let config_version = root.attributes.get("mameconfig").cloned();
        let extensions = root
            .attributes
            .iter()
            .filter(|(name, _)| !["build", "debug", "mameconfig"].contains(&name.as_str()))
            .map(|(name, value)| {
                let (field_name, namespace_uri) = attribute_name(name);
                XmlExtension {
                    record_kind: "document".into(),
                    record_name: None,
                    field_name,
                    namespace_uri,
                    value: serde_json::json!(value).into(),
                    location: root.location,
                }
            })
            .collect();
        let mut sink = start(MameHeader {
            build,
            debug,
            config_version,
            location: root.location,
            extensions,
        })?;
        parse_machine_records(reader, positions, &mut budget, empty, true, |record| {
            consume(&mut sink, record)
        })?;
        Ok(ValidatedMame(sink))
    })
}

#[cfg(test)]
impl MameCatalog {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        read_with::<_, crate::Error>(
            bytes,
            |header| {
                Ok(Self {
                    build: header.build,
                    debug: header.debug,
                    config_version: header.config_version,
                    machines: Vec::new(),
                    extensions: header.extensions,
                })
            },
            |catalog, record| {
                match record {
                    MameRecord::Machine(machine) => catalog.machines.push(*machine),
                    MameRecord::Extension(extension) => catalog.extensions.push(extension),
                }
                Ok(())
            },
        )
        .map(ValidatedMame::into_inner)
    }
}

fn parse_machine_records<E: From<crate::Error>>(
    reader: &mut quick_xml::reader::NsReader<&[u8]>,
    positions: &mut xml_reader::PositionMap<'_>,
    budget: &mut NodeBudget,
    empty: bool,
    retain_extensions: bool,
    mut consume: impl FnMut(MameRecord) -> Result<(), E>,
) -> Result<(), E> {
    let mut machine_names = std::collections::HashSet::new();
    if !empty {
        loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            let node = match event {
                Event::Start(child) => {
                    xml_reader::read_element(reader, namespace, &child, budget, 1, positions)?
                }
                Event::Empty(child) => {
                    xml_reader::element_from_start(reader, namespace, &child, budget, 1, positions)?
                }
                Event::End(_) => break,
                Event::Eof => {
                    return Err(crate::Error::XmlValidation(
                        "unexpected end of input inside <mame>".into(),
                    )
                    .into());
                }
                _ => continue,
            };
            let record = parse_record(&node, &mut machine_names, retain_extensions)?;
            drop(node);
            if let Some(record) = record {
                consume(record)?;
            }
        }
    }
    loop {
        match xml_reader::next(reader, positions)?.1 {
            Event::Eof => break,
            Event::Text(text)
                if text
                    .xml10_content()
                    .bytes()
                    .all(quick_xml::utils::is_whitespace) => {}
            Event::Comment(_) | Event::PI(_) => {}
            _ => {
                return Err(
                    crate::Error::XmlValidation("content after the document root".into()).into(),
                );
            }
        }
    }
    if machine_names.is_empty() {
        return Err(
            crate::Error::XmlValidation("MAME document has no machine records".into()).into(),
        );
    }
    Ok(())
}

fn parse_record(
    node: &Element,
    machine_names: &mut std::collections::HashSet<String>,
    retain_extensions: bool,
) -> crate::Result<Option<MameRecord>> {
    if node.name == "machine" {
        let machine = parse_machine(node, retain_extensions)?;
        if !machine_names.insert(machine.name.clone()) {
            return Err(crate::Error::XmlValidation(format!(
                "duplicate MAME machine name {:?}",
                machine.name
            )));
        }
        Ok(Some(MameRecord::Machine(Box::new(machine))))
    } else if retain_extensions {
        extension("document", None, node)
            .map(MameRecord::Extension)
            .map(Some)
    } else {
        Ok(None)
    }
}

pub fn parse_xml_element(bytes: &[u8]) -> crate::Result<Element> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let mut budget = NodeBudget::default();
        let root = loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::Start(start) => {
                    break xml_reader::read_element(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        0,
                        positions,
                    )?;
                }
                Event::Empty(start) => {
                    break xml_reader::element_from_start(
                        reader,
                        namespace,
                        &start,
                        &mut budget,
                        0,
                        positions,
                    )?;
                }
                Event::Eof => {
                    return Err(crate::Error::XmlValidation("missing document root".into()));
                }
                _ => {}
            }
        };
        loop {
            match xml_reader::next(reader, positions)?.1 {
                Event::Eof => return Ok(root),
                Event::Text(text) if text.xml10_content().trim().is_empty() => {}
                Event::Comment(_) | Event::PI(_) => {}
                _ => {
                    return Err(crate::Error::XmlValidation(
                        "content after the document root".into(),
                    ));
                }
            }
        }
    })
}

fn parse_device_reference(node: &Element) -> crate::Result<DeviceReference> {
    Ok(DeviceReference {
        tag: required(node, "tag")?.to_owned(),
        name: required(node, "name")?.to_owned(),
        location: node.location,
    })
}

fn machine_attribute_extensions(name: &str, node: &Element) -> Vec<XmlExtension> {
    node.attributes
        .iter()
        .filter(|(key, _)| {
            ![
                "name",
                "sourcefile",
                "cloneof",
                "romof",
                "sampleof",
                "isdevice",
                "runnable",
                "isbios",
                "ismechanical",
                "isconsumable",
            ]
            .contains(&key.as_str())
        })
        .map(|(key, val)| {
            let (field_name, namespace_uri) = attribute_name(key);
            XmlExtension {
                record_kind: "machine".into(),
                record_name: Some(name.into()),
                field_name,
                namespace_uri,
                value: serde_json::json!(val).into(),
                location: node.location,
            }
        })
        .collect()
}

struct MachineChildren {
    description: String,
    description_location: RecordLocation,
    year: Option<String>,
    year_location: Option<RecordLocation>,
    manufacturer: Option<String>,
    manufacturer_location: Option<RecordLocation>,
    bios_sets: Vec<MachineBiosSet>,
    specification: Vec<MachineSpecificationElement>,
    device_refs: Vec<DeviceReference>,
    switches: Vec<MachineSwitch>,
    assets: Vec<MachineAsset>,
    extensions: Vec<XmlExtension>,
}

fn parse_machine(node: &Element, retain_extensions: bool) -> crate::Result<Machine> {
    let name = required(node, "name")?;
    let flags = MachineFlags::parse(&node.attributes)?;
    let mut children = parse_machine_children(node, name, retain_extensions)?;
    if retain_extensions {
        children
            .extensions
            .extend(machine_attribute_extensions(name, node));
    }
    Ok(Machine {
        name: name.into(),
        parent: node.attributes.get("cloneof").cloned(),
        rom_of: node.attributes.get("romof").cloned(),
        sample_of: node.attributes.get("sampleof").cloned(),
        location: node.location,
        facts: MachineFacts {
            source_file: node.attributes.get("sourcefile").cloned(),
            description: children.description,
            description_location: children.description_location,
            year: children.year,
            year_location: children.year_location,
            manufacturer: children.manufacturer,
            manufacturer_location: children.manufacturer_location,
            flags,
            attributes_location: node.location,
        },
        assets: children.assets,
        device_refs: children.device_refs,
        switches: children.switches,
        bios_sets: children.bios_sets,
        specification: children.specification,
        extensions: children.extensions,
    })
}

fn parse_machine_children(
    node: &Element,
    machine_name: &str,
    retain_extensions: bool,
) -> crate::Result<MachineChildren> {
    let mut description = None;
    let mut description_location = None;
    let mut year = None;
    let mut year_location = None;
    let mut manufacturer = None;
    let mut manufacturer_location = None;
    let mut bios_sets = Vec::new();
    let mut specification = Vec::new();
    let mut device_refs = Vec::new();
    let mut switches = Vec::new();
    let mut assets = Vec::new();
    let mut extensions = Vec::new();
    let mut singleton_specification_elements = std::collections::BTreeSet::new();
    let mut machine_text_fields = std::collections::HashSet::new();
    for (element_order, child) in node.children().enumerate() {
        match child.name.as_str() {
            "description" | "year" | "manufacturer" => {
                if !machine_text_fields.insert(child.name.as_str()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate machine {} field for {:?}",
                        child.name, machine_name
                    )));
                }
                let text = child.direct_text();
                match child.name.as_str() {
                    "description" => {
                        description = Some(text);
                        description_location = Some(child.location);
                    }
                    "year" => {
                        year = Some(text);
                        year_location = Some(child.location);
                    }
                    "manufacturer" => {
                        manufacturer = Some(text);
                        manufacturer_location = Some(child.location);
                    }
                    _ => {}
                }
                if retain_extensions && child.children().next().is_some() {
                    extensions.push(extension("machine", Some(machine_name), child)?);
                }
            }
            "biosset" => bios_sets.push(parse_machine_bios_set(child)?),
            "device_ref" => device_refs.push(parse_device_reference(child)?),
            "dipswitch" => {
                switches.push(parse_machine_switch(child, MachineSwitchKind::DipSwitch)?);
            }
            "configuration" => switches.push(parse_machine_switch(
                child,
                MachineSwitchKind::Configuration,
            )?),
            "rom" | "disk" => assets.push(parse_asset(child, retain_extensions)?),
            "sound" | "input" | "driver" => {
                if !singleton_specification_elements.insert(child.name.as_str()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "MAME machine {machine_name:?} contains duplicate <{}>",
                        child.name
                    )));
                }
                specification.push(parse_machine_specification(child, element_order)?);
            }
            "sample" | "chip" | "display" | "port" | "adjuster" | "feature" | "device" | "slot"
            | "softwarelist" | "ramoption" => {
                specification.push(parse_machine_specification(child, element_order)?);
            }
            _ if retain_extensions => {
                extensions.push(extension("machine", Some(machine_name), child)?);
            }
            _ => {}
        }
        collect_unknown_child_attributes(child, retain_extensions, &mut extensions);
    }
    let description = description.ok_or_else(|| {
        crate::Error::XmlValidation(format!(
            "MAME machine {machine_name:?} is missing description"
        ))
    })?;
    let description_location = description_location.ok_or_else(|| {
        crate::Error::XmlValidation(format!(
            "MAME machine {machine_name:?} is missing description"
        ))
    })?;
    Ok(MachineChildren {
        description,
        description_location,
        year,
        year_location,
        manufacturer,
        manufacturer_location,
        bios_sets,
        specification,
        device_refs,
        switches,
        assets,
        extensions,
    })
}

fn collect_unknown_child_attributes(
    child: &Element,
    retain_extensions: bool,
    extensions: &mut Vec<XmlExtension>,
) {
    if !retain_extensions || matches!(child.name.as_str(), "rom" | "disk") {
        return;
    }
    for (key, value) in &child.attributes {
        if known_child_attribute(&child.name, key) {
            continue;
        }
        let (field_name, namespace_uri) = attribute_name(key);
        extensions.push(XmlExtension {
            record_kind: child.name.clone(),
            record_name: child.attributes.get("name").cloned(),
            field_name,
            namespace_uri,
            value: serde_json::json!(value).into(),
            location: child.location,
        });
    }
}

fn parse_machine_specification(
    child: &Element,
    element_order: usize,
) -> crate::Result<MachineSpecificationElement> {
    Ok(MachineSpecificationElement {
        element_order: i64::try_from(element_order).map_err(|_| {
            crate::Error::InvalidPath("MAME element order exceeds SQLite INTEGER".into())
        })?,
        value: specification::parse_element(child)?,
    })
}

fn parse_machine_bios_set(node: &Element) -> crate::Result<MachineBiosSet> {
    Ok(MachineBiosSet {
        name: required(node, "name")?.to_owned(),
        description: node.attributes.get("description").cloned(),
        is_default: parse_mame_boolean(node.attributes.get("default"), "biosset default")?,
        location: node.location,
    })
}

fn parse_mame_boolean(value: Option<&String>, field: &str) -> crate::Result<bool> {
    parse_mame_boolean_with_default(value, false, field)
}

fn parse_optional_mame_boolean(
    value: Option<&String>,
    field: &str,
) -> crate::Result<Option<MameBoolean>> {
    value
        .map(|value| match value.as_str() {
            "no" => Ok(MameBoolean::No),
            "yes" => Ok(MameBoolean::Yes),
            other => Err(crate::Error::XmlValidation(format!(
                "invalid MAME {field} value {other:?}"
            ))),
        })
        .transpose()
}

fn parse_mame_boolean_with_default(
    value: Option<&String>,
    default: bool,
    field: &str,
) -> crate::Result<bool> {
    match value.map(String::as_str) {
        None => Ok(default),
        Some("no") => Ok(false),
        Some("yes") => Ok(true),
        Some(other) => Err(crate::Error::XmlValidation(format!(
            "invalid MAME {field} value {other:?}"
        ))),
    }
}

fn parse_machine_switch(node: &Element, kind: MachineSwitchKind) -> crate::Result<MachineSwitch> {
    let name = required(node, "name")?.to_owned();
    let tag = required(node, "tag")?.to_owned();
    let mask = parse_mame_integer(required(node, "mask")?)?;
    let mut locations = Vec::new();
    let mut values = Vec::new();
    let mut condition = None;
    for child in node.children() {
        match child.name.as_str() {
            "condition" => condition = Some(specification::parse_condition(child)?),
            "diplocation" | "conflocation" => locations.push(MachineSwitchLocation {
                name: required(child, "name")?.to_owned(),
                number: required(child, "number")?.to_owned(),
                inverted: parse_mame_boolean(
                    child.attributes.get("inverted"),
                    "switch location inverted",
                )?,
                location: child.location,
            }),
            "dipvalue" | "confsetting" => values.push(MachineSwitchValue {
                name: required(child, "name")?.to_owned(),
                value: parse_mame_integer(required(child, "value")?)?,
                default: parse_mame_boolean(
                    child.attributes.get("default"),
                    "switch value default",
                )?,
                condition: child
                    .children()
                    .find(|nested| nested.name == "condition")
                    .map(specification::parse_condition)
                    .transpose()?,
                location: child.location,
            }),
            _ => {}
        }
    }
    Ok(MachineSwitch {
        kind,
        name,
        tag,
        mask,
        location: node.location,
        condition,
        locations,
        values,
    })
}

fn parse_mame_integer(value: &str) -> crate::Result<u64> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || {
                value
                    .parse::<u64>()
                    .or_else(|_| u64::from_str_radix(value, 16))
            },
            |hex| u64::from_str_radix(hex, 16),
        );
    parsed.map_err(|_| crate::Error::XmlValidation(format!("invalid MAME integer {value:?}")))
}

fn parse_mame_asset_attributes(node: &Element) -> crate::Result<MameAssetAttributes> {
    let value = |name: &str| node.attributes.get(name).cloned();
    let number = |name: &str| {
        node.attributes
            .get(name)
            .map(|value| parse_mame_integer(value).map(MameOffset))
            .transpose()
    };
    let boolean = |name: &str| parse_optional_mame_boolean(node.attributes.get(name), name);

    Ok(MameAssetAttributes {
        region: value("region"),
        bios: value("bios"),
        offset: number("offset")?,
        optional: boolean("optional")?.unwrap_or_default(),
        sound_only: boolean("soundonly")?,
        dispose: boolean("dispose")?,
        load_flag: value("loadflag"),
        value: value("value"),
        inverted: boolean("inverted")?,
        ovha: value("ovha"),
        no_thread: boolean("nothread")?,
        disk_index: value("index"),
        writable: boolean("writable")?.or_else(|| (node.name == "disk").then_some(MameBoolean::No)),
        writeable: boolean("writeable")?,
    })
}

fn parse_asset(node: &Element, retain_extensions: bool) -> crate::Result<MachineAsset> {
    let name = required(node, "name")?;
    let size = node
        .attributes
        .get("size")
        .map(|size| {
            size.parse::<u64>().map_err(|_| {
                crate::Error::XmlValidation(format!("invalid MAME asset size {size:?}"))
            })
        })
        .transpose()?;
    let sha1 = node
        .attributes
        .get("sha1")
        .map(|digest| decode_hex(digest))
        .transpose()?;
    let crc = node
        .attributes
        .get("crc")
        .map(|digest| decode_hex_sized(digest, 8))
        .transpose()?;
    let md5 = node
        .attributes
        .get("md5")
        .map(|digest| decode_hex_sized(digest, 32))
        .transpose()?;
    let merge_name = node.attributes.get("merge").cloned();
    let dump_status = match node.attributes.get("status").map(String::as_str) {
        None | Some("good") => MameDumpStatus::Good,
        Some("baddump") => MameDumpStatus::BadDump,
        Some("nodump") => MameDumpStatus::NoDump,
        Some(other) => {
            return Err(crate::Error::XmlValidation(format!(
                "invalid MAME asset status {other:?}"
            )));
        }
    };
    let disk_requirement = parse_disk_requirement(node, name, sha1.as_deref())?;
    let attributes = parse_mame_asset_attributes(node)?;
    let mut extensions = Vec::new();
    if retain_extensions {
        for child in node.children() {
            extensions.push(extension(node.name.as_str(), Some(name), child)?);
        }
        for (key, val) in &node.attributes {
            if !known_asset_attribute(&node.name, key) {
                let (field_name, namespace_uri) = attribute_name(key);
                extensions.push(XmlExtension {
                    record_kind: node.name.clone(),
                    record_name: Some(name.into()),
                    field_name,
                    namespace_uri,
                    value: serde_json::json!(val).into(),
                    location: node.location,
                });
            }
        }
    }
    Ok(MachineAsset {
        name: name.into(),
        role: if node.name == "rom" {
            AssetRole::Rom
        } else {
            AssetRole::Disk
        },
        size,
        crc,
        md5,
        sha1,
        merge_name,
        dump_status,
        disk_requirement,
        location: node.location,
        attributes,
        extensions,
    })
}

fn parse_disk_requirement(
    node: &Element,
    name: &str,
    sha1: Option<&[u8]>,
) -> crate::Result<Option<DiskRequirement>> {
    if node.name != "disk" {
        return Ok(None);
    }
    let expected_sha1 = sha1
        .map(<[u8; 20]>::try_from)
        .transpose()
        .map_err(|_| crate::Error::XmlValidation("invalid MAME disk SHA-1 length".into()))?
        .map(DiskIdentitySha1::new);
    let requirement = DiskRequirement::new(
        DiskName::new(name),
        expected_sha1,
        DiskDigestScope::ChdHeaderSha1,
    );
    Ok(Some(match node.attributes.get("merge") {
        Some(parent) => requirement.with_parent(ParentDiskName::new(parent.clone())),
        None => requirement,
    }))
}

fn extension(kind: &str, record_name: Option<&str>, node: &Element) -> crate::Result<XmlExtension> {
    let (field_name, namespace_uri) = attribute_name(&node.name);
    Ok(XmlExtension {
        record_kind: kind.into(),
        record_name: record_name.map(str::to_owned),
        field_name: format!("element:{field_name}"),
        namespace_uri,
        value: ExtensionValue::encode(node)?,
        location: node.location,
    })
}

fn known_child_attribute(element: &str, attribute: &str) -> bool {
    match element {
        "biosset" => ["name", "description", "default"].contains(&attribute),
        "device_ref" => ["name", "tag"].contains(&attribute),
        "sample" | "slot" => ["name"].contains(&attribute),
        "chip" => ["name", "tag", "type", "clock"].contains(&attribute),
        "display" => [
            "tag", "type", "rotate", "flipx", "width", "height", "refresh", "pixclock", "htotal",
            "hbend", "hbstart", "vtotal", "vbend", "vbstart",
        ]
        .contains(&attribute),
        "sound" => ["channels"].contains(&attribute),
        "input" => ["service", "tilt", "players", "coins"].contains(&attribute),
        "port" => ["tag"].contains(&attribute),
        "adjuster" | "ramoption" => ["name", "default"].contains(&attribute),
        "driver" => [
            "status",
            "emulation",
            "cocktail",
            "savestate",
            "requiresartwork",
            "unofficial",
            "nosoundhardware",
            "incomplete",
        ]
        .contains(&attribute),
        "feature" => ["type", "status", "overall"].contains(&attribute),
        "device" => ["type", "tag", "fixed_image", "mandatory", "interface"].contains(&attribute),
        "softwarelist" => ["tag", "name", "status", "filter"].contains(&attribute),
        "dipswitch" | "configuration" => ["name", "tag", "mask"].contains(&attribute),
        "diplocation" | "conflocation" => ["name", "number", "inverted"].contains(&attribute),
        "dipvalue" | "confsetting" => ["name", "value", "default"].contains(&attribute),
        "rom" => [
            "name",
            "size",
            "sha1",
            "crc",
            "md5",
            "merge",
            "region",
            "bios",
            "status",
            "offset",
            "optional",
            "soundonly",
            "dispose",
            "loadflag",
            "value",
            "inverted",
            "ovha",
            "nothread",
        ]
        .contains(&attribute),
        "disk" => [
            "name",
            "sha1",
            "merge",
            "region",
            "index",
            "writable",
            "writeable",
            "status",
            "optional",
        ]
        .contains(&attribute),
        _ => false,
    }
}

fn known_asset_attribute(element: &str, attribute: &str) -> bool {
    match element {
        "rom" => [
            "name",
            "size",
            "sha1",
            "crc",
            "md5",
            "merge",
            "region",
            "bios",
            "status",
            "offset",
            "optional",
            "soundonly",
            "dispose",
            "loadflag",
            "value",
            "inverted",
            "ovha",
            "nothread",
        ]
        .contains(&attribute),
        "disk" => [
            "name",
            "sha1",
            "merge",
            "region",
            "index",
            "writable",
            "writeable",
            "status",
            "optional",
        ]
        .contains(&attribute),
        _ => false,
    }
}

fn required<'a>(element: &'a Element, name: &str) -> crate::Result<&'a str> {
    element
        .attributes
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| {
            crate::Error::XmlValidation(format!(
                "<{}> is missing required {name:?} attribute",
                element.name
            ))
        })
}

fn attribute_name(name: &str) -> (String, Option<String>) {
    name.strip_prefix('{')
        .and_then(|name| name.split_once('}'))
        .map_or_else(
            || (name.to_owned(), None),
            |(namespace, local)| (local.to_owned(), Some(namespace.to_owned())),
        )
}

fn decode_hex(value: &str) -> crate::Result<Vec<u8>> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(crate::Error::XmlValidation(format!(
            "invalid MAME SHA-1 {value:?}"
        )));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|error| crate::Error::XmlValidation(error.to_string()))
        })
        .collect()
}

fn decode_hex_sized(value: &str, length: usize) -> crate::Result<Vec<u8>> {
    if value.len() != length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(crate::Error::XmlValidation(format!(
            "invalid MAME hexadecimal digest {value:?}"
        )));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|error| crate::Error::XmlValidation(error.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::xml_reader::{MAX_XML_DEPTH, MAX_XML_NODES};

    use super::*;

    #[test]
    fn streaming_delivers_records_before_a_late_parse_error() {
        let mut names = Vec::new();
        let result = read_with::<_, crate::Error>(
            br#"<mame><machine name="first"><description>First</description></machine><machine name="broken">"#,
            |_| Ok(()),
            |(), record| {
                if let MameRecord::Machine(machine) = record {
                    names.push(machine.name);
                }
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(names, ["first"]);
    }

    #[test]
    fn content_before_the_root_never_starts_publication() {
        for prefix in ["junk", "<![CDATA[junk]]>", "&amp;", "\u{a0}"] {
            let xml = format!(
                "{prefix}<mame><machine name='x'><description>X</description></machine></mame>"
            );
            let mut started = false;
            let result = read_with::<_, crate::Error>(
                xml.as_bytes(),
                |_| {
                    started = true;
                    Ok(())
                },
                |(), _| Ok(()),
            );
            assert!(result.is_err(), "accepted invalid prefix {prefix:?}");
            assert!(!started);
        }
    }

    #[test]
    fn only_xml_whitespace_is_allowed_outside_the_root() {
        for whitespace in [" ", "\t", "\r", "\n", " \t\r\n"] {
            let xml = format!(
                "{whitespace}<mame><machine name='x'><description>X</description></machine></mame>{whitespace}"
            );
            assert!(MameCatalog::parse(xml.as_bytes()).is_ok());
        }
        for suffix in ["\u{a0}", "\u{2003}", "\u{85}"] {
            let xml = format!(
                "<mame><machine name='x'><description>X</description></machine></mame>{suffix}"
            );
            assert!(
                MameCatalog::parse(xml.as_bytes()).is_err(),
                "accepted invalid suffix {suffix:?}"
            );
        }
    }

    #[test]
    fn encoded_extension_preserves_existing_json_bytes() -> crate::Result<()> {
        let node = parse_xml_element(
            br#"<future xmlns:x="urn:future" z="&amp;" a="one">before<x:item/>after</future>"#,
        )?;
        assert_eq!(
            ExtensionValue::encode(&node)?.as_str(),
            serde_json::to_value(&node)?.to_string(),
        );
        Ok(())
    }

    #[test]
    fn extension_tree_keeps_mixed_text_and_element_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let missing = || std::io::Error::other("synthetic XML structure is incomplete");
        let root = parse_xml_element(
            br#"<mame><machine name="x"><description>X</description><future>before<x/>after</future></machine></mame>"#,
        )?;
        let machine = root.children().next().ok_or_else(missing)?;
        let future = machine
            .children()
            .find(|child| child.name == "future")
            .ok_or_else(missing)?;
        let serialized = serde_json::to_value(future)?;
        let content = serialized
            .get("content")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(missing)?;
        assert_eq!(content.len(), 3);
        assert!(matches!(content.first(), Some(item) if item["value"] == "before"));
        assert!(matches!(content.get(1), Some(item) if item["kind"] == "element"));
        assert!(matches!(content.get(2), Some(item) if item["value"] == "after"));
        Ok(())
    }

    #[test]
    fn deeply_nested_documents_fail_with_a_parse_error() {
        let mut xml = String::from("<mame>");
        for _ in 0..=MAX_XML_DEPTH {
            xml.push_str("<x>");
        }
        for _ in 0..=MAX_XML_DEPTH {
            xml.push_str("</x>");
        }
        xml.push_str("</mame>");
        assert!(parse_xml_element(xml.as_bytes()).is_err());
    }

    #[test]
    fn documents_with_too_many_elements_fail_before_building_the_tree() {
        let mut xml = String::from("<mame>");
        for _ in 0..=MAX_XML_NODES {
            xml.push_str("<x/>");
        }
        xml.push_str("</mame>");
        assert!(parse_xml_element(xml.as_bytes()).is_err());
    }

    #[test]
    fn duplicate_machine_text_fields_are_rejected() {
        let xml = br#"<mame><machine name="duplicate"><description>one</description><description>two</description></machine></mame>"#;
        assert!(MameCatalog::parse(xml).is_err());
    }

    #[test]
    fn mame_asset_spec_attributes_are_parsed_as_typed_facts() -> crate::Result<()> {
        let rom = parse_xml_element(
            br#"<rom name="boot.bin" region="maincpu" bios="rev-a" offset="0x100" optional="yes" soundonly="no" dispose="yes" loadflag="LOAD16_BYTE" value="0x42" inverted="no" ovha="0x80" nothread="yes"/>"#,
        )?;
        let rom = parse_asset(&rom, false)?;

        assert_eq!(rom.attributes.region.as_deref(), Some("maincpu"));
        assert_eq!(rom.attributes.bios.as_deref(), Some("rev-a"));
        assert_eq!(rom.attributes.offset, Some(MameOffset(0x100)));
        assert_eq!(rom.attributes.optional, MameBoolean::Yes);
        assert_eq!(rom.dump_status, MameDumpStatus::Good);
        assert_eq!(rom.attributes.sound_only, Some(MameBoolean::No));
        assert_eq!(rom.attributes.dispose, Some(MameBoolean::Yes));
        assert_eq!(rom.attributes.load_flag.as_deref(), Some("LOAD16_BYTE"));
        assert_eq!(rom.attributes.value.as_deref(), Some("0x42"));
        assert_eq!(rom.attributes.inverted, Some(MameBoolean::No));
        assert_eq!(rom.attributes.ovha.as_deref(), Some("0x80"));
        assert_eq!(rom.attributes.no_thread, Some(MameBoolean::Yes));
        let defaults = parse_asset(
            &parse_xml_element(br#"<disk name="default.chd" size="0"/>"#)?,
            false,
        )?;
        assert_eq!(defaults.dump_status, MameDumpStatus::Good);
        assert_eq!(defaults.attributes.optional, MameBoolean::No);
        assert_eq!(defaults.attributes.writable, Some(MameBoolean::No));
        Ok(())
    }

    #[test]
    fn nested_machine_text_fields_are_retained_as_extensions()
    -> Result<(), Box<dyn std::error::Error>> {
        let catalog = MameCatalog::parse(br#"<mame><machine name="nested"><description>before<x/>after</description></machine></mame>"#)?;
        let machine = catalog
            .machines
            .first()
            .ok_or_else(|| std::io::Error::other("machine missing"))?;
        assert!(
            machine
                .extensions
                .iter()
                .any(|extension| extension.field_name == "element:description")
        );
        Ok(())
    }
}
