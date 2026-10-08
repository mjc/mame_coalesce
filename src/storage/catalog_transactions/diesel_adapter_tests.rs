#![allow(clippy::expect_used)]

use diesel::{
    Connection, RunQueryDsl, connection::SimpleConnection, sql_query, sql_types::BigInt,
    sqlite::SqliteConnection,
};

use super::{DieselTransactionDriver, TransactionFailure, TransactionOutcome, run_with_driver};

#[derive(diesel::QueryableByName)]
struct RowCount {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(Debug, PartialEq, Eq)]
enum BodyFailure {
    Deliberate,
}

fn initialize_probe_table(conn: &mut SqliteConnection) {
    conn.batch_execute(
        "CREATE TABLE transaction_probe (value TEXT NOT NULL);
         INSERT INTO transaction_probe VALUES ('preexisting');",
    )
    .expect("create probe table and baseline row");
}

fn connection_with_probe_table() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").expect("in-memory SQLite");
    initialize_probe_table(&mut conn);
    conn
}

fn row_count(conn: &mut SqliteConnection) -> diesel::QueryResult<i64> {
    sql_query("SELECT COUNT(*) AS count FROM transaction_probe")
        .get_result::<RowCount>(conn)
        .map(|row| row.count)
}

#[test]
fn diesel_adapter_commits_body_rows_durably() {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let path = directory.path().join("committed.sqlite");
    let path = path.to_str().expect("UTF-8 temporary database path");
    let mut conn = SqliteConnection::establish(path).expect("file-backed SQLite");
    initialize_probe_table(&mut conn);
    let mut driver = DieselTransactionDriver;

    let outcome = run_with_driver(&mut conn, &mut driver, |conn, _run_key| {
        sql_query("INSERT INTO transaction_probe VALUES ('committed')").execute(conn)?;
        Ok::<_, diesel::result::Error>(())
    });

    assert!(matches!(outcome, TransactionOutcome::Committed { .. }));
    drop(conn);
    let mut reopened = SqliteConnection::establish(path).expect("reopen committed database");
    assert_eq!(row_count(&mut reopened).expect("query persisted rows"), 2);
}

#[test]
fn diesel_adapter_rolls_back_all_body_rows_after_failure() {
    let mut conn = connection_with_probe_table();
    let mut driver = DieselTransactionDriver;

    let outcome = run_with_driver(&mut conn, &mut driver, |conn, _run_key| {
        sql_query("INSERT INTO transaction_probe VALUES ('prefix')")
            .execute(conn)
            .expect("insert first row before deliberate failure");
        sql_query("INSERT INTO transaction_probe VALUES ('failing row')")
            .execute(conn)
            .expect("insert second row before deliberate failure");
        Err::<(), _>(BodyFailure::Deliberate)
    });

    assert!(matches!(
        outcome,
        TransactionOutcome::ConfirmedRollback {
            primary: TransactionFailure::Operation(BodyFailure::Deliberate),
            ..
        }
    ));
    assert_eq!(row_count(&mut conn).expect("query rows after rollback"), 1);
}

#[test]
fn diesel_adapter_refuses_outer_transaction_without_rolling_it_back() {
    let mut conn = connection_with_probe_table();
    let mut driver = DieselTransactionDriver;

    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        sql_query("INSERT INTO transaction_probe VALUES ('outer')").execute(conn)?;

        let outcome = run_with_driver(conn, &mut driver, |conn, _run_key| {
            sql_query("INSERT INTO transaction_probe VALUES ('nested import')").execute(conn)?;
            Ok::<_, diesel::result::Error>(())
        });

        assert!(matches!(
            outcome,
            TransactionOutcome::Unresolved {
                primary: TransactionFailure::Database(diesel::result::Error::AlreadyInTransaction),
                ..
            }
        ));
        assert_eq!(row_count(conn)?, 2, "outer transaction remains active");
        Ok(())
    })
    .expect("outer transaction should still commit");

    assert_eq!(row_count(&mut conn).expect("query durable outer row"), 2);
}
