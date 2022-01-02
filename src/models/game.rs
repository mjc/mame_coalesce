use super::DataFile;
use crate::schema::games;

#[derive(Identifiable, Queryable, Associations, PartialEq)]
#[belongs_to(DataFile)]
pub struct Game {
    id: i32,
    name: String,
    is_bios: Option<bool>,
    clone_of: Option<i32>,
    rom_of: Option<i32>,
    sample_of: Option<i32>,
    board: Option<String>,
    rebuildto: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
    data_file_id: i32,
}
