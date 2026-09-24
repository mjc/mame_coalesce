use std::collections::{BTreeMap, HashSet};

use crate::{
    logiqx::RecordLocation,
    mame::{Element, XmlExtension, parse_xml_element},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub version: Option<String>,
    pub entries: Vec<Entry>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub location: RecordLocation,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub assets: Vec<Asset>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub location: RecordLocation,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ArchiveId(String);

impl TryFrom<&str> for ArchiveId {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            Ok(Self(value.to_owned()))
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CloneReference {
    Parent,
    Archive(ArchiveId),
}

impl Catalog {
    /// Parses the repository's documented-field synthetic projection only.
    /// This is not a claim of production DAT-o-MATIC P/C XML conformance.
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        let root = parse_xml_element(bytes)?;
        if root.name != "datafile" {
            return Err(parse_error(
                "expected the synthetic <datafile> root",
                "document",
                None,
                root.location,
            ));
        }

        let mut version = None;
        let mut entries = Vec::new();
        let mut entry_names = HashSet::new();
        let mut extensions = Vec::new();
        for (name, value) in &root.attributes {
            extensions.push(attribute_extension(
                "document",
                None,
                name,
                value,
                root.location,
            ));
        }

        let mut saw_header = false;
        for child in root.children() {
            match child.name.as_str() {
                "header" => {
                    if std::mem::replace(&mut saw_header, true) {
                        return Err(parse_error(
                            "duplicate header",
                            "header",
                            None,
                            child.location,
                        ));
                    }
                    for field in child.children() {
                        match field.name.as_str() {
                            "version" => {
                                if version.replace(field.direct_text()).is_some() {
                                    return Err(parse_error(
                                        "duplicate header version",
                                        "header",
                                        None,
                                        field.location,
                                    ));
                                }
                            }
                            "name" | "description" => extensions.push(XmlExtension {
                                record_kind: "header".into(),
                                record_name: None,
                                field_name: field.name.clone(),
                                namespace_uri: None,
                                value: serde_json::json!(field.direct_text()),
                                location: field.location,
                            }),
                            _ => extensions.push(element_extension("header", None, field)?),
                        }
                    }
                    for (name, value) in &child.attributes {
                        extensions.push(attribute_extension(
                            "header",
                            None,
                            name,
                            value,
                            child.location,
                        ));
                    }
                }
                "game" => {
                    let entry = parse_entry(child)?;
                    if !entry_names.insert(entry.name.clone()) {
                        return Err(parse_error(
                            "duplicate archive name",
                            "game",
                            Some(&entry.name),
                            child.location,
                        ));
                    }
                    entries.push(entry);
                }
                _ => extensions.push(element_extension("document", None, child)?),
            }
        }
        if entries.is_empty() {
            return Err(parse_error(
                "document has no archive records",
                "document",
                None,
                root.location,
            ));
        }

        Ok(Self {
            version,
            entries,
            extensions,
        })
    }
}

fn parse_entry(node: &Element) -> crate::Result<Entry> {
    let name = required(node, "name", "game")?.to_owned();
    let (metadata, mut extensions) = parse_entry_attributes(node, &name)?;
    let assets = parse_assets(node, &name, &mut extensions)?;
    Ok(Entry {
        name,
        location: node.location,
        metadata,
        assets,
        extensions,
    })
}

fn parse_entry_attributes(
    node: &Element,
    name: &str,
) -> crate::Result<(BTreeMap<String, serde_json::Value>, Vec<XmlExtension>)> {
    let mut metadata = BTreeMap::new();
    for (source, target) in [
        ("namealt", "name_alt"),
        ("region", "region"),
        ("version", "version"),
        ("bios", "bios"),
    ] {
        metadata.insert(
            target.into(),
            node.attributes
                .get(source)
                .map_or(serde_json::Value::Null, |value| serde_json::json!(value)),
        );
    }
    metadata.insert(
        "languages".into(),
        node.attributes
            .get("languages")
            .map_or(serde_json::Value::Null, |value| {
                serde_json::json!(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                )
            }),
    );

    let mut extensions = Vec::new();
    if let Some(value) = node.attributes.get("clone") {
        metadata.insert(
            "clone".into(),
            serde_json::json!(parse_clone_reference(value, name, node.location)?),
        );
        extensions.push(attribute_extension(
            "game",
            Some(name),
            "clone",
            value,
            node.location,
        ));
    } else {
        metadata.insert("clone".into(), serde_json::Value::Null);
    }
    if let Some(value) = node.attributes.get("mergeof") {
        metadata.insert(
            "mergeof".into(),
            serde_json::json!(parse_archive_id(value, "mergeof", name, node.location)?.0),
        );
        extensions.push(attribute_extension(
            "game",
            Some(name),
            "mergeof",
            value,
            node.location,
        ));
    } else {
        metadata.insert("mergeof".into(), serde_json::Value::Null);
    }

    let known = [
        "name",
        "namealt",
        "region",
        "languages",
        "version",
        "bios",
        "clone",
        "mergeof",
    ];
    for (field, value) in &node.attributes {
        if !known.contains(&field.as_str()) {
            extensions.push(attribute_extension(
                "game",
                Some(name),
                field,
                value,
                node.location,
            ));
        }
    }

    Ok((metadata, extensions))
}

fn parse_clone_reference(
    value: &str,
    name: &str,
    location: RecordLocation,
) -> crate::Result<String> {
    let reference = if value == "P" {
        CloneReference::Parent
    } else {
        CloneReference::Archive(parse_archive_id(value, "clone", name, location)?)
    };
    Ok(match reference {
        CloneReference::Parent => "P".to_owned(),
        CloneReference::Archive(id) => id.0,
    })
}

fn parse_archive_id(
    value: &str,
    field: &str,
    name: &str,
    location: RecordLocation,
) -> crate::Result<ArchiveId> {
    ArchiveId::try_from(value).map_err(|()| {
        parse_error(
            &format!("{field} must be a numeric source archive ID"),
            "game",
            Some(name),
            location,
        )
    })
}

fn parse_assets(
    node: &Element,
    name: &str,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<Vec<Asset>> {
    let mut assets = Vec::new();
    let mut asset_names = HashSet::new();
    for child in node.children() {
        if child.name != "rom" {
            extensions.push(element_extension("game", Some(name), child)?);
            continue;
        }
        let asset = parse_asset(child, name)?;
        if !asset_names.insert(asset.name.clone()) {
            return Err(parse_error(
                "duplicate ROM name",
                "rom",
                Some(&asset.name),
                child.location,
            ));
        }
        assets.push(asset);
    }
    Ok(assets)
}

fn parse_asset(node: &Element, entry: &str) -> crate::Result<Asset> {
    let name = required(node, "name", "rom")?.to_owned();
    let size = node
        .attributes
        .get("size")
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| parse_error("invalid ROM size", "rom", Some(&name), node.location))
        })
        .transpose()?;
    let crc = node
        .attributes
        .get("crc")
        .map(|value| decode_hex(value, 8, "CRC", &name, node.location))
        .transpose()?;
    let sha1 = node
        .attributes
        .get("sha1")
        .map(|value| decode_hex(value, 40, "SHA-1", &name, node.location))
        .transpose()?;
    let mut extensions: Vec<XmlExtension> = node
        .attributes
        .iter()
        .filter(|(field, _)| !["name", "size", "crc", "sha1"].contains(&field.as_str()))
        .map(|(field, value)| attribute_extension("rom", Some(entry), field, value, node.location))
        .collect();
    for child in node.children() {
        extensions.push(element_extension("rom", Some(entry), child)?);
    }
    Ok(Asset {
        name,
        size,
        crc,
        sha1,
        location: node.location,
        extensions,
    })
}

