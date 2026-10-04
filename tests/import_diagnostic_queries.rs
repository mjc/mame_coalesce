use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    create_backup,
    database::Database,
    diagnostics::CoordinateConvention,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    import_diagnostics::{
        self, DiagnosticCode, DiagnosticPageLimit, DiagnosticQueryError, DiagnosticSeverity,
        ImportRunStatus, NoIntroDiagnosticOwner,
    },
    logiqx::{LogiqxMode, RecordLocation},
    no_intro_db_xml::NoIntroDatabaseMode,
    restore_backup,
};

#[path = "support/import_warning_fixture.rs"]
mod import_warning_fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct TriggerName {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct StoredText {
    #[diesel(sql_type = Text)]
    value: String,
}

struct OwnerFixture {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    document: Utf8PathBuf,
    database: Database,
    first: mame_coalesce::app::CatalogImportReport,
    second: mame_coalesce::app::CatalogImportReport,
}

fn request(
    document_path: Utf8PathBuf,
    format: CatalogDocumentFormat,
    name: &str,
) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path,
        format,
        source_key: PublishingSourceKey::new(format!("diagnostic-source-{name}")),
        source_display_name: format!("Diagnostic source {name}"),
        catalog_key: CatalogKey::new(format!("diagnostic-catalog-{name}")),
        catalog_display_name: format!("Diagnostic catalog {name}"),
        scope: CatalogScope::Complete,
    }
}

fn drop_table_triggers(connection: &mut SqliteConnection, table: &str) -> TestResult {
    let triggers = sql_query(
        "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
    )
    .bind::<Text, _>(table)
    .load::<TriggerName>(connection)?;
    for trigger in triggers {
        let escaped = trigger.name.replace('"', "\"\"");
        connection.batch_execute(&format!("DROP TRIGGER \"{escaped}\""))?;
    }
    Ok(())
}

fn owner_fixture() -> TestResult<OwnerFixture> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = Utf8PathBuf::try_from(directory.path().join("all-owners.xml"))?;
    std::fs::write(
        &document,
        import_warning_fixture::native_owner_nul_document(),
    )?;
    let database = Database::open(&path)?;
    let import_request = request(
        document.clone(),
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
        "all-owners",
    );
    let first = app::import_catalog(&database, &import_request)?;
    let second = app::import_catalog(&database, &import_request)?;
    assert_eq!(first.status, CatalogImportStatus::Succeeded);
    assert_eq!(second.status, CatalogImportStatus::Succeeded);
    assert_eq!(first.snapshot_key, second.snapshot_key);
    assert_ne!(first.run_key, second.run_key);
    assert_eq!(first.diagnostic_count, 14);
    assert_eq!(second.diagnostic_count, 14);
    Ok(OwnerFixture {
        directory,
        path,
        document,
        database,
        first,
        second,
    })
}

