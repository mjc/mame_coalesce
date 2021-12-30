use super::Rom;

#[derive(Debug, Deserialize)]
pub struct Game {
    pub name: String,
    #[serde(default)]
    pub sourcefile: String,
    #[serde(default)]
    pub isbios: String, // bool
    #[serde(default)]
    pub cloneof: String,
    #[serde(default)]
    pub romof: String,
    #[serde(default)]
    pub sampleof: String,
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub rebuildto: String,
    #[serde(default)]
    pub year: String, // should probably be a DateTime
    #[serde(default)]
    pub manufacturer: String,
    #[serde(rename = "rom", default)]
    pub roms: Vec<Rom>,
}
