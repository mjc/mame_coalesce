use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{Binary, Nullable, Text},
};
use sha1::{Digest as _, Sha1};

use crate::{
    document_input::{self, MAX_DOCUMENT_BYTES},
    domain::{
        AcquisitionKey, DocumentDigest, DocumentKey, PublishingSource, PublishingSourceKey,
        SnapshotKey,
    },
    hashes::Sha1Digest,
    logiqx::DataFile,
    storage::{
        catalog_ids::{FetchAttemptId, FileReceiptId, SourceFileId},
        db::{Pool, create_db_pool},
    },
};

#[derive(Clone, Debug)]
pub struct AcquisitionMetadata {
    pub source_key: PublishingSourceKey,
    pub source_uri: Option<String>,
    pub method: Option<String>,
    pub transport_headers: Vec<TransportHeader>,
    pub expected_sha256: Option<DocumentDigest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedDocument {
    pub document_key: DocumentKey,
    pub source_file_id: SourceFileId,
    pub file_receipt_id: FileReceiptId,
    /// Opaque compatibility key for the same persisted receipt.
    pub acquisition_key: AcquisitionKey,
}

struct RetainedPublication {
    document_key: DocumentKey,
    attempt_key: String,
    receipt_key: String,
    acquisition_key: AcquisitionKey,
    object_key: String,
    sha1: [u8; 20],
    byte_length: i64,
}

pub struct DocumentStore {
    pool: Pool,
    object_store: Utf8PathBuf,
    _database_guard: Option<crate::database::Database>,
}

#[derive(QueryableByName)]
struct RetainedPayload {
    #[diesel(sql_type = Binary)]
    sha256: Vec<u8>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Text)]
    object_key: String,
    #[diesel(sql_type = Text)]
    codec: String,
}

#[derive(QueryableByName)]
struct SourceFileRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    source_file_id: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Text)]
    object_key: String,
    #[diesel(sql_type = Text)]
    codec: String,
}

#[derive(QueryableByName)]
struct InsertedId {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    id: i64,
}

#[derive(QueryableByName)]
struct DatabaseFile {
    #[diesel(sql_type = Text)]
    file: String,
}

#[derive(QueryableByName)]
struct PublishingSourceRow {
    #[diesel(sql_type = Text)]
    source_key: String,
    #[diesel(sql_type = Text)]
    display_name: String,
}

impl DocumentStore {
    pub(crate) fn from_pool(pool: Pool) -> crate::Result<Self> {
        let object_store = object_store_for_pool(&pool)?;
        Ok(Self {
            pool,
            object_store,
            _database_guard: None,
        })
    }

    pub fn open(database_url: &str) -> crate::Result<Self> {
        if database_url == ":memory:" {
            return Self::from_pool(create_db_pool(database_url)?);
        }
        let database = crate::database::Database::open(&camino::Utf8PathBuf::from(database_url))?;
        let pool = database.pool().clone();
        let object_store = object_store_for_pool(&pool)?;
        Ok(Self {
            pool,
            object_store,
            _database_guard: Some(database),
        })
    }

    pub fn register_source(&self, source: &PublishingSource) -> crate::Result<()> {
        let mut conn = self.pool.get()?;
        super::publishing_sources::register(&mut conn, source)
    }

    pub fn resolve_source(
        &self,
        key: &PublishingSourceKey,
    ) -> crate::Result<Option<PublishingSource>> {
        let mut conn = self.pool.get()?;
        let source = sql_query(
            "SELECT publisher_key AS source_key, display_name \
             FROM catalog_publishers WHERE publisher_key = ?",
        )
        .bind::<Text, _>(key.as_str())
        .get_result::<PublishingSourceRow>(&mut conn)
        .optional()?;
        Ok(source.map(|source| PublishingSource::new(source.source_key, source.display_name)))
    }

