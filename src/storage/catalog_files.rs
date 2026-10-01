//! Read published catalog occurrences by shared file identity.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use thiserror::Error;

use crate::{
    domain::{CatalogContentId, CatalogRegistryId, CatalogSetId},
    storage::catalog_content::registry_id,
};

use super::db::Pool;
pub use crate::domain::ContentDigestAlgorithm as DigestAlgorithm;
pub use crate::storage::catalog_identity::OccurrenceId;

#[cfg(test)]
mod tests;

const MAX_PAGE_SIZE: usize = 500;
const MAX_BULK_OCCURRENCES: usize = 10_000;
const REQUEST_TABLE: &str = "temp.catalog_files_requested_occurrences";

/// The namespace that owns an occurrence: either a catalog root or a software list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetGroupKind {
    Root,
    SoftwareList { name: String },
}

/// Stable source element kinds used by catalog snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceElementKind {
    MameMachine,
    SoftwareItem,
    LogiqxGame,
    ClrMameProSet,
    NoIntroPcGame,
    NoIntroDatGame,
    NoIntroDatabaseGame,
}

/// Type of a source occurrence within its owning set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccurrenceKind {
    MameRom,
    MameDisk,
    MameSample,
    LogiqxRom,
    LogiqxDisk,
    LogiqxSample,
    ClrMameProRom,
    ClrMameProSample,
    NoIntroPcFile,
    NoIntroDatRom,
    NoIntroDatabaseFile,
    SoftwareRomEntry,
    SoftwareRomOperation,
    SoftwareDiskEntry,
}

/// A source location in the original catalog document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: i64,
    pub column: i64,
}

/// The kind of software area containing a native ROM or disk entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareAreaKind {
    Data,
    Disk,
}

/// The actual software part and area that own a file occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareAssetOwner {
    pub part_id: i64,
    pub area_id: i64,
    pub part_order: i64,
    pub part_name: String,
    pub area_name: String,
    pub area_order: i64,
    pub area_kind: SoftwareAreaKind,
}

/// Where a digest assertion came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DigestProvenance {
    SourceDeclared,
    Computed,
    Unknown,
}

/// Digest assertion attached to one source occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OccurrenceDigest {
    pub algorithm: DigestAlgorithm,
    pub value: Vec<u8>,
    pub scope: String,
    pub provenance: DigestProvenance,
}

/// Source and owning-set provenance for one asset occurrence.
///
/// Numeric database IDs identify rows only within the registry generation that loaded them;
/// callers should use source/catalog/snapshot keys for durable external references.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OccurrenceProvenance {
    pub source_key: String,
    pub source_name: String,
    pub catalog_key: String,
    pub catalog_name: String,
    pub snapshot_key: String,
    pub document_key: String,
    pub interpretation_key: String,
    pub format: String,
    pub set_group_id: i64,
    pub set_group_kind: SetGroupKind,
    pub set_id: CatalogSetId,
    pub set_name: String,
    pub source_element_kind: SourceElementKind,
    pub occurrence_kind: OccurrenceKind,
    pub occurrence_order: i64,
    pub set_location: SourceLocation,
    pub native_occurrence_location: Option<SourceLocation>,
    pub asset_name: Option<String>,
    pub software_owner: Option<SoftwareAssetOwner>,
}

/// One source occurrence, retaining its shared identity and all digest assertions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogFileOccurrence {
    pub occurrence_id: OccurrenceId,
    pub content_id: Option<CatalogContentId>,
    pub provenance: OccurrenceProvenance,
    pub digests: Vec<OccurrenceDigest>,
    pub no_intro_dat_rom: Option<NoIntroDatRomPayload>,
}

/// No-Intro flat DAT declarations attached to one ROM occurrence.
///
/// Non-hash strings preserve source lexemes. Hash strings are lowercase for valid byte values and
/// preserve the exact invalid source lexeme; `None` means absent and `Some("")` means explicitly
/// empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatRomPayload {
    pub name: String,
    pub size_text: Option<String>,
    pub crc_text: Option<String>,
    pub md5_text: Option<String>,
    pub sha1_text: Option<String>,
    pub sha256_text: Option<String>,
    pub status_text: Option<String>,
    pub serial_text: Option<String>,
    pub header_text: Option<String>,
    pub date_text: Option<String>,
    pub mia_text: Option<String>,
    pub source_order: i64,
    pub location: SourceLocation,
}

