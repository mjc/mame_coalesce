use camino::Utf8PathBuf;
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Bool, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use std::fmt::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn request(path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("native-machine-source"),
        source_display_name: "Native machine source".into(),
        catalog_key: CatalogKey::new("native-machine-catalog"),
        catalog_display_name: "Native machine catalog".into(),
        scope: CatalogScope::Complete,
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct SwitchRow {
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct SwitchLocationRow {
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    location_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    number: String,
    #[diesel(sql_type = Bool)]
    inverted: bool,
    #[diesel(sql_type = Bool)]
    inverted_specified: bool,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct SwitchValueRow {
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    value_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = Bool)]
    is_default: bool,
    #[diesel(sql_type = Bool)]
    default_specified: bool,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct SwitchConditionRow {
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = Text)]
    relation: String,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[test]
fn machine_switch_import_preserves_rows_and_large_corpus() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    write_switch_fixture(&document)?;
    let report = app::import_catalog(&database, &request(document))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    assert_switch_records(&mut connection)?;
    assert_switch_locations(&mut connection)?;
    assert_switch_values(&mut connection)?;
    assert_switch_attribute_provenance(&mut connection)?;
    Ok(())
}

const BATCH_CORPUS: usize = 130;

fn write_switch_fixture(document: &Utf8PathBuf) -> TestResult {
    let mut xml = String::from(
        "<mame mameconfig='10'>\n<machine name='system'>\n<description>System</description>\n",
    );
    xml.push_str(
        "<dipswitch name='Power' tag=':SW' mask='0x01'>\n\
         <condition tag=':CFG' mask='0x01' relation='eq' value='1'/>\n\
         <diplocation name='SW1' number='1' inverted='yes'/>\n\
         <dipvalue name='Off' value='0x00' default='yes'>\n\
         <condition tag=':CFG' mask='0x01' relation='ne' value='0'/>\n\
         </dipvalue>\n\
         <dipvalue name='On' value='0x01' default='no'/>\n\
         </dipswitch>\n\
         <configuration name='Mode' tag=':CFG' mask='mode'>\n\
         <conflocation name='JP1' number='2'/>\n\
         <confsetting name='Automatic' value='automatic' default='yes'/>\n\
         </configuration>\n",
    );
    for index in 0..BATCH_CORPUS {
        write!(
            xml,
            "<dipswitch name='Batch {index}' tag=':B{index}' mask='0x01'>\n\
             <diplocation name='SW{index}' number='{index}' inverted='no'/>\n\
             <dipvalue name='Value {index}' value='0x{index:02x}' default='no'/>\n\
             </dipswitch>\n"
        )?;
    }
    xml.push_str("</machine>\n</mame>\n");
    std::fs::write(document, xml)?;
    Ok(())
}

fn assert_switch_records(connection: &mut SqliteConnection) -> TestResult {
    let switches = sql_query(
        "SELECT switch_order,kind,name,tag,mask,source_order,source_line,source_column \
         FROM machine_switches ORDER BY switch_order",
    )
    .load::<SwitchRow>(connection)?;
    assert_eq!(switches.len(), BATCH_CORPUS + 2);
    assert_eq!(
        switches[0],
        SwitchRow {
            switch_order: 0,
            kind: "dipswitch".into(),
            name: "Power".into(),
            tag: ":SW".into(),
            mask: "0x01".into(),
            source_order: 1,
            source_line: 4,
            source_column: 1,
        }
    );
    assert_eq!(
        switches[1],
        SwitchRow {
            switch_order: 1,
            kind: "configuration".into(),
            name: "Mode".into(),
            tag: ":CFG".into(),
            mask: "mode".into(),
            source_order: 2,
            source_line: 12,
            source_column: 1,
        }
    );
    for (index, row) in switches.iter().skip(2).enumerate() {
        assert_eq!(
            row,
            &SwitchRow {
                switch_order: i64::try_from(index + 2)?,
                kind: "dipswitch".into(),
                name: format!("Batch {index}"),
                tag: format!(":B{index}"),
                mask: "0x01".into(),
                source_order: i64::try_from(index + 3)?,
                source_line: i64::try_from(16 + index * 4)?,
                source_column: 1,
            }
        );
    }
    Ok(())
}