    pub fn retain<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
    ) -> crate::Result<RetainedDocument> {
        self.retain_with_limit_and_validation(metadata, reader, MAX_DOCUMENT_BYTES, true)
    }

    #[cfg(test)]
    pub(crate) fn retain_unvalidated<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
    ) -> crate::Result<RetainedDocument> {
        self.retain_with_options(metadata, reader, MAX_DOCUMENT_BYTES, false)
    }

    pub fn retain_path(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        // Catalog import performs format validation while parsing this same
        // retained byte sequence; a preliminary XML walk would traverse it twice.
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_mame(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_clrmamepro(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_mame_softwarelist(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_no_intro_pc_xml(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_no_intro_dat_xml(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    pub(crate) fn retain_path_no_intro_database_xml(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false)
    }

    fn retain_path_with_options(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
        validate_xml: bool,
    ) -> crate::Result<RetainedDocument> {
        let mut metadata = AcquisitionMetadata {
            source_key,
            source_uri: Some(path.as_str().to_owned()),
            method: Some("local-file".to_owned()),
            transport_headers: Vec::new(),
            expected_sha256: None,
        };
        let canonical_path = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                self.record_failed_attempt(
                    &metadata,
                    "io",
                    &error.to_string(),
                    verification_before_read(&metadata),
                )?;
                return Err(error.into());
            }
        };
        if !canonical_path.is_file() {
            let error = crate::Error::InvalidPath(format!(
                "catalog document is not a regular file: {}",
                canonical_path.display()
            ));
            self.record_failed_attempt(
                &metadata,
                "unsafe_path",
                &error.to_string(),
                verification_before_read(&metadata),
            )?;
            return Err(error);
        }
        metadata.source_uri = Some(canonical_path.to_string_lossy().into_owned());
        match File::open(&canonical_path) {
            Ok(file) => self.retain_with_options(&metadata, file, MAX_DOCUMENT_BYTES, validate_xml),
            Err(error) => {
                self.record_failed_attempt(
                    &metadata,
                    "io",
                    &error.to_string(),
                    verification_before_read(&metadata),
                )?;
                Err(error.into())
            }
        }
    }

    pub fn load(&self, key: &DocumentKey) -> crate::Result<Vec<u8>> {
        let mut conn = self.pool.get()?;
        let key_string = key.to_string();
        let mut rows = sql_query(
            "SELECT sha256, sha1, byte_length, object_key, codec \
             FROM catalog_source_files WHERE sha256 = ?",
        )
        .bind::<Binary, _>(key.digest().as_slice())
        .load::<RetainedPayload>(&mut conn)?;
        if rows.len() > 1 {
            return Err(crate::Error::DocumentDigestCollision);
        }
        let retained = rows
            .pop()
            .ok_or_else(|| crate::Error::DocumentUnavailable(key_string.clone()))?;
        drop(conn);
        read_retained_object(&self.object_store, key, retained)
    }

    /// Recover exact original bytes by the catalog's typed source-file identity.
    pub fn load_source_file(&self, source_file_id: SourceFileId) -> crate::Result<Vec<u8>> {
        let retained = sql_query(
            "SELECT sha256, sha1, byte_length, object_key, codec \
             FROM catalog_source_files WHERE source_file_id = ?",
        )
        .bind::<diesel::sql_types::BigInt, _>(source_file_id.as_i64())
        .get_result::<RetainedPayload>(&mut self.pool.get()?)
        .optional()?
        .ok_or_else(|| crate::Error::DocumentUnavailable(source_file_id.as_i64().to_string()))?;
        let digest = hex::encode(&retained.sha256);
        let key =
            DocumentKey::from_digest(DocumentDigest::from_hex(&digest).map_err(|_| {
                crate::Error::DocumentUnavailable(source_file_id.as_i64().to_string())
            })?);
        read_retained_object(&self.object_store, &key, retained)
    }

    pub fn load_snapshot(&self, snapshot: &SnapshotKey) -> crate::Result<Vec<u8>> {
        Err(crate::Error::DocumentUnavailable(
            snapshot.as_str().to_owned(),
        ))
    }

    fn retain_with_limit_and_validation<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
        limit: usize,
        validate_xml: bool,
    ) -> crate::Result<RetainedDocument> {
        self.retain_with_options(metadata, reader, limit, validate_xml)
    }

    fn retain_with_options<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
        limit: usize,
        validate_xml: bool,
    ) -> crate::Result<RetainedDocument> {
        let raw = match document_input::read_bounded(reader, limit) {
            Ok(raw) => raw,
            Err(error) => {
                self.record_failed_attempt(
                    metadata,
                    error_code(&error),
                    &error.to_string(),
                    verification_before_read(metadata),
                )?;
                return Err(error);
            }
        };
        let key = DocumentKey::from_bytes(&raw);
        if metadata
            .expected_sha256
            .is_some_and(|expected| expected.as_bytes() != key.digest())
        {
            let error = crate::Error::DocumentDigestMismatch;
            self.record_failed_attempt(
                metadata,
                error_code(&error),
                &error.to_string(),
                "mismatch",
            )?;
            return Err(error);
        }
        if validate_xml && let Err(error) = DataFile::validate_document_bytes(&raw) {
            self.record_failed_attempt(
                metadata,
                error_code(&error),
                &error.to_string(),
                verification_after_digest(metadata),
            )?;
            return Err(error);
        }

        let result = self.persist_retained(metadata, &raw, key, limit);
        match result {
            Ok(retained) => Ok(retained),
            Err(error) => {
                self.record_failed_attempt(
                    metadata,
                    error_code(&error),
                    &error.to_string(),
                    verification_after_digest(metadata),
                )?;
                Err(error)
            }
        }
    }

    fn persist_retained(
        &self,
        metadata: &AcquisitionMetadata,
        raw: &[u8],
        key: DocumentKey,
        limit: usize,
    ) -> crate::Result<RetainedDocument> {
        let sha1 = sha1(raw);
        let byte_length =
            i64::try_from(raw.len()).map_err(|_| crate::Error::DocumentTooLarge { limit })?;
        let acquisition_key = AcquisitionKey::fresh();
        let attempt_key = uuid::Uuid::new_v4().to_string();
        let receipt_key = acquisition_key.to_string();
        let object_key = self.store_object(raw, key)?;
        self.persist_retained_rows(
            metadata,
            RetainedPublication {
                document_key: key,
                attempt_key,
                receipt_key,
                acquisition_key,
                object_key,
                sha1,
                byte_length,
            },
        )
    }

    fn persist_retained_rows(
        &self,
        metadata: &AcquisitionMetadata,
        publication: RetainedPublication,
    ) -> crate::Result<RetainedDocument> {
        let RetainedPublication {
            document_key,
            attempt_key,
            receipt_key,
            acquisition_key,
            object_key,
            sha1,
            byte_length,
        } = publication;
        let verification_status = verification_after_digest(metadata);
        let mut conn = self.pool.get()?;
        conn.immediate_transaction::<_, crate::Error, _>(|conn| {
            let publisher_id = super::publishing_sources::lookup_id(conn, &metadata.source_key)?
                .ok_or_else(|| {
                    crate::Error::SourceIdentityConflict(metadata.source_key.as_str().to_owned())
                })?;
            let source_file_id = ensure_source_file(
                conn,
                document_key.digest().as_slice(),
                &object_key,
                &sha1,
                byte_length,
            )?;
            let fetch = sql_query(
                "INSERT INTO catalog_fetch_attempts \
                 (publisher_id, attempt_key, uri, method, requested_at, responded_at, outcome, \
                  verification_status, declared_filename) \
                 VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, NULL, 'retained', ?, NULL) \
                 RETURNING fetch_attempt_id AS id",
            )
            .bind::<diesel::sql_types::BigInt, _>(publisher_id.as_i64())
            .bind::<Text, _>(&attempt_key)
            .bind::<Text, _>(metadata.source_uri.as_deref().unwrap_or_default())
            .bind::<Text, _>(metadata.method.as_deref().unwrap_or_default())
            .bind::<Text, _>(verification_status)
            .get_result::<InsertedId>(conn)?;
            let fetch_attempt_id = positive_id::<FetchAttemptId>(fetch.id)?;
            insert_fetch_headers(conn, fetch_attempt_id.as_i64(), &metadata.transport_headers)?;
            insert_expected_sha256(conn, fetch_attempt_id.as_i64(), metadata.expected_sha256)?;
            let receipt = sql_query(
                "INSERT INTO catalog_file_receipts \
                 (receipt_key, fetch_attempt_id, source_file_id) VALUES (?, ?, ?) \
                 RETURNING file_receipt_id AS id",
            )
            .bind::<Text, _>(&receipt_key)
            .bind::<diesel::sql_types::BigInt, _>(fetch_attempt_id.as_i64())
            .bind::<diesel::sql_types::BigInt, _>(source_file_id.as_i64())
            .get_result::<InsertedId>(conn)?;
            let file_receipt_id = positive_id::<FileReceiptId>(receipt.id)?;
            Ok(RetainedDocument {
                document_key,
                source_file_id,
                file_receipt_id,
                acquisition_key,
            })
        })
    }

    fn store_object(&self, raw: &[u8], key: DocumentKey) -> crate::Result<String> {
        let object_key = format!("sha256/{}.zst", hex::encode(key.digest()));
        let object_path = self.object_store.join(&object_key);
        let parent = object_path
            .parent()
            .ok_or_else(|| crate::Error::InvalidPath(object_path.to_string()))?;
        fs::create_dir_all(parent)?;
        if object_path.exists() {
            let existing = read_object(&object_path, raw.len())?;
            if existing != raw {
                return Err(crate::Error::DocumentDigestCollision);
            }
            return Ok(object_key);
        }
        let compressed = zstd::encode_all(raw, 3)?;
        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            hex::encode(key.digest()),
            uuid::Uuid::new_v4()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&compressed)?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temporary, &object_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if read_object(&object_path, raw.len())? != raw {
                    let _ = fs::remove_file(&temporary);
                    return Err(crate::Error::DocumentDigestCollision);
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                return Err(error.into());
            }
        }
        fs::remove_file(temporary)?;
        Ok(object_key)
    }

    fn record_failed_attempt(
        &self,
        metadata: &AcquisitionMetadata,
        code: &str,
        diagnostic: &str,
        verification_status: &str,
    ) -> crate::Result<()> {
        let attempt_key = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get()?;
        conn.immediate_transaction::<_, crate::Error, _>(|conn| {
            let fetch_attempt =
                insert_fetch_attempt(conn, metadata, &attempt_key, "failed", verification_status)?;
            insert_fetch_headers(conn, fetch_attempt.as_i64(), &metadata.transport_headers)?;
            insert_expected_sha256(conn, fetch_attempt.as_i64(), metadata.expected_sha256)?;
            let _ = (code, diagnostic);
            Ok(())
        })
    }
}