fn alternate_document_key(fixture: &OwnerFixture) -> TestResult<String> {
    let document = Utf8PathBuf::try_from(fixture.directory.path().join("other.xml"))?;
    std::fs::write(
        &document,
        b"<datafile><game name='other'><rom name='other.rom'/></game></datafile>",
    )?;
    let imported = app::import_catalog(
        &fixture.database,
        &request(
            document,
            CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
            "other-document",
        ),
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    let key = sql_query("SELECT document_key AS value FROM import_runs WHERE run_key=?")
        .bind::<Text, _>(imported.run_key.to_string())
        .get_result::<StoredText>(&mut connection)?;
    Ok(key.value)
}

fn set_diagnostic_document(
    fixture: &OwnerFixture,
    run_key: &mame_coalesce::domain::ImportRunKey,
    order: i64,
    document_key: &str,
) -> TestResult<usize> {
    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;
    drop_table_triggers(&mut connection, "import_diagnostics")?;
    Ok(sql_query(
        "UPDATE import_diagnostics SET document_key=? WHERE run_key=? AND diagnostic_order=?",
    )
    .bind::<Text, _>(document_key)
    .bind::<Text, _>(run_key.to_string())
    .bind::<BigInt, _>(order)
    .execute(&mut connection)?)
}

fn assert_all_owner_kinds(page: &import_diagnostics::DiagnosticPage) -> TestResult<(String, i64)> {
    let mut counts = [0_u8; 13];
    let mut details_key = None;
    let mut archive_id = None;
    for diagnostic in &page.diagnostics {
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
        assert_eq!(diagnostic.code, DiagnosticCode::XmlNulRecovered);
        assert_eq!(
            diagnostic.coordinates,
            Some(CoordinateConvention::XmlUnicodeScalars)
        );
        assert_eq!(diagnostic.owners.len(), 1);
        assert!(diagnostic.source_line.is_some_and(|line| line > 0));
        assert!(diagnostic.source_column.is_some_and(|column| column > 0));
        let excerpt = diagnostic
            .excerpt
            .as_ref()
            .ok_or("missing source excerpt")?;
        assert!(excerpt.problem().is_some());
        assert!(excerpt.source_problem().is_some());
        assert!(excerpt.original_problem().is_some());
        match &diagnostic.owners[0] {
            NoIntroDiagnosticOwner::ExportDocument { extent, .. } => {
                counts[0] += 1;
                assert_eq!(extent.start(), RecordLocation { line: 1, column: 1 });
            }
            NoIntroDiagnosticOwner::ExportHeader { .. } => counts[1] += 1,
            NoIntroDiagnosticOwner::HeaderField { .. } => counts[2] += 1,
            NoIntroDiagnosticOwner::Game { .. } => counts[3] += 1,
            NoIntroDiagnosticOwner::ArchiveDescription { id, .. } => {
                counts[4] += 1;
                archive_id = Some(id.as_i64());
            }
            NoIntroDiagnosticOwner::DumpSource { .. } => counts[5] += 1,
            NoIntroDiagnosticOwner::DumpDetails { .. } => {
                counts[6] += 1;
                details_key = Some(diagnostic.key.as_str().to_owned());
            }
            NoIntroDiagnosticOwner::DumpSerials { .. } => counts[7] += 1,
            NoIntroDiagnosticOwner::DumpFile { .. } => counts[8] += 1,
            NoIntroDiagnosticOwner::Release { .. } => counts[9] += 1,
            NoIntroDiagnosticOwner::ReleaseDetails { .. } => counts[10] += 1,
            NoIntroDiagnosticOwner::ReleaseSerials { .. } => counts[11] += 1,
            NoIntroDiagnosticOwner::ReleaseFile { .. } => counts[12] += 1,
        }
    }
    assert_eq!(counts, [2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    Ok((
        details_key.ok_or("dump details diagnostic missing")?,
        archive_id.ok_or("archive owner missing")?,
    ))
}

#[test]
fn page_query_rejects_a_diagnostic_from_another_document() -> TestResult {
    let fixture = owner_fixture()?;
    let other_document = alternate_document_key(&fixture)?;
    assert_eq!(
        set_diagnostic_document(&fixture, &fixture.first.run_key, 1, &other_document)?,
        1
    );
    assert!(matches!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.first.run_key,
            None,
            DiagnosticPageLimit::new(2)?,
        ),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[test]
fn summary_rejects_an_out_of_page_first_diagnostic_from_another_document() -> TestResult {
    let fixture = owner_fixture()?;
    let first_page = import_diagnostics::for_run(
        &fixture.database,
        &fixture.first.run_key,
        None,
        DiagnosticPageLimit::new(1)?,
    )?;
    let cursor = first_page
        .next_cursor
        .as_ref()
        .ok_or("second page missing")?;
    let other_document = alternate_document_key(&fixture)?;
    assert_eq!(
        set_diagnostic_document(&fixture, &fixture.first.run_key, 0, &other_document)?,
        1
    );
    assert!(matches!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.first.run_key,
            Some(cursor),
            DiagnosticPageLimit::new(1)?,
        ),
        Err(DiagnosticQueryError::InvalidMetadata(_))
    ));
    Ok(())
}

#[test]
fn page_limits_and_stored_orders_are_checked() {
    assert!(DiagnosticPageLimit::new(0).is_err());
    assert!(DiagnosticPageLimit::new(501).is_err());
    assert!(DiagnosticPageLimit::new(1).is_ok());
    assert!(DiagnosticPageLimit::new(500).is_ok());
    assert!(mame_coalesce::import_diagnostics::DiagnosticOrder::new(-1).is_none());
    assert_eq!(
        mame_coalesce::import_diagnostics::DiagnosticOrder::new(0)
            .map(mame_coalesce::import_diagnostics::DiagnosticOrder::get),
        Some(0)
    );
}

#[test]
fn recovery_diagnostics_can_be_queried_after_the_input_is_removed() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(
        &document,
        b"<datafile><game name='a\0b'/><game name='c\0d'/></datafile>",
    )?;
    let database = Database::open(&path)?;
    let imported = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document.clone(),
            format: CatalogDocumentFormat::NoIntroDatabase(
                NoIntroDatabaseMode::NullRecoveryCompatible,
            ),
            source_key: PublishingSourceKey::new("diagnostic-source"),
            source_display_name: "Diagnostic source".into(),
            catalog_key: CatalogKey::new("diagnostic-catalog"),
            catalog_display_name: "Diagnostic catalog".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    std::fs::remove_file(&document)?;
    let page = import_diagnostics::for_run(
        &database,
        &imported.run_key,
        None,
        DiagnosticPageLimit::new(1)?,
    )?;
    assert_eq!(page.diagnostics.len(), 1);
    assert_eq!(page.run.status, ImportRunStatus::Succeeded);
    assert_eq!(
        page.run.document.retention,
        import_diagnostics::DocumentRetention::Retained
    );
    assert_eq!(
        page.run.interpretation.format,
        "no-intro-database-xml-nul-compatible"
    );
    assert!(page.run.snapshot.is_some());
    let summary = page.run.summary.clone();
    assert!(summary.is_some());
    let diagnostic = &page.diagnostics[0];
    assert_eq!(diagnostic.order.get(), 0);
    assert_eq!(diagnostic.code, DiagnosticCode::XmlNulRecovered);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.record_kind.as_deref(), Some("document"));
    assert!(diagnostic.field_name.is_none());
    assert!(diagnostic.offending_text.is_none());
    assert!(diagnostic.source_line.is_some_and(|line| line > 0));
    assert!(diagnostic.source_column.is_some_and(|column| column > 0));
    assert!(diagnostic.excerpt.is_some());
    assert!(!diagnostic.owners.is_empty());
    let cursor = page
        .next_cursor
        .as_ref()
        .ok_or("second warning has a cursor")?;
    let final_page = import_diagnostics::for_run(
        &database,
        &imported.run_key,
        Some(cursor),
        DiagnosticPageLimit::new(1)?,
    )?;
    assert_eq!(final_page.diagnostics.len(), 1);
    assert_eq!(final_page.diagnostics[0].order.get(), 1);
    assert!(final_page.next_cursor.is_none());
    assert_eq!(final_page.run.summary, summary);
    Ok(())
}

