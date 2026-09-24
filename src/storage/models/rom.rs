use chrono::{NaiveDate, NaiveDateTime};
use diesel::{Associations, Insertable, Queryable};

use super::Game;
use crate::{Error, logiqx, storage::schema::roms};

#[derive(Queryable, Insertable, Associations, PartialEq, Eq, Debug, Hash)]
#[diesel(table_name = roms)]
#[diesel(belongs_to(Game))]
pub struct Rom {
    pub id: i32,
    pub name: String,
    pub size: i32,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: Option<NaiveDate>,
    pub updated_at: Option<NaiveDateTime>,
    pub inserted_at: Option<NaiveDateTime>,
    pub game_id: Option<i32>,
    pub archive_file_id: Option<i32>,
}

impl Rom {
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = roms)]
pub struct New {
    pub name: String,
    pub size: i32,
    pub md5: Vec<u8>,
    pub sha1: Vec<u8>,
    pub crc: Vec<u8>,
    pub date: Option<String>,
    pub updated_at: Option<String>,
    pub inserted_at: Option<String>,
    pub game_id: i32,
}

impl New {
    pub fn from_logiqx(rom: &logiqx::Rom, game_id: i32) -> crate::Result<Self> {
        let size = rom.size().ok_or_else(|| {
            Error::InvalidPath(format!(
                "Logiqx ROM {} has no size; current cache schema requires it",
                rom.name()
            ))
        })?;
        let size = i32::try_from(size).map_err(|_| Error::InvalidRomSize(size))?;
        let required_hash = |hash: Option<&[u8]>, name: &str| {
            hash.map(<[u8]>::to_vec).ok_or_else(|| {
                Error::InvalidHash(format!(
                    "Logiqx ROM {} has no {name}; current cache schema requires it",
                    rom.name()
                ))
            })
        };
        Ok(Self {
            name: rom.name().to_owned(),
            size,
            md5: required_hash(rom.md5(), "MD5")?,
            sha1: required_hash(rom.sha1(), "SHA1")?,
            crc: required_hash(rom.crc(), "CRC")?,
            date: None,
            updated_at: None,
            inserted_at: None,
            game_id,
        })
    }
}
