//! Source-only diagnostic messages for an in-progress catalog import.

mod reader;
pub use reader::{CatalogMessageReader, SourceMessage};

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use super::catalog_ids::{ImportId, ImportMessageId};

/// Maximum number of source bytes retained for one message.
pub const MAX_EXCERPT_BYTES: usize = 512;

/// Severity persisted for a source diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageSeverity {
    /// The import can continue, but the source produced a diagnostic.
    Warning,
    /// The source prevents this import from succeeding.
    Error,
}

impl MessageSeverity {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Byte stream that owns a diagnostic's source byte offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteView {
    /// Bytes retained exactly as supplied to the importer.
    RetainedOriginalBytes,
    /// XML bytes after transport decoding and before text repair/decoding.
    TransportDecodedXmlBytes,
}

impl ByteView {
    const fn as_sql(self) -> &'static str {
        match self {
            Self::RetainedOriginalBytes => "retained_original_bytes",
            Self::TransportDecodedXmlBytes => "transport_decoded_xml_bytes",
        }
    }
}

/// A checked, start-inclusive and end-exclusive byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteSpan {
    start: usize,
    end: usize,
}

impl ByteSpan {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Option<Self> {
        if start <= end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    /// Inclusive start byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Exclusive end byte offset; equal to `start` for an empty/EOF anchor.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }
}

/// Bounded source bytes and the independently proven diagnostic coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageEvidence {
    excerpt: Option<Vec<u8>>,
    view: Option<ByteView>,
    excerpt_source_start: Option<usize>,
    excerpt_problem: Option<ByteSpan>,
    source_problem: Option<ByteSpan>,
    original_problem: Option<ByteSpan>,
}

impl MessageEvidence {
    /// Build a bounded byte excerpt while preserving independent full ranges.
    ///
    /// An original range supplies source coordinates only when the excerpt view
    /// itself is the retained original. A local highlight is saved only when the
    /// complete source range fits in the excerpt; partial highlights are never
    /// clipped. If `original_problem` is present, `original_byte_length` must
    /// also be present so the range can be checked against the retained file.
    #[must_use]
    pub fn capture(
        view: ByteView,
        view_bytes: &[u8],
        source_problem: Option<ByteSpan>,
        original_problem: Option<ByteSpan>,
        original_byte_length: Option<usize>,
    ) -> Option<Self> {
        let source_problem = source_problem.or_else(|| {
            (view == ByteView::RetainedOriginalBytes)
                .then_some(original_problem)
                .flatten()
        });
        let (excerpt, stored_view, stored_excerpt_start, excerpt_problem) =
            if let Some(problem) = source_problem {
                let (excerpt_start, excerpt_end) = bounded_window(view_bytes.len(), problem.start);
                let excerpt = view_bytes.get(excerpt_start..excerpt_end)?.to_vec();
                let local_problem = problem.start.checked_sub(excerpt_start).and_then(|start| {
                    let end = problem.end.checked_sub(excerpt_start)?;
                    (problem.end <= excerpt_end).then_some(ByteSpan { start, end })
                });
                (
                    Some(excerpt),
                    Some(view),
                    Some(excerpt_start),
                    local_problem,
                )
            } else {
                (None, None, None, None)
            };

        Self::from_persisted(
            excerpt,
            stored_view,
            stored_excerpt_start,
            excerpt_problem,
            source_problem,
            original_problem,
            Some(view_bytes.len()),
            original_byte_length,
        )
        .ok()
    }

