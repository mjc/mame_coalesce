use std::{ffi::OsStr, io::Write, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
#[cfg(test)]
use fmmap::{MmapFile, MmapFileExt};

use indicatif::ParallelProgressIterator;
use log::info;

use rayon::prelude::*;
use sha1::{Digest, Sha1};

use walkdir::{DirEntry, WalkDir};
use xxhash_rust::xxh3::Xxh3;

use crate::{
    Error,
    domain::{
        ArchiveMemberSelector, CompleteSourceScan, EvidenceProvenance, EvidenceScope,
        ObservedContent, ScanProvenance, ScanRunKey, SourceFingerprint, SourceLocation,
        SourceObservation, SourceRoot,
    },
    hashes::{Sha1Digest, Xxh3Digest},
    progress,
};

pub fn source(
    path: &Utf8Path,
    jobs: usize,
    excluded_paths: &[Utf8PathBuf],
) -> crate::Result<CompleteSourceScan> {
    let source_root = path.canonicalize_utf8()?;
    info!("Looking in path: {source_root}");
    let file_list = walk_for_files(&source_root, excluded_paths)?;
    let source_root = SourceRoot::new(source_root.to_string());
    let scan_run = ScanRunKey::fresh();
    let observations = get_all_observations(&file_list, jobs, &source_root, scan_run)?;

    info!(
        "rom files found (unpacked and packed both): {}",
        observations.len()
    );
    CompleteSourceScan::new(source_root, scan_run, observations)
}

fn get_all_observations(
    file_list: &[Utf8PathBuf],
    jobs: usize,
    source_root: &SourceRoot,
    scan_run: ScanRunKey,
) -> crate::Result<Vec<SourceObservation>> {
    let bar = progress::bar(file_list.len() as u64);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs).build()?;
    pool.install(|| {
        file_list
            .par_iter()
            .progress_with(bar)
            .try_fold(Vec::new, |mut observations, path| {
                observations.extend(scan_one_path(path, source_root, scan_run)?);
                Ok(observations)
            })
            .try_reduce(Vec::new, |mut left, mut right| {
                left.append(&mut right);
                Ok(left)
            })
    })
}

fn scan_one_path(
    path: &Utf8Path,
    source_root: &SourceRoot,
    scan_run: ScanRunKey,
) -> crate::Result<Vec<SourceObservation>> {
    match crate::sources::detect(path)? {
        crate::sources::SourceKind::BareFile => scan_bare_file(path, source_root, scan_run),
        crate::sources::SourceKind::Archive(backend) => {
            scan_archive(path, backend, source_root, scan_run)
        }
    }
}

#[cfg(test)]
fn scan_path(path: &Utf8Path) -> crate::Result<Vec<SourceObservation>> {
    let source_root = SourceRoot::new(
        path.parent()
            .ok_or_else(|| Error::InvalidPath(format!("source path has no parent: {path}")))?
            .as_str()
            .to_owned(),
    );
    scan_one_path(path, &source_root, ScanRunKey::fresh())
}

fn scan_bare_file(
    path: &Utf8Path,
    source_root: &SourceRoot,
    scan_run: ScanRunKey,
) -> crate::Result<Vec<SourceObservation>> {
    let mut hash_writer = RomHashWriter::default();
    crate::sources::stream_file(path, &mut hash_writer)?;
    let (size, sha1, xxhash3) = hash_writer.finish();
    Ok(vec![SourceObservation {
        source_root: source_root.clone(),
        scan_run,
        location: SourceLocation::BareFile {
            path: path.to_string(),
        },
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size: Some(size),
            crc: None,
            md5: None,
            sha1: Some(sha1),
            xxh3: xxhash3,
        },
        fingerprint: SourceFingerprint::new(sha1),
        scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
    }])
}

#[cfg(test)]
fn scan_zip(mmap: &MmapFile) -> crate::Result<Vec<SourceObservation>> {
    let path = Utf8Path::from_path(mmap.path())
        .ok_or_else(|| Error::InvalidPath("invalid path".to_owned()))?;
    scan_path(path)
}