fn read_retained_object(
    object_store: &Utf8Path,
    key: &DocumentKey,
    retained: RetainedPayload,
) -> crate::Result<Vec<u8>> {
    let key_string = key.to_string();
    if retained.sha256.as_slice() != key.digest() || retained.codec != "zstd" {
        return Err(crate::Error::DocumentDigestCollision);
    }
    let expected_length = usize::try_from(retained.byte_length)
        .map_err(|_| crate::Error::DocumentUnavailable(key_string.clone()))?;
    let object_path = object_store.join(&retained.object_key);
    let mut decoder = zstd::Decoder::new(File::open(object_path)?)?;
    let mut payload = Vec::with_capacity(expected_length.min(MAX_DOCUMENT_BYTES));
    decoder
        .by_ref()
        .take(
            u64::try_from(expected_length)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut payload)?;
    if payload.len() != expected_length
        || DocumentKey::from_bytes(&payload) != *key
        || retained
            .sha1
            .as_deref()
            .is_some_and(|stored| sha1(&payload).as_slice() != stored)
    {
        return Err(crate::Error::DocumentUnavailable(key_string));
    }
    Ok(payload)
}

fn verification_before_read(metadata: &AcquisitionMetadata) -> &'static str {
    if metadata.expected_sha256.is_some() {
        "unavailable"
    } else {
        "not_requested"
    }
}

fn verification_after_digest(metadata: &AcquisitionMetadata) -> &'static str {
    if metadata.expected_sha256.is_some() {
        "verified"
    } else {
        "not_requested"
    }
}

fn positive_id<Id>(value: i64) -> crate::Result<Id>
where
    Id: TryFrom<i64>,
    <Id as TryFrom<i64>>::Error: std::fmt::Display,
{
    Id::try_from(value).map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
}

