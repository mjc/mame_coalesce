//! Typed, source-free views of import runs and their persisted diagnostics.

use crate::{
    diagnostics::{CoordinateConvention, ExcerptView, SourceExcerpt},
    domain::{
        CatalogKey, CatalogRegistryId, CatalogSetId, DocumentKey, ImportRunKey, NoIntroArchiveId,
        ParserInterpretationKey, PublishingSourceKey, SnapshotKey,
    },
    no_intro_db_xml::XmlSourceExtent,
    storage::catalog_files::{NoIntroDumpSourceId, NoIntroReleaseId},
};

use crate::domain::OccurrenceId;

const MAX_PAGE_SIZE: usize = 500;

/// A validated number of diagnostic rows returned by one query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticPageLimit(usize);

impl DiagnosticPageLimit {
    /// Create a page limit in the supported range `1..=500`.
    ///
    /// # Errors
    ///
    /// Returns an error when `value` is outside the supported range.
    pub fn new(value: usize) -> Result<Self, DiagnosticQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(DiagnosticQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }

    pub(crate) const fn get(self) -> usize {
        self.0
    }
}

/// A nonnegative, dense position within one import run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticOrder(i64);

impl DiagnosticOrder {
    /// First diagnostic position in every run.
    pub const FIRST: Self = Self(0);

    /// Construct an order suitable for SQLite's signed integer storage.
    #[must_use]
    pub const fn new(value: i64) -> Option<Self> {
        if value >= 0 { Some(Self(value)) } else { None }
    }

    /// The persisted order value.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

/// Stable UUID identity for one persisted diagnostic row.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiagnosticKey(String);

impl DiagnosticKey {
    /// Allocate a UUID key for a newly inserted diagnostic.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Check a database key and preserve its exact canonical representation.
    pub(crate) fn from_database(value: String) -> Option<Self> {
        let parsed = uuid::Uuid::parse_str(&value).ok()?;
        (parsed.to_string() == value).then_some(Self(value))
    }

    /// The canonical stored UUID text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Persisted import-run lifecycle, including states used during a transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportRunStatus {
    /// The run has been recorded but processing has not started.
    Pending,
    /// The run is actively parsing and storing catalog data.
    Running,
    /// The import completed and published a snapshot.
    Succeeded,
    /// Parsing or persistence failed before publication.
    Failed,
}

/// Diagnostic severity stored in the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// A condition was recovered from and the import continued.
    Warning,
    /// The condition prevented the import from completing.
    Error,
}

/// Known diagnostic codes written by the current import pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticCode {
    /// The document could not be parsed into a catalog.
    ParseFailed,
    /// An XML NUL byte was recovered according to the selected import mode.
    XmlNulRecovered,
}

/// Publication source metadata shared by every catalog format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticSource {
    /// Stable source key.
    pub key: PublishingSourceKey,
    /// Human-readable source name.
    pub display_name: String,
}

/// Catalog identity and its publishing source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticCatalog {
    /// Stable catalog key.
    pub key: CatalogKey,
    /// Human-readable catalog name.
    pub display_name: String,
    /// Source that published this catalog.
    pub source: DiagnosticSource,
}

/// External document identity and retained-object metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticDocument {
    /// Content-addressed document identity.
    pub key: DocumentKey,
    /// Retained SHA-1 digest, when known.
    pub sha1: Option<[u8; 20]>,
    /// Retained SHA-256 digest, when known.
    pub sha256: Option<[u8; 32]>,
    /// Original document size in bytes, when known.
    pub byte_length: Option<i64>,
    /// Source-provided format hint, when available.
    pub format_hint: Option<String>,
    /// Whether source bytes are retained outside SQLite.
    pub retention: DocumentRetention,
    /// External object-store identity, when the document is retained.
    pub object_key: Option<String>,
}

/// Whether source bytes are retained outside SQLite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentRetention {
    /// The original bytes are not available from the document store.
    Unavailable,
    /// The original bytes are retained outside SQLite.
    Retained,
}

/// Parser identity and format-level interpretation metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticInterpretation {
    /// Stable parser-interpretation key.
    pub key: ParserInterpretationKey,
    /// Catalog format identifier.
    pub format: String,
    /// Parser implementation name, when recorded.
    pub parser_name: Option<String>,
    /// Parser implementation version, when recorded.
    pub parser_version: Option<String>,
    /// Format-reading rules version, when recorded.
    pub rules_version: Option<String>,
}

/// Snapshot provenance, absent for failed or not-yet-published runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticSnapshot {
    /// Shared catalog-snapshot identity.
    pub key: SnapshotKey,
}

