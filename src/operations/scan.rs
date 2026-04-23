use std::{
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
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
    hashes::{Sha1Digest, Xxh3Digest},
    models::NewRomFile,
    progress,
};

pub fn source(path: &Utf8Path, jobs: usize, pool: &Pool) -> crate::Result<Utf8PathBuf> {
    let source_root = path.canonicalize_utf8()?;
    info!("Looking in path: {source_root}");
    let excluded_paths = db::database_file_paths(pool)?;
    let file_list = walk_for_files(&source_root, &excluded_paths);
    let new_rom_files = get_all_rom_files(&file_list, jobs)?;

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
    Ok(source_root)
}

fn get_all_rom_files(file_list: &[Utf8PathBuf], jobs: usize) -> crate::Result<Vec<NewRomFile>> {
    let bar = progress::bar(file_list.len() as u64);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs).build()?;
    pool.install(|| {
        file_list
            .par_iter()
            .progress_with(bar)
            .try_fold(Vec::new, |mut rom_files, path| {
                rom_files.extend(scan_path(path)?);
                Ok(rom_files)
            })
            .try_reduce(Vec::new, |mut left, mut right| {
                left.append(&mut right);
                Ok(left)
            })
    })
}

fn scan_path(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    let mmap = crate::hashes::mmap_path(path)?;
    infer::get_from_path(path)?.map_or_else(
        || scan_bare_file(path),
        |file_type| match file_type.mime_type() {
            "application/zip" => scan_zip(&mmap),
            "application/x-7z-compressed" => scan_7z(path),
            "application/vnd.rar" => scan_rar(path),
            _mime_type => scan_bare_file(path),
        },
    )
}

fn scan_bare_file(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    NewRomFile::from_path(path)
        .map(|nrf| vec![nrf])
        .ok_or_else(|| Error::InvalidPath(format!("couldn't scan file: {path}")))
}

fn scan_zip(mmap: &MmapFile) -> crate::Result<Vec<NewRomFile>> {
    let path = Utf8Path::from_path(mmap.path())
        .ok_or_else(|| Error::InvalidPath("invalid path".to_owned()))?;
    let reader = mmap.reader(0).map_err(|e| Error::Mmap(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(reader)?;
    let mut rom_files = Vec::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        // enclosed_name() returns &Path in zip 2.x
        let name = file
            .enclosed_name()
            .ok_or_else(|| Error::InvalidPath(format!("invalid name inside zip: {path:?}")))?
            .clone();

        let (sha1, xxh3) = hash_reader_chunks(&mut file)?;
        let nrf = NewRomFile::from_archive(path, &name, sha1, xxh3).ok_or_else(|| {
            Error::InvalidPath(format!("couldn't make database entry for file: {path:?}"))
        })?;
        rom_files.push(nrf);
    }

    Ok(rom_files)
}

fn scan_rar(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    let mut archive = unrar::Archive::new(path.as_std_path()).open_for_processing()?;
    let mut rom_files = Vec::new();

    while let Some(header) = archive.read_header()? {
        let entry_path = safe_rar_entry_path(header.entry())?;
        archive = if let Some(filename) = entry_path {
            let (data, rest) = header.read()?;
            let sha1 = crate::hashes::sha1_bytes(&data);
            let xxh3 = crate::hashes::xxhash3_bytes(&data);
            if let Some(nrf) = NewRomFile::from_archive(path, &filename, sha1, xxh3) {
                rom_files.push(nrf);
            }
            rest
        } else {
            header.skip()?
        };
    }

    Ok(rom_files)
}

fn safe_rar_entry_path(header: &unrar::FileHeader) -> crate::Result<Option<PathBuf>> {
    if !header.is_file() {
        return Ok(None);
    }

    let name = header.filename.to_str().ok_or_else(|| {
        Error::InvalidPath(format!(
            "RAR entry name is not UTF-8: {}",
            header.filename.display()
        ))
    })?;
    if name.contains('\\') || name.contains('\0') {
        return Err(Error::InvalidPath(format!("unsafe RAR entry name: {name}")));
    }

    let path = Path::new(name);
    let components = path.components().collect::<Vec<_>>();
    if components.is_empty()
        || !components
            .iter()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(Error::InvalidPath(format!("unsafe RAR entry name: {name}")));
    }

    Ok(Some(path.to_owned()))
}

