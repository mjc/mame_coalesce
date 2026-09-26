use std::collections::BTreeMap;

use serde::Serialize;
use xml::{
    common::Position,
    reader::{ParserConfig, XmlEvent},
};

use crate::{document_input, logiqx::RecordLocation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MameCatalog {
    pub build: Option<String>,
    pub machines: Vec<Machine>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub name: String,
    pub parent: Option<String>,
    pub location: RecordLocation,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub assets: Vec<MachineAsset>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineAsset {
    pub name: String,
    pub role: &'static str,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub merge_name: Option<String>,
    pub dump_status: Option<String>,
    pub location: RecordLocation,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlExtension {
    pub record_kind: String,
    pub record_name: Option<String>,
    pub field_name: String,
    pub namespace_uri: Option<String>,
    pub value: serde_json::Value,
    pub location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Element {
    name: String,
    attributes: BTreeMap<String, String>,
    content: Vec<ElementContent>,
    #[serde(skip)]
    location: RecordLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum ElementContent {
    Text(String),
    Element(Element),
}

impl Element {
    fn children(&self) -> impl Iterator<Item = &Self> {
        self.content.iter().filter_map(|content| match content {
            ElementContent::Element(child) => Some(child),
            ElementContent::Text(_) => None,
        })
    }

    fn direct_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|content| match content {
                ElementContent::Text(text) => Some(text.as_str()),
                ElementContent::Element(_) => None,
            })
            .collect()
    }
}

const MAX_XML_DEPTH: usize = 256;
const MAX_XML_NODES: usize = 200_000;

impl MameCatalog {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        let root = parse_xml_element(bytes)?;
        if root.name != "mame" {
            return Err(crate::Error::XmlValidation(format!(
                "expected <mame>, found <{}>",
                root.name
            )));
        }
        let build = root.attributes.get("build").cloned();
        let mut root_extensions: Vec<XmlExtension> = root
            .attributes
            .iter()
            .filter(|(name, _)| name.as_str() != "build")
            .map(|(name, value)| {
                let (field_name, namespace_uri) = attribute_name(name);
                XmlExtension {
                    record_kind: "document".into(),
                    record_name: None,
                    field_name,
                    namespace_uri,
                    value: serde_json::json!(value),
                    location: root.location,
                }
            })
            .collect();
        let mut machines = Vec::new();
        let mut machine_names = std::collections::HashSet::new();
        for node in root.children() {
            if node.name == "machine" {
                let machine = parse_machine(node)?;
                if !machine_names.insert(machine.name.clone()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate MAME machine name {:?}",
                        machine.name
                    )));
                }
                machines.push(machine);
            } else {
                root_extensions.push(extension("document", None, node)?);
            }
        }
        if machines.is_empty() {
            return Err(crate::Error::XmlValidation(
                "MAME document has no machine records".into(),
            ));
        }
        Ok(Self {
            build,
            machines,
            extensions: root_extensions,
        })
    }
}

fn parse_xml_element(bytes: &[u8]) -> crate::Result<Element> {
    let xml = document_input::decode_xml(bytes)?;
    if xml
        .windows(b"<!ENTITY".len())
        .any(|marker| marker == b"<!ENTITY")
    {
        return Err(crate::Error::XmlEntityNotAllowed);
    }
    let mut reader = ParserConfig::new()
        .max_entity_expansion_length(1024)
        .max_entity_expansion_depth(4)
        .max_name_length(4096)
        .max_attributes(1024)
        .max_attribute_length(1024 * 1024)
        .max_data_length(1024 * 1024)
        .allow_multiple_root_elements(false)
        .create_reader(xml.as_ref());
    let mut stack: Vec<Element> = Vec::new();
    let mut root = None;
    let mut node_count = 0_usize;
    loop {
        match reader.next() {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => {
                node_count = node_count.checked_add(1).ok_or_else(|| {
                    crate::Error::XmlValidation("XML element count overflow".into())
                })?;
                if node_count > MAX_XML_NODES {
                    return Err(crate::Error::XmlValidation(format!(
                        "XML element count exceeds {MAX_XML_NODES} nodes"
                    )));
                }
                if stack.len() >= MAX_XML_DEPTH {
                    return Err(crate::Error::XmlValidation(format!(
                        "XML element nesting exceeds {MAX_XML_DEPTH} levels"
                    )));
                }
                let element_name = name.namespace.map_or_else(
                    || name.local_name.clone(),
                    |namespace| format!("{{{namespace}}}{}", name.local_name),
                );
                stack.push(Element {
                    name: element_name,
                    attributes: attributes
                        .into_iter()
                        .map(|attribute| {
                            let name = attribute.name.namespace.map_or_else(
                                || attribute.name.local_name.clone(),
                                |namespace| format!("{{{namespace}}}{}", attribute.name.local_name),
                            );
                            (name, attribute.value)
                        })
                        .collect(),
                    content: Vec::new(),
                    location: location(reader.position()),
                });
            }
            Ok(XmlEvent::Characters(text) | XmlEvent::CData(text)) => {
                if let Some(element) = stack.last_mut() {
                    element.content.push(ElementContent::Text(text));
                }
            }
            Ok(XmlEvent::EndElement { .. }) => {
                let element = stack.pop().ok_or_else(|| {
                    crate::Error::XmlValidation("unexpected closing element".into())
                })?;
                if let Some(parent) = stack.last_mut() {
                    parent.content.push(ElementContent::Element(element));
                } else {
                    root = Some(element);
                }
            }
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => {}
            Err(error) => return Err(crate::Error::XmlValidation(error.to_string())),
        }
    }
    root.ok_or_else(|| crate::Error::XmlValidation("missing document root".into()))
}

