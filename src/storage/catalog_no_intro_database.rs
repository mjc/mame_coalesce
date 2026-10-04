//! Source-free queries over exact published No-Intro database exports.
//!
//! Pages limit the number of games, not the number of children within a game.
//! Every returned game includes its archive, dump-source and release owners in
//! source order. File references share the occurrence IDs used by
//! [`crate::catalog_files`]; their payloads are not copied into another model.
//! Originals remain external and are never reopened by these queries.

mod children;
mod positions;
mod queries;
mod reader;

use thiserror::Error;

pub use crate::storage::no_intro_database_fields::{
    NoIntroDatabaseArchiveField, NoIntroDatabaseDumpDetailsField, NoIntroDatabaseDumpFileField,
    NoIntroDatabaseDumpSerialsField, NoIntroDatabaseReleaseDetailsField,
    NoIntroDatabaseReleaseFileField, NoIntroDatabaseReleaseSerialsField,
};

use crate::{
    domain::{
        CatalogRegistryId, CatalogSetId, DocumentKey, NoIntroArchiveId, ParserInterpretationKey,
        PublishingSourceKey, SnapshotKey,
    },
    logiqx::{AttributePosition, RecordLocation},
    no_intro_db_xml::{
        ArchiveDescription, NoIntroDatabaseDocument, ReleaseSerials, SourceDetails, SourceSerials,
    },
    storage::catalog_files::{
        NoIntroDatabaseDigestValue, NoIntroDumpSourceId, NoIntroReleaseId, OccurrenceId,
    },
};

const MAX_PAGE_SIZE: usize = 500;

/// A validated maximum number of No-Intro database games returned by one query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoIntroDatabasePageLimit(usize);

impl NoIntroDatabasePageLimit {
    /// Create a page limit in the supported range `1..=500`.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested limit is outside that range.
    pub fn new(value: usize) -> Result<Self, NoIntroDatabaseQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(NoIntroDatabaseQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }
}

/// Continuation for a page, pinned to the published registry and exact snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseCursor {
    registry_id: CatalogRegistryId,
    snapshot_key: SnapshotKey,
    list_order: i64,
    owner_id: CatalogSetId,
}

/// Publication provenance for one queried No-Intro database snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseSnapshot {
    pub registry_id: CatalogRegistryId,
    pub snapshot_key: SnapshotKey,
    pub source_key: PublishingSourceKey,
    pub source_name: String,
    pub catalog_key: crate::domain::CatalogKey,
    pub catalog_name: String,
    pub document_key: DocumentKey,
    pub interpretation_key: ParserInterpretationKey,
    pub format: String,
    pub declared_version: Option<String>,
}

/// One bounded game page with native document facts and publication provenance.
#[derive(Debug, PartialEq, Eq)]
pub struct NoIntroDatabasePage {
    pub snapshot: NoIntroDatabaseSnapshot,
    pub document: NoIntroDatabaseDocument,
    pub games: Vec<NoIntroDatabaseGame>,
    pub next_cursor: Option<NoIntroDatabaseCursor>,
}

/// One game and its complete native child sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseGame {
    pub id: CatalogSetId,
    pub list_order: i64,
    pub location: RecordLocation,
    pub name: crate::xml_reader::DeclaredText,
    pub children: Vec<NoIntroDatabaseGameChild>,
}

/// Archive, dump source, and release owners in original game-child order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoIntroDatabaseGameChild {
    Archive(NoIntroDatabaseArchive),
    DumpSource(NoIntroDatabaseDumpSource),
    Release(NoIntroDatabaseRelease),
}

impl NoIntroDatabaseGameChild {
    #[must_use]
    pub const fn source_order(&self) -> i64 {
        match self {
            Self::Archive(value) => value.source_order,
            Self::DumpSource(value) => value.source_order,
            Self::Release(value) => value.source_order,
        }
    }
}

/// One archive declaration, including the actual reported relationship IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseArchive {
    pub id: NoIntroArchiveId,
    pub source_order: i64,
    pub location: RecordLocation,
    pub description: ArchiveDescription,
    pub clone_relationship_id: Option<i64>,
    pub merge_relationship_id: Option<i64>,
    pub attribute_positions: Vec<AttributePosition<NoIntroDatabaseArchiveField>>,
}

