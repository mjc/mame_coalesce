use std::{fs::File, path::Path};

use crc32fast::Hasher;
use memmap2::{Mmap, MmapOptions};
use sha1::{Digest, Sha1};

pub trait MultiHash {
    fn all_hashes(&self) -> (Vec<u8>, Vec<u8>);
}

impl MultiHash for Path {
    fn all_hashes(&self) -> (Vec<u8>, Vec<u8>) {
        File::open(self).unwrap().all_hashes()
    }
}

impl MultiHash for File {
    fn all_hashes(&self) -> (Vec<u8>, Vec<u8>) {
        // takes forever on large files without mmap.
        let mmap = unsafe { MmapOptions::new().map_copy_read_only(self).unwrap() };
        mmap.all_hashes()
    }
}

impl MultiHash for Mmap {
    fn all_hashes(&self) -> (Vec<u8>, Vec<u8>) {
        let mut crc32 = Hasher::new();
        let mut sha1 = Sha1::new();

        for chunk in self.chunks(16_384) {
            crc32.update(chunk);
            sha1.update(chunk);
        }

        (
            crc32.finalize().to_le_bytes().to_vec(), // TODO: check if LE is correct here
            sha1.finalize().to_vec(),
        )
    }
}
