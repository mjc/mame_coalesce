use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{Binary, Nullable, Text},
};
use sha1::{Digest as _, Sha1};

use crate::{
    document_input::{self, MAX_DOCUMENT_BYTES},
    domain::{AcquisitionKey, DocumentDigest, DocumentKey, PublishingSource, PublishingSourceKey},
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
    MameListXml,
    MameListXmlGzip,
    MameSoftwareListXml,
    MameSoftwareListXmlGzip,
    ClrMameProText,
    ClrMameProTextGzip,
    NoIntroPcXml,
    NoIntroPcXmlGzip,
}

impl FormatHint {
    const fn as_str(self) -> &'static str {
        match self {
            Self::LogiqxXml => "logiqx+xml",
            Self::LogiqxXmlGzip => "logiqx+xml+gzip",
            Self::MameListXml => "mame-listxml",
            Self::MameListXmlGzip => "mame-listxml+gzip",
            Self::MameSoftwareListXml => "mame-softwarelist-xml",
            Self::MameSoftwareListXmlGzip => "mame-softwarelist-xml+gzip",
            Self::ClrMameProText => "clrmamepro-text",
            Self::ClrMameProTextGzip => "clrmamepro-text+gzip",
            Self::NoIntroPcXml => "no-intro-pc-xml",
            Self::NoIntroPcXmlGzip => "no-intro-pc-xml+gzip",
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
    object_store: Utf8PathBuf,
    temporary_object_store: bool,
    _database_guard: Option<crate::database::Database>,
}

#[derive(QueryableByName)]
struct ExistingDocument {
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Binary>)]
    sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    object_key: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<diesel::sql_types::BigInt>)]
    byte_length: Option<i64>,
    #[diesel(sql_type = Text)]
    retention_status: String,
}

