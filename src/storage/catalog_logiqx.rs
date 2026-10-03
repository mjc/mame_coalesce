//! Bounded queries for published native Logiqx document and game facts.

mod document;
mod games;
mod media;
mod positions;
mod queries;
mod reader;

use thiserror::Error;

use crate::{
    database::Database,
    domain::{CatalogRegistryId, CatalogSetId, SnapshotKey},
    logiqx::{
        AttributePosition, BiosSetAttribute, ClrMameProAttribute, DocumentAttribute, GameAttribute,
        NameAttribute, RecordLocation, ReleaseAttribute, RomCenterAttribute,
    },
    storage::catalog_files::OccurrenceId,
};

const MAX_PAGE_SIZE: usize = 500;

pub use crate::logiqx::{
    HeaderTextField as LogiqxHeaderTextField, HeaderTextPosition as LogiqxHeaderTextPosition,
};

/// An effective header option together with its original declaration state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxOptionValue<T> {
    Explicit(T),
    Defaulted(T),
}

impl<T> LogiqxOptionValue<T> {
    const fn new(value: T, was_present: bool) -> Self {
        if was_present {
            Self::Explicit(value)
        } else {
            Self::Defaulted(value)
        }
    }

    #[must_use]
    pub const fn effective(&self) -> &T {
        match self {
            Self::Explicit(value) | Self::Defaulted(value) => value,
        }
    }

    #[must_use]
    pub const fn was_present(&self) -> bool {
        matches!(self, Self::Explicit(_))
    }
}

/// A validated maximum number of Logiqx games returned by one query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogiqxPageLimit(usize);

impl LogiqxPageLimit {
    /// Create a page limit in the supported range `1..=500`.
    pub fn new(value: usize) -> Result<Self, LogiqxQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(LogiqxQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }
}

/// Continuation for a Logiqx game page, pinned to its snapshot and registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxCursor {
    generation: CatalogRegistryId,
    snapshot: SnapshotKey,
    order: i64,
    owner_id: CatalogSetId,
}

/// Source location of a direct text child, distinct from XML attribute locations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxTextValue {
    pub value: String,
    pub source_order: i64,
    pub location: RecordLocation,
}

/// One native Logiqx header and its ten nullable PCDATA values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxHeader {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub date: Option<String>,
    pub author: Option<String>,
    pub email: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
    pub comment: Option<String>,
    pub category: Option<String>,
    pub text_positions: Vec<LogiqxHeaderTextPosition>,
}

/// Native datafile facts; a missing header differs from a present empty header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxDocument {
    pub build: Option<String>,
    pub debug: LogiqxYesNo,
    pub debug_was_present: bool,
    pub file_name: Option<String>,
    pub sha1: Option<Vec<u8>>,
    pub header: Option<LogiqxHeader>,
    pub clrmamepro: Option<LogiqxClrMameProOptions>,
    pub romcenter: Option<LogiqxRomCenterOptions>,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<DocumentAttribute>>,
}

/// A closed Logiqx yes/no source value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxYesNo {
    Yes,
    No,
}

/// Native `ClrMamePro` XML header options, retaining effective defaults and presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxClrMameProOptions {
    pub source_order: i64,
    pub location: RecordLocation,
    pub header: Option<String>,
    pub forcemerging: LogiqxOptionValue<LogiqxForceMerging>,
    pub forcenodump: LogiqxOptionValue<LogiqxForceNoDump>,
    pub forcepacking: LogiqxOptionValue<LogiqxForcePacking>,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<ClrMameProAttribute>>,
}

/// Closed `forcemerging` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxForceMerging {
    Split,
    None,
    Full,
}

/// Closed `forcenodump` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxForceNoDump {
    Obsolete,
    Required,
    Ignore,
}

/// Closed `forcepacking` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxForcePacking {
    Zip,
    Unzip,
}

/// Native `RomCenter` XML header options, retaining effective defaults and presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxRomCenterOptions {
    pub source_order: i64,
    pub location: RecordLocation,
    pub plugin: Option<String>,
    pub rommode: LogiqxOptionValue<LogiqxRomMode>,
    pub biosmode: LogiqxOptionValue<LogiqxRomMode>,
    pub samplemode: LogiqxOptionValue<LogiqxSampleMode>,
    pub lockrommode: LogiqxOptionValue<LogiqxYesNo>,
    pub lockbiosmode: LogiqxOptionValue<LogiqxYesNo>,
    pub locksamplemode: LogiqxOptionValue<LogiqxYesNo>,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<RomCenterAttribute>>,
}

/// Closed `RomCenter` ROM or BIOS mode values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxRomMode {
    Split,
    Merged,
    Unmerged,
}

/// Closed `RomCenter` sample mode values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxSampleMode {
    Merged,
    Unmerged,
}

