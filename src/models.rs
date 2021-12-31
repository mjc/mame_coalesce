use crate::diesel::{Insertable, Queryable};

use crate::schema::{data_files, games, roms};

#[derive(Identifiable, Queryable, Associations, PartialEq, Debug)]
pub struct DataFile {
    id: i32,
    build: Option<String>,
    debug: Option<String>,
    file_name: Option<String>,
    name: String,
    description: Option<String>,
    category: Option<String>,
    version: Option<String>,
    author: Option<String>,
    email: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
}

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

#[derive(Queryable)]
pub struct Rom {
    id: i32,

    name: String,
    md5: Vec<u8>,
    sha1: Vec<u8>,
    crc: Vec<u8>,
    date: String,        // utc date
    updated_at: String,  // utc datetime
    inserted_at: String, // utc datetime
    game_id: i32,
}

#[derive(Queryable)]
pub struct File {
    id: Option<i32>,
    path: String,
    name: String,
    crc: Vec<u8>,
    sha1: Vec<u8>,
    md5: Vec<u8>,
    in_archive: bool,
    rom_id: Option<i32>,
}
