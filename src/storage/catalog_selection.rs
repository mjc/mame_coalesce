//! Resolve source-free catalog selectors to one immutable published edition.

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use thiserror::Error;

use super::catalog_ids::{
    CatalogId, CatalogIdError, CoverageId, EditionId, FetchAttemptId, FileReceiptId, PublisherId,
    ReadingRulesId, SourceFileId,
};
use crate::{
    domain::{CatalogKey, CatalogScope},
    storage::{
        catalog_coverage,
        reading_rules::{self, ReadingRulesSpec},
    },
};

/// Select a published catalog edition by its stable catalog, edition, or source identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogSelection<'a> {
    /// Select the latest published edition of this exact catalog identity.
    Latest { catalog_id: CatalogId },
    /// Pin one edition directly. The edition must already be published.
    Edition(EditionId),
    /// Resolve exact catalog key, retained source URI/path, then published names.
    ByNameOrSource(&'a str),
}

/// Failure to resolve or read one pinned catalog edition.
#[derive(Debug, Error)]
pub enum CatalogSelectionError {
    /// No published catalog edition matches the supplied selector.
    #[error("no published catalog edition matches {selector:?}")]
    NotFound { selector: String },
    /// More than one distinct catalog matches a display or header name.
    #[error("catalog selector {selector:?} matches multiple catalogs: {catalog_ids:?}")]
    Ambiguous {
        selector: String,
        catalog_ids: Vec<CatalogId>,
    },
    /// A stored identifier or metadata value violates its typed representation.
    #[error("invalid catalog metadata: {0}")]
    InvalidMetadata(String),
    /// A retained source path could not be resolved to an absolute UTF-8 path.
    #[error("invalid catalog source path: {0}")]
    InvalidSourcePath(String),
    /// A stored catalog identifier is not a positive SQLite ID.
    #[error(transparent)]
    InvalidId(#[from] CatalogIdError),
    /// A database query failed.
    #[error(transparent)]
    Database(#[from] diesel::result::Error),
    /// Canonical coverage or interpretation metadata is invalid.
    #[error(transparent)]
    Catalog(#[from] crate::Error),
}

/// The selected publication and metadata reached through its canonical owners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedCatalogEdition {
    pub publisher_id: PublisherId,
    pub publisher_key: String,
    pub publisher_display_name: String,
    pub publisher_locator: Option<String>,
    pub catalog_id: CatalogId,
    pub catalog_key: String,
    pub catalog_display_name: String,
    pub edition_id: EditionId,
    pub published_at: String,
    pub source_file_id: SourceFileId,
    pub source: CatalogSourceMetadata,
    pub reading_rules_id: ReadingRulesId,
    pub reading_rules: ReadingRulesSpec,
    pub coverage_id: CoverageId,
    pub coverage: CatalogScope,
    pub file_receipt_id: Option<FileReceiptId>,
    pub receipt: Option<CatalogReceiptMetadata>,
    pub previous_edition_id: Option<EditionId>,
}

/// Metadata for the retained source file, not its current acquisition attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSourceMetadata {
    pub sha256: [u8; 32],
    pub sha1: Option<[u8; 20]>,
    pub byte_length: u64,
    pub object_key: String,
    pub codec: String,
}

/// Acquisition evidence attached to an edition's immutable file receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogReceiptMetadata {
    pub receipt_key: String,
    pub fetch_attempt_id: FetchAttemptId,
    pub source_uri: String,
    pub method: String,
    pub requested_at: String,
    pub responded_at: Option<String>,
    pub declared_filename: Option<String>,
    pub verification_status: String,
}

