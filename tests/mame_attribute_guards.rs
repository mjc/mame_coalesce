use std::collections::BTreeSet;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files,
    catalog_machines::{MachinePageLimit, machines_for_snapshot},
    database::Database,
    domain::{CatalogKey, CatalogScope, OccurrenceId, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const XML: &str = include_str!("../fixtures/specifications/mame-machine-fields.xml");

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct ForeignKeyRow {
    #[diesel(sql_type = Text)]
    table: String,
    #[diesel(sql_type = Text)]
    from: String,
    #[diesel(sql_type = Text)]
    to: String,
}

struct Fixture {
    _directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
    snapshot: SnapshotKey,
}

fn import_fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("mame.xml"))?;
    std::fs::write(&document, XML)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("mame-attribute-guards"),
            source_display_name: "MAME attribute guards".to_owned(),
            catalog_key: CatalogKey::new("mame-attribute-guards"),
            catalog_display_name: "MAME attribute guards".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(Fixture {
        _directory: directory,
        path,
        database,
        snapshot: report
            .snapshot_key
            .ok_or("published fixture snapshot missing")?,
    })
}

fn open_connection(path: &Utf8PathBuf) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    Ok(connection)
}

fn machine_id(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    name: &str,
) -> TestResult<i64> {
    Ok(sql_query(
        "SELECT sets.set_id AS value FROM catalog_sets AS sets \
         JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key=? AND sets.set_name=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(name)
    .get_result::<IdRow>(connection)?
    .value)
}

fn rom_occurrence(connection: &mut SqliteConnection, snapshot: &SnapshotKey) -> TestResult<i64> {
    Ok(sql_query(
        "SELECT occurrence.occurrence_id AS value FROM asset_occurrences AS occurrence \
         JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id \
         JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key=? AND occurrence.claim_kind='mame_rom' \
         AND sets.set_name='complete'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IdRow>(connection)?
    .value)
}

fn drop_trigger(connection: &mut SqliteConnection, name: &str) -> TestResult {
    let sql = sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='trigger' AND name=?")
        .bind::<Text, _>(name)
        .get_result::<TextRow>(connection)?
        .value;
    assert!(sql.contains(name), "resolved trigger definition for {name}");
    connection.batch_execute(&format!("DROP TRIGGER {name}"))?;
    Ok(())
}

fn assert_machine_read_fails(fixture: &Fixture) -> TestResult {
    let result = machines_for_snapshot(
        &fixture.database,
        &fixture.snapshot,
        None,
        MachinePageLimit::new(20)?,
    );
    assert!(
        result.is_err(),
        "public machine reads must reject corrupt provenance"
    );
    Ok(())
}

#[test]
fn source_free_public_readers_reject_removed_extra_and_malformed_positions() -> TestResult {
    // Missing native witness: the public machine reader sees the corrupt stored
    // edition without reparsing the XML source.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    drop_trigger(
        &mut connection,
        "mame_machines_attribute_positions_attribute_position_delete",
    )?;
    sql_query("DELETE FROM mame_machines_attribute_positions WHERE set_id=? AND field_kind=0")
        .bind::<BigInt, _>(complete)
        .execute(&mut connection)?;
    drop(connection);
    assert_machine_read_fails(&fixture)?;

    // An otherwise valid closed field code is still extra when its raw source
    // attribute was absent on the owner (the `parent` machine has default false).
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let parent = machine_id(&mut connection, &fixture.snapshot, "parent")?;
    drop_trigger(
        &mut connection,
        "mame_machines_attribute_positions_attribute_position_insert",
    )?;
    sql_query("INSERT INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,3,100,1,1)")
        .bind::<BigInt, _>(parent)
        .execute(&mut connection)?;
    drop(connection);
    assert_machine_read_fails(&fixture)?;

    // Fractional real source coordinates survive SQLite storage when checks are
    // disabled, so readers must validate storage classes at their boundary.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    drop_trigger(
        &mut connection,
        "mame_machines_attribute_positions_attribute_position_update",
    )?;
    connection.batch_execute("PRAGMA ignore_check_constraints=ON")?;
    sql_query("UPDATE mame_machines_attribute_positions SET source_column=1.5 WHERE set_id=? AND field_kind=0")
        .bind::<BigInt, _>(complete)
        .execute(&mut connection)?;
    drop(connection);
    assert_machine_read_fails(&fixture)?;

    // Native and compatibility fields share one lexical ordinal sequence.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    drop_trigger(
        &mut connection,
        "mame_machine_compatibility_attribute_positions_attribute_position_update",
    )?;
    let native_order = sql_query(
        "SELECT source_order AS value FROM mame_machines_attribute_positions \
         WHERE set_id=? AND field_kind=0",
    )
    .bind::<BigInt, _>(complete)
    .get_result::<IdRow>(&mut connection)?
    .value;
    sql_query(
        "UPDATE mame_machine_compatibility_attribute_positions SET source_order=? WHERE set_id=?",
    )
    .bind::<BigInt, _>(native_order)
    .bind::<BigInt, _>(complete)
    .execute(&mut connection)?;
    drop(connection);
    assert_machine_read_fails(&fixture)?;

    // The public file reader validates the same provenance for ROM payloads.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let occurrence = rom_occurrence(&mut connection, &fixture.snapshot)?;
    drop_trigger(
        &mut connection,
        "mame_rom_claims_attribute_positions_attribute_position_delete",
    )?;
    sql_query(
        "DELETE FROM mame_rom_claims_attribute_positions WHERE occurrence_id=? AND field_kind=0",
    )
    .bind::<BigInt, _>(occurrence)
    .execute(&mut connection)?;
    drop(connection);
    assert!(
        catalog_files::occurrences_for_ids(
            &fixture.database,
            &[OccurrenceId::from_database(occurrence)],
        )
        .is_err(),
        "public file reads must reject missing ROM name provenance"
    );
    Ok(())
}