#[test]
fn failed_runs_keep_their_error_diagnostic_without_snapshot_or_native_owner() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("broken.xml"))?;
    std::fs::write(&document, b"<datafile><game name='broken'></datafile>")?;
    let database = Database::open(&path)?;
    let imported = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("broken-source"),
            source_display_name: "Broken source".into(),
            catalog_key: CatalogKey::new("broken-catalog"),
            catalog_display_name: "Broken catalog".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Failed);

    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA ignore_check_constraints=ON")?;
    drop_table_triggers(&mut connection, "import_diagnostics")?;
    sql_query(
        "UPDATE import_diagnostics SET source_line=NULL,source_column=-5, \
         coordinate_view=NULL,column_convention=NULL WHERE run_key=?",
    )
    .bind::<Text, _>(imported.run_key.to_string())
    .execute(&mut connection)?;

    let page = import_diagnostics::for_run(
        &database,
        &imported.run_key,
        None,
        DiagnosticPageLimit::new(10)?,
    )?;
    assert_eq!(page.run.status, ImportRunStatus::Failed);
    assert!(page.run.snapshot.is_none());
    assert_eq!(page.diagnostics.len(), 1);
    assert_eq!(page.diagnostics[0].code, DiagnosticCode::ParseFailed);
    assert_eq!(page.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert!(page.diagnostics[0].owners.is_empty());
    assert_eq!(page.diagnostics[0].source_line, None);
    assert_eq!(page.diagnostics[0].source_column, Some(-5));
    assert_eq!(page.diagnostics[0].coordinates, None);
    assert_eq!(
        page.run.summary.as_deref(),
        Some(page.diagnostics[0].message.as_str())
    );
    Ok(())
}

