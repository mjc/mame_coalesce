#[derive(Debug, Deserialize)]
pub struct Rom {
    pub name: String,
    pub size: String,
    pub md5: String,
    pub sha1: String,
    pub crc: String,
    #[serde(default)]
    pub merge: String,
    #[serde(default)]
    pub status: String, // baddump|nodump|good|verified
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub date: String, // should probably be DateTime
}
