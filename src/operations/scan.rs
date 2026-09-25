use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(test)]
use fmmap::{MmapFile, MmapFileExt};

use indicatif::ParallelProgressIterator;
use log::{info, warn};

use rayon::prelude::*;
use sha1::{Digest, Sha1};

use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::Xxh3;

use crate::{
    Error,
    hashes::{Sha1Digest, Xxh3Digest},
    progress,
    storage::{
        db::{self, Pool},
        models::NewRomFile,
    },
};

pub fn source(path: &Utf8Path, jobs: usize, pool: &Pool) -> crate::Result<Utf8PathBuf> {
    let source_root = path.canonicalize_utf8()?;
    info!("Looking in path: {source_root}");
    let excluded_paths = db::database_file_paths(pool)?;
    let file_list = walk_for_files(&source_root, &excluded_paths)?;
    let new_rom_files = get_all_rom_files(&file_list, jobs)?;

    info!(
        "rom files found (unpacked and packed both): {}",
        new_rom_files.len()
    );
    let associated_roms =
        db::replace_rom_files_for_source_root(pool, &source_root, &new_rom_files)?;
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
    match crate::sources::detect(path)? {
        crate::sources::SourceKind::BareFile => scan_bare_file(path),
        crate::sources::SourceKind::Archive(backend) => scan_archive(path, backend),
    }
}

fn scan_bare_file(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    let mut hash_writer = RomHashWriter::default();
    crate::sources::stream_file(path, &mut hash_writer)?;
    let (sha1, xxhash3) = hash_writer.finish();
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidPath(format!("source path has no filename: {path}")))?;
    let parent_path = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(format!("source path has no parent: {path}")))?;
    Ok(vec![NewRomFile {
        parent_path: parent_path.to_string(),
        path: path.to_string(),
        name: name.to_owned(),
        sha1,
        xxhash3,
        in_archive: false,
        archive_backend: None,
        archive_member_index: None,
        rom_id: None,
    }])
}

#[cfg(test)]
fn scan_zip(mmap: &MmapFile) -> crate::Result<Vec<NewRomFile>> {
    let path = Utf8Path::from_path(mmap.path())
        .ok_or_else(|| Error::InvalidPath("invalid path".to_owned()))?;
    scan_archive(path, crate::domain::ArchiveBackend::Zip)
}

#[cfg(test)]
fn scan_7z(path: &Utf8Path) -> crate::Result<Vec<NewRomFile>> {
    scan_archive(path, crate::domain::ArchiveBackend::SevenZip)
}

fn scan_archive(
    path: &Utf8Path,
    backend: crate::domain::ArchiveBackend,
) -> crate::Result<Vec<NewRomFile>> {
    let mut rom_files = Vec::new();
    crate::sources::stream_archive(path, backend, None, |member, reader| {
        let mut hash_writer = RomHashWriter::default();
        std::io::copy(reader, &mut hash_writer)?;
        let (sha1, xxhash3) = hash_writer.finish();
        let name = Path::new(&member.selector.name);
        rom_files.push(archive_rom_file(
            path,
            name,
            sha1,
            xxhash3,
            backend,
            member.selector.index as u64,
        )?);
        Ok(())
    })?;
    Ok(rom_files)
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

fn archive_rom_file(
    archive_path: &Utf8Path,
    member_path: &Path,
    sha1: Sha1Digest,
    xxhash3: Xxh3Digest,
    backend: crate::domain::ArchiveBackend,
    index: u64,
) -> std::io::Result<NewRomFile> {
    NewRomFile::from_archive(archive_path, member_path, sha1, xxhash3, backend, index).ok_or_else(
        || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "couldn't represent archive member {} in {archive_path}",
                    member_path.display()
                ),
            )
        },
    )
}

fn walk_for_files(
    dir: &Utf8Path,
    excluded_paths: &[Utf8PathBuf],
) -> crate::Result<Vec<Utf8PathBuf>> {
    let files = WalkDir::new(dir)
        .into_iter()
        .filter_entry(entry_is_relevant)
        .try_fold(Vec::new(), |mut files, entry| {
            let entry = entry.map_err(|error| {
                Error::InvalidPath(format!("failed to traverse source path: {error}"))
            })?;
            if !entry.file_type().is_dir() {
                files.push(entry);
            }
            Ok::<_, crate::Error>(files)
        })?;
    let paths = optimize_file_order(files)
        .into_iter()
        .map(|entry| source_path_from_path_buf(entry.into_path()))
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(paths
        .into_iter()
        .filter(|path| {
            !excluded_paths
                .iter()
                .any(|excluded_path| path == excluded_path)
        })
        .collect())
}

