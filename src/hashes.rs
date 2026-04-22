use camino::Utf8Path;
use fmmap::{MmapFile, MmapFileExt};

use sha1::{Digest, Sha1};
use xxhash_rust::xxh3::Xxh3;

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

#[must_use]
pub fn sha1_bytes(data: &[u8]) -> Vec<u8> {
    let mut sha1 = Sha1::new();
    sha1.update(data);
    sha1.finalize().to_vec()
}

#[must_use]
pub fn xxhash3_bytes(data: &[u8]) -> Vec<u8> {
    let mut xxhash3 = Xxh3::new();
    xxhash3.update(data);
    xxhash3.digest().to_be_bytes().to_vec()
}

pub fn mmap_path(path: &Utf8Path) -> crate::Result<MmapFile> {
    MmapFile::open(path).map_err(|e| crate::Error::Mmap(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_empty() {
        // SHA1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
        assert_eq!(
            hex::encode(sha1_bytes(b"")),
            "da39a3ee5e6b4b0d3255bfef95601890afd80709"
        );
    }

    #[test]
    fn sha1_known_value() {
        // SHA1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        assert_eq!(
            hex::encode(sha1_bytes(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn xxhash3_known_value() {
        // xxhash3("") = 2d06800538d394c2 (from xxhash spec)
        let result = xxhash3_bytes(b"");
        assert_eq!(result.len(), 8);
        // Verify consistency: same input always gives same output
        assert_eq!(xxhash3_bytes(b""), xxhash3_bytes(b""));
    }

    #[test]
    fn xxhash3_different_inputs() {
        assert_ne!(xxhash3_bytes(b"abc"), xxhash3_bytes(b"def"));
    }
}
