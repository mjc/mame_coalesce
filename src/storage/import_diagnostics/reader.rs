//! Read persisted import diagnostics without opening source documents.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable},
};

use crate::{
    database::Database,
    diagnostics::{ByteRange, CoordinateConvention, ExcerptView, SourceExcerpt},
    domain::{
        CatalogKey, CatalogRegistryId, CatalogSetId, DocumentKey, ImportRunKey, NoIntroArchiveId,
        ParserInterpretationKey, PublishingSourceKey, SnapshotKey,
    },
    logiqx::RecordLocation,
    no_intro_db_xml::XmlSourceExtent,
    storage::{
        catalog_content::registry_id,
        catalog_files::{NoIntroDumpSourceId, NoIntroReleaseId},
    },
};

use super::model::{
    DiagnosticCatalog, DiagnosticCode, DiagnosticCursor, DiagnosticDocument,
    DiagnosticInterpretation, DiagnosticKey, DiagnosticOrder, DiagnosticPage, DiagnosticPageLimit,
    DiagnosticQueryError, DiagnosticRun, DiagnosticSeverity, DiagnosticSnapshot, DiagnosticSource,
    DocumentRetention, ImportDiagnostic, ImportRunStatus, NoIntroDiagnosticOwner,
};

type QueryResult<T> = Result<T, DiagnosticQueryError>;

