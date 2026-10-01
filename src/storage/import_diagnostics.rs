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
    let problem = StoredRange::new(excerpt.and_then(SourceExcerpt::problem))?;
    let source_problem = StoredRange::new(excerpt.and_then(SourceExcerpt::source_problem))?;
    let original_problem = StoredRange::new(excerpt.and_then(SourceExcerpt::original_problem))?;
    let excerpt_start = excerpt
        .map(|excerpt| stored_offset(excerpt.start_byte()))
        .transpose()?;
    sql_query(
        "INSERT INTO import_diagnostics \
         (diagnostic_key, run_key, document_key, code, message, record_kind, record_name, source_line, source_column, \
          source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, problem_end_byte, \
          source_problem_start_byte, source_problem_end_byte, original_problem_start_byte, original_problem_end_byte, \
          coordinate_view, column_convention) \
         VALUES (?, ?, ?, 'parse_failed', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
    .bind::<Text, _>(run.to_string())
    .bind::<Text, _>(document.to_string())
    .bind::<Text, _>(error.to_string())
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
    Ok(())
}