fn source_path_from_path_buf(path: PathBuf) -> crate::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| Error::InvalidPath(format!("source path is not UTF-8: {}", path.display())))
}

fn entry_is_relevant(entry: &DirEntry) -> bool {
    entry_name_is_relevant(entry.file_name(), entry.depth())
}

fn entry_name_is_relevant(name: &OsStr, depth: usize) -> bool {
    // Hidden entries below the root are intentionally skipped; invalid UTF-8 names proceed to
    // path conversion so they fail the scan instead of appearing to have disappeared.
    name.to_str()
        .is_none_or(|name| depth == 0 || !name.starts_with('.'))
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

        let files = walk_for_files(root, &[])?
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
    fn walk_for_files_propagates_traversal_errors() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let missing_root = root.join("removed-during-scan");
        let Err(error) = walk_for_files(&missing_root, &[]) else {
            return Err("expected a traversal error for the missing root".into());
        };

        assert!(error.to_string().contains("failed to traverse source path"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_entry_names_are_not_skipped_as_hidden() {
        use std::os::unix::ffi::OsStrExt;

        assert!(entry_name_is_relevant(OsStr::from_bytes(b"x\xff"), 1));
        assert!(!entry_name_is_relevant(OsStr::new(".hidden"), 1));
    }

    #[cfg(unix)]
    #[test]
    fn source_paths_reject_non_utf8_paths() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(std::ffi::OsString::from_vec(b"bad-\xff".to_vec()));
        let Err(error) = source_path_from_path_buf(path) else {
            return Err("expected non-UTF-8 path to fail conversion".into());
        };

        assert!(error.to_string().contains("source path is not UTF-8"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn archive_member_conversion_errors_are_propagated() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::ffi::OsStrExt;

        let member = Path::new(OsStr::from_bytes(b"bad-\xff"));
        let Err(error) = archive_rom_file(
            Utf8Path::new("/source/archive.zip"),
            member,
            Sha1Digest::default(),
            Xxh3Digest::default(),
            crate::domain::ArchiveBackend::Rar,
            0,
        ) else {
            return Err("expected non-UTF-8 archive member to fail conversion".into());
        };

        assert!(
            error
                .to_string()
                .contains("couldn't represent archive member")
        );
        Ok(())
    }

    #[test]
    fn source_walk_excludes_database_and_sidecar_paths() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let database_path = root.join("cache.sqlite");
        let pool = db::create_db_pool(database_path.as_str())?;
        let rom_path = root.join("game.rom");
        std::fs::write(&rom_path, b"rom")?;
        let excluded_paths = db::database_file_paths(&pool)?;

        assert!(excluded_paths.contains(&database_path.canonicalize_utf8()?));
        assert!(excluded_paths.contains(&Utf8PathBuf::from(format!("{database_path}-wal"))));
        assert!(excluded_paths.contains(&Utf8PathBuf::from(format!("{database_path}-shm"))));
        assert_eq!(walk_for_files(root, &excluded_paths)?, vec![rom_path]);
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
    fn scan_path_detects_and_persists_renamed_zip_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let path = root.join("archive.data");
        std::fs::write(&path, make_test_zip(&[("game.rom", b"rom")])?)?;

        let scanned = scan_path(&path)?;
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].archive_backend.as_deref(), Some("zip"));
        assert_eq!(scanned[0].archive_member_index, Some(0));
        assert_eq!(scanned[0].name, "game.rom");
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
        assert_eq!(rom_files[0].archive_backend.as_deref(), Some("zip"));
        assert_eq!(rom_files[0].archive_member_index, Some(0));
        assert_eq!(rom_files[1].archive_member_index, Some(1));
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

        assert!(error.to_string().contains("unsafe archive member name"));
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
        let files = walk_for_files(root, &[])?;

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

        assert!(error.to_string().contains("ZIP"), "{error}");
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