#[test]
fn public_query_resolves_all_native_owner_kinds_and_full_export_extent() -> TestResult {
    let fixture = owner_fixture()?;
    let page = import_diagnostics::for_run(
        &fixture.database,
        &fixture.first.run_key,
        None,
        DiagnosticPageLimit::new(20)?,
    )?;
    assert_eq!(page.diagnostics.len(), 14);
    assert_eq!(page.run.status, ImportRunStatus::Succeeded);
    assert_eq!(
        page.run.catalog.display_name,
        "Diagnostic catalog all-owners"
    );
    assert_eq!(
        page.run.catalog.source.display_name,
        "Diagnostic source all-owners"
    );
    assert!(page.run.snapshot.is_some());
    assert!(page.run.summary.is_some());
    assert!(page.next_cursor.is_none());

    assert_all_owner_kinds(&page)?;
    Ok(())
}

#[test]
fn source_free_query_survives_backup_restore_and_missing_objects() -> TestResult {
    let fixture = owner_fixture()?;
    let backup = Utf8PathBuf::try_from(fixture.directory.path().join("backup.sqlite"))?;
    let restored = Utf8PathBuf::try_from(fixture.directory.path().join("restored.sqlite"))?;
    create_backup(&fixture.path, &backup)?;
    restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    std::fs::remove_file(&fixture.document)?;
    let restored_objects = Utf8PathBuf::from(format!("{restored}.documents"));
    let unavailable_objects = fixture
        .directory
        .path()
        .join("unavailable-restored-documents");
    std::fs::rename(&restored_objects, &unavailable_objects)?;
    let restored_page = import_diagnostics::for_run(
        &restored_database,
        &fixture.second.run_key,
        None,
        DiagnosticPageLimit::new(20)?,
    )?;
    assert_eq!(restored_page.diagnostics.len(), 14);
    assert_eq!(
        restored_page.run.snapshot.as_ref().map(|item| &item.key),
        fixture.second.snapshot_key.as_ref()
    );
    assert_eq!(
        restored_page.run.document.retention,
        import_diagnostics::DocumentRetention::Retained
    );
    Ok(())
}

#[test]
fn cursors_reject_other_runs_missing_anchors_and_registry_changes() -> TestResult {
    let fixture = owner_fixture()?;
    let first_page = import_diagnostics::for_run(
        &fixture.database,
        &fixture.first.run_key,
        None,
        DiagnosticPageLimit::new(1)?,
    )?;
    let cursor = first_page.next_cursor.as_ref().ok_or("cursor missing")?;
    assert!(matches!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.second.run_key,
            Some(cursor),
            DiagnosticPageLimit::new(1)?,
        ),
        Err(DiagnosticQueryError::CursorRunMismatch)
    ));

    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;
    drop_table_triggers(&mut connection, "import_diagnostics")?;
    sql_query("DELETE FROM import_diagnostics WHERE run_key=? AND diagnostic_order=0")
        .bind::<Text, _>(fixture.first.run_key.to_string())
        .execute(&mut connection)?;
    assert!(matches!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.first.run_key,
            Some(cursor),
            DiagnosticPageLimit::new(1)?,
        ),
        Err(DiagnosticQueryError::InvalidCursor)
    ));

    drop_table_triggers(&mut connection, "file_id_registries")?;
    sql_query("UPDATE file_id_registries SET registry_uuid=randomblob(16) WHERE registry_id=1")
        .execute(&mut connection)?;
    assert!(matches!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.first.run_key,
            Some(cursor),
            DiagnosticPageLimit::new(1)?,
        ),
        Err(DiagnosticQueryError::CursorRegistryMismatch)
    ));
    Ok(())
}

