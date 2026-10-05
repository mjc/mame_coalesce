use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    Error,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct StringRow {
    #[diesel(sql_type = Text)]
    value: String,
}

fn import_request(
    directory: &tempfile::TempDir,
    key: &str,
    contents: &str,
) -> TestResult<CatalogImportRequest> {
    let document_path = Utf8PathBuf::try_from(directory.path().join(format!("{key}.dat")))?;
    std::fs::write(&document_path, contents)?;
    Ok(CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::ClrMamePro,
        source_key: PublishingSourceKey::new("cmp-streaming-storage"),
        source_display_name: "CMP streaming storage".into(),
        catalog_key: CatalogKey::new("cmp-streaming-storage"),
        catalog_display_name: "CMP streaming storage".into(),
        scope: CatalogScope::Complete,
    })
}

fn count(connection: &mut SqliteConnection, table: &str) -> TestResult<i64> {
    Ok(sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
        .get_result::<CountRow>(connection)?
        .count)
}

fn string_value(connection: &mut SqliteConnection, query: &str, key: &str) -> TestResult<String> {
    Ok(sql_query(query)
        .bind::<Text, _>(key)
        .get_result::<StringRow>(connection)?
        .value)
}

#[test]
fn storage_failure_on_completed_set_precedes_invalid_late_set_and_rolls_back() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    let control = import_request(
        &directory,
        "healthy-control",
        "game ( name control rom ( name control.bin size 1 crc 12345678 ) )",
    )?;
    let control_report = app::import_catalog(&database, &control)?;
    assert_eq!(control_report.status, CatalogImportStatus::Succeeded);
    assert!(control_report.snapshot_key.is_some());

    let rollback_tables = [
        "catalog_snapshots",
        "snapshot_publications",
        "catalog_set_groups",
        "catalog_sets",
        "cmp_set_facts",
        "asset_requirements",
        "asset_occurrences",
        "cmp_rom_claims",
        "cmp_rom_field_positions",
        "catalog_contents",
        "digest_values",
        "catalog_content_digest_assertions",
        "occurrence_digest_assertions",
        "record_namespaces",
        "records",
        "import_runs",
        "import_diagnostics",
    ];
    let before = rollback_tables
        .iter()
        .map(|table| Ok((*table, count(&mut connection, table)?)))
        .collect::<TestResult<Vec<_>>>()?;

    connection.batch_execute(
        "CREATE TRIGGER cmp_rom_storage_sentinel BEFORE INSERT ON cmp_rom_claims
         BEGIN SELECT RAISE(ABORT, 'cmp-rom-write-before-invalid-tail'); END;",
    )?;
    let late_invalid = import_request(
        &directory,
        "storage-sentinel",
        "game ( name staged rom ( name staged.bin size 1 sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ) ) \
         game ( name broken rom ( name broken.bin size nope ) )",
    )?;
    let result = app::import_catalog(&database, &late_invalid);
    connection.batch_execute("DROP TRIGGER cmp_rom_storage_sentinel;")?;

    let error = match result {
        Err(error) => error,
        Ok(report) => {
            return Err(format!(
                "storage sentinel was not terminal; import returned status {} with {} diagnostics",
                report.status.as_str(),
                report.diagnostic_count
            )
            .into());
        }
    };
    assert!(
        matches!(
            &error,
            Error::Diesel(diesel::result::Error::DatabaseError(..))
        ),
        "storage error changed type: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("cmp-rom-write-before-invalid-tail"),
        "storage sentinel was not returned: {error}"
    );

    for (table, expected) in before {
        assert_eq!(
            count(&mut connection, table)?,
            expected,
            "rollback in {table}"
        );
    }
    assert_eq!(count(&mut connection, "documents")?, 2);
    Ok(())
}

#[test]
fn early_header_semantic_error_disables_later_native_writes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    let control = import_request(
        &directory,
        "healthy-control",
        "game ( name control rom ( name control.bin size 1 crc 12345678 ) )",
    )?;
    assert_eq!(
        app::import_catalog(&database, &control)?.status,
        CatalogImportStatus::Succeeded
    );
    let claims_before = count(&mut connection, "cmp_rom_claims")?;
    connection.batch_execute(
        "CREATE TRIGGER cmp_rom_storage_sentinel BEFORE INSERT ON cmp_rom_claims
         BEGIN SELECT RAISE(ABORT, 'callback-was-not-disabled'); END;",
    )?;

    let request = import_request(
        &directory,
        "invalid-header",
        "clrmamepro ( name first name second ) game ( name later rom ( name later.bin crc 87654321 ) )",
    )?;
    let result = app::import_catalog(&database, &request);
    connection.batch_execute("DROP TRIGGER cmp_rom_storage_sentinel;")?;
    let report = result?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(count(&mut connection, "cmp_rom_claims")?, claims_before);
    assert_eq!(count(&mut connection, "cmp_header_facts")?, 0);
    Ok(())
}

#[test]
fn streamed_comments_and_late_header_finalize_once_at_eof_and_reimport() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let source =
        "; before\ngame ( name one ) ; between\nclrmamepro ( name late version v9 ) ; after\n";
    let request = import_request(&directory, "late-header", source)?;

    let first = app::import_catalog(&database, &request)?;
    assert_eq!(first.status, CatalogImportStatus::Succeeded);
    let snapshot = first.snapshot_key.as_ref().ok_or("snapshot missing")?;
    assert_eq!(count(&mut connection, "cmp_documents")?, 1);
    assert_eq!(count(&mut connection, "cmp_comments")?, 3);
    assert_eq!(
        string_value(
            &mut connection,
            "SELECT name AS value FROM cmp_header_facts WHERE snapshot_key = ?",
            snapshot.as_str(),
        )?,
        "late"
    );
    assert_eq!(
        count(&mut connection, "cmp_comments")?,
        count(&mut connection, "cmp_documents")? + 2
    );

    let second = app::import_catalog(&database, &request)?;
    assert_eq!(second.status, CatalogImportStatus::Succeeded);
    assert_eq!(second.snapshot_key, first.snapshot_key);
    assert_eq!(count(&mut connection, "cmp_documents")?, 1);
    assert_eq!(count(&mut connection, "cmp_comments")?, 3);
    Ok(())
}

#[test]
fn late_form_failure_rolls_back_streamed_comments_and_completed_sets() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let before = [
        "catalog_snapshots",
        "catalog_set_groups",
        "catalog_sets",
        "cmp_set_facts",
        "cmp_comments",
        "cmp_documents",
        "cmp_rom_claims",
        "catalog_contents",
    ]
    .iter()
    .map(|table| Ok((*table, count(&mut connection, table)?)))
    .collect::<TestResult<Vec<_>>>()?;
    let request = import_request(
        &directory,
        "late-form-failure",
        "; streamed comment\ngame ( name staged rom ( name staged.bin size 1 sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ) ) game ( name broken rom ( name broken.bin size ) )",
    )?;

    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    for (table, expected) in before {
        assert_eq!(
            count(&mut connection, table)?,
            expected,
            "rollback in {table}"
        );
    }
    Ok(())
}
