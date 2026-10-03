#![allow(clippy::expect_used)]

use super::{EDITION_VALIDATION_QUERY, SnapshotKey, Text, validate_edition};
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use camino::Utf8PathBuf;
use diesel::{QueryableByName, RunQueryDsl, connection::SimpleConnection, sql_query};

fn import_edition(
    database: &Database,
    directory: &tempfile::TempDir,
    catalog: &str,
    repetitions: usize,
) -> crate::Result<SnapshotKey> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{catalog}.xml")))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
    let xml = include_str!("../../../fixtures/specifications/mame-machine-fields.xml");
    let (header, rest) = xml
        .split_once("\t<machine")
        .expect("synthetic machine header");
    let (machine, _) = rest
        .split_once("</machine>")
        .expect("synthetic complete machine");
    std::fs::write(
        &path,
        format!(
            "{header}{}\n</mame>",
            format!("<machine{machine}</machine>").repeat(repetitions)
        ),
    )?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("machine-attribute-query-plan"),
            source_display_name: "Machine query plan".into(),
            catalog_key: CatalogKey::new(catalog),
            catalog_display_name: catalog.into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.expect("successful publication"))
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn edition_validation_searches_requested_owners_in_a_populated_database() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let requested = import_edition(&database, &directory, "requested", 2)?;
    let unrelated = import_edition(&database, &directory, "unrelated", 48)?;
    let mut conn = database.pool().get()?;
    validate_edition(&mut conn, &requested)?;
    validate_edition(&mut conn, &unrelated)?;
    conn.batch_execute("ANALYZE")?;
    let plan = sql_query(format!("EXPLAIN QUERY PLAN {EDITION_VALIDATION_QUERY}"))
        .bind::<Text, _>(requested.as_str())
        .load::<ExplainRow>(&mut conn)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    let scans = plan
        .iter()
        .filter(|detail| {
            detail.strip_prefix("SCAN ").is_some_and(|scan| {
                scan.split_whitespace().next().is_some_and(|alias| {
                    matches!(
                        alias,
                        "owner"
                            | "position"
                            | "native"
                            | "compat"
                            | "groups"
                            | "sets"
                            | "set_owner"
                            | "occurrence"
                            | "document"
                            | "catalog_sets"
                            | "asset_occurrences"
                            | "catalog_set_groups"
                    ) || (alias.starts_with("mame_")
                        && !matches!(
                            alias,
                            "mame_expected_attribute_positions" | "mame_actual_attribute_positions"
                        ))
                        || alias.starts_with("machine_switch")
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(
        scans.is_empty(),
        "unbounded native scans: {scans:?}; plan: {plan:?}"
    );
    conn.batch_execute("DROP TRIGGER mame_machines_attribute_positions_attribute_position_delete")?;
    sql_query("DELETE FROM mame_machines_attribute_positions WHERE field_kind=0 AND set_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=?)")
        .bind::<Text,_>(unrelated.as_str()).execute(&mut conn)?;
    validate_edition(&mut conn, &requested)?;
    assert!(validate_edition(&mut conn, &unrelated).is_err());
    Ok(())
}
