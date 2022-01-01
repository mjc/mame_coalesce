use std::{
    fs::File,
    path::{Path, PathBuf},
};

use log::{debug, info};
use memmap2::MmapOptions;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct RomFile {
    path: PathBuf,
    crc: Vec<u8>,
    sha1: Vec<u8>,
    md5: Vec<u8>,
    in_archive: bool,
}

impl RomFile {
    pub fn from_path(path: PathBuf, in_archive: bool) -> RomFile {
        let (crc, sha1, md5) = Self::compute_hashes(&path);
        RomFile {
            path: path,
            crc: crc,
            sha1: sha1,
            md5: md5,
            in_archive: in_archive,
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
                println!("Found a text file: {:?}", path.file_name());
                false
            }
            "application/x-cpio" => {
                println!(
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

    /// Get a reference to the rom file's path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get a reference to the rom file's crc.
    pub fn crc(&self) -> &[u8] {
        self.crc.as_ref()
    }

    /// Get a reference to the rom file's sha1.
    pub fn sha1(&self) -> &[u8] {
        self.sha1.as_ref()
    }

    /// Get a reference to the rom file's md5.
    pub fn md5(&self) -> &[u8] {
        self.md5.as_ref()
    }

    /// Get a reference to the rom file's in archive.
    pub fn in_archive(&self) -> bool {
        self.in_archive
    }
}