/// A validated bound for content-to-occurrence pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContentOccurrenceLimit(usize);

impl ContentOccurrenceLimit {
    /// Create a page limit in the inclusive range `1..=500`.
    pub fn new(value: usize) -> Result<Self, CatalogFilesError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(CatalogFilesError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }
}

/// Opaque keyset position, bound to the content UUID used to create it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentOccurrenceCursor {
    content: CatalogContentId,
    registry: CatalogRegistryId,
    after_occurrence: i64,
}

/// One keyset page of published occurrences for a content UUID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentOccurrencePage {
    pub occurrences: Vec<CatalogFileOccurrence>,
    pub next_cursor: Option<ContentOccurrenceCursor>,
}

#[derive(Debug, Error)]
pub enum CatalogFilesError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("cursor belongs to a different catalog content UUID")]
    CursorContentMismatch,
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("catalog registry lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("stored catalog content UUID contains {0} bytes; expected 16")]
    InvalidContentIdLength(usize),
    #[error("digest assertion refers to occurrence {0}, which was not selected")]
    MissingOccurrenceOwner(i64),
    #[error("No-Intro DAT ROM occurrence {0} has no native payload")]
    MissingNoIntroDatRomPayload(i64),
    #[error("No-Intro DAT ROM occurrence {0} is not owned by a No-Intro DAT game")]
    MismatchedNoIntroDatRomOwner(i64),
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
    #[error("bulk request contains {requested} occurrence IDs; maximum is {maximum}")]
    BulkRequestTooLarge { requested: usize, maximum: usize },
    #[error("unsupported stored {field} value: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error(
        "occurrence {occurrence_id} has {algorithm:?} digest with {actual} bytes; expected {expected}"
    )]
    DigestLengthMismatch {
        occurrence_id: i64,
        algorithm: DigestAlgorithm,
        actual: usize,
        expected: usize,
    },
}

#[derive(QueryableByName)]
struct OccurrenceRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    source_key: String,
    #[diesel(sql_type = Text)]
    source_name: String,
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    catalog_name: String,
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Text)]
    interpretation_key: String,
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = BigInt)]
    set_group_id: i64,
    #[diesel(sql_type = Text)]
    group_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    software_list_name: Option<String>,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    source_element_kind: String,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = BigInt)]
    occurrence_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct NativePayloadRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    asset_name: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    software_part_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    software_area_name: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    software_area_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    software_area_kind: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    software_part_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    software_area_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    software_part_order: Option<i64>,
}

#[derive(QueryableByName)]
struct DigestRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
    #[diesel(sql_type = Text)]
    scope: String,
    #[diesel(sql_type = Text)]
    provenance: String,
}

#[derive(QueryableByName)]
struct NoIntroDatRomRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha256_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    status_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mia_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

/// Resolve a batch of source occurrence identities in one database transaction.
///
/// Unlinked occurrences are returned with `content_id: None`; duplicate requested IDs do not
/// duplicate owners. Only occurrences in published snapshots are visible.
pub fn occurrences_for_ids(
    database: &crate::database::Database,
    occurrence_ids: &[OccurrenceId],
) -> Result<Vec<CatalogFileOccurrence>, CatalogFilesError> {
    occurrences_for_ids_in_pool(database.pool(), occurrence_ids)
}