fn remove_publication(connection: &mut SqliteConnection, snapshot: &SnapshotKey) -> TestResult {
    drop_trigger(connection, "snapshot_publications_are_immutable_delete")?;
    sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str())
        .execute(connection)?;
    Ok(())
}

fn republish(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) \
         SELECT catalog_key,document_key,interpretation_key,snapshot_key \
         FROM catalog_snapshots WHERE snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .execute(connection)
}

#[test]
fn publication_rejects_incomplete_or_extra_presence_witnesses() -> TestResult {
    // The control checks the true source-presence case: parent.isdevice defaults
    // false, but no position is expected because the raw XML field was absent.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let parent = machine_id(&mut connection, &fixture.snapshot, "parent")?;
    let raw = sql_query("SELECT is_device AS value FROM mame_machines WHERE set_id=?")
        .bind::<BigInt, _>(parent)
        .get_result::<IdRow>(&mut connection)?
        .value;
    let specified =
        sql_query("SELECT is_device_specified AS value FROM mame_machines WHERE set_id=?")
            .bind::<BigInt, _>(parent)
            .get_result::<IdRow>(&mut connection)?
            .value;
    assert_eq!((raw, specified), (0, 0));
    let has_absent_field_position = sql_query(
        "SELECT COUNT(*) AS value FROM mame_machines_attribute_positions \
         WHERE set_id=? AND field_kind=3",
    )
    .bind::<BigInt, _>(parent)
    .get_result::<IdRow>(&mut connection)?
    .value;
    assert_eq!(has_absent_field_position, 0);

    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    remove_publication(&mut connection, &fixture.snapshot)?;
    drop_trigger(
        &mut connection,
        "mame_machines_attribute_positions_attribute_position_delete",
    )?;
    sql_query("DELETE FROM mame_machines_attribute_positions WHERE set_id=? AND field_kind=0")
        .bind::<BigInt, _>(complete)
        .execute(&mut connection)?;
    assert!(republish(&mut connection, &fixture.snapshot).is_err());

    // Publication also rejects a present witness whose source coordinate is
    // malformed, even when SQLite CHECK constraints are bypassed.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    remove_publication(&mut connection, &fixture.snapshot)?;
    drop_trigger(
        &mut connection,
        "mame_machines_attribute_positions_attribute_position_update",
    )?;
    connection.batch_execute("PRAGMA ignore_check_constraints=ON")?;
    sql_query("UPDATE mame_machines_attribute_positions SET source_line=1.5 WHERE set_id=? AND field_kind=0")
        .bind::<BigInt, _>(complete)
        .execute(&mut connection)?;
    assert!(republish(&mut connection, &fixture.snapshot).is_err());

    // A fabricated witness for the omitted default-false field is rejected as
    // extra even though its key and coordinates are otherwise well formed.
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let parent = machine_id(&mut connection, &fixture.snapshot, "parent")?;
    remove_publication(&mut connection, &fixture.snapshot)?;
    sql_query("INSERT INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,3,100,1,1)")
        .bind::<BigInt, _>(parent)
        .execute(&mut connection)?;
    assert!(republish(&mut connection, &fixture.snapshot).is_err());
    Ok(())
}

