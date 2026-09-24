use std::{fs::File, io::Read};

use camino::Utf8Path;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Binary, Nullable, Text},
};
use sha1::{Digest as _, Sha1};

use crate::{
    document_input::{self, MAX_DOCUMENT_BYTES},
    domain::{AcquisitionKey, DocumentDigest, DocumentKey, PublishingSourceKey},
    hashes::Sha1Digest,
    logiqx::DataFile,
    storage::db::{Pool, create_db_pool},
};

#[derive(Clone, Debug)]
pub struct AcquisitionMetadata {
    pub source_key: PublishingSourceKey,
    pub source_uri: Option<String>,
    pub method: Option<String>,
    pub transport_metadata: Option<serde_json::Value>,
    pub expected_sha256: Option<DocumentDigest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormatHint {
    LogiqxXml,
    LogiqxXmlGzip,
}

impl FormatHint {
    const fn as_str(self) -> &'static str {
        match self {
            Self::LogiqxXml => "logiqx+xml",
            Self::LogiqxXmlGzip => "logiqx+xml+gzip",
        }
    }

    fn for_bytes(bytes: &[u8]) -> Self {
        if bytes.starts_with(&[0x1f, 0x8b]) {
            Self::LogiqxXmlGzip
        } else {
            Self::LogiqxXml
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedDocument {
    pub document_key: DocumentKey,
    pub acquisition_key: AcquisitionKey,
}

pub struct DocumentStore {
    pool: Pool,
}

#[derive(QueryableByName)]
struct ExistingDocument {
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Binary>)]
    payload: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    retention_status: String,
}

#[derive(QueryableByName)]
struct RetainedPayload {
    #[diesel(sql_type = Binary)]
    payload: Vec<u8>,
}

impl DocumentStore {
    pub fn open(database_url: &str) -> crate::Result<Self> {
        Ok(Self {
            pool: create_db_pool(database_url)?,
        })
    }

    #[cfg(test)]
    const fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn retain<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
    ) -> crate::Result<RetainedDocument> {
        self.retain_with_limit(metadata, reader, MAX_DOCUMENT_BYTES)
    }

