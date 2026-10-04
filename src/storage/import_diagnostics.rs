//! Persist parser evidence independently of rolled-back catalog facts.

mod model;
mod reader;
pub use model::{
    DiagnosticCatalog, DiagnosticCode, DiagnosticCursor, DiagnosticDocument,
    DiagnosticInterpretation, DiagnosticKey, DiagnosticOrder, DiagnosticPage, DiagnosticPageLimit,
    DiagnosticQueryError, DiagnosticRun, DiagnosticSeverity, DiagnosticSnapshot, DiagnosticSource,
    DocumentRetention, ImportDiagnostic, ImportRunStatus, NoIntroDiagnosticOwner,
};
pub use reader::for_run;

use crate::{
    diagnostics::{ByteRange, SourceExcerpt},
    domain::{DocumentKey, ImportRunKey},
};
use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

#[derive(Default)]
struct StoredRange {
    start: Option<i64>,
    end: Option<i64>,
}

fn stored_offset(offset: usize) -> crate::Result<i64> {
    i64::try_from(offset).map_err(|_| {
        crate::Error::DatabaseSchema("diagnostic byte offset exceeds SQLite range".into())
    })
}

impl StoredRange {
    fn new(range: Option<ByteRange>) -> crate::Result<Self> {
        range.map_or_else(
            || Ok(Self::default()),
            |range| {
                Ok(Self {
                    start: Some(stored_offset(range.start())?),
                    end: Some(stored_offset(range.end())?),
                })
            },
        )
    }
}

pub(super) fn insert_parse_error(
    conn: &mut SqliteConnection,
    run: &ImportRunKey,
    document: &DocumentKey,
    error: &crate::Error,
) -> crate::Result<()> {
    let (kind, name, line, column, excerpt, coordinates) = match error {
        crate::Error::CatalogParse {
            record_kind,
            record_name,
            line,
            column,
            excerpt,
            coordinates,
            ..
        } => (
            record_kind.as_deref(),
            record_name.as_deref(),
            *line,
            *column,
            excerpt.as_deref(),
            *coordinates,
        ),
        _ => (None, None, None, None, None, None),
    };
    insert(
        conn,
        run,
        document,
        Diagnostic {
            order: DiagnosticOrder::FIRST,
            severity: DiagnosticSeverity::Error,
            code: DiagnosticCode::ParseFailed,
            message: &error.to_string(),
            kind,
            name,
            line,
            column,
            excerpt,
            coordinates,
        },
    )
    .map(|_| ())
}

#[derive(Clone, Copy)]
struct Diagnostic<'a> {
    order: DiagnosticOrder,
    severity: DiagnosticSeverity,
    code: DiagnosticCode,
    message: &'a str,
    kind: Option<&'a str>,
    name: Option<&'a str>,
    line: Option<i64>,
    column: Option<i64>,
    excerpt: Option<&'a SourceExcerpt>,
    coordinates: Option<crate::diagnostics::CoordinateConvention>,
}

pub(super) fn insert_recovery_warning(
    conn: &mut SqliteConnection,
    run: &ImportRunKey,
    document: &DocumentKey,
    order: DiagnosticOrder,
    warning: &crate::no_intro_db_xml::RecoveryWarning,
) -> crate::Result<DiagnosticKey> {
    insert(
        conn,
        run,
        document,
        Diagnostic {
            order,
            severity: DiagnosticSeverity::Warning,
            code: DiagnosticCode::XmlNulRecovered,
            message: "Replaced XML-forbidden U+0000 with U+FFFD in the parse view; original bytes unchanged",
            kind: Some("document"),
            name: None,
            line: Some(warning.location.line),
            column: Some(warning.location.column),
            excerpt: warning.excerpt.as_ref(),
            coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
        },
    )
}