fn insert_fetch_attempt(
    conn: &mut diesel::SqliteConnection,
    metadata: &AcquisitionMetadata,
    attempt_key: &str,
    outcome: &str,
    verification_status: &str,
) -> crate::Result<FetchAttemptId> {
    let publisher_id = super::publishing_sources::lookup_id(conn, &metadata.source_key)?
        .ok_or_else(|| {
            crate::Error::SourceIdentityConflict(metadata.source_key.as_str().to_owned())
        })?;
    let row = sql_query(
        "INSERT INTO catalog_fetch_attempts \
         (publisher_id, attempt_key, uri, method, requested_at, responded_at, outcome, \
          verification_status, declared_filename) \
         VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, NULL, ?, ?, NULL) \
         RETURNING fetch_attempt_id AS id",
    )
    .bind::<diesel::sql_types::BigInt, _>(publisher_id.as_i64())
    .bind::<Text, _>(attempt_key)
    .bind::<Text, _>(metadata.source_uri.as_deref().unwrap_or_default())
    .bind::<Text, _>(metadata.method.as_deref().unwrap_or_default())
    .bind::<Text, _>(outcome)
    .bind::<Text, _>(verification_status)
    .get_result::<InsertedId>(conn)?;
    positive_id(row.id)
}

fn insert_fetch_headers(
    conn: &mut diesel::SqliteConnection,
    fetch_attempt_id: i64,
    headers: &[TransportHeader],
) -> crate::Result<()> {
    for (order, header) in headers.iter().enumerate() {
        let order = i64::try_from(order)
            .map_err(|_| crate::Error::DatabaseSchema("too many transport headers".into()))?;
        sql_query(
            "INSERT INTO catalog_fetch_headers \
             (fetch_attempt_id, list_order, name, value) VALUES (?, ?, ?, ?)",
        )
        .bind::<diesel::sql_types::BigInt, _>(fetch_attempt_id)
        .bind::<diesel::sql_types::BigInt, _>(order)
        .bind::<Text, _>(&header.name)
        .bind::<Text, _>(&header.value)
        .execute(conn)?;
    }
    Ok(())
}

fn insert_expected_sha256(
    conn: &mut diesel::SqliteConnection,
    fetch_attempt_id: i64,
    expected_sha256: Option<DocumentDigest>,
) -> crate::Result<()> {
    if let Some(digest) = expected_sha256 {
        sql_query(
            "INSERT INTO catalog_fetch_hashes \
             (fetch_attempt_id, list_order, algorithm, expected_bytes) \
             VALUES (?, 0, 'sha256', ?)",
        )
        .bind::<diesel::sql_types::BigInt, _>(fetch_attempt_id)
        .bind::<Binary, _>(digest.as_bytes().as_slice())
        .execute(conn)?;
    }
    Ok(())
}

fn object_store_for_pool(pool: &Pool) -> crate::Result<Utf8PathBuf> {
    let file = sql_query("PRAGMA database_list")
        .load::<DatabaseFile>(&mut pool.get()?)?
        .into_iter()
        .find(|database| !database.file.is_empty())
        .map(|database| database.file);
    if let Some(file) = file {
        return Ok(Utf8PathBuf::from(format!("{file}.documents")));
    }
    let path = pool.temporary_documents_path().ok_or_else(|| {
        crate::Error::InvalidPath("database has no external document store".to_owned())
    })?;
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .map_err(|path| crate::Error::InvalidPath(path.display().to_string()))
}

fn ensure_source_file(
    conn: &mut diesel::SqliteConnection,
    digest: &[u8],
    object_key: &str,
    sha1: &Sha1Digest,
    byte_length: i64,
) -> crate::Result<SourceFileId> {
    let existing = sql_query(
        "SELECT source_file_id, sha1, byte_length, object_key, codec \
         FROM catalog_source_files WHERE sha256 = ?",
    )
    .bind::<Binary, _>(digest)
    .load::<SourceFileRow>(conn)?;
    if existing.iter().any(|row| row.byte_length != byte_length) {
        return Err(crate::Error::DocumentDigestCollision);
    }
    if existing.len() > 1 {
        return Err(crate::Error::DocumentDigestCollision);
    }
    if let Some(row) = existing.into_iter().next() {
        if row.codec != "zstd"
            || row.object_key != object_key
            || row
                .sha1
                .as_deref()
                .is_some_and(|stored| stored != sha1.as_slice())
        {
            return Err(crate::Error::DocumentDigestCollision);
        }
        return positive_id(row.source_file_id);
    }
    let row = sql_query(
        "INSERT INTO catalog_source_files \
         (sha256, sha1, byte_length, object_key, codec) VALUES (?, ?, ?, ?, 'zstd') \
         RETURNING source_file_id AS id",
    )
    .bind::<Binary, _>(digest)
    .bind::<Binary, _>(sha1.as_slice())
    .bind::<diesel::sql_types::BigInt, _>(byte_length)
    .bind::<Text, _>(object_key)
    .get_result::<InsertedId>(conn)?;
    positive_id(row.id)
}

