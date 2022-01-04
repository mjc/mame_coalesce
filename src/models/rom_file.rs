use std::path::{Path, PathBuf};

use log::debug;

use crate::{hashes::MultiHash, schema::rom_files};

use super::Rom;

#[derive(Queryable, Insertable, AsChangeset, Associations, PartialEq, Debug)]
#[diesel(table_name = rom_files)]
#[belongs_to(Rom)]
pub struct RomFile {
    pub id: Option<i32>,
    pub path: String,
    pub name: String,
    pub crc: Vec<u8>,
    pub sha1: Vec<u8>,
    pub md5: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl RomFile {
    pub fn is_archive(path: &Path) -> bool {
        match tree_magic::from_filepath(&path).as_str() {
            "application/zip" => true,
            "application/x-7z-compressed" => true,
            "text/plain" => {
                debug!("Found a text file: {:?}", path.file_name());
                false
            }
            "application/x-cpio" => {
                debug!(
                    "Found an archive that calls itself cpio, this is weird: {:?}",
                    path.file_name()
                );
                true
            }
            "application/x-n64-rom" => false,
            "application/octet-stream" => {
                debug!(
                    "Only detected as a generic binary file: {:?}",
                    &path.file_name().unwrap()
                );
                false
            }
            mime => {
                debug!(
                    "Unknown mime type, assuming that it isn't an archive {:?}",
                    mime
                );
                false
            }
        }
    }
}

#[derive(Insertable, AsChangeset)]
#[table_name = "rom_files"]
pub struct NewRomFile {
    pub path: String,
    pub name: String,
    pub crc: Vec<u8>,
    pub sha1: Vec<u8>,
    pub md5: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl NewRomFile {
    pub fn from_path(path: PathBuf) -> NewRomFile {
        let (crc, sha1) = path.all_hashes();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let rom_file_path = path.to_str().unwrap().to_string();
        NewRomFile {
            path: rom_file_path,
            name: name,
            crc: crc,
            sha1: sha1,
            md5: Vec::<u8>::new(),
            in_archive: false,
            rom_id: None,
        }
    }

    pub fn from_archive(
        path: &PathBuf,
        name: &String,
        crc: Vec<u8>,
        sha1: Vec<u8>,
        md5: Vec<u8>,
    ) -> NewRomFile {
        NewRomFile {
            path: path.to_str().unwrap().to_string(),
            name: name.clone(),
            crc,
            sha1,
            md5,
            in_archive: true,
            rom_id: None,
        }
    }
}
