//! Source-free metadata queries for exact published flat No-Intro DAT snapshots.
//!
//! Game pages are bounded; each selected game includes all supported children.
//! Raw source ordinals may have gaps where compatible parsing skipped vendor
//! elements. Declared text is retained independently from effective directives.

mod children;
mod header;
mod positions;
mod queries;
mod reader;

#[cfg(test)]
mod queries_tests;

use thiserror::Error;

pub use crate::xml_reader::DeclaredText;

pub use crate::storage::no_intro_dat_fields::{
    NoIntroDatClrMameProField, NoIntroDatGameField, NoIntroDatHeaderField,
    NoIntroDatRomCenterField, NoIntroDatRomField,
};

use crate::{
    NoIntroDatMode,
    domain::{
        CatalogKey, CatalogRegistryId, CatalogSetId, DocumentKey, ParserInterpretationKey,
        PublishingSourceKey, SnapshotKey,
    },
    logiqx::{AttributePosition, RecordLocation},
    storage::catalog_files::{NoIntroDatRomPayload, OccurrenceId},
};

const MAX_PAGE_SIZE: usize = 500;

/// Checked game count per page; complete children are never truncated.
///
/// ```
/// use mame_coalesce::catalog_no_intro_dat::NoIntroDatPageLimit;
/// assert!(NoIntroDatPageLimit::new(1).is_ok());
/// assert!(NoIntroDatPageLimit::new(500).is_ok());
/// assert!(NoIntroDatPageLimit::new(0).is_err());
/// assert!(NoIntroDatPageLimit::new(501).is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoIntroDatPageLimit(usize);

impl NoIntroDatPageLimit {
    /// Accept a page size in `1..=500`.
    ///
    /// # Errors
    /// Returns an error for zero or a size above 500.
    pub fn new(value: usize) -> Result<Self, NoIntroDatQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(NoIntroDatQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }
}

/// Opaque continuation pinned to a registry generation and published snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatCursor {
    registry_id: CatalogRegistryId,
    snapshot_key: SnapshotKey,
    list_order: i64,
    owner_id: CatalogSetId,
}

/// Publication provenance; identical names do not imply identical snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatSnapshot {
    pub registry_id: CatalogRegistryId,
    pub snapshot_key: SnapshotKey,
    pub source_key: PublishingSourceKey,
    pub source_name: String,
    pub catalog_key: CatalogKey,
    pub catalog_name: String,
    pub document_key: DocumentKey,
    pub interpretation_key: ParserInterpretationKey,
    pub format: String,
    pub declared_version: Option<String>,
}

/// One bounded game page and complete native document/header facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatPage {
    pub snapshot: NoIntroDatSnapshot,
    pub document: NoIntroDatDocument,
    pub games: Vec<NoIntroDatGame>,
    pub next_cursor: Option<NoIntroDatCursor>,
}

/// The actual parsing mode and retained datafile envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatDocument {
    pub mode: NoIntroDatMode,
    pub schema_location: Option<String>,
    pub location: RecordLocation,
    pub header: NoIntroDatHeader,
}

/// Required header owner, including present-empty directives in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatHeader {
    pub source_order: usize,
    pub location: RecordLocation,
    pub children: Vec<NoIntroDatHeaderChild>,
}

/// Supported text fields and directives in the header's mixed child sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoIntroDatHeaderChild {
    Text {
        field: NoIntroDatHeaderField,
        value: DeclaredText,
    },
    ClrMamePro(NoIntroDatClrMameProDirective),
    RomCenter(NoIntroDatRomCenterDirective),
}

impl NoIntroDatHeaderChild {
    #[must_use]
    pub const fn source_order(&self) -> usize {
        match self {
            Self::Text { value, .. } => value.source_order,
            Self::ClrMamePro(value) => value.source_order,
            Self::RomCenter(value) => value.source_order,
        }
    }
}

/// An effective known `forcenodump` token, never a replacement for the declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoIntroDatForceNoDump {
    Obsolete,
    Required,
    Ignore,
}

/// An optional, possibly empty, `ClrMamePro` header directive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatClrMameProDirective {
    pub source_order: usize,
    pub location: RecordLocation,
    pub forcenodump: Option<DeclaredText>,
    pub header: Option<DeclaredText>,
    pub forcenodump_effective: Option<NoIntroDatForceNoDump>,
}

