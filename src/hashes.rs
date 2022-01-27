use camino::Utf8Path;
use fmmap::{MmapFile, MmapFileExt};

use sha1::{Digest, Sha1};

use crate::MameResult;

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
