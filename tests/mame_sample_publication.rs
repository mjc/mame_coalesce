#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SEED_XML: &str = "<mame mameconfig='10'><machine name='seed'><description>Seed</description><sample name='seed.wav'/></machine></mame>";
const SHA1: &str = "1111111111111111111111111111111111111111";

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn import_request(path: Utf8PathBuf, format: CatalogDocumentFormat) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format,
        source_key: PublishingSourceKey::new("mame-sample-publication"),
        source_display_name: "MAME sample publication".into(),
        catalog_key: CatalogKey::new("mame-sample-publication"),
        catalog_display_name: "MAME sample publication".into(),
        scope: CatalogScope::Complete,
    }
}

fn seed_database() -> TestResult<(tempfile::TempDir, Utf8PathBuf, String, String)> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("seed.xml"))?;
    std::fs::write(&document_path, SEED_XML)?;
    let report = app::import_catalog(
        &database,
        &import_request(document_path, CatalogDocumentFormat::MameListXml),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("seed snapshot missing")?;

    let cmp_path = Utf8PathBuf::try_from(directory.path().join("seed.cmp"))?;
    std::fs::write(
        &cmp_path,
        "clrmamepro ( name Catalog description Description version v1 date 2024-01-02 author Author email author@example.test homepage https://home.test url https://url.test comment Header category Arcade header DAT forcemerging full forcezipping zip forcepacking split forcenodump required )\nGAME ( NAME set-a DESCRIPTION Description SAMPLE sample.wav ROM ( NAME a.bin SIZE 8 ) )",
    )?;
    let cmp_report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: cmp_path,
            format: CatalogDocumentFormat::ClrMamePro,
            source_key: PublishingSourceKey::new("cmp-sample-format"),
            source_display_name: "CMP sample format".into(),
            catalog_key: CatalogKey::new("cmp-sample-format"),
            catalog_display_name: "CMP sample format".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(cmp_report.status, CatalogImportStatus::Succeeded);
    let cmp_snapshot = cmp_report.snapshot_key.ok_or("CMP snapshot missing")?;
    Ok((
        directory,
        database_path,
        snapshot.to_string(),
        cmp_snapshot.to_string(),
    ))
}

fn pending_machine(
    connection: &mut SqliteConnection,
    base: &str,
    interpretation_base: &str,
    label: &str,
) -> TestResult<(String, i64)> {
    let key = format!("pending-{label}");
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalogs WHERE catalog_key='mame-sample-publication'")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,(SELECT interpretation_key FROM catalog_snapshots WHERE snapshot_key=?),coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .bind::<Text, _>(interpretation_base)
        .bind::<Text, _>(base)
        .execute(connection)?;
    sql_query("INSERT INTO mame_document_facts(snapshot_key,build,debug,debug_specified,config_version,source_line,source_column) SELECT ?,build,debug,debug_specified,config_version,source_line,source_column FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    let group = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         VALUES (?,'root',0) RETURNING set_group_id AS value",
    )
    .bind::<Text, _>(&key)
    .get_result::<IdRow>(connection)?
    .value;
    let owner = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         VALUES (?,'mame_machine',0,'pending',1,1) RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group)
    .get_result::<IdRow>(connection)?
    .value;
    sql_query("INSERT INTO mame_machines(set_id,description,description_source_order,description_line,description_column,is_device,is_device_specified,runnable,runnable_specified,is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,attributes_line,attributes_column) VALUES (?,'Pending',0,1,1,0,0,1,0,0,0,0,0,1,1)")
        .bind::<BigInt, _>(owner)
        .execute(connection)?;
    Ok((key, owner))
}

fn pending_cmp_set(connection: &mut SqliteConnection, base: &str) -> TestResult<(String, i64)> {
    let key = "pending-cmp-sample-owner".to_owned();
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalog_snapshots JOIN catalogs USING(catalog_key) WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    let group = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         VALUES (?,'root',0) RETURNING set_group_id AS value",
    )
    .bind::<Text, _>(&key)
    .get_result::<IdRow>(connection)?
    .value;
    let set_id = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT ?,sets.source_element_kind,sets.list_order,sets.set_name,sets.source_line,sets.source_column FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? RETURNING set_id AS value")
        .bind::<BigInt, _>(group)
        .bind::<Text, _>(base)
        .get_result::<IdRow>(connection)?
        .value;
    sql_query("INSERT INTO cmp_set_facts(record_id,source_block,document_order,description,year,manufacturer,rebuildto,region,release_year_text,release_month_text,release_day_text,serial) SELECT ?,facts.source_block,facts.document_order,facts.description,facts.year,facts.manufacturer,facts.rebuildto,facts.region,facts.release_year_text,facts.release_month_text,facts.release_day_text,facts.serial FROM cmp_set_facts AS facts JOIN catalog_sets AS sets ON sets.set_id=facts.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?")
        .bind::<BigInt, _>(set_id)
        .bind::<Text, _>(base)
        .execute(connection)?;
    Ok((key, set_id))
}