/// An optional, possibly empty, `RomCenter` header directive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatRomCenterDirective {
    pub source_order: usize,
    pub location: RecordLocation,
    pub plugin: Option<DeclaredText>,
}

/// One game with actual catalog identity and complete native metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatGame {
    pub id: CatalogSetId,
    pub list_order: i64,
    pub source_order: usize,
    pub location: RecordLocation,
    pub name: DeclaredText,
    pub publisher_id: Option<DeclaredText>,
    pub cloneof: Option<NoIntroDatParent>,
    pub cloneofid: Option<NoIntroDatParent>,
    pub children: Vec<NoIntroDatGameChild>,
}

/// An unresolved parent literal with its actual reported relationship registry ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatParent {
    pub relationship_id: i64,
    pub target: DeclaredText,
}

/// Description, repeated text facts, releases and ROMs in mixed source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoIntroDatGameChild {
    Description(DeclaredText),
    Category(NoIntroDatRepeatedText),
    Identifier(NoIntroDatRepeatedText),
    Release(NoIntroDatRelease),
    Rom(Box<NoIntroDatRomReference>),
}

impl NoIntroDatGameChild {
    #[must_use]
    pub const fn source_order(&self) -> usize {
        match self {
            Self::Description(value) => value.source_order,
            Self::Category(value) | Self::Identifier(value) => value.value.source_order,
            Self::Release(value) => value.source_order,
            Self::Rom(value) => value.source_order,
        }
    }
}

/// A family-relative ordinal and independently positioned declared text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatRepeatedText {
    pub family_order: i64,
    pub value: DeclaredText,
}

/// Actual composite release key; not a database-export release registry ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoIntroDatReleaseKey {
    game_id: CatalogSetId,
    release_order: i64,
}

impl NoIntroDatReleaseKey {
    #[must_use]
    pub const fn game_id(self) -> CatalogSetId {
        self.game_id
    }

    #[must_use]
    pub const fn release_order(self) -> i64 {
        self.release_order
    }
}

/// A release's native attributes and their own source positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatRelease {
    pub key: NoIntroDatReleaseKey,
    pub source_order: usize,
    pub location: RecordLocation,
    pub name: DeclaredText,
    pub region: DeclaredText,
}

/// A native ROM claim and the same payload used by occurrence queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoIntroDatRomReference {
    pub occurrence_id: OccurrenceId,
    pub occurrence_order: i64,
    pub source_order: usize,
    pub location: RecordLocation,
    pub attribute_positions: Vec<AttributePosition<NoIntroDatRomField>>,
    pub payload: NoIntroDatRomPayload,
}

/// Query failures never substitute missing native facts with reconstructed guesses.
#[derive(Debug, Error)]
pub enum NoIntroDatQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("native file lookup failed: {0}")]
    Files(#[from] crate::storage::catalog_files::CatalogFilesError),
    #[error("snapshot lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("snapshot {0} is not a published flat No-Intro DAT")]
    NotPublished(SnapshotKey),
    #[error("native DAT document facts are missing for snapshot {0}")]
    MissingDocument(SnapshotKey),
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to a different snapshot")]
    CursorSnapshotMismatch,
    #[error("native DAT metadata is inconsistent for owner {0}")]
    InvalidMetadata(i64),
    #[error("native DAT metadata is missing {field} for owner {owner}")]
    MissingNative { field: &'static str, owner: i64 },
    #[error("native DAT metadata has mismatched owner links for row {0}")]
    MismatchedOwner(i64),
    #[error("source positions are inconsistent for owner {0}")]
    InvalidPositions(i64),
    #[error("invalid stored {field} for owner {owner}: {value}")]
    InvalidStoredValue {
        field: &'static str,
        owner: i64,
        value: String,
    },
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
}

/// Read one page from an exact published DAT, without opening its original file.
///
/// # Errors
/// Rejects wrong/unpublished snapshots, mismatched cursors and inconsistent native
/// metadata. A page limit bounds games only, not their supported children.
pub fn games_for_snapshot(
    database: &crate::database::Database,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatCursor>,
    limit: NoIntroDatPageLimit,
) -> Result<NoIntroDatPage, NoIntroDatQueryError> {
    reader::games_for_snapshot(database, snapshot, cursor, limit)
}