#[test]
fn owner_hydration_rejects_wrong_storage_class_and_dangling_rows() -> TestResult {
    let fixture = owner_fixture()?;
    let page = import_diagnostics::for_run(
        &fixture.database,
        &fixture.second.run_key,
        None,
        DiagnosticPageLimit::new(20)?,
    )?;
    let (details_key, archive_id) = assert_all_owner_kinds(&page)?;
    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")?;
    drop_table_triggers(&mut connection, "no_intro_dump_details_diagnostics")?;
    let changed = sql_query(
        "UPDATE no_intro_dump_details_diagnostics SET diagnostic_key=CAST(diagnostic_key AS BLOB) \
         WHERE run_key=? AND diagnostic_key=?",
    )
    .bind::<Text, _>(fixture.second.run_key.to_string())
    .bind::<Text, _>(&details_key)
    .execute(&mut connection)?;
    assert_eq!(changed, 1);
    assert!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.second.run_key,
            None,
            DiagnosticPageLimit::new(20)?,
        )
        .is_err()
    );

    let changed = sql_query(
        "UPDATE no_intro_dump_details_diagnostics SET diagnostic_key=CAST(diagnostic_key AS TEXT) \
         WHERE run_key=? AND diagnostic_key=CAST(? AS BLOB)",
    )
    .bind::<Text, _>(fixture.second.run_key.to_string())
    .bind::<Text, _>(&details_key)
    .execute(&mut connection)?;
    assert_eq!(changed, 1);
    drop_table_triggers(&mut connection, "no_intro_archive_descriptions")?;
    let changed = sql_query(
        "UPDATE no_intro_archive_descriptions SET source_line=CAST(source_line AS BLOB) \
         WHERE archive_id=?",
    )
    .bind::<BigInt, _>(archive_id)
    .execute(&mut connection)?;
    assert_eq!(changed, 1);
    assert!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.second.run_key,
            None,
            DiagnosticPageLimit::new(20)?,
        )
        .is_err()
    );
    let changed = sql_query(
        "UPDATE no_intro_archive_descriptions SET source_line=CAST(source_line AS INTEGER) \
         WHERE archive_id=?",
    )
    .bind::<BigInt, _>(archive_id)
    .execute(&mut connection)?;
    assert_eq!(changed, 1);
    let restored_page = import_diagnostics::for_run(
        &fixture.database,
        &fixture.second.run_key,
        None,
        DiagnosticPageLimit::new(20)?,
    )?;
    assert_all_owner_kinds(&restored_page)?;
    let deleted = sql_query("DELETE FROM no_intro_archive_descriptions WHERE archive_id=?")
        .bind::<BigInt, _>(archive_id)
        .execute(&mut connection)?;
    assert_eq!(deleted, 1);
    assert!(
        import_diagnostics::for_run(
            &fixture.database,
            &fixture.second.run_key,
            None,
            DiagnosticPageLimit::new(20)?,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn non_no_intro_runs_use_the_shared_source_free_context() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = Utf8PathBuf::try_from(directory.path().join("logiqx.xml"))?;
    std::fs::write(
        &document,
        b"<datafile><game name='game'><rom name='rom'/></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    let imported = app::import_catalog(
        &database,
        &request(
            document.clone(),
            CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
            "logiqx",
        ),
    )?;
    std::fs::remove_file(document)?;
    let page = import_diagnostics::for_run(
        &database,
        &imported.run_key,
        None,
        DiagnosticPageLimit::new(1)?,
    )?;
    assert!(page.diagnostics.is_empty());
    assert!(page.next_cursor.is_none());
    assert!(page.run.summary.is_none());
    assert!(page.run.snapshot.is_some());
    assert_eq!(page.run.interpretation.format, "logiqx");
    Ok(())
}