#[test]
fn published_positions_are_immutable_even_with_foreign_and_recursive_triggers_off() -> TestResult {
    let fixture = import_fixture()?;
    let mut connection = open_connection(&fixture.path)?;
    let complete = machine_id(&mut connection, &fixture.snapshot, "complete")?;
    let parent = machine_id(&mut connection, &fixture.snapshot, "parent")?;

    assert!(
        sql_query("INSERT INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,100,1,1)")
            .bind::<BigInt, _>(parent)
            .execute(&mut connection)
            .is_err(),
        "published owners must reject new, otherwise valid position rows"
    );

    for sql in [
        "UPDATE mame_machines_attribute_positions SET source_order=100 WHERE set_id=? AND field_kind=0",
        "DELETE FROM mame_machines_attribute_positions WHERE set_id=? AND field_kind=0",
        "INSERT OR REPLACE INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,100,1,1)",
    ] {
        assert!(
            sql_query(sql)
                .bind::<BigInt, _>(complete)
                .execute(&mut connection)
                .is_err(),
            "published position mutation unexpectedly succeeded: {sql}"
        );
    }

    let unique_order = sql_query(
        "SELECT source_order AS value FROM mame_machines_attribute_positions \
         WHERE set_id=? AND field_kind=0",
    )
    .bind::<BigInt, _>(parent)
    .get_result::<IdRow>(&mut connection)?
    .value;
    let alternate_collision = sql_query("INSERT OR REPLACE INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,?,1,1)")
        .bind::<BigInt, _>(parent)
        .bind::<BigInt, _>(unique_order)
        .execute(&mut connection);
    assert!(
        alternate_collision.is_err(),
        "REPLACE must not erase a row through the owner/ordinal unique key"
    );

    let retained = sql_query(
        "SELECT COUNT(*) AS value FROM mame_machines_attribute_positions \
         WHERE set_id=? AND field_kind=0 AND source_order=?",
    )
    .bind::<BigInt, _>(parent)
    .bind::<BigInt, _>(unique_order)
    .get_result::<IdRow>(&mut connection)?
    .value;
    assert_eq!(retained, 1);

    let foreign_keys =
        sql_query("PRAGMA foreign_key_list(machine_switch_value_conditions_attribute_positions)")
            .load::<ForeignKeyRow>(&mut connection)?
            .into_iter()
            .map(|row| (row.table, row.from, row.to))
            .collect::<BTreeSet<_>>();
    assert!(foreign_keys.contains(&(
        "machine_switch_value_conditions".to_owned(),
        "condition_order".to_owned(),
        "condition_order".to_owned(),
    )));
    let violations = sql_query(
        "SELECT COUNT(*) AS value FROM pragma_foreign_key_check \
         WHERE \"table\"='machine_switch_value_conditions_attribute_positions'",
    )
    .get_result::<IdRow>(&mut connection)?
    .value;
    assert_eq!(
        violations, 0,
        "fixture positions must reference actual complete condition rows"
    );
    let complete_conditions = sql_query(
        "SELECT COUNT(*) AS value FROM machine_switch_value_conditions_attribute_positions AS position \
         JOIN machine_switch_value_conditions AS condition USING(set_id,switch_order,value_order,condition_order)",
    )
    .get_result::<IdRow>(&mut connection)?
    .value;
    assert!(
        complete_conditions > 0,
        "condition positions must join their actual condition owners"
    );
    Ok(())
}

#[test]
fn public_history_rejects_fractional_mame_coordinates_and_ordinals() -> TestResult {
    for (column, value) in [("source_column", "1.5"), ("source_order", "0.5")] {
        let fixture = import_fixture()?;
        let mut connection = open_connection(&fixture.path)?;
        drop_trigger(
            &mut connection,
            "mame_document_facts_attribute_positions_attribute_position_update",
        )?;
        connection.batch_execute("PRAGMA ignore_check_constraints=ON")?;
        let sql = format!(
            "UPDATE mame_document_facts_attribute_positions SET {column}={value} \
             WHERE document_id=(SELECT document_id FROM mame_document_facts WHERE snapshot_key=?) \
             AND field_kind=0"
        );
        sql_query(sql)
            .bind::<Text, _>(fixture.snapshot.as_str())
            .execute(&mut connection)?;
        drop(connection);
        assert!(
            app::diff_catalog_snapshots(&fixture.database, &fixture.snapshot, &fixture.snapshot)
                .is_err(),
            "snapshot history must reject fractional {column} without source XML"
        );
    }
    Ok(())
}
