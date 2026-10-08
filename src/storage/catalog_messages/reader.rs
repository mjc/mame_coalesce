//! Source-free reads of persisted catalog import messages.

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use super::super::catalog_ids::{
    CatalogIdError, EditionId, ImportId, ImportMessageId, ReadingRulesId, SourceFileId,
};
use super::{ByteSpan, ByteView, MessageEvidence, MessageSeverity};
use crate::{database::Database, storage::db::Pool};

/// Reads source-only diagnostics from canonical import metadata.
pub struct CatalogMessageReader<'database> {
    pool: &'database Pool,
}

impl<'database> CatalogMessageReader<'database> {
    /// Create a reader backed by an already opened canonical database.
    #[must_use]
    pub fn new(database: &'database Database) -> Self {
        Self {
            pool: database.pool(),
        }
    }

    /// Load one diagnostic by its stable message identifier.
    pub fn by_message_id(
        &self,
        message_id: ImportMessageId,
    ) -> crate::Result<Option<SourceMessage>> {
        let mut connection = self.pool.get()?;
        let row = sql_query(format!(
            "{SOURCE_MESSAGE_SELECT} WHERE message.message_id = ?"
        ))
        .bind::<BigInt, _>(message_id.as_i64())
        .get_result::<SourceMessageRow>(&mut connection)
        .optional()?;
        row.map(SourceMessageRow::into_message).transpose()
    }

    /// Load an import's diagnostics in their persisted source order with one query.
    pub fn for_import(&self, import_id: ImportId) -> crate::Result<Vec<SourceMessage>> {
        let mut connection = self.pool.get()?;
        sql_query(format!(
            "{SOURCE_MESSAGE_SELECT} WHERE message.import_id = ? \
             ORDER BY message.message_order, message.message_id"
        ))
        .bind::<BigInt, _>(import_id.as_i64())
        .load::<SourceMessageRow>(&mut connection)?
        .into_iter()
        .map(SourceMessageRow::into_message)
        .collect()
    }
}

const SOURCE_MESSAGE_SELECT: &str = "SELECT \
    message.message_id, message.import_id, message.message_order, \
    message.severity, message.code, message.message, message.record_kind, \
    message.record_name, message.field_name, message.offending_text, \
    message.edition_id, message.source_file_id, message.reading_rules_id, \
    message.excerpt, message.source_view, message.excerpt_source_start, \
    message.excerpt_problem_start, message.excerpt_problem_end, \
    message.source_problem_start, message.source_problem_end, \
    message.original_problem_start, message.original_problem_end, \
    source.byte_length AS original_byte_length, \
    decoded.byte_length AS decoded_byte_length \
FROM catalog_import_messages AS message \
JOIN catalog_source_files AS source USING (source_file_id) \
LEFT JOIN catalog_decoded_xml_views AS decoded USING (source_file_id)";

#[derive(QueryableByName)]
struct SourceMessageRow {
    #[diesel(sql_type = BigInt)]
    message_id: i64,
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
    #[diesel(sql_type = Nullable<Text>)]
    record_kind: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    record_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    field_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    offending_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    edition_id: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_file_id: i64,
    #[diesel(sql_type = BigInt)]
    reading_rules_id: i64,
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
    #[diesel(sql_type = BigInt)]
    original_byte_length: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    decoded_byte_length: Option<i64>,
}

