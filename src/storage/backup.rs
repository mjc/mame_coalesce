use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, copy},
};

use atomic_write_file::AtomicWriteFile;
use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    Connection, SqliteConnection,
    migration::MigrationSource,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Integer, Nullable, Text},
};
use diesel_migrations::MigrationHarness;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::{Error, Result, storage::db::MIGRATIONS};

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
    /// Problems in rebuildable DAT-cache or scan-inventory rows.
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
    #[diesel(sql_type = Nullable<Binary>)]
    payload: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    byte_length: Option<i64>,
}

#[derive(QueryableByName)]
struct LegacyDocumentRow {
    #[diesel(sql_type = BigInt)]
    rowid: i64,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Binary)]
    payload: Vec<u8>,
    #[diesel(sql_type = Binary)]
    sha1: Vec<u8>,
    #[diesel(sql_type = Binary)]
    sha256: Vec<u8>,
    #[diesel(sql_type = BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    parent_sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    parent_sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    parent_byte_length: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, QueryableByName)]
struct SchemaObject {
    #[diesel(sql_type = Text)]
    object_type: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    table_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    sql: Option<String>,
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
    let (canonical_source, _cache_lock) =
        crate::database::lock_cache_file(source, crate::database::CacheLockMode::Shared)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."));
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
    stage
        .persist_noclobber(destination)
        .map_err(|error| backup_error(format!("could not publish backup: {}", error.error)))?;
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
    check_connection(&mut conn, false)
}