    /// Validate and construct source evidence read from persisted metadata.
    pub(crate) fn from_persisted(
        excerpt: Option<Vec<u8>>,
        view: Option<ByteView>,
        excerpt_source_start: Option<usize>,
        excerpt_problem: Option<ByteSpan>,
        source_problem: Option<ByteSpan>,
        original_problem: Option<ByteSpan>,
        view_byte_length: Option<usize>,
        original_byte_length: Option<usize>,
    ) -> crate::Result<Self> {
        let invalid = |detail: &str| {
            crate::Error::DatabaseSchema(format!(
                "invalid persisted catalog import message evidence: {detail}"
            ))
        };
        if let Some(problem) = original_problem
            && original_byte_length.is_none_or(|length| problem.end > length)
        {
            return Err(invalid(
                "original problem range exceeds the retained source",
            ));
        }
        if let Some(problem) = source_problem {
            if view.is_none() || view_byte_length.is_none_or(|length| problem.end > length) {
                return Err(invalid("source problem range exceeds its byte view"));
            }
        }
        if view.is_some() && view_byte_length.is_none() {
            return Err(invalid("source byte view has no persisted length"));
        }
        if view == Some(ByteView::RetainedOriginalBytes)
            && matches!((source_problem, original_problem), (Some(source), Some(original)) if source != original)
        {
            return Err(invalid(
                "retained-original coordinates disagree with source coordinates",
            ));
        }

        match (excerpt.as_deref(), excerpt_source_start) {
            (None, None) if excerpt_problem.is_none() => {}
            (Some(bytes), Some(start)) => {
                if bytes.len() > MAX_EXCERPT_BYTES {
                    return Err(invalid("excerpt exceeds the configured byte limit"));
                }
                let view_length =
                    view_byte_length.ok_or_else(|| invalid("excerpt has no byte view"))?;
                let end = start
                    .checked_add(bytes.len())
                    .ok_or_else(|| invalid("excerpt end overflows"))?;
                if view.is_none() || end > view_length {
                    return Err(invalid("excerpt exceeds its persisted byte view"));
                }

                let source_span = source_problem.or_else(|| {
                    (view == Some(ByteView::RetainedOriginalBytes))
                        .then_some(original_problem)
                        .flatten()
                });
                let expected_highlight = source_span.and_then(|span| {
                    let local_start = span.start.checked_sub(start)?;
                    let local_end = span.end.checked_sub(start)?;
                    (span.end <= end).then_some(ByteSpan {
                        start: local_start,
                        end: local_end,
                    })
                });
                if excerpt_problem != expected_highlight {
                    return Err(invalid(
                        "excerpt highlight is partial or disagrees with its source range",
                    ));
                }
            }
            _ => return Err(invalid("excerpt and source start are not paired")),
        }

        Ok(Self {
            excerpt,
            view,
            excerpt_source_start,
            excerpt_problem,
            source_problem,
            original_problem,
        })
    }

    /// The saved excerpt, absent when no source-view span is known.
    #[must_use]
    pub fn excerpt(&self) -> Option<&[u8]> {
        self.excerpt.as_deref()
    }

    /// Byte view owning the saved excerpt, when one exists.
    #[must_use]
    pub const fn view(&self) -> Option<ByteView> {
        self.view
    }

    /// Excerpt start offset in its byte view.
    #[must_use]
    pub const fn excerpt_source_start(&self) -> Option<usize> {
        self.excerpt_source_start
    }

    /// Complete problem range relative to the saved excerpt, if fully included.
    #[must_use]
    pub const fn excerpt_problem(&self) -> Option<ByteSpan> {
        self.excerpt_problem
    }

    /// Complete problem range in the byte view, independent of excerpt clipping.
    #[must_use]
    pub const fn source_problem(&self) -> Option<ByteSpan> {
        self.source_problem
    }

    /// Complete problem range in the retained original file, when proven.
    #[must_use]
    pub const fn original_problem(&self) -> Option<ByteSpan> {
        self.original_problem
    }
}

fn bounded_window(source_len: usize, anchor: usize) -> (usize, usize) {
    let end = source_len.min(anchor.saturating_add(MAX_EXCERPT_BYTES));
    let start = end.saturating_sub(MAX_EXCERPT_BYTES);
    (start, end)
}