fn assert_switch_locations(connection: &mut SqliteConnection) -> TestResult {
    let locations = sql_query(
        "SELECT switch_order,location_order,source_order,name,number,inverted,inverted_specified,source_line,source_column \
         FROM machine_switch_locations ORDER BY switch_order,location_order",
    )
    .load::<SwitchLocationRow>(connection)?;
    assert_eq!(locations.len(), BATCH_CORPUS + 2);
    assert_eq!(
        locations[0],
        SwitchLocationRow {
            switch_order: 0,
            location_order: 0,
            source_order: 1,
            name: "SW1".into(),
            number: "1".into(),
            inverted: true,
            inverted_specified: true,
            source_line: 6,
            source_column: 1,
        }
    );
    assert_eq!(
        locations[1],
        SwitchLocationRow {
            switch_order: 1,
            location_order: 0,
            source_order: 0,
            name: "JP1".into(),
            number: "2".into(),
            inverted: false,
            inverted_specified: false,
            source_line: 13,
            source_column: 1,
        }
    );
    for (index, row) in locations.iter().skip(2).enumerate() {
        assert_eq!(
            row,
            &SwitchLocationRow {
                switch_order: i64::try_from(index + 2)?,
                location_order: 0,
                source_order: 0,
                name: format!("SW{index}"),
                number: index.to_string(),
                inverted: false,
                inverted_specified: true,
                source_line: i64::try_from(17 + index * 4)?,
                source_column: 1,
            }
        );
    }
    Ok(())
}

fn assert_switch_values(connection: &mut SqliteConnection) -> TestResult {
    let values = sql_query(
        "SELECT switch_order,value_order,source_order,name,value,is_default,default_specified,source_line,source_column \
         FROM machine_switch_values ORDER BY switch_order,value_order",
    )
    .load::<SwitchValueRow>(connection)?;
    assert_eq!(values.len(), BATCH_CORPUS + 3);
    assert_eq!(
        &values[..3],
        &[
            SwitchValueRow {
                switch_order: 0,
                value_order: 0,
                source_order: 2,
                name: "Off".into(),
                value: "0x00".into(),
                is_default: true,
                default_specified: true,
                source_line: 7,
                source_column: 1,
            },
            SwitchValueRow {
                switch_order: 0,
                value_order: 1,
                source_order: 3,
                name: "On".into(),
                value: "0x01".into(),
                is_default: false,
                default_specified: true,
                source_line: 10,
                source_column: 1,
            },
            SwitchValueRow {
                switch_order: 1,
                value_order: 0,
                source_order: 1,
                name: "Automatic".into(),
                value: "automatic".into(),
                is_default: true,
                default_specified: true,
                source_line: 14,
                source_column: 1,
            },
        ]
    );
    for (index, row) in values.iter().skip(3).enumerate() {
        assert_eq!(
            row,
            &SwitchValueRow {
                switch_order: i64::try_from(index + 2)?,
                value_order: 0,
                source_order: 1,
                name: format!("Value {index}"),
                value: format!("0x{index:02x}"),
                is_default: false,
                default_specified: true,
                source_line: i64::try_from(18 + index * 4)?,
                source_column: 1,
            }
        );
    }
    Ok(())
}