/// Validate a backup in a same-directory staging copy, then atomically replace the cache.
/// An existing cache is only replaced with `RestorePolicy::ReplaceExisting`.
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

    match policy {
        RestorePolicy::CreateNew => {
            stage.persist_noclobber(destination).map_err(|error| {
                backup_error(format!("could not publish restored cache: {}", error.error))
            })?;
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
            match replacement.commit() {
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

    let check_rows = sql_query("PRAGMA integrity_check(50)")
        .load::<IntegrityCheckRow>(conn)
        .map_err(|error| backup_error(format!("SQLite integrity check failed: {error}")))?;
    for row in check_rows {
        if row.integrity_check != "ok" {
            push_issue(&mut report.durable_issues, row.integrity_check);
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

    if !check_migrations(conn, &mut report)? {
        return Ok(report);
    }
    check_retained_documents(conn, &mut report)?;
    check_legacy_documents(conn, &mut report)?;
    check_publication_states(conn, &mut report)?;
    Ok(report)
}

fn check_migrations(conn: &mut SqliteConnection, report: &mut IntegrityReport) -> Result<bool> {
    let expected = <_ as MigrationSource<diesel::sqlite::Sqlite>>::migrations(&MIGRATIONS)
        .map_err(|error| backup_error(format!("embedded migration list failed: {error}")))?
        .into_iter()
        .map(|migration| migration.name().version().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let Some(_migration_table) = sql_query(
        "SELECT name AS value FROM sqlite_schema \
         WHERE type = 'table' AND name = '__diesel_schema_migrations'",
    )
    .get_result::<TextValue>(conn)
    .optional()
    .map_err(|error| backup_error(format!("could not inspect migration schema: {error}")))?
    else {
        push_issue(
            &mut report.durable_issues,
            "database is missing its schema migration history".to_owned(),
        );
        return Ok(false);
    };
    let applied =
        sql_query("SELECT version AS value FROM __diesel_schema_migrations ORDER BY version")
            .load::<TextValue>(conn)
            .map_err(|error| backup_error(format!("could not read applied migrations: {error}")))?
            .into_iter()
            .map(|row| row.value)
            .collect::<std::collections::BTreeSet<_>>();
    if expected != applied {
        push_issue(
            &mut report.durable_issues,
            "applied database migrations do not match this program's schema".to_owned(),
        );
        return Ok(false);
    }
    let expected_schema = migrated_schema()?;
    let actual_schema = schema_objects(conn)?;
    if expected_schema != actual_schema {
        let missing = expected_schema
            .difference(&actual_schema)
            .take(5)
            .map(|object| format!("{}:{}", object.object_type, object.name))
            .collect::<Vec<_>>();
        let unexpected = actual_schema
            .difference(&expected_schema)
            .take(5)
            .map(|object| format!("{}:{}", object.object_type, object.name))
            .collect::<Vec<_>>();
        push_issue(
            &mut report.durable_issues,
            format!(
                "database schema does not match its applied migration history (missing: {missing:?}, unexpected: {unexpected:?})"
            ),
        );
        return Ok(false);
    }
    Ok(true)
}

fn migrated_schema() -> Result<BTreeSet<SchemaObject>> {
    let mut conn = SqliteConnection::establish(":memory:")
        .map_err(|error| backup_error(format!("could not create schema reference: {error}")))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|error| backup_error(format!("could not build schema reference: {error}")))?;
    schema_objects(&mut conn)
}

fn schema_objects(conn: &mut SqliteConnection) -> Result<BTreeSet<SchemaObject>> {
    let objects = sql_query(
        "SELECT type AS object_type, name, tbl_name AS table_name, sql \
         FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name, tbl_name",
    )
    .load::<SchemaObject>(conn)
    .map_err(|error| backup_error(format!("could not inspect database schema: {error}")))?;
    Ok(objects.into_iter().collect())
}

fn check_retained_documents(
    conn: &mut SqliteConnection,
    report: &mut IntegrityReport,
) -> Result<()> {
    let mut last_rowid = i64::MIN;
    let mut inclusive = true;
    loop {
        let Some(document) = sql_query(if inclusive {
            "SELECT rowid, document_key, payload, sha1, sha256, byte_length FROM documents \
                 WHERE retention_status = 'retained' AND rowid >= ? ORDER BY rowid LIMIT 1"
        } else {
            "SELECT rowid, document_key, payload, sha1, sha256, byte_length FROM documents \
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
        match (
            document.payload,
            document.sha1,
            document.sha256,
            document.byte_length,
        ) {
            (Some(payload), expected_sha1, Some(expected_digest), Some(byte_length))
                if usize::try_from(byte_length).ok() == Some(payload.len())
                    && Sha256::digest(&payload).as_slice() == expected_digest.as_slice()
                    && crate::domain::DocumentKey::from_bytes(&payload).to_string()
                        == document.document_key.as_str()
                    && expected_sha1.as_ref().is_none_or(|digest| {
                        sha1::Sha1::digest(&payload).as_slice() == digest.as_slice()
                    }) => {}
            _ => push_issue(
                &mut report.durable_issues,
                format!(
                    "retained document {} has invalid bytes or digest metadata",
                    document.document_key
                ),
            ),
        }
        if report.durable_issues.len() >= MAX_REPORTED_ISSUES {
            break;
        }
    }
    Ok(())
}

fn check_legacy_documents(conn: &mut SqliteConnection, report: &mut IntegrityReport) -> Result<()> {
    let mut last_rowid = i64::MIN;
    let mut inclusive = true;
    loop {
        let Some(document) = sql_query(if inclusive {
            "SELECT payloads.rowid, payloads.document_key, payloads.payload, payloads.sha1, \
                payloads.sha256, payloads.byte_length, documents.sha1 AS parent_sha1, \
                documents.sha256 AS parent_sha256, documents.byte_length AS parent_byte_length \
         FROM legacy_document_payloads AS payloads \
         LEFT JOIN documents ON documents.document_key = payloads.document_key \
         WHERE payloads.rowid >= ? ORDER BY payloads.rowid LIMIT 1"
        } else {
            "SELECT payloads.rowid, payloads.document_key, payloads.payload, payloads.sha1, \
                payloads.sha256, payloads.byte_length, documents.sha1 AS parent_sha1, \
                documents.sha256 AS parent_sha256, documents.byte_length AS parent_byte_length \
         FROM legacy_document_payloads AS payloads \
         LEFT JOIN documents ON documents.document_key = payloads.document_key \
         WHERE payloads.rowid > ? ORDER BY payloads.rowid LIMIT 1"
        })
        .bind::<BigInt, _>(last_rowid)
        .get_result::<LegacyDocumentRow>(conn)
        .optional()
        .map_err(|error| backup_error(format!("could not inspect legacy documents: {error}")))?
        else {
            break;
        };
        last_rowid = document.rowid;
        inclusive = false;
        if usize::try_from(document.byte_length).ok() != Some(document.payload.len())
            || Sha256::digest(&document.payload).as_slice() != document.sha256.as_slice()
            || sha1::Sha1::digest(&document.payload).as_slice() != document.sha1.as_slice()
            || crate::domain::DocumentKey::from_bytes(&document.payload).to_string()
                != document.document_key.as_str()
            || document.parent_sha1.as_deref() != Some(document.sha1.as_slice())
            || document.parent_byte_length != Some(document.byte_length)
            || document
                .parent_sha256
                .as_deref()
                .is_some_and(|digest| digest != document.sha256.as_slice())
        {
            push_issue(
                &mut report.durable_issues,
                format!(
                    "legacy retained document {} has invalid bytes or digest metadata",
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
    matches!(
        table,
        "data_files" | "games" | "roms" | "rom_files" | "archive_files"
    )
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
        let _database = database(path)?;
        let mut conn = connect(path)?;
        let document_key = crate::domain::DocumentKey::from_bytes(payload).to_string();
        sql_query(
            "INSERT INTO publishing_sources (source_key, display_name) VALUES ('source', 'Source')",
        )
        .execute(&mut conn)?;
        sql_query(
            "INSERT INTO documents (document_key, sha1, byte_length, sha256, payload, retention_status) \
             VALUES (?, ?, ?, ?, ?, 'retained')",
        )
        .bind::<Text, _>(&document_key)
        .bind::<Binary, _>(sha1::Sha1::digest(payload).as_slice())
            .bind::<BigInt, _>(i64::try_from(payload.len()).map_err(|error| backup_error(error.to_string()))?)
        .bind::<Binary, _>(Sha256::digest(payload).as_slice())
        .bind::<Binary, _>(payload)
        .execute(&mut conn)?;
        sql_query(
            "INSERT INTO acquisitions (acquisition_key, source_key, document_key, method) \
             VALUES ('acquisition', 'source', ?, 'test')",
        )
        .bind::<Text, _>(&document_key)
        .execute(&mut conn)?;
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

        let mut conn = connect(&restored)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(payload).as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, payload);
        let acquisition_count = sql_query(
            "SELECT COUNT(*) AS count FROM acquisitions WHERE acquisition_key = 'acquisition'",
        )
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
        app::import_catalog(&database, &machine_request)?;

        let logiqx_bytes =
            std::fs::read(fixture_root.join("fixtures/catalog/logiqx/catalog-a-v1.dat"))?;
        let logiqx_request = CatalogImportRequest {
            document_path: utf8(fixture_root.join("fixtures/catalog/logiqx/catalog-a-v1.dat"))?,
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("logiqx-source"),
            source_display_name: "Logiqx source".to_owned(),
            catalog_key: CatalogKey::new("logiqx-catalog"),
            catalog_display_name: "Logiqx catalog".to_owned(),
            scope: CatalogScope::Unknown,
        };
        app::import_catalog(&database, &logiqx_request)?;

        let mut conn = connect(&source_path)?;
        let assertion = sql_query(
            "SELECT assertion_key AS value FROM relationship_assertions \
             WHERE source_field = 'device_ref' LIMIT 1",
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
                    rule_version: "backup-round-trip-v1".to_owned(),
                    supporting_assertions: vec![supporting_assertion],
                },
                evidence: serde_json::json!({"test": "preserve adjudication"}),
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
        drop(database);

        create_backup(&source_path, &backup_path)?;
        restore_backup(&backup_path, &restored_path, RestorePolicy::CreateNew)?;

        let mut restored = connect(&restored_path)?;
        for (table, expected) in [
            ("catalogs", 2),
            ("catalog_snapshots", 2),
            ("snapshot_publications", 2),
            ("relationship_assertions", 3),
            ("relationship_reviews", 1),
        ] {
            let count = sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
                .get_result::<CountRow>(&mut restored)?
                .count;
            assert!(
                count >= expected,
                "{table}: expected at least {expected}, got {count}"
            );
        }
        let unknown_fields = sql_query(
            "SELECT COUNT(*) AS count FROM snapshot_extensions \
             WHERE field_name = 'flag' AND raw_value_json = '\"preserved\"'",
        )
        .get_result::<CountRow>(&mut restored)?
        .count;
        assert_eq!(unknown_fields, 1);
        for bytes in [&machine_bytes, &logiqx_bytes] {
            let payload = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
                .bind::<Binary, _>(Sha256::digest(bytes).as_slice())
                .get_result::<RetainedPayload>(&mut restored)?;
            assert_eq!(payload.payload, *bytes);
        }
        drop(restored);
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"keep me").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"keep me");
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"keep me").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"keep me");
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"keep me").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"keep me");
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"destination").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"destination");
        Ok(())
    }

    #[test]
    fn integrity_reports_retained_document_digest_corruption() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        add_retained_document(&path, b"original")?;
        let mut conn = connect(&path)?;
        let immutable_trigger = sql_query(
            "SELECT sql AS value FROM sqlite_schema \
             WHERE type = 'trigger' AND name = 'documents_are_immutable_update'",
        )
        .get_result::<TextValue>(&mut conn)?
        .value;
        sql_query("DROP TRIGGER documents_are_immutable_update").execute(&mut conn)?;
        sql_query("UPDATE documents SET payload = ? WHERE sha256 = ?")
            .bind::<Binary, _>(b"corrupt!".as_slice())
            .bind::<Binary, _>(Sha256::digest(b"original").as_slice())
            .execute(&mut conn)?;
        sql_query(immutable_trigger).execute(&mut conn)?;

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
    fn integrity_separates_rebuildable_foreign_key_problems() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database_guard = database(&path)?;
        let mut conn = connect(&path)?;
        sql_query("PRAGMA foreign_keys = OFF").execute(&mut conn)?;
        sql_query(
            "INSERT INTO roms (name, size, md5, sha1, crc, game_id) \
             VALUES ('orphan-rom', 0, zeroblob(16), zeroblob(20), zeroblob(4), 999)",
        )
        .execute(&mut conn)?;

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
                .any(|issue| issue.contains("roms"))
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
    fn integrity_checks_durable_foreign_keys_after_many_inventory_violations() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database = database(&path)?;
        let mut conn = connect(&path)?;
        sql_query("PRAGMA foreign_keys = OFF").execute(&mut conn)?;
        for id in 1..=60 {
            sql_query(
                "INSERT INTO roms (name, size, md5, sha1, crc, game_id) \
                 VALUES (?, 0, zeroblob(16), zeroblob(20), zeroblob(4), ?)",
            )
            .bind::<Text, _>(format!("orphan-rom-{id}"))
            .bind::<BigInt, _>(id)
            .execute(&mut conn)?;
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
                .any(|issue| issue.contains("acquisitions"))
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"keep me").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"keep me");
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
        let mut conn = connect(&destination)?;
        let retained = sql_query("SELECT payload FROM documents WHERE sha256 = ?")
            .bind::<Binary, _>(Sha256::digest(b"keep me").as_slice())
            .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(retained.payload, b"keep me");
        Ok(())
    }

    #[test]
    fn integrity_does_not_create_migration_metadata_in_an_unrecognized_database() -> Result<()> {
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
                .any(|issue| issue.contains("migration"))
        );
        let mut conn = connect(&path)?;
        let migration_tables = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_schema \
             WHERE type = 'table' AND name = '__diesel_schema_migrations'",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(migration_tables, 0);
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
    fn integrity_rejects_a_schema_that_disagrees_with_migration_history() -> Result<()> {
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

    #[test]
    fn integrity_checks_legacy_payload_against_parent_document_metadata() -> Result<()> {
        let directory = tempdir()?;
        let path = utf8(directory.path().join("cache.sqlite"))?;
        let database = database(&path)?;
        let mut conn = connect(&path)?;
        let payload = b"legacy bytes";
        let key = crate::domain::DocumentKey::from_bytes(payload).to_string();
        sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES (?, ?, ?)")
            .bind::<Text, _>(&key)
            .bind::<Binary, _>(sha1::Sha1::digest(payload).as_slice())
            .bind::<BigInt, _>(
                i64::try_from(payload.len()).map_err(|error| backup_error(error.to_string()))?,
            )
            .execute(&mut conn)?;
        sql_query(
            "INSERT INTO legacy_document_payloads \
             (document_key, sha256, sha1, byte_length, payload, format_hint) \
             VALUES (?, ?, ?, ?, ?, 'test')",
        )
        .bind::<Text, _>(&key)
        .bind::<Binary, _>(Sha256::digest(payload).as_slice())
        .bind::<Binary, _>(sha1::Sha1::digest(payload).as_slice())
        .bind::<BigInt, _>(
            i64::try_from(payload.len()).map_err(|error| backup_error(error.to_string()))?,
        )
        .bind::<Binary, _>(payload)
        .execute(&mut conn)?;
        sql_query("DROP TRIGGER documents_are_immutable_update").execute(&mut conn)?;
        sql_query("UPDATE documents SET sha1 = zeroblob(20) WHERE document_key = ?")
            .bind::<Text, _>(&key)
            .execute(&mut conn)?;
        let mut report = IntegrityReport::default();
        check_legacy_documents(&mut conn, &mut report)?;
        drop(conn);
        drop(database);
        assert!(
            report
                .durable_issues
                .iter()
                .any(|issue| issue.contains("legacy retained document")),
            "{report:?}"
        );
        Ok(())
    }

    #[derive(QueryableByName)]
    struct RetainedPayload {
        #[diesel(sql_type = Binary)]
        payload: Vec<u8>,
    }

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }
}