fn read_object(path: &Utf8Path, expected_length: usize) -> crate::Result<Vec<u8>> {
    let mut decoder = zstd::Decoder::new(File::open(path)?)?;
    let mut payload = Vec::with_capacity(expected_length.min(MAX_DOCUMENT_BYTES));
    decoder
        .by_ref()
        .take(
            u64::try_from(expected_length)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut payload)?;
    if payload.len() != expected_length {
        return Err(crate::Error::DocumentDigestCollision);
    }
    Ok(payload)
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

    use diesel::sql_types::BigInt;
    use flate2::{Compression, write::GzEncoder};
    use tempfile::TempDir;

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    const VALID_DAT: &[u8] =
        b"<datafile><header><name>Retention fixture</name></header></datafile>";

    #[test]
    fn retained_source_uses_catalog_ids_and_preserves_fetch_facts() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut metadata = acquisition("source-a");
        metadata.expected_sha256 = Some(DocumentDigest::from_bytes(VALID_DAT));
        metadata.transport_headers = vec![
            TransportHeader {
                name: "Warning".to_owned(),
                value: "first".to_owned(),
            },
            TransportHeader {
                name: "Warning".to_owned(),
                value: "second".to_owned(),
            },
        ];

        let retained = store.retain(&metadata, VALID_DAT)?;

        assert!(retained.source_file_id.as_i64() > 0);
        assert!(retained.file_receipt_id.as_i64() > 0);
        assert_eq!(store.load(&retained.document_key)?, VALID_DAT);
        let mut conn = store.pool.get()?;
        let source = sql_query(
            "SELECT sha256, sha1, byte_length, object_key, codec \
             FROM catalog_source_files WHERE source_file_id = ?",
        )
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .get_result::<RetainedPayload>(&mut conn)?;
        assert_eq!(source.sha256.as_slice(), retained.document_key.digest());
        assert_eq!(source.sha1.as_deref(), Some(sha1(VALID_DAT).as_slice()));
        assert_eq!(source.byte_length, i64::try_from(VALID_DAT.len())?);
        assert_eq!(source.codec, "zstd");
        assert!(store.object_store.join(source.object_key).is_file());

        let headers = sql_query(
            "SELECT name, value FROM catalog_fetch_headers \
             WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                 FROM catalog_file_receipts WHERE file_receipt_id = ?) \
             ORDER BY list_order",
        )
        .bind::<BigInt, _>(retained.file_receipt_id.as_i64())
        .load::<TransportHeaderRow>(&mut conn)?;
        assert_eq!(
            headers
                .into_iter()
                .map(|header| (header.name, header.value))
                .collect::<Vec<_>>(),
            vec![
                ("Warning".to_owned(), "first".to_owned()),
                ("Warning".to_owned(), "second".to_owned())
            ]
        );
        let expected_hash = sql_query(
            "SELECT algorithm, expected_bytes AS value FROM catalog_fetch_hashes \
             WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                 FROM catalog_file_receipts WHERE file_receipt_id = ?)",
        )
        .bind::<BigInt, _>(retained.file_receipt_id.as_i64())
        .get_result::<FetchHashRow>(&mut conn)?;
        assert_eq!(expected_hash.algorithm, "sha256");
        assert_eq!(
            expected_hash.value.as_slice(),
            metadata.expected_sha256.unwrap().as_bytes().as_slice()
        );
        Ok(())
    }

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    #[derive(QueryableByName)]
    struct ValueRow {
        #[diesel(sql_type = Text)]
        value: String,
    }

    #[derive(QueryableByName)]
    struct FetchHashRow {
        #[diesel(sql_type = Text)]
        algorithm: String,
        #[diesel(sql_type = Binary)]
        value: Vec<u8>,
    }

    #[derive(QueryableByName)]
    struct TransportHeaderRow {
        #[diesel(sql_type = Text)]
        name: String,
        #[diesel(sql_type = Text)]
        value: String,
    }

    #[derive(QueryableByName)]
    struct ColumnNameRow {
        #[diesel(sql_type = Text)]
        name: String,
    }

    fn setup_store() -> TestResult<(TempDir, DocumentStore)> {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("catalog.sqlite");
        let store = DocumentStore::open(&database_path.to_string_lossy())?;
        store.register_source(&PublishingSource::new("source-a", "Publisher A"))?;
        store.register_source(&PublishingSource::new("source-b", "Publisher B"))?;
        Ok((directory, store))
    }

    fn acquisition(source_key: &str) -> AcquisitionMetadata {
        AcquisitionMetadata {
            source_key: PublishingSourceKey::new(source_key),
            source_uri: Some(format!("https://example.invalid/{source_key}/catalog.dat")),
            method: Some("https".to_owned()),
            transport_headers: vec![TransportHeader {
                name: "content-type".to_owned(),
                value: "application/xml".to_owned(),
            }],
            expected_sha256: None,
        }
    }

    fn count(store: &DocumentStore, table: &str) -> TestResult<i64> {
        let table = match table {
            "documents" => "catalog_source_files",
            "acquisitions" => "catalog_file_receipts",
            "acquisition_attempts" => "catalog_fetch_attempts",
            _ => table,
        };
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
    fn memory_originals_live_until_the_last_pool_owner_drops() -> TestResult {
        let pool = create_db_pool(":memory:")?;
        let first = DocumentStore::from_pool(pool.clone())?;
        first.register_source(&PublishingSource::new("source-a", "Publisher A"))?;
        let retained = first.retain(&acquisition("source-a"), VALID_DAT)?;
        let root = first.object_store.clone();
        drop(first);
        assert!(root.is_dir());
        let second = DocumentStore::from_pool(pool.clone())?;
        assert_eq!(second.object_store, root);
        assert_eq!(second.load(&retained.document_key)?, VALID_DAT);
        drop(pool);
        assert!(root.is_dir());
        drop(second);
        assert!(!root.exists());
        Ok(())
    }

    #[test]
    fn fresh_store_registers_and_resolves_source_before_successful_retention() -> TestResult {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("fresh.sqlite");
        let store = DocumentStore::open(&database_path.to_string_lossy())?;
        store.register_source(&PublishingSource::new("fresh-source", "Fresh Publisher"))?;

        let source = store
            .resolve_source(&PublishingSourceKey::new("fresh-source"))?
            .ok_or("registered source did not resolve")?;
        assert_eq!(source.key().as_str(), "fresh-source");
        assert_eq!(source.display_name(), "Fresh Publisher");
        assert!(
            store
                .resolve_source(&PublishingSourceKey::new("missing-source"))?
                .is_none()
        );
        let retained = store.retain(&acquisition("fresh-source"), VALID_DAT)?;
        assert_eq!(store.load(&retained.document_key)?, VALID_DAT);
        let object_key = sql_query(
            "SELECT object_key AS value FROM catalog_source_files WHERE source_file_id = ?",
        )
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .get_result::<ValueRow>(&mut store.pool.get()?)?
        .value;
        assert!(object_key.starts_with("sha256/"));
        assert!(store.object_store.join(&object_key).is_file());
        let columns = sql_query("PRAGMA table_info(catalog_source_files)")
            .load::<ColumnNameRow>(&mut store.pool.get()?)?;
        assert!(!columns.iter().any(|column| column.name == "payload"));
        store.register_source(&PublishingSource::new("fresh-source", "Renamed Publisher"))?;
        let renamed = store
            .resolve_source(&PublishingSourceKey::new("fresh-source"))?
            .ok_or("renamed source did not resolve")?;
        assert_eq!(renamed.display_name(), "Renamed Publisher");
        assert_eq!(store.load(&retained.document_key)?, VALID_DAT);
        assert_eq!(count(&store, "acquisitions")?, 1);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
        Ok(())
    }

    #[test]
    fn acquisition_transport_headers_are_ordered_relational_values() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut metadata = acquisition("source-a");
        metadata.transport_headers.push(TransportHeader {
            name: "etag".to_owned(),
            value: "\"catalog-v1\"".to_owned(),
        });
        let retained = store.retain(&metadata, VALID_DAT)?;
        let mut conn = store.pool.get()?;
        let receipt_key = retained.acquisition_key.to_string();
        let headers = sql_query(
            "SELECT name, value FROM catalog_fetch_headers \
             WHERE fetch_attempt_id = (SELECT fetch_attempt_id FROM catalog_file_receipts \
                 WHERE receipt_key = ?) ORDER BY list_order",
        )
        .bind::<Text, _>(&receipt_key)
        .load::<TransportHeaderRow>(&mut conn)?;
        assert_eq!(
            headers
                .into_iter()
                .map(|header| (header.name, header.value))
                .collect::<Vec<_>>(),
            vec![
                ("content-type".to_owned(), "application/xml".to_owned()),
                ("etag".to_owned(), "\"catalog-v1\"".to_owned()),
            ]
        );
        let json_columns = sql_query(
            "SELECT COUNT(*) AS count FROM ( \
                SELECT name FROM pragma_table_info('catalog_fetch_attempts') \
                UNION ALL SELECT name FROM pragma_table_info('catalog_fetch_headers') \
             ) WHERE name LIKE '%json'",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(json_columns, 0);
        Ok(())
    }

    #[test]
    fn fresh_store_persists_failed_attempt_after_source_registration() -> TestResult {
        let directory = tempfile::tempdir()?;
        let database_path = directory.path().join("fresh-failure.sqlite");
        let store = DocumentStore::open(&database_path.to_string_lossy())?;
        store.register_source(&PublishingSource::new("fresh-source", "Fresh Publisher"))?;

        let mut failed = acquisition("fresh-source");
        failed.expected_sha256 = Some(DocumentDigest::from_hex(&"00".repeat(32))?);
        assert!(matches!(
            store.retain(&failed, VALID_DAT),
            Err(crate::Error::DocumentDigestMismatch)
        ));

        let mut conn = store.pool.get()?;
        let attempt = sql_query(
            "SELECT attempt.outcome, attempt.verification_status \
             FROM catalog_fetch_attempts AS attempt \
             JOIN catalog_publishers AS publisher USING (publisher_id) \
             WHERE publisher.publisher_key = 'fresh-source'",
        )
        .get_result::<FailedAttemptRow>(&mut conn)?;
        assert_eq!(attempt.outcome, "failed");
        assert_eq!(attempt.verification_status, "mismatch");
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisitions")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
        assert_eq!(count(&store, "catalog_fetch_headers")?, 1);
        Ok(())
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
                .header()?
                .name(),
            "Retention fixture"
        );
        let mut conn = store.pool.get()?;
        let status = sql_query(
            "SELECT verification_status FROM catalog_fetch_attempts \
                       WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                           FROM catalog_file_receipts WHERE receipt_key = ?)",
        )
        .bind::<Text, _>(first.acquisition_key.to_string())
        .get_result::<StatusRow>(&mut conn)?
        .verification_status;
        assert_eq!(status, "not_requested");
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
            br#"<!DOCTYPE datafile [<!ENTITY remote SYSTEM "http://127.0.0.1:9/catalog.dtd">]><datafile><header><name>&remote;</name></header></datafile>"#;
        assert!(store.retain(&acquisition("source-a"), external).is_err());
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisitions")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        let mut conn = store.pool.get()?;
        let failures = sql_query(
            "SELECT COUNT(*) AS count FROM catalog_fetch_attempts WHERE outcome = 'failed'",
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
        let serialized_key = retained.document_key.to_string();
        let restored_path = directory.path().join("restored.sqlite");
        let original_path = directory.path().join("catalog.sqlite");
        let original_objects = Utf8PathBuf::from(format!("{}.documents", original_path.display()));
        let restored_objects = Utf8PathBuf::from(format!("{}.documents", restored_path.display()));
        let object_key = format!("sha256/{}.zst", hex::encode(retained.document_key.digest()));
        drop(store);
        fs::copy(&original_path, &restored_path)?;
        let restored_object = restored_objects.join(&object_key);
        fs::create_dir_all(restored_object.parent().ok_or("object has no parent")?)?;
        fs::copy(original_objects.join(&object_key), restored_object)?;
        let restored = DocumentStore::open(&restored_path.to_string_lossy())?;
        let restored_key = serialized_key.parse()?;
        assert_eq!(restored.load(&restored_key)?, VALID_DAT);
        assert_eq!(count(&restored, "acquisitions")?, 1);
        assert_eq!(count(&restored, "acquisition_attempts")?, 1);
        Ok(())
    }

    #[test]
    fn oversized_stream_is_recorded_as_a_failed_attempt() -> TestResult {
        let (_directory, store) = setup_store()?;
        let oversized = io::Cursor::new(VALID_DAT);
        let result =
            store.retain_with_limit_and_validation(&acquisition("source-a"), oversized, 8, true);
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
        verified.expected_sha256 = Some(DocumentDigest::from_hex(
            "0cad48a4d2d1c1427572ab458f1ed2f927d58b85852ea0d4a4ef28e6bc554e88",
        )?);
        let retained = store.retain(&verified, VALID_DAT)?;
        let mut mismatched = acquisition("source-b");
        let mismatch_digest = DocumentDigest::from_hex(&"00".repeat(32))?;
        mismatched.expected_sha256 = Some(mismatch_digest);
        assert!(matches!(
            store.retain(&mismatched, VALID_DAT),
            Err(crate::Error::DocumentDigestMismatch)
        ));
        let mut conn = store.pool.get()?;
        let status = sql_query(
            "SELECT verification_status FROM catalog_fetch_attempts \
                       WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                           FROM catalog_file_receipts WHERE receipt_key = ?)",
        )
        .bind::<Text, _>(retained.acquisition_key.to_string())
        .get_result::<StatusRow>(&mut conn)?
        .verification_status;
        assert_eq!(status, "verified");
        let acquisition_digest = sql_query(
            "SELECT expected_bytes AS expected_sha256 FROM catalog_fetch_hashes \
                       WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                           FROM catalog_file_receipts WHERE receipt_key = ?)",
        )
        .bind::<Text, _>(retained.acquisition_key.to_string())
        .get_result::<ExpectedDigestRow>(&mut conn)?
        .expected_sha256;
        assert_eq!(
            acquisition_digest,
            Some(DocumentDigest::from_bytes(VALID_DAT).as_bytes().to_vec())
        );
        let failed_digest = sql_query(
            "SELECT expected_bytes AS expected_sha256 FROM catalog_fetch_hashes \
                       WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                           FROM catalog_fetch_attempts WHERE outcome = 'failed')",
        )
        .get_result::<ExpectedDigestRow>(&mut conn)?
        .expected_sha256;
        assert_eq!(failed_digest, Some(mismatch_digest.as_bytes().to_vec()));
        assert_eq!(count(&store, "documents")?, 1);
        assert_eq!(count(&store, "acquisitions")?, 1);
        assert_eq!(count(&store, "acquisition_attempts")?, 2);
        Ok(())
    }

    #[test]
    fn rejects_digest_mismatch_before_parsing_and_records_digest_failure() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut metadata = acquisition("source-a");
        metadata.expected_sha256 = Some(DocumentDigest::from_hex(
            "0cad48a4d2d1c1427572ab458f1ed2f927d58b85852ea0d4a4ef28e6bc554e88",
        )?);
        assert!(matches!(
            store.retain(&metadata, b"malformed XML".as_slice()),
            Err(crate::Error::DocumentDigestMismatch)
        ));
        let mut conn = store.pool.get()?;
        let status = sql_query(
            "SELECT verification_status FROM catalog_fetch_attempts WHERE outcome = 'failed'",
        )
        .get_result::<StatusRow>(&mut conn)?
        .verification_status;
        assert_eq!(status, "mismatch");
        assert_eq!(count(&store, "documents")?, 0);
        Ok(())
    }

    #[test]
    fn malformed_source_digest_is_rejected() {
        assert!(DocumentDigest::from_hex("not-a-sha256-digest").is_err());
        assert!(DocumentDigest::from_hex("00").is_err());
        assert!(DocumentDigest::from_hex(&"gg".repeat(32)).is_err());
        assert!("g".repeat(64).parse::<DocumentDigest>().is_err());
    }

    #[derive(QueryableByName)]
    struct FailedAttemptRow {
        #[diesel(sql_type = Text)]
        outcome: String,
        #[diesel(sql_type = Text)]
        verification_status: String,
    }

    #[test]
    fn retained_payload_and_acquisition_rows_cannot_be_replaced() -> TestResult {
        let (_directory, store) = setup_store()?;
        let retained = store.retain(&acquisition("source-a"), VALID_DAT)?;
        let mut conn = store.pool.get()?;
        assert!(
            sql_query(
                "UPDATE catalog_fetch_attempts SET uri = 'changed' \
                 WHERE fetch_attempt_id = (SELECT fetch_attempt_id \
                     FROM catalog_file_receipts WHERE file_receipt_id = ?)",
            )
            .bind::<BigInt, _>(retained.file_receipt_id.as_i64())
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
        let first_fetch_id = sql_query(
            "SELECT fetch_attempt_id AS id FROM catalog_file_receipts WHERE file_receipt_id = ?",
        )
        .bind::<BigInt, _>(first.file_receipt_id.as_i64())
        .get_result::<InsertedId>(&mut conn)?
        .id;
        let duplicate_receipt = sql_query(
            "INSERT INTO catalog_file_receipts (receipt_key, fetch_attempt_id, source_file_id) \
             VALUES ('duplicate-receipt', ?, ?)",
        )
        .bind::<BigInt, _>(first_fetch_id)
        .bind::<BigInt, _>(second.source_file_id.as_i64())
        .execute(&mut conn);
        assert!(duplicate_receipt.is_err());
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
                .header()?
                .name(),
            "Retention fixture"
        );
        Ok(())
    }

    #[test]
    fn retained_import_keeps_source_outside_sqlite() -> TestResult {
        let (_directory, store) = setup_store()?;
        let key = DocumentKey::from_bytes(VALID_DAT);
        let retained = store.retain(&acquisition("source-a"), VALID_DAT)?;
        assert_eq!(retained.document_key, key);
        assert_eq!(store.load(&key)?, VALID_DAT);
        assert_eq!(count(&store, "documents")?, 1);
        assert_eq!(count(&store, "acquisitions")?, 1);

        let mut conn = store.pool.get()?;
        let document = sql_query(
            "SELECT sha256, object_key AS value FROM catalog_source_files \
             WHERE source_file_id = ?",
        )
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .get_result::<DocumentObjectRow>(&mut conn)?;
        assert_eq!(document.sha256.as_slice(), key.digest());
        assert!(store.object_store.join(document.value).is_file());
        assert_eq!(store.load(&key)?, VALID_DAT);
        Ok(())
    }

    #[test]
    fn changed_sidecar_content_cannot_be_silently_replaced() -> TestResult {
        let (_directory, store) = setup_store()?;
        let key = DocumentKey::from_bytes(VALID_DAT);
        store.retain(&acquisition("source-a"), VALID_DAT)?;
        assert_eq!(store.load(&key)?, VALID_DAT);

        let object_path = store
            .object_store
            .join(format!("sha256/{}.zst", hex::encode(key.digest())));
        fs::write(&object_path, b"corrupt")?;
        assert!(store.retain(&acquisition("source-a"), VALID_DAT).is_err());
        Ok(())
    }

    #[test]
    fn legacy_sha1_and_length_mismatches_reject_retention() -> TestResult {
        let wrong_sha1 = [0_u8; 20];
        let correct_sha1 = sha1(VALID_DAT);
        let correct_length = i64::try_from(VALID_DAT.len())?;
        for (stored_sha1, stored_length) in [
            (&wrong_sha1, correct_length),
            (&correct_sha1, correct_length + 1),
        ] {
            let (_directory, store) = setup_store()?;
            let key = DocumentKey::from_bytes(VALID_DAT);
            let mut conn = store.pool.get()?;
            sql_query(
                "INSERT INTO catalog_source_files \
                       (sha256, sha1, byte_length, object_key, codec) \
                       VALUES (?, ?, ?, 'stale.zst', 'zstd')",
            )
            .bind::<Binary, _>(key.digest().as_slice())
            .bind::<Binary, _>(stored_sha1.as_slice())
            .bind::<BigInt, _>(stored_length)
            .execute(&mut conn)?;
            drop(conn);

            assert!(matches!(
                store.retain(&acquisition("source-a"), VALID_DAT),
                Err(crate::Error::DocumentDigestCollision)
            ));
            assert_eq!(count(&store, "acquisitions")?, 0);
        }
        Ok(())
    }

    #[test]
    fn sidecar_with_mismatched_sha1_metadata_is_rejected_on_load() -> TestResult {
        let (_directory, store) = setup_store()?;
        let key = DocumentKey::from_bytes(VALID_DAT);
        let incorrect_sha1 = [0_u8; 20];
        assert_ne!(incorrect_sha1, sha1(VALID_DAT));
        let mut conn = store.pool.get()?;
        let object_key = store.store_object(VALID_DAT, key)?;
        sql_query(
            "INSERT INTO catalog_source_files \
             (sha256, sha1, byte_length, object_key, codec) VALUES (?, ?, ?, ?, 'zstd')",
        )
        .bind::<Binary, _>(key.digest().as_slice())
        .bind::<Binary, _>(incorrect_sha1.as_slice())
        .bind::<BigInt, _>(i64::try_from(VALID_DAT.len())?)
        .bind::<Text, _>(object_key)
        .execute(&mut conn)?;
        drop(conn);

        assert!(matches!(
            store.load(&key),
            Err(crate::Error::DocumentUnavailable(_))
        ));
        Ok(())
    }

    #[test]
    fn sqlite_has_no_inline_source_payload_column() -> TestResult {
        let (_directory, store) = setup_store()?;
        let mut conn = store.pool.get()?;
        let columns = sql_query("PRAGMA table_info(catalog_source_files)")
            .load::<ColumnNameRow>(&mut conn)?;
        assert!(!columns.iter().any(|column| column.name == "payload"));
        Ok(())
    }

    #[derive(QueryableByName)]
    struct DocumentObjectRow {
        #[diesel(sql_type = Binary)]
        sha256: Vec<u8>,
        #[diesel(sql_type = Text)]
        value: String,
    }
}