fn decode_hex(
    value: &str,
    digits: usize,
    algorithm: &str,
    name: &str,
    location: RecordLocation,
) -> crate::Result<Vec<u8>> {
    if value.len() != digits || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(parse_error(
            &format!("invalid ROM {algorithm}"),
            "rom",
            Some(name),
            location,
        ));
    }
    (0..digits)
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| parse_error("invalid hexadecimal hash", "rom", Some(name), location))
        })
        .collect()
}

fn required<'a>(node: &'a Element, field: &str, kind: &str) -> crate::Result<&'a str> {
    node.attributes
        .get(field)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            parse_error(
                &format!("missing required {field:?}"),
                kind,
                None,
                node.location,
            )
        })
}

fn parse_error(
    message: &str,
    kind: &str,
    name: Option<&str>,
    location: RecordLocation,
) -> crate::Error {
    crate::Error::CatalogParse {
        message: message.into(),
        record_kind: Some(kind.into()),
        record_name: name.map(str::to_owned),
        line: Some(location.line),
        column: Some(location.column),
    }
}

fn attribute_extension(
    kind: &str,
    name: Option<&str>,
    field: &str,
    value: &str,
    location: RecordLocation,
) -> XmlExtension {
    let (field_name, namespace_uri) = split_attribute_name(field);
    XmlExtension {
        record_kind: kind.into(),
        record_name: name.map(str::to_owned),
        field_name,
        namespace_uri,
        value: serde_json::json!(value),
        location,
    }
}

fn element_extension(
    kind: &str,
    name: Option<&str>,
    element: &Element,
) -> crate::Result<XmlExtension> {
    Ok(XmlExtension {
        record_kind: kind.into(),
        record_name: name.map(str::to_owned),
        field_name: format!("element:{}", element.name),
        namespace_uri: None,
        value: serde_json::to_value(element)?,
        location: element.location,
    })
}

fn split_attribute_name(name: &str) -> (String, Option<String>) {
    name.strip_prefix('{')
        .and_then(|name| name.split_once('}'))
        .map_or_else(
            || (name.to_owned(), None),
            |(namespace, local)| (local.to_owned(), Some(namespace.to_owned())),
        )
}
