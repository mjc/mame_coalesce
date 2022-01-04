use chrono::{NaiveDate, NaiveDateTime};

use super::Game;
use crate::{logiqx, schema::roms};

#[derive(Queryable, Insertable, AsChangeset, Associations, PartialEq, Debug)]
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

#[derive(Insertable, AsChangeset)]
#[table_name = "roms"]
pub struct NewRom {
    pub name: String,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: String,                // utc date
    pub updated_at: Option<String>,  // utc datetime
    pub inserted_at: Option<String>, // utc datetime
    pub game_id: i32,
}

impl NewRom {
    pub fn from_logiqx(rom: &logiqx::Rom, game_id: &i32) -> NewRom {
        NewRom {
            name: rom.name().to_string(),
            md5: rom.md5().to_vec(),
            sha1: rom.sha1().to_vec(),
            crc: rom.crc().to_vec(),
            date: "".to_string(),
            updated_at: None,
            inserted_at: None,
            game_id: *game_id as i32,
        }
    }
}