macro_rules! row {
    ($name:ident { $($field:ident: $rust_ty:ty => $sql_ty:ty),* $(,)? }) => {
        #[derive(QueryableByName)]
        struct $name {
            $(#[diesel(sql_type = $sql_ty)] $field: $rust_ty,)*
        }
    };
}

row!(RunRow {
    run_key: Vec<u8> => Binary,
    status: Vec<u8> => Binary,
    started_at: Option<Vec<u8>> => Nullable<Binary>,
    finished_at: Option<Vec<u8>> => Nullable<Binary>,
    run_snapshot: Option<Vec<u8>> => Nullable<Binary>,
    document_key: Vec<u8> => Binary,
    document_sha1: Option<Vec<u8>> => Nullable<Binary>,
    document_sha256: Option<Vec<u8>> => Nullable<Binary>,
    byte_length: Option<i64> => Nullable<BigInt>,
    format_hint: Option<Vec<u8>> => Nullable<Binary>,
    retention_status: Vec<u8> => Binary,
    object_key: Option<Vec<u8>> => Nullable<Binary>,
    catalog_key: Vec<u8> => Binary,
    catalog_name: Vec<u8> => Binary,
    source_key: Vec<u8> => Binary,
    source_name: Vec<u8> => Binary,
    interpretation_key: Vec<u8> => Binary,
    format: Vec<u8> => Binary,
    parser_name: Option<Vec<u8>> => Nullable<Binary>,
    parser_version: Option<Vec<u8>> => Nullable<Binary>,
    rules_version: Option<Vec<u8>> => Nullable<Binary>,
    actual_snapshot: Option<Vec<u8>> => Nullable<Binary>,
    valid: i64 => BigInt,
});

row!(RawDiagnostic {
    run_key: Vec<u8> => Binary,
    document_key: Vec<u8> => Binary,
    key: Vec<u8> => Binary,
    order_text: Vec<u8> => Binary,
    severity: Vec<u8> => Binary,
    code: Vec<u8> => Binary,
    message: Vec<u8> => Binary,
    record_kind: Option<Vec<u8>> => Nullable<Binary>,
    record_name: Option<Vec<u8>> => Nullable<Binary>,
    field_name: Option<Vec<u8>> => Nullable<Binary>,
    offending_text: Option<Vec<u8>> => Nullable<Binary>,
    source_line: Option<i64> => Nullable<BigInt>,
    source_column: Option<i64> => Nullable<BigInt>,
    excerpt: Option<Vec<u8>> => Nullable<Binary>,
    excerpt_view: Option<Vec<u8>> => Nullable<Binary>,
    excerpt_start: Option<i64> => Nullable<BigInt>,
    problem_start: Option<i64> => Nullable<BigInt>,
    problem_end: Option<i64> => Nullable<BigInt>,
    source_problem_start: Option<i64> => Nullable<BigInt>,
    source_problem_end: Option<i64> => Nullable<BigInt>,
    original_problem_start: Option<i64> => Nullable<BigInt>,
    original_problem_end: Option<i64> => Nullable<BigInt>,
    coordinate_view: Option<Vec<u8>> => Nullable<Binary>,
    column_convention: Option<Vec<u8>> => Nullable<Binary>,
    valid: i64 => BigInt,
});

row!(SummaryRow {
    run_key: Vec<u8> => Binary,
    document_key: Vec<u8> => Binary,
    diagnostic_order: Option<i64> => Nullable<BigInt>,
    message: Vec<u8> => Binary,
    valid: i64 => BigInt,
});

row!(OwnerRow {
    diagnostic_key: Vec<u8> => Binary,
    link_run_key: Vec<u8> => Binary,
    link_snapshot_key: Vec<u8> => Binary,
    owner_key: Vec<u8> => Binary,
    owner_order: Option<Vec<u8>> => Nullable<Binary>,
    ancestry_snapshot: Option<Vec<u8>> => Nullable<Binary>,
    start_line: Option<Vec<u8>> => Nullable<Binary>,
    start_column: Option<Vec<u8>> => Nullable<Binary>,
    end_line: Option<Vec<u8>> => Nullable<Binary>,
    end_column: Option<Vec<u8>> => Nullable<Binary>,
    valid: i64 => BigInt,
});

const RUN_SQL: &str = "
SELECT CAST(r.run_key AS BLOB) AS run_key, CAST(r.status AS BLOB) AS status,
       CAST(r.started_at AS BLOB) AS started_at, CAST(r.finished_at AS BLOB) AS finished_at,
       CAST(r.snapshot_key AS BLOB) AS run_snapshot,
       CAST(d.document_key AS BLOB) AS document_key, d.sha1 AS document_sha1,
       d.sha256 AS document_sha256,
       CASE WHEN typeof(d.byte_length)='integer' THEN d.byte_length END AS byte_length,
       CAST(d.format_hint AS BLOB) AS format_hint,
       CAST(d.retention_status AS BLOB) AS retention_status,
       CAST(d.object_key AS BLOB) AS object_key,
       CAST(c.catalog_key AS BLOB) AS catalog_key,
       CAST(c.display_name AS BLOB) AS catalog_name,
       CAST(s.source_key AS BLOB) AS source_key,
       CAST(s.display_name AS BLOB) AS source_name,
       CAST(p.interpretation_key AS BLOB) AS interpretation_key,
       CAST(p.format AS BLOB) AS format,
       CAST(p.parser_name AS BLOB) AS parser_name,
       CAST(p.parser_version AS BLOB) AS parser_version,
       CAST(p.rules_version AS BLOB) AS rules_version,
       CAST(snapshot.snapshot_key AS BLOB) AS actual_snapshot,
       typeof(r.run_key)='text' AND typeof(r.status)='text'
       AND (r.started_at IS NULL OR typeof(r.started_at)='text')
       AND (r.finished_at IS NULL OR typeof(r.finished_at)='text')
       AND (r.snapshot_key IS NULL OR typeof(r.snapshot_key)='text')
       AND typeof(d.document_key)='text'
       AND (d.sha1 IS NULL OR (typeof(d.sha1)='blob' AND length(d.sha1)=20))
       AND (d.sha256 IS NULL OR (typeof(d.sha256)='blob' AND length(d.sha256)=32))
       AND (d.byte_length IS NULL OR (typeof(d.byte_length)='integer' AND d.byte_length>=0))
       AND (d.format_hint IS NULL OR typeof(d.format_hint)='text')
       AND typeof(d.retention_status)='text'
       AND (d.object_key IS NULL OR typeof(d.object_key)='text')
       AND typeof(c.catalog_key)='text' AND typeof(c.display_name)='text'
       AND typeof(s.source_key)='text' AND typeof(s.display_name)='text'
       AND typeof(p.interpretation_key)='text' AND typeof(p.format)='text'
       AND (p.parser_name IS NULL OR typeof(p.parser_name)='text')
       AND (p.parser_version IS NULL OR typeof(p.parser_version)='text')
       AND (p.rules_version IS NULL OR typeof(p.rules_version)='text')
       AND (r.snapshot_key IS NULL OR
            (typeof(snapshot.snapshot_key)='text'
             AND snapshot.catalog_key=r.catalog_key
             AND snapshot.document_key=r.document_key
             AND snapshot.interpretation_key=r.interpretation_key)) AS valid
FROM import_runs AS r
JOIN documents AS d ON d.document_key=r.document_key
JOIN catalogs AS c ON c.catalog_key=r.catalog_key
JOIN publishing_sources AS s ON s.source_key=c.source_key
JOIN parser_interpretations AS p ON p.interpretation_key=r.interpretation_key
LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=r.snapshot_key
WHERE r.run_key=?";

const PAGE_PROJECTION: &str = "
SELECT CAST(d.run_key AS BLOB) AS run_key,
       CAST(d.document_key AS BLOB) AS document_key,
       CAST(d.diagnostic_key AS BLOB) AS key,
       CAST(d.diagnostic_order AS BLOB) AS order_text,
       CAST(d.severity AS BLOB) AS severity, CAST(d.code AS BLOB) AS code,
       CAST(d.message AS BLOB) AS message,
       CAST(d.record_kind AS BLOB) AS record_kind,
       CAST(d.record_name AS BLOB) AS record_name,
       CAST(d.field_name AS BLOB) AS field_name,
       CAST(d.offending_text AS BLOB) AS offending_text,
       CASE WHEN typeof(d.source_line)='integer' THEN d.source_line END AS source_line,
       CASE WHEN typeof(d.source_column)='integer' THEN d.source_column END AS source_column,
       d.source_excerpt AS excerpt, CAST(d.excerpt_view AS BLOB) AS excerpt_view,
       CASE WHEN typeof(d.excerpt_start_byte)='integer' THEN d.excerpt_start_byte END AS excerpt_start,
       CASE WHEN typeof(d.problem_start_byte)='integer' THEN d.problem_start_byte END AS problem_start,
       CASE WHEN typeof(d.problem_end_byte)='integer' THEN d.problem_end_byte END AS problem_end,
       CASE WHEN typeof(d.source_problem_start_byte)='integer' THEN d.source_problem_start_byte END AS source_problem_start,
       CASE WHEN typeof(d.source_problem_end_byte)='integer' THEN d.source_problem_end_byte END AS source_problem_end,
       CASE WHEN typeof(d.original_problem_start_byte)='integer' THEN d.original_problem_start_byte END AS original_problem_start,
       CASE WHEN typeof(d.original_problem_end_byte)='integer' THEN d.original_problem_end_byte END AS original_problem_end,
       CAST(d.coordinate_view AS BLOB) AS coordinate_view,
       CAST(d.column_convention AS BLOB) AS column_convention,
       typeof(d.run_key)='text' AND typeof(d.document_key)='text'
       AND typeof(d.diagnostic_key)='text' AND typeof(d.diagnostic_order)='integer'
       AND d.diagnostic_order>=0 AND typeof(d.severity)='text' AND typeof(d.code)='text'
       AND typeof(d.message)='text'
       AND (d.record_kind IS NULL OR typeof(d.record_kind)='text')
       AND (d.record_name IS NULL OR typeof(d.record_name)='text')
       AND (d.field_name IS NULL OR typeof(d.field_name)='text')
       AND (d.offending_text IS NULL OR typeof(d.offending_text)='text')
       AND (d.source_line IS NULL OR typeof(d.source_line)='integer')
       AND (d.source_column IS NULL OR typeof(d.source_column)='integer')
       AND (d.source_excerpt IS NULL OR typeof(d.source_excerpt)='blob')
       AND (d.excerpt_view IS NULL OR typeof(d.excerpt_view)='text')
       AND (d.excerpt_start_byte IS NULL OR typeof(d.excerpt_start_byte)='integer')
       AND (d.problem_start_byte IS NULL OR typeof(d.problem_start_byte)='integer')
       AND (d.problem_end_byte IS NULL OR typeof(d.problem_end_byte)='integer')
       AND (d.source_problem_start_byte IS NULL OR typeof(d.source_problem_start_byte)='integer')
       AND (d.source_problem_end_byte IS NULL OR typeof(d.source_problem_end_byte)='integer')
       AND (d.original_problem_start_byte IS NULL OR typeof(d.original_problem_start_byte)='integer')
       AND (d.original_problem_end_byte IS NULL OR typeof(d.original_problem_end_byte)='integer')
       AND (d.coordinate_view IS NULL OR typeof(d.coordinate_view)='text')
       AND (d.column_convention IS NULL OR typeof(d.column_convention)='text') AS valid
FROM import_diagnostics AS d";

fn page_sql(has_cursor: bool) -> String {
    let predicate = if has_cursor {
        "WHERE d.run_key=? AND d.diagnostic_order>?"
    } else {
        "WHERE d.run_key=?"
    };
    format!("{PAGE_PROJECTION} {predicate} ORDER BY d.diagnostic_order LIMIT ?")
}

const SUMMARY_SQL: &str = "
SELECT CAST(d.run_key AS BLOB) AS run_key,
       CAST(d.document_key AS BLOB) AS document_key,
       CASE WHEN typeof(d.diagnostic_order)='integer' THEN d.diagnostic_order END AS diagnostic_order,
       CAST(d.message AS BLOB) AS message,
       typeof(d.run_key)='text' AND typeof(d.document_key)='text'
       AND typeof(d.diagnostic_order)='integer' AND d.diagnostic_order=0
       AND typeof(d.message)='text' AS valid
FROM import_diagnostics AS d WHERE d.run_key=?
ORDER BY d.diagnostic_order LIMIT 1";

// A byte-identical BLOB key does not match the TEXT paging predicate. Probe
// that one indexed key so corrupted rows cannot masquerade as an empty run.
const BLOB_RUN_KEY_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM import_diagnostics WHERE run_key=?) AS count";