    pub fn retain_path(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        let mut metadata = AcquisitionMetadata {
            source_key,
            source_uri: Some(path.as_str().to_owned()),
            method: Some("local-file".to_owned()),
            transport_metadata: None,
            expected_sha256: None,
        };
        let canonical_path = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                self.record_failed_attempt(&metadata, "io", &error.to_string())?;
                return Err(error.into());
            }
        };
        if !canonical_path.is_file() {
            let error = crate::Error::InvalidPath(format!(
                "catalog document is not a regular file: {}",
                canonical_path.display()
            ));
            self.record_failed_attempt(&metadata, "unsafe_path", &error.to_string())?;
            return Err(error);
        }
        metadata.source_uri = Some(canonical_path.to_string_lossy().into_owned());
        match File::open(&canonical_path) {
            Ok(file) => self.retain(&metadata, file),
            Err(error) => {
                self.record_failed_attempt(&metadata, "io", &error.to_string())?;
                Err(error.into())
            }
        }
    }

    pub fn load(&self, key: &DocumentKey) -> crate::Result<Vec<u8>> {
        let mut conn = self.pool.get()?;
        let row = sql_query(
            "SELECT payload FROM documents \
             WHERE document_key = ? AND retention_status = 'retained' AND payload IS NOT NULL",
        )
        .bind::<Text, _>(key.to_string())
        .get_result::<RetainedPayload>(&mut conn)
        .optional()?
        .ok_or_else(|| crate::Error::DocumentUnavailable(key.to_string()))?;
        Ok(row.payload)
    }

    fn retain_with_limit<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
        limit: usize,
    ) -> crate::Result<RetainedDocument> {
        let transport_metadata = metadata
            .transport_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let raw = match document_input::read_bounded(reader, limit) {
            Ok(raw) => raw,
            Err(error) => {
                self.record_failed_attempt(metadata, error_code(&error), &error.to_string())?;
                return Err(error);
            }
        };
        let key = DocumentKey::from_bytes(&raw);
        if metadata
            .expected_sha256
            .is_some_and(|expected| expected.as_bytes() != key.digest())
        {
            let error = crate::Error::DocumentDigestMismatch;
            self.record_failed_attempt(metadata, error_code(&error), &error.to_string())?;
            return Err(error);
        }
        if let Err(error) = DataFile::from_reader(raw.as_slice()) {
            self.record_failed_attempt(metadata, error_code(&error), &error.to_string())?;
            return Err(error);
        }

        let result =
            self.persist_retained(metadata, transport_metadata.as_deref(), &raw, key, limit);
        match result {
            Ok(retained) => Ok(retained),
            Err(error) => {
                self.record_failed_attempt(metadata, error_code(&error), &error.to_string())?;
                Err(error)
            }
        }
    }

    fn persist_retained(
        &self,
        metadata: &AcquisitionMetadata,
        transport_metadata: Option<&str>,
        raw: &[u8],
        key: DocumentKey,
        limit: usize,
    ) -> crate::Result<RetainedDocument> {
        let verification_status = if metadata.expected_sha256.is_some() {
            "verified"
        } else {
            "unverified"
        };
        let sha1 = sha1(raw);
        let byte_length =
            i64::try_from(raw.len()).map_err(|_| crate::Error::DocumentTooLarge { limit })?;
        let format_hint = FormatHint::for_bytes(raw);
        let acquisition_key = AcquisitionKey::fresh();
        let attempt_key = uuid::Uuid::new_v4().to_string();
        let acquisition_key_string = acquisition_key.to_string();
        let document_key_string = key.to_string();
        {
            let mut conn = self.pool.get()?;
            conn.immediate_transaction::<_, crate::Error, _>(|conn| {
                let existing = sql_query(
                    "SELECT document_key, payload, retention_status FROM documents WHERE sha256 = ?",
                )
                .bind::<Binary, _>(key.digest().as_slice())
                .get_result::<ExistingDocument>(conn)
                .optional()?;
                match existing {
                    Some(document)
                        if document.document_key == document_key_string
                            && document.retention_status == "retained"
                            && document.payload.as_deref() == Some(raw) => {}
                    Some(_) => return Err(crate::Error::DocumentDigestCollision),
                    None => {
                        sql_query(
                            "INSERT INTO documents \
                             (document_key, sha1, byte_length, sha256, payload, format_hint, retention_status) \
                             VALUES (?, ?, ?, ?, ?, ?, 'retained')",
                        )
                        .bind::<Text, _>(&document_key_string)
                        .bind::<Binary, _>(sha1.as_slice())
                        .bind::<diesel::sql_types::BigInt, _>(byte_length)
                        .bind::<Binary, _>(key.digest().as_slice())
                        .bind::<Binary, _>(&raw)
                        .bind::<Text, _>(format_hint.as_str())
                        .execute(conn)?;
                    }
                }

                sql_query(
                    "INSERT INTO acquisitions \
                     (acquisition_key, source_key, document_key, source_uri, method, acquired_at, \
                      transport_metadata_json, expected_sha256, verification_status) \
                     VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, ?, ?, ?)",
                )
                .bind::<Text, _>(&acquisition_key_string)
                .bind::<Text, _>(metadata.source_key.as_str())
                .bind::<Text, _>(&document_key_string)
                .bind::<Nullable<Text>, _>(&metadata.source_uri)
                .bind::<Nullable<Text>, _>(&metadata.method)
                .bind::<Nullable<Text>, _>(&transport_metadata)
                .bind::<Nullable<Binary>, _>(
                    metadata.expected_sha256.map(|digest| digest.as_bytes().to_vec()),
                )
                .bind::<Text, _>(verification_status)
                .execute(conn)?;

                sql_query(
                    "INSERT INTO acquisition_attempts \
                     (attempt_key, source_key, source_uri, method, transport_metadata_json, expected_sha256, outcome, \
                      verification_status, document_key, acquisition_key) \
                     VALUES (?, ?, ?, ?, ?, ?, 'retained', ?, ?, ?)",
                )
                .bind::<Text, _>(&attempt_key)
                .bind::<Text, _>(metadata.source_key.as_str())
                .bind::<Nullable<Text>, _>(&metadata.source_uri)
                .bind::<Nullable<Text>, _>(&metadata.method)
                .bind::<Nullable<Text>, _>(&transport_metadata)
                .bind::<Nullable<Binary>, _>(
                    metadata.expected_sha256.map(|digest| digest.as_bytes().to_vec()),
                )
                .bind::<Text, _>(verification_status)
                .bind::<Text, _>(&document_key_string)
                .bind::<Text, _>(&acquisition_key_string)
                .execute(conn)?;

                Ok(RetainedDocument {
                    document_key: key,
                    acquisition_key,
                })
            })
        }
    }

    fn record_failed_attempt(
        &self,
        metadata: &AcquisitionMetadata,
        code: &str,
        diagnostic: &str,
    ) -> crate::Result<()> {
        let transport_metadata = metadata
            .transport_metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let mut conn = self.pool.get()?;
        sql_query(
            "INSERT INTO acquisition_attempts \
             (attempt_key, source_key, source_uri, method, transport_metadata_json, expected_sha256, outcome, \
              verification_status, diagnostic) \
             VALUES (?, ?, ?, ?, ?, ?, 'failed', 'rejected', ?)",
        )
        .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
        .bind::<Text, _>(metadata.source_key.as_str())
        .bind::<Nullable<Text>, _>(&metadata.source_uri)
        .bind::<Nullable<Text>, _>(&metadata.method)
        .bind::<Nullable<Text>, _>(transport_metadata)
        .bind::<Nullable<Binary>, _>(
            metadata.expected_sha256.map(|digest| digest.as_bytes().to_vec()),
        )
        .bind::<Text, _>(format!("{code}: {diagnostic}"))
        .execute(&mut conn)?;
        Ok(())
    }
}