impl SourceMessageRow {
    fn into_message(self) -> crate::Result<SourceMessage> {
        let field = |name: &str| format!("catalog_import_messages.{name}");
        let message_id = checked_id(self.message_id, "message_id")?;
        let import_id = checked_id(self.import_id, "import_id")?;
        let message_order = stored_usize(self.message_order, &field("message_order"))?;
        let severity = match self.severity.as_str() {
            "warning" => MessageSeverity::Warning,
            "error" => MessageSeverity::Error,
            value => return Err(invalid_persisted_value(&field("severity"), value)),
        };
        let edition_id = self
            .edition_id
            .map(|value| checked_id(value, "edition_id"))
            .transpose()?;
        let source_file_id = checked_id(self.source_file_id, "source_file_id")?;
        let reading_rules_id = checked_id(self.reading_rules_id, "reading_rules_id")?;
        let original_byte_length = stored_usize(
            self.original_byte_length,
            "catalog_source_files.byte_length",
        )?;
        let original_problem = stored_span(
            self.original_problem_start,
            self.original_problem_end,
            &field("original_problem"),
        )?;
        let view = match self.source_view.as_deref() {
            None => None,
            Some("retained_original_bytes") => Some(ByteView::RetainedOriginalBytes),
            Some("transport_decoded_xml_bytes") => Some(ByteView::TransportDecodedXmlBytes),
            Some(value) => return Err(invalid_persisted_value(&field("source_view"), value)),
        };
        let source_problem = stored_span(
            self.source_problem_start,
            self.source_problem_end,
            &field("source_problem"),
        )?;
        let excerpt_problem = stored_span(
            self.excerpt_problem_start,
            self.excerpt_problem_end,
            &field("excerpt_problem"),
        )?;
        let excerpt_source_start = self
            .excerpt_source_start
            .map(|value| stored_usize(value, &field("excerpt_source_start")))
            .transpose()?;

        let view_length = match view {
            Some(ByteView::RetainedOriginalBytes) => Some(original_byte_length),
            Some(ByteView::TransportDecodedXmlBytes) => Some(stored_usize(
                self.decoded_byte_length.ok_or_else(|| {
                    invalid_persisted_value(
                        "catalog_decoded_xml_views.byte_length",
                        "missing decoded XML byte view",
                    )
                })?,
                "catalog_decoded_xml_views.byte_length",
            )?),
            None => None,
        };
        let evidence = MessageEvidence::from_persisted(
            self.excerpt,
            view,
            excerpt_source_start,
            excerpt_problem,
            source_problem,
            original_problem,
            view_length,
            Some(original_byte_length),
        )?;

        Ok(SourceMessage {
            message_id,
            import_id,
            message_order,
            severity,
            code: self.code,
            message: self.message,
            record_kind: self.record_kind,
            record_name: self.record_name,
            field_name: self.field_name,
            offending_text: self.offending_text,
            edition_id,
            source_file_id,
            reading_rules_id,
            evidence,
        })
    }
}

/// One persisted source diagnostic and the canonical identities that own it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMessage {
    message_id: ImportMessageId,
    import_id: ImportId,
    message_order: usize,
    severity: MessageSeverity,
    code: String,
    message: String,
    record_kind: Option<String>,
    record_name: Option<String>,
    field_name: Option<String>,
    offending_text: Option<String>,
    edition_id: Option<EditionId>,
    source_file_id: SourceFileId,
    reading_rules_id: ReadingRulesId,
    evidence: MessageEvidence,
}

impl SourceMessage {
    /// Stable identity of this message row.
    #[must_use]
    pub const fn message_id(&self) -> ImportMessageId {
        self.message_id
    }

    /// Import that produced this message.
    #[must_use]
    pub const fn import_id(&self) -> ImportId {
        self.import_id
    }

    /// Zero-based order within its import.
    #[must_use]
    pub const fn message_order(&self) -> usize {
        self.message_order
    }

