use super::DataFile;
use crate::{logiqx, schema::games};

#[derive(
    Identifiable,
    Queryable,
    QueryableByName,
    AsChangeset,
    Associations,
    PartialEq,
    Debug,
    Eq,
    Ord,
    PartialOrd,
    Clone,
)]
#[diesel(table_name = games)]
#[table_name = "games"]
#[belongs_to(DataFile)]
#[belongs_to(Game, foreign_key = "parent_id")]
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
    /// Get a reference to the game's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
    pub fn default() -> Game {
        Game {
            id: -1,
            name: "".to_string(),
            is_bios: None,
            clone_of: None,
            rom_of: None,
            sample_of: None,
            board: None,
            rebuildto: None,
            year: None,
            manufacturer: None,
            data_file_id: None,
            parent_id: None,
        }
    }
}

#[derive(Insertable, AsChangeset)]
#[table_name = "games"]
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
    pub fn from_logiqx(logiqx: &logiqx::Game, data_file_id: &i32) -> Self {
        // TODO: don't clone, lol
        New {
            name: logiqx.name().to_string(),
            is_bios: Some(logiqx.isbios().to_string()),
            clone_of: logiqx.cloneof().map(|c| c.to_string()),
            rom_of: Some(logiqx.romof().to_string()),
            sample_of: Some(logiqx.sampleof().to_string()),
            board: Some(logiqx.board().to_string()),
            rebuildto: Some(logiqx.rebuildto().to_string()),
            year: Some(logiqx.year().to_string()),
            manufacturer: Some(logiqx.manufacturer().to_string()),
            data_file_id: Some(*data_file_id),
        }
    }
}
