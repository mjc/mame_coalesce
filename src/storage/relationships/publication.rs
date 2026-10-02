use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::RelationshipId;
use crate::domain::RelationshipAssertionKey;

const LOOKUP_SQL: &str = concat!(
    "WITH requested(assertion_key) AS (VALUES (?)) ",
    include_str!("../db/relationship_readiness.sql")
);

#[derive(QueryableByName)]
struct Found {
    #[diesel(sql_type = Nullable<BigInt>)]
    relationship_id: Option<i64>,
    #[diesel(sql_type = BigInt, column_name = is_published)]
    found: i64,
}

pub(super) fn published_id(
    conn: &mut SqliteConnection,
    key: &RelationshipAssertionKey,
) -> crate::Result<Option<RelationshipId>> {
    let row = sql_query(LOOKUP_SQL)
        .bind::<Text, _>(key.as_str())
        .get_result::<Found>(conn)?;
    Ok(row
        .relationship_id
        .filter(|_| row.found == 1)
        .map(RelationshipId))
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
            "registered-source-key",
            "missing-reported-dat-parent",
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
                        "SCAN registry",
                        "SCAN link",
                        "SCAN reference",
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
