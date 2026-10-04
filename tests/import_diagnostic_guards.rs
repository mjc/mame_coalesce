use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    database::Database,
    diagnostics::{ByteRange, ExcerptView, SourceExcerpt},
};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Fixture {
    _directory: tempfile::TempDir,
    _database: Database,
    connection: SqliteConnection,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "temporary database path was not UTF-8")?;
        let database = Database::open(&database_path)?;
        let mut connection = SqliteConnection::establish(database_path.as_str())?;
        connection.batch_execute("PRAGMA foreign_keys = ON")?;
        connection.batch_execute(
            "INSERT INTO publishing_sources(source_key, display_name)
                 VALUES ('source', 'Source');
             INSERT INTO catalogs(catalog_key, source_key, display_name)
                 VALUES ('catalog', 'source', 'Catalog');
             INSERT INTO documents(document_key) VALUES ('document-a'), ('document-b');
             INSERT INTO parser_interpretations(interpretation_key, format)
                 VALUES ('parser', 'logiqx');
             INSERT INTO import_runs(
                 run_key, catalog_key, document_key, interpretation_key,
                 status
             ) VALUES
                 ('run-a', 'catalog', 'document-a', 'parser', 'failed'),
                 ('run-b', 'catalog', 'document-b', 'parser', 'failed');",
        )?;
        Ok(Self {
            _directory: directory,
            _database: database,
            connection,
        })
    }

    fn insert_diagnostic(
        &mut self,
        key: &str,
        run_key: &str,
        document_key: &str,
        extra_columns: &str,
        extra_values: &str,
    ) -> diesel::QueryResult<()> {
        let statement = format!(
            "INSERT INTO import_diagnostics(
                 diagnostic_key, run_key, diagnostic_order, document_key, code, message{extra_columns}
             ) VALUES (
                 '{key}', '{run_key}', (SELECT COUNT(*) FROM import_diagnostics WHERE run_key='{run_key}'), '{document_key}', 'test', 'test'{extra_values}
             )"
        );
        self.connection.batch_execute(&statement)
    }
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct ClippedSpan {
    #[diesel(sql_type = BigInt, column_name = excerpt_start_byte)]
    excerpt_start: i64,
    #[diesel(sql_type = Nullable<BigInt>, column_name = problem_start_byte)]
    problem_start: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>, column_name = problem_end_byte)]
    problem_end: Option<i64>,
    #[diesel(sql_type = BigInt, column_name = source_problem_start_byte)]
    source_start: i64,
    #[diesel(sql_type = BigInt, column_name = source_problem_end_byte)]
    source_end: i64,
    #[diesel(sql_type = Nullable<BigInt>, column_name = original_problem_start_byte)]
    original_start: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>, column_name = original_problem_end_byte)]
    original_end: Option<i64>,
}

const INVALID_DIAGNOSTICS: &[(&str, &str, &str)] = &[
    (
        "half_problem_pair",
        ", source_excerpt, excerpt_view, problem_start_byte, problem_end_byte",
        ", x'6162', 'retained_original_bytes', 0, NULL",
    ),
    (
        "half_source_pair",
        ", source_excerpt, excerpt_view, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 0, NULL",
    ),
    (
        "half_original_pair",
        ", source_excerpt, excerpt_view, original_problem_start_byte, original_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 0, NULL",
    ),
    (
        "fractional_excerpt_offset",
        ", source_excerpt, excerpt_view, excerpt_start_byte",
        ", x'61', 'retained_original_bytes', 1.5",
    ),
    (
        "text_excerpt_offset",
        ", source_excerpt, excerpt_view, excerpt_start_byte",
        ", x'61', 'retained_original_bytes', 'not-an-integer'",
    ),
    (
        "fractional_problem_bound",
        ", source_excerpt, excerpt_view, problem_start_byte, problem_end_byte",
        ", x'6162', 'retained_original_bytes', 0.5, 1",
    ),
    (
        "text_problem_bound",
        ", source_excerpt, excerpt_view, problem_start_byte, problem_end_byte",
        ", x'6162', 'retained_original_bytes', 'not-an-integer', 1",
    ),
    (
        "text_source_bound",
        ", source_excerpt, excerpt_view, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 'not-an-integer', 2",
    ),
    (
        "fractional_source_bound",
        ", source_excerpt, excerpt_view, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 0.5, 2",
    ),
    (
        "negative_excerpt_offset",
        ", source_excerpt, excerpt_view, excerpt_start_byte",
        ", x'61', 'retained_original_bytes', -1",
    ),
    (
        "problem_after_excerpt",
        ", source_excerpt, problem_start_byte, problem_end_byte",
        ", x'6162', 0, 3",
    ),
    (
        "problem_bounds_without_excerpt",
        ", problem_start_byte, problem_end_byte",
        ", 0, 0",
    ),
    (
        "source_bounds_without_excerpt",
        ", source_problem_start_byte, source_problem_end_byte",
        ", 0, 0",
    ),
    (
        "original_bounds_without_excerpt",
        ", original_problem_start_byte, original_problem_end_byte",
        ", 0, 0",
    ),
    (
        "negative_source_bound",
        ", source_excerpt, excerpt_view, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', -1, 2",
    ),
    (
        "reversed_source_bounds",
        ", source_excerpt, excerpt_view, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 2, 1",
    ),
    (
        "reversed_original_bounds",
        ", source_excerpt, excerpt_view, original_problem_start_byte, original_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 2, 1",
    ),
    (
        "invalid_excerpt_view",
        ", source_excerpt, excerpt_view",
        ", x'61', 'decoded_xml'",
    ),
    (
        "text_excerpt",
        ", source_excerpt, excerpt_view",
        ", 'not a blob', 'retained_original_bytes'",
    ),
    ("invalid_severity", ", severity", ", 'fatal'"),
    (
        "original_map_for_transport_view",
        ", source_excerpt, excerpt_view, original_problem_start_byte, original_problem_end_byte",
        ", x'61', 'transport_decoded_xml_bytes', 0, 1",
    ),
    (
        "original_span_disagrees_without_source_span",
        ", source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, problem_end_byte, original_problem_start_byte, original_problem_end_byte",
        ", x'61', 'retained_original_bytes', 0, 0, 1, 1, 2",
    ),
    (
        "source_span_disagrees_with_excerpt_bounds",
        ", source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, \
             problem_end_byte, source_problem_start_byte, source_problem_end_byte",
        ", x'6162', 'retained_original_bytes', 10, 0, 1, 12, 13",
    ),
    (
        "coordinate_metadata_half_pair",
        ", source_line, source_column, coordinate_view",
        ", 1, 1, 'transport_decoded_xml_text'",
    ),
    (
        "invalid_coordinate_view",
        ", source_line, source_column, coordinate_view, column_convention",
        ", 1, 1, 'original_bytes', 'unicode_scalar_1based'",
    ),
    (
        "invalid_column_convention",
        ", source_line, source_column, coordinate_view, column_convention",
        ", 1, 1, 'transport_decoded_xml_text', 'unicode_scalar_0based'",
    ),
];

