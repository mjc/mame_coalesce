use quick_xml::events::Event;

use crate::{
    disk::{DiskDigestScope, DiskIdentitySha1, DiskName, DiskRequirement, ParentDiskName},
    domain::AssetRole,
    logiqx::RecordLocation,
    xml_reader::{self, NodeBudget},
};

use crate::xml_reader::Element;
pub use crate::xml_reader::SourceByteView;

mod attributes;
mod specification;
pub use crate::xml_reader::{AttributeLocation, AttributePosition, SourceExtent};
pub use attributes::*;
pub use specification::*;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(test)]
pub struct MameCatalog {
    pub build: Option<String>,
    pub debug: bool,
    pub debug_specified: bool,
    pub config_version: String,
    pub machines: Vec<Machine>,
    pub extensions: Vec<XmlExtension>,
    pub attribute_positions: Vec<AttributePosition<MameDocumentAttribute>>,
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
    pub description_source_order: i64,
    pub year: Option<String>,
    pub year_location: Option<RecordLocation>,
    pub year_source_order: Option<i64>,
    pub manufacturer: Option<String>,
    pub manufacturer_location: Option<RecordLocation>,
    pub manufacturer_source_order: Option<i64>,
    pub flags: MachineFlags,
    pub attributes_location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<MameMachineAttribute>>,
    pub compatibility_attribute_positions:
        Vec<AttributePosition<MameMachineCompatibilityAttribute>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineFlags(u16);

impl MachineFlags {
    const DEVICE: u16 = 1 << 0;
    const RUNNABLE: u16 = 1 << 1;
    const BIOS: u16 = 1 << 2;
    const MECHANICAL: u16 = 1 << 3;
    const CONSUMABLE: u16 = 1 << 4;
    const DEVICE_SPECIFIED: u16 = 1 << 5;
    const RUNNABLE_SPECIFIED: u16 = 1 << 6;
    const BIOS_SPECIFIED: u16 = 1 << 7;
    const MECHANICAL_SPECIFIED: u16 = 1 << 8;
    const CONSUMABLE_SPECIFIED: u16 = 1 << 9;

    /// Rebuild flags from native stored value/presence pairs without XML parsing.
    ///
    /// Pair order is device, runnable, BIOS, mechanical, consumable.
    pub(crate) fn from_stored(values: [(bool, bool); 5]) -> crate::Result<Self> {
        let mut flags = 0;
        for ((effective, specified), (value_bit, specified_bit, default, name)) in
            values.into_iter().zip([
                (Self::DEVICE, Self::DEVICE_SPECIFIED, false, "isdevice"),
                (Self::RUNNABLE, Self::RUNNABLE_SPECIFIED, true, "runnable"),
                (Self::BIOS, Self::BIOS_SPECIFIED, false, "isbios"),
                (
                    Self::MECHANICAL,
                    Self::MECHANICAL_SPECIFIED,
                    false,
                    "ismechanical",
                ),
                (
                    Self::CONSUMABLE,
                    Self::CONSUMABLE_SPECIFIED,
                    false,
                    "isconsumable",
                ),
            ])
        {
            if !specified && effective != default {
                return Err(crate::Error::XmlValidation(format!(
                    "stored MAME {name} is not specified but differs from its default"
                )));
            }
            if effective {
                flags |= value_bit;
            }
            if specified {
                flags |= specified_bit;
            }
        }
        Ok(Self(flags))
    }

