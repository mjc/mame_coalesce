use std::{
    fs::{self, File},
    io::{Read, copy},
    path::{Path, PathBuf},
};

use atomic_write_file::AtomicWriteFile;
use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    Connection, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Integer, Nullable, Text},
};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{Error, Result, storage::db::validate_database_schema};

const BACKUP_APPLICATION_ID: i32 = 0x4d43_4231;
const BACKUP_FORMAT_VERSION: i32 = 1;
const MAX_REPORTED_ISSUES: usize = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestorePolicy {
    CreateNew,
    ReplaceExisting,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    Published,
    PublishedDurabilityUnconfirmed { error: String },
    PublicationStateUncertain { error: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupOutcome {
    Published,
    PublishedDurabilityUnconfirmed { error: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IntegrityReport {
    /// Corruption in the SQLite file, retained documents, or durable catalog history.
    pub durable_issues: Vec<String>,
    /// Problems in rebuildable scanned-file inventory rows.
    pub inventory_issues: Vec<String>,
}

impl IntegrityReport {
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.durable_issues.is_empty() && self.inventory_issues.is_empty()
    }
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct ForeignKeyViolation {
    #[diesel(sql_type = Text)]
    table: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    rowid: Option<i64>,
    #[diesel(sql_type = Text)]
    parent: String,
    #[diesel(sql_type = Integer)]
    fkid: i32,
}

#[derive(QueryableByName)]
struct RetainedDocumentRow {
    #[diesel(sql_type = BigInt)]
    rowid: i64,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    object_key: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    byte_length: Option<i64>,
}

#[derive(QueryableByName)]
struct RetainedObjectRow {
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    byte_length: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    object_key: Option<String>,
}

#[derive(QueryableByName)]
struct PragmaValue {
    #[diesel(sql_type = Integer)]
    value: i32,
}

#[derive(QueryableByName)]
struct IntegrityCheckRow {
    #[diesel(sql_type = Text)]
    integrity_check: String,
}

/// Create and validate a standalone, versioned SQLite snapshot without modifying the source.
/// The destination must not already exist.
pub fn create_backup(source: &Utf8Path, destination: &Utf8Path) -> Result<BackupOutcome> {
    if source == destination {
        return Err(backup_error("source and destination paths are identical"));
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(backup_error(format!(
            "backup destination already exists: {destination}"
        )));
    }
    reject_sqlite_sidecars(destination)?;
    let destination_objects = document_sidecar_path(destination);
    if fs::symlink_metadata(&destination_objects).is_ok() {
        return Err(backup_error(format!(
            "backup document sidecar already exists: {destination_objects}"
        )));
    }
    let (canonical_source, _cache_lock) =
        crate::database::lock_cache_file(source, crate::database::CacheLockMode::Shared)?;
    check_document_sidecar(&canonical_source)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."));
    let staged_objects = stage_document_sidecar(&canonical_source, parent)?;
    let stage = NamedTempFile::new_in(parent)?;
    let stage_path = stage
        .path()
        .to_str()
        .ok_or_else(|| backup_error("temporary backup path is not valid UTF-8"))?;

    let mut conn = connect(&canonical_source)?;
    sql_query("VACUUM INTO ?")
        .bind::<Text, _>(stage_path)
        .execute(&mut conn)
        .map_err(|error| backup_error(format!("SQLite snapshot failed: {error}")))?;
    drop(conn);

    let mut staged = connect_path(stage.path())?;
    sql_query(format!("PRAGMA application_id = {BACKUP_APPLICATION_ID}"))
        .execute(&mut staged)
        .map_err(|error| backup_error(format!("could not mark backup format: {error}")))?;
    sql_query(format!("PRAGMA user_version = {BACKUP_FORMAT_VERSION}"))
        .execute(&mut staged)
        .map_err(|error| backup_error(format!("could not mark backup version: {error}")))?;
    let report = check_connection(&mut staged, true)?;
    if !report.durable_issues.is_empty() {
        return Err(backup_error(format!(
            "source snapshot failed integrity checks: {}",
            report_summary(&report)
        )));
    }
    drop(staged);
    stage.as_file().sync_all()?;
    if let Some(staged_objects) = staged_objects.as_ref() {
        fs::rename(staged_objects, &destination_objects)?;
    }
    if let Err(error) = stage
        .persist_noclobber(destination)
        .map_err(|error| backup_error(format!("could not publish backup: {}", error.error)))
    {
        if staged_objects.is_some() {
            let _ = fs::remove_dir_all(&destination_objects);
        }
        return Err(error);
    }
    Ok(match sync_parent_directory(destination) {
        Ok(true) => BackupOutcome::Published,
        Ok(false) => BackupOutcome::PublishedDurabilityUnconfirmed {
            error: "parent-directory syncing is unavailable on this platform".to_owned(),
        },
        Err(error) => BackupOutcome::PublishedDurabilityUnconfirmed {
            error: error.to_string(),
        },
    })
}

/// Check a cache or backup in place. This never runs migrations or writes to the inspected file.
pub fn check_integrity(path: &Utf8Path) -> Result<IntegrityReport> {
    if !path.is_file() {
        return Err(backup_error(format!(
            "database file does not exist: {path}"
        )));
    }
    let (canonical_path, _cache_lock) =
        crate::database::lock_cache_file(path, crate::database::CacheLockMode::Exclusive)?;
    reject_sqlite_sidecars(&canonical_path)?;
    let mut conn = connect(&canonical_path)?;
    sql_query("PRAGMA query_only = ON")
        .execute(&mut conn)
        .map_err(|error| {
            backup_error(format!(
                "could not enable read-only integrity checks: {error}"
            ))
        })?;
    let mut report = check_connection(&mut conn, false)?;
    drop(conn);
    if report.durable_issues.is_empty() {
        check_sidecar_contents(&canonical_path, &mut report)?;
    }
    Ok(report)
}

/// Validate a backup in a same-directory staging copy, then atomically replace the cache.
/// An existing cache is only replaced with `RestorePolicy::ReplaceExisting`.
#[allow(clippy::too_many_lines)] // The restore protocol keeps the database and object directory paired.
pub fn restore_backup(
    backup: &Utf8Path,
    destination: &Utf8Path,
    policy: RestorePolicy,
) -> Result<RestoreOutcome> {
    if fs::symlink_metadata(destination).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(backup_error("refusing to replace a symlink cache path"));
    }
    let (canonical_destination, _cache_lock) =
        crate::database::lock_cache_file(destination, crate::database::CacheLockMode::Exclusive)?;
    let backup =
        Utf8PathBuf::from_path_buf(backup.as_std_path().canonicalize()?).map_err(|path| {
            backup_error(format!(
                "backup path is not valid UTF-8: {}",
                path.display()
            ))
        })?;
    if backup == canonical_destination {
        return Err(backup_error("backup and restore paths are identical"));
    }
    crate::database::reject_multiple_hard_links(&backup)?;
    let (canonical_backup, _backup_lock) =
        crate::database::lock_cache_file(&backup, crate::database::CacheLockMode::Exclusive)?;
    reject_sqlite_sidecars(&canonical_backup)?;
    check_document_sidecar(&canonical_backup)?;
    let destination = canonical_destination.as_path();
    let destination_exists = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(backup_error("refusing to replace a symlink cache path"));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(backup_error("cache destination is not a regular file"));
        }
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    match (destination_exists, policy) {
        (true, RestorePolicy::CreateNew) => {
            return Err(backup_error(
                "cache already exists; explicit replace-existing policy is required",
            ));
        }
        (false, RestorePolicy::ReplaceExisting) => {
            return Err(backup_error(
                "replace-existing was requested but the cache does not exist",
            ));
        }
        _ => {}
    }
    reject_sqlite_sidecars(destination)?;

    let parent = destination
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."));
    let stage = validated_restore_stage(&backup, parent)?;
    let staged_objects = stage_document_sidecar(&canonical_backup, parent)?;
    let destination_objects = document_sidecar_path(destination);

    match policy {
        RestorePolicy::CreateNew => {
            if let Some(staged_objects) = staged_objects.as_ref() {
                fs::rename(staged_objects, &destination_objects)?;
            }
            if let Err(error) = stage.persist_noclobber(destination).map_err(|error| {
                backup_error(format!("could not publish restored cache: {}", error.error))
            }) {
                if staged_objects.is_some() {
                    let _ = fs::remove_dir_all(&destination_objects);
                }
                return Err(error);
            }
            Ok(match sync_parent_directory(destination) {
                Ok(true) => RestoreOutcome::Published,
                Ok(false) => RestoreOutcome::PublishedDurabilityUnconfirmed {
                    error: "parent-directory syncing is unavailable on this platform".to_owned(),
                },
                Err(error) => RestoreOutcome::PublishedDurabilityUnconfirmed {
                    error: error.to_string(),
                },
            })
        }
        RestorePolicy::ReplaceExisting => {
            let mut replacement = AtomicWriteFile::open(destination)?;
            copy(&mut File::open(stage.path())?, &mut replacement)?;
            let previous_objects =
                replace_sidecar(staged_objects.as_deref(), &destination_objects)?;
            let outcome = match replacement.commit() {
                Ok(()) => Ok(match sync_parent_directory(destination) {
                    Ok(true) => RestoreOutcome::Published,
                    Ok(false) => RestoreOutcome::PublishedDurabilityUnconfirmed {
                        error: "parent-directory syncing is unavailable on this platform"
                            .to_owned(),
                    },
                    Err(error) => RestoreOutcome::PublishedDurabilityUnconfirmed {
                        error: error.to_string(),
                    },
                }),
                Err(error) => match files_equal(destination, stage.path()) {
                    Ok(true) => Ok(RestoreOutcome::PublishedDurabilityUnconfirmed {
                        error: error.to_string(),
                    }),
                    Ok(false) => Err(error.into()),
                    Err(inspect_error) => Ok(RestoreOutcome::PublicationStateUncertain {
                        error: format!("{error}; could not verify publication: {inspect_error}"),
                    }),
                },
            };
            match outcome {
                Ok(outcome) => {
                    if let Some(previous_objects) = previous_objects {
                        let _ = fs::remove_dir_all(previous_objects);
                    }
                    Ok(outcome)
                }
                Err(error) => {
                    if let Some(previous_objects) = previous_objects {
                        if destination_objects.exists() {
                            let _ = fs::remove_dir_all(&destination_objects);
                        }
                        let _ = fs::rename(previous_objects, &destination_objects);
                    }
                    Err(error)
                }
            }
        }
    }
}

fn validated_restore_stage(backup: &Utf8Path, parent: &Utf8Path) -> Result<NamedTempFile> {
    let snapshot_directory = tempfile::tempdir_in(parent)?;
    let snapshot_path = snapshot_directory.path().join("restore-snapshot.sqlite");
    let snapshot_path_text = snapshot_path
        .to_str()
        .ok_or_else(|| backup_error("temporary restore path is not valid UTF-8"))?;
    let mut source = connect(backup)?;
    sql_query("VACUUM INTO ?")
        .bind::<Text, _>(snapshot_path_text)
        .execute(&mut source)
        .map_err(|error| backup_error(format!("could not snapshot backup database: {error}")))?;
    drop(source);

    let mut stage = NamedTempFile::new_in(parent)?;
    copy(&mut File::open(&snapshot_path)?, stage.as_file_mut())?;
    let mut staged = connect_path(stage.path())?;
    validate_backup_header(&mut staged)?;
    let report = check_connection(&mut staged, true)?;
    if !report.durable_issues.is_empty() {
        return Err(backup_error(format!(
            "backup failed integrity checks: {}",
            report_summary(&report)
        )));
    }
    sql_query("PRAGMA application_id = 0")
        .execute(&mut staged)
        .map_err(|error| backup_error(format!("could not prepare restored cache: {error}")))?;
    sql_query("PRAGMA user_version = 0")
        .execute(&mut staged)
        .map_err(|error| backup_error(format!("could not prepare restored cache: {error}")))?;
    drop(staged);
    stage.as_file().sync_all()?;
    Ok(stage)
}

fn files_equal(left: &Utf8Path, right: &std::path::Path) -> Result<bool> {
    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let mut left_buffer = [0_u8; 16 * 1024];
    let mut right_buffer = [0_u8; 16 * 1024];
    loop {
        let left_len = fill_buffer(&mut left, &mut left_buffer)?;
        let right_len = fill_buffer(&mut right, &mut right_buffer)?;
        if left_len != right_len {
            return Ok(false);
        }
        if left_buffer[..left_len] != right_buffer[..left_len] {
            return Ok(false);
        }
        if left_len == 0 {
            return Ok(true);
        }
    }
}

fn fill_buffer(reader: &mut impl Read, buffer: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let bytes_read = reader.read(&mut buffer[filled..])?;
        if bytes_read == 0 {
            break;
        }
        filled += bytes_read;
    }
    Ok(filled)
}

fn connect(path: &Utf8Path) -> Result<SqliteConnection> {
    if !path.is_file() {
        return Err(backup_error(format!(
            "database file does not exist: {path}"
        )));
    }
    connect_path(path.as_std_path())
}

fn connect_path(path: &std::path::Path) -> Result<SqliteConnection> {
    let path = path
        .to_str()
        .ok_or_else(|| backup_error("database path is not valid UTF-8"))?;
    SqliteConnection::establish(path)
        .map_err(|error| backup_error(format!("could not open SQLite database: {error}")))
}

fn validate_backup_header(conn: &mut SqliteConnection) -> Result<()> {
    let application_id = sql_query("SELECT application_id AS value FROM pragma_application_id")
        .get_result::<PragmaValue>(conn)
        .map_err(|error| backup_error(format!("could not read backup marker: {error}")))?
        .value;
    let version = sql_query("SELECT user_version AS value FROM pragma_user_version")
        .get_result::<PragmaValue>(conn)
        .map_err(|error| backup_error(format!("could not read backup version: {error}")))?
        .value;
    if application_id != BACKUP_APPLICATION_ID || version != BACKUP_FORMAT_VERSION {
        return Err(backup_error(format!(
            "unsupported backup format (application_id={application_id}, version={version})"
        )));
    }
    Ok(())
}

fn check_connection(
    conn: &mut SqliteConnection,
    require_backup_header: bool,
) -> Result<IntegrityReport> {
    let mut report = IntegrityReport::default();
    if require_backup_header {
        validate_backup_header(conn)?;
    }

    // Cap the report, not the check: inventory corruption must not hide later durable failures.
    let check_rows = sql_query("PRAGMA integrity_check(2147483647)")
        .load_iter::<IntegrityCheckRow, _>(conn)
        .map_err(|error| backup_error(format!("SQLite integrity check failed: {error}")))?;
    for row in check_rows {
        let row =
            row.map_err(|error| backup_error(format!("SQLite integrity check failed: {error}")))?;
        if row.integrity_check != "ok" {
            let table = row
                .integrity_check
                .strip_prefix("CHECK constraint failed in ");
            if table.is_some_and(is_rebuildable_table) {
                push_issue(&mut report.inventory_issues, row.integrity_check);
            } else {
                push_issue(&mut report.durable_issues, row.integrity_check);
            }
        }
    }

    let violations = sql_query("SELECT * FROM pragma_foreign_key_check")
        .load_iter::<ForeignKeyViolation, _>(conn)
        .map_err(|error| backup_error(format!("SQLite foreign-key check failed: {error}")))?;
    for violation in violations {
        let violation = violation
            .map_err(|error| backup_error(format!("SQLite foreign-key check failed: {error}")))?;
        let issue = format!(
            "{} row {:?} references missing {} (foreign key {})",
            violation.table, violation.rowid, violation.parent, violation.fkid
        );
        if is_rebuildable_table(&violation.table) {
            push_issue(&mut report.inventory_issues, issue);
        } else {
            push_issue(&mut report.durable_issues, issue);
        }
    }

    if !check_schema(conn, &mut report) {
        return Ok(report);
    }
    check_retained_documents(conn, &mut report)?;
    check_publication_states(conn, &mut report)?;
    check_logiqx_attribute_positions(conn, &mut report)?;
    check_software_attribute_positions(conn, &mut report)?;
    Ok(report)
}

fn check_software_attribute_positions(
    conn: &mut SqliteConnection,
    report: &mut IntegrityReport,
) -> Result<()> {
    let rows=sql_query("SELECT 'Software attributes: ' || reason || ' (' || owner_kind || ':' || owner_a || ',' || owner_b || ',' || owner_c || ', field ' || field_kind || ')' AS integrity_check FROM software_attribute_violations")
        .load_iter::<IntegrityCheckRow,_>(conn)
        .map_err(|error|backup_error(format!("software attribute integrity check failed: {error}")))?;
    for row in rows {
        let row = row.map_err(|error| {
            backup_error(format!(
                "software attribute integrity check failed: {error}"
            ))
        })?;
        push_issue(&mut report.durable_issues, row.integrity_check);
    }
    Ok(())
}

fn check_logiqx_attribute_positions(
    conn: &mut SqliteConnection,
    report: &mut IntegrityReport,
) -> Result<()> {
    // Global corruption checks belong here, not in per-edition publication.
    let rows = sql_query("SELECT 'Logiqx attributes: ' || reason || ' (' || owner_kind || ':' || CAST(owner_a AS TEXT) || ',' || owner_b || ', field ' || field_kind || ')' AS integrity_check FROM logiqx_attribute_violations")
        .load_iter::<IntegrityCheckRow, _>(conn)
        .map_err(|error| backup_error(format!("Logiqx attribute integrity check failed: {error}")))?;
    for row in rows {
        let row = row.map_err(|error| {
            backup_error(format!("Logiqx attribute integrity check failed: {error}"))
        })?;
        push_issue(&mut report.durable_issues, row.integrity_check);
    }
    Ok(())
}

fn check_schema(conn: &mut SqliteConnection, report: &mut IntegrityReport) -> bool {
    if let Err(error) = validate_database_schema(conn) {
        push_issue(
            &mut report.durable_issues,
            format!("database schema validation failed: {error}"),
        );
        return false;
    }
    true
}

fn check_retained_documents(
    conn: &mut SqliteConnection,
    report: &mut IntegrityReport,
) -> Result<()> {
    let mut last_rowid = i64::MIN;
    let mut inclusive = true;
    loop {
        let Some(document) = sql_query(if inclusive {
            "SELECT rowid, document_key, object_key, sha1, sha256, byte_length FROM documents \
                 WHERE retention_status = 'retained' AND rowid >= ? ORDER BY rowid LIMIT 1"
        } else {
            "SELECT rowid, document_key, object_key, sha1, sha256, byte_length FROM documents \
                 WHERE retention_status = 'retained' AND rowid > ? ORDER BY rowid LIMIT 1"
        })
        .bind::<BigInt, _>(last_rowid)
        .get_result::<RetainedDocumentRow>(conn)
        .optional()
        .map_err(|error| backup_error(format!("could not inspect retained documents: {error}")))?
        else {
            break;
        };
        last_rowid = document.rowid;
        inclusive = false;
        if document.object_key.is_none()
            || document.sha1.is_none()
            || document.sha256.as_deref().is_none_or(|digest| {
                format!("sha256:{}", hex::encode(digest)) != document.document_key
            })
            || document.byte_length.is_none_or(|length| length < 0)
        {
            push_issue(
                &mut report.durable_issues,
                format!(
                    "retained document {} has invalid object metadata",
                    document.document_key
                ),
            );
        }
        if report.durable_issues.len() >= MAX_REPORTED_ISSUES {
            break;
        }
    }
    Ok(())
}

fn check_document_sidecar(database: &Utf8Path) -> Result<()> {
    let mut report = IntegrityReport::default();
    check_sidecar_contents(database, &mut report)?;
    if report.durable_issues.is_empty() {
        Ok(())
    } else {
        Err(backup_error(report_summary(&report)))
    }
}

fn check_sidecar_contents(database: &Utf8Path, report: &mut IntegrityReport) -> Result<()> {
    let mut conn = connect(database)?;
    let documents = sql_query(
        "SELECT document_key, sha1, sha256, byte_length, object_key \
         FROM documents WHERE retention_status = 'retained' ORDER BY document_key",
    )
    .load::<RetainedObjectRow>(&mut conn)
    .map_err(|error| backup_error(format!("could not inspect document objects: {error}")))?;
    drop(conn);
    let root = document_sidecar_path(database);
    for document in documents {
        let valid_key = document.object_key.as_deref().is_some_and(|key| {
            key.starts_with("sha256/")
                && !key
                    .split('/')
                    .any(|component| component == ".." || component.is_empty())
        });
        let Some(key) = document.object_key.filter(|_| valid_key) else {
            push_issue(
                &mut report.durable_issues,
                format!(
                    "retained document {} has an invalid object key",
                    document.document_key
                ),
            );
            continue;
        };
        let (Some(expected_sha1), Some(expected_sha256), Some(expected_length)) =
            (document.sha1, document.sha256, document.byte_length)
        else {
            push_issue(
                &mut report.durable_issues,
                format!(
                    "retained document {} is missing digest metadata",
                    document.document_key
                ),
            );
            continue;
        };
        let path = root.join(key);
        let actual = (|| -> std::io::Result<(Vec<u8>, Vec<u8>, u64)> {
            let mut decoder = zstd::Decoder::new(File::open(path)?)?;
            let mut sha256 = Sha256::new();
            let mut sha1 = sha1::Sha1::new();
            let mut length = 0_u64;
            let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
            loop {
                let read = decoder.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                length = length.saturating_add(read as u64);
                sha256.update(&buffer[..read]);
                sha1.update(&buffer[..read]);
            }
            Ok((sha256.finalize().to_vec(), sha1.finalize().to_vec(), length))
        })();
        let matches = actual.is_ok_and(|(sha256, sha1, length)| {
            sha256 == expected_sha256
                && sha1 == expected_sha1
                && length == u64::try_from(expected_length).unwrap_or(u64::MAX)
                && format!("sha256:{}", hex::encode(sha256)) == document.document_key
        });
        if !matches {
            push_issue(
                &mut report.durable_issues,
                format!(
                    "retained document {} has a missing or invalid sidecar object",
                    document.document_key
                ),
            );
        }
        if report.durable_issues.len() >= MAX_REPORTED_ISSUES {
            break;
        }
    }
    Ok(())
}

fn check_publication_states(
    conn: &mut SqliteConnection,
    report: &mut IntegrityReport,
) -> Result<()> {
    let rows = sql_query(
        "SELECT run_key AS value FROM import_runs AS run \
         WHERE status IN ('pending', 'running') \
            OR (status = 'succeeded' AND snapshot_key IS NULL) \
            OR (status <> 'succeeded' AND snapshot_key IS NOT NULL) \
            OR (status = 'succeeded' AND NOT EXISTS ( \
                SELECT 1 FROM snapshot_publications AS publication \
                WHERE publication.snapshot_key = run.snapshot_key \
                  AND publication.catalog_key = run.catalog_key \
                  AND publication.document_key = run.document_key \
                  AND publication.interpretation_key = run.interpretation_key)) \
         ORDER BY run_key LIMIT 50",
    )
    .load::<TextValue>(conn)
    .map_err(|error| backup_error(format!("could not inspect publication states: {error}")))?;
    for row in rows {
        push_issue(
            &mut report.durable_issues,
            format!(
                "import run {} has an incomplete publication state",
                row.value
            ),
        );
    }
    let relationships = sql_query(
        "SELECT assertion_key AS value FROM catalog_relationship_closure \
         WHERE NOT is_complete OR NOT is_published \
         ORDER BY assertion_key LIMIT 50",
    )
    .load::<TextValue>(conn)
    .map_err(|error| {
        backup_error(format!(
            "could not inspect relationship publication states: {error}"
        ))
    })?;
    for row in relationships {
        push_issue(
            &mut report.durable_issues,
            format!(
                "relationship {} has an incomplete evidence publication state",
                row.value
            ),
        );
    }
    let reviews = sql_query(
        "SELECT review_key AS value FROM catalog_relationship_review_closure \
         WHERE NOT is_complete OR NOT is_published ORDER BY review_id LIMIT 50",
    )
    .load::<TextValue>(conn)
    .map_err(|error| {
        backup_error(format!(
            "could not inspect relationship review publication states: {error}"
        ))
    })?;
    for row in reviews {
        push_issue(
            &mut report.durable_issues,
            format!(
                "relationship review {} has an incomplete publication state",
                row.value
            ),
        );
    }
    Ok(())
}

fn reject_sqlite_sidecars(destination: &Utf8Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = format!("{}{suffix}", destination.as_str());
        match fs::symlink_metadata(&sidecar) {
            Ok(_) => {
                return Err(backup_error(format!(
                    "refusing operation while SQLite sidecar exists: {sidecar}"
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn document_sidecar_path(database: &Utf8Path) -> Utf8PathBuf {
    Utf8PathBuf::from(format!("{database}.documents"))
}

fn stage_document_sidecar(database: &Utf8Path, parent: &Utf8Path) -> Result<Option<PathBuf>> {
    let source = document_sidecar_path(database);
    match fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(backup_error("document sidecar is not a regular directory")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let stage = tempfile::tempdir_in(parent)?;
    copy_directory_contents(source.as_std_path(), stage.path())?;
    Ok(Some(stage.keep()))
}

fn replace_sidecar(staged: Option<&Path>, destination: &Utf8Path) -> Result<Option<PathBuf>> {
    let previous = if destination.exists() {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        let backup = parent.join(format!(
            ".{}.{}.documents-old",
            destination.file_name().unwrap_or("cache"),
            uuid::Uuid::new_v4()
        ));
        fs::rename(destination, &backup)?;
        Some(backup.into_std_path_buf())
    } else {
        None
    };
    if let Some(staged) = staged
        && let Err(error) = fs::rename(staged, destination)
    {
        if let Some(previous) = previous.as_ref() {
            let _ = fs::rename(previous, destination);
        }
        return Err(error.into());
    }
    Ok(previous)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(backup_error("document sidecar contains a symbolic link"));
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target)?;
            copy_directory_contents(&entry.path(), &target)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(backup_error("document sidecar contains a special file"));
        }
    }
    Ok(())
}

fn sync_parent_directory(path: &Utf8Path) -> Result<bool> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        File::open(parent)?.sync_all()?;
        Ok(true)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(false)
    }
}

fn is_rebuildable_table(table: &str) -> bool {
    table == "rom_files"
}

fn push_issue(issues: &mut Vec<String>, issue: String) {
    if issues.len() < MAX_REPORTED_ISSUES {
        issues.push(issue);
    }
}

fn report_summary(report: &IntegrityReport) -> String {
    format!(
        "{} durable issue(s), {} rebuildable inventory issue(s)",
        report.durable_issues.len(),
        report.inventory_issues.len()
    )
}

fn backup_error(message: impl Into<String>) -> Error {
    Error::CacheBackup(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{self, CatalogDocumentFormat, CatalogImportRequest};
    use crate::domain::{
        CatalogKey, CatalogScope, ContentDigestAlgorithm, ContentIdentity, PublishingSourceKey,
        RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipOrigin,
        RelationshipReview, RelationshipReviewDecision, RelationshipType,
    };
    use diesel::sql_types::Binary;
    use tempfile::tempdir;

    fn utf8(path: std::path::PathBuf) -> std::io::Result<camino::Utf8PathBuf> {
        camino::Utf8PathBuf::from_path_buf(path)
            .map_err(|path| std::io::Error::other(path.display().to_string()))
    }

    fn database(path: &Utf8Path) -> crate::Result<crate::database::Database> {
        crate::database::Database::open(&path.to_owned())
    }

    fn add_retained_document(path: &Utf8Path, payload: &[u8]) -> Result<()> {
        let store = crate::storage::documents::DocumentStore::open(path.as_str())?;
        let source = crate::domain::PublishingSource::new("source", "Source");
        store.register_source(&source)?;
        store.retain_unvalidated(
            &crate::storage::documents::AcquisitionMetadata {
                source_key: source.key().clone(),
                source_uri: None,
                method: Some("test".to_owned()),
                transport_headers: Vec::new(),
                expected_sha256: None,
            },
            payload,
        )?;
        Ok(())
    }

    fn load_document(path: &Utf8Path, payload: &[u8]) -> Result<Vec<u8>> {
        let store = crate::storage::documents::DocumentStore::open(path.as_str())?;
        store.load(&crate::domain::DocumentKey::from_bytes(payload))
    }

    fn insert_corrupt_rom_file(conn: &mut SqliteConnection, id: i64) -> Result<()> {
        let path = format!("/source/corrupt-{id}.rom");
        sql_query(
            "INSERT INTO rom_files \
             (parent_path, path, name, sha1, xxhash3, in_archive, scan_root, scan_run, \
              observed_size, source_fingerprint, scan_provenance) \
             VALUES ('/source', ?, ?, zeroblob(20), zeroblob(8), 0, '/source', ?, 0, \
                     x'01', 'streamed_sha1_xxh3_v1')",
        )
        .bind::<Text, _>(&path)
        .bind::<Text, _>(format!("corrupt-{id}.rom"))
        .bind::<Text, _>(format!("run-{id}"))
        .execute(conn)?;
        Ok(())
    }

    #[test]
    fn backup_restore_round_trips_retained_documents_and_provenance() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let restored = utf8(directory.path().join("restored.sqlite"))?;
        let payload = b"original catalog document bytes";
        add_retained_document(&source, payload)?;

        create_backup(&source, &backup)?;
        restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;

        assert_eq!(load_document(&restored, payload)?, payload);
        let mut conn = connect(&restored)?;
        let acquisition_count =
            sql_query("SELECT COUNT(*) AS count FROM acquisitions WHERE source_key = 'source'")
                .get_result::<CountRow>(&mut conn)?
                .count;
        assert_eq!(acquisition_count, 1);
        assert!(check_integrity(&restored)?.is_clean());
        Ok(())
    }

    #[test]
    #[allow(clippy::too_many_lines)] // One full round-trip fixture proves the durable catalog graph is retained.
    fn backup_restore_preserves_catalog_snapshots_assertions_and_adjudications() -> Result<()> {
        let directory = tempdir()?;
        let source_path = utf8(directory.path().join("catalogs.sqlite"))?;
        let backup_path = utf8(directory.path().join("catalogs.backup"))?;
        let restored_path = utf8(directory.path().join("catalogs-restored.sqlite"))?;
        let database = database(&source_path)?;

        let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let machine_bytes = std::fs::read(fixture_root.join("fixtures/catalog/mame/machine.xml"))?;
        let machine_request = CatalogImportRequest {
            document_path: utf8(fixture_root.join("fixtures/catalog/mame/machine.xml"))?,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("machine-source"),
            source_display_name: "Machine source".to_owned(),
            catalog_key: CatalogKey::new("machine-catalog"),
            catalog_display_name: "Machine catalog".to_owned(),
            scope: CatalogScope::Unknown,
        };
        let machine_snapshot = app::import_catalog(&database, &machine_request)?
            .snapshot_key
            .ok_or_else(|| crate::Error::InvalidPath("MAME snapshot missing".to_owned()))?;

        let logiqx_bytes =
            std::fs::read(fixture_root.join("fixtures/catalog/logiqx/catalog-a-v1.dat"))?;
        let logiqx_request = CatalogImportRequest {
            document_path: utf8(fixture_root.join("fixtures/catalog/logiqx/catalog-a-v1.dat"))?,
            format: CatalogDocumentFormat::Logiqx(crate::logiqx::LogiqxMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("logiqx-source"),
            source_display_name: "Logiqx source".to_owned(),
            catalog_key: CatalogKey::new("logiqx-catalog"),
            catalog_display_name: "Logiqx catalog".to_owned(),
            scope: CatalogScope::Unknown,
        };
        app::import_catalog(&database, &logiqx_request)?;

        let mut conn = connect(&source_path)?;
        let assertion = sql_query(
            "SELECT assertion_key AS value FROM relationship_assertion_explanations \
             WHERE source_field = 'device_ref' \
               AND source_snapshot_key=(SELECT snapshot_key FROM catalog_snapshots \
                                        WHERE catalog_key='logiqx-catalog')",
        )
        .get_result::<TextValue>(&mut conn)?;
        drop(conn);
        let supporting_assertion = RelationshipAssertionKey::new(assertion.value);
        let identity = ContentIdentity::new(
            ContentDigestAlgorithm::Sha1,
            "0123456789abcdef0123456789abcdef01234567",
        )?;
        let candidate = app::record_relationship(
            &database,
            &RelationshipClaim {
                relation_type: RelationshipType::ExactContentIdentity,
                subject: RelationshipEndpoint::ContentObject(identity.clone()),
                target: RelationshipEndpoint::ContentObject(identity),
                origin: RelationshipOrigin::DerivedCandidate {
                    rule: crate::domain::RelationshipRule::new(
                        "backup-round-trip",
                        "v1",
                        "Backup round-trip witness",
                    )?,
                    supporting_assertions: vec![supporting_assertion],
                },
                evidence: crate::domain::RelationshipEvidence::Rationale {
                    reason: "preserve adjudication".to_owned(),
                },
            },
        )?;
        app::review_relationship(
            &database,
            &candidate,
            &RelationshipReview {
                decision: RelationshipReviewDecision::Accepted,
                note: "preserved decision".to_owned(),
                superseded_by: None,
            },
        )?;
        let original_explanations = app::explain_relationships(&database)?;
        drop(database);

        create_backup(&source_path, &backup_path)?;
        restore_backup(&backup_path, &restored_path, RestorePolicy::CreateNew)?;

        let mut restored = connect(&restored_path)?;
        for (table, expected) in [
            ("catalogs", 2),
            ("catalog_snapshots", 2),
            ("snapshot_publications", 2),
            ("relationship_assertions", 1),
            ("relationship_assertion_explanations", 3),
            ("catalog_relationships", 3),
            ("catalog_relationship_reviews", 1),
        ] {
            let count = sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
                .get_result::<CountRow>(&mut restored)?
                .count;
            assert!(
                count >= expected,
                "{table}: expected at least {expected}, got {count}"
            );
        }
        for bytes in [&machine_bytes, &logiqx_bytes] {
            assert_eq!(load_document(&restored_path, bytes)?, *bytes);
        }
        drop(restored);
        let restored_database = crate::database::Database::open(&restored_path)?;
        assert_eq!(
            app::explain_relationships(&restored_database)?,
            original_explanations,
            "paired backup preserves exact native keys, evidence, support and reviews"
        );
        let restored_source = app::load_snapshot_source(&restored_database, &machine_snapshot)?;
        assert_eq!(restored_source, machine_bytes);
        assert!(
            restored_source
                .windows(b"future:flag=\"preserved\"".len())
                .any(|window| window == b"future:flag=\"preserved\""),
            "restored source retains the vendor literal"
        );
        drop(restored_database);
        assert!(check_integrity(&restored_path)?.is_clean());
        let mut restored = connect(&restored_path)?;
        sql_query("DROP TRIGGER snapshot_publications_are_immutable_delete")
            .execute(&mut restored)?;
        sql_query(
            "DELETE FROM snapshot_publications WHERE snapshot_key = \
             (SELECT snapshot_key FROM import_runs WHERE status = 'succeeded' LIMIT 1)",
        )
        .execute(&mut restored)?;
        let mut report = IntegrityReport::default();
        check_publication_states(&mut restored, &mut report)?;
        drop(restored);
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("incomplete publication")),
            "{report:?}"
        );
        Ok(())
    }

    #[test]
    fn invalid_backup_never_replaces_the_existing_cache() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        create_backup(&source, &backup)?;
        {
            let mut conn = connect(&backup)?;
            sql_query("PRAGMA user_version = 99").execute(&mut conn)?;
        }
        add_retained_document(&destination, b"keep me")?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
        assert_eq!(load_document(&destination, b"keep me")?, b"keep me");
        Ok(())
    }

    #[test]
    fn truncated_backup_preserves_the_existing_cache() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        add_retained_document(&destination, b"keep me")?;
        create_backup(&source, &backup)?;
        File::options().write(true).open(&backup)?.set_len(128)?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
        assert_eq!(load_document(&destination, b"keep me")?, b"keep me");
        Ok(())
    }

    #[test]
    fn missing_durable_reference_preserves_the_existing_cache() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        add_retained_document(&destination, b"keep me")?;
        create_backup(&source, &backup)?;
        let mut conn = connect(&backup)?;
        sql_query("PRAGMA foreign_keys = OFF").execute(&mut conn)?;
        sql_query("DROP TRIGGER documents_are_immutable_delete").execute(&mut conn)?;
        sql_query("DELETE FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"backup").as_slice())
            .execute(&mut conn)?;
        drop(conn);

        assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
        assert_eq!(load_document(&destination, b"keep me")?, b"keep me");
        Ok(())
    }

    #[test]
    fn replacement_requires_an_explicit_policy() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        add_retained_document(&source, b"source")?;
        create_backup(&source, &backup)?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&destination, b"destination")?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::CreateNew).is_err());
        assert_eq!(load_document(&destination, b"destination")?, b"destination");
        Ok(())
    }

    #[test]
    fn integrity_reports_retained_document_digest_corruption() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        add_retained_document(&path, b"original")?;
        let object_dir = document_sidecar_path(&path);
        let mut objects = fs::read_dir(&object_dir)?;
        let shard = objects
            .next()
            .ok_or_else(|| backup_error("missing object shard"))??
            .path();
        let object = fs::read_dir(shard)?
            .next()
            .ok_or_else(|| backup_error("missing object"))??
            .path();
        fs::write(object, b"corrupt")?;

        let report = check_integrity(&path)?;
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("document")),
            "{report:?}"
        );
        Ok(())
    }

    #[test]
    fn integrity_separates_rebuildable_cache_corruption() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database_guard = database(&path)?;
        let mut conn = connect(&path)?;
        sql_query("PRAGMA ignore_check_constraints = ON").execute(&mut conn)?;
        insert_corrupt_rom_file(&mut conn, 1)?;

        drop(conn);
        drop(database_guard);
        let report = check_integrity(&path)?;
        assert!(
            report.durable_issues.is_empty(),
            "{:?}",
            report.durable_issues
        );
        assert!(
            report
                .inventory_issues
                .iter()
                .any(|issue| issue.contains("rom_files"))
        );
        assert!(!report.is_clean());
        Ok(())
    }

    #[test]
    fn backup_refuses_to_publish_beside_sqlite_sidecars() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        drop(database(&source)?);
        fs::write(format!("{backup}-wal"), b"stale WAL bytes")?;

        assert!(create_backup(&source, &backup).is_err());
        assert!(!backup.exists());
        Ok(())
    }

    #[test]
    fn integrity_checks_durable_native_payload_after_many_inventory_violations() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database = database(&path)?;

        let fixture_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let request = CatalogImportRequest {
            document_path: utf8(fixture_root.join("fixtures/catalog/mame/software-list.xml"))?,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("backup-integrity-software-source"),
            source_display_name: "Backup integrity software source".to_owned(),
            catalog_key: CatalogKey::new("backup-integrity-software-catalog"),
            catalog_display_name: "Backup integrity software catalog".to_owned(),
            scope: CatalogScope::Unknown,
        };
        app::import_catalog(&database, &request)?;

        let mut conn = connect(&path)?;
        sql_query("PRAGMA ignore_check_constraints = ON").execute(&mut conn)?;
        for id in 1..=60 {
            insert_corrupt_rom_file(&mut conn, id)?;
        }
        // Deliberately simulate damaged storage, not an authorized catalog edit.
        // Restore the exact guard before checking integrity so schema damage
        // cannot stand in for the native payload CHECK failure under test.
        let guard = sql_query("SELECT sql FROM sqlite_schema WHERE type = 'trigger' AND name = 'software_rom_entries_immutable_update'")
            .get_result::<TriggerDefinition>(&mut conn)?;
        sql_query("DROP TRIGGER software_rom_entries_immutable_update").execute(&mut conn)?;
        let affected = sql_query(
            "UPDATE software_rom_entries SET dump_status = 'invalid' \
             WHERE occurrence_id = (SELECT MIN(occurrence_id) FROM software_rom_entries)",
        )
        .execute(&mut conn)?;
        sql_query(&guard.sql).execute(&mut conn)?;
        assert_eq!(affected, 1, "fixture has a native software ROM payload");
        sql_query("INSERT INTO catalog_contents (content_uuid) VALUES (x'00')")
            .execute(&mut conn)?;
        drop(conn);
        drop(database);

        let report = check_integrity(&path)?;
        for table in ["software_rom_entries", "catalog_contents"] {
            assert!(
                report
                    .durable_issues
                    .contains(&format!("CHECK constraint failed in {table}")),
                "the durable {table} CHECK failure must survive inventory noise: {report:?}"
            );
        }
        assert_eq!(report.inventory_issues.len(), MAX_REPORTED_ISSUES);
        Ok(())
    }

    #[test]
    fn integrity_checks_durable_foreign_keys_after_many_inventory_violations() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database = database(&path)?;
        let mut conn = connect(&path)?;
        sql_query("PRAGMA ignore_check_constraints = ON").execute(&mut conn)?;
        sql_query("PRAGMA foreign_keys = OFF").execute(&mut conn)?;
        for id in 1..=60 {
            insert_corrupt_rom_file(&mut conn, id)?;
        }
        sql_query(
            "INSERT INTO acquisitions (acquisition_key, source_key, document_key, method) \
             VALUES ('orphan-acquisition', 'missing-source', 'missing-document', 'test')",
        )
        .execute(&mut conn)?;
        drop(conn);
        drop(database);

        let report = check_integrity(&path)?;
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("acquisitions")),
            "the durable foreign-key violation must survive inventory noise: {report:?}"
        );
        assert_eq!(report.inventory_issues.len(), MAX_REPORTED_ISSUES);
        Ok(())
    }

    #[test]
    fn restore_refuses_to_replace_a_cache_open_by_the_application() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        add_retained_document(&destination, b"keep me")?;
        create_backup(&source, &backup)?;
        let open_database = database(&destination)?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
        drop(open_database);
        assert_eq!(load_document(&destination, b"keep me")?, b"keep me");
        Ok(())
    }

    #[test]
    fn restore_refuses_to_replace_a_cache_open_by_document_store() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        add_retained_document(&destination, b"keep me")?;
        create_backup(&source, &backup)?;
        let open_store = crate::storage::documents::DocumentStore::open(destination.as_str())?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
        drop(open_store);
        assert_eq!(load_document(&destination, b"keep me")?, b"keep me");
        Ok(())
    }

    #[test]
    fn integrity_rejects_an_unrecognized_schema_without_writing_to_it() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("unrecognized.sqlite"))?;
        let mut conn = connect_path(path.as_std_path())?;
        sql_query("CREATE TABLE unrelated (value TEXT)").execute(&mut conn)?;
        drop(conn);

        let report = check_integrity(&path)?;
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("schema"))
        );
        let mut conn = connect(&path)?;
        let tables = sql_query("SELECT COUNT(*) AS count FROM sqlite_schema WHERE type = 'table'")
            .get_result::<CountRow>(&mut conn)?
            .count;
        assert_eq!(tables, 1);
        Ok(())
    }

    #[test]
    fn integrity_of_missing_database_does_not_create_parent_directories() -> Result<()> {
        let directory = tempdir()?;
        let parent = utf8(directory.path().join("not-created"))?;
        let path = parent.join("cache.sqlite");

        assert!(check_integrity(&path).is_err());
        assert!(!parent.exists());
        Ok(())
    }

    #[test]
    fn integrity_rejects_a_schema_that_disagrees_with_authoritative_ddl() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        add_retained_document(&source, b"schema")?;
        create_backup(&source, &backup)?;
        let mut conn = connect(&backup)?;
        sql_query("DROP TABLE snapshot_publications").execute(&mut conn)?;
        drop(conn);

        let report = check_integrity(&backup)?;
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("schema"))
        );
        Ok(())
    }

    #[test]
    fn restore_rejects_a_backup_with_wal_sidecars() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        create_backup(&source, &backup)?;
        fs::write(format!("{backup}-wal"), b"pending WAL bytes")?;

        assert!(restore_backup(&backup, &destination, RestorePolicy::CreateNew).is_err());
        assert!(!destination.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn restore_resolves_backup_symlinks_before_checking_sidecars() -> Result<()> {
        let directory = tempdir()?;
        let source = utf8(directory.path().join("source.sqlite"))?;
        let backup = utf8(directory.path().join("backup.sqlite"))?;
        let backup_alias = utf8(directory.path().join("backup-alias.sqlite"))?;
        let destination = utf8(directory.path().join("destination.sqlite"))?;
        add_retained_document(&source, b"backup")?;
        create_backup(&source, &backup)?;
        fs::write(format!("{backup}-wal"), b"pending WAL bytes")?;
        std::os::unix::fs::symlink(&backup, &backup_alias)?;

        assert!(restore_backup(&backup_alias, &destination, RestorePolicy::CreateNew).is_err());
        assert!(!destination.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn database_rejects_hard_linked_cache_paths() -> Result<()> {
        let directory = tempdir()?;
        let cache = utf8(directory.path().join("cache.sqlite"))?;
        let alias = utf8(directory.path().join("cache-alias.sqlite"))?;
        drop(database(&cache)?);
        fs::hard_link(&cache, &alias)?;

        assert!(database(&alias).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn database_rejects_dangling_cache_symlinks_without_creating_a_lock() -> Result<()> {
        let directory = tempdir()?;
        let target = utf8(directory.path().join("target.sqlite"))?;
        let alias = utf8(directory.path().join("cache-link.sqlite"))?;
        std::os::unix::fs::symlink(&target, &alias)?;

        assert!(database(&alias).is_err());
        assert!(!target.exists());
        assert!(!directory.path().join("target.sqlite.lock").exists());
        Ok(())
    }

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    #[derive(QueryableByName)]
    struct TriggerDefinition {
        #[diesel(sql_type = Text)]
        sql: String,
    }
}