fn occurrences_for_ids_in_pool(
    pool: &Pool,
    occurrence_ids: &[OccurrenceId],
) -> Result<Vec<CatalogFileOccurrence>, CatalogFilesError> {
    let ids = occurrence_ids
        .iter()
        .map(|id| id.database_value())
        .collect::<BTreeSet<_>>();
    if ids.len() > MAX_BULK_OCCURRENCES {
        return Err(CatalogFilesError::BulkRequestTooLarge {
            requested: ids.len(),
            maximum: MAX_BULK_OCCURRENCES,
        });
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut connection = pool.get()?;
    connection.transaction(|connection| {
        create_request_table(connection)?;
        insert_requested_ids(connection, &ids)?;
        let rows = sql_query(requested_occurrence_select()).load::<OccurrenceRow>(connection)?;
        let result = assemble_occurrences(connection, rows)?;
        drop_request_table(connection)?;
        Ok(result)
    })
}

/// Load one keyset page of published occurrences linked to `content_id`.
pub fn occurrences_for_content(
    database: &crate::database::Database,
    content_id: CatalogContentId,
    limit: ContentOccurrenceLimit,
    cursor: Option<&ContentOccurrenceCursor>,
) -> Result<ContentOccurrencePage, CatalogFilesError> {
    occurrences_for_content_in_pool(database.pool(), content_id, limit, cursor)
}

fn occurrences_for_content_in_pool(
    pool: &Pool,
    content_id: CatalogContentId,
    limit: ContentOccurrenceLimit,
    cursor: Option<&ContentOccurrenceCursor>,
) -> Result<ContentOccurrencePage, CatalogFilesError> {
    if cursor.is_some_and(|cursor| cursor.content != content_id) {
        return Err(CatalogFilesError::CursorContentMismatch);
    }

    let mut connection = pool.get()?;
    connection.transaction(|connection| {
        let current_registry_id = registry_id(connection)?;
        if cursor.is_some_and(|cursor| cursor.registry != current_registry_id) {
            return Err(CatalogFilesError::CursorRegistryMismatch);
        }
        let after_id = cursor.map_or(0, |cursor| cursor.after_occurrence);
        let rows = sql_query(format!(
            "{} LIMIT ?",
            occurrence_select(
                "occurrence.content_uuid = ? AND occurrence.occurrence_id > ?",
                "asset_occurrences AS occurrence",
            )
        ))
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .bind::<BigInt, _>(after_id)
        .bind::<BigInt, _>(
            i64::try_from(limit.0 + 1).map_err(|_| CatalogFilesError::PageLimitOverflow)?,
        )
        .load::<OccurrenceRow>(connection)?;
        let has_more = rows.len() > limit.0;
        create_request_table(connection)?;
        let mut occurrences =
            assemble_occurrences(connection, rows.into_iter().take(limit.0).collect())?;
        let next_cursor = if has_more {
            Some(ContentOccurrenceCursor {
                content: content_id,
                registry: current_registry_id,
                after_occurrence: occurrences
                    .last()
                    .ok_or(CatalogFilesError::PageLimitOverflow)?
                    .occurrence_id
                    .database_value(),
            })
        } else {
            None
        };
        let page = ContentOccurrencePage {
            occurrences: std::mem::take(&mut occurrences),
            next_cursor,
        };
        drop_request_table(connection)?;
        Ok(page)
    })
}

fn create_request_table(connection: &mut SqliteConnection) -> diesel::QueryResult<()> {
    sql_query(format!(
        "CREATE TEMP TABLE {REQUEST_TABLE} (occurrence_id INTEGER PRIMARY KEY)"
    ))
    .execute(connection)?;
    Ok(())
}

fn drop_request_table(connection: &mut SqliteConnection) -> diesel::QueryResult<()> {
    sql_query(format!("DROP TABLE {REQUEST_TABLE}")).execute(connection)?;
    Ok(())
}

fn clear_request_table(connection: &mut SqliteConnection) -> diesel::QueryResult<()> {
    sql_query(format!("DELETE FROM {REQUEST_TABLE}")).execute(connection)?;
    Ok(())
}

fn insert_requested_ids(
    connection: &mut SqliteConnection,
    ids: &BTreeSet<i64>,
) -> diesel::QueryResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let values = ids
        .iter()
        .map(|id| format!("({id})"))
        .collect::<Vec<_>>()
        .join(",");
    sql_query(format!(
        "INSERT INTO {REQUEST_TABLE} (occurrence_id) VALUES {values}"
    ))
    .execute(connection)?;
    Ok(())
}

