use std::{
    fs::File,
    path::{Path, PathBuf},
};

use log::info;
use memmap2::MmapOptions;
use sha1::{Digest, Sha1};

use crate::schema::rom_files;

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
    pub fn from_path(path: PathBuf, in_archive: bool) -> RomFile {
        let (crc, sha1, md5) = Self::compute_hashes(&path);
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let rom_file_path = path.to_str().unwrap().to_string();
        RomFile {
            id: None,
            path: rom_file_path,
            name: name,
            crc: crc,
            sha1: sha1,
            md5: md5,
            in_archive: in_archive,
            rom_id: None,
        }
    }

    fn compute_hashes(path: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut crc32 = crc32fast::Hasher::new();
        let mut sha1 = Sha1::new();

        let f = File::open(path).unwrap();
        // takes forever on large files without mmap.
        let mmap = unsafe { MmapOptions::new().map_copy_read_only(&f).unwrap() };

        // iterate over 16KB chunks
        for chunk in mmap.chunks(16_384) {
            crc32.update(chunk);
            sha1.update(chunk);
        }

        (
            crc32.finalize().to_le_bytes().to_vec(), // check if LE is correct here
            sha1.finalize().to_vec(),
            Vec::<u8>::new(),
        )
    }

    pub fn is_archive(path: &Path) -> bool {
        match tree_magic::from_filepath(&path).as_str() {
            "application/zip" => true,
            "application/x-7z-compressed" => true,
            "text/plain" => {
                info!("Found a text file: {:?}", path.file_name());
                false
            }
            "application/x-cpio" => {
                info!(
                    "Found an archive that calls itself cpio, this is weird: {:?}",
                    path.file_name()
                );
                true
            }
            "application/x-n64-rom" => false,
            mime => {
                info!(
                    "Unknown mime type, assuming that it isn't an archive {:?}",
                    mime
                );
                false
            }
        }
    }
}
