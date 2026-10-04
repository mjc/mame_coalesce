use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    diagnostics::ExcerptView,
    domain::{CatalogKey, CatalogScope, ImportRunKey, PublishingSourceKey, SnapshotKey},
    import_diagnostics::{self, DiagnosticPageLimit, ImportRunStatus, NoIntroDiagnosticOwner},
    logiqx::{LogiqxMode, RecordLocation},
    no_intro_db_xml::NoIntroDatabaseMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const BROKEN_LOGIQX: &[u8] = b"<datafile><!-- \xef\xbf\xbe --></datafile>";

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    document_path: Utf8PathBuf,
    run: ImportRunKey,
    status: CatalogImportStatus,
    snapshot: Option<SnapshotKey>,
}

#[derive(QueryableByName)]
struct TriggerName {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct StorageClasses {
    #[diesel(sql_type = Text)]
    first: String,
    #[diesel(sql_type = Text)]
    second: String,
}

fn fixture(bytes: &[u8], format: CatalogDocumentFormat, name: &str) -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("source.xml"))?;
    std::fs::write(&document_path, bytes)?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document_path.clone(),
            format,
            source_key: PublishingSourceKey::new(format!("evidence-source-{name}")),
            source_display_name: format!("Evidence source {name}"),
            catalog_key: CatalogKey::new(format!("evidence-catalog-{name}")),
            catalog_display_name: format!("Evidence catalog {name}"),
            scope: CatalogScope::Complete,
        },
    )?;
    Ok(Fixture {
        _directory: directory,
        database,
        database_path,
        document_path,
        run: report.run_key,
        status: report.status,
        snapshot: report.snapshot_key,
    })
}

fn logiqx(bytes: &[u8], name: &str) -> TestResult<Fixture> {
    fixture(
        bytes,
        CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
        name,
    )
}

fn no_intro(bytes: &[u8], name: &str) -> TestResult<Fixture> {
    fixture(
        bytes,
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
        name,
    )
}

fn unguarded_connection(fixture: &Fixture, table: &str) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")?;
    let triggers = sql_query(
        "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
    )
    .bind::<Text, _>(table)
    .load::<TriggerName>(&mut connection)?;
    for trigger in triggers {
        let name = trigger.name.replace('"', "\"\"");
        connection.batch_execute(&format!("DROP TRIGGER \"{name}\""))?;
    }
    Ok(connection)
}

fn page(fixture: &Fixture) -> TestResult<import_diagnostics::DiagnosticPage> {
    Ok(import_diagnostics::for_run(
        &fixture.database,
        &fixture.run,
        None,
        DiagnosticPageLimit::new(20)?,
    )?)
}

fn change_diagnostic(fixture: &Fixture, assignment: &str) -> TestResult<usize> {
    let mut connection = unguarded_connection(fixture, "import_diagnostics")?;
    Ok(sql_query(format!(
        "UPDATE import_diagnostics SET {assignment} WHERE run_key=?"
    ))
    .bind::<Text, _>(fixture.run.to_string())
    .execute(&mut connection)?)
}

#[test]
fn pending_and_running_runs_keep_real_import_metadata_without_diagnostics() -> TestResult {
    for (status, expected) in [
        ("pending", ImportRunStatus::Pending),
        ("running", ImportRunStatus::Running),
    ] {
        let fixture = logiqx(b"<datafile/>", status)?;
        assert_eq!(fixture.status, CatalogImportStatus::Succeeded);
        let mut connection = unguarded_connection(&fixture, "import_runs")?;
        let changed = sql_query(
            "UPDATE import_runs SET status=?,snapshot_key=NULL,finished_at=NULL,\
             started_at=CASE WHEN ?='pending' THEN NULL ELSE started_at END WHERE run_key=?",
        )
        .bind::<Text, _>(status)
        .bind::<Text, _>(status)
        .bind::<Text, _>(fixture.run.to_string())
        .execute(&mut connection)?;
        assert_eq!(changed, 1);
        let stored = sql_query(
            "SELECT typeof(status) AS first, typeof(started_at) AS second \
             FROM import_runs WHERE run_key=?",
        )
        .bind::<Text, _>(fixture.run.to_string())
        .get_result::<StorageClasses>(&mut connection)?;
        assert_eq!(stored.first, "text");
        assert_eq!(
            stored.second,
            if status == "pending" { "null" } else { "text" }
        );

        let page = page(&fixture)?;
        assert_eq!(page.run.status, expected);
        assert!(page.diagnostics.is_empty());
        assert!(page.run.summary.is_none());
        assert!(page.run.snapshot.is_none());
        assert_eq!(
            page.run.catalog.display_name,
            format!("Evidence catalog {status}")
        );
        assert_eq!(
            page.run.catalog.source.display_name,
            format!("Evidence source {status}")
        );
        assert_eq!(page.run.interpretation.format, "logiqx");
        assert_eq!(page.run.document.byte_length, Some(11));
    }
    Ok(())
}