pub(super) fn link_no_intro_owner(
    conn: &mut SqliteConnection,
    key: &DiagnosticKey,
    run: &ImportRunKey,
    snapshot: &crate::domain::SnapshotKey,
    owner: &NoIntroDiagnosticOwner,
) -> crate::Result<()> {
    use model::NativeDiagnosticOwnerKey as Key;
    let link = owner.link();
    let table = link.relation;
    let extra = match link.key {
        Key::Snapshot(ref actual)
        | Key::HeaderField {
            snapshot: ref actual,
            ..
        } if actual != snapshot => {
            return Err(crate::Error::DatabaseSchema(
                "diagnostic owner belongs to another snapshot".into(),
            ));
        }
        Key::Snapshot(_) => None,
        Key::HeaderField { source_order, .. } => Some(("source_order", source_order)),
        Key::Game(id) => Some(("set_id", id.as_i64())),
        Key::ArchiveDescription(id) => Some(("archive_id", id.as_i64())),
        Key::DumpSource(id) => Some(("dump_source_id", id.as_i64())),
        Key::DumpFile(id) | Key::ReleaseFile(id) => Some(("occurrence_id", id.database_value())),
        Key::Release(id) => Some(("release_id", id.as_i64())),
    };
    let sql = match extra {
        Some((column, _)) => format!(
            "INSERT INTO {table}(diagnostic_key,run_key,snapshot_key,{column}) VALUES(?,?,?,?)"
        ),
        None => format!("INSERT INTO {table}(diagnostic_key,run_key,snapshot_key) VALUES(?,?,?)"),
    };
    let mut query = sql_query(sql)
        .into_boxed::<diesel::sqlite::Sqlite>()
        .bind::<Text, _>(key.as_str())
        .bind::<Text, _>(run.to_string())
        .bind::<Text, _>(snapshot.as_str());
    if let Some((_, value)) = extra {
        query = query.bind::<BigInt, _>(value);
    }
    query.execute(conn)?;
    Ok(())
}

fn insert(
    conn: &mut SqliteConnection,
    run: &ImportRunKey,
    document: &DocumentKey,
    diagnostic: Diagnostic<'_>,
) -> crate::Result<DiagnosticKey> {
    let Diagnostic {
        order,
        severity,
        code,
        message,
        kind,
        name,
        line,
        column,
        excerpt,
        coordinates,
    } = diagnostic;
    let problem = StoredRange::new(excerpt.and_then(SourceExcerpt::problem))?;
    let source_problem = StoredRange::new(excerpt.and_then(SourceExcerpt::source_problem))?;
    let original_problem = StoredRange::new(excerpt.and_then(SourceExcerpt::original_problem))?;
    let excerpt_start = excerpt
        .and_then(SourceExcerpt::start_byte)
        .map(stored_offset)
        .transpose()?;
    let key = DiagnosticKey::new();
    sql_query(
        "INSERT INTO import_diagnostics \
         (diagnostic_key, run_key, diagnostic_order, document_key, severity, code, message, record_kind, record_name, source_line, source_column, \
          source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, problem_end_byte, \
          source_problem_start_byte, source_problem_end_byte, original_problem_start_byte, original_problem_end_byte, \
          coordinate_view, column_convention) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(run.to_string())
    .bind::<BigInt, _>(order.get())
    .bind::<Text, _>(document.to_string())
    .bind::<Text, _>(match severity { DiagnosticSeverity::Error => "error", DiagnosticSeverity::Warning => "warning" })
    .bind::<Text, _>(match code { DiagnosticCode::ParseFailed => "parse_failed", DiagnosticCode::XmlNulRecovered => "xml_nul_recovered" })
    .bind::<Text, _>(message)
    .bind::<Nullable<Text>, _>(kind)
    .bind::<Nullable<Text>, _>(name)
    .bind::<Nullable<BigInt>, _>(line)
    .bind::<Nullable<BigInt>, _>(column)
    .bind::<Nullable<Binary>, _>(excerpt.map(SourceExcerpt::bytes))
    .bind::<Nullable<Text>, _>(excerpt.map(|excerpt| excerpt.view().as_str()))
    .bind::<Nullable<BigInt>, _>(excerpt_start)
    .bind::<Nullable<BigInt>, _>(problem.start)
    .bind::<Nullable<BigInt>, _>(problem.end)
    .bind::<Nullable<BigInt>, _>(source_problem.start)
    .bind::<Nullable<BigInt>, _>(source_problem.end)
    .bind::<Nullable<BigInt>, _>(original_problem.start)
    .bind::<Nullable<BigInt>, _>(original_problem.end)
    .bind::<Nullable<Text>, _>(coordinates.map(crate::diagnostics::CoordinateConvention::view))
    .bind::<Nullable<Text>, _>(coordinates.map(crate::diagnostics::CoordinateConvention::columns))
    .execute(conn)?;
    Ok(key)
}