    fn parse(attributes: &xml_reader::XmlAttributes) -> crate::Result<Self> {
        let mut flags = 0;
        for (attribute, flag, specified, default) in [
            ("isdevice", Self::DEVICE, Self::DEVICE_SPECIFIED, false),
            ("runnable", Self::RUNNABLE, Self::RUNNABLE_SPECIFIED, true),
            ("isbios", Self::BIOS, Self::BIOS_SPECIFIED, false),
            (
                "ismechanical",
                Self::MECHANICAL,
                Self::MECHANICAL_SPECIFIED,
                false,
            ),
            (
                "isconsumable",
                Self::CONSUMABLE,
                Self::CONSUMABLE_SPECIFIED,
                false,
            ),
        ] {
            if attributes.contains_key(attribute) {
                flags |= specified;
            }
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
    #[must_use]
    pub const fn is_device_specified(self) -> bool {
        self.0 & Self::DEVICE_SPECIFIED != 0
    }
    #[must_use]
    pub const fn is_runnable_specified(self) -> bool {
        self.0 & Self::RUNNABLE_SPECIFIED != 0
    }
    #[must_use]
    pub const fn is_bios_specified(self) -> bool {
        self.0 & Self::BIOS_SPECIFIED != 0
    }
    #[must_use]
    pub const fn is_mechanical_specified(self) -> bool {
        self.0 & Self::MECHANICAL_SPECIFIED != 0
    }
    #[must_use]
    pub const fn is_consumable_specified(self) -> bool {
        self.0 & Self::CONSUMABLE_SPECIFIED != 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineBiosSet {
    pub name: String,
    pub description: String,
    pub is_default: bool,
    pub default_specified: bool,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<MameBiosAttribute>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineSwitchKind {
    DipSwitch,
    Configuration,
}

impl MachineSwitchKind {
    #[must_use]
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
    pub mask: String,
    pub source_order: i64,
    pub location: RecordLocation,
    pub condition: Option<MachineCondition>,
    pub locations: Vec<MachineSwitchLocation>,
    pub values: Vec<MachineSwitchValue>,
    pub attribute_positions: Vec<AttributePosition<MameSwitchAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchLocation {
    pub name: String,
    pub number: String,
    pub inverted: bool,
    pub inverted_specified: bool,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<MameSwitchLocationAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchValue {
    pub name: String,
    pub value: String,
    pub default: bool,
    pub default_specified: bool,
    pub condition: Option<MachineCondition>,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<MameSwitchValueAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceReference {
    pub tag: String,
    pub name: String,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<MameDeviceReferenceAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineAsset {
    pub name: String,
    pub role: AssetRole,
    pub declarations: MameAssetDeclarations,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub merge_name: Option<String>,
    pub dump_status: MameDumpStatus,
    pub disk_requirement: Option<DiskRequirement>,
    pub location: RecordLocation,
    pub source_order: i64,
    pub attributes: MameAssetAttributes,
    pub extensions: Vec<XmlExtension>,
    pub attribute_positions: MameAssetAttributePositions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
// Keep explicit source-text names distinct from checked numeric/hash evidence.
#[allow(clippy::struct_field_names)]
pub struct MameAssetDeclarations {
    pub size_text: Option<String>,
    pub crc_text: Option<String>,
    pub md5_text: Option<String>,
    pub sha1_text: Option<String>,
    pub offset_text: Option<String>,
}

impl MameAssetDeclarations {
    /// Whether every supplied size/hash declaration can be interpreted.
    /// This does not establish whole-file scope or supply a missing strong hash.
    #[must_use]
    pub fn declarations_interpretable(&self) -> bool {
        self.size_text
            .as_deref()
            .is_none_or(|value| declared_size_evidence(value).is_some())
            && self
                .crc_text
                .as_deref()
                .is_none_or(|value| is_hex_text(value, 8))
            && self
                .md5_text
                .as_deref()
                .is_none_or(|value| is_hex_text(value, 32))
            && self
                .sha1_text
                .as_deref()
                .is_none_or(|value| is_hex_text(value, 40))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MameDumpStatus {
    Good,
    BadDump,
    NoDump,
}

impl MameDumpStatus {
    #[must_use]
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
    #[must_use]
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
    pub optional_specified: bool,
    pub status_specified: bool,
    pub sound_only: Option<MameBoolean>,
    pub dispose: Option<MameBoolean>,
    pub load_flag: Option<String>,
    pub value: Option<String>,
    pub inverted: Option<MameBoolean>,
    pub ovha: Option<String>,
    pub no_thread: Option<MameBoolean>,
    pub disk_index: Option<String>,
    pub writable: Option<MameBoolean>,
    pub writable_specified: bool,
    pub writeable: Option<MameBoolean>,
}

impl MameAssetAttributes {
    /// Historical loading fields have no pinned complete-file contract here.
    pub(crate) const fn has_unproven_loading(&self) -> bool {
        self.load_flag.is_some()
            || self.value.is_some()
            || self.inverted.is_some()
            || self.ovha.is_some()
            || self.no_thread.is_some()
    }
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

    #[cfg(test)]
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
    pub debug_specified: bool,
    pub config_version: String,
    pub location: RecordLocation,
    /// Byte offset of the root opening `<`, when the decoded view maps to bytes.
    pub root_extent_start: Option<usize>,
    pub extensions: Vec<XmlExtension>,
    pub attribute_positions: Vec<AttributePosition<MameDocumentAttribute>>,
}

pub enum MameRecord {
    Machine(Box<Machine>),
    Extension(XmlExtension),
}

/// A parsed direct child of `<mame>`, delivered only after its closing event.
pub struct CompleteMameRecord {
    pub record: MameRecord,
    pub source_order: i64,
    pub location: RecordLocation,
    /// Physical interval of this closed root child when byte mapping is valid.
    pub extent: Option<SourceExtent>,
}

/// Root physical extent and end coordinate, available only after accepted EOF.
pub struct MameCaptureProof {
    root_extent: Option<SourceExtent>,
    root_end_location: RecordLocation,
    source_byte_view: Option<SourceByteView>,
}

impl MameCaptureProof {
    #[must_use]
    pub const fn root_extent(&self) -> Option<SourceExtent> {
        self.root_extent
    }

    #[must_use]
    pub const fn root_end_location(&self) -> RecordLocation {
        self.root_end_location
    }

    #[must_use]
    pub const fn source_byte_view(&self) -> Option<SourceByteView> {
        self.source_byte_view
    }
}

/// Only a successful traversal through the closing root and EOF produces this state.
pub struct ValidatedMame<S> {
    sink: S,
    capture: MameCaptureProof,
}

impl<S> ValidatedMame<S> {
    /// Consume the reader-owned valid-EOF proof and return its completed sink.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.sink
    }

    /// Consume the sink together with its validated root/EOF capture proof.
    #[must_use]
    pub fn into_capture_parts(self) -> (S, MameCaptureProof) {
        (self.sink, self.capture)
    }
}

/// Consume records one at a time and return a sink only after valid EOF.
///
/// Parser errors convert through `E::from`; consumers can use
/// a separate error variant for persistence failures.
pub fn read_with<S, E: From<crate::Error>>(
    bytes: &[u8],
    start: impl FnOnce(MameHeader) -> Result<S, E>,
    mut consume: impl FnMut(&mut S, MameRecord) -> Result<(), E>,
) -> Result<ValidatedMame<S>, E> {
    read_captured_with(bytes, start, |sink, complete| {
        consume(sink, complete.record)
    })
}

/// Stream complete root children and return root extent proof only after valid EOF.
pub fn read_captured_with<S, E: From<crate::Error>>(
    bytes: &[u8],
    start: impl FnOnce(MameHeader) -> Result<S, E>,
    mut consume: impl FnMut(&mut S, CompleteMameRecord) -> Result<(), E>,
) -> Result<ValidatedMame<S>, E> {
    xml_reader::with_reader(bytes, |reader, positions| {
        let (namespace, root_start, empty, root_event_span) = loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            match event {
                Event::Start(start) => {
                    let span = reader.last_event_span().ok_or_else(|| {
                        crate::Error::XmlValidation("XML root start has no source range".into())
                    })?;
                    break (namespace, start, false, span);
                }
                Event::Empty(start) => {
                    let span = reader.last_event_span().ok_or_else(|| {
                        crate::Error::XmlValidation("XML root start has no source range".into())
                    })?;
                    break (namespace, start, true, span);
                }
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
        let config_version = required(&root, "mameconfig")?.to_owned();
        let attribute_positions =
            attributes::select(&root.attributes, MameDocumentAttribute::from_name)?;
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
        let root_extent_start = positions.event_extent_start(root_event_span.0)?;
        let mut sink = start(MameHeader {
            build,
            debug,
            debug_specified: root.attributes.contains_key("debug"),
            config_version,
            location: root.location,
            root_extent_start,
            extensions,
            attribute_positions,
        })?;
        let capture = parse_machine_records(
            reader,
            positions,
            &mut budget,
            empty,
            root_event_span,
            true,
            |record| consume(&mut sink, record),
        )?;
        Ok(ValidatedMame { sink, capture })
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
                    debug_specified: header.debug_specified,
                    config_version: header.config_version,
                    machines: Vec::new(),
                    extensions: header.extensions,
                    attribute_positions: header.attribute_positions,
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
    reader: &mut crate::xml_reader::XmlReader<'_>,
    positions: &mut xml_reader::PositionMap<'_>,
    budget: &mut NodeBudget,
    empty: bool,
    root_event_span: (u64, u64),
    retain_extensions: bool,
    mut consume: impl FnMut(CompleteMameRecord) -> Result<(), E>,
) -> Result<MameCaptureProof, E> {
    let mut saw_machine = false;
    let (root_end_offset, root_end_location) = if empty {
        let end = root_event_span.1;
        (end, positions.event_location(end)?)
    } else {
        let mut source_order = 0_usize;
        loop {
            let (namespace, event) = xml_reader::next(reader, positions)?;
            let (node, record_order, record_start) = match event {
                Event::Start(child) => {
                    let (record_start, _) = reader.last_event_span().ok_or_else(|| {
                        crate::Error::XmlValidation("XML child start has no source range".into())
                    })?;
                    let record_order = to_source_order(source_order)?;
                    source_order = source_order.checked_add(1).ok_or_else(|| {
                        crate::Error::XmlValidation("MAME source order overflow".into())
                    })?;
                    let node =
                        xml_reader::read_element(reader, namespace, &child, budget, 1, positions)?;
                    (node, record_order, record_start)
                }
                Event::Empty(child) => {
                    let (record_start, _) = reader.last_event_span().ok_or_else(|| {
                        crate::Error::XmlValidation("XML child start has no source range".into())
                    })?;
                    let record_order = to_source_order(source_order)?;
                    source_order = source_order.checked_add(1).ok_or_else(|| {
                        crate::Error::XmlValidation("MAME source order overflow".into())
                    })?;
                    let node = xml_reader::element_from_start(
                        reader, namespace, &child, budget, 1, positions,
                    )?;
                    (node, record_order, record_start)
                }
                Event::End(_) => {
                    let (_, end) = reader.last_event_span().ok_or_else(|| {
                        crate::Error::XmlValidation("XML root end has no source range".into())
                    })?;
                    break (end, positions.event_location(end)?);
                }
                Event::Eof => {
                    return Err(crate::Error::XmlValidation(
                        "unexpected end of input inside <mame>".into(),
                    )
                    .into());
                }
                _ => continue,
            };
            let location = node.location;
            let (_, record_end) = reader.last_event_span().ok_or_else(|| {
                crate::Error::XmlValidation("XML child end has no source range".into())
            })?;
            let extent = positions.event_extent(record_start, record_end)?;
            let record = parse_record(&node, retain_extensions)?;
            drop(node);
            if let Some(record) = record {
                saw_machine |= matches!(&record, MameRecord::Machine(_));
                consume(CompleteMameRecord {
                    record,
                    source_order: record_order,
                    location,
                    extent,
                })?;
            }
        }
    };
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
    if !saw_machine {
        return Err(
            crate::Error::XmlValidation("MAME document has no machine records".into()).into(),
        );
    }
    let root_extent = positions.event_extent(root_event_span.0, root_end_offset)?;
    Ok(MameCaptureProof {
        root_extent,
        root_end_location,
        source_byte_view: positions.source_byte_view(),
    })
}

fn parse_record(node: &Element, retain_extensions: bool) -> crate::Result<Option<MameRecord>> {
    if node.name == "machine" {
        let machine = parse_machine(node, retain_extensions)?;
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

fn parse_device_reference(node: &Element, source_order: i64) -> crate::Result<DeviceReference> {
    Ok(DeviceReference {
        tag: required(node, "tag")?.to_owned(),
        name: required(node, "name")?.to_owned(),
        source_order,
        location: node.location,
        attribute_positions: attributes::select(
            &node.attributes,
            MameDeviceReferenceAttribute::from_name,
        )?,
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
    description_source_order: i64,
    year: Option<String>,
    year_location: Option<RecordLocation>,
    year_source_order: Option<i64>,
    manufacturer: Option<String>,
    manufacturer_location: Option<RecordLocation>,
    manufacturer_source_order: Option<i64>,
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
            description_source_order: children.description_source_order,
            year: children.year,
            year_location: children.year_location,
            year_source_order: children.year_source_order,
            manufacturer: children.manufacturer,
            manufacturer_location: children.manufacturer_location,
            manufacturer_source_order: children.manufacturer_source_order,
            flags,
            attributes_location: node.location,
            attribute_positions: attributes::select(
                &node.attributes,
                MameMachineAttribute::from_name,
            )?,
            compatibility_attribute_positions: attributes::select(
                &node.attributes,
                MameMachineCompatibilityAttribute::from_name,
            )?,
        },
        assets: children.assets,
        device_refs: children.device_refs,
        switches: children.switches,
        bios_sets: children.bios_sets,
        specification: children.specification,
        extensions: children.extensions,
    })
}

// Keep the exhaustive source-element dispatch in one place so every known family
// receives the same original child ordinal.
#[allow(clippy::too_many_lines)]
fn parse_machine_children(
    node: &Element,
    machine_name: &str,
    retain_extensions: bool,
) -> crate::Result<MachineChildren> {
    let mut description = None;
    let mut description_location = None;
    let mut description_source_order = None;
    let mut year = None;
    let mut year_location = None;
    let mut year_source_order = None;
    let mut manufacturer = None;
    let mut manufacturer_location = None;
    let mut manufacturer_source_order = None;
    let mut bios_sets = Vec::new();
    let mut specification = Vec::new();
    let mut device_refs = Vec::new();
    let mut switches = Vec::new();
    let mut assets = Vec::new();
    let mut extensions = Vec::new();
    let mut singleton_specification_elements = std::collections::BTreeSet::new();
    let mut machine_text_fields = std::collections::HashSet::new();
    for (element_order, child) in node.children().enumerate() {
        let source_order = to_source_order(element_order)?;
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
                        description_source_order = Some(source_order);
                    }
                    "year" => {
                        year = Some(text);
                        year_location = Some(child.location);
                        year_source_order = Some(source_order);
                    }
                    "manufacturer" => {
                        manufacturer = Some(text);
                        manufacturer_location = Some(child.location);
                        manufacturer_source_order = Some(source_order);
                    }
                    _ => {}
                }
                if retain_extensions && child.children().next().is_some() {
                    extensions.push(extension("machine", Some(machine_name), child)?);
                }
            }
            "biosset" => bios_sets.push(parse_machine_bios_set(child, source_order)?),
            "device_ref" => {
                device_refs.push(parse_device_reference(child, source_order)?);
            }
            "dipswitch" => {
                switches.push(parse_machine_switch(
                    child,
                    MachineSwitchKind::DipSwitch,
                    source_order,
                )?);
            }
            "configuration" => switches.push(parse_machine_switch(
                child,
                MachineSwitchKind::Configuration,
                source_order,
            )?),
            "rom" | "disk" => assets.push(parse_asset(child, retain_extensions, source_order)?),
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
        description_source_order: description_source_order.ok_or_else(|| {
            crate::Error::XmlValidation(format!(
                "MAME machine {machine_name:?} is missing description"
            ))
        })?,
        year,
        year_location,
        year_source_order,
        manufacturer,
        manufacturer_location,
        manufacturer_source_order,
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
        element_order: to_source_order(element_order)?,
        value: specification::parse_element(child)?,
    })
}

fn to_source_order(order: usize) -> crate::Result<i64> {
    i64::try_from(order)
        .map_err(|_| crate::Error::InvalidPath("MAME source order exceeds SQLite INTEGER".into()))
}

fn parse_machine_bios_set(node: &Element, source_order: i64) -> crate::Result<MachineBiosSet> {
    Ok(MachineBiosSet {
        name: required(node, "name")?.to_owned(),
        description: required(node, "description")?.to_owned(),
        is_default: parse_mame_boolean(node.attributes.get("default"), "biosset default")?,
        default_specified: node.attributes.contains_key("default"),
        source_order,
        location: node.location,
        attribute_positions: attributes::select(&node.attributes, MameBiosAttribute::from_name)?,
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

fn parse_machine_switch(
    node: &Element,
    kind: MachineSwitchKind,
    source_order: i64,
) -> crate::Result<MachineSwitch> {
    let name = required(node, "name")?.to_owned();
    let tag = required(node, "tag")?.to_owned();
    let mask = required(node, "mask")?.to_owned();
    let mut locations = Vec::new();
    let mut values = Vec::new();
    let mut condition = None;
    for (child_order, child) in node.children().enumerate() {
        let child_source_order = to_source_order(child_order)?;
        match child.name.as_str() {
            "condition" => {
                if condition.is_some() {
                    return Err(crate::Error::XmlValidation(format!(
                        "MAME <{}> {name:?} has multiple conditions",
                        node.name
                    )));
                }
                condition = Some(specification::parse_condition(child, child_order)?);
            }
            "diplocation" | "conflocation" => {
                if !matches!(
                    (kind, child.name.as_str()),
                    (MachineSwitchKind::DipSwitch, "diplocation")
                        | (MachineSwitchKind::Configuration, "conflocation")
                ) {
                    return Err(crate::Error::XmlValidation(format!(
                        "<{}> is not valid under <{}>",
                        child.name, node.name
                    )));
                }
                locations.push(MachineSwitchLocation {
                    name: required(child, "name")?.to_owned(),
                    number: required(child, "number")?.to_owned(),
                    inverted: parse_mame_boolean(
                        child.attributes.get("inverted"),
                        "switch location inverted",
                    )?,
                    inverted_specified: child.attributes.contains_key("inverted"),
                    source_order: child_source_order,
                    location: child.location,
                    attribute_positions: attributes::select(
                        &child.attributes,
                        MameSwitchLocationAttribute::from_name,
                    )?,
                });
            }
            "dipvalue" | "confsetting" => {
                let valid_owner = matches!(
                    (kind, child.name.as_str()),
                    (MachineSwitchKind::DipSwitch, "dipvalue")
                        | (MachineSwitchKind::Configuration, "confsetting")
                );
                if !valid_owner {
                    return Err(crate::Error::XmlValidation(format!(
                        "<{}> is not valid under <{}>",
                        child.name, node.name
                    )));
                }
                values.push(MachineSwitchValue {
                    name: required(child, "name")?.to_owned(),
                    value: required(child, "value")?.to_owned(),
                    default: parse_mame_boolean(
                        child.attributes.get("default"),
                        "switch value default",
                    )?,
                    default_specified: child.attributes.contains_key("default"),
                    condition: specification::parse_optional_condition(child)?,
                    source_order: child_source_order,
                    location: child.location,
                    attribute_positions: attributes::select(
                        &child.attributes,
                        MameSwitchValueAttribute::from_name,
                    )?,
                });
            }
            _ => {}
        }
    }
    Ok(MachineSwitch {
        kind,
        name,
        tag,
        mask,
        source_order,
        location: node.location,
        condition,
        locations,
        values,
        attribute_positions: attributes::select(&node.attributes, MameSwitchAttribute::from_name)?,
    })
}

fn parse_mame_integer(value: &str) -> crate::Result<u64> {
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    // MAME 0.289 output_rom writes offsets with %x, even when every digit is numeric.
    u64::from_str_radix(hex, 16)
        .map_err(|_| crate::Error::XmlValidation(format!("invalid MAME integer {value:?}")))
}

fn parse_mame_asset_attributes(node: &Element) -> crate::Result<MameAssetAttributes> {
    let value = |name: &str| node.attributes.get(name).cloned();
    let number = |name: &str| {
        node.attributes
            .get(name)
            .and_then(|value| parse_mame_integer(value).ok())
            .map(MameOffset)
    };
    let boolean = |name: &str| parse_optional_mame_boolean(node.attributes.get(name), name);

    Ok(MameAssetAttributes {
        region: value("region"),
        bios: value("bios"),
        offset: number("offset"),
        optional: boolean("optional")?.unwrap_or_default(),
        optional_specified: node.attributes.contains_key("optional"),
        status_specified: node.attributes.contains_key("status"),
        sound_only: boolean("soundonly")?,
        dispose: boolean("dispose")?,
        load_flag: value("loadflag"),
        value: value("value"),
        inverted: boolean("inverted")?,
        ovha: value("ovha"),
        no_thread: boolean("nothread")?,
        disk_index: value("index"),
        writable: boolean("writable")?.or_else(|| (node.name == "disk").then_some(MameBoolean::No)),
        writable_specified: node.attributes.contains_key("writable"),
        writeable: boolean("writeable")?,
    })
}

fn parse_asset(
    node: &Element,
    retain_extensions: bool,
    source_order: i64,
) -> crate::Result<MachineAsset> {
    let name = required(node, "name")?;
    let declarations = MameAssetDeclarations {
        size_text: node.attributes.get("size").cloned(),
        crc_text: node.attributes.get("crc").cloned(),
        md5_text: node.attributes.get("md5").cloned(),
        sha1_text: node.attributes.get("sha1").cloned(),
        offset_text: node.attributes.get("offset").cloned(),
    };
    let size = declarations
        .size_text
        .as_deref()
        .and_then(declared_size_evidence);
    let sha1 = declarations
        .sha1_text
        .as_deref()
        .and_then(|digest| decode_hex(digest).ok());
    let crc = declarations
        .crc_text
        .as_deref()
        .and_then(|digest| decode_hex_sized(digest, 8).ok());
    let md5 = declarations
        .md5_text
        .as_deref()
        .and_then(|digest| decode_hex_sized(digest, 32).ok());
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
        declarations,
        size,
        crc,
        md5,
        sha1,
        merge_name,
        dump_status,
        disk_requirement,
        location: node.location,
        source_order,
        attributes,
        extensions,
        attribute_positions: if node.name == "rom" {
            MameAssetAttributePositions::Rom {
                native: attributes::select(&node.attributes, MameRomAttribute::from_name)?,
                compatibility: attributes::select(
                    &node.attributes,
                    MameRomCompatibilityAttribute::from_name,
                )?,
            }
        } else {
            MameAssetAttributePositions::Disk {
                native: attributes::select(&node.attributes, MameDiskAttribute::from_name)?,
                compatibility: attributes::select(
                    &node.attributes,
                    MameDiskCompatibilityAttribute::from_name,
                )?,
            }
        },
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

fn declared_size_evidence(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let size = value.parse::<u64>().ok()?;
    i64::try_from(size).ok().map(|_| size)
}

fn is_hex_text(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
#[allow(clippy::expect_used)] // These tests use expect to identify missing fixture witnesses.
mod tests {
    use crate::xml_reader::{MAX_XML_DEPTH, MAX_XML_NODES};

    use super::*;

    #[test]
    fn streaming_delivers_records_before_a_late_parse_error() {
        let mut names = Vec::new();
        let result = read_with::<_, crate::Error>(
            br#"<mame mameconfig="10"><machine name="first"><description>First</description></machine><machine name="broken">"#,
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
    fn captured_records_keep_root_extent_and_physical_source_order() -> crate::Result<()> {
        let xml = b"\xef\xbb\xbf<mame mameconfig='10'><vendor/><machine name='ordered'><description>Ordered</description><vendor-child/><rom name='r.bin'/></machine><!--after--></mame><!--outside-->";
        let mut records = Vec::new();
        let validated = read_captured_with::<_, crate::Error>(
            xml,
            |header| {
                assert_eq!(header.root_extent_start, Some(3));
                Ok(())
            },
            |(), record| {
                records.push(record);
                Ok(())
            },
        )?;
        let (_, proof) = validated.into_capture_parts();
        assert_eq!(
            proof.source_byte_view(),
            Some(crate::xml_reader::SourceByteView::RetainedOriginal {
                byte_length: xml.len(),
            })
        );

        let root_end = xml
            .windows(b"</mame>".len())
            .position(|window| window == b"</mame>")
            .expect("MAME root closes")
            + b"</mame>".len();
        assert_eq!(
            proof.root_extent(),
            Some(SourceExtent {
                start: 3,
                end: root_end
            })
        );
        assert_eq!(proof.root_end_location().line, 1);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].source_order, 0);
        assert!(matches!(records[0].record, MameRecord::Extension(_)));

        let machine = &records[1];
        assert_eq!(machine.source_order, 1);
        let machine_start = xml
            .windows(b"<machine name='ordered'>".len())
            .position(|window| window == b"<machine name='ordered'>")
            .expect("machine opening tag exists");
        assert_eq!(machine.location.line, 1);
        assert_eq!(
            machine.location.column,
            i64::try_from(machine_start - 2).expect("location fits")
        );
        let machine_end = xml
            .windows(b"</machine>".len())
            .position(|window| window == b"</machine>")
            .expect("machine closing tag exists")
            + b"</machine>".len();
        assert_eq!(
            machine.extent,
            Some(SourceExtent {
                start: machine_start,
                end: machine_end,
            })
        );
        let MameRecord::Machine(machine) = &machine.record else {
            panic!("second root child is the machine")
        };
        assert_eq!(machine.assets[0].source_order, 2);
        assert_eq!(machine.facts.description_source_order, 0);
        Ok(())
    }

    #[test]
    fn capture_does_not_report_utf16_transcoded_offsets_as_source_bytes() -> crate::Result<()> {
        let text =
            "<mame mameconfig='10'><machine name='m'><description>M</description></machine></mame>";
        let mut xml = vec![0xff, 0xfe];
        xml.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        let mut records = Vec::new();
        let validated = read_captured_with::<_, crate::Error>(
            &xml,
            |_| Ok(()),
            |(), record| {
                records.push(record);
                Ok(())
            },
        )?;
        let (_, proof) = validated.into_capture_parts();
        assert_eq!(proof.source_byte_view(), None);
        assert_eq!(proof.root_extent(), None);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_order, 0);
        assert_eq!(records[0].location.line, 1);
        assert_eq!(records[0].extent, None);
        Ok(())
    }

    #[test]
    fn captured_gzip_reports_complete_decoded_view_without_extending_root_extent()
    -> crate::Result<()> {
        use flate2::{Compression, write::GzEncoder};
        use std::io::Write;

        let xml = b"<mame mameconfig='10'><machine name='m'><description>M</description></machine></mame><!--outside-->";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(xml)?;
        let compressed = encoder.finish()?;
        let validated = read_captured_with::<_, crate::Error>(
            compressed.as_slice(),
            |_| Ok(()),
            |(), _| Ok(()),
        )?;
        let (_, proof) = validated.into_capture_parts();

        let root_end = xml
            .windows(b"</mame>".len())
            .position(|window| window == b"</mame>")
            .expect("MAME root closes")
            + b"</mame>".len();
        assert_eq!(
            proof.source_byte_view(),
            Some(crate::xml_reader::SourceByteView::TransportDecodedXml {
                byte_length: xml.len(),
            })
        );
        assert_eq!(proof.root_extent().map(|extent| extent.end), Some(root_end));
        Ok(())
    }

    #[test]
    fn content_before_the_root_never_starts_publication() {
        for prefix in ["junk", "<![CDATA[junk]]>", "&amp;", "\u{a0}"] {
            let xml = format!(
                "{prefix}<mame mameconfig='10'><machine name='x'><description>X</description></machine></mame>"
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
                "{whitespace}<mame mameconfig='10'><machine name='x'><description>X</description></machine></mame>{whitespace}"
            );
            assert!(MameCatalog::parse(xml.as_bytes()).is_ok());
        }
        for suffix in ["\u{a0}", "\u{2003}", "\u{85}"] {
            let xml = format!(
                "<mame mameconfig='10'><machine name='x'><description>X</description></machine></mame>{suffix}"
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
        let xml = br#"<mame mameconfig="10"><machine name="duplicate"><description>one</description><description>two</description></machine></mame>"#;
        assert!(MameCatalog::parse(xml).is_err());
    }

    #[test]
    fn mame_asset_spec_attributes_are_parsed_as_typed_facts() -> crate::Result<()> {
        let rom = parse_xml_element(
            br#"<rom name="boot.bin" region="maincpu" bios="rev-a" offset="0x100" status="good" optional="yes" soundonly="no" dispose="yes" loadflag="LOAD16_BYTE" value="0x42" inverted="no" ovha="0x80" nothread="yes"/>"#,
        )?;
        let rom = parse_asset(&rom, false, 0)?;

        assert_eq!(rom.attributes.region.as_deref(), Some("maincpu"));
        assert_eq!(rom.attributes.bios.as_deref(), Some("rev-a"));
        assert_eq!(rom.attributes.offset, Some(MameOffset(0x100)));
        assert_eq!(rom.attributes.optional, MameBoolean::Yes);
        assert!(rom.attributes.optional_specified);
        assert!(rom.attributes.status_specified);
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
            0,
        )?;
        assert_eq!(defaults.dump_status, MameDumpStatus::Good);
        assert_eq!(defaults.attributes.optional, MameBoolean::No);
        assert_eq!(defaults.attributes.writable, Some(MameBoolean::No));
        assert!(!defaults.attributes.writable_specified);
        Ok(())
    }

    #[test]
    fn asset_declarations_preserve_absence_empty_text_and_usable_evidence() -> crate::Result<()> {
        let absent = parse_asset(
            &parse_xml_element(br#"<rom name="absent.bin"/>"#)?,
            false,
            0,
        )?;
        assert_eq!(absent.declarations.size_text, None);
        assert_eq!(absent.declarations.crc_text, None);
        assert_eq!(absent.declarations.md5_text, None);
        assert_eq!(absent.declarations.sha1_text, None);
        assert_eq!(absent.declarations.offset_text, None);
        assert!(absent.declarations.declarations_interpretable());

        let empty = parse_asset(
            &parse_xml_element(
                br#"<rom name="empty.bin" size="" crc="" md5="" sha1="" offset=""/>"#,
            )?,
            false,
            0,
        )?;
        assert_eq!(empty.declarations.size_text.as_deref(), Some(""));
        assert_eq!(empty.declarations.crc_text.as_deref(), Some(""));
        assert_eq!(empty.declarations.md5_text.as_deref(), Some(""));
        assert_eq!(empty.declarations.sha1_text.as_deref(), Some(""));
        assert_eq!(empty.declarations.offset_text.as_deref(), Some(""));
        assert_eq!(empty.size, None);
        assert_eq!(empty.crc, None);
        assert_eq!(empty.md5, None);
        assert_eq!(empty.sha1, None);
        assert_eq!(empty.attributes.offset, None);
        assert!(!empty.declarations.declarations_interpretable());
        Ok(())
    }

    #[test]
    fn asset_declarations_retain_original_numeric_and_digest_text() -> crate::Result<()> {
        let md5_text = "aA".repeat(16);
        let sha1_text = "Ff".repeat(20);
        let xml = format!(
            r#"<rom name="mixed.bin" size="00042" crc="aBcD0123" md5="{md5_text}" sha1="{sha1_text}" offset="0x00aF"/>"#
        );
        let asset = parse_asset(&parse_xml_element(xml.as_bytes())?, false, 0)?;

        assert_eq!(asset.declarations.size_text.as_deref(), Some("00042"));
        assert_eq!(asset.declarations.crc_text.as_deref(), Some("aBcD0123"));
        assert_eq!(
            asset.declarations.md5_text.as_deref(),
            Some(md5_text.as_str())
        );
        assert_eq!(
            asset.declarations.sha1_text.as_deref(),
            Some(sha1_text.as_str())
        );
        assert_eq!(asset.declarations.offset_text.as_deref(), Some("0x00aF"));
        assert_eq!(asset.size, Some(42));
        assert_eq!(asset.crc, Some(vec![0xab, 0xcd, 0x01, 0x23]));
        assert_eq!(asset.md5.as_ref().map(Vec::len), Some(16));
        assert_eq!(asset.sha1.as_ref().map(Vec::len), Some(20));
        assert_eq!(asset.attributes.offset, Some(MameOffset(0xaf)));
        assert!(asset.declarations.declarations_interpretable());
        Ok(())
    }

    #[test]
    fn invalid_and_overflowing_declarations_are_retained_without_usable_evidence()
    -> crate::Result<()> {
        let invalid = parse_asset(
            &parse_xml_element(
                br#"<rom name="invalid.bin" size="many" crc="" md5="not-hex" sha1="not-hex"/>"#,
            )?,
            false,
            0,
        )?;
        assert_eq!(invalid.declarations.size_text.as_deref(), Some("many"));
        assert_eq!(invalid.declarations.crc_text.as_deref(), Some(""));
        assert_eq!(invalid.declarations.md5_text.as_deref(), Some("not-hex"));
        assert_eq!(invalid.declarations.sha1_text.as_deref(), Some("not-hex"));
        assert_eq!(invalid.size, None);
        assert_eq!(invalid.crc, None);
        assert_eq!(invalid.md5, None);
        assert_eq!(invalid.sha1, None);
        assert!(!invalid.declarations.declarations_interpretable());

        let overflow = parse_asset(
            &parse_xml_element(br#"<rom name="overflow.bin" size="9223372036854775808"/>"#)?,
            false,
            0,
        )?;
        assert_eq!(
            overflow.declarations.size_text.as_deref(),
            Some("9223372036854775808")
        );
        assert_eq!(overflow.size, None);
        assert!(!overflow.declarations.declarations_interpretable());
        Ok(())
    }

    #[test]
    fn invalid_offset_is_preserved_without_affecting_whole_file_identity() -> crate::Result<()> {
        let asset = parse_asset(
            &parse_xml_element(
                br#"<rom name="offset.bin" size="12" crc="aBcD0123" offset="not-a-number"/>"#,
            )?,
            false,
            0,
        )?;

        assert_eq!(
            asset.declarations.offset_text.as_deref(),
            Some("not-a-number")
        );
        assert_eq!(asset.attributes.offset, None);
        assert_eq!(asset.size, Some(12));
        assert_eq!(asset.crc, Some(vec![0xab, 0xcd, 0x01, 0x23]));
        assert!(asset.declarations.declarations_interpretable());
        Ok(())
    }

    #[test]
    fn asset_closed_enums_and_boolean_values_remain_validated() -> crate::Result<()> {
        assert!(
            parse_asset(
                &parse_xml_element(br#"<rom name="bad-status.bin" status="unknown"/>"#)?,
                false,
                0,
            )
            .is_err()
        );
        assert!(
            parse_asset(
                &parse_xml_element(br#"<rom name="bad-boolean.bin" optional="true"/>"#)?,
                false,
                0,
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn nested_machine_text_fields_are_retained_as_extensions()
    -> Result<(), Box<dyn std::error::Error>> {
        let catalog = MameCatalog::parse(br#"<mame mameconfig="10"><machine name="nested"><description>before<x/>after</description></machine></mame>"#)?;
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

    #[test]
    // This single fixture is an intentionally exhaustive presence/order witness across families.
    #[allow(clippy::too_many_lines)]
    fn native_presence_defaults_and_owner_orders_are_retained() -> crate::Result<()> {
        let catalog = MameCatalog::parse(
            br#"<mame mameconfig="10">
                <machine name="ordered" isdevice="no" runnable="yes" isbios="yes" ismechanical="no" isconsumable="yes">
                    <description>Ordered</description><vendor/><year>1990</year><manufacturer>Maker</manufacturer>
                    <biosset name="default" description="Default"/>
                    <biosset name="alternate" description="Alternate" default="no"/>
                    <device_ref tag=":slot" name="cart"/>
                    <rom name="program.bin"/>
                    <disk name="media.chd" writable="yes" optional="no" status="baddump"/>
                    <dipswitch name="service" tag=":io" mask="0xff">
                        <vendor/><diplocation name="SW1" number="1" inverted="no"/>
                        <dipvalue name="On" value="not-numeric" default="no"/>
                    </dipswitch>
                    <configuration name="mode" tag=":io" mask="mode">
                        <conflocation name="CN1" number="2" inverted="yes"/>
                        <confsetting name="Automatic" value="automatic" default="yes"/>
                    </configuration>
                    <display type="raster" refresh="60"/>
                    <display type="vector" refresh="60" flipx="yes"/>
                    <input service="no" players="2"><control type="joy"/><control type="stick" reverse="no"/></input>
                    <driver status="good" emulation="good" savestate="supported" requiresartwork="no" unofficial="yes" nosoundhardware="no" incomplete="yes"/>
                    <slot name="cart"><slotoption name="empty" devname="" default="no"/></slot>
                </machine>
            </mame>"#,
        )?;
        assert!(!catalog.debug);
        assert!(!catalog.debug_specified);
        assert_eq!(catalog.config_version, "10");
        let machine = catalog.machines.first().expect("machine parsed");
        assert_eq!(machine.facts.description_source_order, 0);
        assert_eq!(machine.facts.year_source_order, Some(2));
        assert_eq!(machine.facts.manufacturer_source_order, Some(3));
        assert!(!machine.facts.flags.is_device());
        assert!(machine.facts.flags.is_device_specified());
        assert!(machine.facts.flags.is_runnable());
        assert!(machine.facts.flags.is_runnable_specified());
        assert!(machine.facts.flags.is_bios());
        assert!(machine.facts.flags.is_bios_specified());
        assert!(!machine.facts.flags.is_mechanical());
        assert!(machine.facts.flags.is_mechanical_specified());
        assert!(machine.facts.flags.is_consumable());
        assert!(machine.facts.flags.is_consumable_specified());

        assert_eq!(machine.bios_sets[0].description, "Default");
        assert!(!machine.bios_sets[0].is_default);
        assert!(!machine.bios_sets[0].default_specified);
        assert_eq!(machine.bios_sets[0].source_order, 4);
        assert!(machine.bios_sets[1].default_specified);
        assert_eq!(machine.bios_sets[1].source_order, 5);
        assert_eq!(machine.device_refs[0].source_order, 6);
        assert_eq!(machine.assets[0].source_order, 7);
        assert!(!machine.assets[0].attributes.optional_specified);
        assert!(!machine.assets[0].attributes.status_specified);
        assert!(!machine.assets[0].attributes.writable_specified);
        assert_eq!(machine.assets[1].source_order, 8);
        assert!(machine.assets[1].attributes.optional_specified);
        assert!(machine.assets[1].attributes.status_specified);
        assert!(machine.assets[1].attributes.writable_specified);
        assert_eq!(machine.switches[0].source_order, 9);
        assert_eq!(machine.switches[0].mask, "0xff");
        assert_eq!(machine.switches[0].locations[0].source_order, 1);
        assert!(machine.switches[0].locations[0].inverted_specified);
        assert_eq!(machine.switches[0].values[0].source_order, 2);
        assert_eq!(machine.switches[0].values[0].value, "not-numeric");
        assert!(machine.switches[0].values[0].default_specified);
        assert_eq!(machine.switches[1].source_order, 10);
        assert_eq!(machine.switches[1].mask, "mode");
        assert!(machine.switches[1].locations[0].inverted);
        assert!(machine.switches[1].locations[0].inverted_specified);
        assert_eq!(machine.switches[1].values[0].value, "automatic");

        let displays = machine
            .specification
            .iter()
            .filter_map(|item| match &item.value {
                MachineSpecification::Display(value) => Some((item.element_order, value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!displays[0].1.flip_x_specified);
        assert!(displays[1].1.flip_x_specified);
        assert_eq!(displays[1].1.flip_x, MameBoolean::Yes);
        assert_eq!(displays[0].0, 11);
        let input = machine
            .specification
            .iter()
            .find_map(|item| match &item.value {
                MachineSpecification::Input(value) => Some(value),
                _ => None,
            })
            .expect("input parsed");
        assert!(input.service_specified);
        assert_eq!(input.service, MameBoolean::No);
        assert!(!input.tilt_specified);
        assert_eq!(input.tilt, MameBoolean::No);
        assert!(!input.controls[0].reverse_specified);
        assert!(input.controls[1].reverse_specified);
        let driver = machine
            .specification
            .iter()
            .find_map(|item| match &item.value {
                MachineSpecification::Driver(value) => Some(value),
                _ => None,
            })
            .expect("driver parsed");
        assert!(driver.requires_artwork_specified);
        assert!(driver.unofficial_specified);
        assert!(driver.no_sound_hardware_specified);
        assert!(driver.incomplete_specified);
        let slot = machine
            .specification
            .iter()
            .find_map(|item| match &item.value {
                MachineSpecification::Slot(value) => Some(value),
                _ => None,
            })
            .expect("slot parsed");
        assert!(slot.options[0].default_specified);
        assert_eq!(slot.options[0].is_default, MameBoolean::No);
        Ok(())
    }

    #[test]
    fn machine_flag_storage_rejects_unspecified_non_defaults() -> crate::Result<()> {
        let defaults = MachineFlags::from_stored([
            (false, false),
            (true, false),
            (false, false),
            (false, false),
            (false, false),
        ])?;
        assert!(defaults.is_runnable());
        assert!(!defaults.is_runnable_specified());
        assert!(
            MachineFlags::from_stored([
                (true, false),
                (true, false),
                (false, false),
                (false, false),
                (false, false),
            ])
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn switch_children_require_their_declared_owner_and_unique_conditions() {
        for child in [
            "<confsetting name='x' value='1'/>",
            "<conflocation name='x' number='1'/>",
            "<condition tag=':x' mask='1' relation='eq' value='1'/><condition tag=':x' mask='1' relation='eq' value='1'/>",
        ] {
            let xml = format!(
                "<mame mameconfig='10'><machine name='x'><description>X</description><dipswitch name='s' tag=':x' mask='1'>{child}</dipswitch></machine></mame>"
            );
            assert!(
                MameCatalog::parse(xml.as_bytes()).is_err(),
                "accepted {child}"
            );
        }
        let valid_dip_location = MameCatalog::parse(
            br#"<mame mameconfig="10"><machine name="x"><description>X</description><dipswitch name="s" tag=":x" mask="1"><diplocation name="SW1" number="1"/></dipswitch></machine></mame>"#,
        )
        .expect("diplocation is valid under dipswitch");
        assert_eq!(
            valid_dip_location.machines[0].switches[0].locations[0].name,
            "SW1"
        );
        for child in [
            "<dipvalue name='x' value='1'/>",
            "<diplocation name='x' number='1'/>",
        ] {
            let xml = format!(
                "<mame mameconfig='10'><machine name='x'><description>X</description><configuration name='s' tag=':x' mask='1'>{child}</configuration></machine></mame>"
            );
            assert!(
                MameCatalog::parse(xml.as_bytes()).is_err(),
                "accepted {child}"
            );
        }
        let duplicate_value_condition = br#"<mame mameconfig="10"><machine name="x"><description>X</description><configuration name="s" tag=":x" mask="1"><confsetting name="x" value="1"><condition tag=":x" mask="1" relation="eq" value="1"/><condition tag=":x" mask="1" relation="eq" value="1"/></confsetting></configuration></machine></mame>"#;
        assert!(MameCatalog::parse(duplicate_value_condition).is_err());
    }

    #[test]
    fn required_mame_header_and_bios_description_are_enforced() {
        assert!(
            MameCatalog::parse(
                br#"<mame><machine name="x"><description>X</description></machine></mame>"#
            )
            .is_err()
        );
        assert!(MameCatalog::parse(
            br#"<mame mameconfig="10"><machine name="x"><description>X</description><biosset name="main"/></machine></mame>"#
        )
        .is_err());
    }

    #[test]
    fn explicit_mame_debug_default_retains_presence() -> crate::Result<()> {
        let catalog = MameCatalog::parse(br#"<mame mameconfig="10" debug="no"><machine name="system"><description>System</description></machine></mame>"#)?;
        assert!(!catalog.debug);
        assert!(catalog.debug_specified);
        Ok(())
    }
    #[test]
    fn numeric_offset_queries_use_mame_hexadecimal_source_text() -> crate::Result<()> {
        let catalog=MameCatalog::parse(b"<mame mameconfig='10'><machine name='system'><description>System</description><rom name='rom' size='1' offset='8000'/></machine></mame>")?;
        assert_eq!(
            catalog.machines[0].assets[0]
                .declarations
                .offset_text
                .as_deref(),
            Some("8000")
        );
        assert_eq!(
            catalog.machines[0].assets[0].attributes.offset,
            Some(MameOffset(0x8000))
        );
        Ok(())
    }
}
