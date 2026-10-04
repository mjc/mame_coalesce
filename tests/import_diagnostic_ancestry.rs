use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, ImportRunKey, PublishingSourceKey},
    import_diagnostics::{self, DiagnosticPageLimit},
    no_intro_db_xml::NoIntroDatabaseMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    path: Utf8PathBuf,
    run: ImportRunKey,
}

fn fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(
        &document_path,
        b"<datafile><game name='game'><source><file forcename='source\0'/></source><release><file forcename='release\0'/></release></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDatabase(
                NoIntroDatabaseMode::NullRecoveryCompatible,
            ),
            source_key: PublishingSourceKey::new("ancestry"),
            source_display_name: "Ancestry".into(),
            catalog_key: CatalogKey::new("ancestry"),
            catalog_display_name: "Ancestry".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let valid = import_diagnostics::for_run(
        &database,
        &report.run_key,
        None,
        DiagnosticPageLimit::new(10)?,
    )?;
    assert_eq!(valid.diagnostics.len(), 2);
    assert!(
        valid
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.owners.len() == 1)
    );
    Ok(Fixture {
        _directory: directory,
        database,
        path,
        run: report.run_key,
    })
}

#[derive(QueryableByName)]
struct Trigger {
    #[diesel(sql_type = Text)]
    name: String,
}

fn inject_corruption(fixture: &Fixture, table: &str, mutation: &str) -> TestResult {
    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")?;
    connection.transaction(|connection| {
        let triggers =
            sql_query("SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=?")
                .bind::<Text, _>(table)
                .load::<Trigger>(connection)?;
        for trigger in triggers {
            let quoted_name = trigger.name.replace('"', "\"\"");
            connection.batch_execute(&format!("DROP TRIGGER \"{quoted_name}\""))?;
        }
        assert_eq!(
            sql_query(mutation).execute(connection)?,
            1,
            "corruption witness must modify its actual native owner"
        );
        Ok(())
    })
}

#[test]
fn file_diagnostics_require_actual_history_parents() -> TestResult {
    for (table, mutation) in [
        (
            "no_intro_dump_files",
            "UPDATE no_intro_dump_files SET dump_source_id=999",
        ),
        (
            "no_intro_release_files",
            "UPDATE no_intro_release_files SET release_id=999",
        ),
        (
            "no_intro_release_files",
            "UPDATE no_intro_release_files SET set_id=999",
        ),
        (
            "no_intro_database_games",
            "DELETE FROM no_intro_database_games",
        ),
    ] {
        let fixture = fixture()?;
        inject_corruption(&fixture, table, mutation)?;
        assert!(
            import_diagnostics::for_run(
                &fixture.database,
                &fixture.run,
                None,
                DiagnosticPageLimit::new(10)?
            )
            .is_err(),
            "diagnostic reader accepted invalid ancestry: {mutation}",
        );
    }
    Ok(())
}

#[test]
fn file_diagnostics_require_checked_containing_ancestor_extents() -> TestResult {
    for (table, mutation) in [
        (
            "no_intro_dump_sources",
            "UPDATE no_intro_dump_sources SET source_end_column=source_column+1",
        ),
        (
            "no_intro_releases",
            "UPDATE no_intro_releases SET source_end_column=source_column+1",
        ),
        (
            "no_intro_release_files",
            "UPDATE no_intro_release_files SET source_end_column=(SELECT source_end_column+1 FROM no_intro_releases)",
        ),
        (
            "no_intro_exports",
            "UPDATE no_intro_exports SET document_end_column=2",
        ),
        (
            "no_intro_releases",
            "UPDATE no_intro_releases SET source_end_line=x'31'",
        ),
        (
            "no_intro_dump_sources",
            "UPDATE no_intro_dump_sources SET source_order=0.5",
        ),
    ] {
        let fixture = fixture()?;
        inject_corruption(&fixture, table, mutation)?;
        assert!(
            import_diagnostics::for_run(
                &fixture.database,
                &fixture.run,
                None,
                DiagnosticPageLimit::new(10)?
            )
            .is_err(),
            "diagnostic reader accepted invalid ancestor extent: {mutation}",
        );
    }
    Ok(())
}
