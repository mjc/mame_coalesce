use crate::fs;

pub fn load_datafile(name: String) -> Result<Datafile, &'static str> {
    match fs::read_to_string(name) {
        Ok(contents) => Ok(Datafile::from_str(&contents)),
        Err(_) => Err("Unable to parse datafile"),
    }
}

#[derive(Debug, Deserialize)]
pub struct Datafile {
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub debug: String, // bool
    pub header: Header,
    #[serde(rename = "game", default)]
    pub games: Vec<Game>,
}
impl Datafile {
    pub fn from_str(contents: &str) -> Self {
        serde_xml_rs::from_str(contents).expect("Can't read Logiqx datafile.")
    }
}

#[derive(Debug, Deserialize)]
pub struct Header {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub homepage: String,
    pub url: String,
}

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