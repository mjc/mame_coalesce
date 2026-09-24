use std::collections::HashSet;

use crate::{
    logiqx::RecordLocation,
    mame::{Element, XmlExtension, parse_xml_element},
};

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
    pub lists: Vec<SoftwareList>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareList {
    pub name: SoftwareListName,
    pub description: Option<String>,
    pub location: RecordLocation,
    pub items: Vec<SoftwareItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareItem {
    pub name: SoftwareItemName,
    pub clone_of: Option<SoftwareItemName>,
    pub supported: SupportedStatus,
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
    pub areas: Vec<SoftwareArea>,
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

impl SoftwareComponent {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Rom(_) => "rom",
            Self::Disk(_) => "disk",
        }
    }

    pub const fn location(&self) -> RecordLocation {
        match self {
            Self::Rom(rom) => rom.location,
            Self::Disk(disk) => disk.location,
        }
    }
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
    pub name: ComponentName,
    pub sha1: Option<[u8; 20]>,
    pub status: Option<DumpStatus>,
    pub writeable: bool,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadInstruction {
    Load16Byte,
    Load16Word,
    Load16WordSwap,
    Load32Byte,
    Load32Word,
    Load32WordSwap,
    Load32Dword,
    Load64Word,
    Load64WordSwap,
    Reload,
    Fill,
    Continue,
    ReloadPlain,
    Ignore,
}

impl LoadInstruction {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Load16Byte => "load16_byte",
            Self::Load16Word => "load16_word",
            Self::Load16WordSwap => "load16_word_swap",
            Self::Load32Byte => "load32_byte",
            Self::Load32Word => "load32_word",
            Self::Load32WordSwap => "load32_word_swap",
            Self::Load32Dword => "load32_dword",
            Self::Load64Word => "load64_word",
            Self::Load64WordSwap => "load64_word_swap",
            Self::Reload => "reload",
            Self::Fill => "fill",
            Self::Continue => "continue",
            Self::ReloadPlain => "reload_plain",
            Self::Ignore => "ignore",
        }
    }
}

impl SoftwareListCatalog {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        let root = parse_xml_element(bytes)?;
        let mut extensions = Vec::new();
        let lists = match root.name.as_str() {
            "softwarelist" => vec![parse_list(&root, &mut extensions)?],
            "softwarelists" => {
                retain_unknown_attributes(&root, &[], "document", None, &mut extensions);
                let mut lists = Vec::new();
                for child in root.children() {
                    if child.name == "softwarelist" {
                        lists.push(parse_list(child, &mut extensions)?);
                    } else {
                        extensions.push(extension("document", None, child)?);
                    }
                }
                lists
            }
            other => {
                return Err(crate::Error::XmlValidation(format!(
                    "expected <softwarelist> or <softwarelists>, found <{other}>"
                )));
            }
        };
        if lists.is_empty() {
            return Err(crate::Error::XmlValidation(
                "software-list document has no lists".into(),
            ));
        }
        let mut names = HashSet::new();
        for list in &lists {
            if !names.insert(list.name.as_str()) {
                return Err(crate::Error::XmlValidation(format!(
                    "duplicate software-list name {:?}",
                    list.name.as_str()
                )));
            }
        }
        Ok(Self { lists, extensions })
    }
}