#[test]
fn diagnostic_context_preserves_empty_text_and_independent_nullable_coordinates_source_free()
-> TestResult {
    let fixture = logiqx(BROKEN_LOGIQX, "empty-context")?;
    assert_eq!(fixture.status, CatalogImportStatus::Failed);
    assert_eq!(
        change_diagnostic(
            &fixture,
            "field_name='',offending_text='',source_line=7,source_column=NULL,\
         coordinate_view=NULL,column_convention=NULL",
        )?,
        1
    );
    let result = page(&fixture)?;
    let diagnostic = result
        .diagnostics
        .first()
        .ok_or("failed import diagnostic missing")?;
    assert_eq!(diagnostic.field_name.as_deref(), Some(""));
    assert_eq!(diagnostic.offending_text.as_deref(), Some(""));
    assert_eq!(diagnostic.source_line, Some(7));
    assert_eq!(diagnostic.source_column, None);
    assert_eq!(diagnostic.coordinates, None);
    assert_eq!(result.run.status, ImportRunStatus::Failed);
    assert!(result.run.snapshot.is_none());
    std::fs::remove_file(&fixture.document_path)?;
    assert_eq!(page(&fixture)?.diagnostics.len(), 1);
    Ok(())
}

#[test]
fn valid_saved_excerpt_keeps_unknown_anchor_and_independent_context_fields() -> TestResult {
    let fixture = logiqx(BROKEN_LOGIQX, "unknown-anchor")?;
    assert_eq!(fixture.status, CatalogImportStatus::Failed);
    assert_eq!(
        change_diagnostic(
            &fixture,
            "excerpt_start_byte=NULL,source_problem_start_byte=NULL,\
         source_problem_end_byte=NULL,original_problem_start_byte=NULL,\
         original_problem_end_byte=NULL,field_name=NULL,offending_text=NULL,\
         source_line=NULL,source_column=9,coordinate_view=NULL,column_convention=NULL",
        )?,
        1
    );
    let page = page(&fixture)?;
    let diagnostic = page
        .diagnostics
        .first()
        .ok_or("failed import diagnostic missing")?;
    let excerpt = diagnostic.excerpt.as_ref().ok_or("saved excerpt missing")?;
    assert_eq!(excerpt.start_byte(), None);
    assert!(excerpt.problem().is_some());
    assert_eq!(excerpt.source_problem(), None);
    assert_eq!(excerpt.original_problem(), None);
    assert_eq!(diagnostic.field_name, None);
    assert_eq!(diagnostic.offending_text, None);
    assert_eq!(diagnostic.source_line, None);
    assert_eq!(diagnostic.source_column, Some(9));
    assert_eq!(diagnostic.coordinates, None);
    Ok(())
}

