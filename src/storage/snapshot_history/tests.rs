use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::Text};

#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn mame_specification_history_seeks_samples_in_requested_snapshot() -> crate::Result<()> {
    let pool = crate::storage::db::create_db_pool(":memory:")?;
    let mut connection = pool.get()?;
    let plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        super::MAME_SPECIFICATION_SQL
    ))
    .bind::<Text, _>("requested")
    .load::<PlanRow>(&mut connection)?;
    let scans = plan
        .iter()
        .filter(|step| {
            step.detail.starts_with("SCAN sample") || step.detail.starts_with("SCAN occurrence")
        })
        .map(|step| step.detail.as_str())
        .collect::<Vec<_>>();
    assert!(
        scans.is_empty(),
        "history scans unrelated samples: {scans:?}"
    );
    assert!(
        plan.iter()
            .any(|step| step.detail.starts_with("SEARCH sample USING PRIMARY KEY")),
        "history does not hydrate native samples through actual occurrence keys"
    );
    assert!(
        plan.iter()
            .any(|step| step.detail.starts_with("SEARCH occurrence USING ")
                && step.detail.contains("record_id=?")),
        "history does not restrict media to requested machine owners"
    );
    Ok(())
}