/// Resolve a selector and load the selected edition's source-free metadata.
///
/// Resolution and metadata loading share a read transaction. Once returned, the
/// typed edition ID pins all later workflow reads; this function is not called
/// again to refresh “latest”.
pub fn select_catalog_edition(
    connection: &mut SqliteConnection,
    selection: CatalogSelection<'_>,
) -> Result<SelectedCatalogEdition, CatalogSelectionError> {
    connection.transaction(|connection| {
        let edition_id = match selection {
            CatalogSelection::Latest { catalog_id } => latest_edition_id(connection, catalog_id)?
                .ok_or_else(|| {
                CatalogSelectionError::NotFound {
                    selector: catalog_id.as_i64().to_string(),
                }
            })?,
            CatalogSelection::Edition(edition_id) => edition_id,
            CatalogSelection::ByNameOrSource(value) => {
                let catalog_id = resolve_catalog_id(connection, value)?;
                latest_edition_id(connection, catalog_id)?.ok_or_else(|| {
                    CatalogSelectionError::NotFound {
                        selector: value.to_owned(),
                    }
                })?
            }
        };
        load_published_edition(connection, edition_id)
    })
}

#[derive(QueryableByName)]
struct CatalogIdRow {
    #[diesel(sql_type = BigInt)]
    catalog_id: i64,
    #[diesel(sql_type = Text)]
    catalog_key: String,
}

fn resolve_catalog_id(
    connection: &mut SqliteConnection,
    value: &str,
) -> Result<CatalogId, CatalogSelectionError> {
    if let Some(row) = sql_query("SELECT catalog_id,catalog_key FROM catalogs WHERE catalog_key=?")
        .bind::<Text, _>(value)
        .get_result::<CatalogIdRow>(connection)
        .optional()?
    {
        return checked_id(row.catalog_id);
    }

    let normalized_path = retained_source_path(Utf8Path::new(value))?;
    let source_rows = sql_query(
        "SELECT DISTINCT catalog.catalog_id,catalog.catalog_key \
         FROM catalogs AS catalog \
         JOIN catalog_editions AS edition USING(catalog_id) \
         JOIN published_catalog_editions AS publication \
           ON publication.edition_id=edition.edition_id \
          AND publication.catalog_id=edition.catalog_id \
          AND publication.source_file_id=edition.source_file_id \
          AND publication.reading_rules_id=edition.reading_rules_id \
          AND publication.coverage_id=edition.coverage_id \
         JOIN catalog_file_receipts AS receipt \
           ON receipt.source_file_id=edition.source_file_id \
         JOIN catalog_fetch_attempts AS attempt \
           ON attempt.fetch_attempt_id=receipt.fetch_attempt_id \
          AND attempt.publisher_id=catalog.publisher_id \
         WHERE attempt.outcome='retained' AND attempt.uri IN (?,?) \
         ORDER BY catalog.catalog_id",
    )
    .bind::<Text, _>(value)
    .bind::<Text, _>(normalized_path.as_str())
    .load::<CatalogIdRow>(connection)?;

    if !source_rows.is_empty() {
        let local_key = CatalogKey::for_local_dat(&normalized_path);
        if let Some(local) = source_rows
            .iter()
            .find(|row| row.catalog_key == local_key.as_str())
        {
            return checked_id(local.catalog_id);
        }
        let ids = source_rows
            .into_iter()
            .map(|row| checked_id(row.catalog_id))
            .collect::<Result<Vec<_>, _>>()?;
        return unique_catalog(value, ids);
    }

    let name_rows = sql_query(
        "WITH latest AS ( \
             SELECT publication.edition_id,publication.catalog_id \
             FROM published_catalog_editions AS publication \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM published_catalog_editions AS newer \
                 WHERE newer.catalog_id=publication.catalog_id \
                   AND (newer.published_at>publication.published_at \
                     OR (newer.published_at=publication.published_at \
                       AND newer.edition_id>publication.edition_id)) \
             ) \
         ) \
         SELECT catalog.catalog_id,catalog.catalog_key \
         FROM catalogs AS catalog \
         WHERE catalog.display_name=? \
           AND EXISTS (SELECT 1 FROM published_catalog_editions AS publication \
                       WHERE publication.catalog_id=catalog.catalog_id) \
         UNION \
         SELECT latest.catalog_id,catalog.catalog_key \
         FROM latest \
         JOIN catalog_editions AS edition \
           ON edition.edition_id=latest.edition_id \
          AND edition.catalog_id=latest.catalog_id \
         JOIN catalog_set_groups AS root \
           ON root.edition_id=edition.edition_id AND root.group_kind='root' \
         JOIN logiqx_headers AS header USING(set_group_id) \
         JOIN logiqx_header_text_elements AS header_name \
           ON header_name.header_id=header.header_id \
         JOIN catalogs AS catalog ON catalog.catalog_id=latest.catalog_id \
         WHERE header_name.field_kind=0 AND header_name.text_value=? \
         UNION \
         SELECT latest.catalog_id,catalog.catalog_key \
         FROM latest \
         JOIN catalog_editions AS edition \
           ON edition.edition_id=latest.edition_id \
          AND edition.catalog_id=latest.catalog_id \
         JOIN catalog_set_groups AS root \
           ON root.edition_id=edition.edition_id AND root.group_kind='root' \
         JOIN no_intro_dat_headers AS header USING(set_group_id) \
         JOIN no_intro_dat_header_text_children AS header_name \
           ON header_name.header_id=header.header_id \
         JOIN catalogs AS catalog ON catalog.catalog_id=latest.catalog_id \
         WHERE header_name.field_kind='name' AND header_name.value_text=? \
         ORDER BY catalog_id",
    )
    .bind::<Text, _>(value)
    .bind::<Text, _>(value)
    .bind::<Text, _>(value)
    .load::<CatalogIdRow>(connection)?;
    let ids = name_rows
        .into_iter()
        .map(|row| checked_id(row.catalog_id))
        .collect::<Result<Vec<_>, _>>()?;
    unique_catalog(value, ids)
}