#[test]
fn reader_rejects_partial_inconsistent_and_wrong_storage_class_evidence() -> TestResult {
    let partial = logiqx(BROKEN_LOGIQX, "partial-range")?;
    let mut connection = unguarded_connection(&partial, "import_diagnostics")?;
    assert_eq!(
        sql_query("UPDATE import_diagnostics SET problem_end_byte=NULL WHERE run_key=?",)
            .bind::<Text, _>(partial.run.to_string())
            .execute(&mut connection)?,
        1
    );
    let classes = sql_query(
        "SELECT typeof(problem_start_byte) AS first, typeof(problem_end_byte) AS second \
         FROM import_diagnostics WHERE run_key=?",
    )
    .bind::<Text, _>(partial.run.to_string())
    .get_result::<StorageClasses>(&mut connection)?;
    assert_eq!(
        (classes.first.as_str(), classes.second.as_str()),
        ("integer", "null")
    );
    assert!(
        import_diagnostics::for_run(
            &partial.database,
            &partial.run,
            None,
            DiagnosticPageLimit::new(20)?
        )
        .is_err()
    );

    let inconsistent = logiqx(BROKEN_LOGIQX, "inconsistent-range")?;
    assert_eq!(
        change_diagnostic(
            &inconsistent,
            "source_problem_start_byte=source_problem_start_byte+1,\
         source_problem_end_byte=source_problem_end_byte+1",
        )?,
        1
    );
    assert!(
        import_diagnostics::for_run(
            &inconsistent.database,
            &inconsistent.run,
            None,
            DiagnosticPageLimit::new(20)?
        )
        .is_err()
    );

    let wrong_class = logiqx(BROKEN_LOGIQX, "wrong-class")?;
    let mut connection = unguarded_connection(&wrong_class, "import_diagnostics")?;
    assert_eq!(
        sql_query("UPDATE import_diagnostics SET code=CAST(code AS BLOB) WHERE run_key=?",)
            .bind::<Text, _>(wrong_class.run.to_string())
            .execute(&mut connection)?,
        1
    );
    let stored = sql_query(
        "SELECT typeof(code) AS first, typeof(message) AS second \
         FROM import_diagnostics WHERE run_key=?",
    )
    .bind::<Text, _>(wrong_class.run.to_string())
    .get_result::<StorageClasses>(&mut connection)?;
    assert_eq!(
        (stored.first.as_str(), stored.second.as_str()),
        ("blob", "text")
    );
    assert!(
        import_diagnostics::for_run(
            &wrong_class.database,
            &wrong_class.run,
            None,
            DiagnosticPageLimit::new(20)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn diagnostic_query_returns_each_explicitly_related_native_owner() -> TestResult {
    let xml = b"<datafile><game name='g'><source><details comment1='before\0after'/></source></game></datafile>";
    let fixture = no_intro(xml, "source-details")?;
    assert_eq!(fixture.status, CatalogImportStatus::Succeeded);
    let initial = page(&fixture)?;
    let diagnostic = initial
        .diagnostics
        .first()
        .ok_or("NUL diagnostic missing")?;
    // Automatic recovery links only the most specific element. The schema
    // also permits explicitly relating that diagnostic to another real owner.
    assert!(matches!(
        diagnostic.owners.as_slice(),
        [NoIntroDiagnosticOwner::DumpDetails { .. }]
    ));
    let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=ON")?;
    assert_eq!(
        sql_query(
            "INSERT INTO no_intro_dump_source_diagnostics \
             (diagnostic_key,run_key,snapshot_key,dump_source_id) \
             SELECT diagnostic_key,run_key,snapshot_key,dump_source_id \
             FROM no_intro_dump_details_diagnostics WHERE diagnostic_key=?",
        )
        .bind::<Text, _>(diagnostic.key.as_str())
        .execute(&mut connection)?,
        1
    );
    let page = page(&fixture)?;
    assert_eq!(page.run.status, ImportRunStatus::Succeeded);
    let diagnostic = page.diagnostics.first().ok_or("NUL diagnostic missing")?;
    assert_eq!(page.diagnostics.len(), 1);
    assert_eq!(diagnostic.owners.len(), 2);
    assert_eq!(
        page.run.snapshot.as_ref().map(|snapshot| &snapshot.key),
        fixture.snapshot.as_ref()
    );
    let excerpt = diagnostic
        .excerpt
        .as_ref()
        .ok_or("saved NUL excerpt missing")?;
    assert_eq!(excerpt.view(), ExcerptView::RetainedOriginalBytes);
    assert!(excerpt.bytes().contains(&0));
    let problem = excerpt.problem().ok_or("NUL highlight missing")?;
    assert_eq!(problem.end(), problem.start() + 1);
    assert_eq!(
        excerpt.bytes().get(problem.start()..problem.end()),
        Some(&b"\0"[..])
    );
    let source_problem = excerpt.source_problem().ok_or("source range missing")?;
    assert_eq!(excerpt.original_problem(), Some(source_problem));
    assert_eq!(
        excerpt
            .start_byte()
            .and_then(|anchor| anchor.checked_add(problem.start())),
        Some(source_problem.start())
    );
    let nul_offset = xml
        .iter()
        .position(|byte| *byte == 0)
        .ok_or("fixture NUL missing")?;
    assert_eq!(source_problem.start(), nul_offset);
    let source_id = diagnostic
        .owners
        .iter()
        .find_map(|owner| match owner {
            NoIntroDiagnosticOwner::DumpSource { id, .. } => Some(*id),
            _ => None,
        })
        .ok_or("dump source owner missing")?;
    let details_id = diagnostic
        .owners
        .iter()
        .find_map(|owner| match owner {
            NoIntroDiagnosticOwner::DumpDetails { id, .. } => Some(*id),
            _ => None,
        })
        .ok_or("dump details owner missing")?;
    assert_eq!(source_id, details_id);
    let location = RecordLocation {
        line: diagnostic.source_line.ok_or("warning line missing")?,
        column: diagnostic.source_column.ok_or("warning column missing")?,
    };
    assert!(diagnostic.owners.iter().all(|owner| match owner {
        NoIntroDiagnosticOwner::DumpSource { extent, .. }
        | NoIntroDiagnosticOwner::DumpDetails { extent, .. } => extent.contains(location),
        _ => false,
    }));
    assert_eq!(diagnostic.source_line, Some(1));
    assert!(diagnostic.source_column.is_some_and(|column| column > 0));
    Ok(())
}