#[test]
fn diagnostics_reject_invalid_ranges_types_and_enum_values() -> TestResult {
    let mut fixture = Fixture::new()?;
    for &(key, columns, values) in INVALID_DIAGNOSTICS {
        assert!(
            fixture
                .insert_diagnostic(key, "run-a", "document-a", columns, values)
                .is_err(),
            "SQLite accepted invalid diagnostic {key}"
        );
    }
    Ok(())
}

#[test]
fn diagnostic_document_must_match_its_failed_run() -> TestResult {
    let mut fixture = Fixture::new()?;
    assert!(
        fixture
            .insert_diagnostic("wrong-document", "run-a", "document-b", "", "")
            .is_err()
    );
    fixture.insert_diagnostic("matching-document", "run-a", "document-a", "", "")?;
    Ok(())
}

#[test]
fn empty_eof_excerpt_and_retained_original_mapping_are_valid() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.insert_diagnostic(
        "empty-eof",
        "run-a",
        "document-a",
        ", source_excerpt, excerpt_view, excerpt_start_byte, problem_start_byte, \
         problem_end_byte, source_problem_start_byte, source_problem_end_byte, \
         original_problem_start_byte, original_problem_end_byte, coordinate_view, \
         column_convention, source_line, source_column",
        ", x'', 'retained_original_bytes', 120, 0, 0, 120, 120, 120, 120, \
         'transport_decoded_xml_text', 'unicode_scalar_1based', 1, 1",
    )?;
    fixture.insert_diagnostic(
        "warning-severity",
        "run-a",
        "document-a",
        ", severity",
        ", 'warning'",
    )?;
    let severity = sql_query(
        "SELECT severity AS value FROM import_diagnostics WHERE diagnostic_key = 'empty-eof'",
    )
    .get_result::<TextValue>(&mut fixture.connection)?;
    assert_eq!(severity.value, "error");
    Ok(())
}

#[test]
fn clipped_excerpt_keeps_full_source_span_without_original_mapping() -> TestResult {
    let mut fixture = Fixture::new()?;
    let source = b"0123456789abcdefghijklmnopqrstuv";
    let context = ByteRange::new(4, 28).ok_or("invalid context range")?;
    let problem = ByteRange::new(8, 20).ok_or("invalid problem range")?;
    let window = ByteRange::new(8, 16).ok_or("invalid clipping window")?;
    let excerpt = SourceExcerpt::capture(
        source,
        ExcerptView::TransportDecodedXmlBytes,
        context,
        Some(problem),
    )
    .ok_or("failed to capture source excerpt")?
    .clip(window)
    .ok_or("failed to clip source excerpt")?;
    let source_problem = excerpt
        .source_problem()
        .ok_or("missing full problem range")?;
    assert_eq!(excerpt.bytes(), b"cdefghij");
    assert_eq!(excerpt.problem(), None);
    assert_eq!(excerpt.original_problem(), None);

    let excerpt_values = format!(
        ", x'636465666768696a', '{}', {}, {}, {}",
        excerpt.view().as_str(),
        excerpt.start_byte().ok_or("missing captured anchor")?,
        source_problem.start(),
        source_problem.end(),
    );
    fixture.insert_diagnostic(
        "clipped-transport",
        "run-a",
        "document-a",
        ", source_excerpt, excerpt_view, excerpt_start_byte, source_problem_start_byte, \
         source_problem_end_byte",
        &excerpt_values,
    )?;
    let span = sql_query(
        "SELECT excerpt_start_byte, problem_start_byte, problem_end_byte, \
                source_problem_start_byte, source_problem_end_byte, \
                original_problem_start_byte, original_problem_end_byte \
         FROM import_diagnostics WHERE diagnostic_key = 'clipped-transport'",
    )
    .get_result::<ClippedSpan>(&mut fixture.connection)?;
    assert_eq!(span.excerpt_start, 12);
    assert_eq!((span.problem_start, span.problem_end), (None, None));
    assert_eq!((span.source_start, span.source_end), (8, 20));
    assert_eq!((span.original_start, span.original_end), (None, None));
    Ok(())
}