/// Load one source-free diagnostic page in a consistent read transaction.
///
/// The returned context includes retained-document metadata only; this query
/// never opens the document or depends on its current availability.
pub fn for_run(
    database: &Database,
    run: &ImportRunKey,
    cursor: Option<&DiagnosticCursor>,
    limit: DiagnosticPageLimit,
) -> QueryResult<DiagnosticPage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let registry = registry_id(connection)?;
        validate_cursor(cursor, *run, registry)?;
        let run_info = load_run(connection, run)?;
        validate_cursor_anchor(connection, run, cursor)?;
        validate_diagnostic_run_key(connection, run)?;
        let summary = load_summary(connection, &run_info)?;
        let rows = load_page_rows(connection, &run_info, cursor, limit)?;
        let mut diagnostics = rows
            .into_iter()
            .map(parse_diagnostic)
            .collect::<QueryResult<Vec<_>>>()?;
        validate_page_orders(&diagnostics, cursor)?;
        let has_more = diagnostics.len() > limit.get();
        diagnostics.truncate(limit.get());
        let owners = load_owners(connection, run, &run_info, &diagnostics)?;
        for diagnostic in &mut diagnostics {
            diagnostic.owners = owners
                .get(diagnostic.key.as_str())
                .cloned()
                .unwrap_or_default();
            validate_owners(&run_info, diagnostic)?;
        }
        let next_cursor = if has_more {
            diagnostics.last().map(|last| DiagnosticCursor {
                registry,
                run: *run,
                order: last.order,
                key: last.key.clone(),
            })
        } else {
            None
        };
        Ok(DiagnosticPage {
            run: DiagnosticRun {
                summary,
                ..run_info
            },
            diagnostics,
            next_cursor,
        })
    })
}

fn validate_cursor(
    cursor: Option<&DiagnosticCursor>,
    run: ImportRunKey,
    registry: CatalogRegistryId,
) -> QueryResult<()> {
    let Some(cursor) = cursor else { return Ok(()) };
    if cursor.run != run {
        return Err(DiagnosticQueryError::CursorRunMismatch);
    }
    if cursor.registry != registry {
        return Err(DiagnosticQueryError::CursorRegistryMismatch);
    }
    Ok(())
}

fn load_run(connection: &mut SqliteConnection, run: &ImportRunKey) -> QueryResult<DiagnosticRun> {
    let run_text = run.to_string();
    let row = sql_query(RUN_SQL)
        .bind::<diesel::sql_types::Text, _>(&run_text)
        .get_result::<RunRow>(connection)
        .optional()?
        .ok_or(DiagnosticQueryError::MissingRun(*run))?;
    if row.valid != 1 || text(row.run_key, "run key")? != run_text {
        return Err(invalid(run_text));
    }
    let status = ImportRunStatus::parse(&text(row.status, "run status")?)
        .ok_or_else(|| invalid(run_text.clone()))?;
    let document_key = text(row.document_key, "document key")?
        .parse::<DocumentKey>()
        .map_err(|_| invalid(run_text.clone()))?;
    let sha1 = row
        .document_sha1
        .map(|value| digest::<20>(value, &run_text))
        .transpose()?;
    let sha256 = row
        .document_sha256
        .map(|value| digest::<32>(value, &run_text))
        .transpose()?;
    let retention = DocumentRetention::parse(&text(row.retention_status, "retention status")?)
        .ok_or_else(|| invalid(run_text.clone()))?;
    let key = row
        .run_snapshot
        .map(|value| text(value, "snapshot key"))
        .transpose()?
        .map(SnapshotKey::from_persisted);
    let actual_snapshot = row
        .actual_snapshot
        .map(|value| text(value, "snapshot provenance"))
        .transpose()?;
    if key.as_ref().map(SnapshotKey::as_str) != actual_snapshot.as_deref()
        || (status == ImportRunStatus::Failed && key.is_some())
    {
        return Err(invalid(run_text));
    }
    Ok(DiagnosticRun {
        key: *run,
        status,
        started_at: row
            .started_at
            .map(|value| text(value, "started_at"))
            .transpose()?,
        finished_at: row
            .finished_at
            .map(|value| text(value, "finished_at"))
            .transpose()?,
        summary: None,
        document: DiagnosticDocument {
            key: document_key,
            sha1,
            sha256,
            byte_length: row.byte_length,
            format_hint: row
                .format_hint
                .map(|value| text(value, "format hint"))
                .transpose()?,
            retention,
            object_key: row
                .object_key
                .map(|value| text(value, "object key"))
                .transpose()?,
        },
        catalog: DiagnosticCatalog {
            key: CatalogKey::new(text(row.catalog_key, "catalog key")?),
            display_name: text(row.catalog_name, "catalog name")?,
            source: DiagnosticSource {
                key: PublishingSourceKey::new(text(row.source_key, "source key")?),
                display_name: text(row.source_name, "source name")?,
            },
        },
        interpretation: DiagnosticInterpretation {
            key: ParserInterpretationKey::from_persisted(text(
                row.interpretation_key,
                "interpretation key",
            )?),
            format: text(row.format, "format")?,
            parser_name: row
                .parser_name
                .map(|value| text(value, "parser name"))
                .transpose()?,
            parser_version: row
                .parser_version
                .map(|value| text(value, "parser version"))
                .transpose()?,
            rules_version: row
                .rules_version
                .map(|value| text(value, "rules version"))
                .transpose()?,
        },
        snapshot: key.map(|key| DiagnosticSnapshot { key }),
    })
}

fn load_summary(
    connection: &mut SqliteConnection,
    run: &DiagnosticRun,
) -> QueryResult<Option<String>> {
    let run_text = run.key.to_string();
    let document_text = run.document.key.to_string();
    let row = sql_query(SUMMARY_SQL)
        .bind::<diesel::sql_types::Text, _>(&run_text)
        .get_result::<SummaryRow>(connection)
        .optional()?;
    row.map(|row| {
        if row.valid != 1
            || row.run_key.as_slice() != run_text.as_bytes()
            || row.document_key.as_slice() != document_text.as_bytes()
            || row.diagnostic_order != Some(DiagnosticOrder::FIRST.get())
        {
            return Err(invalid(run_text));
        }
        text(row.message, "first diagnostic message")
    })
    .transpose()
}

fn validate_diagnostic_run_key(
    connection: &mut SqliteConnection,
    run: &ImportRunKey,
) -> QueryResult<()> {
    let run_text = run.to_string();
    let row = sql_query(BLOB_RUN_KEY_SQL)
        .bind::<Binary, _>(run_text.as_bytes())
        .get_result::<CountRow>(connection)?;
    if row.count != 0 {
        return Err(invalid(run_text));
    }
    Ok(())
}

fn load_page_rows(
    connection: &mut SqliteConnection,
    run: &DiagnosticRun,
    cursor: Option<&DiagnosticCursor>,
    limit: DiagnosticPageLimit,
) -> QueryResult<Vec<RawDiagnostic>> {
    let bound = i64::try_from(limit.get())
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid(run.key.to_string()))?;
    let run_text = run.key.to_string();
    let document_text = run.document.key.to_string();
    let mut query = sql_query(page_sql(cursor.is_some()))
        .into_boxed::<diesel::sqlite::Sqlite>()
        .bind::<diesel::sql_types::Text, _>(&run_text);
    if let Some(cursor) = cursor {
        query = query.bind::<BigInt, _>(cursor.order.get());
    }
    let rows = query
        .bind::<BigInt, _>(bound)
        .load::<RawDiagnostic>(connection)?;
    for row in &rows {
        if row.valid != 1
            || row.run_key.as_slice() != run_text.as_bytes()
            || row.document_key.as_slice() != document_text.as_bytes()
        {
            return Err(invalid(run_text));
        }
    }
    Ok(rows)
}

