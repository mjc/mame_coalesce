use std::path::Path;

use camino::Utf8Path;

use crate::{hashes, schema::rom_files};

use super::Rom;

#[derive(Queryable, QueryableByName, Insertable, Associations, PartialEq, Eq, Debug, Hash)]
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
    pub xxhash3: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl RomFile {
    /// Get a reference to the rom file's path.
    pub fn path(&self) -> &str {
        self.path.as_ref()
    }

    /// Get a reference to the rom file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get the rom file's in archive.
    pub const fn in_archive(&self) -> bool {
        self.in_archive
    }

    // I hate this
    pub const fn in_archive_str(&self) -> &str {
        if self.in_archive {
            "true"
        } else {
            "false"
        }
    }
}

#[derive(Insertable, Debug)]
#[table_name = "rom_files"]
pub struct New {
    pub parent_path: String,
    pub path: String,
    pub name: String,
    pub sha1: Vec<u8>,
    pub xxhash3: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl New {
    // TODO: should go away
    pub fn from_path(path: &Utf8Path) -> Option<Self> {
        let mmap = hashes::mmap_path(path).ok()?;
        let sha1 = hashes::stream_sha1(&mmap);
        let xxhash3 = hashes::stream_xxhash3(&mmap);

        let name = path.file_name()?.to_owned();
        let parent_path = path.parent()?.to_string();
        let path = path.to_string();
        Some(Self {
            parent_path,
            path,
            name,
            sha1,
            xxhash3,
            in_archive: false,
            rom_id: None,
        })
    }

    pub fn from_archive(
        path: &Utf8Path,
        name: &Path,
        sha1: Vec<u8>,
        xxhash3: Vec<u8>,
    ) -> Option<Self> {
        let parent_path = path.parent()?.to_string();
        let path = path.to_string();
        let name = name.to_str()?.to_owned();
        Some(Self {
            parent_path,
            path,
            name,
            sha1,
            xxhash3,
            in_archive: true,
            rom_id: None,
        })
    }

    /// Get a reference to the new rom file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Set the new rom file's sha1.
    pub fn set_sha1(&mut self, sha1: Vec<u8>) {
        self.sha1 = sha1;
    }

    /// Set the new rom file's xxhash3.
    pub fn set_xxhash3(&mut self, xxhash3: Vec<u8>) {
        self.xxhash3 = xxhash3;
    }
}
