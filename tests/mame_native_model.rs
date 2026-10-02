use camino::Utf8PathBuf;
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

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
    }
    let group = sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0) RETURNING set_group_id AS count")
        .bind::<Text,_>(&key).get_result::<Count>(connection)?.count;
    let owner = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES (?,'mame_machine',0,'pending',1,1) RETURNING set_id AS count")
        .bind::<BigInt,_>(group).get_result::<Count>(connection)?.count;
    if machine {
        sql_query("INSERT INTO mame_machines(set_id,description,description_source_order,description_line,description_column,is_device,is_device_specified,runnable,runnable_specified,is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,attributes_line,attributes_column) VALUES (?,'Pending',0,1,1,0,0,1,0,0,0,0,0,1,1)")
            .bind::<BigInt,_>(owner).execute(connection)?;
    }
    Ok((key, owner))
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
        }
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
    assert_eq!(publish_pending(connection, &key)?, 1);
    let (key, owner) = pending_machine(connection, base, "nested-collision", true, true)?;
    sql_query("INSERT INTO machine_switches(set_id,switch_order,kind,name,tag,mask,source_order,source_line,source_column) VALUES (?,0,'dipswitch','Switch',':DSW','0x01',1,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
    sql_query("INSERT INTO machine_switch_locations(set_id,switch_order,location_order,source_order,name,number,inverted,inverted_specified,source_line,source_column) VALUES (?,0,0,0,'SW','1',0,0,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
    sql_query("INSERT INTO machine_switch_values(set_id,switch_order,value_order,source_order,name,value,is_default,default_specified,source_line,source_column) VALUES (?,0,0,0,'Off','0x00',0,0,1,1)")
        .bind::<BigInt,_>(owner).execute(connection)?;
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
        .split_once("WHEN EXISTS (")
        .ok_or("missing publication predicate")?
        .1
        .rsplit_once(") BEGIN")
        .ok_or("missing publication body")?
        .0
        .replace("NEW.snapshot_key", "'requested'");
    let steps = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
        .load::<PlanStep>(&mut connection)?
        .into_iter()
        .map(|step| step.detail)
        .collect::<Vec<_>>();
    for table in [
        "mame_machines",
        "mame_bios_sets",
        "mame_rom_claims",
        "mame_disk_claims",
        "mame_machine_samples",
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