#[derive(Debug)]
pub struct MessageDraft<'a> {
    order: usize,
    severity: MessageSeverity,
    code: &'a str,
    message: &'a str,
    evidence: &'a MessageEvidence,
}

impl<'a> MessageDraft<'a> {
    /// Create a source-only message with an explicit import-local order.
    #[must_use]
    pub const fn new(
        order: usize,
        severity: MessageSeverity,
        code: &'a str,
        message: &'a str,
        evidence: &'a MessageEvidence,
    ) -> Self {
        Self {
            order,
            severity,
            code,
            message,
            evidence,
        }
    }

    /// Zero-based message order within the import.
    #[must_use]
    pub const fn order(&self) -> usize {
        self.order
    }

    /// Diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> MessageSeverity {
        self.severity
    }

    /// Stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'a str {
        self.code
    }

    /// Human-readable diagnostic text.
    #[must_use]
    pub const fn message(&self) -> &'a str {
        self.message
    }

    /// Proven source evidence attached to this message.
    #[must_use]
    pub const fn evidence(&self) -> &'a MessageEvidence {
        self.evidence
    }
}

#[derive(QueryableByName)]
struct InsertedMessage {
    #[diesel(sql_type = BigInt)]
    message_id: i64,
}

/// Insert a source-only message while its import is still running.
///
/// Source-file identity, reading rules, and the actual current edition are read
/// from the import row, so callers cannot attach a message to another source.
/// Missing or terminal imports return a database error.
pub fn insert_source_message(
    conn: &mut SqliteConnection,
    import_id: ImportId,
    draft: MessageDraft<'_>,
) -> crate::Result<ImportMessageId> {
    let order = i64::try_from(draft.order)
        .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))?;
    let row = sql_query(
        "INSERT INTO catalog_import_messages (
             import_id, message_order, severity, code, message,
             source_file_id, reading_rules_id, edition_id,
             excerpt, source_view, excerpt_source_start,
             excerpt_problem_start, excerpt_problem_end,
             source_problem_start, source_problem_end,
             original_problem_start, original_problem_end
         )
         SELECT run.import_id, ?, ?, ?, ?,
                run.source_file_id, run.reading_rules_id, run.edition_id,
                ?, ?, ?, ?, ?, ?, ?, ?, ?
         FROM catalog_imports AS run
         WHERE run.import_id = ? AND run.status = 'running'
         RETURNING message_id",
    )
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(draft.severity.as_sql())
    .bind::<Text, _>(draft.code)
    .bind::<Text, _>(draft.message)
    .bind::<Nullable<Binary>, _>(draft.evidence.excerpt.as_deref())
    .bind::<Nullable<Text>, _>(draft.evidence.view.map(ByteView::as_sql))
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(draft.evidence.excerpt_source_start)?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.excerpt_problem.map(|span| span.start),
    )?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.excerpt_problem.map(|span| span.end),
    )?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.source_problem.map(|span| span.start),
    )?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.source_problem.map(|span| span.end),
    )?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.original_problem.map(|span| span.start),
    )?)
    .bind::<Nullable<BigInt>, _>(to_nullable_integer(
        draft.evidence.original_problem.map(|span| span.end),
    )?)
    .bind::<BigInt, _>(import_id.as_i64())
    .get_result::<InsertedMessage>(conn)?;

    ImportMessageId::try_from(row.message_id)
        .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)).into())
}

