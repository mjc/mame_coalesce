use super::{EDITION_VALIDATION_QUERY, SnapshotKey, Text, validate_edition};
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use camino::Utf8PathBuf;
use diesel::{QueryableByName, RunQueryDsl, connection::SimpleConnection, sql_query};

const TITLE: &str = "<software name='game' cloneof='parent' supported='yes'>
    <description>Game</description><year>2000</year><publisher>P</publisher>
    <info name='serial' value='1'/><sharedfeat name='compatibility' value='x'/>
    <part name='cart' interface='cart'><feature name='slot' value='x'/>
    <dataarea name='roms' size='1'><rom name='game.bin' size='1' offset='0'/></dataarea>
    <diskarea name='disk'><disk name='game.chd'/></diskarea>
    <dipswitch name='Mode' tag='config' mask='1'><dipvalue name='Off' value='0' default='yes'/></dipswitch>
    </part></software>";

fn import_edition(
    database: &Database,
    directory: &tempfile::TempDir,
    catalog: &str,
    title_count: usize,
) -> crate::Result<SnapshotKey> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{catalog}.xml")))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
    std::fs::write(
        &path,
        format!(
            "<softwarelists build='{catalog}'><softwarelist name='list' description='List'>{}</softwarelist></softwarelists>",
            TITLE.repeat(title_count)
        ),
    )?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("attribute-history-plan"),
            source_display_name: "Attribute history plan".into(),
            catalog_key: CatalogKey::new(catalog),
            catalog_display_name: catalog.into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    report.snapshot_key.ok_or_else(|| {
        crate::Error::InvalidPath("successful import did not publish a snapshot".into())
    })
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn edition_validation_is_owner_bounded_in_populated_database() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let requested = import_edition(&database, &directory, "requested", 2)?;
    let unrelated = import_edition(&database, &directory, "unrelated", 64)?;
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
    assert!(!plan.is_empty());
    let scans = plan
        .iter()
        .filter(|detail| {
            detail.strip_prefix("SCAN ").is_some_and(|scan| {
                scan.split_whitespace().next().is_some_and(|alias| {
                    matches!(
                        alias,
                        "owner"
                            | "position"
                            | "groups"
                            | "sets"
                            | "item"
                            | "part"
                            | "area"
                            | "detail"
                            | "occurrence"
                            | "document"
                            | "wrapper"
                            | "link"
                            | "switch"
                            | "other"
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(
        scans.is_empty(),
        "unbounded native scans: {scans:?}; plan: {plan:?}"
    );

    // Damage only an unrelated edition; the requested boundary must stay local.
    conn.batch_execute("DROP TRIGGER software_list_attribute_positions_immutable_delete;")?;
    sql_query("DELETE FROM software_list_attribute_positions WHERE field_kind=0 AND namespace_id IN (SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key=?)")
        .bind::<Text,_>(unrelated.as_str()).execute(&mut conn)?;
    validate_edition(&mut conn, &requested)?;
    assert!(validate_edition(&mut conn, &unrelated).is_err());
    Ok(())
}
