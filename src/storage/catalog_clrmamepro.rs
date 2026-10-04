//! Source-free native `ClrMamePro` metadata for exact published catalog snapshots.
//!
//! A page bounds sets, not their children or the document's lexical comments.
//! Raw declarations and positions remain distinct from effective directives.

mod document;
mod positions;
mod queries;
mod reader;
mod sets;

#[cfg(test)]
mod queries_tests;

pub use super::clrmamepro_fields::{
    ClrMameProDeclaredValue, ClrMameProFieldPosition, ClrMameProHeaderField,
    ClrMameProPositionedField, ClrMameProRelationshipId, ClrMameProRomField, ClrMameProSetField,
};

use crate::{
    database::Database,
    domain::{
        CatalogKey, CatalogRegistryId, CatalogSetId, DocumentKey, ParserInterpretationKey,
        PublishingSourceKey, SnapshotKey,
    },
    storage::catalog_files::{
        ClrMameProRomPayload, ClrMameProSamplePayload, OccurrenceId, SourceLocation,
    },
};

/// Checked maximum number of sets; children of a selected set are never truncated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClrMameProPageLimit(usize);

impl ClrMameProPageLimit {
    /// Accept a set count in `1..=500`.
    ///
    /// # Errors
    /// Rejects zero or more than 500 sets.
    ///
    /// ```
    /// use mame_coalesce::catalog_clrmamepro::ClrMameProPageLimit;
    /// assert!(ClrMameProPageLimit::new(0).is_err());
    /// assert!(ClrMameProPageLimit::new(500).is_ok());
    /// assert!(ClrMameProPageLimit::new(501).is_err());
    /// ```
    pub fn new(value: usize) -> Result<Self, ClrMameProQueryError> {
        if (1..=500).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ClrMameProQueryError::InvalidPageLimit(value))
        }
    }
}

/// Actual supported-set anchor, pinned to snapshot and registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProCursor {
    registry_id: CatalogRegistryId,
    snapshot_key: SnapshotKey,
    owner_id: CatalogSetId,
    list_order: i64,
}

/// Publication and source provenance of one queried edition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProSnapshot {
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

/// Native document declaration and complete lexical comments; no invented root position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProDocument {
    pub header_present: bool,
    pub comment_count: i64,
    pub comments: Vec<ClrMameProComment>,
    pub header: Option<ClrMameProHeader>,
}

/// A semicolon token owned by the document, including its semicolon and whitespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProComment {
    pub comment_order: i64,
    pub text: String,
    pub location: SourceLocation,
}

/// One present header with source-ordered scalar values and derived options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProHeader {
    pub source_block: String,
    pub source_order: usize,
    pub location: SourceLocation,
    pub fields: Vec<ClrMameProHeaderValue>,
    pub forcemerging_effective: Option<ClrMameProForceMerging>,
    pub forcezipping_effective: Option<ClrMameProForceZipping>,
    pub forcenodump_effective: Option<ClrMameProForceNoDump>,
}

/// One of fifteen declared header fields; absent and empty differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProHeaderValue {
    pub field: ClrMameProHeaderField,
    pub value: ClrMameProDeclaredValue,
}

/// Recognized native merging directive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProForceMerging {
    None,
    Split,
    Full,
}
/// Recognized native zipping directive, independent of forcepacking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProForceZipping {
    Zip,
    Unzip,
}
/// Recognized nodump directive; absent on a present header defaults to Obsolete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProForceNoDump {
    Obsolete,
    Required,
    Ignore,
}

/// One actual set owner with complete native mixed children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProSet {
    pub id: CatalogSetId,
    pub list_order: i64,
    pub source_block: String,
    pub source_order: usize,
    pub location: SourceLocation,
    pub children: Vec<ClrMameProSetChild>,
}

/// Native scalar/parent fields and mixed ROM/sample references in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClrMameProSetChild {
    Field {
        field: ClrMameProSetField,
        value: ClrMameProDeclaredValue,
    },
    Parent(ClrMameProParentReference),
    Rom(Box<ClrMameProRomReference>),
    Sample(ClrMameProSampleReference),
}

impl ClrMameProSetChild {
    #[must_use]
    pub const fn source_order(&self) -> usize {
        match self {
            Self::Field { value, .. } => value.position.source_order,
            Self::Parent(parent) => parent.target.position.source_order,
            Self::Rom(rom) => rom.payload.source_order,
            Self::Sample(sample) => sample.payload.position.source_order,
        }
    }
}

/// Separate unresolved clone and sample-parent declarations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProParentKind {
    CloneOf,
    SampleOf,
}

/// Reported registry identity and exact unresolved parent target declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProParentReference {
    pub kind: ClrMameProParentKind,
    pub relationship_id: ClrMameProRelationshipId,
    pub target: ClrMameProDeclaredValue,
}

/// Actual ROM occurrence and the same native payload exposed by file queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProRomReference {
    pub occurrence_id: OccurrenceId,
    pub occurrence_order: i64,
    pub payload: ClrMameProRomPayload,
}

/// Actual filename-only sample; no invented content identity or file evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProSampleReference {
    pub occurrence_id: OccurrenceId,
    pub occurrence_order: i64,
    pub payload: ClrMameProSamplePayload,
}

/// One bounded set page and complete native document metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProPage {
    pub snapshot: ClrMameProSnapshot,
    pub document: ClrMameProDocument,
    pub sets: Vec<ClrMameProSet>,
    pub next_cursor: Option<ClrMameProCursor>,
}

/// Native query failures never substitute guessed or reparsed source values.
#[derive(Debug, thiserror::Error)]
pub enum ClrMameProQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("catalog registry lookup failed: {0}")]
    Registry(#[source] Box<crate::Error>),
    #[error("native file lookup failed: {0}")]
    Files(#[source] Box<super::catalog_files::CatalogFilesError>),
    #[error("page limit {0} is outside 1..=500")]
    InvalidPageLimit(usize),
    #[error("snapshot {0} is not a published native CMP interpretation")]
    NotPublished(SnapshotKey),
    #[error("cursor belongs to another registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to another snapshot")]
    CursorSnapshotMismatch,
    #[error("native CMP metadata is inconsistent for owner {0}")]
    InvalidMetadata(i64),
    #[error("native CMP source positions are inconsistent for owner {0}")]
    InvalidPositions(i64),
    #[error("native CMP owner links are inconsistent for owner {0}")]
    MismatchedOwner(i64),
}

impl From<crate::Error> for ClrMameProQueryError {
    fn from(error: crate::Error) -> Self {
        Self::Registry(Box::new(error))
    }
}

impl From<super::catalog_files::CatalogFilesError> for ClrMameProQueryError {
    fn from(error: super::catalog_files::CatalogFilesError) -> Self {
        Self::Files(Box::new(error))
    }
}

/// Return one complete native set page without opening the retained original.
///
/// # Errors
/// Rejects wrong/unpublished interpretations, mismatched cursors and inconsistent
/// native values, positions or ownership. Bounds apply to sets, not descendants.
pub fn sets_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&ClrMameProCursor>,
    limit: ClrMameProPageLimit,
) -> Result<ClrMameProPage, ClrMameProQueryError> {
    reader::sets_for_snapshot(database, snapshot, cursor, limit)
}
