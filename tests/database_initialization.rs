#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::BigInt,
};
use mame_coalesce::database::Database;
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn count(conn: &mut SqliteConnection, query: &str) -> i64 {
    sql_query(query)
        .get_result::<Count>(conn)
        .expect("query count")
        .count
}

#[test]
fn fresh_database_uses_the_approved_native_catalog_model() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite")).expect("UTF-8 path");
    let _database = Database::open(&path).expect("create greenfield database");
    let mut connection =
        SqliteConnection::establish(path.as_str()).expect("inspect greenfield database");
    for table in [
        "catalog_publishers",
        "catalog_source_files",
        "catalog_fetch_attempts",
        "catalog_file_receipts",
        "catalog_reading_rules",
        "catalog_file_byte_contracts",
        "catalog_xml_repairs",
        "catalog_editions",
        "published_catalog_editions",
        "catalog_imports",
        "catalog_source_elements",
        "catalog_media_entries",
        "shared_catalog_files",
        "shared_file_sizes",
        "shared_file_hashes",
        "mame_documents",
        "software_documents",
        "logiqx_documents",
        "clrmamepro_documents",
        "no_intro_dat_documents",
        "no_intro_export_documents",
    ] {
        assert_eq!(
            count(
                &mut connection,
                &format!(
                    "SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name='{table}'"
                )
            ),
            1,
            "approved table missing: {table}"
        );
    }
    for table in [
        "catalog_snapshots",
        "asset_occurrences",
        "parser_interpretations",
        "documents",
        "catalog_contents",
        "__diesel_schema_migrations",
    ] {
        assert_eq!(
            count(
                &mut connection,
                &format!(
                    "SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name='{table}'"
                )
            ),
            0,
            "obsolete catalog table survives: {table}"
        );
    }
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM file_id_registries WHERE registry_id=1 AND typeof(registry_uuid)='blob' AND length(registry_uuid)=16"
        ),
        1
    );
}

#[test]
fn new_database_is_created_directly_without_migration_history() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite")).expect("UTF-8 path");
    let database = Database::open(&path).expect("create database");
    let mut connection = SqliteConnection::establish(path.as_str()).expect("inspect database");
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM sqlite_schema WHERE name = '__diesel_schema_migrations'"
        ),
        0
    );
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM database_schema"
        ),
        1
    );
    drop(database);
    let reopened = Database::open(&path).expect("reopen current database");
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM database_schema"
        ),
        1
    );
    drop(reopened);
}

#[test]
fn an_existing_unrecognized_database_is_not_converted_or_erased() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("other.sqlite")).expect("UTF-8 path");
    let mut connection =
        SqliteConnection::establish(path.as_str()).expect("create foreign database");
    connection
        .batch_execute("CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES ('keep');")
        .expect("unrelated data");
    assert!(
        Database::open(&path).is_err(),
        "greenfield startup must not install an upgrade path"
    );
    assert_eq!(
        count(&mut connection, "SELECT count(*) AS count FROM unrelated"),
        1
    );
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM sqlite_schema WHERE name = 'database_schema'"
        ),
        0
    );
}

#[test]
fn a_changed_schema_is_not_silently_repaired() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite")).expect("UTF-8 path");
    drop(Database::open(&path).expect("create database"));
    let mut connection = SqliteConnection::establish(path.as_str()).expect("alter database");
    connection
        .batch_execute("CREATE TABLE sqlitex_unexpected(value TEXT)")
        .expect("unexpected schema");
    assert!(Database::open(&path).is_err());
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM sqlite_schema WHERE name = 'sqlitex_unexpected'"
        ),
        1
    );
}

#[test]
fn a_different_schema_fingerprint_is_rejected_without_upgrade() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite")).expect("UTF-8 path");
    drop(Database::open(&path).expect("create database"));
    let mut connection = SqliteConnection::establish(path.as_str()).expect("inspect database");
    connection
        .batch_execute("UPDATE database_schema SET schema_digest=zeroblob(32)")
        .expect("substitute prior schema fingerprint");
    assert!(Database::open(&path).is_err());
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM database_schema WHERE schema_digest=zeroblob(32)"
        ),
        1
    );
    assert_eq!(
        count(
            &mut connection,
            "SELECT count(*) AS count FROM sqlite_schema WHERE name='__diesel_schema_migrations'"
        ),
        0
    );
}
