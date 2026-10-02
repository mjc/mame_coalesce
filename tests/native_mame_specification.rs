use std::path::Path;

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct IntegerRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct ConditionOwnerRow {
    #[diesel(sql_type = Text)]
    family: String,
    #[diesel(sql_type = BigInt)]
    owner_order: i64,
    #[diesel(sql_type = Text)]
    relation: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = Text)]
    value: String,
}

fn setup() -> Result<(tempfile::TempDir, Database, SqliteConnection), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("catalog.sqlite");
    let database =
        Database::open(&Utf8PathBuf::from_path_buf(path.clone()).map_err(|_| "non-UTF8 db path")?)?;
    let connection = SqliteConnection::establish(path.to_str().ok_or("non-UTF8 db path")?)?;
    Ok((directory, database, connection))
}

fn request() -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let document_path = Utf8PathBuf::from_path_buf(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/all-fields.xml"),
    )
    .map_err(|_| "non-UTF8 MAME fixture path")?;
    Ok(CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("native-mame-test"),
        source_display_name: "Native MAME fixture".to_owned(),
        catalog_key: CatalogKey::new("native-mame-machine-fixture"),
        catalog_display_name: "Native MAME machine fixture".to_owned(),
        scope: CatalogScope::Unknown,
    })
}

#[test]
fn machine_specification_is_stored_in_native_families_with_typed_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let report = app::import_catalog(&database, &request()?)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);

    let view_kind = sql_query(
        "SELECT type AS value FROM sqlite_master WHERE name = 'mame_machine_spec_elements'",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(view_kind.value, "view");

    let union_wide_table_count = sql_query(
        "SELECT COUNT(*) AS value FROM sqlite_master \
         WHERE type = 'table' AND name = 'mame_machine_spec_elements'",
    )
    .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(union_wide_table_count.value, 0);

    let fields = sql_query(
        "SELECT GROUP_CONCAT(name, ',') AS value FROM pragma_table_info('mame_machine_displays')",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert!(fields.value.contains("refresh"));
    assert!(fields.value.contains("pixel_clock"));
    assert!(!fields.value.contains("driver_status"));
    assert!(!fields.value.contains("device_type"));

    let native_counts = sql_query(
        "SELECT COUNT(*) AS value FROM mame_machine_spec_elements \
         WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE set_name = 'complete')",
    )
    .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(native_counts.value, 14);

    let ordered = sql_query(
        "SELECT GROUP_CONCAT(element_type, ',') AS value FROM ( \
           SELECT element_type FROM mame_machine_spec_elements \
           WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE set_name = 'complete') \
           ORDER BY element_order \
         )",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        ordered.value,
        "sample,chip,chip,display,sound,input,port,adjuster,driver,feature,device,slot,softwarelist,ramoption"
    );

    let adjuster = sql_query(
        "SELECT name || ':' || default_value AS value FROM mame_machine_adjusters \
         WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE set_name = 'complete')",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(adjuster.value, "Volume:80");

    let condition_owners = sql_query(
        "SELECT 'adjuster' AS family, element_order AS owner_order, relation, mask, value \
         FROM mame_machine_adjuster_conditions \
         UNION ALL \
         SELECT 'switch', switch_order, relation, mask, value FROM machine_switch_conditions \
         UNION ALL \
         SELECT 'switch_value', value_order, relation, mask, value \
         FROM machine_switch_value_conditions \
         ORDER BY family, owner_order",
    )
    .load::<ConditionOwnerRow>(&mut connection)?;
    assert_condition_owners(&condition_owners);

    for (table, parent) in [
        ("mame_machine_input_controls", "mame_machine_inputs"),
        ("mame_machine_analogs", "mame_machine_ports"),
        ("mame_machine_device_instances", "mame_machine_devices"),
        ("mame_machine_device_extensions", "mame_machine_devices"),
        ("mame_machine_slot_options", "mame_machine_slots"),
        ("mame_machine_adjuster_conditions", "mame_machine_adjusters"),
        ("machine_switch_conditions", "machine_switches"),
        ("machine_switch_value_conditions", "machine_switch_values"),
    ] {
        let fk = sql_query(format!(
            "SELECT COUNT(*) AS value FROM pragma_foreign_key_list('{table}') WHERE \"table\" = '{parent}'"
        ))
        .get_result::<IntegerRow>(&mut connection)?;
        assert!(fk.value > 0, "{table} must have a real FK to {parent}");
    }

    let record_fk = sql_query(
        "SELECT COUNT(*) AS value FROM pragma_foreign_key_list('mame_machine_displays') \
         WHERE \"table\" = 'catalog_sets' AND \"to\" = 'set_id'",
    )
    .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(record_fk.value, 1);

    let checks = sql_query("SELECT COUNT(*) AS value FROM pragma_foreign_key_check")
        .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(checks.value, 0);
    Ok(())
}

fn assert_condition_owners(condition_owners: &[ConditionOwnerRow]) {
    assert_eq!(condition_owners.len(), 5);
    assert_eq!(condition_owners[0].family, "adjuster");
    assert_eq!(condition_owners[0].relation, "lt");
    assert_eq!(condition_owners[0].mask, "0x01");
    assert_eq!(condition_owners[0].value, "0x02");
    assert!(condition_owners.iter().any(|condition| {
        condition.family == "switch"
            && condition.owner_order == 0
            && condition.relation == "eq"
            && condition.mask == "0x01"
            && condition.value == "0x02"
    }));
    assert!(condition_owners.iter().any(|condition| {
        condition.family == "switch_value"
            && condition.owner_order == 0
            && condition.relation == "ne"
            && condition.mask == "0x01"
            && condition.value == "0x00"
    }));
}