fn requested_occurrence_select() -> String {
    occurrence_select(
        "occurrence.occurrence_id = requested.occurrence_id",
        &format!("{REQUEST_TABLE} AS requested CROSS JOIN asset_occurrences AS occurrence"),
    )
}

fn occurrence_select(predicate: &str, from: &str) -> String {
    format!(
        "SELECT occurrence.occurrence_id, occurrence.content_uuid, \
                source.source_key, source.display_name AS source_name, \
                catalog.catalog_key, catalog.display_name AS catalog_name, \
                snapshot.snapshot_key, snapshot.document_key, snapshot.interpretation_key, \
                interpretation.format, group_row.set_group_id, group_row.kind AS group_kind, \
                software_list.name AS software_list_name, catalog_set.set_id, catalog_set.set_name, \
                catalog_set.source_element_kind, occurrence.claim_kind, \
                occurrence.occurrence_order, catalog_set.source_line, catalog_set.source_column \
         FROM {from} \
         JOIN catalog_sets AS catalog_set ON catalog_set.set_id = occurrence.record_id \
         JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id \
         JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = group_row.snapshot_key \
         JOIN snapshot_publications AS publication ON publication.snapshot_key = snapshot.snapshot_key \
         JOIN catalogs AS catalog ON catalog.catalog_key = snapshot.catalog_key \
         JOIN publishing_sources AS source ON source.source_key = catalog.source_key \
         JOIN documents AS document ON document.document_key = snapshot.document_key \
         JOIN parser_interpretations AS interpretation \
           ON interpretation.interpretation_key = snapshot.interpretation_key \
         LEFT JOIN software_lists AS software_list ON software_list.namespace_id = group_row.set_group_id \
         WHERE {predicate} \
         ORDER BY occurrence.occurrence_id"
    )
}

const fn native_payload_select() -> &'static str {
    "SELECT occurrence.occurrence_id, \
            CASE \
              WHEN occurrence.claim_kind = 'software_rom_operation' THEN NULL \
              WHEN group_row.kind = 'software_list' \
                THEN COALESCE(software_rom.name, software_disk.name, software_declaration.name) \
              ELSE COALESCE(mame_rom.name, mame_disk.name, logiqx_rom.name, logiqx_disk.name, \
                            logiqx_sample.name, cmp_rom.name, cmp_sample.sample_name, \
                            no_intro_file.name, mame_sample.name) \
            END AS asset_name, \
            COALESCE(mame_rom.source_line, mame_disk.source_line, logiqx_rom.source_line, \
                     logiqx_disk.source_line, logiqx_sample.source_line, cmp_rom.source_line, \
                     cmp_sample.source_line, no_intro_file.source_line, mame_sample.source_line, \
                     software_rom.source_line, software_disk.source_line) AS native_line, \
            COALESCE(mame_rom.source_column, mame_disk.source_column, logiqx_rom.source_column, \
                     logiqx_disk.source_column, logiqx_sample.source_column, cmp_rom.source_column, \
                     cmp_sample.source_column, no_intro_file.source_column, mame_sample.source_column, \
                     software_rom.source_column, software_disk.source_column) AS native_column, \
            software_part.part_name AS software_part_name, \
            software_area.area_name AS software_area_name, \
            software_area.area_order AS software_area_order, \
            software_area.area_kind AS software_area_kind, \
            software_part.part_id AS software_part_id, \
            software_area.area_id AS software_area_id, \
            software_part.part_order AS software_part_order \
     FROM temp.catalog_files_requested_occurrences AS requested \
     CROSS JOIN asset_occurrences AS occurrence \
     JOIN catalog_sets AS catalog_set ON catalog_set.set_id = occurrence.record_id \
     JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id \
     LEFT JOIN mame_rom_claims AS mame_rom ON mame_rom.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN mame_disk_claims AS mame_disk ON mame_disk.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN logiqx_rom_claims AS logiqx_rom ON logiqx_rom.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN logiqx_disk_claims AS logiqx_disk ON logiqx_disk.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN logiqx_sample_claims AS logiqx_sample ON logiqx_sample.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN cmp_rom_claims AS cmp_rom ON cmp_rom.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN cmp_samples AS cmp_sample ON cmp_sample.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN no_intro_pc_file_claims AS no_intro_file ON no_intro_file.occurrence_id = occurrence.occurrence_id \
     LEFT JOIN mame_machine_samples AS mame_sample \
       ON mame_sample.set_id = catalog_set.set_id \
      AND mame_sample.element_order = occurrence.occurrence_order \
      AND occurrence.claim_kind = 'mame_sample' \
     LEFT JOIN software_rom_entries AS software_rom \
       ON software_rom.occurrence_id = occurrence.occurrence_id \
      AND software_rom.record_id = catalog_set.set_id \
     LEFT JOIN software_disk_entries AS software_disk \
       ON software_disk.occurrence_id = occurrence.occurrence_id \
      AND software_disk.record_id = catalog_set.set_id \
     LEFT JOIN software_file_uses AS software_use \
       ON software_use.occurrence_id = occurrence.occurrence_id \
      AND software_use.record_id = catalog_set.set_id \
     LEFT JOIN software_file_declarations AS software_declaration \
       ON software_declaration.occurrence_id = software_use.declaration_occurrence_id \
      AND software_declaration.record_id = software_use.record_id \
     LEFT JOIN software_areas AS software_area \
       ON software_area.area_id = COALESCE(software_rom.area_id, software_disk.area_id) \
      AND software_area.record_id = catalog_set.set_id \
     LEFT JOIN software_parts AS software_part \
       ON software_part.part_id = software_area.part_id \
      AND software_part.record_id = software_area.record_id \
     WHERE occurrence.occurrence_id = requested.occurrence_id \
     ORDER BY occurrence.occurrence_id"
}