fn insert_sample_occurrence(
    connection: &mut SqliteConnection,
    owner: i64,
    order: i64,
) -> TestResult<i64> {
    Ok(sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES (?,?,'mame_sample',NULL) RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(owner)
    .bind::<BigInt, _>(order)
    .get_result::<IdRow>(connection)?
    .value)
}

fn insert_sample(
    connection: &mut SqliteConnection,
    occurrence: i64,
    name: &str,
    source_order: i64,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO mame_samples(occurrence_id,name,source_order,source_line,source_column) \
         VALUES (?,?,?,?,?)",
    )
    .bind::<BigInt, _>(occurrence)
    .bind::<Text, _>(name)
    .bind::<BigInt, _>(source_order)
    .bind::<BigInt, _>(1)
    .bind::<BigInt, _>(1)
    .execute(connection)
}

fn publish(connection: &mut SqliteConnection, key: &str) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(key)
        .execute(connection)
}

fn assert_publication_rejected(
    connection: &mut SqliteConnection,
    key: &str,
    reason: &str,
) -> TestResult {
    let error = publish(connection, key)
        .err()
        .ok_or_else(|| format!("{reason} was published"))?;
    assert!(
        error.to_string().contains("native MAME publication"),
        "{reason}: {error}"
    );
    Ok(())
}

fn insert_digest_assertion(
    connection: &mut SqliteConnection,
    occurrence: i64,
    digest_hex: &str,
    provenance: &str,
) -> TestResult {
    let digest_bytes = hex::decode(digest_hex)?;
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES ('sha1',?)")
        .bind::<Binary, _>(&digest_bytes)
        .execute(connection)?;
    let digest_id = sql_query(
        "SELECT digest_id AS value FROM digest_values WHERE algorithm='sha1' AND digest=?",
    )
    .bind::<Binary, _>(&digest_bytes)
    .get_result::<IdRow>(connection)?
    .value;
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES (?,?, 'whole_asset', ?)")
        .bind::<BigInt, _>(occurrence)
        .bind::<BigInt, _>(digest_id)
        .bind::<Text, _>(provenance)
        .execute(connection)?;
    Ok(())
}

#[test]
fn mame_sample_publication_requires_native_payload_and_accepts_computed_assertions() -> TestResult {
    let (_directory, database_path, base, _cmp_base) = seed_database()?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;

    let (key, owner) = pending_machine(&mut connection, &base, &base, "missing-payload")?;
    let occurrence = insert_sample_occurrence(&mut connection, owner, 0)?;
    assert_publication_rejected(
        &mut connection,
        &key,
        "sample occurrence without native payload",
    )?;

    insert_sample(&mut connection, occurrence, "pending.wav", 1)?;
    insert_digest_assertion(&mut connection, occurrence, SHA1, "computed")?;
    assert_eq!(publish(&mut connection, &key)?, 1);
    Ok(())
}

#[test]
fn mame_samples_reject_source_declared_digests_but_allow_computed_metadata() -> TestResult {
    let (_directory, database_path, base, _cmp_base) = seed_database()?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;

    let (key, owner) = pending_machine(&mut connection, &base, &base, "declared-digest")?;
    let occurrence = insert_sample_occurrence(&mut connection, owner, 0)?;
    insert_sample(&mut connection, occurrence, "declared.wav", 1)?;
    insert_digest_assertion(&mut connection, occurrence, SHA1, "source_declared")?;
    assert_publication_rejected(&mut connection, &key, "source-declared sample digest")?;

    let (key, owner) = pending_machine(&mut connection, &base, &base, "computed-digest")?;
    let occurrence = insert_sample_occurrence(&mut connection, owner, 0)?;
    insert_sample(&mut connection, occurrence, "computed.wav", 1)?;
    insert_digest_assertion(&mut connection, occurrence, SHA1, "computed")?;
    assert_eq!(publish(&mut connection, &key)?, 1);
    Ok(())
}

