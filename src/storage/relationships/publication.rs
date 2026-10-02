use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::RelationshipAssertionKey;

const LOOKUP_SQL: &str = concat!(
    "WITH requested(assertion_key) AS (VALUES (?)) ",
    include_str!("../db/relationship_readiness.sql")
);

#[derive(QueryableByName)]
struct Found {
    #[diesel(sql_type = BigInt, column_name = is_published)]
    found: i64,
}

pub(super) fn is_published(
    conn: &mut SqliteConnection,
    key: &RelationshipAssertionKey,
) -> crate::Result<bool> {
    Ok(sql_query(LOOKUP_SQL)
        .bind::<Text, _>(key.as_str())
        .get_result::<Found>(conn)?
        .found
        == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(QueryableByName)]
    struct Plan {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    #[test]
    fn published_key_lookups_seek_native_owners_instead_of_scanning_aliases() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let mut conn = database.pool().get()?;
        for key in [
            "generic-key",
            "mame-dependency:123:4",
            "no-intro-dat-cloneof:123",
        ] {
            let plans = sql_query(format!("EXPLAIN QUERY PLAN {LOOKUP_SQL}"))
                .bind::<Text, _>(key)
                .load::<Plan>(&mut conn)?;
            let scans = plans
                .into_iter()
                .map(|row| row.detail)
                .filter(|detail| {
                    [
                        "SCAN dependency",
                        "SCAN position",
                        "SCAN game",
                        "SCAN assertion",
                        "SCAN relationship_assertions",
                    ]
                    .iter()
                    .any(|prefix| detail.starts_with(prefix))
                })
                .collect::<Vec<_>>();
            assert!(
                scans.is_empty(),
                "published key {key} scans unrelated aliases: {scans:?}"
            );
        }
        Ok(())
    }
}
