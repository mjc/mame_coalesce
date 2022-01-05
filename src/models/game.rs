use super::DataFile;
use crate::{logiqx, schema::games};

#[derive(
    Identifiable, Queryable, QueryableByName, AsChangeset, Associations, PartialEq, Debug, Eq, Hash,
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
}

#[derive(Insertable, AsChangeset)]
#[table_name = "games"]
pub struct NewGame {
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

impl NewGame {
    pub fn from_logiqx(logiqx: &logiqx::Game, data_file_id: &i32) -> Self {
        // TODO: don't clone, lol
        NewGame {
            name: logiqx.name.clone(),
            is_bios: Some(logiqx.isbios.clone()),
            clone_of: logiqx.cloneof().map(|x| x.clone()),
            rom_of: Some(logiqx.romof.clone()),
            sample_of: Some(logiqx.sampleof.clone()),
            board: Some(logiqx.board.clone()),
            rebuildto: Some(logiqx.rebuildto.clone()),
            year: Some(logiqx.year.clone()),
            manufacturer: Some(logiqx.manufacturer.clone()),
            data_file_id: Some(*data_file_id),
        }
    }
}
