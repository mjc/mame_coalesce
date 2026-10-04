use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus};
use mame_coalesce::database::Database;
use mame_coalesce::domain::{CatalogKey, CatalogScope, ImportRunKey, PublishingSourceKey};
use mame_coalesce::import_diagnostics::{
    self, DiagnosticCode, DiagnosticPage, DiagnosticPageLimit, DiagnosticQueryError,
};
use mame_coalesce::logiqx::LogiqxMode;
use mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode;

#[path = "support/import_warning_fixture.rs"]
mod import_warning_fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BROKEN_LOGIQX: &[u8] = b"<datafile><!-- \xef\xbf\xbe --></datafile>";
const VALID_LOGIQX: &[u8] = b"<datafile><game name=\"valid\"/></datafile>";

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    run: ImportRunKey,
    report: app::CatalogImportReport,
}

#[derive(QueryableByName)]
struct TriggerName {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
    #[diesel(sql_type = BigInt)]
    matching_blobs: i64,
}

#[derive(QueryableByName)]
struct StoredRun {
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    status_class: String,
    #[diesel(sql_type = Text)]
    snapshot_class: String,
}

#[derive(QueryableByName)]
struct StoredLink {
    #[diesel(sql_type = Text)]
    diagnostic_key: String,
    #[diesel(sql_type = Text)]
    run_key: String,
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    diagnostic_class: String,
    #[diesel(sql_type = Text)]
    run_class: String,
    #[diesel(sql_type = Text)]
    snapshot_class: String,
}

fn fixture(bytes: &[u8], format: CatalogDocumentFormat, name: &str) -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("source.xml"))?;
    std::fs::write(&document_path, bytes)?;
    let database = Database::open(&database_path)?;
    let report = import_document(&database, &document_path, format, name)?;
    Ok(Fixture {
        _directory: directory,
        database,
        database_path,
        run: report.run_key,
        report,
    })
}

fn import_document(
    database: &Database,
    document_path: &Utf8PathBuf,
    format: CatalogDocumentFormat,
    name: &str,
) -> TestResult<app::CatalogImportReport> {
    Ok(app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: document_path.clone(),
            format,
            source_key: PublishingSourceKey::new(format!("review-source-{name}")),
            source_display_name: format!("Review source {name}"),
            catalog_key: CatalogKey::new(format!("review-catalog-{name}")),
            catalog_display_name: format!("Review catalog {name}"),
            scope: CatalogScope::Complete,
        },
    )?)
}

fn no_intro_fixture(name: &str) -> TestResult<Fixture> {
    fixture(
        import_warning_fixture::native_owner_nul_document().as_bytes(),
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
        name,
    )
}

fn page(
    database: &Database,
    run: &ImportRunKey,
    limit: usize,
) -> Result<DiagnosticPage, DiagnosticQueryError> {
    DiagnosticPageLimit::new(limit)
        .and_then(|limit| import_diagnostics::for_run(database, run, None, limit))
}

fn unguarded_connection(path: &Utf8PathBuf, table: &str) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")?;
    let triggers = sql_query(
        "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
    )
    .bind::<Text, _>(table)
    .load::<TriggerName>(&mut connection)?;
    for trigger in triggers {
        let escaped = trigger.name.replace('"', "\"\"");
        connection.batch_execute(&format!("DROP TRIGGER \"{escaped}\""))?;
    }
    Ok(connection)
}

fn run_bytes(run: &ImportRunKey) -> Vec<u8> {
    run.to_string().into_bytes()
}