fn scan_7z(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    let archive = r7z::Archive::open(path.as_std_path())?;
    let mut rom_files = Vec::new();

    archive.stream_files(|entry, reader| {
        let filename = entry
            .safe_path()
            .ok_or_else(|| r7z::R7zError::UnsafePath(entry.name.clone()))?;
        let mut hash_writer = RomHashWriter::default();
        std::io::copy(reader, &mut hash_writer)?;
        let (sha1, xxh3) = hash_writer.finish();
        if let Some(nrf) = NewRomFile::from_archive(path, filename, sha1, xxh3) {
            rom_files.push(nrf);
        }
        Ok(())
    })?;

    Ok(rom_files)
}

fn hash_reader_chunks<R: Read>(mut reader: R) -> crate::Result<(Sha1Digest, Xxh3Digest)> {
    let mut sha1hasher = Sha1::new();
    let mut xxhash3 = Xxh3::new();
    let mut scratch = vec![0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut scratch)?;
        if read == 0 {
            break;
        }
        let chunk = &scratch[..read];
        sha1hasher.update(chunk);
        xxhash3.update(chunk);
    }

    Ok((sha1hasher.finalize().into(), xxhash3.digest().to_be_bytes()))
}

#[derive(Default)]
struct RomHashWriter {
    sha1: Sha1,
    xxhash3: Xxh3,
}

impl RomHashWriter {
    fn finish(self) -> (Sha1Digest, Xxh3Digest) {
        (
            self.sha1.finalize().into(),
            self.xxhash3.digest().to_be_bytes(),
        )
    }
}