fn assert_switch_attribute_provenance(connection: &mut SqliteConnection) -> TestResult {
    let attribute_positions = sql_query(
        "SELECT count(*) AS count FROM machine_switches_attribute_positions UNION ALL \
         SELECT count(*) FROM machine_switch_locations_attribute_positions UNION ALL \
         SELECT count(*) FROM machine_switch_values_attribute_positions UNION ALL \
         SELECT count(*) FROM machine_switch_conditions_attribute_positions UNION ALL \
         SELECT count(*) FROM machine_switch_value_conditions_attribute_positions",
    )
    .load::<Count>(connection)?;
    assert_eq!(
        attribute_positions
            .iter()
            .map(|row| row.count)
            .collect::<Vec<_>>(),
        [396, 395, 399, 4, 4]
    );
    let switch_conditions = sql_query(
        "SELECT tag,mask,relation,value,source_line,source_column \
         FROM machine_switch_conditions ORDER BY set_id,switch_order,condition_order",
    )
    .load::<SwitchConditionRow>(connection)?;
    assert_eq!(
        switch_conditions,
        [SwitchConditionRow {
            tag: ":CFG".into(),
            mask: "0x01".into(),
            relation: "eq".into(),
            value: "1".into(),
            source_line: 5,
            source_column: 1,
        }]
    );
    let value_conditions = sql_query(
        "SELECT tag,mask,relation,value,source_line,source_column \
         FROM machine_switch_value_conditions ORDER BY set_id,switch_order,value_order,condition_order",
    )
    .load::<SwitchConditionRow>(connection)?;
    assert_eq!(
        value_conditions,
        [SwitchConditionRow {
            tag: ":CFG".into(),
            mask: "0x01".into(),
            relation: "ne".into(),
            value: "0".into(),
            source_line: 8,
            source_column: 1,
        }]
    );
    Ok(())
}

#[test]
fn required_mame_header_and_bios_fields_fail_without_publishing_partial_facts() -> TestResult {
    for xml in [
        "<mame><machine name='system'><description>System</description></machine></mame>",
        "<mame mameconfig='10'><machine name='system'><description>System</description><biosset name='main'/></machine></mame>",
    ] {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&database_path)?;
        let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
        std::fs::write(&document, xml)?;
        let report = app::import_catalog(&database, &request(document))?;
        assert_eq!(report.status, CatalogImportStatus::Failed);
        assert!(report.snapshot_key.is_none());
        let mut connection = SqliteConnection::establish(database_path.as_str())?;
        for table in [
            "snapshot_publications",
            "catalog_sets",
            "mame_document_facts",
        ] {
            let rows = sql_query(format!("SELECT count(*) AS count FROM {table}"))
                .get_result::<Count>(&mut connection)?;
            assert_eq!(rows.count, 0, "failed import leaked {table}");
        }
    }
    Ok(())
}

#[test]
fn document_build_and_default_presence_have_native_query_storage() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document,
        "<mame build='0.289-test' mameconfig='10'><machine name='system'><description>System</description></machine></mame>",
    )?;
    let report = app::import_catalog(&database, &request(document))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let rows = sql_query(
        "SELECT count(*) AS count FROM mame_document_facts \
         WHERE build='0.289-test' AND debug=0 AND debug_specified=0 AND config_version='10'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(rows.count, 1);
    Ok(())
}

#[test]
fn native_machine_children_require_real_owners_without_sqlite_foreign_keys() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let _database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let error = sql_query(
        "INSERT INTO mame_machine_chips \
         (set_id,element_order,name,kind,source_line,source_column) \
         VALUES (999,0,'orphan','cpu',1,1)",
    )
    .execute(&mut connection)
    .err()
    .ok_or("native MAME chip without an actual machine owner was accepted")?;
    assert!(error.to_string().contains("native MAME owner"), "{error}");
    Ok(())
}

