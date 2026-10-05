//! Bounded queries for native MAME software-list metadata.

mod attributes;
mod queries;
use super::catalog_files::XmlAttributePosition;
pub use crate::mame_softwarelist::{
    SoftwareDataAreaAttribute, SoftwareDipSwitchAttribute, SoftwareDipValueAttribute,
    SoftwareDiskAreaAttribute, SoftwareDiskAttribute, SoftwareItemAttribute, SoftwareListAttribute,
    SoftwareNamedValueAttribute, SoftwarePartAttribute, SoftwareRomAttribute,
    SoftwareWrapperAttribute,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareAreaAttributePositions {
    Data(Vec<XmlAttributePosition<SoftwareDataAreaAttribute>>),
    Disk(Vec<XmlAttributePosition<SoftwareDiskAreaAttribute>>),
}

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use thiserror::Error;

use crate::{
    database::Database,
    domain::{
        CatalogKey, CatalogRegistryId, CatalogSetId, DocumentKey, ParserInterpretationKey,
        PublishingSourceKey, SnapshotKey,
    },
    storage::{
        catalog_content::registry_id,
        catalog_files::{OccurrenceId, SourceLocation},
    },
};

const MAX_PAGE_SIZE: usize = 500;

pub use crate::mame_softwarelist::SoftwareTextField;
pub use crate::mame_softwarelist::{
    Endianness as SoftwareEndianness, SupportedStatus as SoftwareSupportedStatus,
};

/// A validated maximum number of rows returned by one query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoftwarePageLimit(usize);

impl SoftwarePageLimit {
    pub fn new(value: usize) -> Result<Self, SoftwareQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(SoftwareQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }

    fn database_value(self) -> QueryResult<i64> {
        i64::try_from(self.0).map_err(|_| SoftwareQueryError::PageLimitOverflow)
    }
}

/// Database-local software-list identity. It is valid only for its registry generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoftwareListId(i64);

impl SoftwareListId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

/// Database-local software part identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoftwarePartId(i64);

impl SoftwarePartId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

/// Database-local software area identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoftwareAreaId(i64);

impl SoftwareAreaId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

/// Continuation for a software-list page, bound to its snapshot and registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListCursor {
    generation: CatalogRegistryId,
    snapshot: SnapshotKey,
    order: i64,
    id: SoftwareListId,
}

/// Continuation for a title page, bound to its snapshot, list, and registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareTitleCursor {
    generation: CatalogRegistryId,
    snapshot: SnapshotKey,
    list: SoftwareListId,
    order: i64,
    id: CatalogSetId,
}

/// Envelope metadata for a native software-list document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareEnvelope {
    SingleList,
    PluralLists { build: Option<String> },
}

