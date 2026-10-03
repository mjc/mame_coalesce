use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Integer, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = Integer)]
    field_kind: i32,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct SetIdRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
}

fn import(
    database: &Database,
    directory: &tempfile::TempDir,
    contents: &str,
) -> Result<SnapshotKey, Box<dyn std::error::Error>> {
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("positions.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::Logiqx(
                mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
            ),
            source_key: PublishingSourceKey::new("logiqx-text-positions"),
            source_display_name: "Logiqx text positions".to_owned(),
            catalog_key: CatalogKey::new("logiqx-text-positions"),
            catalog_display_name: "Logiqx text positions".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    report
        .snapshot_key
        .ok_or_else(|| "Logiqx import did not produce a snapshot".into())
}

fn positions(
    conn: &mut SqliteConnection,
    query: &str,
    owner_id: &str,
) -> Result<Vec<PositionRow>, diesel::result::Error> {
    sql_query(query).bind::<Text, _>(owner_id).load(conn)
}

#[test]
fn import_tracks_every_text_field_when_source_order_moves() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let previous = import(
        &database,
        &directory,
        "<datafile>\n  <header>\n    <name>Catalog</name>\n    <description>About</description>\n    <category>Games</category>\n    <version>1</version>\n    <date>2024</date>\n    <author>Author</author>\n    <email>a@example.test</email>\n    <homepage>Home</homepage>\n    <url>URL</url>\n    <comment>Note</comment>\n  </header>\n  <game name='set'>\n    <description>Game</description>\n    <year>1990</year>\n    <manufacturer>Maker</manufacturer>\n  </game>\n</datafile>",
    )?;
    let current = import(
        &database,
        &directory,
        "<datafile>\n  <header>\n    <vendor-field/>\n    <name>Catalog</name>\n    <description></description>\n    <category>Games</category>\n    <version>2</version>\n    <date>2025</date>\n    <author>Author</author>\n    <email>a@example.test</email>\n    <homepage>Home</homepage>\n    <url>URL</url>\n    <comment>Note</comment>\n  </header>\n  <game name='set'>\n    <vendor-field/>\n    <year>1990</year>\n    <description>Game</description>\n    <manufacturer>Maker</manufacturer>\n  </game>\n</datafile>",
    )?;
    let mut conn = SqliteConnection::establish(database_path.as_str())?;
    let set = sql_query(
        "SELECT catalog_sets.set_id AS set_id FROM catalog_sets \
         JOIN catalog_set_groups USING (set_group_id) \
         WHERE snapshot_key = ? AND set_name = 'set'",
    )
    .bind::<Text, _>(current.as_str())
    .get_result::<SetIdRow>(&mut conn)?;

    let previous_header = positions(
        &mut conn,
        "SELECT field_kind, source_order, source_line, source_column \
         FROM logiqx_header_text_positions WHERE snapshot_key = ? ORDER BY field_kind",
        previous.as_str(),
    )?;
    assert_eq!(
        previous_header
            .iter()
            .map(|row| (row.field_kind, row.source_order))
            .collect::<Vec<_>>(),
        (0..10)
            .map(|kind| (kind, i64::from(kind)))
            .collect::<Vec<_>>()
    );

    let header = positions(
        &mut conn,
        "SELECT field_kind, source_order, source_line, source_column \
         FROM logiqx_header_text_positions WHERE snapshot_key = ? ORDER BY field_kind",
        current.as_str(),
    )?;
    assert_eq!(
        header
            .iter()
            .map(|row| (
                row.field_kind,
                row.source_order,
                row.source_line,
                row.source_column
            ))
            .collect::<Vec<_>>(),
        (0..10)
            .map(|kind| (kind, i64::from(kind) + 1, i64::from(kind) + 4, 5))
            .collect::<Vec<_>>()
    );

    let game = sql_query(
        "SELECT field_kind, source_order, source_line, source_column \
         FROM logiqx_game_text_positions WHERE set_id = ? ORDER BY field_kind",
    )
    .bind::<BigInt, _>(set.set_id)
    .load::<PositionRow>(&mut conn)?;
    assert_eq!(
        game.iter()
            .map(|row| (
                row.field_kind,
                row.source_order,
                row.source_line,
                row.source_column
            ))
            .collect::<Vec<_>>(),
        vec![(0, 2, 18, 5), (1, 1, 17, 5), (2, 3, 19, 5)]
    );
    Ok(())
}