fn sha1(bytes: &[u8]) -> Sha1Digest {
    Sha1::digest(bytes).into()
}

const fn error_code(error: &crate::Error) -> &'static str {
    match error {
        crate::Error::DocumentTooLarge { .. } => "document_too_large",
        crate::Error::DocumentDigestMismatch => "digest_mismatch",
        crate::Error::XmlEntityNotAllowed => "entity_not_allowed",
        crate::Error::XmlValidation(_) | crate::Error::Xml(_) => "invalid_xml",
        crate::Error::Io(_) => "read_failed",
        _ => "retention_failed",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Barrier},
        thread,
    };

    use diesel::{
        SqliteConnection, connection::SimpleConnection, migration::MigrationSource,
        sql_types::BigInt,
    };
    use diesel_migrations::MigrationHarness;
    use flate2::{Compression, write::GzEncoder};
    use tempfile::TempDir;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    const VALID_DAT: &[u8] =
        b"<datafile><header><name>Retention fixture</name></header></datafile>";

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn setup_store() -> TestResult<(TempDir, DocumentStore)> {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("catalog.sqlite");
        let pool = create_db_pool(&database_path.to_string_lossy())?;
        let mut conn = pool.get()?;
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name) \
             VALUES ('source-a', 'Publisher A'), ('source-b', 'Publisher B');",
        )?;
        drop(conn);
        Ok((directory, DocumentStore::new(pool)))
    }

    fn acquisition(source_key: &str) -> AcquisitionMetadata {
        AcquisitionMetadata {
            source_key: PublishingSourceKey::new(source_key),
            source_uri: Some(format!("https://example.invalid/{source_key}/catalog.dat")),
            method: Some("https".to_owned()),
            transport_metadata: Some(serde_json::json!({"content-type": "application/xml"})),
            expected_sha256: None,
        }
    }

    fn count(store: &DocumentStore, table: &str) -> TestResult<i64> {
        let mut conn = store.pool.get()?;
        Ok(sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
            .get_result::<CountRow>(&mut conn)
            .map(|row| row.count)?)
    }

    #[test]
    fn document_keys_are_stable_and_content_derived() {
        assert_eq!(
            DocumentKey::from_bytes(b"catalog"),
            DocumentKey::from_bytes(b"catalog")
        );
        assert_ne!(
            DocumentKey::from_bytes(b"catalog"),
            DocumentKey::from_bytes(b"changed")
        );
    }

    #[test]
    fn retain_path_rejects_directories_and_records_the_failed_attempt() -> TestResult {
        let (directory, store) = setup_store()?;
        let path = Utf8Path::from_path(directory.path()).ok_or("non-UTF-8 test path")?;
        let result = store.retain_path(PublishingSourceKey::new("source-a"), path);
        assert!(matches!(result, Err(crate::Error::InvalidPath(_))));
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisitions")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
        Ok(())
    }

    #[test]
    fn identical_bytes_share_payload_but_keep_each_source_acquisition() -> TestResult {
        let (_directory, store) = setup_store()?;
        let first = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let second = store.retain(&acquisition("source-b"), VALID_DAT)?;
        assert_eq!(first.document_key, second.document_key);
        assert_ne!(first.acquisition_key, second.acquisition_key);
        assert_eq!(count(&store, "documents")?, 1);
        assert_eq!(count(&store, "acquisitions")?, 2);
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        let retained_bytes = store.load(&first.document_key)?;
        assert_eq!(retained_bytes, VALID_DAT);
        assert_eq!(
            DataFile::from_reader(retained_bytes.as_slice())?
                .header()
                .name(),
            "Retention fixture"
        );
        let mut conn = store.pool.get()?;
        let status =
            sql_query("SELECT verification_status FROM acquisitions WHERE acquisition_key = ?")
                .bind::<Text, _>(first.acquisition_key.to_string())
                .get_result::<StatusRow>(&mut conn)?
                .verification_status;
        assert_eq!(status, "unverified");
        Ok(())
    }

    #[test]
    fn concurrent_identical_retention_shares_payload_and_keeps_both_acquisitions() -> TestResult {
        let (directory, store) = setup_store()?;
        let second_database = directory.path().join("catalog.sqlite");
        let stores = [
            Arc::new(store),
            Arc::new(DocumentStore::open(&second_database.to_string_lossy())?),
        ];
        let barrier = Arc::new(Barrier::new(2));
        let spawn = |index, store: Arc<DocumentStore>| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                store.retain(
                    &acquisition(if index == 0 { "source-a" } else { "source-b" }),
                    VALID_DAT,
                )
            })
        };
        let join_retention = |handle: thread::JoinHandle<crate::Result<RetainedDocument>>| {
            handle
                .join()
                .map_err(|_| io::Error::other("retention thread panicked"))?
                .map_err(|error| io::Error::other(error.to_string()))
        };
        let first_handle = spawn(0, Arc::clone(&stores[0]));
        let second_handle = spawn(1, Arc::clone(&stores[1]));
        let first = join_retention(first_handle)?;
        let second = join_retention(second_handle)?;
        assert_eq!(first.document_key, second.document_key);
        assert_ne!(first.acquisition_key, second.acquisition_key);
        assert_eq!(count(&stores[0], "documents")?, 1);
        assert_eq!(count(&stores[0], "acquisitions")?, 2);
        Ok(())
    }

    #[test]
    fn changed_bytes_get_new_immutable_document_keys() -> TestResult {
        let (_directory, store) = setup_store()?;
        let original = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let changed: &[u8] = b"<datafile><header><name>Changed</name></header></datafile>";
        let replacement = store.retain(&acquisition("source-a"), changed)?;
        assert_ne!(original.document_key, replacement.document_key);
        assert_eq!(store.load(&original.document_key)?, VALID_DAT);
        assert_eq!(store.load(&replacement.document_key)?, changed);
        assert_eq!(count(&store, "documents")?, 2);
        Ok(())
    }

    #[test]
    fn malformed_or_external_entity_documents_record_failure_without_payload() -> TestResult {
        let (_directory, store) = setup_store()?;
        let malformed: &[u8] = b"<datafile><header>";
        assert!(store.retain(&acquisition("source-a"), malformed).is_err());
        let external: &[u8] =
            br#"<!DOCTYPE datafile SYSTEM "http://127.0.0.1:9/catalog.dtd"><datafile/>"#;
        assert!(store.retain(&acquisition("source-a"), external).is_err());
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisitions")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        let mut conn = store.pool.get()?;
        let failures = sql_query(
            "SELECT COUNT(*) AS count FROM acquisition_attempts \
             WHERE outcome = 'failed' AND diagnostic IS NOT NULL",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(failures, 2);
        Ok(())
    }

    #[test]
    fn retained_bytes_and_provenance_survive_database_backup_restore() -> TestResult {
        let (directory, store) = setup_store()?;
        let retained = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let backup_path = directory.path().join("restored.sqlite");
        let original_path = directory.path().join("catalog.sqlite");
        drop(store);
        std::fs::copy(&original_path, &backup_path)?;
        let restored = DocumentStore::open(&backup_path.to_string_lossy())?;
        assert_eq!(restored.load(&retained.document_key)?, VALID_DAT);
        assert_eq!(count(&restored, "acquisitions")?, 1);
        assert_eq!(count(&restored, "acquisition_attempts")?, 1);
        Ok(())
    }

    #[test]
    fn oversized_stream_is_recorded_as_a_failed_attempt() -> TestResult {
        let (_directory, store) = setup_store()?;
        let oversized = io::Cursor::new(VALID_DAT);
        let result = store.retain_with_limit(&acquisition("source-a"), oversized, 8);
        assert!(matches!(
            result,
            Err(crate::Error::DocumentTooLarge { limit: 8 })
        ));
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
        Ok(())
    }

    #[derive(QueryableByName)]
    struct StatusRow {
        #[diesel(sql_type = Text)]
        verification_status: String,
    }

    #[derive(QueryableByName)]
    struct ExpectedDigestRow {
        #[diesel(sql_type = Nullable<Binary>)]
        expected_sha256: Option<Vec<u8>>,
    }

    #[test]
    fn verifies_source_digest_when_available_and_rejects_mismatches() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut verified = acquisition("source-a");
        verified.expected_sha256 = Some(DocumentDigest::from_bytes(VALID_DAT));
        let retained = store.retain(&verified, VALID_DAT)?;
        let mut mismatched = acquisition("source-b");
        mismatched.expected_sha256 = Some(DocumentDigest::from_bytes(b"other bytes"));
        assert!(matches!(
            store.retain(&mismatched, VALID_DAT),
            Err(crate::Error::DocumentDigestMismatch)
        ));
        let mut conn = store.pool.get()?;
        let status =
            sql_query("SELECT verification_status FROM acquisitions WHERE acquisition_key = ?")
                .bind::<Text, _>(retained.acquisition_key.to_string())
                .get_result::<StatusRow>(&mut conn)?
                .verification_status;
        assert_eq!(status, "verified");
        let acquisition_digest =
            sql_query("SELECT expected_sha256 FROM acquisitions WHERE acquisition_key = ?")
                .bind::<Text, _>(retained.acquisition_key.to_string())
                .get_result::<ExpectedDigestRow>(&mut conn)?
                .expected_sha256;
        assert_eq!(
            acquisition_digest,
            Some(DocumentDigest::from_bytes(VALID_DAT).as_bytes().to_vec())
        );
        let failed_digest =
            sql_query("SELECT expected_sha256 FROM acquisition_attempts WHERE outcome = 'failed'")
                .get_result::<ExpectedDigestRow>(&mut conn)?
                .expected_sha256;
        assert_eq!(
            failed_digest,
            Some(
                DocumentDigest::from_bytes(b"other bytes")
                    .as_bytes()
                    .to_vec()
            )
        );
        assert_eq!(count(&store, "documents")?, 1);
        assert_eq!(count(&store, "acquisitions")?, 1);
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        Ok(())
    }

    #[test]
    fn rejects_digest_mismatch_before_parsing_and_records_digest_failure() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut metadata = acquisition("source-a");
        metadata.expected_sha256 = Some(DocumentDigest::from_bytes(VALID_DAT));
        assert!(matches!(
            store.retain(&metadata, b"malformed XML".as_slice()),
            Err(crate::Error::DocumentDigestMismatch)
        ));
        let mut conn = store.pool.get()?;
        let diagnostic = sql_query("SELECT diagnostic FROM acquisition_attempts")
            .get_result::<DiagnosticRow>(&mut conn)?
            .diagnostic;
        assert!(diagnostic.starts_with("digest_mismatch:"));
        assert_eq!(count(&store, "documents")?, 0);
        Ok(())
    }

    #[derive(QueryableByName)]
    struct DiagnosticRow {
        #[diesel(sql_type = Text)]
        diagnostic: String,
    }

    #[test]
    fn retained_payload_and_acquisition_rows_cannot_be_replaced() -> TestResult {
        let (_directory, store) = setup_store()?;
        let retained = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let mut conn = store.pool.get()?;
        assert!(
            sql_query("UPDATE documents SET payload = X'00' WHERE document_key = ?",)
                .bind::<Text, _>(retained.document_key.to_string())
                .execute(&mut conn)
                .is_err()
        );
        assert!(
            sql_query(
                "INSERT OR REPLACE INTO documents \
             (document_key, sha1, byte_length, sha256, payload, format_hint, retention_status) \
             VALUES (?, X'00', 1, ?, X'00', 'logiqx+xml', 'retained')",
            )
            .bind::<Text, _>(retained.document_key.to_string())
            .bind::<Binary, _>(retained.document_key.digest().as_slice())
            .execute(&mut conn)
            .is_err()
        );
        assert!(
            sql_query("UPDATE acquisitions SET source_uri = 'changed' WHERE acquisition_key = ?",)
                .bind::<Text, _>(retained.acquisition_key.to_string())
                .execute(&mut conn)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn retained_attempt_must_reference_its_acquisitions_document() -> TestResult {
        let (_directory, store) = setup_store()?;
        let first = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let changed: &[u8] = b"<datafile><header><name>Other</name></header></datafile>";
        let second = store.retain(&acquisition("source-b"), changed)?;
        let mut conn = store.pool.get()?;
        let mismatched_document = sql_query(
            "INSERT INTO acquisition_attempts \
             (attempt_key, source_key, outcome, verification_status, document_key, acquisition_key) \
             VALUES ('mismatched-pair', 'source-a', 'retained', 'unverified', ?, ?)",
        )
        .bind::<Text, _>(second.document_key.to_string())
        .bind::<Text, _>(first.acquisition_key.to_string())
        .execute(&mut conn);
        assert!(mismatched_document.is_err());
        let mismatched_source = sql_query(
            "INSERT INTO acquisition_attempts \
             (attempt_key, source_key, outcome, verification_status, document_key, acquisition_key) \
             VALUES ('mismatched-source', 'source-b', 'retained', 'unverified', ?, ?)",
        )
        .bind::<Text, _>(first.document_key.to_string())
        .bind::<Text, _>(first.acquisition_key.to_string())
        .execute(&mut conn);
        assert!(mismatched_source.is_err());
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        Ok(())
    }

    #[test]
    fn compressed_documents_retain_original_bytes_and_parse_with_expansion_bounds() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, VALID_DAT)?;
        let compressed = encoder.finish()?;
        let retained = store.retain(&acquisition("source-a"), compressed.as_slice())?;
        assert_eq!(store.load(&retained.document_key)?, compressed);
        assert_eq!(
            DataFile::from_reader(compressed.as_slice())?
                .header()
                .name(),
            "Retention fixture"
        );
        Ok(())
    }

    #[test]
    fn migration_keeps_prior_identity_rows_unavailable_until_bytes_are_retained() -> TestResult {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = crate::storage::db::MIGRATIONS.migrations()?;
        assert!(migrations.len() > 1);
        conn.applied_migrations()?;
        for migration in &migrations[..migrations.len() - 1] {
            conn.run_migration(migration.as_ref())?;
        }
        sql_query(
            "INSERT INTO publishing_sources (source_key, display_name) VALUES ('legacy', 'Legacy')",
        )
        .execute(&mut conn)?;
        sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES ('metadata-only', X'0000000000000000000000000000000000000000', 1)")
            .execute(&mut conn)?;
        sql_query(
            "INSERT INTO acquisitions (acquisition_key, source_key, document_key, method) \
             VALUES ('legacy-acquisition', 'legacy', 'metadata-only', 'legacy-import')",
        )
        .execute(&mut conn)?;

        conn.run_pending_migrations(crate::storage::db::MIGRATIONS)?;
        let document = sql_query(
            "SELECT retention_status, payload FROM documents WHERE document_key = 'metadata-only'",
        )
        .get_result::<DocumentStatusRow>(&mut conn)?;
        assert_eq!(document.retention_status, "unavailable");
        assert!(document.payload.is_none());
        let status = sql_query(
            "SELECT verification_status FROM acquisitions WHERE acquisition_key = 'legacy-acquisition'",
        )
        .get_result::<StatusRow>(&mut conn)?
        .verification_status;
        assert_eq!(status, "unverified");
        assert!(
            conn.run_pending_migrations(crate::storage::db::MIGRATIONS)?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn down_migration_removes_new_triggers_and_columns() -> TestResult {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        conn.run_pending_migrations(crate::storage::db::MIGRATIONS)?;
        sql_query(
            "INSERT INTO publishing_sources (source_key, display_name) VALUES ('legacy', 'Legacy')",
        )
        .execute(&mut conn)?;
        sql_query("INSERT INTO documents (document_key) VALUES ('legacy-document')")
            .execute(&mut conn)?;
        sql_query(
            "INSERT INTO acquisitions (acquisition_key, source_key, document_key) \
             VALUES ('legacy-acquisition', 'legacy', 'legacy-document')",
        )
        .execute(&mut conn)?;

        conn.revert_last_migration(crate::storage::db::MIGRATIONS)?;
        sql_query(
            "UPDATE acquisitions SET source_uri = 'restored' \
             WHERE acquisition_key = 'legacy-acquisition'",
        )
        .execute(&mut conn)?;
        let attempts_table = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master \
             WHERE type = 'table' AND name = 'acquisition_attempts'",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(attempts_table, 0);
        Ok(())
    }

    #[derive(QueryableByName)]
    struct DocumentStatusRow {
        #[diesel(sql_type = Text)]
        retention_status: String,
        #[diesel(sql_type = Nullable<Binary>)]
        payload: Option<Vec<u8>>,
    }
}