    /// Persisted diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> MessageSeverity {
        self.severity
    }

    /// Stable diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Human-readable diagnostic text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Native source record kind, when present.
    #[must_use]
    pub fn record_kind(&self) -> Option<&str> {
        self.record_kind.as_deref()
    }

    /// Native source record name, when present.
    #[must_use]
    pub fn record_name(&self) -> Option<&str> {
        self.record_name.as_deref()
    }

    /// Native field name, when present.
    #[must_use]
    pub fn field_name(&self) -> Option<&str> {
        self.field_name.as_deref()
    }

    /// Original offending source text, when present.
    #[must_use]
    pub fn offending_text(&self) -> Option<&str> {
        self.offending_text.as_deref()
    }

    /// Edition selected by the producing import, when one exists.
    #[must_use]
    pub const fn edition_id(&self) -> Option<EditionId> {
        self.edition_id
    }

    /// Retained source-file identity.
    #[must_use]
    pub const fn source_file_id(&self) -> SourceFileId {
        self.source_file_id
    }

    /// Immutable parser-reading policy identity.
    #[must_use]
    pub const fn reading_rules_id(&self) -> ReadingRulesId {
        self.reading_rules_id
    }

    /// Bounded excerpt bytes, never loaded from the external original.
    #[must_use]
    pub fn excerpt(&self) -> Option<&[u8]> {
        self.evidence.excerpt()
    }

    /// Byte view that owns the excerpt and source span.
    #[must_use]
    pub const fn source_view(&self) -> Option<ByteView> {
        self.evidence.view()
    }

    /// Excerpt start in its source view.
    #[must_use]
    pub const fn excerpt_source_start(&self) -> Option<usize> {
        self.evidence.excerpt_source_start()
    }

    /// Complete problem range relative to the excerpt, if fully included.
    #[must_use]
    pub const fn excerpt_problem(&self) -> Option<ByteSpan> {
        self.evidence.excerpt_problem()
    }

    /// Complete problem range in the indicated source byte view.
    #[must_use]
    pub const fn source_problem(&self) -> Option<ByteSpan> {
        self.evidence.source_problem()
    }

    /// Independently retained-original problem range, when known.
    #[must_use]
    pub const fn original_problem(&self) -> Option<ByteSpan> {
        self.evidence.original_problem()
    }
}

fn stored_span(
    start: Option<i64>,
    end: Option<i64>,
    field: &str,
) -> crate::Result<Option<ByteSpan>> {
    match (start, end) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) => {
            let start = stored_usize(start, field)?;
            let end = stored_usize(end, field)?;
            ByteSpan::new(start, end)
                .map(Some)
                .ok_or_else(|| invalid_persisted_value(field, "range start is after range end"))
        }
        _ => Err(invalid_persisted_value(
            field,
            "range endpoints are not paired",
        )),
    }
}

fn stored_usize(value: i64, field: &str) -> crate::Result<usize> {
    usize::try_from(value)
        .map_err(|_| invalid_persisted_value(field, "value is negative or out of range"))
}

fn checked_id<Id>(value: i64, field: &str) -> crate::Result<Id>
where
    Id: TryFrom<i64, Error = CatalogIdError>,
{
    Id::try_from(value).map_err(|error| invalid_persisted_value(field, &error.to_string()))
}

