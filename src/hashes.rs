use std::fs::File;

use camino::Utf8Path;
use fmmap::{MmapFile, MmapFileExt};
use memmap2::{Mmap, MmapOptions};
use sha1::{Digest, Sha1};

use crate::MameResult;

pub trait MultiHash {
    fn all_hashes(&self) -> (Vec<u8>, Vec<u8>);
}

impl MultiHash for Utf8Path {
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
        let mut sha1 = Sha1::new();

        for chunk in self.chunks(16_384) {
            sha1.update(chunk);
        }

        (Vec::<u8>::default(), sha1.finalize().to_vec())
    }
}

pub fn stream_sha1(mmap: &MmapFile) -> MameResult<Vec<u8>> {
    let mut sha1 = Sha1::new();

    mmap.as_slice()
        .chunks(16_384)
        .for_each(|chunk| sha1.update(chunk));

    Ok(sha1.finalize().to_vec())
}

pub fn mmap_path(path: &Utf8Path) -> MameResult<MmapFile> {
    let mmap = MmapFile::open(path)?;
    Ok(mmap)
}
