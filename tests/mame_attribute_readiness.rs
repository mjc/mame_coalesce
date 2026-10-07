//! Check the production set comparison independently of native view construction.
use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const READINESS: &str = concat!(
    "WITH requested(snapshot_key) AS (VALUES (?)), ",
    include_str!("../src/storage/db/mame_attribute_readiness.sql")
);

#[derive(QueryableByName)]
struct Invalid {
    #[diesel(sql_type = BigInt)]
    invalid: i64,
}

#[derive(QueryableByName)]
struct Plan {
    #[diesel(sql_type = Text)]
    detail: String,
}

fn connection() -> TestResult<SqliteConnection> {
    let mut conn = SqliteConnection::establish(":memory:")?;
    conn.batch_execute(
        "CREATE TABLE mame_expected_attribute_positions (
         snapshot_key TEXT, family INTEGER, owner_a INTEGER, owner_b INTEGER,
         owner_c INTEGER, owner_d INTEGER, field_kind INTEGER);
         CREATE TABLE mame_actual_attribute_positions AS
         SELECT * FROM mame_expected_attribute_positions WHERE 0;
         CREATE TABLE mame_attribute_ordinal_collisions(snapshot_key TEXT);",
    )?;
    Ok(conn)
}

fn invalid(conn: &mut SqliteConnection, snapshot: Option<&str>) -> TestResult<bool> {
    Ok(sql_query(READINESS)
        .bind::<Nullable<Text>, _>(snapshot)
        .get_result::<Invalid>(conn)
        .optional()?
        .is_some_and(|row| row.invalid != 0))
}

#[test]
fn presence_comparison_uses_one_sort_without_materializing_two_copies() -> TestResult {
    let mut conn = connection()?;
    let plans = sql_query(format!("EXPLAIN QUERY PLAN {READINESS}"))
        .bind::<Text, _>("selected")
        .load::<Plan>(&mut conn)?;
    assert!(
        !plans
            .iter()
            .any(|row| row.detail == "MATERIALIZE expected" || row.detail == "MATERIALIZE actual"),
        "presence comparison must stream both sides into one grouped comparison"
    );
    assert_eq!(
        plans
            .iter()
            .filter(|row| row.detail.contains("USE TEMP B-TREE FOR GROUP BY"))
            .count(),
        1
    );
    assert!(
        !plans
            .iter()
            .any(|row| row.detail.contains("EXCEPT USING TEMP B-TREE"))
    );
    Ok(())
}

fn seed(conn: &mut SqliteConnection, table: &'static str, mask: u32) -> TestResult {
    // Distinguish family, owner, field, and malformed sentinel; include a NULL
    // key to retain SQLite's set equality semantics for corrupt storage too.
    let keys = [
        (0, Some(1), 0),
        (1, Some(1), 0),
        (1, Some(2), 0),
        (1, Some(1), -1),
        (1, None, 0),
    ];
    for (index, (family, owner, field)) in keys.into_iter().enumerate() {
        if mask & (1 << index) != 0 {
            // Duplicate witnesses must retain set semantics, not count equality.
            for _ in 0..2 {
                sql_query(format!(
                    "INSERT INTO {table} VALUES ('selected', ?, ?, 0, 0, 0, ?)"
                ))
                .bind::<BigInt, _>(family)
                .bind::<Nullable<BigInt>, _>(owner)
                .bind::<BigInt, _>(field)
                .execute(conn)?;
            }
        }
    }
    Ok(())
}

#[test]
fn grouped_presence_retains_symmetric_difference_and_snapshot_isolation() -> TestResult {
    let mut conn = connection()?;
    for expected in 0..32 {
        for actual in 0..32 {
            conn.batch_execute("DELETE FROM mame_expected_attribute_positions;
                DELETE FROM mame_actual_attribute_positions;
                INSERT INTO mame_actual_attribute_positions VALUES ('unrelated', 9, 99, 0, 0, 0, 0);")?;
            seed(&mut conn, "mame_expected_attribute_positions", expected)?;
            seed(&mut conn, "mame_actual_attribute_positions", actual)?;
            assert_eq!(
                invalid(&mut conn, Some("selected"))?,
                expected != actual,
                "expected mask {expected}, actual mask {actual}"
            );
        }
    }
    assert!(!invalid(&mut conn, Some("absent"))?);
    assert!(!invalid(&mut conn, None)?);
    Ok(())
}

#[test]
fn ordinal_collisions_still_reject_an_otherwise_equal_snapshot() -> TestResult {
    let mut conn = connection()?;
    seed(&mut conn, "mame_expected_attribute_positions", 7)?;
    seed(&mut conn, "mame_actual_attribute_positions", 7)?;
    conn.batch_execute("INSERT INTO mame_attribute_ordinal_collisions VALUES ('unrelated')")?;
    assert!(!invalid(&mut conn, Some("selected"))?);
    conn.batch_execute("INSERT INTO mame_attribute_ordinal_collisions VALUES ('selected')")?;
    assert!(invalid(&mut conn, Some("selected"))?);
    Ok(())
}

#[test]
fn duplicate_counts_do_not_change_presence_but_every_owner_coordinate_does() -> TestResult {
    let mut conn = connection()?;
    conn.batch_execute(
        "INSERT INTO mame_expected_attribute_positions VALUES ('selected', 1, 2, 3, 4, 5, 6);
         INSERT INTO mame_actual_attribute_positions
         SELECT * FROM mame_expected_attribute_positions;
         INSERT INTO mame_actual_attribute_positions
         SELECT * FROM mame_expected_attribute_positions;",
    )?;
    assert!(!invalid(&mut conn, Some("selected"))?);
    for column in [
        "family",
        "owner_a",
        "owner_b",
        "owner_c",
        "owner_d",
        "field_kind",
    ] {
        conn.batch_execute(&format!(
            "UPDATE mame_actual_attribute_positions SET {column}={column}+1"
        ))?;
        assert!(invalid(&mut conn, Some("selected"))?, "coordinate {column}");
        conn.batch_execute(&format!(
            "UPDATE mame_actual_attribute_positions SET {column}={column}-1"
        ))?;
        assert!(!invalid(&mut conn, Some("selected"))?);
    }
    Ok(())
}
