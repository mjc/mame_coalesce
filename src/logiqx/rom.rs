#[derive(Debug, Deserialize)]
pub struct Rom {
    pub name: String,
    pub size: i32,
    #[serde(with = "hex")]
    pub md5: Vec<u8>,
    #[serde(with = "hex")]
    pub sha1: Vec<u8>,
    #[serde(with = "hex")]
    pub crc: Vec<u8>,
    #[serde(default)]
    pub merge: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub date: String,
}

impl Rom {
    /// Get a reference to the rom's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the rom's size.
    pub fn size(&self) -> &i32 {
        &self.size
    }

    /// Get a reference to the rom's md5.
    pub fn md5(&self) -> &Vec<u8> {
        self.md5.as_ref()
    }

    /// Get a reference to the rom's sha1.
    pub fn sha1(&self) -> &Vec<u8> {
        self.sha1.as_ref()
    }

    /// Get a reference to the rom's crc.
    pub fn crc(&self) -> &Vec<u8> {
        self.crc.as_ref()
    }
}
