use crate::{logiqx::RecordLocation, xml_reader::Element};

#[derive(Debug)]
pub struct Rom {
    name: String,
    size: Option<u64>,
    md5: Option<Vec<u8>>,
    sha1: Option<Vec<u8>>,
    crc: Option<Vec<u8>>,
    merge: Option<String>,
    status: Option<String>,
    serial: Option<String>,
    date: Option<String>,
    status_was_explicit: bool,
    location: RecordLocation,
}

impl Rom {
    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let status = element.attributes.get("status").cloned();
        if status
            .as_deref()
            .is_some_and(|value| !["baddump", "nodump", "good", "verified"].contains(&value))
        {
            return Err(crate::Error::XmlValidation(format!(
                "invalid status value {:?} on <rom>",
                status.as_deref().unwrap_or_default()
            )));
        }
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
            status,
            serial: element.attributes.get("serial").cloned(),
            date: element.attributes.get("date").cloned(),
            status_was_explicit: element.attributes.contains_key("status"),
            location: element.location,
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
    pub fn effective_status(&self) -> &str {
        self.status.as_deref().unwrap_or("good")
    }

    #[must_use]
    pub const fn status_was_explicit(&self) -> bool {
        self.status_was_explicit
    }

    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
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

#[derive(Debug)]
pub struct Disk {
    name: String,
    sha1: Option<Vec<u8>>,
    md5: Option<Vec<u8>>,
    merge: Option<String>,
    status: Option<String>,
    status_was_explicit: bool,
    location: RecordLocation,
}

impl Disk {
    pub(crate) fn from_xml(element: &Element) -> crate::Result<Self> {
        let status = element.attributes.get("status").cloned();
        if status
            .as_deref()
            .is_some_and(|value| !["baddump", "nodump", "good", "verified"].contains(&value))
        {
            return Err(crate::Error::XmlValidation(format!(
                "invalid status value {:?} on <disk>",
                status.as_deref().unwrap_or_default()
            )));
        }
        Ok(Self {
            name: element.required_attribute("name")?,
            sha1: xml_hash(element, "sha1", 20)?,
            md5: xml_hash(element, "md5", 16)?,
            merge: element.attributes.get("merge").cloned(),
            status,
            status_was_explicit: element.attributes.contains_key("status"),
            location: element.location,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn sha1(&self) -> Option<&[u8]> {
        self.sha1.as_deref()
    }
    #[must_use]
    pub fn md5(&self) -> Option<&[u8]> {
        self.md5.as_deref()
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
    pub fn effective_status(&self) -> &str {
        self.status.as_deref().unwrap_or("good")
    }
    #[must_use]
    pub const fn status_was_explicit(&self) -> bool {
        self.status_was_explicit
    }
    #[must_use]
    pub const fn location(&self) -> RecordLocation {
        self.location
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
