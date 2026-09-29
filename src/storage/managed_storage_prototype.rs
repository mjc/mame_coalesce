//! Test-only spike for evaluating immutable file-backed payload storage.
//! This is deliberately not wired into application storage.

use std::{
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

fn object_key(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn object_path(root: &Path, key: &str) -> PathBuf {
    root.join(key)
}

fn publish(root: &Path, source: impl Read) -> io::Result<String> {
    publish_checked(root, None, source)
}

fn publish_checked(
    root: &Path,
    expected_key: Option<&str>,
    mut source: impl Read,
) -> io::Result<String> {
    let mut staging = NamedTempFile::new_in(root)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        staging.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
    }
    staging.as_file().sync_all()?;
    let key = hex::encode(hasher.finalize());
    if expected_key.is_some_and(|expected| expected != key) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source digest does not match expected object key",
        ));
    }
    let destination = object_path(root, &key);
    match staging.persist_noclobber(&destination) {
        Ok(_) => Ok(key),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            verify_object(&destination, &key)?;
            Ok(key)
        }
        Err(error) => Err(error.error),
    }
}

fn restore(root: &Path, key: &str, destination: &Path) -> io::Result<()> {
    let source = File::open(object_path(root, key))?;
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "restore path has no parent"))?;
    let mut staging = NamedTempFile::new_in(parent)?;
    let mut hasher = Sha256::new();
    let mut reader = io::BufReader::new(source);
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        staging.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
    }
    if hex::encode(hasher.finalize()) != key {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "managed object digest mismatch",
        ));
    }
    staging.as_file().sync_all()?;
    staging.persist(destination).map_err(|error| error.error)?;
    Ok(())
}

fn verify_object(path: &Path, expected_key: &str) -> io::Result<()> {
    let bytes = fs::read(path)?;
    if object_key(&bytes) == expected_key {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "existing managed object digest mismatch",
        ))
    }
}

fn zip_with_comment(comment: &str, payload: &[u8]) -> io::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let mut archive = ZipWriter::new(cursor);
    archive
        .set_comment(comment)
        .map_err(|error| io::Error::other(error.to_string()))?;
    archive
        .start_file("game.rom", SimpleFileOptions::default())
        .map_err(|error| io::Error::other(error.to_string()))?;
    archive.write_all(payload)?;
    archive
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| io::Error::other(error.to_string()))
}

#[test]
fn distinct_container_metadata_survives_while_equal_member_payload_deduplicates() -> io::Result<()>
{
    let directory = tempfile::tempdir()?;
    let payload = b"same synthetic ROM bytes";
    let first = zip_with_comment("acquired from source A", payload)?;
    let second = zip_with_comment("acquired from source B", payload)?;
    assert_ne!(first, second);

    let first_archive_key = publish(directory.path(), Cursor::new(&first))?;
    let second_archive_key = publish(directory.path(), Cursor::new(&second))?;
    let first_member_key = publish(directory.path(), Cursor::new(zip_member(&first)?))?;
    let second_member_key = publish(directory.path(), Cursor::new(zip_member(&second)?))?;

    assert_ne!(first_archive_key, second_archive_key);
    assert_eq!(first_member_key, second_member_key);
    assert_eq!(
        fs::read(object_path(directory.path(), &first_archive_key))?,
        first
    );
    assert_eq!(
        fs::read(object_path(directory.path(), &second_archive_key))?,
        second
    );
    assert_eq!(
        fs::read_dir(directory.path())?.count(),
        3,
        "two original containers and one shared member payload remain"
    );
    eprintln!(
        "fixture bytes: archives={}+{}, shared_member={}, managed_total={}",
        first.len(),
        second.len(),
        payload.len(),
        first.len() + second.len() + payload.len()
    );
    Ok(())
}

fn zip_member(archive: &[u8]) -> io::Result<Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(archive))
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut member = archive
        .by_name("game.rom")
        .map_err(|error| io::Error::other(error.to_string()))?;
    let mut bytes = Vec::new();
    member.read_to_end(&mut bytes)?;
    Ok(bytes)
}

struct FailsAfter {
    cursor: Cursor<Vec<u8>>,
    remaining: usize,
}

impl Read for FailsAfter {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("injected source read failure"));
        }
        let limit = buffer.len().min(self.remaining);
        let read = self.cursor.read(&mut buffer[..limit])?;
        self.remaining -= read;
        Ok(read)
    }
}

#[test]
fn interrupted_copy_and_digest_mismatch_never_publish_partial_objects() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let source = b"source remains unchanged after interrupted managed copy";
    let interrupted = FailsAfter {
        cursor: Cursor::new(source.to_vec()),
        remaining: 7,
    };
    assert!(publish(directory.path(), interrupted).is_err());
    assert_eq!(fs::read_dir(directory.path())?.count(), 0);

    let wrong_key = object_key(b"different bytes");
    assert!(publish_checked(directory.path(), Some(&wrong_key), Cursor::new(source)).is_err());
    assert_eq!(fs::read_dir(directory.path())?.count(), 0);
    Ok(())
}

#[test]
fn restore_verifies_before_replacing_and_preserves_the_source_object() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let source = b"immutable source bytes";
    let key = publish(directory.path(), Cursor::new(source))?;
    let restored = directory.path().join("restored.bin");
    fs::write(&restored, b"previous destination")?;
    restore(directory.path(), &key, &restored)?;
    assert_eq!(fs::read(&restored)?, source);
    assert_eq!(fs::read(object_path(directory.path(), &key))?, source);

    fs::write(object_path(directory.path(), &key), b"corrupted object")?;
    fs::write(&restored, b"previous destination")?;
    assert!(restore(directory.path(), &key, &restored).is_err());
    assert_eq!(fs::read(&restored)?, b"previous destination");
    Ok(())
}
