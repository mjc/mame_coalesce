use std::path::Path;

use camino::Utf8Path;
use diesel::{Associations, Insertable, Queryable};

use crate::{
    hashes::{self, Sha1Digest, Xxh3Digest},
    storage::schema::rom_files,
};

#[derive(Queryable, Associations, PartialEq, Eq, Debug, Hash)]
#[diesel(table_name = rom_files)]
#[diesel(belongs_to(crate::storage::models::Rom))]
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
    pub archive_backend: Option<String>,
    pub archive_member_index: Option<i64>,
    pub rom_id: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = rom_files)]
pub struct New {
    pub parent_path: String,
    pub path: String,
    pub name: String,
    pub sha1: Sha1Digest,
    pub xxhash3: Xxh3Digest,
    pub in_archive: bool,
    pub archive_backend: Option<String>,
    pub archive_member_index: Option<i64>,
    pub rom_id: Option<i32>,
}

impl New {
    #[must_use]
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
            archive_backend: None,
            archive_member_index: None,
            rom_id: None,
        })
    }

    #[must_use]
    pub fn from_archive(
        path: &Utf8Path,
        name: &Path,
        sha1: Sha1Digest,
        xxhash3: Xxh3Digest,
        backend: crate::domain::ArchiveBackend,
        index: u64,
    ) -> Option<Self> {
        let parent_path = path.parent()?.to_string();
        let path = path.to_string();
        let name = name.to_str()?.to_owned();
        let archive_member_index = i64::try_from(index).ok()?;
        Some(Self {
            parent_path,
            path,
            name,
            sha1,
            xxhash3,
            in_archive: true,
            archive_backend: Some(backend.storage_key().to_owned()),
            archive_member_index: Some(archive_member_index),
            rom_id: None,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
