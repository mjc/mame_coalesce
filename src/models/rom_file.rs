use camino::Utf8Path;

use crate::{hashes::MultiHash, schema::rom_files};

use super::Rom;

#[derive(
    Queryable, QueryableByName, Insertable, AsChangeset, Associations, PartialEq, Eq, Debug, Hash,
)]
#[table_name = "rom_files"]
#[diesel(table_name = rom_files)]
#[belongs_to(Rom)]
pub struct RomFile {
    pub id: i32,
    pub parent_path: String,
    pub parent_game_name: Option<String>,
    pub path: String,
    pub name: String,
    pub crc: Option<Vec<u8>>,
    pub sha1: Vec<u8>,
    pub md5: Option<Vec<u8>>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl RomFile {
    pub fn is_archive(path: &Utf8Path) -> Option<infer::Type> {
        infer::get_from_path(path)
            .ok()
            .flatten()
            .filter(|t| match t.mime_type() {
                "application/zip" | "application/x-7z-compressed" => true,
                _ => false,
            })
    }

    /// Get a reference to the rom file's path.
    pub fn path(&self) -> &str {
        self.path.as_ref()
    }

    /// Get a reference to the rom file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get the rom file's in archive.
    pub fn in_archive(&self) -> bool {
        self.in_archive
    }

    // I hate this
    pub fn in_archive_str(&self) -> &str {
        if self.in_archive {
            "true"
        } else {
            "false"
        }
    }
}

#[derive(Insertable, AsChangeset, Debug)]
#[table_name = "rom_files"]
pub struct NewRomFile {
    pub parent_path: String,
    pub path: String,
    pub name: String,
    pub sha1: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl NewRomFile {
    // TODO: should go away
    pub fn from_path(rom_file_path: &Utf8Path) -> Option<NewRomFile> {
        let (_crc, sha1) = rom_file_path.all_hashes();
        let name = rom_file_path.file_name()?.to_string();
        let parent_path = rom_file_path.parent()?.to_string();
        let path = rom_file_path.to_string();
        Some(NewRomFile {
            parent_path,
            path,
            name,
            sha1,
            in_archive: false,
            rom_id: None,
        })
    }

    pub fn from_archive(path: &Utf8Path, name: &str, sha1: Vec<u8>) -> Option<NewRomFile> {
        let parent_path = path.parent()?.to_string();
        let path = path.to_string();
        let name = name.to_string();
        Some(NewRomFile {
            parent_path,
            path,
            name,
            sha1,
            in_archive: true,
            rom_id: None,
        })
    }

    /// Get a reference to the new rom file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
