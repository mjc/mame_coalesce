use crate::{logiqx::RecordLocation, xml_reader::Element};

#[derive(Debug)]
pub struct Rom {
    name: String,
    size: SizeAttributeField,
    md5: DigestAttributeField,
    sha1: DigestAttributeField,
    crc: DigestAttributeField,
    merge: Option<String>,
    status: Option<String>,
    serial: Option<String>,
    date: Option<String>,
    status_was_explicit: bool,
    location: RecordLocation,
}

#[derive(Debug)]
enum SizeAttributeField {
    Missing,
    Valid { text: String, value: u64 },
    Uninterpreted(String),
}

impl SizeAttributeField {
    fn from_attribute(value: Option<&str>) -> Self {
        let Some(text) = value else {
            return Self::Missing;
        };

        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Self::Uninterpreted(text.to_owned());
        }

        match text.parse::<u64>() {
            Ok(value) if i64::try_from(value).is_ok() => Self::Valid {
                text: text.to_owned(),
                value,
            },
            _ => Self::Uninterpreted(text.to_owned()),
        }
    }

    const fn value(&self) -> Option<u64> {
        match self {
            Self::Valid { value, .. } => Some(*value),
            Self::Missing | Self::Uninterpreted(_) => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Missing => None,
            Self::Valid { text, .. } | Self::Uninterpreted(text) => Some(text),
        }
    }

    const fn is_uninterpreted(&self) -> bool {
        matches!(self, Self::Uninterpreted(_))
    }
}

#[derive(Debug)]
enum DigestAttributeField {
    Missing,
    Valid { text: String, bytes: Vec<u8> },
    Uninterpreted(String),
}

impl DigestAttributeField {
    fn from_attribute(value: Option<&str>, expected_bytes: usize) -> Self {
        let Some(text) = value else {
            return Self::Missing;
        };

        if text.len() != expected_bytes * 2 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Self::Uninterpreted(text.to_owned());
        }

        hex::decode(text).map_or_else(
            |_| Self::Uninterpreted(text.to_owned()),
            |bytes| Self::Valid {
                text: text.to_owned(),
                bytes,
            },
        )
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Missing => None,
            Self::Valid { text, .. } | Self::Uninterpreted(text) => Some(text),
        }
    }

    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Valid { bytes, .. } => Some(bytes),
            Self::Missing | Self::Uninterpreted(_) => None,
        }
    }

    const fn is_uninterpreted(&self) -> bool {
        matches!(self, Self::Uninterpreted(_))
    }
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
        Ok(Self {
            name: element.required_attribute("name")?,
            size: SizeAttributeField::from_attribute(
                element.attributes.get("size").map(String::as_str),
            ),
            md5: DigestAttributeField::from_attribute(
                element.attributes.get("md5").map(String::as_str),
                16,
            ),
            sha1: DigestAttributeField::from_attribute(
                element.attributes.get("sha1").map(String::as_str),
                20,
            ),
            crc: DigestAttributeField::from_attribute(
                element.attributes.get("crc").map(String::as_str),
                4,
            ),
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
        self.size.value()
    }

    #[must_use]
    pub fn size_text(&self) -> Option<&str> {
        self.size.text()
    }

    #[must_use]
    pub fn md5_text(&self) -> Option<&str> {
        self.md5.text()
    }

    #[must_use]
    pub fn sha1_text(&self) -> Option<&str> {
        self.sha1.text()
    }

    #[must_use]
    pub fn crc_text(&self) -> Option<&str> {
        self.crc.text()
    }

    #[must_use]
    pub(crate) const fn has_uninterpreted_fields(&self) -> bool {
        self.size.is_uninterpreted()
            || self.md5.is_uninterpreted()
            || self.sha1.is_uninterpreted()
            || self.crc.is_uninterpreted()
    }

    #[must_use]
    pub fn md5(&self) -> Option<&[u8]> {
        self.md5.bytes()
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&[u8]> {
        self.sha1.bytes()
    }

    #[must_use]
    pub fn crc(&self) -> Option<&[u8]> {
        self.crc.bytes()
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
    sha1: DigestAttributeField,
    md5: DigestAttributeField,
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
            sha1: DigestAttributeField::from_attribute(
                element.attributes.get("sha1").map(String::as_str),
                20,
            ),
            md5: DigestAttributeField::from_attribute(
                element.attributes.get("md5").map(String::as_str),
                16,
            ),
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
        self.sha1.bytes()
    }

    #[must_use]
    pub fn sha1_text(&self) -> Option<&str> {
        self.sha1.text()
    }

    #[must_use]
    pub fn md5(&self) -> Option<&[u8]> {
        self.md5.bytes()
    }

    #[must_use]
    pub fn md5_text(&self) -> Option<&str> {
        self.md5.text()
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        logiqx::{
            RecordLocation,
            rom::{Disk, Rom},
        },
        xml_reader::{Element, ElementContent},
    };

    fn element(name: &str, attributes: &[(&str, &str)]) -> Element {
        Element {
            attributes: attributes
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            content: vec![ElementContent::Text(String::new())],
            name: name.to_owned(),
            location: RecordLocation { line: 1, column: 1 },
        }
    }

    #[test]
    fn raw_logiqx_cdata_preserves_declared_values_without_invalid_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let rom = Rom::from_xml(&element(
            "rom",
            &[
                ("name", "source.bin"),
                ("size", "9223372036854775808"),
                ("crc", "00G00000"),
                ("md5", "aBcDeF0123456789aBcDeF0123456789"),
                ("sha1", ""),
            ],
        ))?;

        assert_eq!(rom.size(), None);
        assert_eq!(rom.size_text(), Some("9223372036854775808"));
        assert_eq!(rom.crc_text(), Some("00G00000"));
        assert_eq!(rom.crc(), None);
        assert_eq!(rom.md5_text(), Some("aBcDeF0123456789aBcDeF0123456789"));
        assert_eq!(
            rom.md5(),
            Some(hex::decode("aBcDeF0123456789aBcDeF0123456789")?.as_slice())
        );
        assert_eq!(rom.sha1_text(), Some(""));
        assert_eq!(rom.sha1(), None);

        for (size_text, expected_size) in [
            ("+1", None),
            (" 1", None),
            ("1 ", None),
            ("١", None),
            ("00042", Some(42)),
            ("9223372036854775807", Some(i64::MAX as u64)),
            ("9223372036854775808", None),
        ] {
            let rom = Rom::from_xml(&element(
                "rom",
                &[("name", "size.bin"), ("size", size_text)],
            ))?;
            assert_eq!(rom.size_text(), Some(size_text));
            assert_eq!(rom.size(), expected_size);
        }

        let zeroes = "0".repeat(500);
        let rom = Rom::from_xml(&element(
            "rom",
            &[("name", "zeroes.bin"), ("size", zeroes.as_str())],
        ))?;
        assert_eq!(rom.size_text(), Some(zeroes.as_str()));
        assert_eq!(rom.size(), Some(0));

        let disk = Disk::from_xml(&element(
            "disk",
            &[("name", "source.chd"), ("sha1", "invalid"), ("md5", "")],
        ))?;
        assert_eq!(disk.sha1_text(), Some("invalid"));
        assert_eq!(disk.sha1(), None);
        assert_eq!(disk.md5_text(), Some(""));
        assert_eq!(disk.md5(), None);
        Ok(())
    }
}