impl Write for RomHashWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sha1.update(buf);
        self.xxhash3.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn walk_for_files(dir: &Utf8Path, excluded_paths: &[Utf8PathBuf]) -> Vec<Utf8PathBuf> {
    let v = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .flatten()
        .filter(|entry| !entry.file_type().is_dir())
        .collect();
    let optimized = optimize_file_order(v);
    optimized
        .into_iter()
        .filter_map(|direntry| Utf8PathBuf::from_path_buf(direntry.into_path()).ok())
        .filter(|path| {
            !excluded_paths
                .iter()
                .any(|excluded_path| path == excluded_path)
        })
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
    use std::collections::BTreeSet;
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

    fn write_version_rar(path: &Utf8Path) -> Result<(), Box<dyn std::error::Error>> {
        let archive = hex::decode(
            "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
        )?;
        std::fs::write(path, archive)?;
        Ok(())
    }

    #[test]
    fn walk_for_files_skips_hidden_entries_below_root() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        std::fs::write(root.join("visible.rom"), b"visible")?;
        std::fs::write(root.join(".root-hidden.rom"), b"root hidden is allowed")?;
        std::fs::create_dir(root.join(".hidden-dir"))?;
        std::fs::write(root.join(".hidden-dir").join("inside.rom"), b"hidden dir")?;
        std::fs::create_dir(root.join("visible-dir"))?;
        std::fs::write(root.join("visible-dir").join(".hidden.rom"), b"hidden file")?;
        std::fs::write(root.join("visible-dir").join("nested.rom"), b"nested")?;

        let files = walk_for_files(root, &[])
            .into_iter()
            .map(|path| path.strip_prefix(root).map(Utf8Path::to_owned))
            .collect::<Result<BTreeSet<_>, _>>()?;

        assert_eq!(
            files,
            BTreeSet::from([
                Utf8PathBuf::from("visible-dir/nested.rom"),
                Utf8PathBuf::from("visible.rom"),
            ])
        );
        Ok(())
    }

    #[test]
    fn scan_bare_file_reports_expected_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let path = root.join("game.rom");
        std::fs::write(&path, b"rom")?;

        let rom_files = scan_path(&path)?;

        assert_eq!(rom_files.len(), 1);
        assert_eq!(rom_files[0].name, "game.rom");
        assert_eq!(rom_files[0].parent_path, root.as_str());
        assert_eq!(rom_files[0].path, path.as_str());
        assert_eq!(rom_files[0].sha1, crate::hashes::sha1_bytes(b"rom"));
        assert_eq!(rom_files[0].xxhash3, crate::hashes::xxhash3_bytes(b"rom"));
        assert!(!rom_files[0].in_archive);
        Ok(())
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

    #[test]
    fn scan_zip_skips_directory_entries() -> Result<(), Box<dyn std::error::Error>> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        zip.add_directory("nested/", options)?;
        zip.start_file("nested/game.rom", options)?;
        zip.write_all(b"rom")?;
        let zip_data = zip.finish()?.into_inner();

        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &zip_data)?;

        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let mmap = crate::hashes::mmap_path(utf8_path)?;
        let rom_files = scan_zip(&mmap)?;

        assert_eq!(rom_files.len(), 1);
        assert_eq!(rom_files[0].name, "nested/game.rom");
        Ok(())
    }

    #[test]
    fn scan_zip_rejects_unsafe_enclosed_names() -> Result<(), Box<dyn std::error::Error>> {
        let zip_data = make_test_zip(&[("../evil.rom", b"evil" as &[u8])])?;
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), &zip_data)?;
        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let mmap = crate::hashes::mmap_path(utf8_path)?;

        let Err(error) = scan_zip(&mmap) else {
            return Err("expected unsafe zip entry to fail".into());
        };

        assert!(error.to_string().contains("invalid name inside zip"));
        Ok(())
    }

    #[test]
    fn rayon_scan_has_deterministic_content_across_job_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        std::fs::write(root.join("a.rom"), b"a")?;
        std::fs::write(root.join("b.rom"), b"b")?;
        std::fs::write(root.join("c.rom"), b"c")?;
        let files = walk_for_files(root, &[]);

        let normalize = |mut rom_files: Vec<NewRomFile>| {
            rom_files.sort_by(|left, right| left.name.cmp(&right.name));
            rom_files
                .into_iter()
                .map(|rom_file| (rom_file.name, rom_file.sha1, rom_file.xxhash3))
                .collect::<Vec<_>>()
        };

        let jobs_zero = normalize(get_all_rom_files(&files, 0)?);
        let jobs_one = normalize(get_all_rom_files(&files, 1)?);
        let jobs_two = normalize(get_all_rom_files(&files, 2)?);

        assert_eq!(jobs_zero, jobs_one);
        assert_eq!(jobs_one, jobs_two);
        Ok(())
    }

    #[test]
    fn scan_path_reports_corrupt_zip() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), b"PK\x03\x04not a valid zip")?;
        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;

        let Err(error) = scan_path(utf8_path) else {
            return Err("expected corrupt zip scan to fail".into());
        };

        assert!(error.to_string().contains("Zip error"));
        Ok(())
    }

    #[test]
    fn scan_path_reports_corrupt_7z_file() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), b"7z\xBC\xAF\x27\x1Cnot a valid 7z")?;
        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;

        let Err(error) = scan_path(utf8_path) else {
            return Err("expected corrupt archive scan to fail".into());
        };

        assert!(error.to_string().contains("Archive error"));
        Ok(())
    }

    #[test]
    fn scan_path_reports_corrupt_rar_file() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::NamedTempFile::new()?;
        std::fs::write(tmp.path(), b"Rar!\x1A\x07\x00not a valid rar")?;
        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;

        let Err(error) = scan_path(utf8_path) else {
            return Err("expected corrupt RAR scan to fail".into());
        };

        assert!(error.to_string().contains("RAR error"));
        Ok(())
    }

    #[test]
    fn scan_rar_reads_file_entries() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::NamedTempFile::new()?;
        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        write_version_rar(utf8_path)?;

        let rom_files = scan_path(utf8_path)?;

        assert_eq!(rom_files.len(), 1);
        assert_eq!(rom_files[0].name, "VERSION");
        assert_eq!(rom_files[0].sha1, crate::hashes::sha1_bytes(b"unrar-0.4.0"));
        assert_eq!(
            rom_files[0].xxhash3,
            crate::hashes::xxhash3_bytes(b"unrar-0.4.0")
        );
        assert!(rom_files[0].in_archive);
        Ok(())
    }

    #[test]
    fn scan_7z_skips_directory_entries() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::NamedTempFile::new()?;
        let archive_data = r7z::ArchiveBuilder::new()
            .add_directory("nested", r7z::EntryMeta::default())
            .add_file("nested/game.rom", b"rom")
            .build()?;
        std::fs::write(tmp.path(), archive_data)?;

        let utf8_path = camino::Utf8Path::from_path(tmp.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let rom_files = scan_7z(utf8_path)?;

        assert_eq!(rom_files.len(), 1);
        assert_eq!(rom_files[0].name, "nested/game.rom");
        assert_eq!(rom_files[0].sha1, crate::hashes::sha1_bytes(b"rom"));
        assert_eq!(rom_files[0].xxhash3, crate::hashes::xxhash3_bytes(b"rom"));
        Ok(())
    }
}