/// Snapshot and publication provenance shared by every row in a result page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareSnapshot {
    pub wrapper_attribute_positions: Vec<XmlAttributePosition<SoftwareWrapperAttribute>>,
    pub registry_id: CatalogRegistryId,
    pub snapshot_key: SnapshotKey,
    pub source_key: PublishingSourceKey,
    pub source_name: String,
    pub catalog_key: CatalogKey,
    pub catalog_name: String,
    pub document_key: DocumentKey,
    pub interpretation_key: ParserInterpretationKey,
    pub format: String,
    pub envelope: SoftwareEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareListPage {
    pub snapshot: SoftwareSnapshot,
    pub lists: Vec<SoftwareList>,
    pub next_cursor: Option<SoftwareListCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareTitlePage {
    pub snapshot: SoftwareSnapshot,
    pub list: SoftwareList,
    pub titles: Vec<SoftwareTitle>,
    pub next_cursor: Option<SoftwareTitleCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareTextPosition {
    pub field: SoftwareTextField,
    pub source_order: i64,
    pub location: SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareList {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwareListAttribute>>,
    pub id: SoftwareListId,
    pub name: String,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub text_positions: Vec<SoftwareTextPosition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareTitle {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwareItemAttribute>>,
    pub id: CatalogSetId,
    pub name: String,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub clone_of: Option<String>,
    pub supported: SoftwareSupportedStatus,
    pub supported_specified: bool,
    pub description: String,
    pub year: String,
    pub publisher: String,
    pub notes: Option<String>,
    pub text_positions: Vec<SoftwareTextPosition>,
    pub info: Vec<SoftwareNamedValue>,
    pub shared_features: Vec<SoftwareNamedValue>,
    pub parts: Vec<SoftwarePart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareNamedValue {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwareNamedValueAttribute>>,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub name: String,
    pub value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwarePart {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwarePartAttribute>>,
    pub id: SoftwarePartId,
    pub name: String,
    pub interface: String,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub features: Vec<SoftwareNamedValue>,
    pub switches: Vec<SoftwareDipSwitch>,
    pub areas: Vec<SoftwareArea>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipSwitch {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwareDipSwitchAttribute>>,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub name: String,
    pub tag: String,
    pub mask: String,
    pub values: Vec<SoftwareDipValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDipValue {
    pub attribute_positions: Vec<XmlAttributePosition<SoftwareDipValueAttribute>>,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub name: String,
    pub value: String,
    pub is_default: bool,
    pub default_specified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareDataWidth {
    Bits8,
    Bits16,
    Bits32,
    Bits64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareAreaFields {
    Data {
        size_text: String,
        size: Option<i64>,
        width: SoftwareDataWidth,
        width_specified: bool,
        endianness: SoftwareEndianness,
        endianness_specified: bool,
    },
    Disk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareArea {
    pub attribute_positions: SoftwareAreaAttributePositions,
    pub id: SoftwareAreaId,
    pub name: String,
    pub order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    pub fields: SoftwareAreaFields,
    pub entry_ids: Vec<OccurrenceId>,
}

#[derive(Debug, Error)]
pub enum SoftwareQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("snapshot lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("snapshot {0} is not a published MAME software-list edition")]
    NotPublishedSoftware(SnapshotKey),
    #[error("software list {0} does not belong to the requested snapshot")]
    ListNotInSnapshot(i64),
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to a different snapshot")]
    CursorSnapshotMismatch,
    #[error("cursor belongs to a different software list")]
    CursorListMismatch,
    #[error("invalid stored {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("native software metadata has inconsistent owner links for row {0}")]
    MismatchedOwner(i64),
    #[error("native software metadata is missing {field} for owner {owner}")]
    MissingNative { field: &'static str, owner: i64 },
    #[error("native software metadata has duplicate or invalid source positions for owner {owner}")]
    InvalidPositions { owner: i64 },
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
}

type QueryResult<T> = Result<T, SoftwareQueryError>;

#[derive(QueryableByName)]
struct SnapshotRow {
    #[diesel(sql_type = Binary)]
    registry_uuid: Vec<u8>,
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    source_key: String,
    #[diesel(sql_type = Text)]
    source_name: String,
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    catalog_name: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Text)]
    interpretation_key: String,
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = Text)]
    envelope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
    #[diesel(sql_type = BigInt)]
    wrapper_header_count: i64,
}

#[derive(QueryableByName)]
struct ListRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    notes: Option<String>,
    #[diesel(sql_type = BigInt)]
    order_value: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    column: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    group_snapshot_ok: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    group_kind: Option<String>,
}

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct TitleRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    order_value: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    clone_of: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    supported: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    supported_specified: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    publisher: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    notes: Option<String>,
}

#[derive(QueryableByName)]
struct NamedRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    row_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct PartRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    order_value: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    interface: String,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct FeatureRow {
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    row_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct SwitchRow {
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    row_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct DipValueRow {
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    row_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = BigInt)]
    is_default: i64,
    #[diesel(sql_type = BigInt)]
    default_specified: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[derive(QueryableByName)]
struct AreaRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    data_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = BigInt)]
    order_value: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    width: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    width_specified: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    endianness: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    endianness_specified: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    column: Option<i64>,
    #[diesel(sql_type = BigInt)]
    part_record_id: i64,
    #[diesel(sql_type = BigInt)]
    item_owner_id: i64,
}

#[derive(QueryableByName)]
struct EntryRow {
    #[diesel(sql_type = BigInt)]
    area_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    occurrence_record_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    claim_kind: Option<String>,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    area_record_id: i64,
    #[diesel(sql_type = Text)]
    area_kind: String,
}

#[derive(QueryableByName)]
struct OccurrenceOwnerViolation {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

/// Return one bounded page of lists from an exact published software-list edition.
pub fn lists_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    limit: SoftwarePageLimit,
    cursor: Option<&SoftwareListCursor>,
) -> QueryResult<SoftwareListPage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let generation = registry_id(connection)?;
        validate_list_cursor(cursor, generation, snapshot)?;
        let provenance = snapshot_row(connection, snapshot)?;
        let bound = limit
            .database_value()?
            .checked_add(1)
            .ok_or(SoftwareQueryError::PageLimitOverflow)?;
        let rows = match cursor {
            Some(cursor) => sql_query(queries::NEXT_LIST_PAGE)
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(cursor.order)
                .bind::<BigInt, _>(cursor.id.database_value())
                .bind::<BigInt, _>(bound)
                .load::<ListRow>(connection)?,
            None => sql_query(queries::FIRST_LIST_PAGE)
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(bound)
                .load::<ListRow>(connection)?,
        };
        let mut lists = rows
            .into_iter()
            .map(parse_list_row)
            .collect::<QueryResult<Vec<_>>>()?;
        let has_more = lists.len() > limit.0;
        if has_more {
            lists.pop();
        }
        let positions = load_list_positions(connection, &lists)?;
        for list in &mut lists {
            list.text_positions = positions
                .get(&list.id.database_value())
                .cloned()
                .unwrap_or_default();
            validate_notes_position(
                list.notes.is_some(),
                &list.text_positions,
                list.id.database_value(),
            )?;
        }
        attributes::lists(connection, &mut lists)?;
        let next_cursor = if has_more {
            lists.last().map(|list| SoftwareListCursor {
                generation,
                snapshot: snapshot.clone(),
                order: list.order,
                id: list.id,
            })
        } else {
            None
        };
        Ok(SoftwareListPage {
            snapshot: provenance,
            lists,
            next_cursor,
        })
    })
}

/// Return one bounded page of titles and all native metadata owned by those titles.
pub fn titles_for_list(
    database: &Database,
    snapshot: &SnapshotKey,
    list_id: SoftwareListId,
    limit: SoftwarePageLimit,
    cursor: Option<&SoftwareTitleCursor>,
) -> QueryResult<SoftwareTitlePage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let generation = registry_id(connection)?;
        validate_title_cursor(cursor, generation, snapshot, list_id)?;
        let provenance = snapshot_row(connection, snapshot)?;
        let list = selected_list(connection, snapshot, list_id)?;
        let bound = limit
            .database_value()?
            .checked_add(1)
            .ok_or(SoftwareQueryError::PageLimitOverflow)?;
        let title_rows = match cursor {
            Some(cursor) => sql_query(queries::NEXT_TITLE_PAGE)
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(list_id.database_value())
                .bind::<BigInt, _>(cursor.order)
                .bind::<BigInt, _>(cursor.id.as_i64())
                .bind::<BigInt, _>(bound)
                .load::<TitleRow>(connection)?,
            None => sql_query(queries::FIRST_TITLE_PAGE)
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(list_id.database_value())
                .bind::<BigInt, _>(bound)
                .load::<TitleRow>(connection)?,
        };
        let has_more = title_rows.len() > limit.0;
        let mut titles = title_rows
            .into_iter()
            .take(limit.0)
            .map(parse_title_row)
            .collect::<QueryResult<Vec<_>>>()?;
        create_requested_titles(connection)?;
        insert_requested_titles(connection, &titles)?;
        validate_occurrence_owners(connection)?;
        load_title_children(connection, &mut titles)?;
        drop_requested_titles(connection)?;
        let next_cursor = if has_more {
            titles.last().map(|title| SoftwareTitleCursor {
                generation,
                snapshot: snapshot.clone(),
                list: list_id,
                order: title.order,
                id: title.id,
            })
        } else {
            None
        };
        Ok(SoftwareTitlePage {
            snapshot: provenance,
            list,
            titles,
            next_cursor,
        })
    })
}