fn assemble_occurrences(
    connection: &mut SqliteConnection,
    rows: Vec<OccurrenceRow>,
) -> Result<Vec<CatalogFileOccurrence>, CatalogFilesError> {
    let mut occurrences = rows
        .into_iter()
        .map(try_occurrence)
        .collect::<Result<Vec<_>, _>>()?;
    let ids = occurrences
        .iter()
        .map(|occurrence| occurrence.occurrence_id.database_value())
        .collect::<BTreeSet<_>>();
    if ids.is_empty() {
        return Ok(occurrences);
    }

    // The requested-ID table bounds native payload and digest reads without multiplying owners.
    clear_request_table(connection)?;
    insert_requested_ids(connection, &ids)?;
    let native_rows = sql_query(native_payload_select()).load::<NativePayloadRow>(connection)?;
    let by_id = occurrences
        .iter()
        .enumerate()
        .map(|(index, occurrence)| (occurrence.occurrence_id.database_value(), index))
        .collect::<BTreeMap<_, _>>();
    for row in native_rows {
        let index = *by_id
            .get(&row.occurrence_id)
            .ok_or(CatalogFilesError::MissingOccurrenceOwner(row.occurrence_id))?;
        let occurrence = occurrences
            .get_mut(index)
            .ok_or(CatalogFilesError::MissingOccurrenceOwner(row.occurrence_id))?;
        occurrence.provenance.asset_name = row.asset_name;
        occurrence.provenance.native_occurrence_location = optional_location(
            row.native_line,
            row.native_column,
            "native occurrence location",
        )?;
        occurrence.provenance.software_owner = match &occurrence.provenance.set_group_kind {
            SetGroupKind::Root => None,
            SetGroupKind::SoftwareList { .. } => Some(SoftwareAssetOwner {
                part_id: required_native(row.software_part_id, "software part ID")?,
                area_id: required_native(row.software_area_id, "software area ID")?,
                part_order: required_native(row.software_part_order, "software part order")?,
                part_name: required_native(row.software_part_name, "software part name")?,
                area_name: required_native(row.software_area_name, "software area name")?,
                area_order: required_native(row.software_area_order, "software area order")?,
                area_kind: parse_software_area_kind(required_native(
                    row.software_area_kind,
                    "software area kind",
                )?)?,
            }),
        };
    }
    attach_no_intro_dat_rom_payloads(connection, &mut occurrences)?;
    let digests = sql_query(digest_select()).load::<DigestRow>(connection)?;
    for row in digests {
        let algorithm = parse_algorithm(row.algorithm)?;
        let actual = row.digest.len();
        let expected = algorithm.byte_length();
        if actual != expected {
            return Err(CatalogFilesError::DigestLengthMismatch {
                occurrence_id: row.occurrence_id,
                algorithm,
                actual,
                expected,
            });
        }
        let index = *by_id
            .get(&row.occurrence_id)
            .ok_or(CatalogFilesError::MissingOccurrenceOwner(row.occurrence_id))?;
        occurrences
            .get_mut(index)
            .ok_or(CatalogFilesError::MissingOccurrenceOwner(row.occurrence_id))?
            .digests
            .push(OccurrenceDigest {
                algorithm,
                value: row.digest,
                scope: row.scope,
                provenance: parse_provenance(row.provenance)?,
            });
    }
    clear_request_table(connection)?;
    Ok(occurrences)
}