fn parse_diagnostic(mut row: RawDiagnostic) -> QueryResult<ImportDiagnostic> {
    // These small identity columns are copied before taking the excerpt BLOB;
    // the large evidence allocation is moved, never cloned.
    let order_text = text(row.order_text.clone(), "diagnostic order")?;
    let order = order_text
        .parse::<i64>()
        .ok()
        .and_then(DiagnosticOrder::new)
        .ok_or_else(|| invalid(order_text.clone()))?;
    let key_text = text(row.key.clone(), "diagnostic key")?;
    let key =
        DiagnosticKey::from_database(key_text.clone()).ok_or_else(|| invalid(key_text.clone()))?;
    let excerpt = parse_excerpt(&mut row, &key_text)?;
    let severity = DiagnosticSeverity::parse(&text(row.severity, "severity")?)
        .ok_or_else(|| invalid(key_text.clone()))?;
    let code = DiagnosticCode::parse(&text(row.code, "diagnostic code")?)
        .ok_or_else(|| invalid(key_text.clone()))?;
    let coordinates = match (&row.coordinate_view, &row.column_convention) {
        (None, None) => None,
        (Some(view), Some(convention)) => {
            let convention = CoordinateConvention::parse(
                &text(view.clone(), "coordinate view")?,
                &text(convention.clone(), "column convention")?,
            )
            .ok_or_else(|| invalid(key_text.clone()))?;
            if !matches!((row.source_line, row.source_column), (Some(line), Some(column)) if line > 0 && column > 0)
            {
                return Err(invalid(key_text));
            }
            Some(convention)
        }
        _ => return Err(invalid(key_text)),
    };
    Ok(ImportDiagnostic {
        key,
        order,
        severity,
        code,
        message: text(row.message, "diagnostic message")?,
        record_kind: row
            .record_kind
            .map(|value| text(value, "record kind"))
            .transpose()?,
        record_name: row
            .record_name
            .map(|value| text(value, "record name"))
            .transpose()?,
        field_name: row
            .field_name
            .map(|value| text(value, "field name"))
            .transpose()?,
        offending_text: row
            .offending_text
            .map(|value| text(value, "offending text"))
            .transpose()?,
        source_line: row.source_line,
        source_column: row.source_column,
        excerpt,
        coordinates,
        owners: Vec::new(),
    })
}

fn parse_excerpt(row: &mut RawDiagnostic, owner: &str) -> QueryResult<Option<SourceExcerpt>> {
    let pair = |start: Option<i64>, end: Option<i64>| -> QueryResult<Option<ByteRange>> {
        match (start, end) {
            (None, None) => Ok(None),
            (Some(start), Some(end)) => {
                let start = usize::try_from(start).map_err(|_| invalid(owner.to_owned()))?;
                let end = usize::try_from(end).map_err(|_| invalid(owner.to_owned()))?;
                ByteRange::new(start, end)
                    .map(Some)
                    .ok_or_else(|| invalid(owner.to_owned()))
            }
            _ => Err(invalid(owner.to_owned())),
        }
    };
    let start = row
        .excerpt_start
        .map(|value| usize::try_from(value).map_err(|_| invalid(owner.to_owned())))
        .transpose()?;
    let problem = pair(row.problem_start, row.problem_end)?;
    let source_problem = pair(row.source_problem_start, row.source_problem_end)?;
    let original_problem = pair(row.original_problem_start, row.original_problem_end)?;
    match (row.excerpt.is_some(), row.excerpt_view.as_ref()) {
        (false, None)
            if start.is_none()
                && problem.is_none()
                && source_problem.is_none()
                && original_problem.is_none() =>
        {
            Ok(None)
        }
        (true, Some(view)) => {
            let view = ExcerptView::parse(&text(view.clone(), "excerpt view")?)
                .ok_or_else(|| invalid(owner.to_owned()))?;
            let bytes = row
                .excerpt
                .take()
                .ok_or_else(|| invalid(owner.to_owned()))?;
            SourceExcerpt::from_saved_parts(
                bytes,
                view,
                start,
                problem,
                source_problem,
                original_problem,
            )
            .map(Some)
            .ok_or_else(|| invalid(owner.to_owned()))
        }
        _ => Err(invalid(owner.to_owned())),
    }
}

fn validate_page_orders(
    rows: &[ImportDiagnostic],
    cursor: Option<&DiagnosticCursor>,
) -> QueryResult<()> {
    let mut expected = cursor.map_or(Some(0), |cursor| cursor.order.get().checked_add(1));
    for row in rows {
        if expected != Some(row.order.get()) {
            return Err(DiagnosticQueryError::InvalidCursor);
        }
        expected = row.order.get().checked_add(1);
    }
    Ok(())
}

