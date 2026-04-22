use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Rom {
    name: String,
    size: i32,
    #[serde(with = "hex")]
    md5: Vec<u8>,
    #[serde(with = "hex")]
    sha1: Vec<u8>,
    #[serde(with = "hex")]
    crc: Vec<u8>,
    #[serde(default)]
    merge: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    serial: String,
    #[serde(default)]
    date: String,
}

impl Rom {
    /// Get a reference to the rom's name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the rom's size.
    #[must_use]
    pub const fn size(&self) -> &i32 {
        &self.size
    }

    /// Get a reference to the rom's md5.
    #[must_use]
    pub fn md5(&self) -> &Vec<u8> {
        self.md5.as_ref()
    }

    /// Get a reference to the rom's sha1.
    #[must_use]
    pub fn sha1(&self) -> &Vec<u8> {
        self.sha1.as_ref()
    }

    /// Get a reference to the rom's crc.
    #[must_use]
    pub fn crc(&self) -> &Vec<u8> {
        self.crc.as_ref()
    }

    /// Get a reference to the rom's merge.
    #[must_use]
    pub fn merge(&self) -> &str {
        self.merge.as_ref()
    }

    /// Get a reference to the rom's status.
    #[must_use]
    pub fn status(&self) -> &str {
        self.status.as_ref()
    }

    /// Get a reference to the rom's serial.
    #[must_use]
    pub fn serial(&self) -> &str {
        self.serial.as_ref()
    }

    /// Get a reference to the rom's date.
    #[must_use]
    pub fn date(&self) -> &str {
        self.date.as_ref()
    }
}