#[derive(QueryableByName)]
struct RetainedPayload {
    #[diesel(sql_type = Binary)]
    sha256: Vec<u8>,
    #[diesel(sql_type = Binary)]
    sha1: Vec<u8>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Text)]
    object_key: String,
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
        let (object_store, temporary_object_store) = object_store_for_pool(&pool)?;
        Ok(Self {
            pool,
            object_store,
            temporary_object_store,
            _database_guard: None,
        })
    }

    pub fn open(database_url: &str) -> crate::Result<Self> {
        if database_url == ":memory:" {
            return Self::from_pool(create_db_pool(database_url)?);
        }
        let database = crate::database::Database::open(&camino::Utf8PathBuf::from(database_url))?;
        let pool = database.pool().clone();
        let (object_store, temporary_object_store) = object_store_for_pool(&pool)?;
        Ok(Self {
            pool,
            object_store,
            temporary_object_store,
            _database_guard: Some(database),
        })
    }

    pub fn register_source(&self, source: &PublishingSource) -> crate::Result<()> {
        let mut conn = self.pool.get()?;
        sql_query(
            "INSERT INTO publishing_sources (source_key, display_name) VALUES (?, ?) \
             ON CONFLICT (source_key) DO UPDATE SET display_name = excluded.display_name",
        )
        .bind::<Text, _>(source.key().as_str())
        .bind::<Text, _>(source.display_name())
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn resolve_source(
        &self,
        key: &PublishingSourceKey,
    ) -> crate::Result<Option<PublishingSource>> {
        let mut conn = self.pool.get()?;
        let source = sql_query(
            "SELECT source_key, display_name FROM publishing_sources WHERE source_key = ?",
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
        self.retain_with_options(metadata, reader, MAX_DOCUMENT_BYTES, false, None)
    }

    pub fn retain_path(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        // Catalog import performs format validation while parsing this same
        // retained byte sequence; a preliminary XML walk would traverse it twice.
        self.retain_path_with_options(source_key, path, false, None)
    }

    pub(crate) fn retain_path_mame(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false, Some(FormatHint::MameListXml))
    }

    pub(crate) fn retain_path_clrmamepro(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false, Some(FormatHint::ClrMameProText))
    }

    pub(crate) fn retain_path_mame_softwarelist(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(
            source_key,
            path,
            false,
            Some(FormatHint::MameSoftwareListXml),
        )
    }

    pub(crate) fn retain_path_no_intro_pc_xml(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
    ) -> crate::Result<RetainedDocument> {
        self.retain_path_with_options(source_key, path, false, Some(FormatHint::NoIntroPcXml))
    }

    fn retain_path_with_options(
        &self,
        source_key: PublishingSourceKey,
        path: &Utf8Path,
        validate_xml: bool,
        requested_format: Option<FormatHint>,
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
            Ok(file) => self.retain_with_options(
                &metadata,
                file,
                MAX_DOCUMENT_BYTES,
                validate_xml,
                requested_format,
            ),
            Err(error) => {
                self.record_failed_attempt(&metadata, "io", &error.to_string())?;
                Err(error.into())
            }
        }
    }

    pub fn load(&self, key: &DocumentKey) -> crate::Result<Vec<u8>> {
        let mut conn = self.pool.get()?;
        let key_string = key.to_string();
        let retained = sql_query(
            "SELECT sha256, sha1, byte_length, object_key FROM documents \
             WHERE document_key = ? AND retention_status = 'retained'",
        )
        .bind::<Text, _>(&key_string)
        .get_result::<RetainedPayload>(&mut conn)
        .optional()?
        .ok_or_else(|| crate::Error::DocumentUnavailable(key_string.clone()))?;
        if retained.sha256.as_slice() != key.digest() {
            return Err(crate::Error::DocumentDigestCollision);
        }
        let expected_length = usize::try_from(retained.byte_length)
            .map_err(|_| crate::Error::DocumentUnavailable(key_string.clone()))?;
        let object_path = self.object_store.join(&retained.object_key);
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
            || sha1(&payload).as_slice() != retained.sha1
        {
            return Err(crate::Error::DocumentUnavailable(key_string));
        }
        Ok(payload)
    }

    fn retain_with_limit_and_validation<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
        limit: usize,
        validate_xml: bool,
    ) -> crate::Result<RetainedDocument> {
        self.retain_with_options(metadata, reader, limit, validate_xml, None)
    }

    fn retain_with_options<R: Read>(
        &self,
        metadata: &AcquisitionMetadata,
        reader: R,
        limit: usize,
        validate_xml: bool,
        requested_format: Option<FormatHint>,
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
        if validate_xml && let Err(error) = DataFile::validate_document_bytes(&raw) {
            self.record_failed_attempt(metadata, error_code(&error), &error.to_string())?;
            return Err(error);
        }

        let result = self.persist_retained(
            metadata,
            transport_metadata.as_deref(),
            &raw,
            key,
            limit,
            requested_format,
        );
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
        requested_format: Option<FormatHint>,
    ) -> crate::Result<RetainedDocument> {
        let verification_status = if metadata.expected_sha256.is_some() {
            "verified"
        } else {
            "unverified"
        };
        let sha1 = sha1(raw);
        let byte_length =
            i64::try_from(raw.len()).map_err(|_| crate::Error::DocumentTooLarge { limit })?;
        let format_hint = requested_format.map_or_else(
            || FormatHint::for_bytes(raw),
            |hint| match (hint, raw.starts_with(&[0x1f, 0x8b])) {
                (FormatHint::MameListXml | FormatHint::MameListXmlGzip, false) => {
                    FormatHint::MameListXml
                }
                (FormatHint::MameListXml | FormatHint::MameListXmlGzip, true) => {
                    FormatHint::MameListXmlGzip
                }
                (FormatHint::MameSoftwareListXml | FormatHint::MameSoftwareListXmlGzip, false) => {
                    FormatHint::MameSoftwareListXml
                }
                (FormatHint::MameSoftwareListXml | FormatHint::MameSoftwareListXmlGzip, true) => {
                    FormatHint::MameSoftwareListXmlGzip
                }
                (FormatHint::ClrMameProText | FormatHint::ClrMameProTextGzip, false) => {
                    FormatHint::ClrMameProText
                }
                (FormatHint::ClrMameProText | FormatHint::ClrMameProTextGzip, true) => {
                    FormatHint::ClrMameProTextGzip
                }
                (FormatHint::NoIntroPcXml | FormatHint::NoIntroPcXmlGzip, false) => {
                    FormatHint::NoIntroPcXml
                }
                (FormatHint::NoIntroPcXml | FormatHint::NoIntroPcXmlGzip, true) => {
                    FormatHint::NoIntroPcXmlGzip
                }
                (_, false) => FormatHint::LogiqxXml,
                (_, true) => FormatHint::LogiqxXmlGzip,
            },
        );
        let acquisition_key = AcquisitionKey::fresh();
        let attempt_key = uuid::Uuid::new_v4().to_string();
        let acquisition_key_string = acquisition_key.to_string();
        let document_key_string = key.to_string();
        let object_key = self.store_object(raw, key)?;
        {
            let mut conn = self.pool.get()?;
            conn.immediate_transaction::<_, crate::Error, _>(|conn| {
                ensure_document_retained(
                    conn,
                    key.digest().as_slice(),
                    &document_key_string,
                    &object_key,
                    &sha1,
                    byte_length,
                    format_hint,
                )?;

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

fn object_store_for_pool(pool: &Pool) -> crate::Result<(Utf8PathBuf, bool)> {
    let file = sql_query("PRAGMA database_list")
        .load::<DatabaseFile>(&mut pool.get()?)?
        .into_iter()
        .find(|database| !database.file.is_empty())
        .map(|database| database.file);
    if let Some(file) = file {
        return Ok((Utf8PathBuf::from(format!("{file}.documents")), false));
    }
    let path = std::env::temp_dir().join(format!(
        "mame-coalesce-{}-{}.documents",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    Ok((
        Utf8PathBuf::from_path_buf(path)
            .map_err(|path| crate::Error::InvalidPath(path.display().to_string()))?,
        true,
    ))
}

impl Drop for DocumentStore {
    fn drop(&mut self) {
        if self.temporary_object_store {
            let _ = fs::remove_dir_all(&self.object_store);
        }
    }
}

fn ensure_document_retained(
    conn: &mut diesel::SqliteConnection,
    digest: &[u8],
    document_key: &str,
    object_key: &str,
    sha1: &Sha1Digest,
    byte_length: i64,
    format_hint: FormatHint,
) -> crate::Result<()> {
    let existing = sql_query(
        "SELECT document_key, sha256, object_key, retention_status, sha1, byte_length FROM documents \
         WHERE sha256 = ? OR document_key = ?",
    )
    .bind::<Binary, _>(digest)
    .bind::<Text, _>(document_key)
    .load::<ExistingDocument>(conn)?;
    if existing.len() > 1 {
        return Err(crate::Error::DocumentDigestCollision);
    }
    match existing.into_iter().next() {
        Some(document)
            if document.document_key == document_key
                && document.retention_status == "retained"
                && document.sha256.as_deref() == Some(digest)
                && document.object_key.as_deref() == Some(object_key) => {}
        Some(document)
            if document.document_key == document_key
                && document.retention_status == "unavailable"
                && document.sha256.is_none()
                && document.object_key.is_none() =>
        {
            if document
                .sha1
                .as_deref()
                .is_some_and(|stored| stored != sha1)
                || document
                    .byte_length
                    .is_some_and(|stored| stored != byte_length)
            {
                return Err(crate::Error::DocumentDigestCollision);
            }
            sql_query(
                "UPDATE documents SET sha256 = ?, sha1 = ?, byte_length = ?, object_key = ?, \
                 retention_status = 'retained', format_hint = ? WHERE document_key = ?",
            )
            .bind::<Binary, _>(digest)
            .bind::<Binary, _>(sha1.as_slice())
            .bind::<diesel::sql_types::BigInt, _>(byte_length)
            .bind::<Text, _>(object_key)
            .bind::<Text, _>(format_hint.as_str())
            .bind::<Text, _>(document_key)
            .execute(conn)?;
        }
        Some(_) => return Err(crate::Error::DocumentDigestCollision),
        None => {
            sql_query(
                "INSERT INTO documents \
                 (document_key, sha1, byte_length, sha256, object_key, format_hint, retention_status) \
                 VALUES (?, ?, ?, ?, ?, ?, 'retained')",
            )
            .bind::<Text, _>(document_key)
            .bind::<Binary, _>(sha1.as_slice())
            .bind::<diesel::sql_types::BigInt, _>(byte_length)
            .bind::<Binary, _>(digest)
            .bind::<Text, _>(object_key)
            .bind::<Text, _>(format_hint.as_str())
            .execute(conn)?;
        }
    }
    Ok(())
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

    #[derive(QueryableByName)]
    struct ValueRow {
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
        let object_key =
            sql_query("SELECT object_key AS value FROM documents WHERE document_key = ?")
                .bind::<Text, _>(retained.document_key.to_string())
                .get_result::<ValueRow>(&mut store.pool.get()?)?
                .value;
        assert!(object_key.starts_with("sha256/"));
        assert!(store.object_store.join(&object_key).is_file());
        let columns = sql_query("PRAGMA table_info(documents)")
            .load::<ColumnNameRow>(&mut store.pool.get()?)?;
        assert!(!columns.iter().any(|column| column.name == "payload"));
        assert_eq!(count(&store, "acquisitions")?, 1);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
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
            "SELECT outcome, diagnostic FROM acquisition_attempts WHERE source_key = 'fresh-source'",
        )
        .get_result::<FailedAttemptRow>(&mut conn)?;
        assert_eq!(attempt.outcome, "failed");
        assert!(
            attempt
                .diagnostic
                .as_deref()
                .is_some_and(|diagnostic| diagnostic.starts_with("digest_mismatch:"))
        );
        assert_eq!(count(&store, "documents")?, 0);
        assert_eq!(count(&store, "acquisitions")?, 0);
        assert_eq!(count(&store, "acquisition_attempts")?, 1);
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
            br#"<!DOCTYPE datafile [<!ENTITY remote SYSTEM "http://127.0.0.1:9/catalog.dtd">]><datafile><header><name>&remote;</name></header></datafile>"#;
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
        let serialized_key = retained.document_key.to_string();
        let backup_path = directory.path().join("backup.sqlite");
        let restored_path = directory.path().join("restored.sqlite");
        let original_path = directory.path().join("catalog.sqlite");
        drop(store);
        let original_path = Utf8Path::from_path(&original_path).ok_or("non-UTF-8 path")?;
        let backup_path = Utf8Path::from_path(&backup_path).ok_or("non-UTF-8 path")?;
        let restored_path = Utf8Path::from_path(&restored_path).ok_or("non-UTF-8 path")?;
        crate::storage::backup::create_backup(original_path, backup_path)?;
        crate::storage::backup::restore_backup(
            backup_path,
            restored_path,
            crate::storage::backup::RestorePolicy::CreateNew,
        )?;
        let restored = DocumentStore::open(restored_path.as_str())?;
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
        let diagnostic = sql_query("SELECT diagnostic FROM acquisition_attempts")
            .get_result::<DiagnosticRow>(&mut conn)?
            .diagnostic;
        assert!(diagnostic.starts_with("digest_mismatch:"));
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
    struct DiagnosticRow {
        #[diesel(sql_type = Text)]
        diagnostic: String,
    }

    #[derive(QueryableByName)]
    struct FailedAttemptRow {
        #[diesel(sql_type = Text)]
        outcome: String,
        #[diesel(sql_type = Nullable<Text>)]
        diagnostic: Option<String>,
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
        let retain_source_documents_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000001_retain_source_documents"
            })
            .ok_or("retain_source_documents migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..retain_source_documents_index] {
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
            "SELECT retention_status FROM documents WHERE document_key = 'metadata-only'",
        )
        .get_result::<DocumentStatusRow>(&mut conn)?;
        assert_eq!(document.retention_status, "unavailable");
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
    fn retained_import_keeps_source_outside_sqlite() -> TestResult {
        let (_directory, store) = setup_store()?;
        let key = DocumentKey::from_bytes(VALID_DAT);
        let mut conn = store.pool.get()?;
        sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES (?, ?, ?)")
            .bind::<Text, _>(key.to_string())
            .bind::<Binary, _>(sha1(VALID_DAT).as_slice())
            .bind::<BigInt, _>(i64::try_from(VALID_DAT.len())?)
            .execute(&mut conn)?;
        sql_query(
            "INSERT INTO acquisitions (acquisition_key, source_key, document_key, method) \
             VALUES ('legacy-content-key', 'source-a', ?, 'legacy-import')",
        )
        .bind::<Text, _>(key.to_string())
        .execute(&mut conn)?;
        drop(conn);

        let retained = store.retain(&acquisition("source-a"), VALID_DAT)?;
        assert_eq!(retained.document_key, key);
        assert_eq!(store.load(&key)?, VALID_DAT);
        assert_eq!(count(&store, "documents")?, 1);
        assert_eq!(count(&store, "acquisitions")?, 2);

        let mut conn = store.pool.get()?;
        let document = sql_query(
            "SELECT retention_status, sha256, object_key AS value FROM documents WHERE document_key = ?",
        )
        .bind::<Text, _>(key.to_string())
        .get_result::<DocumentObjectRow>(&mut conn)?;
        assert_eq!(document.retention_status, "retained");
        assert_eq!(document.sha256.as_deref(), Some(key.digest().as_slice()));
        assert!(store.object_store.join(document.value).is_file());
        assert_eq!(store.load(&key)?, VALID_DAT);
        Ok(())
    }

    #[test]
    fn changed_sidecar_content_cannot_be_silently_replaced() -> TestResult {
        let (_directory, store) = setup_store()?;
        let key = DocumentKey::from_bytes(VALID_DAT);
        let mut conn = store.pool.get()?;
        sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES (?, ?, ?)")
            .bind::<Text, _>(key.to_string())
            .bind::<Binary, _>(sha1(VALID_DAT).as_slice())
            .bind::<BigInt, _>(i64::try_from(VALID_DAT.len())?)
            .execute(&mut conn)?;
        drop(conn);

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
            sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES (?, ?, ?)")
                .bind::<Text, _>(key.to_string())
                .bind::<Binary, _>(stored_sha1.as_slice())
                .bind::<BigInt, _>(stored_length)
                .execute(&mut conn)?;
            drop(conn);

            assert!(matches!(
                store.retain(&acquisition("source-a"), VALID_DAT),
                Err(crate::Error::DocumentDigestCollision)
            ));
            assert!(matches!(
                store.load(&key),
                Err(crate::Error::DocumentUnavailable(_))
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
        sql_query("INSERT INTO documents (document_key, sha1, byte_length) VALUES (?, ?, ?)")
            .bind::<Text, _>(key.to_string())
            .bind::<Binary, _>(incorrect_sha1.as_slice())
            .bind::<BigInt, _>(i64::try_from(VALID_DAT.len())?)
            .execute(&mut conn)?;
        let object_key = store.store_object(VALID_DAT, key)?;
        sql_query(
            "UPDATE documents SET sha256 = ?, object_key = ?, retention_status = 'retained' \
             WHERE document_key = ?",
        )
        .bind::<Binary, _>(key.digest().as_slice())
        .bind::<Text, _>(object_key)
        .bind::<Text, _>(key.to_string())
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
        let columns = sql_query("PRAGMA table_info(documents)").load::<ColumnNameRow>(&mut conn)?;
        assert!(!columns.iter().any(|column| column.name == "payload"));
        Ok(())
    }

    #[test]
    fn down_migrations_remove_software_list_snapshot_and_document_retention_extensions()
    -> TestResult {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = crate::storage::db::MIGRATIONS.migrations()?;
        let document_retention_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000001_retain_source_documents"
            })
            .ok_or("document retention migration not found")?;
        let legacy_retention_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-25-000000_allow_legacy_document_retention"
            })
            .ok_or("legacy document retention migration not found")?;
        let snapshot_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000002_publish_logiqx_snapshots"
            })
            .ok_or("snapshot migration not found")?;
        let machine_asset_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000003_mame_machine_asset_semantics"
            })
            .ok_or("machine asset migration not found")?;
        let software_list_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000004_mame_softwarelist_semantics"
            })
            .ok_or("software-list migration not found")?;
        assert!(document_retention_index < legacy_retention_index);
        conn.applied_migrations()?;
        for migration in &migrations[..=legacy_retention_index] {
            conn.run_migration(migration.as_ref())?;
        }
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

        // Keep the legacy-retention migration applied for its explicit rollback
        // assertion below.
        let rollback_indexes = (legacy_retention_index + 1..migrations.len())
            .rev()
            .chain((software_list_index..legacy_retention_index).rev());
        for index in rollback_indexes {
            conn.revert_migration(migrations[index].as_ref())?;
        }
        let software_list_tables = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master \
             WHERE type = 'table' AND name IN ('software_lists', 'software_items', \
                 'software_parts', 'software_areas', 'software_components', \
                 'software_item_dependencies')",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(software_list_tables, 0);

        conn.revert_migration(migrations[machine_asset_index].as_ref())?;
        let snapshot_tables_after_asset_revert = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master \
             WHERE type = 'table' AND name IN ('snapshot_publications', 'snapshot_sets', 'asset_requirements', \
                 'snapshot_extensions', 'import_diagnostics')",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(snapshot_tables_after_asset_revert, 5);
        conn.revert_migration(migrations[snapshot_index].as_ref())?;
        let snapshot_tables = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master \
             WHERE type = 'table' AND name IN ('snapshot_publications', 'snapshot_sets', 'asset_requirements', \
                 'snapshot_extensions', 'import_diagnostics')",
        )
        .get_result::<CountRow>(&mut conn)?
        .count;
        assert_eq!(snapshot_tables, 0);
        conn.revert_migration(migrations[legacy_retention_index].as_ref())?;
        conn.revert_migration(migrations[document_retention_index].as_ref())?;
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
    }

    #[derive(QueryableByName)]
    struct DocumentObjectRow {
        #[diesel(sql_type = Text)]
        retention_status: String,
        #[diesel(sql_type = Nullable<Binary>)]
        sha256: Option<Vec<u8>>,
        #[diesel(sql_type = Text)]
        value: String,
    }
}