#[cfg(test)]
fn scan_7z(path: &Utf8Path) -> crate::Result<Vec<SourceObservation>> {
    scan_path(path)
}

fn scan_archive(
    path: &Utf8Path,
    backend: crate::domain::ArchiveBackend,
    source_root: &SourceRoot,
    scan_run: ScanRunKey,
) -> crate::Result<Vec<SourceObservation>> {
    let fingerprint = fingerprint_file(path)?;
    let mut observations = Vec::new();
    crate::sources::stream_archive(path, backend, None, |member, reader| {
        let mut hash_writer = RomHashWriter::default();
        std::io::copy(reader, &mut hash_writer)?;
        let (size, sha1, xxhash3) = hash_writer.finish();
        observations.push(SourceObservation {
            source_root: source_root.clone(),
            scan_run,
            location: SourceLocation::ArchiveMember {
                path: path.to_string(),
                backend,
                selector: ArchiveMemberSelector::IndexAndName {
                    index: u64::try_from(member.selector.index).map_err(|_| {
                        Error::InvalidPath("archive member index exceeds u64".to_owned())
                    })?,
                    name: member.selector.name.clone(),
                },
            },
            observed: ObservedContent {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::Computed,
                size: Some(size),
                crc: None,
                md5: None,
                sha1: Some(sha1),
                xxh3: xxhash3,
            },
            fingerprint,
            scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
        });
        Ok(())
    })?;
    verify_archive_fingerprint(path, fingerprint)?;
    Ok(observations)
}

fn fingerprint_file(path: &Utf8Path) -> crate::Result<SourceFingerprint> {
    let mut hasher = Sha1::new();
    crate::sources::stream_file(path, &mut hasher)?;
    Ok(SourceFingerprint::new(hasher.finalize().into()))
}

fn verify_archive_fingerprint(path: &Utf8Path, expected: SourceFingerprint) -> crate::Result<()> {
    if fingerprint_file(path)? != expected {
        return Err(Error::InvalidPath(format!(
            "archive changed while it was being scanned: {path}"
        )));
    }
    Ok(())
}

#[derive(Default)]
struct RomHashWriter {
    sha1: Sha1,
    xxhash3: Xxh3,
    size: u64,
}

impl RomHashWriter {
    fn finish(self) -> (u64, Sha1Digest, Xxh3Digest) {
        (
            self.size,
            self.sha1.finalize().into(),
            self.xxhash3.digest().to_be_bytes(),
        )
    }
}

