use std::{fs::File, path::Path};

use crc32fast::Hasher;
use memmap2::MmapOptions;
use sha1::{Digest, Sha1};

pub fn compute_all_hashes(path: &Path) -> (Vec<u8>, Vec<u8>) {
    let mut crc32 = Hasher::new();
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
    )
}
