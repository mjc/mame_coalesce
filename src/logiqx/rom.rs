use crate::xml_reader::Element;
use serde::{Deserialize, Deserializer, de::Error as _};

#[derive(Debug, Deserialize)]
pub struct Rom {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@size")]
    size: Option<u64>,
    #[serde(rename = "@md5", default, deserialize_with = "deserialize_md5")]
    md5: Option<Vec<u8>>,
    #[serde(rename = "@sha1", default, deserialize_with = "deserialize_sha1")]
    sha1: Option<Vec<u8>>,
    #[serde(rename = "@crc", default, deserialize_with = "deserialize_crc")]
    crc: Option<Vec<u8>>,
    #[serde(rename = "@merge")]
    merge: Option<String>,
    #[serde(rename = "@status")]
    status: Option<String>,
    #[serde(rename = "@serial")]
    serial: Option<String>,
    #[serde(rename = "@date")]
    date: Option<String>,
}

impl Rom {
    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let size = element
            .attributes
            .get("size")
            .map(|value| {
                value.parse().map_err(|error| {
                    crate::Error::XmlValidation(format!("invalid ROM size {value:?}: {error}"))
                })
            })
            .transpose()?;
        Ok(Self {
            name: element.required_attribute("name")?,
            size,
            md5: xml_hash(element, "md5", 16)?,
            sha1: xml_hash(element, "sha1", 20)?,
            crc: xml_hash(element, "crc", 4)?,
            merge: element.attributes.get("merge").cloned(),
            status: element.attributes.get("status").cloned(),
            serial: element.attributes.get("serial").cloned(),
            date: element.attributes.get("date").cloned(),
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        self.size
    }

    #[must_use]
    pub fn md5(&self) -> Option<&[u8]> {
        self.md5.as_deref()
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&[u8]> {
        self.sha1.as_deref()
    }

    #[must_use]
    pub fn crc(&self) -> Option<&[u8]> {
        self.crc.as_deref()
    }

    #[must_use]
    pub fn merge(&self) -> Option<&str> {
        self.merge.as_deref()
    }

    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    #[must_use]
    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }

    #[must_use]
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }
}

fn xml_hash(
    element: &Element,
    name: &str,
    expected_bytes: usize,
) -> crate::Result<Option<Vec<u8>>> {
    element
        .attributes
        .get(name)
        .map(|value| {
            let bytes = hex::decode(value).map_err(|error| {
                crate::Error::XmlValidation(format!("invalid {name} digest: {error}"))
            })?;
            if bytes.len() != expected_bytes {
                return Err(crate::Error::XmlValidation(format!(
                    "{name} digest has {} bytes; expected {expected_bytes}",
                    bytes.len()
                )));
            }
            Ok(bytes)
        })
        .transpose()
}

fn deserialize_hash<'de, D>(deserializer: D, byte_len: usize) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let bytes = hex::decode(value).map_err(D::Error::custom)?;
    if bytes.len() != byte_len {
        return Err(D::Error::custom(format!(
            "hash has {} bytes; expected {byte_len}",
            bytes.len()
        )));
    }
    Ok(Some(bytes))
}

fn deserialize_md5<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_hash(deserializer, 16)
}

fn deserialize_sha1<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_hash(deserializer, 20)
}

fn deserialize_crc<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_hash(deserializer, 4)
}