impl Write for RomHashWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.size = self
            .size
            .checked_add(u64::try_from(buf.len()).map_err(std::io::Error::other)?)
            .ok_or_else(|| std::io::Error::other("observed source size overflowed"))?;
        self.sha1.update(buf);
        self.xxhash3.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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

    #[test]
    fn source_walk_excludes_database_and_sidecar_paths() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let database_path = root.join("cache.sqlite");
        let pool = crate::storage::db::create_db_pool(database_path.as_str())?;
        let rom_path = root.join("game.rom");
        std::fs::write(&rom_path, b"rom")?;
        let excluded_paths = crate::storage::db::database_file_paths(&pool)?;

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

        let observations = scan_path(&path)?;

        assert_eq!(observations.len(), 1);
        assert!(matches!(
            observations[0].location,
            SourceLocation::BareFile { .. }
        ));
        assert_eq!(observations[0].location.path(), path.as_str());
        assert_eq!(observations[0].source_root.as_str(), root.as_str());
        assert_eq!(
            observations[0].observed.sha1,
            Some(crate::hashes::sha1_bytes(b"rom"))
        );
        assert_eq!(observations[0].observed.size, Some(3));
        assert_eq!(
            observations[0].observed.xxh3,
            crate::hashes::xxhash3_bytes(b"rom")
        );
        assert_eq!(
            observations[0].fingerprint.digest(),
            crate::hashes::sha1_bytes(b"rom")
        );
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
        assert!(matches!(
            scanned[0].location,
            SourceLocation::ArchiveMember {
                backend: crate::domain::ArchiveBackend::Zip,
                selector: ArchiveMemberSelector::IndexAndName { index: 0, .. },
                ..
            }
        ));
        assert_eq!(scanned[0].location.member_name(), Some("game.rom"));
        assert_eq!(
            scanned[0].fingerprint.digest(),
            crate::hashes::sha1_bytes(&std::fs::read(&path)?)
        );
        Ok(())
    }

    #[test]
    fn archive_fingerprint_check_rejects_a_replaced_archive()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temp path is not UTF-8"))?;
        let path = root.join("archive.zip");
        std::fs::write(&path, make_test_zip(&[("game.rom", b"old")])?)?;
        let scanned_fingerprint = fingerprint_file(&path)?;

        std::fs::write(&path, make_test_zip(&[("game.rom", b"new")])?)?;

        let Err(error) = verify_archive_fingerprint(&path, scanned_fingerprint) else {
            return Err("expected archive replacement to invalidate the scan".into());
        };
        assert!(
            error
                .to_string()
                .contains("changed while it was being scanned")
        );
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
        assert_eq!(rom_files[0].observed.sha1, Some(expected_sha1));
        assert_eq!(rom_files[0].observed.xxh3, expected_xxh);
        assert_eq!(rom_files[0].observed.size, Some(content.len() as u64));
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
        assert!(matches!(
            rom_files[0].location,
            SourceLocation::ArchiveMember {
                backend: crate::domain::ArchiveBackend::Zip,
                selector: ArchiveMemberSelector::IndexAndName { index: 0, .. },
                ..
            }
        ));
        assert!(matches!(
            rom_files[1].location,
            SourceLocation::ArchiveMember {
                backend: crate::domain::ArchiveBackend::Zip,
                selector: ArchiveMemberSelector::IndexAndName { index: 1, .. },
                ..
            }
        ));
        // Verify hashes differ between entries
        assert_ne!(rom_files[0].observed.sha1, rom_files[1].observed.sha1);
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
        assert_eq!(rom_files[0].location.member_name(), Some("nested/game.rom"));
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

        let source_root = SourceRoot::new(root.as_str());
        let normalize = |mut observations: Vec<SourceObservation>| {
            observations.sort_by(|left, right| {
                left.location
                    .member_name()
                    .cmp(&right.location.member_name())
            });
            observations
                .into_iter()
                .map(|observation| {
                    (
                        observation.location.member_name().map(str::to_owned),
                        observation.observed.sha1,
                        observation.observed.xxh3,
                        observation.observed.size,
                    )
                })
                .collect::<Vec<_>>()
        };

        let jobs_zero = normalize(get_all_observations(
            &files,
            0,
            &source_root,
            ScanRunKey::fresh(),
        )?);
        let jobs_one = normalize(get_all_observations(
            &files,
            1,
            &source_root,
            ScanRunKey::fresh(),
        )?);
        let jobs_two = normalize(get_all_observations(
            &files,
            2,
            &source_root,
            ScanRunKey::fresh(),
        )?);

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
        assert_eq!(rom_files[0].location.member_name(), Some("VERSION"));
        assert_eq!(
            rom_files[0].observed.sha1,
            Some(crate::hashes::sha1_bytes(b"unrar-0.4.0"))
        );
        assert_eq!(
            rom_files[0].observed.xxh3,
            crate::hashes::xxhash3_bytes(b"unrar-0.4.0")
        );
        assert!(matches!(
            rom_files[0].location,
            SourceLocation::ArchiveMember { .. }
        ));
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
        assert_eq!(rom_files[0].location.member_name(), Some("nested/game.rom"));
        assert_eq!(
            rom_files[0].observed.sha1,
            Some(crate::hashes::sha1_bytes(b"rom"))
        );
        assert_eq!(
            rom_files[0].observed.xxh3,
            crate::hashes::xxhash3_bytes(b"rom")
        );
        Ok(())
    }
}
