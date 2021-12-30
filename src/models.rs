use crate::diesel::{Insertable, Queryable};

use crate::schema::{data_files, games, roms};

#[derive(Queryable)]
pub struct DataFile {
    id: i32,
    build: String,
    debug: String,
    file_name: String,
    name: String,
    description: String,
    category: String,
    version: String,
    author: String,
    email: String,
    homepage: String,
    url: String,
}

#[derive(Insertable)]
#[table_name = "data_files"]
pub struct NewDataFile<'a> {
    pub build: &'a str,
    pub debug: &'a str,
    pub file_name: &'a str,
    pub name: &'a str,
    pub description: &'a str,
    pub version: &'a str,
    pub author: &'a str,
    pub homepage: &'a str,
    pub url: &'a str,
}

#[derive(Queryable)]
pub struct Game {
    id: i32,
    name: String,
    is_bios: bool,
    clone_of: i32,  // should be a relation
    rom_of: i32,    // should be a relation
    sample_of: i32, // should be a relation
    board: String,
    rebuildto: String,
    year: String,
    manufacturer: String,
    data_file_id: i32,
}

// #[derive(Insertable)]
// #[table_name = "games"]
// pub struct NewGame<'a> {
//     name: &'a str,
//     is_bios: &'a bool,
//     clone_of: &'a i32,  // should be a relation
//     rom_of: &'a i32,    // should be a relation
//     sample_of: &'a i32, // should be a relation
//     board: &'a str,
//     rebuildto: &'a str,
//     year: &'a str,
//     manufacturer: &'a str,
//     data_file_id: &'a i32,
// }

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

// #[derive(Insertable)]
// #[table_name = "roms"]
// pub struct NewRom<'a> {
//     name: &'a str,
//     md5: &'a Vec<u8>,
//     sha1: &'a Vec<u8>,
//     crc: &'a Vec<u8>,
//     date: &'a str,        // utc date
//     updated_at: &'a str,  // utc datetime
//     inserted_at: &'a str, // utc datetime
//     game_id: &'a i32,
// }
