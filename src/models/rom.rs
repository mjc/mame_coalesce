use chrono::{NaiveDate, NaiveDateTime};

use super::Game;
use crate::{logiqx, schema::roms};

#[derive(Queryable, Insertable, Associations, PartialEq, Eq, Debug, Hash)]
#[diesel(table_name = roms)]
#[belongs_to(Game)]
pub struct Rom {
    pub id: i32,
    pub name: String,
    pub size: i32,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: Option<NaiveDate>, // utc date
    pub updated_at: Option<NaiveDateTime>,
    pub inserted_at: Option<NaiveDateTime>,
    pub game_id: Option<i32>,
    pub archive_file_id: Option<i32>,
}

impl Rom {
    /// Get a reference to the rom's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

#[derive(Insertable, Debug)]
#[table_name = "roms"]
pub struct New {
    pub name: String,
    pub size: i32,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: Option<String>,        // utc date
    pub updated_at: Option<String>,  // utc date time
    pub inserted_at: Option<String>, // utc date time
    pub game_id: i32,
}

impl New {
    pub fn from_logiqx(rom: &logiqx::Rom, game_id: i32) -> Self {
        Self {
            name: rom.name().to_owned(),
            size: *rom.size(),
            md5: rom.md5().clone(),
            sha1: rom.sha1().clone(),
            crc: rom.crc().clone(),
            date: None,
            updated_at: None,
            inserted_at: None,
            game_id,
        }
    }
}
