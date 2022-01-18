use camino::Utf8Path;
use log::{debug, warn};

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
    pub crc: Vec<u8>,
    pub sha1: Vec<u8>,
    pub md5: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl RomFile {
    pub fn is_archive(path: &Utf8Path) -> bool {
        match infer::get_from_path(&path) {
            Ok(Some(kind)) => match kind.mime_type() {
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
                    warn!(
                        "Unknown mime type. assuming that it isn't an archive {:?}",
                        mime
                    );
                    false
                }
            },
            Ok(None) => {
                warn!(
                    "Unable to detect file type. Assuming it isn't an archive. {:?}",
                    &path
                );
                false
            }
            Error(e) => {
                warn!("Unable to read file: {:?}", &path)
            }
        }
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
    pub crc: Vec<u8>,
    pub sha1: Vec<u8>,
    pub md5: Vec<u8>,
    pub in_archive: bool,
    pub rom_id: Option<i32>,
}

impl NewRomFile {
    pub fn from_path(rom_file_path: &Utf8Path) -> NewRomFile {
        let (crc, sha1) = rom_file_path.all_hashes();
        let name = rom_file_path.file_name().unwrap().to_string();
        let parent = rom_file_path.parent().unwrap().to_string();
        let path = rom_file_path.to_string();
        NewRomFile {
            parent_path: parent,
            path,
            name,
            crc,
            sha1,
            md5: Vec::<u8>::new(),
            in_archive: false,
            rom_id: None,
        }
    }

    pub fn from_archive(
        path: &Utf8Path,
        name: &str,
        crc: Vec<u8>,
        sha1: Vec<u8>,
        md5: Vec<u8>,
    ) -> NewRomFile {
        NewRomFile {
            parent_path: path.parent().unwrap().to_string(),
            path: path.to_string(),
            name: name.to_string(),
            crc,
            sha1,
            md5,
            in_archive: true,
            rom_id: None,
        }
    }

    /// Get a reference to the new rom file's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