fn attach_no_intro_dat_rom_payloads(
    connection: &mut SqliteConnection,
    occurrences: &mut [CatalogFileOccurrence],
) -> Result<(), CatalogFilesError> {
    let mut native_dat_roms = sql_query(no_intro_dat_rom_select())
        .load::<NoIntroDatRomRow>(connection)?
        .into_iter()
        .map(|row| {
            let occurrence_id = row.occurrence_id;
            let payload = NoIntroDatRomPayload {
                name: row.name,
                size_text: row.size_text,
                crc_text: row.crc_text,
                md5_text: row.md5_text,
                sha1_text: row.sha1_text,
                sha256_text: row.sha256_text,
                status_text: row.status_text,
                serial_text: row.serial_text,
                header_text: row.header_text,
                date_text: row.date_text,
                mia_text: row.mia_text,
                source_order: row.source_order,
                location: SourceLocation {
                    line: row.source_line,
                    column: row.source_column,
                },
            };
            (occurrence_id, payload)
        })
        .collect::<BTreeMap<_, _>>();
    for occurrence in occurrences {
        let occurrence_id = occurrence.occurrence_id.database_value();
        if occurrence.provenance.occurrence_kind == OccurrenceKind::NoIntroDatRom {
            if occurrence.provenance.source_element_kind != SourceElementKind::NoIntroDatGame {
                return Err(CatalogFilesError::MismatchedNoIntroDatRomOwner(
                    occurrence_id,
                ));
            }
            let payload = native_dat_roms.remove(&occurrence_id).ok_or(
                CatalogFilesError::MissingNoIntroDatRomPayload(occurrence_id),
            )?;
            occurrence.provenance.asset_name = Some(payload.name.clone());
            occurrence.provenance.native_occurrence_location = Some(payload.location);
            occurrence.no_intro_dat_rom = Some(payload);
        } else if native_dat_roms.contains_key(&occurrence_id) {
            return Err(CatalogFilesError::MismatchedNoIntroDatRomOwner(
                occurrence_id,
            ));
        }
    }
    if let Some((occurrence_id, _)) = native_dat_roms.first_key_value() {
        return Err(CatalogFilesError::MissingOccurrenceOwner(*occurrence_id));
    }
    Ok(())
}

