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
    db::{self, SyncPool},
    models::NewRomFile,
    progress, MameResult,
};

pub fn scan_source(path: &Utf8Path, jobs: usize, pool: &SyncPool) -> MameResult<Utf8PathBuf> {
    info!("Looking in path: {}", path);
    let file_list = walk_for_files(path);
    let new_rom_files = get_all_rom_files(&file_list, jobs)?;

    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    db::import_rom_files(&mut pool.get()?, &new_rom_files)?;
    // TODO: warning if nothing associated
    // TODO: pick datafile to scan for
    Ok(path.to_path_buf())
}

pub fn get_all_rom_files(file_list: &Vec<Utf8PathBuf>, jobs: usize) -> MameResult<Vec<NewRomFile>> {
    let bar = progress::bar(file_list.len() as u64);
    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build_global()?;
    Ok(file_list
        .par_iter()
        .progress_with(bar)
        .filter_map(|p| build_new_rom_files(p))
        .flatten_iter()
        .collect())
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

fn scan_zip(mmap: &MmapFile) -> MameResult<Vec<NewRomFile>> {
    let path = Utf8Path::from_path(mmap.path()).ok_or("invalid path")?;
    let reader = mmap.reader(0)?;
    let mut zip = zip::ZipArchive::new(reader)?;

    let mut rom_files = Vec::new();

    let mut sha1hasher = Sha1::new();
    let xxhash3 = Xxh3::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let name = file
            .enclosed_name()
            .ok_or("invalid name inside zip: {path:?}")?;
        let mut nrf = NewRomFile::from_archive(path, name, Vec::new(), Vec::new())
            .ok_or("couldn't make database entry for file: {path:?}")?;

        std::io::copy(&mut file, &mut sha1hasher)?;
        let sha1 = sha1hasher.finalize_reset().to_vec();
        nrf.set_sha1(sha1);
        let xxh = xxhash3.digest().to_be_bytes().to_vec();
        nrf.set_xxhash3(xxh);
        rom_files.push(nrf);
    }

    Ok(rom_files)
}

fn scan_libarchive(path: &Utf8Path) -> MameResult<Vec<NewRomFile>> {
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
        ArchiveContents::StartOfEntry(s) => {
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
            warn!("couldn't read {} from {:?}: {:?}", name, path, e);
        }
    });

    Ok(rom_files)
}

pub fn walk_for_files(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
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
        .map_or(false, |s| entry.depth() == 0 || !s.starts_with('.'))
}

#[cfg(target_os = "linux")]
fn optimize_file_order(mut dirs: Vec<DirEntry>) -> Vec<DirEntry> {
    // TODO: figure out fiemap

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