fn parse_list(node: &Element, extensions: &mut Vec<XmlExtension>) -> crate::Result<SoftwareList> {
    let name = SoftwareListName::new(required(node, "name")?);
    retain_unknown_attributes(
        node,
        &["name", "description"],
        "software_list",
        Some(name.as_str()),
        extensions,
    );
    let mut items = Vec::new();
    let mut item_names = HashSet::<String>::new();
    for child in node.children() {
        match child.name.as_str() {
            "software" => {
                let item = parse_item(child, &name, extensions)?;
                if !item_names.insert(item.name.as_str().to_owned()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate software item {:?} in list {:?}",
                        item.name.as_str(),
                        name.as_str()
                    )));
                }
                items.push(item);
            }
            _ => extensions.push(extension("software_list", Some(name.as_str()), child)?),
        }
    }
    if items.is_empty() {
        return Err(crate::Error::XmlValidation(format!(
            "software list {:?} has no software items",
            name.as_str()
        )));
    }
    Ok(SoftwareList {
        name,
        description: node.attributes.get("description").cloned(),
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
        None | Some("yes") => SupportedStatus::Yes,
        Some("partial") => SupportedStatus::Partial,
        Some("no") => SupportedStatus::No,
        Some(other) => {
            return Err(invalid_value("software", name.as_str(), "supported", other));
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
                notes = Some(child.direct_text());
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
    if parts.is_empty() {
        return Err(crate::Error::XmlValidation(format!(
            "software item {:?} has no parts",
            name.as_str()
        )));
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
    let mut areas = Vec::new();
    let mut area_names = HashSet::<(AreaKind, String)>::new();
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
                if !area_names.insert((area.kind, area.name.as_str().to_owned())) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate {} area {:?} in part {:?}",
                        area.kind.as_str(),
                        area.name.as_str(),
                        name.as_str()
                    )));
                }
                areas.push(area);
            }
            _ => extensions.push(extension("software_part", Some(&record), child)?),
        }
    }
    Ok(SoftwarePart {
        name,
        interface,
        features,
        areas,
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
            Some(parse_number(&required(node, "size")?)?),
            Some(parse_width(node.attributes.get("width"))?),
            Some(parse_endianness(node.attributes.get("endianness"))?),
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
    let has_name = node
        .attributes
        .get("name")
        .is_some_and(|name| !name.trim().is_empty());
    let has_other_evidence = ["size", "crc", "sha1", "offset", "value", "loadflag"]
        .iter()
        .any(|attribute| {
            node.attributes
                .get(*attribute)
                .is_some_and(|value| !value.trim().is_empty())
        });
    if !has_name && !has_other_evidence {
        return Err(crate::Error::CatalogParse {
            message: "ROM record has no identifying or load evidence".into(),
            record_kind: Some("rom".into()),
            record_name: None,
            line: Some(node.location.line),
            column: Some(node.location.column),
        });
    }
    Ok(SoftwareRom {
        name: node.attributes.get("name").cloned().map(ComponentName::new),
        size: parse_optional_number(node, "size")?,
        crc: parse_digest(node, "crc")?,
        sha1: parse_digest(node, "sha1")?,
        offset: parse_optional_number(node, "offset")?,
        value: node.attributes.get("value").cloned(),
        status: parse_status(node.attributes.get("status"), "rom")?,
        load: node
            .attributes
            .get("loadflag")
            .map(|value| parse_load(value))
            .transpose()?,
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
        None | Some("no") => false,
        Some("yes") => true,
        Some(other) => return Err(invalid_value("disk", "?", "writeable", other)),
    };
    Ok(SoftwareDisk {
        name: ComponentName::new(required(node, "name")?),
        sha1: parse_digest(node, "sha1")?,
        status: parse_status(node.attributes.get("status"), "disk")?,
        writeable,
        location: node.location,
    })
}

fn parse_status(value: Option<&String>, kind: &str) -> crate::Result<Option<DumpStatus>> {
    match value.map(String::as_str) {
        None => Ok(None),
        Some("good") => Ok(Some(DumpStatus::Good)),
        Some("baddump") => Ok(Some(DumpStatus::BadDump)),
        Some("nodump") => Ok(Some(DumpStatus::NoDump)),
        Some(other) => Err(invalid_value(kind, "?", "status", other)),
    }
}

fn parse_load(value: &str) -> crate::Result<LoadInstruction> {
    match value {
        "load16_byte" => Ok(LoadInstruction::Load16Byte),
        "load16_word" => Ok(LoadInstruction::Load16Word),
        "load16_word_swap" => Ok(LoadInstruction::Load16WordSwap),
        "load32_byte" => Ok(LoadInstruction::Load32Byte),
        "load32_word" => Ok(LoadInstruction::Load32Word),
        "load32_word_swap" => Ok(LoadInstruction::Load32WordSwap),
        "load32_dword" => Ok(LoadInstruction::Load32Dword),
        "load64_word" => Ok(LoadInstruction::Load64Word),
        "load64_word_swap" => Ok(LoadInstruction::Load64WordSwap),
        "reload" => Ok(LoadInstruction::Reload),
        "fill" => Ok(LoadInstruction::Fill),
        "continue" => Ok(LoadInstruction::Continue),
        "reload_plain" => Ok(LoadInstruction::ReloadPlain),
        "ignore" => Ok(LoadInstruction::Ignore),
        other => Err(invalid_value("rom", "?", "loadflag", other)),
    }
}

fn parse_width(value: Option<&String>) -> crate::Result<u8> {
    let width = value.map_or(Ok(8), |value| {
        value
            .parse::<u8>()
            .map_err(|_| crate::Error::XmlValidation(format!("invalid dataarea width {value:?}")))
    })?;
    if matches!(width, 8 | 16 | 32 | 64) {
        Ok(width)
    } else {
        Err(crate::Error::XmlValidation(format!(
            "unsupported dataarea width {width}"
        )))
    }
}

fn parse_endianness(value: Option<&String>) -> crate::Result<Endianness> {
    match value.map(String::as_str) {
        None | Some("little") => Ok(Endianness::Little),
        Some("big") => Ok(Endianness::Big),
        Some(other) => Err(crate::Error::XmlValidation(format!(
            "invalid dataarea endianness {other:?}"
        ))),
    }
}

fn parse_optional_number(node: &Element, name: &str) -> crate::Result<Option<u64>> {
    node.attributes
        .get(name)
        .map(|value| parse_number(value))
        .transpose()
}

fn parse_number(value: &str) -> crate::Result<u64> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"));
    let parsed = parsed
        .map_or_else(|| value.parse(), |hex| u64::from_str_radix(hex, 16))
        .map_err(|_| crate::Error::XmlValidation(format!("invalid unsigned integer {value:?}")))?;
    i64::try_from(parsed).map_err(|_| {
        crate::Error::XmlValidation(format!("integer exceeds supported storage range {value:?}"))
    })?;
    Ok(parsed)
}

fn parse_digest<const N: usize>(node: &Element, field: &str) -> crate::Result<Option<[u8; N]>> {
    let Some(value) = node.attributes.get(field) else {
        return Ok(None);
    };
    let bytes = hex::decode(value)
        .map_err(|_| crate::Error::XmlValidation(format!("invalid {field} digest {value:?}")))?;
    let digest: [u8; N] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        crate::Error::XmlValidation(format!(
            "invalid {field} digest length {}, expected {}",
            bytes.len(),
            N
        ))
    })?;
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
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
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

