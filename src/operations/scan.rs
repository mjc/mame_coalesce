use std::{fs::File, io::BufReader, path::Path};

use camino::{Utf8Path, Utf8PathBuf};
use compress_tools::{ArchiveContents, ArchiveIterator};
use fmmap::{MmapFile, MmapFileExt};

use indicatif::ParallelProgressIterator;
use log::{info, warn};

use rayon::prelude::*;
use sha1::{Digest, Sha1};

use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::Xxh3;

use crate::{
    Error,
    db::{self, Pool},
    models::NewRomFile,
    progress,
};

pub fn source(path: &Utf8Path, jobs: usize, pool: &Pool) -> crate::Result<Utf8PathBuf> {
    info!("Looking in path: {path}");
    let file_list = walk_for_files(path);
    let new_rom_files = get_all_rom_files(&file_list, jobs);

    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    let associated_roms = db::import_rom_files(pool, &new_rom_files)?;
    if associated_roms == 0 && !new_rom_files.is_empty() {
        warn!(
            "scanned {} ROM files, but none matched imported DAT ROMs",
            new_rom_files.len()
        );
    }
    Ok(path.to_path_buf())
}

fn get_all_rom_files(file_list: &[Utf8PathBuf], jobs: usize) -> Vec<NewRomFile> {
    let bar = progress::bar(file_list.len() as u64);
    // Ignore "already initialized" error — the global pool is reused across calls.
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build_global();
    file_list
        .par_iter()
        .progress_with(bar)
        .filter_map(|p| build_new_rom_files(p))
        .flatten_iter()
        .collect()
}

fn build_new_rom_files(path: &Utf8Path) -> Option<Vec<NewRomFile>> {
    let single_rom = || NewRomFile::from_path(path).map(|nrf| vec![nrf]);
    let mmap = crate::hashes::mmap_path(path).ok()?;
    infer::get_from_path(path)
        .ok()
        .flatten()
        .map_or_else(single_rom, |t| match t.mime_type() {
            "application/zip" => scan_zip(&mmap).ok(),
            "application/x-7z-compressed" | "application/vnd.rar" => scan_libarchive(path).ok(),
            _mime_type => single_rom(),
        })
}

fn scan_zip(mmap: &MmapFile) -> crate::Result<Vec<NewRomFile>> {
    let path = Utf8Path::from_path(mmap.path())
        .ok_or_else(|| Error::InvalidPath("invalid path".to_owned()))?;
    let reader = mmap.reader(0).map_err(|e| Error::Mmap(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut rom_files = Vec::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        // enclosed_name() returns &Path in zip 2.x
        let name = file
            .enclosed_name()
            .ok_or_else(|| Error::InvalidPath(format!("invalid name inside zip: {path:?}")))?
            .clone();
        let mut nrf =
            NewRomFile::from_archive(path, &name, Vec::new(), Vec::new()).ok_or_else(|| {
                Error::InvalidPath(format!("couldn't make database entry for file: {path:?}"))
            })?;

        // Read entry into buffer so both hashers can see the data
        let mut buf = Vec::new();
        std::io::copy(&mut file, &mut buf)?;

        let sha1 = crate::hashes::sha1_bytes(&buf);
        let xxh = crate::hashes::xxhash3_bytes(&buf);
        nrf.set_sha1(sha1);
        nrf.set_xxhash3(xxh);
        rom_files.push(nrf);
    }

    Ok(rom_files)
}

fn scan_libarchive(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    let f = File::open(path)?;
    let reader = BufReader::new(f);
    // let mmap = mmap_path(path)?;
    // let chunks = mmap.chunks(16_384);
    let mut rom_files: Vec<NewRomFile> = Vec::new();
    let iter = ArchiveIterator::from_read(reader)?;

    let mut name = String::new();
    let mut sha1hasher = Sha1::new();
    let mut xxhash3 = Xxh3::new();

    iter.for_each(|content| match content {
        ArchiveContents::StartOfEntry(s, _) => {
            name = s;
            sha1hasher.reset();
            xxhash3.reset();
        }
        ArchiveContents::DataChunk(v) => {
            sha1hasher.update(&v);
            xxhash3.update(&v);
        }
        ArchiveContents::EndOfEntry => {
            let sha1 = sha1hasher.finalize_reset().to_vec();
            let xxh3 = xxhash3.digest().to_be_bytes().to_vec();
            let filename = Path::new(&name);
            if let Some(nrf) = NewRomFile::from_archive(path, filename, sha1, xxh3) {
                rom_files.push(nrf);
            }
        }
        ArchiveContents::Err(e) => {
            warn!("couldn't read {name} from {path:?}: {e:?}");
        }
    });

    Ok(rom_files)
}

fn walk_for_files(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    let v = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .flatten()
        .filter(|entry| !entry.file_type().is_dir())
        .collect();
    let optimized = optimize_file_order(v);
    optimized
        .iter()
        .filter_map(|direntry| Utf8PathBuf::from_path_buf(direntry.path().to_path_buf()).ok())
        .collect()
}

fn entry_is_relevant(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| entry.depth() == 0 || !s.starts_with('.'))
}

#[cfg(target_os = "linux")]
fn optimize_file_order(mut dirs: Vec<DirEntry>) -> Vec<DirEntry> {
    use walkdir::DirEntryExt;
    dirs.sort_by(|a, b| {
        let a_inode = a.ino();
        let b_inode = b.ino();
        a_inode.cmp(&b_inode)
    });
    dirs
}

#[cfg(not(target_os = "linux"))]
fn optimize_file_order(mut dirs: Vec<DirEntry>) -> Vec<DirEntry> {
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use zip::write::SimpleFileOptions;

    fn make_test_zip(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, data) in entries {
            zip.start_file(*name, options)?;
            zip.write_all(data)?;
        }
        Ok(zip.finish()?.into_inner())
    }

    #[test]
    fn scan_zip_computes_correct_hashes() -> Result<(), Box<dyn std::error::Error>> {
        let content = b"hello rom";
        let zip_data = make_test_zip(&[("test.rom", content)])?;

        let expected_sha1 = crate::hashes::sha1_bytes(content);
        let expected_xxh = crate::hashes::xxhash3_bytes(content);

        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &zip_data)?;

        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let mmap = crate::hashes::mmap_path(utf8_path)?;
        let rom_files = scan_zip(&mmap)?;

        assert_eq!(rom_files.len(), 1);
        assert_eq!(rom_files[0].sha1, expected_sha1);
        assert_eq!(rom_files[0].xxhash3, expected_xxh);
        Ok(())
    }

    #[test]
    fn scan_zip_multiple_entries() -> Result<(), Box<dyn std::error::Error>> {
        let entries = [("a.rom", b"aaaa" as &[u8]), ("b.rom", b"bbbb" as &[u8])];
        let zip_data = make_test_zip(&entries)?;

        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &zip_data)?;

        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let mmap = crate::hashes::mmap_path(utf8_path)?;
        let rom_files = scan_zip(&mmap)?;

        assert_eq!(rom_files.len(), 2);
        // Verify hashes differ between entries
        assert_ne!(rom_files[0].sha1, rom_files[1].sha1);
        Ok(())
    }
}