/// One dump source and its independently ordered optional details, serials, and files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseDumpSource {
    pub id: NoIntroDumpSourceId,
    pub source_order: i64,
    pub location: RecordLocation,
    pub details: Option<SourceDetails>,
    pub serials: Option<SourceSerials>,
    pub details_attribute_positions: Vec<AttributePosition<NoIntroDatabaseDumpDetailsField>>,
    pub serials_attribute_positions: Vec<AttributePosition<NoIntroDatabaseDumpSerialsField>>,
    pub files: Vec<NoIntroDatabaseFileReference<NoIntroDatabaseDumpFileField>>,
}

/// One release and its independently ordered optional details, serials, and files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseRelease {
    pub id: NoIntroReleaseId,
    pub source_order: i64,
    pub location: RecordLocation,
    pub details: Option<NoIntroDatabaseReleaseDetails>,
    pub serials: Option<ReleaseSerials>,
    pub serials_attribute_positions: Vec<AttributePosition<NoIntroDatabaseReleaseSerialsField>>,
    pub files: Vec<NoIntroDatabaseFileReference<NoIntroDatabaseReleaseFileField>>,
}

/// Release details with typed NFO digests that do not invent a valid raw spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseReleaseDetails {
    pub source_order: i64,
    pub location: RecordLocation,
    pub opening_end: RecordLocation,
    pub archivename: Option<String>,
    pub category: Option<String>,
    pub comment: Option<String>,
    pub date: Option<String>,
    pub dirname: Option<String>,
    pub group: Option<String>,
    pub id: Option<String>,
    pub nfo_crc32: Option<NoIntroDatabaseDigestValue>,
    pub nfo_size: Option<String>,
    pub nfocrc: Option<NoIntroDatabaseDigestValue>,
    pub nfoname: Option<String>,
    pub nfosize: Option<String>,
    pub origin: Option<String>,
    pub originalformat: Option<String>,
    pub region: Option<String>,
    pub rominfo: Option<String>,
    pub tool: Option<String>,
    pub attribute_positions: Vec<AttributePosition<NoIntroDatabaseReleaseDetailsField>>,
}

/// A file occurrence reference; callers can hydrate payloads through `catalog_files`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatabaseFileReference<Field> {
    pub occurrence_id: OccurrenceId,
    /// Ordinal among every file sibling of this game, including the other owner kind.
    pub occurrence_order: i64,
    /// Ordinal among this source or release's mixed details, serials and file children.
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<Field>>,
}

/// Errors returned by native No-Intro database queries.
#[derive(Debug, Error)]
pub enum NoIntroDatabaseQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("snapshot lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("snapshot {0} is not a published No-Intro database export")]
    NotPublished(SnapshotKey),
    #[error("native No-Intro document facts are missing for snapshot {0}")]
    MissingDocument(SnapshotKey),
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to a different snapshot")]
    CursorSnapshotMismatch,
    #[error("native No-Intro metadata is inconsistent for owner {0}")]
    InvalidMetadata(i64),
    #[error("native No-Intro metadata is missing {field} for owner {owner}")]
    MissingNative { field: &'static str, owner: i64 },
    #[error("native No-Intro metadata has mismatched owner links for row {0}")]
    MismatchedOwner(i64),
    #[error("invalid stored {field} for owner {owner}: {value}")]
    InvalidStoredValue {
        field: &'static str,
        owner: i64,
        value: String,
    },
    #[error("source positions are inconsistent for owner {0}")]
    InvalidPositions(i64),
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
}

/// Return one bounded page from an exact published No-Intro database snapshot.
///
/// Document, provenance and descendants are read in one database transaction.
/// A continuation belongs to the same snapshot and registry generation, and
/// requires its actual last game owner to remain present. Paired backups retain
/// that generation; a fresh database rebuild does not.
///
/// # Errors
///
/// Returns an error for an unpublished or non-export snapshot, a mismatched
/// continuation, inconsistent native facts, or a database access failure.
pub fn games_for_snapshot(
    database: &crate::database::Database,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatabaseCursor>,
    limit: NoIntroDatabasePageLimit,
) -> Result<NoIntroDatabasePage, NoIntroDatabaseQueryError> {
    reader::games_for_snapshot(database, snapshot, cursor, limit)
}