fn snapshot_row(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> QueryResult<SoftwareSnapshot> {
    let row = sql_query(queries::PUBLISHED_SNAPSHOT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotRow>(connection)
        .map_err(|error| match error {
            diesel::result::Error::NotFound => {
                SoftwareQueryError::NotPublishedSoftware(snapshot.clone())
            }
            other => SoftwareQueryError::Database(other),
        })?;
    if row.snapshot_key != snapshot.as_str() {
        return Err(SoftwareQueryError::NotPublishedSoftware(snapshot.clone()));
    }
    let envelope = match (
        row.envelope_kind.as_str(),
        row.wrapper_header_count,
        row.build,
    ) {
        ("single_list", 0, None) => SoftwareEnvelope::SingleList,
        ("plural_lists", 1, build) => SoftwareEnvelope::PluralLists { build },
        _ => {
            return Err(invalid(
                "software envelope/header combination",
                row.envelope_kind,
            ));
        }
    };
    let wrapper_attribute_positions = attributes::wrapper(connection, snapshot, &envelope)?;
    let registry_bytes: [u8; 16] = row
        .registry_uuid
        .try_into()
        .map_err(|bytes: Vec<u8>| invalid("registry UUID length", bytes.len().to_string()))?;
    Ok(SoftwareSnapshot {
        registry_id: CatalogRegistryId::from_bytes(registry_bytes),
        snapshot_key: SnapshotKey::from_persisted(row.snapshot_key),
        source_key: PublishingSourceKey::new(row.source_key),
        source_name: row.source_name,
        catalog_key: CatalogKey::new(row.catalog_key),
        catalog_name: row.catalog_name,
        document_key: row.document_key.parse()?,
        interpretation_key: ParserInterpretationKey::from_persisted(row.interpretation_key),
        format: row.format,
        envelope,
        wrapper_attribute_positions,
    })
}

fn parse_list_row(row: ListRow) -> QueryResult<SoftwareList> {
    if row.group_snapshot_ok != Some(1) || row.group_kind.as_deref() != Some("software_list") {
        return Err(SoftwareQueryError::MismatchedOwner(row.id));
    }
    require_native_owner(row.native_id, row.id, "software list")?;
    Ok(SoftwareList {
        id: SoftwareListId::from_database(row.id),
        name: required_value(row.name, "software list name", row.id)?,
        description: row.description,
        notes: row.notes,
        order: row.order_value,
        source_order: required_value(row.source_order, "software list source order", row.id)?,
        location: SourceLocation {
            line: required_value(row.line, "software list line", row.id)?,
            column: required_value(row.column, "software list column", row.id)?,
        },
        text_positions: Vec::new(),
        attribute_positions: Vec::new(),
    })
}

fn selected_list(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    id: SoftwareListId,
) -> QueryResult<SoftwareList> {
    let rows = sql_query(queries::SELECTED_LIST)
        .bind::<Text, _>(snapshot.as_str())
        .bind::<BigInt, _>(id.database_value())
        .load::<ListRow>(connection)?;
    let Some(row) = rows.into_iter().next() else {
        return Err(SoftwareQueryError::ListNotInSnapshot(id.database_value()));
    };
    let mut list = parse_list_row(row)?;
    load_owner_request_table(connection, &[id.database_value()])?;
    let positions = sql_query(queries::LIST_TEXT_POSITIONS).load::<PositionRow>(connection)?;
    drop_owner_request_table(connection)?;
    list.text_positions = parse_positions(positions, id.database_value())?;
    validate_notes_position(
        list.notes.is_some(),
        &list.text_positions,
        id.database_value(),
    )?;
    attributes::lists(connection, std::slice::from_mut(&mut list))?;
    Ok(list)
}

fn load_list_positions(
    connection: &mut SqliteConnection,
    lists: &[SoftwareList],
) -> QueryResult<BTreeMap<i64, Vec<SoftwareTextPosition>>> {
    let ids = lists
        .iter()
        .map(|list| list.id.database_value())
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    load_owner_request_table(connection, &ids)?;
    let rows = sql_query(queries::LIST_TEXT_POSITIONS).load::<PositionRow>(connection)?;
    drop_owner_request_table(connection)?;
    let mut grouped = BTreeMap::<i64, Vec<PositionRow>>::new();
    for row in rows {
        grouped.entry(row.owner_id).or_default().push(row);
    }
    grouped
        .into_iter()
        .map(|(owner, rows)| Ok((owner, parse_positions(rows, owner)?)))
        .collect()
}

fn parse_positions(rows: Vec<PositionRow>, owner: i64) -> QueryResult<Vec<SoftwareTextPosition>> {
    let mut previous = None;
    rows.into_iter()
        .map(|row| {
            if row.owner_id != owner || previous.is_some_and(|order| order >= row.source_order) {
                return Err(SoftwareQueryError::InvalidPositions { owner });
            }
            previous = Some(row.source_order);
            let field = match row.field_kind {
                0 => SoftwareTextField::Description,
                1 => SoftwareTextField::Year,
                2 => SoftwareTextField::Publisher,
                3 => SoftwareTextField::Notes,
                value => return Err(invalid("software text field kind", value.to_string())),
            };
            Ok(SoftwareTextPosition {
                field,
                source_order: row.source_order,
                location: SourceLocation {
                    line: row.line,
                    column: row.column,
                },
            })
        })
        .collect()
}

fn validate_notes_position(
    present: bool,
    positions: &[SoftwareTextPosition],
    owner: i64,
) -> QueryResult<()> {
    let count = positions
        .iter()
        .filter(|position| position.field == SoftwareTextField::Notes)
        .count();
    if (count == 1) != present || count > 1 {
        return Err(SoftwareQueryError::InvalidPositions { owner });
    }
    Ok(())
}

fn parse_title_row(row: TitleRow) -> QueryResult<SoftwareTitle> {
    require_native_owner(row.native_id, row.id, "software item")?;
    let supported = required_value(row.supported, "software supported status", row.id)?;
    Ok(SoftwareTitle {
        id: CatalogSetId::from_database(row.id),
        name: row.name,
        order: row.order_value,
        source_order: required_value(row.source_order, "software item source order", row.id)?,
        location: SourceLocation {
            line: row.line,
            column: row.column,
        },
        clone_of: row.clone_of,
        supported: match supported.as_str() {
            "yes" => SoftwareSupportedStatus::Yes,
            "partial" => SoftwareSupportedStatus::Partial,
            "no" => SoftwareSupportedStatus::No,
            value => return Err(invalid("software supported status", value.to_owned())),
        },
        supported_specified: stored_bool(
            required_value(
                row.supported_specified,
                "software supported_specified",
                row.id,
            )?,
            "software supported_specified",
        )?,
        description: required_value(row.description, "software description", row.id)?,
        year: required_value(row.year, "software year", row.id)?,
        publisher: required_value(row.publisher, "software publisher", row.id)?,
        notes: row.notes,
        text_positions: Vec::new(),
        info: Vec::new(),
        attribute_positions: Vec::new(),
        shared_features: Vec::new(),
        parts: Vec::new(),
    })
}

fn required_value<T>(value: Option<T>, field: &'static str, owner: i64) -> QueryResult<T> {
    value.ok_or(SoftwareQueryError::MissingNative { field, owner })
}

fn require_native_owner(value: Option<i64>, owner: i64, field: &'static str) -> QueryResult<()> {
    if required_value(value, field, owner)? != owner {
        return Err(SoftwareQueryError::MismatchedOwner(owner));
    }
    Ok(())
}

fn create_requested_titles(connection: &mut SqliteConnection) -> QueryResult<()> {
    connection.batch_execute("CREATE TEMP TABLE IF NOT EXISTS catalog_software_requested_owners (owner_id INTEGER PRIMARY KEY) WITHOUT ROWID; DELETE FROM temp.catalog_software_requested_owners")?;
    Ok(())
}

fn insert_requested_titles(
    connection: &mut SqliteConnection,
    titles: &[SoftwareTitle],
) -> QueryResult<()> {
    for title in titles {
        sql_query("INSERT INTO temp.catalog_software_requested_owners(owner_id) VALUES (?)")
            .bind::<BigInt, _>(title.id.as_i64())
            .execute(connection)?;
    }
    Ok(())
}

fn load_owner_request_table(connection: &mut SqliteConnection, ids: &[i64]) -> QueryResult<()> {
    create_requested_titles(connection)?;
    for id in ids {
        sql_query("INSERT INTO temp.catalog_software_requested_owners(owner_id) VALUES (?)")
            .bind::<BigInt, _>(*id)
            .execute(connection)?;
    }
    Ok(())
}

fn drop_requested_titles(connection: &mut SqliteConnection) -> QueryResult<()> {
    connection.batch_execute("DROP TABLE temp.catalog_software_requested_owners")?;
    Ok(())
}

fn drop_owner_request_table(connection: &mut SqliteConnection) -> QueryResult<()> {
    drop_requested_titles(connection)
}

fn validate_occurrence_owners(connection: &mut SqliteConnection) -> QueryResult<()> {
    // Start from shared occurrences, independently of the native area traversal.
    // A selected title with no surviving parts must not hide orphan occurrences.
    if let Some(violation) = sql_query(queries::INVALID_OCCURRENCE_OWNER)
        .get_result::<OccurrenceOwnerViolation>(connection)
        .optional()?
    {
        return Err(SoftwareQueryError::MismatchedOwner(violation.occurrence_id));
    }
    Ok(())
}

fn load_title_children(
    connection: &mut SqliteConnection,
    titles: &mut [SoftwareTitle],
) -> QueryResult<()> {
    if titles.is_empty() {
        return Ok(());
    }
    let mut attribute_positions = attributes::titles(connection)?;
    let mut title_ids = BTreeMap::<i64, usize>::new();
    for (index, title) in titles.iter().enumerate() {
        title_ids.insert(title.id.as_i64(), index);
    }

    let rows = sql_query(queries::TITLE_TEXT_POSITIONS).load::<PositionRow>(connection)?;
    let mut groups = group_positions(rows);
    for title in titles.iter_mut() {
        title.text_positions = parse_positions(
            groups.remove(&title.id.as_i64()).unwrap_or_default(),
            title.id.as_i64(),
        )?;
        validate_notes_position(
            title.notes.is_some(),
            &title.text_positions,
            title.id.as_i64(),
        )?;
        for field in [
            SoftwareTextField::Description,
            SoftwareTextField::Year,
            SoftwareTextField::Publisher,
        ] {
            if title
                .text_positions
                .iter()
                .filter(|position| position.field == field)
                .count()
                != 1
            {
                return Err(SoftwareQueryError::InvalidPositions {
                    owner: title.id.as_i64(),
                });
            }
        }
    }

    for family in [TitleValueFamily::Info, TitleValueFamily::SharedFeature] {
        let rows = sql_query(family.query()).load::<NamedRow>(connection)?;
        let grouped = group_named(rows, &title_ids)?;
        for (owner, values) in grouped {
            let title = titles
                .get_mut(
                    *title_ids
                        .get(&owner)
                        .ok_or(SoftwareQueryError::MismatchedOwner(owner))?,
                )
                .ok_or(SoftwareQueryError::MismatchedOwner(owner))?;
            *match family {
                TitleValueFamily::Info => &mut title.info,
                TitleValueFamily::SharedFeature => &mut title.shared_features,
            } = values;
        }
    }

    let parts = sql_query(queries::PARTS).load::<PartRow>(connection)?;
    let mut part_records = BTreeMap::<i64, i64>::new();
    let mut part_index = BTreeMap::<i64, (usize, usize)>::new();
    for row in parts {
        let Some(&title_index) = title_ids.get(&row.record_id) else {
            return Err(SoftwareQueryError::MismatchedOwner(row.id));
        };
        part_records.insert(row.id, row.record_id);
        let title = titles
            .get_mut(title_index)
            .ok_or(SoftwareQueryError::MismatchedOwner(row.record_id))?;
        let index = title.parts.len();
        title.parts.push(SoftwarePart {
            id: SoftwarePartId::from_database(row.id),
            name: row.name,
            interface: row.interface,
            order: row.order_value,
            source_order: row.source_order,
            location: SourceLocation {
                line: row.line,
                column: row.column,
            },
            features: Vec::new(),
            attribute_positions: Vec::new(),
            switches: Vec::new(),
            areas: Vec::new(),
        });
        part_index.insert(row.id, (title_index, index));
    }
    load_part_features(connection, &part_index, titles)?;
    load_switches(connection, &part_index, titles)?;
    load_areas(connection, &part_index, &part_records, titles)?;
    attributes::attach_titles(&mut attribute_positions, titles)?;
    attribute_positions.finish()?;
    Ok(())
}

#[derive(Clone, Copy)]
enum TitleValueFamily {
    Info,
    SharedFeature,
}

impl TitleValueFamily {
    const fn query(self) -> &'static str {
        match self {
            Self::Info => queries::TITLE_INFO,
            Self::SharedFeature => queries::TITLE_SHARED_FEATURES,
        }
    }
}

fn group_positions(rows: Vec<PositionRow>) -> BTreeMap<i64, Vec<PositionRow>> {
    let mut result = BTreeMap::<i64, Vec<PositionRow>>::new();
    for row in rows {
        result.entry(row.owner_id).or_default().push(row);
    }
    result
}

fn group_named(
    rows: Vec<NamedRow>,
    owners: &BTreeMap<i64, usize>,
) -> QueryResult<BTreeMap<i64, Vec<SoftwareNamedValue>>> {
    let mut result = BTreeMap::<i64, Vec<SoftwareNamedValue>>::new();
    for row in rows {
        if !owners.contains_key(&row.owner_id) {
            return Err(SoftwareQueryError::MismatchedOwner(row.owner_id));
        }
        let values = result.entry(row.owner_id).or_default();
        if usize::try_from(row.row_order).ok() != Some(values.len()) {
            return Err(SoftwareQueryError::InvalidPositions {
                owner: row.owner_id,
            });
        }
        values.push(SoftwareNamedValue {
            attribute_positions: Vec::new(),
            order: row.row_order,
            source_order: row.source_order,
            location: SourceLocation {
                line: row.line,
                column: row.column,
            },
            name: row.name,
            value: row.value,
        });
    }
    Ok(result)
}

fn load_part_features(
    connection: &mut SqliteConnection,
    part_index: &BTreeMap<i64, (usize, usize)>,
    titles: &mut [SoftwareTitle],
) -> QueryResult<()> {
    if part_index.is_empty() {
        return Ok(());
    }
    let rows = sql_query(queries::PART_FEATURES).load::<FeatureRow>(connection)?;
    let mut counts = BTreeMap::<i64, i64>::new();
    for row in rows {
        let (title_index, part_no) = *part_index
            .get(&row.part_id)
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        let expected = counts.entry(row.part_id).or_default();
        if row.row_order != *expected {
            return Err(SoftwareQueryError::InvalidPositions { owner: row.part_id });
        }
        *expected += 1;
        let part = titles
            .get_mut(title_index)
            .and_then(|title| title.parts.get_mut(part_no))
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        part.features.push(SoftwareNamedValue {
            attribute_positions: Vec::new(),
            order: row.row_order,
            source_order: row.source_order,
            location: SourceLocation {
                line: row.line,
                column: row.column,
            },
            name: row.name,
            value: row.value,
        });
    }
    Ok(())
}

fn load_switches(
    connection: &mut SqliteConnection,
    part_index: &BTreeMap<i64, (usize, usize)>,
    titles: &mut [SoftwareTitle],
) -> QueryResult<()> {
    if part_index.is_empty() {
        return Ok(());
    }
    let switches = sql_query(queries::DIP_SWITCHES).load::<SwitchRow>(connection)?;
    let mut index = BTreeMap::<(i64, i64), (usize, usize, usize)>::new();
    let mut counts = BTreeMap::<i64, i64>::new();
    for row in switches {
        let (title_index, part_no) = *part_index
            .get(&row.part_id)
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        let expected = counts.entry(row.part_id).or_default();
        if row.row_order != *expected {
            return Err(SoftwareQueryError::InvalidPositions { owner: row.part_id });
        }
        *expected += 1;
        let part = titles
            .get_mut(title_index)
            .and_then(|title| title.parts.get_mut(part_no))
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        let switch_no = part.switches.len();
        part.switches.push(SoftwareDipSwitch {
            attribute_positions: Vec::new(),
            order: row.row_order,
            source_order: row.source_order,
            location: SourceLocation {
                line: row.line,
                column: row.column,
            },
            name: row.name,
            tag: row.tag,
            mask: row.mask,
            values: Vec::new(),
        });
        index.insert(
            (row.part_id, row.row_order),
            (title_index, part_no, switch_no),
        );
    }
    let values = sql_query(queries::DIP_VALUES).load::<DipValueRow>(connection)?;
    let mut value_counts = BTreeMap::<(i64, i64), i64>::new();
    for row in values {
        let key = (row.part_id, row.switch_order);
        let (title_index, part_no, switch_no) = *index
            .get(&key)
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        let expected = value_counts.entry(key).or_default();
        if row.row_order != *expected {
            return Err(SoftwareQueryError::InvalidPositions { owner: row.part_id });
        }
        *expected += 1;
        let switch = titles
            .get_mut(title_index)
            .and_then(|title| title.parts.get_mut(part_no))
            .and_then(|part| part.switches.get_mut(switch_no))
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        switch.values.push(SoftwareDipValue {
            attribute_positions: Vec::new(),
            order: row.row_order,
            source_order: row.source_order,
            location: SourceLocation {
                line: row.line,
                column: row.column,
            },
            name: row.name,
            value: row.value,
            is_default: stored_bool(row.is_default, "software DIP value is_default")?,
            default_specified: stored_bool(
                row.default_specified,
                "software DIP value default_specified",
            )?,
        });
    }
    Ok(())
}

fn load_areas(
    connection: &mut SqliteConnection,
    part_index: &BTreeMap<i64, (usize, usize)>,
    part_records: &BTreeMap<i64, i64>,
    titles: &mut [SoftwareTitle],
) -> QueryResult<()> {
    if part_index.is_empty() {
        return Ok(());
    }
    let area_rows = sql_query(queries::AREAS).load::<AreaRow>(connection)?;
    let mut area_index = BTreeMap::<i64, (usize, usize, usize)>::new();
    for row in area_rows {
        let Some(&(title_index, part_no)) = part_index.get(&row.part_id) else {
            return Err(SoftwareQueryError::MismatchedOwner(row.id));
        };
        if row.record_id != row.part_record_id
            || row.record_id != row.item_owner_id
            || part_records.get(&row.part_id) != Some(&row.record_id)
        {
            return Err(SoftwareQueryError::MismatchedOwner(row.id));
        }
        let title = titles
            .get_mut(title_index)
            .and_then(|title| title.parts.get_mut(part_no))
            .ok_or(SoftwareQueryError::MismatchedOwner(row.part_id))?;
        let area_no = title.areas.len();
        area_index.insert(row.id, (title_index, part_no, area_no));
        title.areas.push(parse_area_row(row)?);
    }
    let area_ids = area_index.keys().copied().collect::<Vec<_>>();
    if area_ids.is_empty() {
        return Ok(());
    }
    replace_owner_request_table(connection, &area_ids)?;
    let roms = sql_query(queries::ROM_ENTRIES).load::<EntryRow>(connection)?;
    let disks = sql_query(queries::DISK_ENTRIES).load::<EntryRow>(connection)?;
    let mut entries = BTreeMap::<i64, Vec<(i64, i64, i64, String)>>::new();
    for (rows, expected_kind) in [(roms, "data"), (disks, "disk")] {
        for row in rows {
            let occurrence_record_id = required_value(
                row.occurrence_record_id,
                "software entry occurrence owner",
                row.occurrence_id,
            )?;
            let claim_kind = required_value(
                row.claim_kind,
                "software entry occurrence kind",
                row.occurrence_id,
            )?;
            if row.record_id != occurrence_record_id || row.record_id != row.area_record_id {
                return Err(SoftwareQueryError::MismatchedOwner(row.occurrence_id));
            }
            if row.area_kind != expected_kind
                || (expected_kind == "disk" && claim_kind != "software_disk_entry")
                || (expected_kind == "data"
                    && !matches!(
                        claim_kind.as_str(),
                        "software_rom_entry" | "software_rom_operation"
                    ))
            {
                return Err(SoftwareQueryError::MismatchedOwner(row.occurrence_id));
            }
            entries.entry(row.area_id).or_default().push((
                row.source_order,
                row.component_order,
                row.occurrence_id,
                claim_kind,
            ));
        }
    }
    for (area_id, values) in &mut entries {
        values.sort_by_key(|value| value.0);
        let area = area_index
            .get(area_id)
            .and_then(|(title, part, index)| {
                titles
                    .get_mut(*title)
                    .and_then(|title| title.parts.get_mut(*part))
                    .and_then(|part| part.areas.get_mut(*index))
            })
            .ok_or(SoftwareQueryError::MismatchedOwner(*area_id))?;
        for (_, _, occurrence, _) in values {
            area.entry_ids
                .push(OccurrenceId::from_database(*occurrence));
        }
    }
    Ok(())
}

fn parse_area_row(row: AreaRow) -> QueryResult<SoftwareArea> {
    let fields = match row.kind.as_str() {
        "data" => {
            require_native_owner(row.data_id, row.id, "software data area")?;
            if super::software_area::native_kind(row.id, row.data_id, row.disk_id)
                != Some(crate::mame_softwarelist::AreaKind::Data)
            {
                return Err(SoftwareQueryError::MismatchedOwner(row.id));
            }
            SoftwareAreaFields::Data {
                size_text: required_value(row.size_text, "data area declared_size_text", row.id)?,
                size: row.size,
                width: parse_width(required_value(row.width, "data area width", row.id)?)?,
                width_specified: stored_bool(
                    required_value(row.width_specified, "data area width_specified", row.id)?,
                    "software area width_specified",
                )?,
                endianness: parse_endianness(required_value(
                    row.endianness,
                    "data area endianness",
                    row.id,
                )?)?,
                endianness_specified: stored_bool(
                    required_value(
                        row.endianness_specified,
                        "data area endianness_specified",
                        row.id,
                    )?,
                    "software area endianness_specified",
                )?,
            }
        }
        "disk" => {
            require_native_owner(row.disk_id, row.id, "software disk area")?;
            if super::software_area::native_kind(row.id, row.data_id, row.disk_id)
                != Some(crate::mame_softwarelist::AreaKind::Disk)
                || row.size_text.is_some()
                || row.size.is_some()
                || row.width.is_some()
                || row.width_specified.is_some()
                || row.endianness.is_some()
                || row.endianness_specified.is_some()
            {
                return Err(SoftwareQueryError::MismatchedOwner(row.id));
            }
            SoftwareAreaFields::Disk
        }
        value => return Err(invalid("software area kind", value.to_owned())),
    };
    Ok(SoftwareArea {
        attribute_positions: match &fields {
            SoftwareAreaFields::Data { .. } => SoftwareAreaAttributePositions::Data(Vec::new()),
            SoftwareAreaFields::Disk => SoftwareAreaAttributePositions::Disk(Vec::new()),
        },
        id: SoftwareAreaId::from_database(row.id),
        name: required_value(row.name, "software area name", row.id)?,
        order: row.order_value,
        source_order: required_value(row.source_order, "software area source order", row.id)?,
        location: SourceLocation {
            line: required_value(row.line, "software area source line", row.id)?,
            column: required_value(row.column, "software area source column", row.id)?,
        },
        fields,
        entry_ids: Vec::new(),
    })
}

fn replace_owner_request_table(connection: &mut SqliteConnection, ids: &[i64]) -> QueryResult<()> {
    connection.batch_execute("DELETE FROM temp.catalog_software_requested_owners")?;
    for id in ids {
        sql_query("INSERT INTO temp.catalog_software_requested_owners(owner_id) VALUES (?)")
            .bind::<BigInt, _>(*id)
            .execute(connection)?;
    }
    Ok(())
}

fn parse_width(value: i64) -> QueryResult<SoftwareDataWidth> {
    match value {
        8 => Ok(SoftwareDataWidth::Bits8),
        16 => Ok(SoftwareDataWidth::Bits16),
        32 => Ok(SoftwareDataWidth::Bits32),
        64 => Ok(SoftwareDataWidth::Bits64),
        other => Err(invalid("software data width", other.to_string())),
    }
}

fn parse_endianness(value: String) -> QueryResult<SoftwareEndianness> {
    match value.as_str() {
        "little" => Ok(SoftwareEndianness::Little),
        "big" => Ok(SoftwareEndianness::Big),
        _ => Err(invalid("software endianness", value)),
    }
}

fn validate_list_cursor(
    cursor: Option<&SoftwareListCursor>,
    generation: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.generation != generation {
            return Err(SoftwareQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot != snapshot {
            return Err(SoftwareQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn validate_title_cursor(
    cursor: Option<&SoftwareTitleCursor>,
    generation: CatalogRegistryId,
    snapshot: &SnapshotKey,
    list: SoftwareListId,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.generation != generation {
            return Err(SoftwareQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot != snapshot {
            return Err(SoftwareQueryError::CursorSnapshotMismatch);
        }
        if cursor.list != list {
            return Err(SoftwareQueryError::CursorListMismatch);
        }
    }
    Ok(())
}

fn stored_bool(value: i64, field: &'static str) -> QueryResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(invalid(field, other.to_string())),
    }
}

const fn invalid(field: &'static str, value: String) -> SoftwareQueryError {
    SoftwareQueryError::InvalidStoredValue { field, value }
}