#[test]
fn mame_samples_require_mame_sample_occurrences_with_actual_machine_owners() -> TestResult {
    let (_directory, database_path, base, cmp_base) = seed_database()?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let (_key, owner) = pending_machine(&mut connection, &base, &base, "wrong-kind")?;

    let rom_occurrence = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES (?,1,'mame_rom',NULL) RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(owner)
    .get_result::<IdRow>(&mut connection)?
    .value;
    assert!(
        insert_sample(&mut connection, rom_occurrence, "rom-as-sample.wav", 1).is_err(),
        "mame_samples accepted a ROM occurrence"
    );

    let error = pending_machine(&mut connection, &base, &cmp_base, "wrong-format")
        .expect_err("MAME native machine owner was accepted with a CMP interpretation");
    let message = error.to_string();
    assert!(
        message.contains("native") || message.contains("format"),
        "wrong-format owner failed for an unrelated reason: {message}"
    );

    assert!(
        sql_query(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
             VALUES (999999,0,'mame_sample',NULL)",
        )
        .execute(&mut connection)
        .is_err(),
        "asset_occurrences accepted a sample without an actual catalog-set parent"
    );

    let (_cmp_key, cmp_owner) = pending_cmp_set(&mut connection, &cmp_base)?;
    let wrong_owner_error = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES (?,99,'mame_sample',NULL)",
    )
    .bind::<BigInt, _>(cmp_owner)
    .execute(&mut connection)
    .expect_err("unpublished CMP set accepted a MAME sample occurrence");
    assert!(
        wrong_owner_error
            .to_string()
            .contains("occurrence kind does not match its native record"),
        "wrong-owner sample failed for the wrong reason: {wrong_owner_error}"
    );

    let cmp_sample_occurrence = sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) SELECT ?,occurrence.occurrence_order,occurrence.claim_kind,occurrence.content_uuid FROM asset_occurrences AS occurrence JOIN cmp_samples USING(occurrence_id) WHERE occurrence.record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1) RETURNING occurrence_id AS value")
        .bind::<BigInt, _>(cmp_owner)
        .bind::<Text, _>(&cmp_base)
        .get_result::<IdRow>(&mut connection)?
        .value;
    sql_query("INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) SELECT ?,sample_name,source_field,source_order,is_quoted,source_line,source_column FROM cmp_samples WHERE occurrence_id=(SELECT occurrence_id FROM asset_occurrences WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1) AND claim_kind='cmp_sample')")
        .bind::<BigInt, _>(cmp_sample_occurrence)
        .bind::<Text, _>(&cmp_base)
        .execute(&mut connection)?;

    assert!(
        insert_sample(&mut connection, 999_999, "dangling.wav", 0).is_err(),
        "mame_samples accepted a dangling occurrence ID"
    );
    Ok(())
}

#[test]
fn mame_sample_positions_are_unique_across_machine_children() -> TestResult {
    let (_directory, database_path, base, _cmp_base) = seed_database()?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let (key, owner) = pending_machine(&mut connection, &base, &base, "duplicate-child-position")?;
    sql_query("INSERT INTO mame_machine_chips(set_id,element_order,name,kind,source_line,source_column) VALUES (?,1,'cpu','cpu',1,1)")
        .bind::<BigInt, _>(owner)
        .execute(&mut connection)?;
    let occurrence = insert_sample_occurrence(&mut connection, owner, 0)?;
    insert_sample(&mut connection, occurrence, "same-position.wav", 1)?;
    assert_publication_rejected(
        &mut connection,
        &key,
        "duplicate machine-child source position",
    )?;
    Ok(())
}

#[test]
fn published_mame_samples_reject_late_children_and_payload_replacement() -> TestResult {
    let (_directory, database_path, base, _cmp_base) = seed_database()?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let (key, owner) = pending_machine(&mut connection, &base, &base, "immutable")?;
    let occurrence = insert_sample_occurrence(&mut connection, owner, 0)?;
    insert_sample(&mut connection, occurrence, "stable.wav", 1)?;
    assert_eq!(publish(&mut connection, &key)?, 1);

    assert!(
        insert_sample_occurrence(&mut connection, owner, 1).is_err(),
        "a late sample occurrence was accepted"
    );
    assert!(
        sql_query("INSERT OR REPLACE INTO mame_samples(occurrence_id,name,source_order,source_line,source_column) VALUES (?,'replacement.wav',1,1,1)")
            .bind::<BigInt, _>(occurrence)
            .execute(&mut connection)
            .is_err(),
        "INSERT OR REPLACE changed a published sample payload"
    );
    assert!(
        sql_query("UPDATE mame_samples SET name='mutated.wav' WHERE occurrence_id=?")
            .bind::<BigInt, _>(occurrence)
            .execute(&mut connection)
            .is_err(),
        "a published sample payload was mutable"
    );
    assert!(
        sql_query("DELETE FROM mame_samples WHERE occurrence_id=?")
            .bind::<BigInt, _>(occurrence)
            .execute(&mut connection)
            .is_err(),
        "a published sample payload was deletable"
    );
    Ok(())
}
