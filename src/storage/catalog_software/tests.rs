use std::fmt::Write as _;

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, connection::SimpleConnection, sql_query, sql_types::BigInt,
};

use super::{
    queries::{
        AREAS, DIP_SWITCHES, DIP_VALUES, DISK_ENTRIES, LIST_TEXT_POSITIONS, PART_FEATURES, PARTS,
        ROM_ENTRIES, TITLE_INFO, TITLE_SHARED_FEATURES, TITLE_TEXT_POSITIONS,
    },
    *,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn attribute_integrity_reports_missing_and_orphaned_native_positions() -> TestResult {
    let fixture = fixture()?;
    let mut conn = fixture.database.pool().get()?;
    let clean = sql_query("SELECT COUNT(*) AS owner_id FROM software_attribute_violations")
        .get_result::<OwnerId>(&mut conn)?;
    assert_eq!(clean.owner_id, 0);
    conn.batch_execute("PRAGMA foreign_keys=OFF; DROP TRIGGER software_item_attribute_positions_immutable_delete; DROP TRIGGER software_list_attribute_positions_immutable_update;")?;
    sql_query("DELETE FROM software_item_attribute_positions WHERE record_id=? AND field_kind=2")
        .bind::<BigInt, _>(fixture.title.as_i64())
        .execute(&mut conn)?;
    let missing=sql_query("SELECT COUNT(*) AS owner_id FROM software_attribute_violations WHERE owner_kind='item' AND owner_a=? AND field_kind=2 AND reason='missing_position'")
        .bind::<BigInt,_>(fixture.title.as_i64()).get_result::<OwnerId>(&mut conn)?;
    assert_eq!(missing.owner_id, 1);
    sql_query("UPDATE software_list_attribute_positions SET namespace_id=900001 WHERE namespace_id=? AND field_kind=0")
        .bind::<BigInt,_>(fixture.list.database_value()).execute(&mut conn)?;
    let orphan=sql_query("SELECT COUNT(*) AS owner_id FROM software_attribute_violations WHERE owner_kind='list' AND owner_a=900001 AND field_kind=0 AND reason='orphan_or_invalid_position'")
        .get_result::<OwnerId>(&mut conn)?;
    assert_eq!(orphan.owner_id, 1);
    drop(conn);
    assert!(
        lists_for_snapshot(
            &fixture.database,
            &fixture.snapshot,
            SoftwarePageLimit::new(10)?,
            None
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn wrapper_replacement_rejects_both_native_id_and_edition_collisions() -> TestResult {
    let fixture = fixture()?;
    let mut conn = fixture.database.pool().get()?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    for key in ["wrapper-draft-a", "wrapper-draft-b"] {
        sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,catalog_key,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
            .bind::<Text,_>(key).bind::<Text,_>(fixture.snapshot.as_str()).execute(&mut conn)?;
        sql_query(
            "INSERT INTO software_documents(snapshot_key,envelope_kind) VALUES (?,'plural_lists')",
        )
        .bind::<Text, _>(key)
        .execute(&mut conn)?;
    }
    conn.batch_execute("INSERT INTO software_wrapper_headers(wrapper_id,snapshot_key,build) VALUES (900001,'wrapper-draft-a','');")?;
    for (id, key) in [(900_001, "wrapper-draft-b"), (900_002, "wrapper-draft-a")] {
        assert!(sql_query("INSERT OR REPLACE INTO software_wrapper_headers(wrapper_id,snapshot_key,build) VALUES (?,?,'replacement')")
            .bind::<BigInt,_>(id).bind::<Text,_>(key).execute(&mut conn).is_err());
    }
    let survivor=sql_query("SELECT wrapper_id AS owner_id FROM software_wrapper_headers WHERE snapshot_key='wrapper-draft-a' AND build=''")
        .get_result::<OwnerId>(&mut conn)?;
    assert_eq!(survivor.owner_id, 900_001);
    Ok(())
}

fn software_list_xml() -> TestResult<String> {
    let mut xml = String::from(
        "<softwarelists><softwarelist name=\"fixture\" description=\"List\"><notes>List notes</notes>",
    );
    for index in 0..32 {
        write!(
            xml,
            "<software name=\"game{index}\" supported=\"yes\"><description>Game {index}</description><year>2024</year><publisher>Fixture publisher</publisher><notes>Title notes {index}</notes><info name=\"serial\" value=\"{index}\"/><info name=\"region\" value=\"world\"/><sharedfeat name=\"compatibility\" value=\"arcade\"/><part name=\"cart\" interface=\"cart\"><feature name=\"slot\" value=\"{index}\"/><dipswitch name=\"Mode\" tag=\"config\" mask=\"1\"><dipvalue name=\"Off\" value=\"0\" default=\"yes\"/><dipvalue name=\"On\" value=\"1\"/></dipswitch><dataarea name=\"roms\" size=\"16\"><rom name=\"game{index}.bin\" size=\"1\" offset=\"0\"/></dataarea><diskarea name=\"disk\"><disk name=\"game{index}.chd\"/></diskarea></part></software>"
        )?;
    }
    xml.push_str("</softwarelist></softwarelists>");
    Ok(xml)
}

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    snapshot: SnapshotKey,
    list: SoftwareListId,
    title: CatalogSetId,
    part: i64,
    data_area: i64,
    rom_occurrence: i64,
    disk_occurrence: i64,
    switch_order: i64,
}

#[derive(QueryableByName)]
struct FixtureOwners {
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    area_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    disk_occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
}

#[derive(QueryableByName)]
struct OwnerId {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
}

fn fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let database = Database::in_memory()?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, software_list_xml()?)?;
    let report = crate::app::import_catalog(
        &database,
        &crate::app::CatalogImportRequest {
            document_path,
            format: crate::app::CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("catalog-software-regression"),
            source_display_name: "Catalog software regression".to_owned(),
            catalog_key: CatalogKey::new("catalog-software-regression"),
            catalog_display_name: "Catalog software regression".to_owned(),
            scope: crate::domain::CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, crate::app::CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("import did not publish a snapshot")?;
    let lists = lists_for_snapshot(&database, &snapshot, SoftwarePageLimit::new(10)?, None)?;
    let list = lists.lists.first().ok_or("imported list missing")?.id;
    let titles = titles_for_list(
        &database,
        &snapshot,
        list,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let title = titles.titles.first().ok_or("imported item missing")?.id;

    let mut connection = database.pool().get()?;
    let ids = sql_query(
        "SELECT parts.part_id, areas.area_id, \
         rom.occurrence_id, disk.occurrence_id AS disk_occurrence_id, \
         (SELECT dipswitch_order FROM software_part_dipswitches \
          WHERE part_id = parts.part_id LIMIT 1) AS switch_order \
         FROM software_parts AS parts \
         JOIN software_areas AS areas ON areas.part_id = parts.part_id \
           AND areas.area_kind = 'data' \
         JOIN software_rom_entries AS rom ON rom.area_id = areas.area_id \
         JOIN software_areas AS disk_areas ON disk_areas.part_id = parts.part_id \
           AND disk_areas.area_kind = 'disk' \
         JOIN software_disk_entries AS disk ON disk.area_id = disk_areas.area_id \
         WHERE parts.record_id = ? LIMIT 1",
    )
    .bind::<BigInt, _>(title.as_i64())
    .get_result::<FixtureOwners>(&mut connection)?;
    drop(connection);
    Ok(Fixture {
        _directory: directory,
        database,
        snapshot,
        list,
        title,
        part: ids.part_id,
        data_area: ids.area_id,
        rom_occurrence: ids.occurrence_id,
        disk_occurrence: ids.disk_occurrence_id,
        switch_order: ids.switch_order,
    })
}

fn corrupt(fixture: &Fixture, sql: &str) -> TestResult {
    let mut connection = fixture.database.pool().get()?;
    // The fixture was imported through the public application path. Disable only
    // the guards needed to make this private regression fixture inconsistent.
    connection.batch_execute("PRAGMA foreign_keys = OFF; PRAGMA ignore_check_constraints = ON;")?;
    connection.batch_execute(sql)?;
    Ok(())
}

fn assert_owner_table_absent(database: &Database) -> TestResult {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }
    // The canonical project pool has max_size(1), so this checks the exact
    // connection returned by the reader, including its connection-local TEMP schema.
    let mut connection = database.pool().get()?;
    let count = sql_query(
        "SELECT COUNT(*) AS count FROM sqlite_temp_master \
         WHERE type = 'table' AND name = 'catalog_software_requested_owners'",
    )
    .get_result::<Count>(&mut connection)?
    .count;
    assert_eq!(count, 0, "temporary owner table leaked from the reader");
    Ok(())
}

#[test]
fn missing_software_list_and_item_rows_are_errors() -> TestResult {
    let list_fixture = fixture()?;
    corrupt(
        &list_fixture,
        &format!(
            "DROP TRIGGER software_lists_native_immutable_delete; \
             DELETE FROM software_lists WHERE namespace_id = {};",
            list_fixture.list.database_value()
        ),
    )?;
    assert!(
        lists_for_snapshot(
            &list_fixture.database,
            &list_fixture.snapshot,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err()
    );

    let item_fixture = fixture()?;
    corrupt(
        &item_fixture,
        &format!(
            "DROP TRIGGER software_items_native_immutable_delete; \
             DELETE FROM software_items WHERE record_id = {};",
            item_fixture.title.as_i64()
        ),
    )?;
    assert!(
        titles_for_list(
            &item_fixture.database,
            &item_fixture.snapshot,
            item_fixture.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn missing_required_text_positions_are_rejected() -> TestResult {
    for field_kind in 0..=3 {
        let fixture = fixture()?;
        corrupt(
            &fixture,
            &format!(
                "DROP TRIGGER software_item_text_positions_immutable_delete; \
                 DELETE FROM software_item_text_positions \
                 WHERE record_id = {} AND field_kind = {field_kind};",
                fixture.title.as_i64()
            ),
        )?;
        assert!(
            titles_for_list(
                &fixture.database,
                &fixture.snapshot,
                fixture.list,
                SoftwarePageLimit::new(10)?,
                None,
            )
            .is_err(),
            "accepted missing text position kind {field_kind}"
        );
    }
    Ok(())
}

#[test]
fn invalid_supported_boolean_and_area_values_are_rejected() -> TestResult {
    for (update, trigger) in [
        (
            "UPDATE software_items SET supported = 'unknown' WHERE record_id = {title};",
            "software_items_native_immutable_update",
        ),
        (
            "UPDATE software_items SET supported_specified = 2 WHERE record_id = {title};",
            "software_items_native_immutable_update",
        ),
        (
            "UPDATE software_part_dip_values SET is_default = 2 WHERE part_id = {part};",
            "software_part_dip_values_immutable_update",
        ),
        (
            "UPDATE software_areas SET area_kind = 'unknown' WHERE area_id = {area};",
            "software_areas_immutable_update",
        ),
        (
            "UPDATE software_data_areas SET width = 7 WHERE area_id = {area};",
            "software_data_areas_immutable_update",
        ),
        (
            "UPDATE software_data_areas SET endianness = 'middle' WHERE area_id = {area};",
            "software_data_areas_immutable_update",
        ),
    ] {
        let fixture = fixture()?;
        let sql = update
            .replace("{title}", &fixture.title.as_i64().to_string())
            .replace("{part}", &fixture.part.to_string())
            .replace("{area}", &fixture.data_area.to_string());
        corrupt(&fixture, &format!("DROP TRIGGER {trigger}; {sql}"))?;
        assert!(
            titles_for_list(
                &fixture.database,
                &fixture.snapshot,
                fixture.list,
                SoftwarePageLimit::new(10)?,
                None,
            )
            .is_err(),
            "accepted corrupted value from SQL: {sql}"
        );
    }
    Ok(())
}

#[test]
fn rom_entry_record_and_claim_kind_must_match_the_native_owner() -> TestResult {
    let record_fixture = fixture()?;
    corrupt(
        &record_fixture,
        &format!(
            "DROP TRIGGER software_rom_entries_immutable_update; \
             UPDATE software_rom_entries SET record_id = record_id + 100000 \
             WHERE occurrence_id = {};",
            record_fixture.rom_occurrence
        ),
    )?;
    assert!(
        titles_for_list(
            &record_fixture.database,
            &record_fixture.snapshot,
            record_fixture.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err()
    );

    let claim_fixture = fixture()?;
    corrupt(
        &claim_fixture,
        &format!(
            "DROP TRIGGER asset_occurrences_are_immutable_update; \
             UPDATE asset_occurrences SET claim_kind = 'software_disk_entry' \
             WHERE occurrence_id = {};",
            claim_fixture.rom_occurrence
        ),
    )?;
    assert!(
        titles_for_list(
            &claim_fixture.database,
            &claim_fixture.snapshot,
            claim_fixture.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn orphan_dip_values_are_errors() -> TestResult {
    let dip_fixture = fixture()?;
    corrupt(
        &dip_fixture,
        &format!(
            "DROP TRIGGER software_part_dipswitches_immutable_delete; \
             DELETE FROM software_part_dipswitches \
             WHERE part_id = {} AND dipswitch_order = {};",
            dip_fixture.part, dip_fixture.switch_order
        ),
    )?;
    assert!(
        titles_for_list(
            &dip_fixture.database,
            &dip_fixture.snapshot,
            dip_fixture.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err(),
        "orphan DIP values were silently omitted"
    );

    Ok(())
}

#[test]
fn missing_rom_occurrence_is_an_error() -> TestResult {
    let fixture = fixture()?;
    assert_missing_occurrence(&fixture, fixture.rom_occurrence)
}

#[test]
fn missing_disk_occurrence_is_an_error() -> TestResult {
    let fixture = fixture()?;
    assert_missing_occurrence(&fixture, fixture.disk_occurrence)
}

fn assert_missing_occurrence(fixture: &Fixture, occurrence: i64) -> TestResult {
    corrupt(
        fixture,
        &format!(
            "DROP TRIGGER asset_occurrences_are_immutable_delete; \
             DELETE FROM asset_occurrences WHERE occurrence_id = {occurrence};"
        ),
    )?;
    assert!(
        titles_for_list(
            &fixture.database,
            &fixture.snapshot,
            fixture.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err(),
        "missing occurrence was silently omitted"
    );
    Ok(())
}

#[test]
fn temporary_owner_table_is_cleaned_up_after_success_and_reader_error() -> TestResult {
    let successful = fixture()?;
    titles_for_list(
        &successful.database,
        &successful.snapshot,
        successful.list,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    assert_owner_table_absent(&successful.database)?;

    let failing = fixture()?;
    corrupt(
        &failing,
        &format!(
            "DROP TRIGGER software_areas_immutable_update; \
             UPDATE software_areas SET area_kind = 'unknown' WHERE area_id = {};",
            failing.data_area
        ),
    )?;
    assert!(
        titles_for_list(
            &failing.database,
            &failing.snapshot,
            failing.list,
            SoftwarePageLimit::new(10)?,
            None,
        )
        .is_err()
    );
    assert_owner_table_absent(&failing.database)?;
    Ok(())
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    detail: String,
}

#[test]
fn populated_owner_queries_use_requested_rows_in_the_query_plan() -> TestResult {
    let fixture = fixture()?;
    let mut connection = fixture.database.pool().get()?;
    connection.batch_execute(
        "CREATE TEMP TABLE catalog_software_requested_owners \
           (owner_id INTEGER PRIMARY KEY) WITHOUT ROWID;",
    )?;
    connection.batch_execute("ANALYZE")?;

    let queries = [
        ("list positions", LIST_TEXT_POSITIONS),
        ("item positions", TITLE_TEXT_POSITIONS),
        ("item info", TITLE_INFO),
        ("shared features", TITLE_SHARED_FEATURES),
        ("parts", PARTS),
        ("part features", PART_FEATURES),
        ("dip switches", DIP_SWITCHES),
        ("dip values", DIP_VALUES),
        ("areas", AREAS),
        ("ROM entries", ROM_ENTRIES),
        ("disk entries", DISK_ENTRIES),
    ];

    let title_ids = sql_query(
        "SELECT set_id AS owner_id FROM catalog_sets \
         WHERE set_group_id = ? AND source_element_kind = 'software_item' \
         ORDER BY list_order, set_id LIMIT 10",
    )
    .bind::<BigInt, _>(fixture.list.database_value())
    .load::<OwnerId>(&mut connection)?
    .into_iter()
    .map(|row| row.owner_id)
    .collect::<Vec<_>>();
    assert_eq!(title_ids.len(), 10, "fixture should fill a full title page");
    let area_ids = sql_query(
        "SELECT areas.area_id AS owner_id FROM software_areas AS areas \
         JOIN software_parts AS parts ON parts.part_id = areas.part_id \
         WHERE parts.record_id IN (SELECT set_id FROM catalog_sets \
           WHERE set_group_id = ? AND source_element_kind = 'software_item' \
           ORDER BY list_order, set_id LIMIT 10) ORDER BY areas.area_id",
    )
    .bind::<BigInt, _>(fixture.list.database_value())
    .load::<OwnerId>(&mut connection)?
    .into_iter()
    .map(|row| row.owner_id)
    .collect::<Vec<_>>();
    assert_eq!(
        area_ids.len(),
        20,
        "fixture should include both areas per title"
    );

    for (label, sql) in queries {
        connection.batch_execute("DELETE FROM temp.catalog_software_requested_owners")?;
        let owners = match label {
            "list positions" => vec![fixture.list.database_value()],
            "ROM entries" | "disk entries" => area_ids.clone(),
            _ => title_ids.clone(),
        };
        for owner in owners {
            sql_query("INSERT INTO temp.catalog_software_requested_owners(owner_id) VALUES (?)")
                .bind::<BigInt, _>(owner)
                .execute(&mut connection)?;
        }
        connection.batch_execute("ANALYZE temp.catalog_software_requested_owners")?;
        let explain = format!("EXPLAIN QUERY PLAN {sql}");
        let plan = sql_query(explain).load::<ExplainRow>(&mut connection)?;
        let details = plan.into_iter().map(|row| row.detail).collect::<Vec<_>>();
        assert!(!details.is_empty(), "{label} query had no plan rows");
        let global_scans = details
            .iter()
            .filter(|detail| detail.starts_with("SCAN ") && !detail.starts_with("SCAN requested"))
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            global_scans.is_empty(),
            "{label} query scans native rows globally: {global_scans:?}; plan: {details:?}"
        );
        assert!(
            details.iter().any(|detail| detail.contains("requested")),
            "{label} query plan does not use the requested-owner table: {details:?}"
        );
    }
    Ok(())
}