fn unique_catalog(
    selector: &str,
    mut catalog_ids: Vec<CatalogId>,
) -> Result<CatalogId, CatalogSelectionError> {
    catalog_ids.sort_unstable();
    catalog_ids.dedup();
    if let Some(catalog_id) = catalog_ids.first().copied() {
        if catalog_ids.len() == 1 {
            return Ok(catalog_id);
        }
        return Err(CatalogSelectionError::Ambiguous {
            selector: selector.to_owned(),
            catalog_ids,
        });
    }
    Err(CatalogSelectionError::NotFound {
        selector: selector.to_owned(),
    })
}

#[derive(QueryableByName)]
struct EditionIdRow {
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
}

fn latest_edition_id(
    connection: &mut SqliteConnection,
    catalog_id: CatalogId,
) -> Result<Option<EditionId>, CatalogSelectionError> {
    let row = sql_query(
        "SELECT edition_id FROM published_catalog_editions \
         WHERE catalog_id=? ORDER BY published_at DESC,edition_id DESC LIMIT 1",
    )
    .bind::<BigInt, _>(catalog_id.as_i64())
    .get_result::<EditionIdRow>(connection)
    .optional()?;
    row.map(|row| checked_id(row.edition_id)).transpose()
}

#[derive(QueryableByName)]
struct EditionMetadataRow {
    #[diesel(sql_type = BigInt)]
    publisher_id: i64,
    #[diesel(sql_type = Text)]
    publisher_key: String,
    #[diesel(sql_type = Text)]
    publisher_display_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    publisher_locator: Option<String>,
    #[diesel(sql_type = BigInt)]
    catalog_id: i64,
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    catalog_display_name: String,
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
    #[diesel(sql_type = Text)]
    published_at: String,
    #[diesel(sql_type = BigInt)]
    source_file_id: i64,
    #[diesel(sql_type = Binary)]
    source_sha256: Vec<u8>,
    #[diesel(sql_type = Nullable<Binary>)]
    source_sha1: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    source_byte_length: i64,
    #[diesel(sql_type = Text)]
    source_object_key: String,
    #[diesel(sql_type = Text)]
    source_codec: String,
    #[diesel(sql_type = BigInt)]
    reading_rules_id: i64,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    file_receipt_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    receipt_source_file_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    receipt_key: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    fetch_attempt_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    fetch_publisher_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_uri: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    fetch_method: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    requested_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    responded_at: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    declared_filename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    verification_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    fetch_outcome: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    previous_edition_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    joined_previous_edition_id: Option<i64>,
}

