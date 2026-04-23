use crate::{logiqx, storage::schema::games};
use diesel::{Associations, Identifiable, Insertable, Queryable};

use super::DataFile;

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug, Eq, Ord, PartialOrd, Clone)]
#[diesel(table_name = games)]
#[diesel(belongs_to(DataFile))]
#[diesel(belongs_to(Game, foreign_key = parent_id))]
pub struct Game {
    pub id: i32,
    pub name: String,
    pub is_bios: Option<String>,
    pub clone_of: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub board: Option<String>,
    pub rebuildto: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub data_file_id: Option<i32>,
    pub parent_id: Option<i32>,
}

impl Game {
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

#[derive(Insertable, Debug)]
#[diesel(table_name = games)]
pub struct New {
    pub name: String,
    pub is_bios: Option<String>,
    pub clone_of: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub board: Option<String>,
    pub rebuildto: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub data_file_id: Option<i32>,
}

impl New {
    #[must_use]
    pub fn from_logiqx(logiqx: &logiqx::Game, data_file_id: i32) -> Self {
        Self {
            name: logiqx.name().to_owned(),
            is_bios: Some(logiqx.isbios().to_owned()),
            clone_of: logiqx.cloneof().map(std::string::ToString::to_string),
            rom_of: Some(logiqx.romof().to_owned()),
            sample_of: Some(logiqx.sampleof().to_owned()),
            board: Some(logiqx.board().to_owned()),
            rebuildto: Some(logiqx.rebuildto().to_owned()),
            year: Some(logiqx.year().to_owned()),
            manufacturer: Some(logiqx.manufacturer().to_owned()),
            data_file_id: Some(data_file_id),
        }
    }
}
