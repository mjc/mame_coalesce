use diesel::{
    Connection, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Text},
};

use crate::domain::{ContentDigestAlgorithm, ContentIdentity};

use super::{DigestTarget, Identifier, digest_target};

#[derive(QueryableByName)]
struct Plan {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn digest_target_lookup_and_readiness_seek_actual_populated_owners() -> crate::Result<()> {
    let database = crate::database::Database::in_memory()?;
    let mut connection = database.pool().get()?;
    let (target, declared_target, digest_id) = connection.transaction::<_, crate::Error, _>(|connection| {
        let mut selected = None;
        for value in 1..=500 {
            let identity =
                ContentIdentity::new(ContentDigestAlgorithm::Sha256, format!("{value:064x}"))?;
            let observed = digest_target(connection, &identity, DigestTarget::Observed)?;
            let declared = digest_target(connection, &identity, DigestTarget::Declared)?;
            if value == 251 {
                let digest_id = sql_query("SELECT digest_id AS value FROM digest_values WHERE algorithm='sha256' AND digest=?")
                    .bind::<Binary,_>(hex::decode(identity.digest()).map_err(|error| crate::Error::InvalidHash(error.to_string()))?)
                    .get_result::<Identifier>(connection)?.value;
                selected = Some((observed.database_value(), declared.database_value(), digest_id));
            }
        }
        selected
            .ok_or_else(|| crate::Error::DatabaseSchema("missing populated plan owner".into()))
    })?;
    sql_query("ANALYZE").execute(&mut connection)?;
    assert_ne!(
        target, declared_target,
        "equal digests retain distinct endpoint owners"
    );
    for (owner, expected_target) in [
        (DigestTarget::Observed, target),
        (DigestTarget::Declared, declared_target),
    ] {
        let actual = sql_query(owner.lookup_sql())
            .bind::<BigInt, _>(digest_id)
            .get_result::<Identifier>(&mut connection)?;
        assert_eq!(
            actual.value, expected_target,
            "lookup must return the actual populated owner"
        );
        let plans = sql_query(format!("EXPLAIN QUERY PLAN {}", owner.lookup_sql()))
            .bind::<BigInt, _>(digest_id)
            .load::<Plan>(&mut connection)?;
        assert!(
            plans.iter().any(|plan| plan.detail.starts_with("SEARCH ")
                && plan.detail.contains(owner.table())
                && plan.detail.contains("digest_id=?")),
            "missing native digest lookup seek: {:?}",
            plans.iter().map(|plan| &plan.detail).collect::<Vec<_>>()
        );
        assert!(
            !plans.iter().any(|plan| plan.detail.starts_with("SCAN ")),
            "native digest lookup scanned unrelated owners"
        );
    }
    let plans = sql_query(
        "EXPLAIN QUERY PLAN SELECT * FROM catalog_relationship_target_details WHERE target_id=?",
    )
    .bind::<BigInt, _>(target)
    .load::<Plan>(&mut connection)?;
    for alias in ["target", "observed_target", "digest_values"] {
        assert!(
            plans
                .iter()
                .any(|plan| plan.detail.starts_with(&format!("SEARCH {alias} "))
                    && (plan.detail.contains("PRIMARY KEY")
                        || plan.detail.contains("INTEGER PRIMARY KEY"))),
            "missing actual readiness owner seek for {alias}: {:?}",
            plans.iter().map(|plan| &plan.detail).collect::<Vec<_>>()
        );
    }
    assert!(
        !plans.iter().any(|plan| plan.detail.starts_with("SCAN ")),
        "target readiness scanned unrelated owners: {:?}",
        plans.iter().map(|plan| &plan.detail).collect::<Vec<_>>()
    );
    Ok(())
}
