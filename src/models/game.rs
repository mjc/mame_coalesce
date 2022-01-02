use super::DataFile;
use crate::schema::games;

#[derive(Identifiable, Queryable, AsChangeset, Associations, PartialEq)]
#[diesel(table_name = games)]
#[belongs_to(DataFile)]
pub struct Game {
    id: i32,
    name: String,
    is_bios: Option<String>,
    clone_of: Option<String>,
    rom_of: Option<String>,
    sample_of: Option<String>,
    board: Option<String>,
    rebuildto: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
    data_file_id: Option<i32>,
}