fn load_published_edition(
    connection: &mut SqliteConnection,
    edition_id: EditionId,
) -> Result<SelectedCatalogEdition, CatalogSelectionError> {
    let row = sql_query(
        "SELECT publisher.publisher_id,publisher.publisher_key, \
                publisher.display_name AS publisher_display_name,publisher.locator AS publisher_locator, \
                catalog.catalog_id,catalog.catalog_key,catalog.display_name AS catalog_display_name, \
                publication.edition_id,publication.published_at, \
                source.source_file_id,source.sha256 AS source_sha256,source.sha1 AS source_sha1, \
                source.byte_length AS source_byte_length,source.object_key AS source_object_key, \
                source.codec AS source_codec,edition.reading_rules_id,edition.coverage_id, \
                edition.file_receipt_id,receipt.source_file_id AS receipt_source_file_id, \
                receipt.receipt_key,attempt.fetch_attempt_id, \
                attempt.publisher_id AS fetch_publisher_id,attempt.uri AS source_uri, \
                attempt.method AS fetch_method,attempt.requested_at,attempt.responded_at, \
                attempt.declared_filename,attempt.verification_status, \
                attempt.outcome AS fetch_outcome,edition.previous_edition_id, \
                previous.edition_id AS joined_previous_edition_id \
         FROM published_catalog_editions AS publication \
         JOIN catalog_editions AS edition \
           ON edition.edition_id=publication.edition_id \
          AND edition.catalog_id=publication.catalog_id \
          AND edition.source_file_id=publication.source_file_id \
          AND edition.reading_rules_id=publication.reading_rules_id \
          AND edition.coverage_id=publication.coverage_id \
         JOIN catalogs AS catalog ON catalog.catalog_id=edition.catalog_id \
         JOIN catalog_publishers AS publisher ON publisher.publisher_id=catalog.publisher_id \
         JOIN catalog_source_files AS source ON source.source_file_id=edition.source_file_id \
         LEFT JOIN catalog_file_receipts AS receipt \
           ON receipt.file_receipt_id=edition.file_receipt_id \
         LEFT JOIN catalog_fetch_attempts AS attempt \
           ON attempt.fetch_attempt_id=receipt.fetch_attempt_id \
          AND attempt.publisher_id=catalog.publisher_id \
         LEFT JOIN catalog_editions AS previous \
           ON previous.edition_id=edition.previous_edition_id \
          AND previous.catalog_id=edition.catalog_id \
         WHERE publication.edition_id=?",
    )
    .bind::<BigInt, _>(edition_id.as_i64())
    .get_result::<EditionMetadataRow>(connection)
    .optional()?
    .ok_or_else(|| CatalogSelectionError::NotFound {
        selector: edition_id.as_i64().to_string(),
    })?;

    let source_sha256 = fixed_digest::<32>(row.source_sha256, "catalog_source_files.sha256")?;
    let source_sha1 = row
        .source_sha1
        .map(|value| fixed_digest::<20>(value, "catalog_source_files.sha1"))
        .transpose()?;
    let byte_length = u64::try_from(row.source_byte_length).map_err(|_| {
        CatalogSelectionError::InvalidMetadata(format!(
            "catalog_source_files.byte_length is negative: {}",
            row.source_byte_length
        ))
    })?;
    let receipt = match row.file_receipt_id {
        None => None,
        Some(_) => {
            if row.receipt_source_file_id != Some(row.source_file_id) {
                return Err(CatalogSelectionError::InvalidMetadata(format!(
                    "edition {} receipt does not reference its source file",
                    row.edition_id
                )));
            }
            let receipt_key = required(row.receipt_key, "catalog_file_receipts.receipt_key")?;
            let fetch_attempt_id = checked_id(required(
                row.fetch_attempt_id,
                "catalog_file_receipts.fetch_attempt_id",
            )?)?;
            let source_uri = required(row.source_uri, "catalog_fetch_attempts.uri")?;
            let method = required(row.fetch_method, "catalog_fetch_attempts.method")?;
            let requested_at = required(row.requested_at, "catalog_fetch_attempts.requested_at")?;
            let verification_status = required(
                row.verification_status,
                "catalog_fetch_attempts.verification_status",
            )?;
            if row.fetch_publisher_id != Some(row.publisher_id)
                || row.fetch_outcome.as_deref() != Some("retained")
            {
                return Err(CatalogSelectionError::InvalidMetadata(format!(
                    "edition {} receipt does not reference a retained fetch by its publisher",
                    row.edition_id
                )));
            }
            Some(CatalogReceiptMetadata {
                receipt_key,
                fetch_attempt_id,
                source_uri,
                method,
                requested_at,
                responded_at: row.responded_at,
                declared_filename: row.declared_filename,
                verification_status,
            })
        }
    };
    let previous_edition_id = match row.previous_edition_id {
        None => None,
        Some(previous_id) if row.joined_previous_edition_id == Some(previous_id) => {
            Some(checked_id(previous_id)?)
        }
        Some(previous_id) => {
            return Err(CatalogSelectionError::InvalidMetadata(format!(
                "edition {} has missing or cross-catalog predecessor {previous_id}",
                row.edition_id
            )));
        }
    };
    let coverage_id = CoverageId::try_from(row.coverage_id)?;
    let reading_rules_id = ReadingRulesId::try_from(row.reading_rules_id)?;
    let coverage = catalog_coverage::load(connection, coverage_id)?;
    let reading_rules = reading_rules::load(connection, reading_rules_id)?;

    Ok(SelectedCatalogEdition {
        publisher_id: checked_id(row.publisher_id)?,
        publisher_key: row.publisher_key,
        publisher_display_name: row.publisher_display_name,
        publisher_locator: row.publisher_locator,
        catalog_id: checked_id(row.catalog_id)?,
        catalog_key: row.catalog_key,
        catalog_display_name: row.catalog_display_name,
        edition_id: checked_id(row.edition_id)?,
        published_at: row.published_at,
        source_file_id: checked_id(row.source_file_id)?,
        source: CatalogSourceMetadata {
            sha256: source_sha256,
            sha1: source_sha1,
            byte_length,
            object_key: row.source_object_key,
            codec: row.source_codec,
        },
        reading_rules_id,
        reading_rules,
        coverage_id,
        coverage,
        file_receipt_id: row.file_receipt_id.map(checked_id).transpose()?,
        receipt,
        previous_edition_id,
    })
}

