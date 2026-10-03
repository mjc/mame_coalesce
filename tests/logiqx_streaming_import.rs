use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BEFORE_EOF_FAULT: &str = "logiqx-first-game-write-before-eof";
const LATE_EOF: &str = "<datafile><game name='first'><rom name='first.rom' size='1' sha1='2222222222222222222222222222222222222222'/></game><game name='unfinished'>";

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        Ok(Self {
            database: Database::open(&path)?,
            connection: SqliteConnection::establish(path.as_str())?,
            directory,
        })
    }

    fn import(&self, name: &str, xml: &str) -> TestResult<app::CatalogImportReport> {
        let document_path = Utf8PathBuf::try_from(self.directory.path().join(name))?;
        std::fs::write(&document_path, xml)?;
        Ok(app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::Logiqx(
                    mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
                ),
                source_key: PublishingSourceKey::new("streaming-logiqx-source"),
                source_display_name: "Streaming Logiqx source".into(),
                catalog_key: CatalogKey::new("streaming-logiqx-catalog"),
                catalog_display_name: "Streaming Logiqx catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?)
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn count(connection: &mut SqliteConnection, table: &str) -> TestResult<i64> {
    Ok(sql_query(format!("SELECT count(*) AS count FROM {table}"))
        .get_result::<Count>(connection)?
        .count)
}

#[test]
fn native_game_writes_happen_before_the_reader_reaches_late_eof() -> TestResult {
    let mut fixture = Fixture::new()?;
    sql_query(format!(
        "CREATE TRIGGER first_game_streaming_fault BEFORE INSERT ON logiqx_games \
         BEGIN SELECT RAISE(ABORT, '{BEFORE_EOF_FAULT}'); END"
    ))
    .execute(&mut fixture.connection)?;

    let result = fixture.import("unfinished.dat", LATE_EOF);
    let error = result.err().ok_or(
        "the reader reached malformed EOF before trying to persist its completed first game",
    )?;
    assert!(error.to_string().contains(BEFORE_EOF_FAULT), "{error}");
    for table in [
        "catalog_snapshots",
        "catalog_sets",
        "logiqx_games",
        "snapshot_publications",
    ] {
        assert_eq!(count(&mut fixture.connection, table)?, 0, "{table}");
    }
    Ok(())
}

#[test]
fn late_eof_rolls_back_completed_native_games_and_file_identity() -> TestResult {
    let mut fixture = Fixture::new()?;
    let before = fixture.import(
        "published.dat",
        "<datafile><game name='published'><rom name='old.rom' size='1' sha1='1111111111111111111111111111111111111111'/></game></datafile>",
    )?;
    assert_eq!(before.status, CatalogImportStatus::Succeeded);
    let tables = [
        "catalog_snapshots",
        "catalog_set_groups",
        "catalog_sets",
        "asset_occurrences",
        "catalog_contents",
        "digest_values",
        "occurrence_content_conflicts",
        "occurrence_content_conflict_hashes",
        "occurrence_content_conflict_sizes",
        "logiqx_games",
        "logiqx_rom_claims",
        "logiqx_game_attribute_positions",
        "logiqx_rom_attribute_positions",
        "occurrence_digest_assertions",
        "snapshot_publications",
    ];
    let before_counts = tables
        .iter()
        .map(|table| count(&mut fixture.connection, table))
        .collect::<TestResult<Vec<_>>>()?;

    let failed = fixture.import("unfinished.dat", LATE_EOF)?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(failed.diagnostic_count, 1);
    for (table, before_count) in tables.iter().zip(before_counts) {
        assert_eq!(
            count(&mut fixture.connection, table)?,
            before_count,
            "{table}"
        );
    }
    let history = app::catalog_snapshot_history(
        &fixture.database,
        &CatalogKey::new("streaming-logiqx-catalog"),
    )?;
    assert_eq!(history.len(), 1);
    assert_eq!(Some(&history[0].snapshot), before.snapshot_key.as_ref());
    assert_eq!(count(&mut fixture.connection, "documents")?, 2);
    assert_eq!(count(&mut fixture.connection, "import_runs")?, 2);
    Ok(())
}