fn no_intro_dat_rom_select() -> String {
    // Keep the bounded request as the outer loop, not a catalog-wide ROM scan.
    format!(
        "SELECT rom.occurrence_id, rom.name, rom.size_text, \
                CASE WHEN crc_field.occurrence_id IS NULL THEN NULL \
                     WHEN crc_value.digest_id IS NOT NULL THEN lower(hex(crc_value.digest)) \
                     ELSE crc_field.invalid_text END AS crc_text, \
                CASE WHEN md5_field.occurrence_id IS NULL THEN NULL \
                     WHEN md5_value.digest_id IS NOT NULL THEN lower(hex(md5_value.digest)) \
                     ELSE md5_field.invalid_text END AS md5_text, \
                CASE WHEN sha1_field.occurrence_id IS NULL THEN NULL \
                     WHEN sha1_value.digest_id IS NOT NULL THEN lower(hex(sha1_value.digest)) \
                     ELSE sha1_field.invalid_text END AS sha1_text, \
                CASE WHEN sha256_field.occurrence_id IS NULL THEN NULL \
                     WHEN sha256_value.digest_id IS NOT NULL THEN lower(hex(sha256_value.digest)) \
                     ELSE sha256_field.invalid_text END AS sha256_text, \
                rom.status_text, rom.serial_text, \
                rom.header_text, rom.date_text, rom.mia_text, rom.source_order, \
                rom.source_line, rom.source_column \
         FROM {REQUEST_TABLE} AS requested \
         CROSS JOIN no_intro_dat_rom_claims AS rom \
         LEFT JOIN no_intro_dat_rom_digest_fields AS crc_field \
           ON crc_field.occurrence_id = rom.occurrence_id AND crc_field.field_kind = 2 \
         LEFT JOIN digest_values AS crc_value ON crc_value.digest_id = crc_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS md5_field \
           ON md5_field.occurrence_id = rom.occurrence_id AND md5_field.field_kind = 3 \
         LEFT JOIN digest_values AS md5_value ON md5_value.digest_id = md5_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS sha1_field \
           ON sha1_field.occurrence_id = rom.occurrence_id AND sha1_field.field_kind = 4 \
         LEFT JOIN digest_values AS sha1_value ON sha1_value.digest_id = sha1_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS sha256_field \
           ON sha256_field.occurrence_id = rom.occurrence_id AND sha256_field.field_kind = 5 \
         LEFT JOIN digest_values AS sha256_value ON sha256_value.digest_id = sha256_field.digest_id \
         WHERE rom.occurrence_id = requested.occurrence_id \
         ORDER BY rom.occurrence_id"
    )
}

fn digest_select() -> String {
    format!(
        "SELECT assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, \
                assertion.provenance \
         FROM {REQUEST_TABLE} AS requested \
         CROSS JOIN occurrence_digest_assertions AS assertion \
           ON assertion.occurrence_id = requested.occurrence_id \
         JOIN digest_values AS digest USING (digest_id) \
         ORDER BY assertion.occurrence_id, assertion.scope, digest.algorithm, digest.digest, assertion.provenance"
    )
}

fn try_occurrence(row: OccurrenceRow) -> Result<CatalogFileOccurrence, CatalogFilesError> {
    let content_id = match row.content_uuid {
        Some(bytes) => {
            let length = bytes.len();
            let bytes: [u8; 16] = bytes
                .try_into()
                .map_err(|_| CatalogFilesError::InvalidContentIdLength(length))?;
            Some(CatalogContentId::from_bytes(bytes))
        }
        None => None,
    };
    let set_group_kind = match row.group_kind.as_str() {
        "root" => SetGroupKind::Root,
        "software_list" => SetGroupKind::SoftwareList {
            name: row
                .software_list_name
                .ok_or_else(|| CatalogFilesError::InvalidStoredValue {
                    field: "software list name",
                    value: "NULL for software_list group".to_owned(),
                })?,
        },
        value => return Err(invalid_value("set group kind", value.to_owned())),
    };
    Ok(CatalogFileOccurrence {
        occurrence_id: OccurrenceId::from_database(row.occurrence_id),
        content_id,
        provenance: OccurrenceProvenance {
            source_key: row.source_key,
            source_name: row.source_name,
            catalog_key: row.catalog_key,
            catalog_name: row.catalog_name,
            snapshot_key: row.snapshot_key,
            document_key: row.document_key,
            interpretation_key: row.interpretation_key,
            format: row.format,
            set_group_id: row.set_group_id,
            set_group_kind,
            set_id: CatalogSetId::from_database(row.set_id),
            set_name: row.set_name,
            source_element_kind: parse_source_element_kind(row.source_element_kind)?,
            occurrence_kind: parse_occurrence_kind(row.claim_kind)?,
            occurrence_order: row.occurrence_order,
            set_location: SourceLocation {
                line: row.source_line,
                column: row.source_column,
            },
            native_occurrence_location: None,
            asset_name: None,
            software_owner: None,
        },
        digests: Vec::new(),
        no_intro_dat_rom: None,
    })
}

