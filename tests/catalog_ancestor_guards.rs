use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::database::Database;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct TriggerSql {
    #[diesel(sql_type=Text)]
    sql: String,
}

#[derive(QueryableByName)]
struct PlanDetail {
    #[diesel(sql_type=Text)]
    detail: String,
}

#[test]
fn logiqx_publication_checks_seek_the_requested_snapshot() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&path)?);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let trigger = sql_query(
        "SELECT sql FROM sqlite_schema WHERE name='logiqx_attribute_positions_publication_guard'",
    )
    .get_result::<TriggerSql>(&mut connection)?
    .sql;
    let predicate = trigger
        .split_once("\nWHEN ")
        .ok_or("publication guard has no predicate")?
        .1
        .rsplit_once("\nBEGIN ")
        .ok_or("publication guard has no body")?
        .0
        .replace("NEW.snapshot_key", "'requested-snapshot'");
    let details = sql_query(format!("EXPLAIN QUERY PLAN SELECT 1 WHERE {predicate}"))
        .load::<PlanDetail>(&mut connection)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    for detail in &details {
        assert!(
            ![
                "SCAN owner",
                "SCAN occurrence",
                "SCAN groups",
                "SCAN sets",
                "SCAN logiqx_"
            ]
            .iter()
            .any(|scan| detail.contains(scan)),
            "publication must not scan other snapshots: {details:#?}"
        );
    }
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with("SEARCH groups") && detail.contains("snapshot_key=?")),
        "publication must seek requested snapshot groups: {details:#?}"
    );
    Ok(())
}

#[test]
fn stable_catalog_ancestors_cannot_be_deleted_rebound_or_replaced() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&path)?);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute(
        "PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;
         INSERT INTO publishing_sources(source_key,display_name) VALUES('publisher','Publisher');
         INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES('catalog','publisher','Catalog');
         INSERT INTO documents(document_key) VALUES('unavailable-document');
         INSERT INTO parser_interpretations(interpretation_key,format) VALUES('parser','logiqx');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES(1,'complete');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
           VALUES('snapshot','catalog','unavailable-document','parser',1);
         INSERT INTO publishing_sources(source_key,display_name) VALUES('other-publisher','Other');",
    )?;
    let probes = [
        "DELETE FROM publishing_sources WHERE source_key='publisher'",
        "UPDATE publishing_sources SET source_key='rebound' WHERE source_key='publisher'",
        "INSERT OR REPLACE INTO publishing_sources(source_key,display_name) VALUES('publisher','Replacement')",
        "DELETE FROM catalogs WHERE catalog_key='catalog'",
        "UPDATE catalogs SET catalog_key='rebound' WHERE catalog_key='catalog'",
        "UPDATE catalogs SET source_key='other-publisher' WHERE catalog_key='catalog'",
        "INSERT OR REPLACE INTO catalogs(catalog_key,source_key,display_name) VALUES('catalog','publisher','Replacement')",
        "UPDATE documents SET document_key='rebound' WHERE document_key='unavailable-document'",
        "INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES('orphan','missing','Orphan')",
    ];
    let mut accepted = Vec::new();
    for probe in probes {
        connection.batch_execute("SAVEPOINT probe")?;
        if connection.batch_execute(probe).is_ok() {
            accepted.push(probe);
        }
        connection.batch_execute("ROLLBACK TO probe; RELEASE probe")?;
    }
    assert!(
        accepted.is_empty(),
        "accepted identity corruption: {accepted:?}"
    );
    // Labels and the first retention of an unavailable document remain editable.
    connection.batch_execute(
        "UPDATE publishing_sources SET display_name='Renamed' WHERE source_key='publisher';
         UPDATE catalogs SET display_name='Renamed' WHERE catalog_key='catalog';
         UPDATE documents SET format_hint='logiqx+xml' WHERE document_key='unavailable-document';",
    )?;
    Ok(())
}
