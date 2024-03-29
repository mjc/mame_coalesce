use camino::Utf8Path;
use fmmap::{MmapFile, MmapFileExt};

use sha1::{Digest, Sha1};
use xxhash_rust::xxh3::Xxh3;

use crate::MameResult;

pub fn stream_sha1(mmap: &MmapFile) -> Vec<u8> {
    let mut sha1 = Sha1::new();

    mmap.as_slice()
        .chunks(0x4000)
        .for_each(|chunk| sha1.update(chunk));

    sha1.finalize().to_vec()
}

pub fn stream_xxhash3(mmap: &MmapFile) -> Vec<u8> {
    let mut xxhash3 = Xxh3::new();

    mmap.as_slice()
        .chunks(0x4000)
        .for_each(|chunk| xxhash3.update(chunk));
    xxhash3.digest().to_be_bytes().to_vec()
}

pub fn mmap_path(path: &Utf8Path) -> MameResult<MmapFile> {
    let mmap = MmapFile::open(path)?;
    Ok(mmap)
}