fn validate_cursor_anchor(
    connection: &mut SqliteConnection,
    run: &ImportRunKey,
    cursor: Option<&DiagnosticCursor>,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        let anchor = sql_query(
            "SELECT COUNT(*) AS count FROM import_diagnostics \
             WHERE run_key=? AND diagnostic_order=? AND diagnostic_key=? \
               AND typeof(run_key)='text' AND typeof(diagnostic_order)='integer' \
               AND typeof(diagnostic_key)='text'",
        )
        .bind::<diesel::sql_types::Text, _>(run.to_string())
        .bind::<BigInt, _>(cursor.order.get())
        .bind::<diesel::sql_types::Text, _>(cursor.key.as_str())
        .get_result::<CountRow>(connection)?;
        if anchor.count != 1 {
            return Err(DiagnosticQueryError::InvalidCursor);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct OwnerSql {
    relation: &'static str,
    path: OwnerPath,
    key_column: &'static str,
    key_type: &'static str,
    order_column: Option<&'static str>,
    joins: &'static str,
    ancestry: &'static str,
    start_line: &'static str,
    start_column: &'static str,
    end_line: &'static str,
    end_column: &'static str,
    owner_check: &'static str,
}

type SqlExtent = [&'static str; 4];

const EXPORT_EXTENT: SqlExtent = [
    "1",
    "1",
    "export.document_end_line",
    "export.document_end_column",
];
const HEADER_EXTENT: SqlExtent = [
    "header.source_line",
    "header.source_column",
    "header.source_end_line",
    "header.source_end_column",
];
const GAME_EXTENT: SqlExtent = [
    "sets.source_line",
    "sets.source_column",
    "game.source_end_line",
    "game.source_end_column",
];
const SOURCE_EXTENT: SqlExtent = [
    "source.source_line",
    "source.source_column",
    "source.source_end_line",
    "source.source_end_column",
];
const RELEASE_EXTENT: SqlExtent = [
    "release.source_line",
    "release.source_column",
    "release.source_end_line",
    "release.source_end_column",
];

#[derive(Clone, Copy)]
enum OwnerPath {
    Document,
    Header,
    HeaderField,
    Game,
    GameChild,
    SourceChild,
    ReleaseChild,
    SourceFile,
    ReleaseFile,
}

impl OwnerPath {
    const fn ancestors(self) -> &'static [SqlExtent] {
        match self {
            Self::Document => &[],
            Self::Header | Self::Game => &[EXPORT_EXTENT],
            Self::HeaderField => &[HEADER_EXTENT, EXPORT_EXTENT],
            Self::GameChild => &[GAME_EXTENT, EXPORT_EXTENT],
            Self::SourceChild | Self::SourceFile => &[SOURCE_EXTENT, GAME_EXTENT, EXPORT_EXTENT],
            Self::ReleaseChild | Self::ReleaseFile => &[RELEASE_EXTENT, GAME_EXTENT, EXPORT_EXTENT],
        }
    }
}

fn checked_sql_extent([line, column, end_line, end_column]: SqlExtent) -> String {
    format!(
        "typeof({line})='integer' AND {line}>0 AND typeof({column})='integer' AND {column}>0 AND typeof({end_line})='integer' AND {end_line}>0 AND typeof({end_column})='integer' AND {end_column}>0 AND ({end_line},{end_column})>({line},{column})"
    )
}

fn owner_integrity_sql(spec: OwnerSql) -> String {
    let mut child = [
        spec.start_line,
        spec.start_column,
        spec.end_line,
        spec.end_column,
    ];
    let mut checks = vec![
        checked_sql_extent(child),
        format!("typeof(owner.{})='{}'", spec.key_column, spec.key_type),
        "typeof(export.snapshot_key)='text' AND typeof(export.source_line)='integer' AND export.source_line>0 AND typeof(export.source_column)='integer' AND export.source_column>0 AND (export.source_line,export.source_column)<(export.document_end_line,export.document_end_column)".into(),
    ];
    for &parent in spec.path.ancestors() {
        checks.push(checked_sql_extent(parent));
        checks.push(format!(
            "({},{})<=({},{}) AND ({},{})>=({},{})",
            parent[0], parent[1], child[0], child[1], parent[2], parent[3], child[2], child[3]
        ));
        child = parent;
    }
    match spec.path {
        OwnerPath::Document => {}
        OwnerPath::Header => checks.push("export.header_present=1 AND typeof(export.header_present)='integer'".into()),
        OwnerPath::HeaderField => checks.push("export.header_present=1 AND typeof(export.header_present)='integer' AND typeof(header.snapshot_key)='text' AND typeof(owner.source_order)='integer' AND owner.source_order>=0".into()),
        path => {
            let game = if matches!(path, OwnerPath::Game) { "owner" } else { "game" };
            checks.push(format!("typeof({game}.set_id)='integer' AND {game}.set_id>0 AND typeof(sets.set_id)='integer' AND sets.set_id>0 AND typeof(sets.set_group_id)='integer' AND sets.set_group_id>0 AND typeof(sets.list_order)='integer' AND sets.list_order>=0 AND typeof(groups.set_group_id)='integer' AND groups.set_group_id>0 AND typeof(groups.snapshot_key)='text' AND typeof(snapshot.snapshot_key)='text' AND typeof(snapshot.interpretation_key)='text' AND typeof(interpretation.interpretation_key)='text'"));
            if !matches!(path, OwnerPath::Game) {
                checks.push("typeof(owner.source_order)='integer' AND owner.source_order>=0".into());
            }
            match path {
                OwnerPath::GameChild => checks.push("typeof(owner.set_id)='integer' AND owner.set_id>0".into()),
                OwnerPath::SourceChild | OwnerPath::SourceFile => checks.push(history_parent_sql("source", "dump_source_id")),
                OwnerPath::ReleaseChild | OwnerPath::ReleaseFile => checks.push(history_parent_sql("release", "release_id")),
                _ => {}
            }
            if matches!(path, OwnerPath::SourceFile | OwnerPath::ReleaseFile) {
                checks.push("typeof(owner.set_id)='integer' AND owner.set_id=sets.set_id AND typeof(occurrence.occurrence_id)='integer' AND typeof(occurrence.record_id)='integer' AND occurrence.record_id=sets.set_id AND typeof(occurrence.occurrence_order)='integer' AND occurrence.occurrence_order>=0".into());
            }
        }
    }
    checks.join(" AND ")
}

fn history_parent_sql(alias: &str, id_column: &str) -> String {
    format!(
        "typeof(owner.{id_column})='integer' AND owner.{id_column}>0 AND typeof({alias}.{id_column})='integer' AND {alias}.{id_column}>0 AND typeof({alias}.set_id)='integer' AND {alias}.set_id>0 AND typeof({alias}.source_order)='integer' AND {alias}.source_order>=0"
    )
}

const OWNER_SQL: [OwnerSql; 13] = [
    OwnerSql {
        relation: "no_intro_export_diagnostics",
        path: OwnerPath::Document,
        key_column: "snapshot_key",
        key_type: "text",
        order_column: None,
        joins: "LEFT JOIN no_intro_exports AS owner ON owner.snapshot_key=l.snapshot_key",
        ancestry: "owner.snapshot_key",
        start_line: "1",
        start_column: "1",
        end_line: "owner.document_end_line",
        end_column: "owner.document_end_column",
        owner_check: "1",
    },
    OwnerSql {
        relation: "no_intro_export_header_diagnostics",
        path: OwnerPath::Header,
        key_column: "snapshot_key",
        key_type: "text",
        order_column: None,
        joins: "LEFT JOIN no_intro_export_headers AS owner ON owner.snapshot_key=l.snapshot_key",
        ancestry: "owner.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "1",
    },
    OwnerSql {
        relation: "no_intro_header_field_diagnostics",
        path: OwnerPath::HeaderField,
        key_column: "snapshot_key",
        key_type: "text",
        order_column: Some("source_order"),
        joins: "LEFT JOIN no_intro_header_fields AS owner ON owner.snapshot_key=l.snapshot_key AND owner.source_order=l.source_order LEFT JOIN no_intro_export_headers AS header ON header.snapshot_key=owner.snapshot_key",
        ancestry: "owner.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "1",
    },
    OwnerSql {
        relation: "no_intro_game_diagnostics",
        path: OwnerPath::Game,
        key_column: "set_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_database_games AS owner ON owner.set_id=l.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=owner.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "sets.source_line",
        start_column: "sets.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "owner.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_archive_diagnostics",
        path: OwnerPath::GameChild,
        key_column: "archive_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_archive_descriptions AS owner ON owner.archive_id=l.archive_id LEFT JOIN no_intro_database_games AS game ON game.set_id=owner.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_dump_source_diagnostics",
        path: OwnerPath::GameChild,
        key_column: "dump_source_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_dump_sources AS owner ON owner.dump_source_id=l.dump_source_id LEFT JOIN no_intro_database_games AS game ON game.set_id=owner.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_dump_details_diagnostics",
        path: OwnerPath::SourceChild,
        key_column: "dump_source_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_dump_details AS owner ON owner.dump_source_id=l.dump_source_id LEFT JOIN no_intro_dump_sources AS source ON source.dump_source_id=owner.dump_source_id LEFT JOIN no_intro_database_games AS game ON game.set_id=source.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "source.dump_source_id IS NOT NULL AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_dump_serials_diagnostics",
        path: OwnerPath::SourceChild,
        key_column: "dump_source_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_dump_serials AS owner ON owner.dump_source_id=l.dump_source_id LEFT JOIN no_intro_dump_sources AS source ON source.dump_source_id=owner.dump_source_id LEFT JOIN no_intro_database_games AS game ON game.set_id=source.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "source.dump_source_id IS NOT NULL AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_dump_file_diagnostics",
        path: OwnerPath::SourceFile,
        key_column: "occurrence_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_dump_files AS owner ON owner.occurrence_id=l.occurrence_id LEFT JOIN no_intro_dump_sources AS source ON source.dump_source_id=owner.dump_source_id LEFT JOIN no_intro_database_games AS game ON game.set_id=source.set_id LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id AND sets.set_id=owner.set_id AND sets.set_id=occurrence.record_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "owner.claim_kind='no_intro_database_source_file' AND occurrence.claim_kind='no_intro_database_source_file' AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_release_diagnostics",
        path: OwnerPath::GameChild,
        key_column: "release_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_releases AS owner ON owner.release_id=l.release_id LEFT JOIN no_intro_database_games AS game ON game.set_id=owner.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_release_details_diagnostics",
        path: OwnerPath::ReleaseChild,
        key_column: "release_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_release_details AS owner ON owner.release_id=l.release_id LEFT JOIN no_intro_releases AS release ON release.release_id=owner.release_id LEFT JOIN no_intro_database_games AS game ON game.set_id=release.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "release.release_id IS NOT NULL AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_release_serials_diagnostics",
        path: OwnerPath::ReleaseChild,
        key_column: "release_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_release_serials AS owner ON owner.release_id=l.release_id LEFT JOIN no_intro_releases AS release ON release.release_id=owner.release_id LEFT JOIN no_intro_database_games AS game ON game.set_id=release.set_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "release.release_id IS NOT NULL AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
    OwnerSql {
        relation: "no_intro_release_file_diagnostics",
        path: OwnerPath::ReleaseFile,
        key_column: "occurrence_id",
        key_type: "integer",
        order_column: None,
        joins: "LEFT JOIN no_intro_release_files AS owner ON owner.occurrence_id=l.occurrence_id LEFT JOIN no_intro_releases AS release ON release.release_id=owner.release_id LEFT JOIN no_intro_database_games AS game ON game.set_id=release.set_id LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id LEFT JOIN catalog_sets AS sets ON sets.set_id=game.set_id AND sets.set_id=owner.set_id AND sets.set_id=occurrence.record_id LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id LEFT JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key LEFT JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key",
        ancestry: "groups.snapshot_key",
        start_line: "owner.source_line",
        start_column: "owner.source_column",
        end_line: "owner.source_end_line",
        end_column: "owner.source_end_column",
        owner_check: "owner.claim_kind='no_intro_database_release_file' AND occurrence.claim_kind='no_intro_database_release_file' AND game.source_element_kind='no_intro_database_game' AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root' AND interpretation.format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')",
    },
];

fn load_owners(
    connection: &mut SqliteConnection,
    run: &ImportRunKey,
    run_info: &DiagnosticRun,
    diagnostics: &[ImportDiagnostic],
) -> QueryResult<BTreeMap<String, Vec<NoIntroDiagnosticOwner>>> {
    let mut result = BTreeMap::new();
    let run_text = run.to_string();
    for batch in diagnostics.chunks(400) {
        let requested = batch
            .iter()
            .map(|diagnostic| diagnostic.key.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        for spec in OWNER_SQL {
            let rows = load_owner_relation(connection, batch, spec)?;
            for row in rows {
                if row.valid != 1 {
                    return Err(invalid("invalid diagnostic owner link".to_owned()));
                }
                let diagnostic_key = text(row.diagnostic_key.clone(), "owner diagnostic key")?;
                let linked_run_key = text(row.link_run_key.clone(), "owner run key")?;
                let linked_snapshot_key =
                    text(row.link_snapshot_key.clone(), "owner snapshot key")?;
                let ancestry_snapshot = row
                    .ancestry_snapshot
                    .as_ref()
                    .map(|value| text(value.clone(), "owner ancestry snapshot"))
                    .transpose()?;
                let expected_snapshot = run_info
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.key.as_str());
                if !requested.contains(&diagnostic_key)
                    || linked_run_key != run_text
                    || Some(linked_snapshot_key.as_str()) != expected_snapshot
                    || ancestry_snapshot.as_deref() != expected_snapshot
                {
                    return Err(invalid(diagnostic_key));
                }
                let owner = parse_owner(spec, row)?;
                let owner_relation = owner.link();
                if owner_relation.relation != spec.relation {
                    return Err(invalid(diagnostic_key));
                }
                result
                    .entry(diagnostic_key)
                    .or_insert_with(Vec::new)
                    .push(owner);
            }
        }
    }
    Ok(result)
}

fn load_owner_relation(
    connection: &mut SqliteConnection,
    diagnostics: &[ImportDiagnostic],
    spec: OwnerSql,
) -> QueryResult<Vec<OwnerRow>> {
    let sql = owner_relation_sql(spec, diagnostics.len());
    let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
    for diagnostic in diagnostics {
        let key = diagnostic.key.as_str();
        // SQLite distinguishes storage classes, so bind byte-identical key
        // bytes as BLOB as well to detect a mis-typed copy of the UUID.
        query = query
            .bind::<diesel::sql_types::Text, _>(key)
            .bind::<Binary, _>(key.as_bytes().to_vec());
    }
    Ok(query.load::<OwnerRow>(connection)?)
}

fn owner_relation_sql(spec: OwnerSql, key_count: usize) -> String {
    let owner_order = spec.order_column.map_or_else(
        || "NULL".to_owned(),
        |column| format!("CAST(l.{column} AS BLOB)"),
    );
    let valid_order = spec.order_column.map_or("1", |column| {
        if column == "source_order" {
            "typeof(l.source_order)='integer' AND l.source_order>=0"
        } else {
            "0"
        }
    });
    let placeholders = std::iter::repeat_n("?,?", key_count)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "SELECT CAST(l.diagnostic_key AS BLOB) AS diagnostic_key, \
                CAST(l.run_key AS BLOB) AS link_run_key, \
                CAST(l.snapshot_key AS BLOB) AS link_snapshot_key, \
                CAST(l.{key} AS BLOB) AS owner_key, \
                {order} AS owner_order, \
                CAST({ancestry} AS BLOB) AS ancestry_snapshot, \
                CAST({start_line} AS BLOB) AS start_line, \
                CAST({start_column} AS BLOB) AS start_column, \
                CAST({end_line} AS BLOB) AS end_line, \
                CAST({end_column} AS BLOB) AS end_column, \
                COALESCE(typeof(l.diagnostic_key)='text' AND typeof(l.run_key)='text' \
                AND typeof(l.snapshot_key)='text' AND typeof(l.{key})='{key_type}' \
                AND d.diagnostic_key IS NOT NULL \
                AND typeof(d.diagnostic_key)='text' AND d.run_key=l.run_key \
                AND {valid_order} AND typeof({ancestry})='text' \
                AND typeof({start_line})='integer' AND {start_line}>0 \
                AND typeof({start_column})='integer' AND {start_column}>0 \
                AND typeof({end_line})='integer' AND {end_line}>0 \
                AND typeof({end_column})='integer' AND {end_column}>0 \
                AND ({owner_check}) AND ({integrity}), 0) AS valid \
         FROM {relation} AS l \
         LEFT JOIN import_diagnostics AS d \
           ON d.diagnostic_key=l.diagnostic_key AND d.run_key=l.run_key \
         {joins} \
         LEFT JOIN no_intro_exports AS export ON export.snapshot_key=l.snapshot_key \
         WHERE l.diagnostic_key IN ({placeholders}) \
         ORDER BY d.diagnostic_order, CAST(l.{key} AS BLOB)",
        key = spec.key_column,
        order = owner_order,
        key_type = spec.key_type,
        valid_order = valid_order,
        ancestry = spec.ancestry,
        start_line = spec.start_line,
        start_column = spec.start_column,
        end_line = spec.end_line,
        end_column = spec.end_column,
        owner_check = spec.owner_check,
        integrity = owner_integrity_sql(spec),
        relation = spec.relation,
        joins = spec.joins,
    )
}

fn parse_owner(spec: OwnerSql, row: OwnerRow) -> QueryResult<NoIntroDiagnosticOwner> {
    let key_text = text(row.owner_key, "native owner key")?;
    let extent = XmlSourceExtent::new(
        RecordLocation {
            line: integer(row.start_line, "owner start line")?,
            column: integer(row.start_column, "owner start column")?,
        },
        RecordLocation {
            line: integer(row.end_line, "owner end line")?,
            column: integer(row.end_column, "owner end column")?,
        },
    )
    .ok_or_else(|| invalid(key_text.clone()))?;
    let owner = match spec.relation {
        "no_intro_export_diagnostics" => NoIntroDiagnosticOwner::ExportDocument {
            snapshot: SnapshotKey::from_persisted(key_text),
            extent,
        },
        "no_intro_export_header_diagnostics" => NoIntroDiagnosticOwner::ExportHeader {
            snapshot: SnapshotKey::from_persisted(key_text),
            extent,
        },
        "no_intro_header_field_diagnostics" => {
            let snapshot = SnapshotKey::from_persisted(key_text);
            let source_order = row
                .owner_order
                .map(|value| text(value, "header field source order"))
                .transpose()?
                .and_then(|value| value.parse::<i64>().ok())
                .filter(|order| *order >= 0)
                .ok_or_else(|| invalid("header field source order".into()))?;
            NoIntroDiagnosticOwner::HeaderField {
                snapshot,
                source_order,
                extent,
            }
        }
        "no_intro_game_diagnostics" => NoIntroDiagnosticOwner::Game {
            id: CatalogSetId::try_from(integer_text(&key_text, "game ID")?)
                .map_err(|_| invalid("game ID".into()))?,
            extent,
        },
        "no_intro_archive_diagnostics" => NoIntroDiagnosticOwner::ArchiveDescription {
            id: NoIntroArchiveId::try_from(integer_text(&key_text, "archive ID")?)
                .map_err(|_| invalid("archive ID".into()))?,
            extent,
        },
        "no_intro_dump_source_diagnostics" => NoIntroDiagnosticOwner::DumpSource {
            id: NoIntroDumpSourceId::try_from(integer_text(&key_text, "dump source ID")?)
                .map_err(|_| invalid("dump source ID".into()))?,
            extent,
        },
        "no_intro_dump_details_diagnostics" => NoIntroDiagnosticOwner::DumpDetails {
            id: NoIntroDumpSourceId::try_from(integer_text(&key_text, "dump details ID")?)
                .map_err(|_| invalid("dump details ID".into()))?,
            extent,
        },
        "no_intro_dump_serials_diagnostics" => NoIntroDiagnosticOwner::DumpSerials {
            id: NoIntroDumpSourceId::try_from(integer_text(&key_text, "dump serials ID")?)
                .map_err(|_| invalid("dump serials ID".into()))?,
            extent,
        },
        "no_intro_dump_file_diagnostics" => NoIntroDiagnosticOwner::DumpFile {
            id: crate::domain::OccurrenceId::try_from(integer_text(&key_text, "dump file ID")?)
                .map_err(|_| invalid("dump file ID".into()))?,
            extent,
        },
        "no_intro_release_diagnostics" => NoIntroDiagnosticOwner::Release {
            id: NoIntroReleaseId::try_from(integer_text(&key_text, "release ID")?)
                .map_err(|_| invalid("release ID".into()))?,
            extent,
        },
        "no_intro_release_details_diagnostics" => NoIntroDiagnosticOwner::ReleaseDetails {
            id: NoIntroReleaseId::try_from(integer_text(&key_text, "release details ID")?)
                .map_err(|_| invalid("release details ID".into()))?,
            extent,
        },
        "no_intro_release_serials_diagnostics" => NoIntroDiagnosticOwner::ReleaseSerials {
            id: NoIntroReleaseId::try_from(integer_text(&key_text, "release serials ID")?)
                .map_err(|_| invalid("release serials ID".into()))?,
            extent,
        },
        "no_intro_release_file_diagnostics" => NoIntroDiagnosticOwner::ReleaseFile {
            id: crate::domain::OccurrenceId::try_from(integer_text(&key_text, "release file ID")?)
                .map_err(|_| invalid("release file ID".into()))?,
            extent,
        },
        _ => return Err(invalid(spec.relation.to_owned())),
    };
    let owner_relation = owner.link();
    if owner_relation.relation != spec.relation {
        return Err(invalid(spec.relation.to_owned()));
    }
    Ok(owner)
}

fn integer(value: Option<Vec<u8>>, field: &'static str) -> QueryResult<i64> {
    value
        .map(|value| text(value, field))
        .transpose()?
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or_else(|| invalid(field.to_owned()))
}

fn integer_text(value: &str, field: &'static str) -> QueryResult<i64> {
    value.parse::<i64>().map_err(|_| invalid(field.to_owned()))
}

fn validate_owners(run: &DiagnosticRun, diagnostic: &ImportDiagnostic) -> QueryResult<()> {
    if run.status == ImportRunStatus::Failed && !diagnostic.owners.is_empty() {
        return Err(invalid(diagnostic.key.as_str().to_owned()));
    }
    if diagnostic.code == DiagnosticCode::ParseFailed
        && (run.status != ImportRunStatus::Failed
            || diagnostic.severity != DiagnosticSeverity::Error
            || !diagnostic.owners.is_empty())
    {
        return Err(invalid(diagnostic.key.as_str().to_owned()));
    }
    if diagnostic.code == DiagnosticCode::XmlNulRecovered
        && (run.status != ImportRunStatus::Succeeded
            || diagnostic.severity != DiagnosticSeverity::Warning
            || run.interpretation.format != "no-intro-database-xml-nul-compatible"
            || diagnostic.owners.is_empty())
    {
        return Err(invalid(diagnostic.key.as_str().to_owned()));
    }
    if !diagnostic.owners.is_empty() {
        if run.interpretation.format != "no-intro-database-xml-nul-compatible"
            && run.interpretation.format != "no-intro-database-xml-compatible"
        {
            return Err(invalid(diagnostic.key.as_str().to_owned()));
        }
        for owner in &diagnostic.owners {
            let owner_extent = owner.link().extent;
            let (start, end) = (owner_extent.start(), owner_extent.end());
            if diagnostic.coordinates != Some(CoordinateConvention::XmlUnicodeScalars)
                || !matches!((diagnostic.source_line, diagnostic.source_column), (Some(line), Some(column)) if owner_extent.contains(RecordLocation { line, column }))
                || start.line <= 0
                || end.line <= 0
            {
                return Err(invalid(diagnostic.key.as_str().to_owned()));
            }
        }
    }
    Ok(())
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

const fn invalid(value: String) -> DiagnosticQueryError {
    DiagnosticQueryError::InvalidMetadata(value)
}

fn text(bytes: Vec<u8>, field: &'static str) -> QueryResult<String> {
    String::from_utf8(bytes).map_err(|_| invalid(field.to_owned()))
}

fn digest<const N: usize>(bytes: Vec<u8>, owner: &str) -> QueryResult<[u8; N]> {
    bytes.try_into().map_err(|_| invalid(owner.to_owned()))
}

#[cfg(test)]
mod tests {
    use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::Text};

    use super::*;

    mod warning_fixture {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/import_warning_fixture.rs"
        ));
    }

    #[derive(Debug, QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    fn page_plan(
        connection: &mut SqliteConnection,
        run: &str,
        cursor: Option<i64>,
    ) -> diesel::QueryResult<Vec<PlanRow>> {
        let mut query = sql_query(format!("EXPLAIN QUERY PLAN {}", page_sql(cursor.is_some())))
            .into_boxed::<diesel::sqlite::Sqlite>()
            .bind::<Text, _>(run);
        if let Some(order) = cursor {
            query = query.bind::<BigInt, _>(order);
        }
        query.bind::<BigInt, _>(51_i64).load(connection)
    }

    #[test]
    fn owner_queries_seek_each_native_link_by_selected_diagnostic_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let database = Database::in_memory()?;
        let mut connection = database.pool().get()?;
        let key = DiagnosticKey::new();

        for spec in OWNER_SQL {
            let sql = format!("EXPLAIN QUERY PLAN {}", owner_relation_sql(spec, 1));
            let plan = sql_query(sql)
                .bind::<Text, _>(key.as_str())
                .bind::<Binary, _>(key.as_str().as_bytes().to_vec())
                .load::<PlanRow>(&mut connection)?;
            assert!(
                plan.iter().any(|row| {
                    row.detail.contains("SEARCH l") && row.detail.contains("PRIMARY KEY")
                }),
                "{} did not seek its diagnostic-key primary key: {plan:?}",
                spec.relation,
            );
            assert!(
                plan.iter().all(|row| { !row.detail.contains("SCAN l") }),
                "{} scans the owner relation: {plan:?}",
                spec.relation,
            );
        }
        Ok(())
    }

    #[test]
    fn populated_owner_queries_seek_links_and_actual_native_ancestors()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::{
            app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
            domain::{CatalogKey, CatalogScope, PublishingSourceKey},
            no_intro_db_xml::NoIntroDatabaseMode,
        };
        let directory = tempfile::tempdir()?;
        let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
        let database = Database::open(&path)?;
        let request = CatalogImportRequest {
            document_path: document.clone(),
            format: CatalogDocumentFormat::NoIntroDatabase(
                NoIntroDatabaseMode::NullRecoveryCompatible,
            ),
            source_key: PublishingSourceKey::new("owner-plan"),
            source_display_name: "Owner plan".into(),
            catalog_key: CatalogKey::new("owner-plan"),
            catalog_display_name: "Owner plan".into(),
            scope: CatalogScope::Complete,
        };
        let xml = warning_fixture::native_owner_nul_document();
        let mut first_run = None;
        for index in 0..16 {
            std::fs::write(
                &document,
                format!("{xml}<!-- unrelated snapshot {index} -->"),
            )?;
            let report = app::import_catalog(&database, &request)?;
            assert_eq!(report.status, CatalogImportStatus::Succeeded);
            assert_eq!(report.diagnostic_count, 14);
            first_run.get_or_insert(report.run_key);
        }
        let run = first_run.ok_or("missing imported run")?;
        let page = for_run(&database, &run, None, DiagnosticPageLimit::new(20)?)?;
        let mut connection = database.pool().get()?;
        // Avoid a one-row interpretation table: scanning that singleton is an
        // optimal plan, but does not test lookup among unrelated reading rules.
        for index in 0..256 {
            sql_query(
                "INSERT INTO parser_interpretations(interpretation_key,format) VALUES(?,'logiqx')",
            )
            .bind::<Text, _>(format!("unrelated-owner-plan-rule-{index}"))
            .execute(&mut connection)?;
        }
        sql_query("ANALYZE").execute(&mut connection)?;
        let blob_run_plan = sql_query(format!("EXPLAIN QUERY PLAN {BLOB_RUN_KEY_SQL}"))
            .bind::<Binary, _>(run.to_string().into_bytes())
            .load::<PlanRow>(&mut connection)?;
        assert!(
            blob_run_plan.iter().any(|row| {
                row.detail.starts_with("SEARCH import_diagnostics ")
                    && row.detail.contains("(run_key=?)")
            }),
            "the corruption probe must seek one run key: {blob_run_plan:?}"
        );
        assert!(
            !blob_run_plan
                .iter()
                .any(|row| row.detail.starts_with("SCAN import_diagnostics")),
            "the corruption probe must not scan unrelated runs: {blob_run_plan:?}"
        );
        for spec in OWNER_SQL {
            let diagnostic = page
                .diagnostics
                .iter()
                .find(|diagnostic| {
                    diagnostic
                        .owners
                        .iter()
                        .any(|owner| owner.link().relation == spec.relation)
                })
                .ok_or("missing actual diagnostic owner")?;
            let plan = sql_query(format!(
                "EXPLAIN QUERY PLAN {}",
                owner_relation_sql(spec, 1)
            ))
            .bind::<Text, _>(diagnostic.key.as_str())
            .bind::<Binary, _>(diagnostic.key.as_str().as_bytes().to_vec())
            .load::<PlanRow>(&mut connection)?;
            assert_owner_plan_seeks(spec, &plan);
        }
        Ok(())
    }

    fn assert_owner_plan_seeks(spec: OwnerSql, plan: &[PlanRow]) {
        for alias in ["l", "owner", "export"] {
            assert!(
                plan.iter()
                    .any(|row| row.detail.starts_with(&format!("SEARCH {alias} "))),
                "{} must seek its actual {alias} key: {plan:?}",
                spec.relation
            );
            assert!(
                !plan
                    .iter()
                    .any(|row| row.detail.starts_with(&format!("SCAN {alias}"))),
                "{} must not scan unrelated {alias} rows: {plan:?}",
                spec.relation
            );
        }
        for alias in [
            "source",
            "release",
            "game",
            "sets",
            "groups",
            "snapshot",
            "interpretation",
            "occurrence",
            "header",
        ] {
            let relevant = plan
                .iter()
                .filter(|row| row.detail.contains(&format!(" {alias} ")))
                .collect::<Vec<_>>();
            assert!(
                relevant
                    .iter()
                    .all(|row| row.detail.starts_with(&format!("SEARCH {alias} "))),
                "{} must seek every actual ancestor {alias}: {plan:?}",
                spec.relation
            );
        }
    }

    #[test]
    fn continuation_page_plan_seeks_the_diagnostic_order_range()
    -> Result<(), Box<dyn std::error::Error>> {
        use diesel::connection::SimpleConnection;

        let database = Database::in_memory()?;
        let mut connection = database.pool().get()?;
        connection.batch_execute("PRAGMA foreign_keys=OFF")?;
        let run_text = uuid::Uuid::new_v4().to_string();
        let other_run = uuid::Uuid::new_v4().to_string();
        let document_key = uuid::Uuid::new_v4().to_string();
        for (run, count) in [(&run_text, 50), (&other_run, 50)] {
            for order in 0..count {
                sql_query(
                    "INSERT INTO import_diagnostics \
                     (diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message) \
                     VALUES(?,?,?,?,'warning','test','message')",
                )
                .bind::<Text, _>(DiagnosticKey::new().as_str().to_owned())
                .bind::<Text, _>(run.as_str())
                .bind::<BigInt, _>(i64::from(order))
                .bind::<Text, _>(&document_key)
                .execute(&mut connection)?;
            }
        }
        sql_query("ANALYZE").execute(&mut connection)?;

        let first_page = page_plan(&mut connection, &run_text, None)?;
        assert!(
            first_page.iter().any(|row| row.detail.contains("SEARCH d")),
            "first page should use the run/order index: {first_page:?}"
        );

        let continuation = page_plan(&mut connection, &run_text, Some(16))?;
        assert!(
            continuation
                .iter()
                .any(|row| row.detail.contains("diagnostic_order>?")),
            "continuation should seek past the anchor in the run/order index: {continuation:?}"
        );
        Ok(())
    }
}