#[test]
fn byte_identical_blob_run_keys_cannot_hide_an_entire_diagnostic_run() -> TestResult {
    let all_blob = no_intro_fixture("all-blob-run-key")?;
    let healthy = page(&all_blob.database, &all_blob.run, 1)?;
    assert_eq!(all_blob.report.status, CatalogImportStatus::Succeeded);
    assert_eq!(healthy.diagnostics.len(), 1);
    assert!(healthy.next_cursor.is_some());

    let mut connection = unguarded_connection(&all_blob.database_path, "import_diagnostics")?;
    let changed =
        sql_query("UPDATE import_diagnostics SET run_key=CAST(run_key AS BLOB) WHERE run_key=?")
            .bind::<Text, _>(all_blob.run.to_string())
            .execute(&mut connection)?;
    assert_eq!(changed, all_blob.report.diagnostic_count);
    let expected_diagnostics = i64::try_from(all_blob.report.diagnostic_count)?;
    let witness = sql_query(
        "SELECT COUNT(*) AS count, SUM(typeof(run_key)='blob') AS matching_blobs \
         FROM import_diagnostics WHERE CAST(run_key AS BLOB)=?",
    )
    .bind::<Binary, _>(run_bytes(&all_blob.run))
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(witness.count, expected_diagnostics);
    assert_eq!(witness.matching_blobs, witness.count);
    assert!(matches!(
        page(&all_blob.database, &all_blob.run, 1),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[test]
fn byte_identical_blob_run_keys_are_rejected_beyond_the_selected_page() -> TestResult {
    let partly_blob = no_intro_fixture("out-of-page-blob-run-key")?;
    let healthy = page(&partly_blob.database, &partly_blob.run, 1)?;
    assert_eq!(partly_blob.report.status, CatalogImportStatus::Succeeded);
    assert_eq!(healthy.diagnostics.len(), 1);
    assert!(healthy.next_cursor.is_some());

    let changed_order = i64::try_from(partly_blob.report.diagnostic_count)? - 1;
    assert!(
        changed_order > 1,
        "fixture must place a corrupted row beyond page one"
    );
    let mut connection = unguarded_connection(&partly_blob.database_path, "import_diagnostics")?;
    let changed = sql_query(
        "UPDATE import_diagnostics SET run_key=CAST(run_key AS BLOB) \
         WHERE run_key=? AND diagnostic_order=?",
    )
    .bind::<Text, _>(partly_blob.run.to_string())
    .bind::<BigInt, _>(changed_order)
    .execute(&mut connection)?;
    assert_eq!(changed, 1);
    let witness = sql_query(
        "SELECT COUNT(*) AS count, SUM(typeof(run_key)='blob') AS matching_blobs \
         FROM import_diagnostics WHERE CAST(run_key AS BLOB)=? AND diagnostic_order=?",
    )
    .bind::<Binary, _>(run_bytes(&partly_blob.run))
    .bind::<BigInt, _>(changed_order)
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(witness.count, 1);
    assert_eq!(witness.matching_blobs, 1);
    assert!(matches!(
        page(&partly_blob.database, &partly_blob.run, 1),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[test]
fn failed_run_with_a_valid_snapshot_is_rejected_even_without_diagnostics() -> TestResult {
    let fixture = fixture(
        VALID_LOGIQX,
        CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
        "successful-logiqx-no-warnings",
    )?;
    assert_eq!(fixture.report.status, CatalogImportStatus::Succeeded);
    assert!(fixture.report.snapshot_key.is_some());
    let expected_snapshot = fixture
        .report
        .snapshot_key
        .as_ref()
        .ok_or_else(|| std::io::Error::other("successful Logiqx import has a snapshot"))?
        .to_string();
    let healthy = page(&fixture.database, &fixture.run, 10)?;
    assert!(healthy.diagnostics.is_empty());
    assert!(healthy.run.snapshot.is_some());

    let mut connection = unguarded_connection(&fixture.database_path, "import_runs")?;
    let changed = sql_query("UPDATE import_runs SET status='failed' WHERE run_key=?")
        .bind::<Text, _>(fixture.run.to_string())
        .execute(&mut connection)?;
    assert_eq!(changed, 1);
    let stored = sql_query(
        "SELECT status, snapshot_key, typeof(status) AS status_class, \
         typeof(snapshot_key) AS snapshot_class FROM import_runs WHERE run_key=?",
    )
    .bind::<Text, _>(fixture.run.to_string())
    .get_result::<StoredRun>(&mut connection)?;
    assert_eq!(stored.status, "failed");
    assert_eq!(stored.snapshot_key, expected_snapshot);
    assert_eq!(stored.status_class, "text");
    assert_eq!(stored.snapshot_class, "text");
    assert!(matches!(
        page(&fixture.database, &fixture.run, 10),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[test]
fn failed_logiqx_parse_diagnostic_rejects_a_no_intro_export_owner_link() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let no_intro_path = Utf8PathBuf::try_from(directory.path().join("no-intro.xml"))?;
    let logiqx_path = Utf8PathBuf::try_from(directory.path().join("broken-logiqx.xml"))?;
    std::fs::write(
        &no_intro_path,
        import_warning_fixture::native_owner_nul_document(),
    )?;
    std::fs::write(&logiqx_path, BROKEN_LOGIQX)?;
    let database = Database::open(&database_path)?;
    let no_intro = import_document(
        &database,
        &no_intro_path,
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
        "owner-export",
    )?;
    assert_eq!(no_intro.status, CatalogImportStatus::Succeeded);
    let export_snapshot = no_intro
        .snapshot_key
        .as_ref()
        .ok_or_else(|| std::io::Error::other("successful No-Intro import has a snapshot"))?
        .to_string();

    let failed = import_document(
        &database,
        &logiqx_path,
        CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
        "failed-logiqx-owner-link",
    )?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    let healthy = page(&database, &failed.run_key, 10)?;
    assert!(healthy.run.snapshot.is_none());
    assert_eq!(healthy.diagnostics.len(), 1);
    assert_eq!(healthy.diagnostics[0].code, DiagnosticCode::ParseFailed);
    assert!(healthy.diagnostics[0].owners.is_empty());

    let mut connection = unguarded_connection(&database_path, "no_intro_export_diagnostics")?;
    let export_count = sql_query(
        "SELECT COUNT(*) AS count, SUM(typeof(snapshot_key)='text') AS matching_blobs \
         FROM no_intro_exports WHERE snapshot_key=?",
    )
    .bind::<Text, _>(&export_snapshot)
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(export_count.count, 1);
    assert_eq!(export_count.matching_blobs, 1);

    let diagnostic_key = sql_query(
        "SELECT diagnostic_key AS value FROM import_diagnostics \
         WHERE run_key=? AND code='parse_failed' AND severity='error'",
    )
    .bind::<Text, _>(failed.run_key.to_string())
    .get_result::<DiagnosticKeyRow>(&mut connection)?
    .value;
    let inserted = sql_query(
        "INSERT INTO no_intro_export_diagnostics(diagnostic_key,run_key,snapshot_key) \
         VALUES (?,?,?)",
    )
    .bind::<Text, _>(&diagnostic_key)
    .bind::<Text, _>(failed.run_key.to_string())
    .bind::<Text, _>(&export_snapshot)
    .execute(&mut connection)?;
    assert_eq!(inserted, 1);
    let link = sql_query(
        "SELECT diagnostic_key, run_key, snapshot_key, \
         typeof(diagnostic_key) AS diagnostic_class, typeof(run_key) AS run_class, \
         typeof(snapshot_key) AS snapshot_class FROM no_intro_export_diagnostics \
         WHERE diagnostic_key=? AND run_key=? AND snapshot_key=?",
    )
    .bind::<Text, _>(&diagnostic_key)
    .bind::<Text, _>(failed.run_key.to_string())
    .bind::<Text, _>(&export_snapshot)
    .get_result::<StoredLink>(&mut connection)?;
    assert_eq!(link.diagnostic_key, diagnostic_key);
    assert_eq!(link.run_key, failed.run_key.to_string());
    assert_eq!(link.snapshot_key, export_snapshot);
    assert_eq!(link.diagnostic_class, "text");
    assert_eq!(link.run_class, "text");
    assert_eq!(link.snapshot_class, "text");
    assert!(matches!(
        page(&database, &failed.run_key, 10),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[derive(QueryableByName)]
struct DiagnosticKeyRow {
    #[diesel(sql_type = Text)]
    value: String,
}
