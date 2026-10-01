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