fn retained_source_path(path: &Utf8Path) -> Result<Utf8PathBuf, CatalogSelectionError> {
    if let Ok(canonical) = path.canonicalize_utf8() {
        return Ok(canonical);
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        let parent = if parent.as_str().is_empty() {
            Utf8Path::new(".")
        } else {
            parent
        };
        if let Ok(canonical) = parent.canonicalize_utf8() {
            return Ok(canonical.join(name));
        }
    }
    let absolute = std::path::absolute(path.as_std_path())
        .map_err(|error| CatalogSelectionError::InvalidSourcePath(error.to_string()))?;
    Utf8PathBuf::try_from(absolute)
        .map_err(|error| CatalogSelectionError::InvalidSourcePath(error.to_string()))
}

fn checked_id<Id>(value: i64) -> Result<Id, CatalogSelectionError>
where
    Id: TryFrom<i64, Error = CatalogIdError>,
{
    Id::try_from(value).map_err(CatalogSelectionError::from)
}

fn fixed_digest<const LENGTH: usize>(
    value: Vec<u8>,
    column: &str,
) -> Result<[u8; LENGTH], CatalogSelectionError> {
    value.try_into().map_err(|value: Vec<u8>| {
        CatalogSelectionError::InvalidMetadata(format!(
            "{column} has {} bytes, expected {LENGTH}",
            value.len()
        ))
    })
}