fn required_native<T>(value: Option<T>, field: &'static str) -> Result<T, CatalogFilesError> {
    value.ok_or_else(|| invalid_value(field, "NULL for software occurrence".to_owned()))
}

fn optional_location(
    line: Option<i64>,
    column: Option<i64>,
    field: &'static str,
) -> Result<Option<SourceLocation>, CatalogFilesError> {
    match (line, column) {
        (Some(line), Some(column)) => Ok(Some(SourceLocation { line, column })),
        (None, None) => Ok(None),
        _ => Err(invalid_value(
            field,
            "line/column only partially stored".to_owned(),
        )),
    }
}

fn parse_software_area_kind(value: String) -> Result<SoftwareAreaKind, CatalogFilesError> {
    match value.as_str() {
        "data" => Ok(SoftwareAreaKind::Data),
        "disk" => Ok(SoftwareAreaKind::Disk),
        _ => Err(invalid_value("software area kind", value)),
    }
}

fn parse_source_element_kind(value: String) -> Result<SourceElementKind, CatalogFilesError> {
    match value.as_str() {
        "mame_machine" => Ok(SourceElementKind::MameMachine),
        "software_item" => Ok(SourceElementKind::SoftwareItem),
        "logiqx_game" => Ok(SourceElementKind::LogiqxGame),
        "cmp_set" => Ok(SourceElementKind::ClrMameProSet),
        "no_intro_pc_game" => Ok(SourceElementKind::NoIntroPcGame),
        "no_intro_dat_game" => Ok(SourceElementKind::NoIntroDatGame),
        "no_intro_database_game" => Ok(SourceElementKind::NoIntroDatabaseGame),
        _ => Err(invalid_value("source element kind", value)),
    }
}

fn parse_occurrence_kind(value: String) -> Result<OccurrenceKind, CatalogFilesError> {
    match value.as_str() {
        "mame_rom" => Ok(OccurrenceKind::MameRom),
        "mame_disk" => Ok(OccurrenceKind::MameDisk),
        "mame_sample" => Ok(OccurrenceKind::MameSample),
        "logiqx_rom" => Ok(OccurrenceKind::LogiqxRom),
        "logiqx_disk" => Ok(OccurrenceKind::LogiqxDisk),
        "logiqx_sample" => Ok(OccurrenceKind::LogiqxSample),
        "cmp_rom" => Ok(OccurrenceKind::ClrMameProRom),
        "cmp_sample" => Ok(OccurrenceKind::ClrMameProSample),
        "no_intro_pc_file" => Ok(OccurrenceKind::NoIntroPcFile),
        "no_intro_dat_rom" => Ok(OccurrenceKind::NoIntroDatRom),
        "no_intro_database_file" => Ok(OccurrenceKind::NoIntroDatabaseFile),
        "software_rom_entry" => Ok(OccurrenceKind::SoftwareRomEntry),
        "software_rom_operation" => Ok(OccurrenceKind::SoftwareRomOperation),
        "software_disk_entry" => Ok(OccurrenceKind::SoftwareDiskEntry),
        _ => Err(invalid_value("occurrence kind", value)),
    }
}

fn parse_algorithm(value: String) -> Result<DigestAlgorithm, CatalogFilesError> {
    match value.as_str() {
        "crc32" => Ok(DigestAlgorithm::Crc32),
        "md5" => Ok(DigestAlgorithm::Md5),
        "sha1" => Ok(DigestAlgorithm::Sha1),
        "sha256" => Ok(DigestAlgorithm::Sha256),
        _ => Err(invalid_value("digest algorithm", value)),
    }
}

fn parse_provenance(value: String) -> Result<DigestProvenance, CatalogFilesError> {
    match value.as_str() {
        "source_declared" => Ok(DigestProvenance::SourceDeclared),
        "computed" => Ok(DigestProvenance::Computed),
        "unknown" => Ok(DigestProvenance::Unknown),
        _ => Err(invalid_value("digest provenance", value)),
    }
}

const fn invalid_value(field: &'static str, value: String) -> CatalogFilesError {
    CatalogFilesError::InvalidStoredValue { field, value }
}
