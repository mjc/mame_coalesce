//! Persist parser evidence independently of rolled-back catalog facts.

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
            severity: Severity::Error,
            code: "parse_failed",
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

enum Severity {
    Error,
    Warning,
}

struct Diagnostic<'a> {
    severity: Severity,
    code: &'a str,
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
    warning: &crate::no_intro_db_xml::RecoveryWarning,
) -> crate::Result<DiagnosticKey> {
    insert(
        conn,
        run,
        document,
        Diagnostic {
            severity: Severity::Warning,
            code: "xml_nul_recovered",
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

pub(super) struct DiagnosticKey(String);

pub(super) fn link_no_intro_details(
    conn: &mut SqliteConnection,
    key: &DiagnosticKey,
) -> crate::Result<()> {
    for sql in [
        "INSERT INTO no_intro_dump_details_diagnostics(diagnostic_key,run_key,dump_source_id) SELECT d.diagnostic_key,d.run_key,p.dump_source_id FROM import_diagnostics d JOIN import_runs r USING(run_key) JOIN catalog_set_groups g ON g.snapshot_key=r.snapshot_key JOIN catalog_sets s USING(set_group_id) JOIN no_intro_dump_sources o USING(set_id) JOIN no_intro_dump_details p USING(dump_source_id) WHERE d.diagnostic_key=? AND (d.source_line>p.source_line OR (d.source_line=p.source_line AND d.source_column>=p.source_column)) AND (d.source_line<p.opening_end_line OR (d.source_line=p.opening_end_line AND d.source_column<p.opening_end_column))",
        "INSERT INTO no_intro_release_details_diagnostics(diagnostic_key,run_key,release_id) SELECT d.diagnostic_key,d.run_key,p.release_id FROM import_diagnostics d JOIN import_runs r USING(run_key) JOIN catalog_set_groups g ON g.snapshot_key=r.snapshot_key JOIN catalog_sets s USING(set_group_id) JOIN no_intro_releases o USING(set_id) JOIN no_intro_release_details p USING(release_id) WHERE d.diagnostic_key=? AND (d.source_line>p.source_line OR (d.source_line=p.source_line AND d.source_column>=p.source_column)) AND (d.source_line<p.opening_end_line OR (d.source_line=p.opening_end_line AND d.source_column<p.opening_end_column))",
    ] {
        sql_query(sql).bind::<Text, _>(&key.0).execute(conn)?;
    }
    Ok(())
}

fn insert(
    conn: &mut SqliteConnection,
    run: &ImportRunKey,
    document: &DocumentKey,
    diagnostic: Diagnostic<'_>,
) -> crate::Result<DiagnosticKey> {
    let Diagnostic {
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
        .map(|excerpt| stored_offset(excerpt.start_byte()))
        .transpose()?;
    let key = DiagnosticKey(uuid::Uuid::new_v4().to_string());
    sql_query(
        "INSERT INTO import_diagnostics \
         (diagnostic_key, run_key, document_key, severity, code, message, record_kind, record_name, source_line, source_column, \
          source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, problem_end_byte, \
          source_problem_start_byte, source_problem_end_byte, original_problem_start_byte, original_problem_end_byte, \
          coordinate_view, column_convention) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(&key.0)
    .bind::<Text, _>(run.to_string())
    .bind::<Text, _>(document.to_string())
    .bind::<Text, _>(match severity { Severity::Error => "error", Severity::Warning => "warning" })
    .bind::<Text, _>(code)
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
