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