fn to_nullable_integer(value: Option<usize>) -> crate::Result<Option<i64>> {
    value
        .map(|value| {
            i64::try_from(value)
                .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))
        })
        .transpose()
        .map_err(Into::into)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use diesel::{
        Connection,
        connection::SimpleConnection,
        sql_types::{Nullable, Text},
    };

    const MESSAGE_SCHEMA: &str = include_str!("../../docs/schema-candidate/diagnostics.sql");
    const CATALOG_SCHEMA: &str = include_str!("db/catalog.sql");

    #[derive(QueryableByName)]
    struct StoredMessage {
        #[diesel(sql_type = BigInt)]
        import_id: i64,
        #[diesel(sql_type = BigInt)]
        message_order: i64,
        #[diesel(sql_type = Text)]
        severity: String,
        #[diesel(sql_type = Text)]
        code: String,
        #[diesel(sql_type = Text)]
        message: String,
        #[diesel(sql_type = BigInt)]
        source_file_id: i64,
        #[diesel(sql_type = BigInt)]
        reading_rules_id: i64,
        #[diesel(sql_type = Nullable<BigInt>)]
        edition_id: Option<i64>,
        #[diesel(sql_type = Nullable<Binary>)]
        excerpt: Option<Vec<u8>>,
        #[diesel(sql_type = Nullable<Text>)]
        source_view: Option<String>,
        #[diesel(sql_type = Nullable<BigInt>)]
        excerpt_source_start: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        excerpt_problem_start: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        excerpt_problem_end: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        source_problem_start: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        source_problem_end: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        original_problem_start: Option<i64>,
        #[diesel(sql_type = Nullable<BigInt>)]
        original_problem_end: Option<i64>,
    }

    fn message_schema_fragment() -> String {
        let definition = MESSAGE_SCHEMA
            .split_once("CREATE TABLE catalog_import_messages")
            .expect("candidate message table exists")
            .1;
        let end = definition
            .find(") STRICT;")
            .expect("candidate message table terminates")
            + ") STRICT;".len();
        format!("CREATE TABLE catalog_import_messages{}", &definition[..end])
    }

    fn coordinate_trigger(name: &str) -> String {
        let marker = format!("CREATE TRIGGER {name}");
        let statement = CATALOG_SCHEMA
            .split_once(&marker)
            .expect("generated catalog schema contains coordinate trigger")
            .1;
        let end = statement.find("END;").expect("trigger terminates") + "END;".len();
        format!("{marker}{}", &statement[..end])
    }

    fn database() -> SqliteConnection {
        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite database");
        connection
            .batch_execute(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE catalog_publishers (
                     publisher_id INTEGER PRIMARY KEY,
                     publisher_key TEXT NOT NULL UNIQUE,
                     display_name TEXT NOT NULL
                 );
                 CREATE TABLE catalogs (
                     catalog_id INTEGER PRIMARY KEY,
                     publisher_id INTEGER NOT NULL REFERENCES catalog_publishers(publisher_id),
                     catalog_key TEXT NOT NULL UNIQUE,
                     display_name TEXT NOT NULL
                 );
                 CREATE TABLE catalog_source_files (
                     source_file_id INTEGER PRIMARY KEY,
                     byte_length INTEGER NOT NULL
                 );
                 CREATE TABLE catalog_reading_rules (
                     reading_rules_id INTEGER PRIMARY KEY,
                     format_family TEXT NOT NULL
                 );
                 CREATE TABLE catalog_editions (
                     edition_id INTEGER PRIMARY KEY
                 );
                 CREATE TABLE catalog_imports (
                     import_id INTEGER PRIMARY KEY,
                     import_key TEXT NOT NULL UNIQUE,
                     catalog_id INTEGER NOT NULL REFERENCES catalogs(catalog_id),
                     source_file_id INTEGER NOT NULL REFERENCES catalog_source_files(source_file_id),
                     reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id),
                     edition_id INTEGER REFERENCES catalog_editions(edition_id),
                     status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
                     started_at TEXT NOT NULL,
                     finished_at TEXT,
                     CHECK ((status = 'running' AND finished_at IS NULL)
                         OR (status = 'succeeded' AND edition_id IS NOT NULL AND finished_at IS NOT NULL)
                         OR (status = 'failed' AND edition_id IS NULL AND finished_at IS NOT NULL)),
                     UNIQUE (import_id, source_file_id, reading_rules_id)
                 );
                 INSERT INTO catalog_publishers VALUES (11, 'pub', 'Publisher');
                 INSERT INTO catalogs VALUES (12, 11, 'catalog', 'Catalog');
                 INSERT INTO catalog_source_files VALUES (101, 20), (102, 30);
                 INSERT INTO catalog_reading_rules VALUES (201, 'mame'), (202, 'logiqx');
                 INSERT INTO catalog_editions VALUES (301);
                 INSERT INTO catalog_imports VALUES
                     (401, 'running-import', 12, 101, 201, NULL, 'running', 'start', NULL),
                     (402, 'finished-import', 12, 102, 202, 301, 'succeeded', 'start', 'finish');",
            )
            .expect("create minimal candidate import fixture");
        connection
            .batch_execute(&message_schema_fragment())
            .expect("install the candidate diagnostic table definition");
        connection
            .batch_execute(&coordinate_trigger("candidate_message_coordinates_insert"))
            .expect("install generated diagnostic insert-coordinate guard");
        connection
            .batch_execute(&coordinate_trigger("candidate_message_coordinates_update"))
            .expect("install generated diagnostic update-coordinate guard");
        connection
    }

    fn stored_message(connection: &mut SqliteConnection, id: ImportMessageId) -> StoredMessage {
        sql_query(
            "SELECT import_id, message_order, severity, code, message,
                    source_file_id, reading_rules_id, edition_id, excerpt,
                    source_view, excerpt_source_start, excerpt_problem_start,
                    excerpt_problem_end, source_problem_start, source_problem_end,
                    original_problem_start, original_problem_end
             FROM catalog_import_messages WHERE message_id = ?",
        )
        .bind::<BigInt, _>(id.as_i64())
        .get_result(connection)
        .expect("load inserted candidate message")
    }

    fn span(start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(start, end).expect("ordered span")
    }

    #[test]
    fn insertion_uses_the_running_import_identity_and_keeps_decoded_and_original_ranges_distinct() {
        let mut connection = database();
        let evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            b"xxbadyy",
            Some(span(2, 5)),
            Some(span(12, 15)),
            Some(20),
        )
        .expect("both byte views have valid independent ranges");
        assert_eq!(evidence.view(), Some(ByteView::TransportDecodedXmlBytes));
        assert_eq!(evidence.source_problem(), Some(span(2, 5)));
        assert_eq!(evidence.original_problem(), Some(span(12, 15)));
        assert_eq!(evidence.excerpt(), Some(&b"xxbadyy"[..]));
        let message_id = insert_source_message(
            &mut connection,
            ImportId::try_from(401).expect("positive import ID"),
            MessageDraft::new(
                0,
                MessageSeverity::Warning,
                "invalid_record",
                "record could not be read",
                &evidence,
            ),
        )
        .expect("running import accepts a source-only message");
        let row = stored_message(&mut connection, message_id);

        assert_eq!(row.import_id, 401);
        assert_eq!(row.message_order, 0);
        assert_eq!(row.severity, "warning");
        assert_eq!(row.code, "invalid_record");
        assert_eq!(row.message, "record could not be read");
        assert_eq!(row.source_file_id, 101);
        assert_eq!(row.reading_rules_id, 201);
        assert_eq!(row.edition_id, None);
        assert_eq!(row.excerpt.as_deref(), Some(&b"xxbadyy"[..]));
        assert_eq!(
            row.source_view.as_deref(),
            Some("transport_decoded_xml_bytes")
        );
        assert_eq!(row.excerpt_source_start, Some(0));
        assert_eq!(row.excerpt_problem_start, Some(2));
        assert_eq!(row.excerpt_problem_end, Some(5));
        assert_eq!(row.source_problem_start, Some(2));
        assert_eq!(row.source_problem_end, Some(5));
        assert_eq!(row.original_problem_start, Some(12));
        assert_eq!(row.original_problem_end, Some(15));
    }

    #[test]
    fn insertion_refuses_missing_and_terminal_imports() {
        let mut connection = database();
        let evidence = MessageEvidence::capture(
            ByteView::RetainedOriginalBytes,
            b"bad",
            Some(span(0, 3)),
            Some(span(0, 3)),
            Some(3),
        )
        .expect("valid retained-original evidence");
        let attempt = |connection: &mut SqliteConnection, import_id| {
            insert_source_message(
                connection,
                import_id,
                MessageDraft::new(
                    0,
                    MessageSeverity::Error,
                    "bad_record",
                    "invalid source",
                    &evidence,
                ),
            )
        };

        assert!(
            attempt(
                &mut connection,
                ImportId::try_from(999).expect("positive import ID"),
            )
            .is_err()
        );
        assert!(
            attempt(
                &mut connection,
                ImportId::try_from(402).expect("positive terminal import ID"),
            )
            .is_err()
        );
    }

    #[test]
    fn candidate_sql_guard_rejects_inconsistent_retained_original_ranges() {
        let mut connection = database();
        let result = connection.batch_execute(
            "INSERT INTO catalog_import_messages (
                 import_id, message_order, severity, code, message,
                 source_file_id, reading_rules_id, excerpt, source_view,
                 excerpt_source_start, excerpt_problem_start, excerpt_problem_end,
                 source_problem_start, source_problem_end,
                 original_problem_start, original_problem_end
             ) VALUES (
                 401, 0, 'error', 'bad_range', 'bad range',
                 101, 201, X'61626364', 'retained_original_bytes',
                 2, 0, 2, 2, 4, 3, 5
             )",
        );

        assert!(result.is_err());
    }

    #[test]
    fn canonical_schema_preserves_evidence_and_seals_terminal_messages() -> crate::Result<()> {
        let mut connection = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
        crate::storage::db::initialize_database(&mut connection)?;
        connection.batch_execute(
            "INSERT INTO catalog_publishers VALUES(11,'publisher','Publisher',NULL);
             INSERT INTO catalogs VALUES(12,11,'catalog','Catalog');
             INSERT INTO catalog_source_files VALUES(101,zeroblob(32),NULL,20,'source','zstd');
             INSERT INTO catalog_decoded_xml_views VALUES(101,7);
             INSERT INTO catalog_reading_rules VALUES(201,'rules','mame','observed','0.289','test','v1');
             INSERT INTO catalog_imports(import_id,import_key,catalog_id,source_file_id,
                reading_rules_id,status,started_at) VALUES(401,'attempt',12,101,201,'running','start');"
        )?;
        let evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            b"xxbadyy",
            Some(span(2, 5)),
            Some(span(12, 15)),
            Some(20),
        )
        .expect("independent valid views");
        let id = insert_source_message(
            &mut connection,
            ImportId::try_from(401).expect("positive import ID"),
            MessageDraft::new(
                0,
                MessageSeverity::Warning,
                "invalid_field",
                "invalid field",
                &evidence,
            ),
        )?;
        let row = stored_message(&mut connection, id);
        assert_eq!(row.source_problem_start, Some(2));
        assert_eq!(row.original_problem_start, Some(12));
        assert_eq!(row.excerpt.as_deref(), Some(b"xxbadyy".as_slice()));
        connection.batch_execute(
            "UPDATE catalog_imports SET status='failed',finished_at='finish' WHERE import_id=401;",
        )?;
        assert!(
            connection
                .batch_execute("UPDATE catalog_import_messages SET message='rewritten'")
                .is_err()
        );
        assert!(
            connection
                .batch_execute("DELETE FROM catalog_import_messages")
                .is_err()
        );
        assert!(
            insert_source_message(
                &mut connection,
                ImportId::try_from(401).expect("positive terminal import ID"),
                MessageDraft::new(1, MessageSeverity::Error, "late", "late message", &evidence),
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn bounded_excerpt_keeps_the_exact_entire_span_highlight() {
        let source = vec![b'x'; MAX_EXCERPT_BYTES * 4];
        let problem = span(900, 907);
        let evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            &source,
            Some(problem),
            Some(span(1_100, 1_107)),
            Some(source.len() * 2),
        )
        .expect("in-bounds source and original ranges");

        let excerpt = evidence
            .excerpt
            .as_ref()
            .expect("known source span saves bytes");
        assert!(excerpt.len() <= MAX_EXCERPT_BYTES);
        assert_eq!(evidence.source_problem, Some(problem));
        assert_eq!(evidence.original_problem, Some(span(1_100, 1_107)));
        assert_eq!(
            excerpt.get(
                evidence
                    .excerpt_problem
                    .expect("full span highlighted")
                    .start
                    ..evidence.excerpt_problem.expect("full span highlighted").end
            ),
            Some(&source[900..907])
        );
    }

    #[test]
    fn a_span_larger_than_the_excerpt_is_not_partially_highlighted() {
        let source = vec![b'z'; MAX_EXCERPT_BYTES * 3];
        let problem = span(700, 700 + MAX_EXCERPT_BYTES + 9);
        let evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            &source,
            Some(problem),
            None,
            None,
        )
        .expect("source range is valid");

        assert!(
            evidence
                .excerpt
                .as_ref()
                .expect("known source span saves bytes")
                .len()
                <= MAX_EXCERPT_BYTES
        );
        assert_eq!(evidence.source_problem, Some(problem));
        assert_eq!(evidence.excerpt_problem, None);
    }

    #[test]
    fn original_coordinates_fall_back_only_for_the_retained_original_view() {
        let source = b"head\0tail";
        let original_problem = span(4, 5);
        let retained = MessageEvidence::capture(
            ByteView::RetainedOriginalBytes,
            source,
            None,
            Some(original_problem),
            Some(source.len()),
        )
        .expect("valid retained range");
        let decoded = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            source,
            None,
            Some(original_problem),
            Some(source.len()),
        )
        .expect("valid independent original range");

        assert_eq!(retained.source_problem, Some(original_problem));
        assert_eq!(retained.original_problem, Some(original_problem));
        assert_eq!(retained.excerpt_problem, Some(span(4, 5)));
        assert_eq!(decoded.source_problem, None);
        assert_eq!(decoded.original_problem, Some(original_problem));
        assert_eq!(decoded.excerpt_problem, None);
        assert_eq!(decoded.excerpt, None);
    }

    #[test]
    fn zero_width_eof_anchor_is_saved_as_an_empty_highlight() {
        let source = b"abc";
        let eof = span(source.len(), source.len());
        let evidence = MessageEvidence::capture(
            ByteView::RetainedOriginalBytes,
            source,
            Some(eof),
            None,
            Some(source.len()),
        )
        .expect("EOF is a valid boundary");

        assert_eq!(evidence.excerpt.as_deref(), Some(source.as_slice()));
        assert_eq!(evidence.excerpt_source_start, Some(0));
        assert_eq!(
            evidence.excerpt_problem,
            Some(span(source.len(), source.len()))
        );
    }

    #[test]
    fn invalid_source_and_original_bounds_are_rejected() {
        let source = b"abc";

        assert!(
            MessageEvidence::capture(
                ByteView::TransportDecodedXmlBytes,
                source,
                Some(span(2, 4)),
                None,
                None,
            )
            .is_none()
        );
        assert!(
            MessageEvidence::capture(
                ByteView::TransportDecodedXmlBytes,
                source,
                None,
                Some(span(2, 4)),
                Some(source.len()),
            )
            .is_none()
        );
    }

    #[test]
    fn retained_original_fallback_is_checked_against_the_view_bytes() {
        assert!(
            MessageEvidence::capture(
                ByteView::RetainedOriginalBytes,
                b"abc",
                None,
                Some(span(2, 4)),
                Some(4),
            )
            .is_none()
        );
    }
}