fn pending_machine(
    connection: &mut SqliteConnection,
    base: &str,
    label: &str,
    header: bool,
    machine: bool,
) -> TestResult<(String, i64)> {
    let key = format!("pending-{label}");
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalogs WHERE catalog_key='native-machine-catalog'")
        .bind::<Text,_>(&key).bind::<Text,_>(&key).execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(&key).bind::<Text,_>(&key).bind::<Text,_>(base).execute(connection)?;
    if header {
        sql_query("INSERT INTO mame_document_facts(snapshot_key,build,debug,debug_specified,config_version,source_line,source_column) SELECT ?,build,debug,debug_specified,config_version,source_line,source_column FROM mame_document_facts WHERE snapshot_key=?")
            .bind::<Text,_>(&key).bind::<Text,_>(base).execute(connection)?;
        sql_query("INSERT INTO mame_document_facts_attribute_positions(document_id,field_kind,source_order,source_line,source_column) SELECT target.document_id,positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM mame_document_facts AS target CROSS JOIN mame_document_facts AS source CROSS JOIN mame_document_facts_attribute_positions AS positions WHERE target.snapshot_key=? AND source.snapshot_key=? AND positions.document_id=source.document_id")
            .bind::<Text,_>(&key).bind::<Text,_>(base).execute(connection)?;
    }
    let group = sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0) RETURNING set_group_id AS count")
        .bind::<Text,_>(&key).get_result::<Count>(connection)?.count;
    let owner = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES (?,'mame_machine',0,'pending',1,1) RETURNING set_id AS count")
        .bind::<BigInt,_>(group).get_result::<Count>(connection)?.count;
    if machine {
        sql_query("INSERT INTO mame_machines(set_id,description,description_source_order,description_line,description_column,is_device,is_device_specified,runnable,runnable_specified,is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,attributes_line,attributes_column) VALUES (?,'Pending',0,1,1,0,0,1,0,0,0,0,0,1,1)")
            .bind::<BigInt,_>(owner).execute(connection)?;
        insert_positions(
            connection,
            "mame_machines_attribute_positions",
            &[("set_id", owner)],
            &[0],
        )?;
    }
    Ok((key, owner))
}

fn insert_positions(
    connection: &mut SqliteConnection,
    table: &str,
    owners: &[(&str, i64)],
    fields: &[i64],
) -> TestResult {
    insert_positions_at(connection, table, owners, fields, 0)
}

fn insert_positions_at(
    connection: &mut SqliteConnection,
    table: &str,
    owners: &[(&str, i64)],
    fields: &[i64],
    first_order: usize,
) -> TestResult {
    let columns = owners
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(",");
    let keys = owners
        .iter()
        .map(|(_, value)| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    for (index, field) in fields.iter().enumerate() {
        let order = first_order + index;
        sql_query(format!(
            "INSERT INTO {table}({columns},field_kind,source_order,source_line,source_column) VALUES ({keys},{field},{order},1,{})",
            order + 1
        )).execute(connection)?;
    }
    Ok(())
}

fn publish_pending(connection: &mut SqliteConnection, key: &str) -> QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(key).execute(connection)
}