/// Native game attributes, direct text values and ordered children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxGame {
    pub id: CatalogSetId,
    pub name: String,
    pub list_order: i64,
    pub location: RecordLocation,
    pub sourcefile: Option<String>,
    pub is_bios: LogiqxYesNo,
    pub is_bios_was_present: bool,
    pub cloneof: Option<LogiqxParentReference>,
    pub romof: Option<LogiqxParentReference>,
    pub sampleof: Option<LogiqxParentReference>,
    pub board: Option<String>,
    pub rebuildto: Option<String>,
    pub description: Option<LogiqxTextValue>,
    pub year: Option<LogiqxTextValue>,
    pub manufacturer: Option<LogiqxTextValue>,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<GameAttribute>>,
    pub comments: Vec<LogiqxComment>,
    pub releases: Vec<LogiqxRelease>,
    pub bios_sets: Vec<LogiqxBiosSet>,
    pub media: Vec<LogiqxMediaReference>,
    pub archives: Vec<LogiqxArchiveReference>,
    pub device_references: Vec<LogiqxDeviceReference>,
}

/// Native parent or merge target literal and its `QName` position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxParentReference {
    pub target_name: String,
    pub position: AttributePosition<GameAttribute>,
}

/// Repeated game comment with its direct-child record location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxComment {
    pub comment_order: i64,
    pub text: String,
    pub source_order: i64,
    pub location: RecordLocation,
}

/// Ordered release fields with effective default and original presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxRelease {
    pub release_order: i64,
    pub name: String,
    pub region: String,
    pub language: Option<String>,
    pub date: Option<String>,
    pub is_default: LogiqxYesNo,
    pub default_was_present: bool,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<ReleaseAttribute>>,
}

/// Ordered BIOS set fields with effective default and original presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxBiosSet {
    pub bios_order: i64,
    pub name: String,
    pub description: String,
    pub is_default: LogiqxYesNo,
    pub default_was_present: bool,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<BiosSetAttribute>>,
}

/// Format-specific kind of native media payload reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxMediaKind {
    Rom,
    Disk,
    Sample,
}

/// Reference to an existing persisted media occurrence; payload stays in `catalog_files`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxMediaReference {
    pub occurrence_id: OccurrenceId,
    pub occurrence_order: i64,
    pub kind: LogiqxMediaKind,
    pub source_order: i64,
    pub location: RecordLocation,
}

/// Ordered named archive reference; this does not imply a file payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxArchiveReference {
    pub archive_order: i64,
    pub name: String,
    pub source_order: i64,
    pub location: RecordLocation,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<NameAttribute>>,
}

/// Ordered native device reference with its `QName` position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxDeviceReference {
    pub reference_order: i64,
    pub name: String,
    pub attribute_positions: Vec<crate::logiqx::AttributePosition<NameAttribute>>,
}

/// Snapshot identity and publication provenance shared by one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxSnapshot {
    pub registry_id: CatalogRegistryId,
    pub snapshot_key: SnapshotKey,
    pub source_key: crate::domain::PublishingSourceKey,
    pub source_name: String,
    pub catalog_key: crate::domain::CatalogKey,
    pub catalog_name: String,
    pub document_key: crate::domain::DocumentKey,
    pub interpretation_key: crate::domain::ParserInterpretationKey,
    pub format: String,
    pub declared_version: Option<String>,
}

/// One bounded game page plus document facts and publication provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxPage {
    pub snapshot: LogiqxSnapshot,
    pub document: LogiqxDocument,
    pub games: Vec<LogiqxGame>,
    pub next_cursor: Option<LogiqxCursor>,
}

/// Errors returned by native Logiqx document/game queries.
#[derive(Debug, Error)]
pub enum LogiqxQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("snapshot lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("snapshot {0} is not a published Logiqx edition")]
    NotPublishedLogiqx(SnapshotKey),
    #[error("native Logiqx document facts are missing for snapshot {0}")]
    MissingDocument(SnapshotKey),
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to a different snapshot")]
    CursorSnapshotMismatch,
    #[error("native Logiqx metadata is missing {field} for owner {owner}")]
    MissingNative { field: &'static str, owner: i64 },
    #[error("native Logiqx metadata has mismatched owner links for row {0}")]
    MismatchedOwner(i64),
    #[error("invalid stored {field} for owner {owner}: {value}")]
    InvalidStoredValue {
        field: &'static str,
        owner: i64,
        value: String,
    },
    #[error("native Logiqx source positions are inconsistent for owner {0}")]
    InvalidPositions(i64),
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
}

/// Return one bounded page from an exact published Logiqx snapshot.
pub fn logiqx_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&LogiqxCursor>,
    limit: LogiqxPageLimit,
) -> Result<LogiqxPage, LogiqxQueryError> {
    reader::logiqx_for_snapshot(database, snapshot, cursor, limit)
}