fn parse_machine(node: &Element) -> crate::Result<Machine> {
    let name = required(node, "name")?;
    let parent = node.attributes.get("cloneof").cloned();
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "sourcefile".into(),
        value(node.attributes.get("sourcefile")),
    );
    for flag in [
        "isdevice",
        "runnable",
        "isbios",
        "ismechanical",
        "isconsumable",
    ] {
        if let Some(flag_value) = node.attributes.get(flag) {
            metadata.insert(flag.into(), value(Some(flag_value)));
        }
    }
    let mut biossets = Vec::new();
    let mut device_refs = Vec::new();
    let mut assets = Vec::new();
    let mut extensions = Vec::new();
    let mut machine_text_fields = std::collections::HashSet::new();
    for child in node.children() {
        match child.name.as_str() {
            "description" | "year" | "manufacturer" => {
                if !machine_text_fields.insert(child.name.as_str()) {
                    return Err(crate::Error::XmlValidation(format!(
                        "duplicate machine {} field for {:?}",
                        child.name, name
                    )));
                }
                metadata.insert(
                    child.name.clone(),
                    serde_json::Value::String(child.direct_text()),
                );
                if child.children().next().is_some() {
                    extensions.push(extension("machine", Some(name), child)?);
                }
            }
            "biosset" => biossets.push(serde_json::json!({"name": required(child, "name")?, "description": child.attributes.get("description"), "default": child.attributes.get("default")})),
            "device_ref" => device_refs.push(serde_json::json!({"name": required(child, "name")?, "order": device_refs.len()})),
            "rom" | "disk" => assets.push(parse_asset(child)?),
            _ => extensions.push(extension("machine", Some(name), child)?),
        }
        for (key, val) in &child.attributes {
            if !matches!(child.name.as_str(), "rom" | "disk")
                && !known_child_attribute(&child.name, key)
            {
                let (field_name, namespace_uri) = attribute_name(key);
                extensions.push(XmlExtension {
                    record_kind: child.name.clone(),
                    record_name: child.attributes.get("name").cloned(),
                    field_name,
                    namespace_uri,
                    value: serde_json::json!(val),
                    location: child.location,
                });
            }
        }
    }
    metadata.insert("biossets".into(), serde_json::json!(biossets));
    metadata.insert("device_refs".into(), serde_json::json!(device_refs));
    for (key, val) in &node.attributes {
        if ![
            "name",
            "sourcefile",
            "isdevice",
            "runnable",
            "isbios",
            "ismechanical",
            "isconsumable",
            "cloneof",
        ]
        .contains(&key.as_str())
        {
            let (field_name, namespace_uri) = attribute_name(key);
            extensions.push(XmlExtension {
                record_kind: "machine".into(),
                record_name: Some(name.into()),
                field_name,
                namespace_uri,
                value: serde_json::json!(val),
                location: node.location,
            });
        }
    }
    Ok(Machine {
        name: name.into(),
        parent,
        location: node.location,
        metadata,
        assets,
        extensions,
    })
}

fn parse_asset(node: &Element) -> crate::Result<MachineAsset> {
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
    let dump_status = node.attributes.get("status").cloned();
    let metadata = node
        .attributes
        .iter()
        .filter(|(key, _)| {
            known_asset_attribute(&node.name, key)
                && !["name", "size", "sha1", "crc", "md5", "merge", "status"]
                    .contains(&key.as_str())
        })
        .map(|(key, val)| (key.clone(), serde_json::json!(val)))
        .collect();
    let mut extensions = Vec::new();
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
                value: serde_json::json!(val),
                location: node.location,
            });
        }
    }
    Ok(MachineAsset {
        name: name.into(),
        role: if node.name == "rom" { "rom" } else { "disk" },
        size,
        crc,
        md5,
        sha1,
        merge_name,
        dump_status,
        location: node.location,
        metadata,
        extensions,
    })
}

fn extension(kind: &str, record_name: Option<&str>, node: &Element) -> crate::Result<XmlExtension> {
    let (field_name, namespace_uri) = attribute_name(&node.name);
    Ok(XmlExtension {
        record_kind: kind.into(),
        record_name: record_name.map(str::to_owned),
        field_name: format!("element:{field_name}"),
        namespace_uri,
        value: serde_json::to_value(node)?,
        location: node.location,
    })
}

fn known_child_attribute(element: &str, attribute: &str) -> bool {
    match element {
        "biosset" => ["name", "description", "default"].contains(&attribute),
        "device_ref" => ["name"].contains(&attribute),
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
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::Error::XmlValidation(format!(
                "<{}> is missing required {name:?} attribute",
                element.name
            ))
        })
}

fn value(value: Option<&String>) -> serde_json::Value {
    value.map_or(serde_json::Value::Null, |s| serde_json::json!(s))
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

fn location(position: xml::common::TextPosition) -> RecordLocation {
    RecordLocation {
        line: i64::try_from(position.row.saturating_add(1)).unwrap_or(i64::MAX),
        column: i64::try_from(position.column.saturating_add(1)).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_tree_keeps_mixed_text_and_element_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let missing = || std::io::Error::other("synthetic XML structure is incomplete");
        let root = parse_xml_element(
            br#"<mame><machine name="x"><future>before<x/>after</future></machine></mame>"#,
        )?;
        let machine = root.children().next().ok_or_else(missing)?;
        let future = machine.children().next().ok_or_else(missing)?;
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