#[test]
fn publication_rejects_a_mame_root_without_any_machine() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document,
        "<mame mameconfig='10'><machine name='seed'><description>Seed</description></machine></mame>",
    )?;
    let base = app::import_catalog(&database, &request(document))?
        .snapshot_key
        .ok_or("seed snapshot missing")?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let key = "empty-native-mame";
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalogs WHERE catalog_key='native-machine-catalog'")
        .bind::<Text,_>(key).bind::<Text,_>(key).execute(&mut connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(key).bind::<Text,_>(key).bind::<Text,_>(base.as_str()).execute(&mut connection)?;
    sql_query("INSERT INTO mame_document_facts(snapshot_key,build,debug,debug_specified,config_version,source_line,source_column) SELECT ?,build,debug,debug_specified,config_version,source_line,source_column FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text,_>(key).bind::<Text,_>(base.as_str()).execute(&mut connection)?;
    sql_query("INSERT INTO mame_document_facts_attribute_positions(document_id,field_kind,source_order,source_line,source_column) SELECT document_id,2,0,1,7 FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text,_>(key).execute(&mut connection)?;
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0)")
        .bind::<Text, _>(key)
        .execute(&mut connection)?;
    let error = publish_pending(&mut connection, key)
        .err()
        .ok_or("empty MAME root was published")?;
    assert!(
        error.to_string().contains("native MAME publication"),
        "{error}"
    );
    Ok(())
}

#[test]
fn publication_rejects_unusable_or_mismatched_native_uuid_declarations() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document,
        "<mame mameconfig='10'><machine name='seed'><description>Seed</description><rom name='trusted' size='16' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/></machine></mame>",
    )?;
    let base = app::import_catalog(&database, &request(document))?
        .snapshot_key
        .ok_or("seed snapshot missing")?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let seed = sql_query(
        "SELECT occurrence_id AS count FROM asset_occurrences WHERE content_uuid IS NOT NULL",
    )
    .get_result::<Count>(&mut connection)?
    .count;
    for (label, size, crc, sha1, md5) in [
        (
            "bad-size",
            "not-number",
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            None,
        ),
        (
            "large-size",
            "9223372036854775808",
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            None,
        ),
        (
            "bad-crc",
            "16",
            Some("not-hex"),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            None,
        ),
        (
            "bad-md5",
            "16",
            None,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Some("not-hex"),
        ),
        (
            "wrong-sha1",
            "16",
            None,
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            None,
        ),
    ] {
        let (key, owner) = pending_machine(&mut connection, base.as_str(), label, true, true)?;
        let occurrence=sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) SELECT ?,0,'mame_rom',content_uuid FROM asset_occurrences WHERE occurrence_id=? RETURNING occurrence_id AS count")
            .bind::<BigInt,_>(owner).bind::<BigInt,_>(seed).get_result::<Count>(&mut connection)?.count;
        sql_query("INSERT INTO mame_rom_claims(occurrence_id,name,size_text,crc_text,sha1_text,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,optional,optional_specified,source_line,source_column) VALUES (?,'bad',?,?,?,'whole_asset','source_declared','good',0,1,0,0,1,1)")
            .bind::<BigInt,_>(occurrence).bind::<Text,_>(size)
            .bind::<diesel::sql_types::Nullable<Text>,_>(crc).bind::<Text,_>(sha1).execute(&mut connection)?;
        if let Some(md5) = md5 {
            sql_query("INSERT INTO mame_rom_compatibility(occurrence_id,md5_text) VALUES (?,?)")
                .bind::<BigInt, _>(occurrence)
                .bind::<Text, _>(md5)
                .execute(&mut connection)?;
            insert_positions_at(
                &mut connection,
                "mame_rom_compatibility_attribute_positions",
                &[("occurrence_id", occurrence)],
                &[0],
                3,
            )?;
        }
        let fields = if crc.is_some() {
            &[0, 2, 3, 4][..]
        } else {
            &[0, 2, 4][..]
        };
        insert_positions(
            &mut connection,
            "mame_rom_claims_attribute_positions",
            &[("occurrence_id", occurrence)],
            fields,
        )?;
        sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) SELECT ?,digest_id,scope,provenance FROM occurrence_digest_assertions WHERE occurrence_id=?")
            .bind::<BigInt,_>(occurrence).bind::<BigInt,_>(seed).execute(&mut connection)?;
        let error = publish_pending(&mut connection, &key)
            .err()
            .ok_or_else(|| {
                format!("{label} published a UUID without usable matching native declarations")
            })?;
        assert!(
            error.to_string().contains("native MAME publication"),
            "{label}: {error}"
        );
    }
    Ok(())
}