/// Import-run context shared by all format-specific diagnostic rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticRun {
    /// Import-run identity supplied to the query.
    pub key: ImportRunKey,
    /// Persisted lifecycle state.
    pub status: ImportRunStatus,
    /// Start timestamp, if recorded.
    pub started_at: Option<String>,
    /// Completion timestamp, if recorded.
    pub finished_at: Option<String>,
    /// First ordered diagnostic message, independent of the requested page.
    /// Deterministic summary from the first diagnostic, independent of paging.
    pub summary: Option<String>,
    /// External document metadata.
    pub document: DiagnosticDocument,
    /// Catalog and publishing-source metadata.
    pub catalog: DiagnosticCatalog,
    /// Parser and format metadata.
    pub interpretation: DiagnosticInterpretation,
    /// Snapshot identity, absent for runs without a snapshot.
    pub snapshot: Option<DiagnosticSnapshot>,
}

/// One native No-Intro database record related to a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoIntroDiagnosticOwner {
    /// The database-export document element.
    ExportDocument {
        /// Snapshot whose document element contains the diagnostic.
        snapshot: SnapshotKey,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// The optional database-export header element.
    ExportHeader {
        /// Snapshot whose header contains the diagnostic.
        snapshot: SnapshotKey,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One field in the export header.
    HeaderField {
        /// Snapshot containing the field.
        snapshot: SnapshotKey,
        /// Field order within that snapshot's header.
        source_order: i64,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One game/set record.
    Game {
        /// Catalog set identity.
        id: CatalogSetId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One archive-description record.
    ArchiveDescription {
        /// Native archive-description identity.
        id: NoIntroArchiveId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One dump-source record.
    DumpSource {
        /// Native dump-source identity.
        id: NoIntroDumpSourceId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// The dump-source details element.
    DumpDetails {
        /// Parent dump-source identity.
        id: NoIntroDumpSourceId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// The dump-source serials element.
    DumpSerials {
        /// Parent dump-source identity.
        id: NoIntroDumpSourceId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One file listed by a dump source.
    DumpFile {
        /// Shared catalog-file occurrence identity.
        id: OccurrenceId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One release record.
    Release {
        /// Native release identity.
        id: NoIntroReleaseId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// The release details element.
    ReleaseDetails {
        /// Parent release identity.
        id: NoIntroReleaseId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// The release serials element.
    ReleaseSerials {
        /// Parent release identity.
        id: NoIntroReleaseId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
    /// One file listed by a release.
    ReleaseFile {
        /// Shared catalog-file occurrence identity.
        id: OccurrenceId,
        /// Exact native source extent, end-exclusive.
        extent: XmlSourceExtent,
    },
}

/// The actual typed primary key addressed by one owner-link relation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeDiagnosticOwnerKey {
    /// Export document, header, or header-field owner identified by snapshot.
    Snapshot(SnapshotKey),
    /// Header field identified by its snapshot and stored order.
    HeaderField {
        /// Snapshot containing the field.
        snapshot: SnapshotKey,
        /// Zero-based native field order.
        source_order: i64,
    },
    /// Game owner identified by its catalog-set ID.
    Game(CatalogSetId),
    /// Archive-description owner.
    ArchiveDescription(NoIntroArchiveId),
    /// Dump-source, details, or serials owner.
    DumpSource(NoIntroDumpSourceId),
    /// Dump-file owner identified by occurrence ID.
    DumpFile(OccurrenceId),
    /// Release, details, or serials owner.
    Release(NoIntroReleaseId),
    /// Release-file owner identified by occurrence ID.
    ReleaseFile(OccurrenceId),
}

/// Closed relation/key mapping used by both SQL insertion and hydration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticOwnerLink {
    /// Exact owner-link relation in the SQLite schema.
    pub relation: &'static str,
    /// Typed native key addressed by the relation.
    pub key: NativeDiagnosticOwnerKey,
    /// Exact source extent persisted for the native owner.
    pub extent: XmlSourceExtent,
}

impl NoIntroDiagnosticOwner {
    pub(crate) fn link(&self) -> DiagnosticOwnerLink {
        let (relation, key, extent) = match self {
            Self::ExportDocument { snapshot, extent } => (
                "no_intro_export_diagnostics",
                NativeDiagnosticOwnerKey::Snapshot(snapshot.clone()),
                *extent,
            ),
            Self::ExportHeader { snapshot, extent } => (
                "no_intro_export_header_diagnostics",
                NativeDiagnosticOwnerKey::Snapshot(snapshot.clone()),
                *extent,
            ),
            Self::HeaderField {
                snapshot,
                source_order,
                extent,
            } => (
                "no_intro_header_field_diagnostics",
                NativeDiagnosticOwnerKey::HeaderField {
                    snapshot: snapshot.clone(),
                    source_order: *source_order,
                },
                *extent,
            ),
            Self::Game { id, extent } => (
                "no_intro_game_diagnostics",
                NativeDiagnosticOwnerKey::Game(*id),
                *extent,
            ),
            Self::ArchiveDescription { id, extent } => (
                "no_intro_archive_diagnostics",
                NativeDiagnosticOwnerKey::ArchiveDescription(*id),
                *extent,
            ),
            Self::DumpSource { id, extent } => (
                "no_intro_dump_source_diagnostics",
                NativeDiagnosticOwnerKey::DumpSource(*id),
                *extent,
            ),
            Self::DumpDetails { id, extent } => (
                "no_intro_dump_details_diagnostics",
                NativeDiagnosticOwnerKey::DumpSource(*id),
                *extent,
            ),
            Self::DumpSerials { id, extent } => (
                "no_intro_dump_serials_diagnostics",
                NativeDiagnosticOwnerKey::DumpSource(*id),
                *extent,
            ),
            Self::DumpFile { id, extent } => (
                "no_intro_dump_file_diagnostics",
                NativeDiagnosticOwnerKey::DumpFile(*id),
                *extent,
            ),
            Self::Release { id, extent } => (
                "no_intro_release_diagnostics",
                NativeDiagnosticOwnerKey::Release(*id),
                *extent,
            ),
            Self::ReleaseDetails { id, extent } => (
                "no_intro_release_details_diagnostics",
                NativeDiagnosticOwnerKey::Release(*id),
                *extent,
            ),
            Self::ReleaseSerials { id, extent } => (
                "no_intro_release_serials_diagnostics",
                NativeDiagnosticOwnerKey::Release(*id),
                *extent,
            ),
            Self::ReleaseFile { id, extent } => (
                "no_intro_release_file_diagnostics",
                NativeDiagnosticOwnerKey::ReleaseFile(*id),
                *extent,
            ),
        };
        DiagnosticOwnerLink {
            relation,
            key,
            extent,
        }
    }
}

/// Persisted diagnostic text, independent coordinates and exact byte evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportDiagnostic {
    /// Stable diagnostic UUID.
    pub key: DiagnosticKey,
    /// Zero-based order within its import run.
    pub order: DiagnosticOrder,
    /// Warning or fatal error.
    pub severity: DiagnosticSeverity,
    /// Closed identifier for the diagnostic condition.
    pub code: DiagnosticCode,
    /// Stored user-facing message; an empty string remains distinct from NULL.
    pub message: String,
    /// Kind of source record, when known.
    pub record_kind: Option<String>,
    /// Source record name, when known.
    pub record_name: Option<String>,
    /// Source field name, when known.
    pub field_name: Option<String>,
    /// Exact offending source text, when identified.
    pub offending_text: Option<String>,
    /// Source line, independently nullable and not inferred from the column.
    pub source_line: Option<i64>,
    /// Source column, independently nullable and not inferred from the line.
    pub source_column: Option<i64>,
    /// Exact saved-byte evidence, when available.
    pub excerpt: Option<SourceExcerpt>,
    /// Declared meaning of line and column coordinates.
    pub coordinates: Option<CoordinateConvention>,
    /// Every stored typed No-Intro database owner link for this row.
    pub owners: Vec<NoIntroDiagnosticOwner>,
}

/// One keyset page of diagnostics and its shared run context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticPage {
    /// Shared run context for all formats.
    pub run: DiagnosticRun,
    /// Ordered diagnostic rows for this page.
    pub diagnostics: Vec<ImportDiagnostic>,
    /// Continuation after the last returned diagnostic.
    pub next_cursor: Option<DiagnosticCursor>,
}

/// Opaque continuation bound to a run, diagnostic anchor and database generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticCursor {
    /// Database registry generation that issued this cursor.
    pub(crate) registry: CatalogRegistryId,
    /// Import run whose ordered diagnostics are being paged.
    pub(crate) run: ImportRunKey,
    /// Last returned diagnostic's order.
    pub(crate) order: DiagnosticOrder,
    /// Last returned diagnostic's stable identity.
    pub(crate) key: DiagnosticKey,
}

/// Errors from source-free import-diagnostic queries.
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("diagnostic query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("database identity registry is invalid: {0}")]
    Registry(#[from] crate::Error),
    #[error("import run {0} does not exist")]
    MissingRun(ImportRunKey),
    #[error("stored diagnostic metadata is invalid for row {0}")]
    InvalidMetadata(String),
    #[error("diagnostic cursor belongs to another run")]
    CursorRunMismatch,
    #[error("diagnostic cursor belongs to another registry generation")]
    CursorRegistryMismatch,
    #[error("diagnostic cursor anchor is missing or inconsistent")]
    InvalidCursor,
}

impl DiagnosticCode {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "parse_failed" => Some(Self::ParseFailed),
            "xml_nul_recovered" => Some(Self::XmlNulRecovered),
            _ => None,
        }
    }
}

impl DiagnosticSeverity {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl ImportRunStatus {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

impl DocumentRetention {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "unavailable" => Some(Self::Unavailable),
            "retained" => Some(Self::Retained),
            _ => None,
        }
    }
}

impl ExcerptView {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "retained_original_bytes" => Some(Self::RetainedOriginalBytes),
            "transport_decoded_xml_bytes" => Some(Self::TransportDecodedXmlBytes),
            _ => None,
        }
    }
}

impl CoordinateConvention {
    pub(crate) fn parse(view: &str, columns: &str) -> Option<Self> {
        if columns != "unicode_scalar_1based" {
            return None;
        }
        match view {
            "transport_decoded_xml_text" => Some(Self::XmlUnicodeScalars),
            "decoded_dat_text" => Some(Self::DatUnicodeScalars),
            _ => None,
        }
    }
}