fn invalid_value(kind: &str, record: &str, field: &str, value: &str) -> crate::Error {
    crate::Error::XmlValidation(format!(
        "invalid {field} value {value:?} on {kind} {record:?}"
    ))
}

fn extension(
    record_kind: &str,
    record_name: Option<&str>,
    node: &Element,
) -> crate::Result<XmlExtension> {
    let value = serde_json::to_value(node)
        .map_err(|error| crate::Error::XmlValidation(error.to_string()))?;
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
            value: serde_json::json!(raw_value),
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
        assert_eq!(item.supported, SupportedStatus::Partial);
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
        assert!(disk.writeable);
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
                && extension.value == serde_json::json!("retained")
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
    fn parser_retains_single_list_attributes_and_rom_extensions() -> crate::Result<()> {
        let xml = br#"<softwarelist name="one" future-root="kept"><software name="game"><description future-text-attribute="retained">Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="game.bin"><future-component-claim value="kept"/></rom><rom loadflag="continue"/></dataarea><diskarea name="media"><disk name="game-disk"><future-disk-claim value="also-kept"/></disk></diskarea></part></software></softwarelist>"#;
        let catalog = SoftwareListCatalog::parse(xml)?;
        assert_eq!(
            catalog
                .extensions
                .iter()
                .filter(|extension| {
                    extension.field_name == "@future-root"
                        && extension.value == serde_json::json!("kept")
                })
                .count(),
            1
        );
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_list"
                && extension.field_name == "@future-root"
                && extension.value == serde_json::json!("kept")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_item"
                && extension.field_name == "@future-text-attribute"
                && extension.value == serde_json::json!("retained")
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_rom"
                && extension.field_name == "element:future-component-claim"
                && extension.value["attributes"]["value"] == "kept"
        }));
        assert!(catalog.extensions.iter().any(|extension| {
            extension.record_kind == "software_disk"
                && extension.field_name == "element:future-disk-claim"
                && extension.value["attributes"]["value"] == "also-kept"
        }));
        Ok(())
    }

    #[test]
    fn parser_rejects_empty_rom_records_and_values_outside_storage_range() {
        let empty_rom = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(empty_rom).is_err());
        let empty_name = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name=""/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(empty_name).is_err());
        let unnamed_load = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom loadflag="continue"/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(unnamed_load).is_ok());
        assert!(parse_number("9223372036854775808").is_err());
        assert!(parse_number("0x8000000000000000").is_err());
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
    fn parser_rejects_external_entity_declarations_and_invalid_hashes() {
        let entity = br#"<!DOCTYPE softwarelist [<!ENTITY external SYSTEM "file:///etc/passwd">]><softwarelist name="one"><software name="game"><description>&external;</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"/></software></softwarelist>"#;
        assert!(matches!(
            SoftwareListCatalog::parse(entity),
            Err(crate::Error::XmlEntityNotAllowed)
        ));

        let invalid_hash = br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="16"><rom name="game.bin" crc="not-hex"/></dataarea></part></software></softwarelist>"#;
        assert!(SoftwareListCatalog::parse(invalid_hash).is_err());
    }
}