#[test]
fn publication_requires_native_header_machine_and_unique_child_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document,
        "<mame mameconfig='10'><machine name='seed'><description>Seed</description></machine></mame>",
    )?;
    let base = app::import_catalog(&database, &request(document))?
        .snapshot_key
        .ok_or("seed import failed")?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    for (label, header, machine, collision) in [
        ("no-header", false, true, false),
        ("no-machine", true, false, false),
        ("child-collision", true, true, true),
    ] {
        let (key, owner) = pending_machine(&mut connection, base.as_str(), label, header, machine)?;
        if collision {
            sql_query("INSERT INTO mame_machine_chips(set_id,element_order,name,kind,source_line,source_column) VALUES (?,0,'cpu','cpu',1,1)")
                .bind::<BigInt,_>(owner).execute(&mut connection)?;
            insert_positions(
                &mut connection,
                "mame_machine_chips_attribute_positions",
                &[("set_id", owner), ("element_order", 0)],
                &[0, 2],
            )?;
        }
        let error = publish_pending(&mut connection, &key)
            .err()
            .ok_or_else(|| format!("incomplete native MAME {label} was published"))?;
        assert!(
            error.to_string().contains("native MAME publication"),
            "{label}: {error}"
        );
    }
    let (key, owner) = pending_machine(&mut connection, base.as_str(), "source-gaps", true, true)?;
    sql_query("INSERT INTO mame_machine_chips(set_id,element_order,name,kind,source_line,source_column) VALUES (?,7,'cpu','cpu',1,1)")
        .bind::<BigInt,_>(owner).execute(&mut connection)?;
    insert_positions(
        &mut connection,
        "mame_machine_chips_attribute_positions",
        &[("set_id", owner), ("element_order", 7)],
        &[0, 2],
    )?;
    for statement in [
        "INSERT INTO mame_machine_displays(set_id,element_order,kind,flip_x,flip_x_specified,refresh,source_line,source_column) VALUES (?,8,'raster',1,0,'60',1,1)",
        "INSERT INTO mame_machine_input_controls(set_id,element_order,control_order,control_type,reverse,reverse_specified,source_line,source_column) VALUES (?,9,0,'joy',0,0,1,1)",
        "INSERT OR REPLACE INTO mame_machine_chips(set_id,element_order,name,kind,source_line,source_column) VALUES (?,7,'replacement','cpu',1,1)",
        "UPDATE mame_machine_chips SET name='updated' WHERE set_id=?",
        "DELETE FROM mame_machine_chips WHERE set_id=?",
    ] {
        assert!(
            sql_query(statement)
                .bind::<BigInt, _>(owner)
                .execute(&mut connection)
                .is_err(),
            "invalid native write accepted: {statement}"
        );
    }
    assert_eq!(publish_pending(&mut connection, &key)?, 1);
    assert_remaining_publication_rules(&mut connection, base.as_str())
}