fn invalid_persisted_value(field: &str, detail: &str) -> crate::Error {
    crate::Error::DatabaseSchema(format!("invalid persisted {field}: {detail}"))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::CatalogMessageReader;
    use crate::{
        AcquisitionMetadata, DocumentStore, PublishingSource,
        database::Database,
        domain::{CatalogKey, DocumentDigest, PublishingSourceKey},
        storage::{
            catalog_ids::{ImportId, PublisherId},
            catalog_lists::{CatalogMetadata, CatalogRepository},
            catalog_messages::{
                ByteSpan, ByteView, MessageDraft, MessageEvidence, MessageSeverity,
                insert_source_message,
            },
            reading_rules::{FormatFamily, ReadingRulesSpec, issue},
        },
    };
    use diesel::{
        Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
        sql_query, sql_types::BigInt,
    };

    #[derive(QueryableByName)]
    struct PublisherRow {
        #[diesel(sql_type = BigInt)]
        publisher_id: i64,
    }

    #[derive(QueryableByName)]
    struct ImportRow {
        #[diesel(sql_type = BigInt)]
        import_id: i64,
    }

    #[test]
    fn source_message_reads_use_canonical_metadata_and_terminal_rows() -> crate::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))
            .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;

        let mut original = vec![b' '; 700];
        let prefix =
            b"<datafile><header><name>messages</name></header><game name=\"fixture\"><description>";
        original[..prefix.len()].copy_from_slice(prefix);
        original[100..650].fill(b'!');
        let suffix = b"</description></game></datafile>";
        let suffix_start = original.len() - suffix.len();
        original[suffix_start..].copy_from_slice(suffix);
        let retained = {
            let store = DocumentStore::open(path.as_str())?;
            let source = PublishingSource::new("message-reader", "Message reader fixture");
            store.register_source(&source)?;
            store.retain(
                &AcquisitionMetadata {
                    source_key: PublishingSourceKey::new("message-reader"),
                    source_uri: Some("https://example.invalid/catalog.dat".to_owned()),
                    method: Some("https".to_owned()),
                    transport_headers: Vec::new(),
                    expected_sha256: Some(DocumentDigest::from_bytes(original.as_slice())),
                },
                original.as_slice(),
            )?
        };

        let database = Database::open(&path)?;
        let mut connection = SqliteConnection::establish(path.as_str())
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
        connection.batch_execute("PRAGMA foreign_keys=ON")?;

        let publisher = sql_query(
            "SELECT publisher_id FROM catalog_publishers \
             WHERE publisher_key='message-reader'",
        )
        .get_result::<PublisherRow>(&mut connection)?;
        let publisher_id = PublisherId::try_from(publisher.publisher_id)?;
        let catalog_key = CatalogKey::new("message-reader-catalog");
        let catalog_id = CatalogRepository::new(&database).register(
            &catalog_key,
            publisher_id,
            "Message reader catalog",
        )?;
        let catalog_metadata: CatalogMetadata = CatalogRepository::new(&database)
            .load(&catalog_key)?
            .expect("registered catalog metadata");
        assert_eq!(catalog_metadata.catalog_id, catalog_id);

        let reading_rules_id = issue(
            &mut connection,
            &ReadingRulesSpec {
                rules_key: "message-reader-rules".to_owned(),
                format_family: FormatFamily::Logiqx,
                dialect: "test".to_owned(),
                specification_version: "1".to_owned(),
                parser_version: "test".to_owned(),
                rules_version: "1".to_owned(),
                replace_nul: None,
                file_byte_contract: None,
            },
        )?;
        sql_query(
            "INSERT INTO catalog_imports \
             (import_key, catalog_id, source_file_id, reading_rules_id, status, started_at) \
             VALUES ('message-reader-import', ?, ?, ?, 'running', '2026-10-08T00:00:00Z')",
        )
        .bind::<BigInt, _>(catalog_id.as_i64())
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .bind::<BigInt, _>(reading_rules_id.as_i64())
        .execute(&mut connection)?;
        let import = sql_query(
            "SELECT import_id FROM catalog_imports WHERE import_key='message-reader-import'",
        )
        .get_result::<ImportRow>(&mut connection)?;
        let import_id = ImportId::try_from(import.import_id)?;

        let decoded_bytes = vec![b'd'; 100];
        sql_query(
            "INSERT INTO catalog_decoded_xml_views (source_file_id, byte_length) VALUES (?, ?)",
        )
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .bind::<BigInt, _>(decoded_bytes.len() as i64)
        .execute(&mut connection)?;

        let span = ByteSpan::new(100, 650).expect("ordered fixture span");
        let evidence = MessageEvidence::capture(
            ByteView::RetainedOriginalBytes,
            &original,
            Some(span),
            Some(span),
            Some(original.len()),
        )
        .expect("valid source evidence");
        let message_id = insert_source_message(
            &mut connection,
            import_id,
            MessageDraft::new(
                0,
                MessageSeverity::Error,
                "invalid-record",
                "The source range exceeds the bounded excerpt",
                &evidence,
            ),
        )?;

        let decoded_span = ByteSpan::new(20, 30).expect("ordered decoded-view span");
        let original_span = ByteSpan::new(200, 210).expect("ordered original-view span");
        let decoded_evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            &decoded_bytes,
            Some(decoded_span),
            Some(original_span),
            Some(original.len()),
        )
        .expect("independent decoded and original coordinates");
        let decoded_message_id = insert_source_message(
            &mut connection,
            import_id,
            MessageDraft::new(
                1,
                MessageSeverity::Warning,
                "decoded-view-note",
                "The decoded source has an independent original range",
                &decoded_evidence,
            ),
        )?;

        let no_excerpt_evidence = MessageEvidence::capture(
            ByteView::TransportDecodedXmlBytes,
            &decoded_bytes,
            None,
            Some(original_span),
            Some(original.len()),
        )
        .expect("original-only evidence need not fabricate an excerpt");
        let no_excerpt_message_id = insert_source_message(
            &mut connection,
            import_id,
            MessageDraft::new(
                2,
                MessageSeverity::Warning,
                "original-only-note",
                "Original coordinates are present without an excerpt",
                &no_excerpt_evidence,
            ),
        )?;

        let reader = CatalogMessageReader::new(&database);
        sql_query(
            "UPDATE catalog_import_messages \
             SET source_problem_end=701, original_problem_end=701 WHERE message_id=?",
        )
        .bind::<BigInt, _>(message_id.as_i64())
        .execute(&mut connection)?;
        assert!(
            reader.by_message_id(message_id).is_err(),
            "persisted ranges beyond the retained source must be rejected"
        );
        sql_query(
            "UPDATE catalog_import_messages \
             SET source_problem_end=650, original_problem_end=650 WHERE message_id=?",
        )
        .bind::<BigInt, _>(message_id.as_i64())
        .execute(&mut connection)?;

        sql_query(
            "UPDATE catalog_imports SET status='failed', finished_at='2026-10-08T00:00:01Z' \
             WHERE import_id=?",
        )
        .bind::<BigInt, _>(import_id.as_i64())
        .execute(&mut connection)?;
        assert!(
            sql_query("DELETE FROM catalog_import_messages WHERE message_id=?")
                .bind::<BigInt, _>(message_id.as_i64())
                .execute(&mut connection)
                .is_err(),
            "terminal import diagnostics remain immutable"
        );

        let by_message = reader
            .by_message_id(message_id)?
            .expect("message exists after terminal transition");
        assert_eq!(by_message.message_id(), message_id);
        assert_eq!(by_message.import_id(), import_id);
        assert_eq!(by_message.severity(), MessageSeverity::Error);
        assert_eq!(by_message.code(), "invalid-record");
        assert_eq!(
            by_message.message(),
            "The source range exceeds the bounded excerpt"
        );
        assert_eq!(by_message.source_file_id(), retained.source_file_id);
        assert_eq!(by_message.reading_rules_id(), reading_rules_id);
        assert_eq!(
            by_message.source_view(),
            Some(ByteView::RetainedOriginalBytes)
        );
        assert!(
            by_message
                .excerpt()
                .is_some_and(|excerpt| excerpt.len() <= super::super::MAX_EXCERPT_BYTES)
        );
        assert!(by_message.excerpt_source_start().is_some());
        assert_eq!(by_message.excerpt_problem(), None);
        assert_eq!(by_message.source_problem(), Some(span));
        assert_eq!(by_message.original_problem(), Some(span));

        let by_import = reader.for_import(import_id)?;
        assert_eq!(by_import.len(), 3);
        assert_eq!(by_import[0], by_message);
        assert_eq!(by_import[1].message_id(), decoded_message_id);
        assert_eq!(
            by_import[1].source_view(),
            Some(ByteView::TransportDecodedXmlBytes)
        );
        assert_eq!(by_import[1].source_problem(), Some(decoded_span));
        assert_eq!(by_import[1].original_problem(), Some(original_span));
        assert_ne!(
            by_import[1].source_problem(),
            by_import[1].original_problem()
        );
        assert!(by_import[1].excerpt().is_some());
        assert_eq!(by_import[2].message_id(), no_excerpt_message_id);
        assert_eq!(by_import[2].source_view(), None);
        assert_eq!(by_import[2].excerpt(), None);
        assert_eq!(by_import[2].source_problem(), None);
        assert_eq!(by_import[2].original_problem(), Some(original_span));
        Ok(())
    }
}
