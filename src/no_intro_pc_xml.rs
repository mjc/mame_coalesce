use std::collections::HashSet;

use crate::{
    logiqx::RecordLocation,
    mame::{ExtensionValue, XmlExtension, parse_xml_element},
    xml_reader::{Element, ElementContent},
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
    pub facts: GameFacts,
    pub location: RecordLocation,
    pub assets: Vec<Asset>,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameFacts {
    pub archive_id: Option<ArchiveId>,
    pub description: Option<String>,
    pub description_location: Option<RecordLocation>,
    pub name_alt: Option<String>,
    pub region: Option<String>,
    pub languages: Option<Vec<String>>,
    pub version: Option<String>,
    pub bios_text: Option<String>,
    pub clone_reference: Option<CloneReference>,
    pub merge_of: Option<ArchiveId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub location: RecordLocation,
    pub extensions: Vec<XmlExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArchiveId(String);

impl ArchiveId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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
pub enum CloneReference {
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
        reject_record_text(&root, "document", None)?;

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
                    parse_header(child, &mut version, &mut extensions)?;
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

fn parse_header(
    node: &Element,
    version: &mut Option<String>,
    extensions: &mut Vec<XmlExtension>,
) -> crate::Result<()> {
    reject_record_text(node, "header", None)?;
    for field in node.children() {
        if matches!(field.name.as_str(), "version" | "name" | "description")
            && (!field.attributes.is_empty() || field.children().next().is_some())
        {
            return Err(parse_error(
                "structured header fields are unsupported",
                "header",
                Some(&field.name),
                field.location,
            ));
        }
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
                value: serde_json::json!(field.direct_text()).into(),
                location: field.location,
            }),
            _ => extensions.push(element_extension("header", None, field)?),
        }
    }
    for (name, value) in &node.attributes {
        extensions.push(attribute_extension(
            "header",
            None,
            name,
            value,
            node.location,
        ));
    }
    Ok(())
}

fn parse_entry(node: &Element) -> crate::Result<Entry> {
    let name = required(node, "name", "game")?.to_owned();
    reject_record_text(node, "game", Some(&name))?;
    let mut facts = parse_entry_attributes(node, &name)?;
    let mut extensions = unknown_entry_attributes(node, &name);
    let archive_id = node
        .attributes
        .get("id")
        .map(|value| parse_archive_id(value, "id", &name, node.location))
        .transpose()?;
    let mut description = None;
    let mut description_location = None;
    for child in node.children() {
        if child.name == "description" {
            if description.replace(child.direct_text()).is_some() {
                return Err(parse_error(
                    "duplicate game description",
                    "game",
                    Some(&name),
                    child.location,
                ));
            }
            description_location = Some(child.location);
            if !child.attributes.is_empty() || child.children().next().is_some() {
                return Err(parse_error(
                    "structured game description is unsupported",
                    "game",
                    Some(&name),
                    child.location,
                ));
            }
        }
    }
    let assets = parse_assets(node, &name, &mut extensions)?;
    facts.archive_id = archive_id;
    facts.description = description;
    facts.description_location = description_location;
    Ok(Entry {
        name,
        facts,
        location: node.location,
        assets,
        extensions,
    })
}

fn parse_entry_attributes(node: &Element, name: &str) -> crate::Result<GameFacts> {
    Ok(GameFacts {
        archive_id: None,
        description: None,
        description_location: None,
        name_alt: node.attributes.get("namealt").cloned(),
        region: node.attributes.get("region").cloned(),
        languages: node.attributes.get("languages").map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_owned)
                .collect()
        }),
        version: node.attributes.get("version").cloned(),
        bios_text: node.attributes.get("bios").cloned(),
        clone_reference: node
            .attributes
            .get("clone")
            .map(|value| parse_clone_reference(value, name, node.location))
            .transpose()?,
        merge_of: node
            .attributes
            .get("mergeof")
            .map(|value| parse_archive_id(value, "mergeof", name, node.location))
            .transpose()?,
    })
}

fn unknown_entry_attributes(node: &Element, name: &str) -> Vec<XmlExtension> {
    const KNOWN: &[&str] = &[
        "name",
        "id",
        "namealt",
        "region",
        "languages",
        "version",
        "bios",
        "clone",
        "mergeof",
    ];
    node.attributes
        .iter()
        .filter(|(field, _)| !KNOWN.contains(&field.as_str()))
        .map(|(field, value)| attribute_extension("game", Some(name), field, value, node.location))
        .collect()
}

fn parse_clone_reference(
    value: &str,
    name: &str,
    location: RecordLocation,
) -> crate::Result<CloneReference> {
    Ok(if value == "P" {
        CloneReference::Parent
    } else {
        CloneReference::Archive(parse_archive_id(value, "clone", name, location)?)
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
            if child.name == "description" {
                continue;
            }
            extensions.push(element_extension("game", Some(name), child)?);
            continue;
        }
        let asset = parse_asset(child)?;
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

fn parse_asset(node: &Element) -> crate::Result<Asset> {
    let name = required(node, "name", "rom")?.to_owned();
    reject_record_text(node, "rom", Some(&name))?;
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
    let md5 = node
        .attributes
        .get("md5")
        .map(|value| decode_hex(value, 32, "MD5", &name, node.location))
        .transpose()?;
    let mut extensions: Vec<XmlExtension> = node
        .attributes
        .iter()
        .filter(|(field, _)| !["name", "size", "crc", "md5", "sha1"].contains(&field.as_str()))
        .map(|(field, value)| attribute_extension("rom", Some(&name), field, value, node.location))
        .collect();
    for child in node.children() {
        extensions.push(element_extension("rom", Some(&name), child)?);
    }
    Ok(Asset {
        name,
        size,
        crc,
        md5,
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
        excerpt: None,
        coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
    }
}

fn reject_record_text(node: &Element, kind: &str, name: Option<&str>) -> crate::Result<()> {
    if node
        .content
        .iter()
        .any(|content| matches!(content, ElementContent::Text(text) if !text.trim().is_empty()))
    {
        return Err(parse_error(
            "unexpected non-whitespace text in record",
            kind,
            name,
            node.location,
        ));
    }
    Ok(())
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
        value: serde_json::json!(value).into(),
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
        value: ExtensionValue::encode(element)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_pc_xml_game_identity_description_and_md5() -> crate::Result<()> {
        let catalog = Catalog::parse(
            br#"<datafile><game name="Wake (Unknown) (Windows)" id="0640"><description>Wake</description><rom name="Wake_Win.zip" size="64847823" crc="1c68eab9" md5="4177620d57a0c4ff75cf5d6f307aa7c2" sha1="50b756a9768340f71256b366241b1543bf664824" /></game></datafile>"#,
        )?;
        let entry = catalog
            .entries
            .first()
            .ok_or_else(|| crate::Error::InvalidPath("No-Intro test entry missing".into()))?;
        assert_eq!(
            entry.facts.archive_id.as_ref().map(ArchiveId::as_str),
            Some("0640")
        );
        assert_eq!(entry.facts.description.as_deref(), Some("Wake"));
        let asset = entry
            .assets
            .first()
            .ok_or_else(|| crate::Error::InvalidPath("No-Intro test ROM missing".into()))?;
        assert_eq!(
            asset.md5.as_deref(),
            Some(
                &[
                    0x41, 0x77, 0x62, 0x0d, 0x57, 0xa0, 0xc4, 0xff, 0x75, 0xcf, 0x5d, 0x6f, 0x30,
                    0x7a, 0xa7, 0xc2,
                ][..]
            )
        );
        Ok(())
    }
}