fn required<T>(value: Option<T>, column: &str) -> Result<T, CatalogSelectionError> {
    value.ok_or_else(|| {
        CatalogSelectionError::InvalidMetadata(format!("missing joined value for {column}"))
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::storage::catalog_ids::{CatalogId, EditionId, FileReceiptId};
    use crate::storage::reading_rules::FormatFamily;
    use diesel::{
        Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
        sql_query, sql_types::BigInt,
    };

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn fixture() -> crate::Result<SqliteConnection> {
        let mut connection = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
        crate::storage::db::initialize_database(&mut connection)?;
        connection.batch_execute(
            "INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL);
             INSERT INTO catalogs VALUES
                (1,1,'/selector/exact.dat','Exact path conflict'),
                (2,1,'catalog/source','Shared display'),
                (3,1,'catalog/ambiguous','Shared display'),
                (4,1,'local-dat:/selector/ambiguous.dat','Local source');
             INSERT INTO catalog_source_files
                (source_file_id,sha256,byte_length,object_key,codec) VALUES
                (1,zeroblob(32),64,'sha256/source-1','zstd'),
                (2,randomblob(32),64,'sha256/source-2','zstd'),
                (3,randomblob(32),64,'sha256/source-3','zstd'),
                (4,randomblob(32),64,'sha256/source-4','zstd'),
                (5,randomblob(32),64,'sha256/source-5','zstd'),
                (6,randomblob(32),64,'sha256/source-6','zstd'),
                (7,randomblob(32),64,'sha256/source-7','zstd');
             INSERT INTO catalog_fetch_attempts
                (fetch_attempt_id,publisher_id,attempt_key,uri,method,requested_at,
                 outcome,verification_status) VALUES
                (1,1,'fetch-1','https://example.test/old.dat','GET','2026-01-01','retained','not_requested'),
                (2,1,'fetch-2','/selector/exact.dat','GET','2026-01-02','retained','not_requested'),
                (3,1,'fetch-3','/selector/exact.dat','GET','2026-01-03','retained','not_requested'),
                (4,1,'fetch-4','/selector/ambiguous.dat','GET','2026-01-04','retained','not_requested'),
                (5,1,'fetch-5','https://example.test/new.dat','GET','2026-01-05','retained','not_requested'),
                (6,1,'fetch-6','/selector/ambiguous.dat','GET','2026-01-06','retained','not_requested'),
                (7,1,'fetch-7','https://example.test/tie.dat','GET','2026-01-07','retained','not_requested');
             INSERT INTO catalog_file_receipts VALUES
                (1,'receipt-1',1,1),(2,'receipt-2',2,2),
                (3,'receipt-3',3,3),(4,'receipt-4',4,4),(5,'receipt-5',5,5),
                (6,'receipt-6',6,6),(7,'receipt-7',7,7);
             INSERT INTO catalog_reading_rules VALUES
                (1,'mame-rules','mame','test','0','test','1');
             INSERT INTO catalog_coverage VALUES(1,'complete');
             INSERT INTO catalog_editions VALUES
                (101,1,1,1,1,1,NULL),
                (102,1,5,1,1,5,101),
                (103,2,3,1,1,3,NULL),
                (104,3,4,1,1,4,NULL),
                (105,4,6,1,1,6,NULL),
                (106,1,7,1,1,7,101);",
        )?;
        Ok(connection)
    }

    fn publish_mame_edition(
        connection: &mut SqliteConnection,
        edition: i64,
        published_at: &str,
    ) -> crate::Result<()> {
        let edition = EditionId::try_from(edition)
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
        crate::storage::test_catalog::publish_minimal_mame(connection, edition, published_at)
    }

    #[test]
    fn canonical_selection_pins_latest_and_prefers_exact_key_before_path_and_name()
    -> crate::Result<()> {
        let mut connection = fixture()?;
        publish_mame_edition(&mut connection, 101, "2026-01-02T00:00:00Z")?;
        publish_mame_edition(&mut connection, 103, "2026-01-02T00:00:00Z")?;
        publish_mame_edition(&mut connection, 104, "2026-01-03T00:00:00Z")?;
        // Insert the older edition last; latest is ordered by time, then edition ID.
        publish_mame_edition(&mut connection, 106, "2026-01-04T00:00:00Z")?;
        publish_mame_edition(&mut connection, 105, "2026-01-04T00:00:00Z")?;
        publish_mame_edition(&mut connection, 102, "2026-01-01T00:00:00Z")?;

        let latest = select_catalog_edition(
            &mut connection,
            CatalogSelection::Latest {
                catalog_id: CatalogId::try_from(1).expect("positive catalog ID"),
            },
        )
        .map_err(selection_error)?;
        assert_eq!(
            latest.catalog_id,
            CatalogId::try_from(1).expect("positive catalog ID")
        );
        assert_eq!(
            latest.edition_id,
            EditionId::try_from(106).expect("positive edition ID")
        );

        let exact_key = select_catalog_edition(
            &mut connection,
            CatalogSelection::ByNameOrSource("/selector/exact.dat"),
        )
        .map_err(selection_error)?;
        assert_eq!(
            exact_key.catalog_id,
            CatalogId::try_from(1).expect("positive catalog ID")
        );

        let local_path = select_catalog_edition(
            &mut connection,
            CatalogSelection::ByNameOrSource("/selector/ambiguous.dat"),
        )
        .map_err(selection_error)?;
        assert_eq!(
            local_path.catalog_id,
            CatalogId::try_from(4).expect("positive catalog ID")
        );

        let pinned = select_catalog_edition(
            &mut connection,
            CatalogSelection::Edition(EditionId::try_from(102).expect("positive edition ID")),
        )
        .map_err(selection_error)?;
        assert_eq!(pinned.source_file_id.as_i64(), 5);
        assert_eq!(pinned.file_receipt_id.map(FileReceiptId::as_i64), Some(5));
        assert_eq!(
            pinned
                .receipt
                .as_ref()
                .map(|receipt| receipt.receipt_key.as_str()),
            Some("receipt-5")
        );
        assert_eq!(
            pinned.previous_edition_id,
            Some(EditionId::try_from(101).expect("positive edition ID"))
        );
        assert_eq!(pinned.reading_rules.format_family, FormatFamily::Mame);
        assert_eq!(pinned.coverage, CatalogScope::Complete);

        let ambiguous = select_catalog_edition(
            &mut connection,
            CatalogSelection::ByNameOrSource("Shared display"),
        );
        match ambiguous {
            Err(CatalogSelectionError::Ambiguous { catalog_ids, .. }) => {
                assert_eq!(catalog_ids.len(), 2);
            }
            result => panic!("expected two-catalog ambiguity, got {result:?}"),
        }

        let row = sql_query("SELECT count(*) AS count FROM published_catalog_editions")
            .get_result::<Count>(&mut connection)?;
        assert_eq!(row.count, 6);
        Ok(())
    }

    fn selection_error(error: CatalogSelectionError) -> crate::Error {
        crate::Error::DatabaseSchema(error.to_string())
    }

    #[test]
    fn source_selector_recognizes_later_receipts_for_the_same_published_source() -> crate::Result<()>
    {
        let mut connection = fixture()?;
        publish_mame_edition(&mut connection, 101, "2026-01-02T00:00:00Z")?;
        connection.batch_execute(
            "INSERT INTO catalog_fetch_attempts
                (fetch_attempt_id,publisher_id,attempt_key,uri,method,requested_at,
                 outcome,verification_status) VALUES
                (8,1,'fetch-8','https://example.test/mirror.dat','GET','2026-02-01','retained','not_requested');
             INSERT INTO catalog_file_receipts VALUES(8,'receipt-8',8,1);",
        )?;
        let selected = select_catalog_edition(
            &mut connection,
            CatalogSelection::ByNameOrSource("https://example.test/mirror.dat"),
        )
        .map_err(selection_error)?;
        assert_eq!(selected.edition_id.as_i64(), 101);
        assert_eq!(selected.file_receipt_id.map(FileReceiptId::as_i64), Some(1));
        Ok(())
    }
}