fn assert_remaining_publication_rules(connection: &mut SqliteConnection, base: &str) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "duplicate-sound", true, true)?;
    for order in [1_i64, 2] {
        sql_query("INSERT INTO mame_machine_sounds(set_id,element_order,channels,source_line,source_column) VALUES (?,?,'1',1,1)")
            .bind::<BigInt,_>(owner).bind::<BigInt,_>(order).execute(connection)?;
        insert_positions(
            connection,
            "mame_machine_sounds_attribute_positions",
            &[("set_id", owner), ("element_order", order)],
            &[0],
        )?;
    }
    assert!(publish_pending(connection, &key).is_err());
    for kind in ["mame_rom", "mame_disk"] {
        let (key, owner) = pending_machine(connection, base, kind, true, true)?;
        sql_query(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES (?,0,?)",
        )
        .bind::<BigInt, _>(owner)
        .bind::<Text, _>(kind)
        .execute(connection)?;
        assert!(
            publish_pending(connection, &key).is_err(),
            "missing {kind} native payload published"
        );
    }
    let (key, owner) = pending_machine(connection, base, "writable-presence", true, true)?;
    let occurrence=sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES (?,0,'mame_disk') RETURNING occurrence_id AS count")
        .bind::<BigInt,_>(owner).get_result::<Count>(connection)?.count;
    for (writable, specified) in [("1", 0), ("NULL", 1)] {
        let statement = format!(
            "INSERT INTO mame_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,optional,optional_specified,writable,writable_specified,source_line,source_column) VALUES (?,'disk','chd_header_sha1','source_declared','good',0,1,0,0,{writable},{specified},1,1)"
        );
        assert!(
            sql_query(statement)
                .bind::<BigInt, _>(occurrence)
                .execute(connection)
                .is_err()
        );
    }
    sql_query("INSERT INTO mame_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,optional,optional_specified,writable,writable_specified,source_line,source_column) VALUES (?,'disk','chd_header_sha1','source_declared','good',0,1,0,0,0,0,1,1)")
        .bind::<BigInt,_>(occurrence).execute(connection)?;
    insert_positions(
        connection,
        "mame_disk_claims_attribute_positions",
        &[("occurrence_id", occurrence)],
        &[0],
    )?;
    assert_eq!(publish_pending(connection, &key)?, 1);
    let (key, owner) = pending_machine(connection, base, "nested-collision", true, true)?;
    sql_query("INSERT INTO machine_switches(set_id,switch_order,kind,name,tag,mask,source_order,source_line,source_column) VALUES (?,0,'dipswitch','Switch',':DSW','0x01',1,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
    sql_query("INSERT INTO machine_switch_locations(set_id,switch_order,location_order,source_order,name,number,inverted,inverted_specified,source_line,source_column) VALUES (?,0,0,0,'SW','1',0,0,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
    sql_query("INSERT INTO machine_switch_values(set_id,switch_order,value_order,source_order,name,value,is_default,default_specified,source_line,source_column) VALUES (?,0,0,0,'Off','0x00',0,0,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
    insert_positions(
        connection,
        "machine_switches_attribute_positions",
        &[("set_id", owner), ("switch_order", 0)],
        &[0, 1, 2],
    )?;
    insert_positions(
        connection,
        "machine_switch_locations_attribute_positions",
        &[
            ("set_id", owner),
            ("switch_order", 0),
            ("location_order", 0),
        ],
        &[0, 1],
    )?;
    insert_positions(
        connection,
        "machine_switch_values_attribute_positions",
        &[("set_id", owner), ("switch_order", 0), ("value_order", 0)],
        &[0, 1],
    )?;
    assert!(publish_pending(connection, &key).is_err());
    Ok(())
}

#[derive(QueryableByName)]
struct PlanStep {
    #[diesel(sql_type=Text)]
    detail: String,
}

#[derive(QueryableByName)]
struct StoredSql {
    #[diesel(sql_type=Text)]
    sql: String,
}

#[test]
fn publication_child_checks_seek_requested_machine_owners() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let trigger =
        sql_query("SELECT sql FROM sqlite_schema WHERE name='mame_native_publication_guard'")
            .get_result::<StoredSql>(&mut connection)?
            .sql;
    let query = trigger
        .split_once("WHEN ")
        .ok_or("missing publication predicate")?
        .1
        .rsplit_once(" BEGIN")
        .ok_or("missing publication body")?
        .0
        .replace("NEW.snapshot_key", "'requested'");
    let steps = sql_query(format!("EXPLAIN QUERY PLAN SELECT {query}"))
        .load::<PlanStep>(&mut connection)?
        .into_iter()
        .map(|step| step.detail)
        .collect::<Vec<_>>();
    for table in [
        "mame_machines",
        "mame_bios_sets",
        "mame_rom_claims",
        "mame_disk_claims",
        "mame_samples",
        "mame_machine_chips",
        "mame_machine_displays",
        "mame_machine_inputs",
        "mame_machine_drivers",
        "machine_switches",
        "machine_switch_values",
        "machine_switch_locations",
    ] {
        assert!(
            steps
                .iter()
                .any(|step| step.contains(&format!("SEARCH {table}"))),
            "missing requested owner lookup for {table}: {steps:?}"
        );
        assert!(
            !steps
                .iter()
                .any(|step| step.starts_with(&format!("SCAN {table}"))),
            "global scan for {table}: {steps:?}"
        );
    }
    Ok(())
}
