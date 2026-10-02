use camino::Utf8PathBuf;
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

const SHA1: &str = "1111111111111111111111111111111111111111";
const OTHER_SHA1: &str = "2222222222222222222222222222222222222222";

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct UuidRow {
    #[diesel(sql_type = Binary)]
    value: Vec<u8>,
}

fn request(path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("mame-evidence-publication"),
        source_display_name: "MAME evidence publication test".into(),
        catalog_key: CatalogKey::new("mame-evidence-publication"),
        catalog_display_name: "MAME evidence publication test".into(),
        scope: CatalogScope::Complete,
    }
}

#[test]
fn publication_requires_canonical_mame_digest_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document = Utf8PathBuf::try_from(directory.path().join("seed.xml"))?;
    std::fs::write(
        &document,
        format!(
            "<mame mameconfig='10'><machine name='seed'><description>Seed</description><rom name='seed.bin' size='16' sha1='{SHA1}'/><disk name='seed.chd' sha1='{OTHER_SHA1}'/></machine></mame>"
        ),
    )?;
    let report = app::import_catalog(&database, &request(document))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let base = report.snapshot_key.ok_or("seed snapshot missing")?;

    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let canonical_uuid = sql_query(
        "SELECT occurrence.content_uuid AS value FROM asset_occurrences AS occurrence \
         JOIN mame_rom_claims AS rom USING (occurrence_id) \
         WHERE rom.name='seed.bin'",
    )
    .get_result::<UuidRow>(&mut connection)?
    .value;
    let reserved_digest_id =
        sql_query("SELECT count(*) AS value FROM digest_values WHERE digest_id=999")
            .get_result::<IdRow>(&mut connection)?
            .value;
    assert_eq!(
        reserved_digest_id, 0,
        "digest id 999 must be dangling in this fixture"
    );

    assert_rom_source_evidence_required(&mut connection, base.as_str())?;
    assert_rom_computed_evidence_allowed(&mut connection, base.as_str(), &canonical_uuid)?;
    assert_rom_dangling_evidence_rejected(&mut connection, base.as_str())?;
    assert_disk_publication_rejections(&mut connection, base.as_str(), &canonical_uuid)?;
    assert_disk_unknown_scope_allowed(&mut connection, base.as_str())?;
    Ok(())
}

fn assert_rom_source_evidence_required(
    connection: &mut SqliteConnection,
    base: &str,
) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "rom-missing", true)?;
    insert_rom(connection, owner, "missing", None, "whole_asset")?;
    assert_native_mame_publication_rejects(
        connection,
        &key,
        "ROM literal without source assertion",
    )?;

    let (key, owner) = pending_machine(connection, base, "rom-extra-source", true)?;
    let occurrence = insert_rom(connection, owner, "extra-source", None, "whole_asset")?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "whole_asset",
        "source_declared",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        OTHER_SHA1,
        "whole_asset",
        "source_declared",
    )?;
    assert_native_mame_publication_rejects(connection, &key, "unbacked ROM source hash")?;

    let (key, owner) = pending_machine(connection, base, "rom-wrong-scope", true)?;
    let occurrence = insert_rom(connection, owner, "wrong-scope", None, "whole_asset")?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "whole_asset",
        "source_declared",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "whole_file",
        "source_declared",
    )?;
    assert_native_mame_publication_rejects(connection, &key, "ROM assertion at wrong scope")?;

    let (key, owner) = pending_machine(connection, base, "rom-computed-only", true)?;
    let occurrence = insert_rom(connection, owner, "computed-only", None, "whole_asset")?;
    insert_digest_assertion(connection, occurrence, SHA1, "whole_asset", "computed")?;
    assert_native_mame_publication_rejects(
        connection,
        &key,
        "computed ROM assertion standing in for source evidence",
    )?;
    Ok(())
}

fn assert_rom_computed_evidence_allowed(
    connection: &mut SqliteConnection,
    base: &str,
    canonical_uuid: &[u8],
) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "rom-positive", true)?;
    let occurrence = insert_rom(
        connection,
        owner,
        "positive",
        Some(canonical_uuid),
        "whole_asset",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "whole_asset",
        "source_declared",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        OTHER_SHA1,
        "whole_asset",
        "computed",
    )?;
    assert_eq!(publish_pending(connection, &key)?, 1);
    Ok(())
}

fn assert_rom_dangling_evidence_rejected(
    connection: &mut SqliteConnection,
    base: &str,
) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "rom-dangling", true)?;
    let occurrence = insert_rom(connection, owner, "dangling", None, "whole_asset")?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "whole_asset",
        "source_declared",
    )?;
    insert_dangling_assertion(connection, occurrence, "whole_asset")?;
    assert_native_mame_publication_rejects(connection, &key, "dangling ROM assertion")?;
    Ok(())
}

fn assert_disk_publication_rejections(
    connection: &mut SqliteConnection,
    base: &str,
    canonical_uuid: &[u8],
) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "disk-uuid", true)?;
    let occurrence = insert_disk(
        connection,
        owner,
        "linked-disk",
        Some(canonical_uuid),
        "chd_header_sha1",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "chd_header_sha1",
        "source_declared",
    )?;
    assert_native_mame_publication_rejects(connection, &key, "disk content UUID")?;

    let (key, owner) = pending_machine(connection, base, "disk-dangling", true)?;
    let occurrence = insert_disk(connection, owner, "dangling-disk", None, "chd_header_sha1")?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "chd_header_sha1",
        "source_declared",
    )?;
    insert_dangling_assertion(connection, occurrence, "chd_header_sha1")?;
    assert_native_mame_publication_rejects(connection, &key, "dangling disk assertion")?;

    let (key, owner) = pending_machine(connection, base, "disk-missing", true)?;
    insert_disk(connection, owner, "missing-source", None, "chd_header_sha1")?;
    assert_native_mame_publication_rejects(
        connection,
        &key,
        "valid disk SHA1 literal without source assertion",
    )?;

    let (key, owner) = pending_machine(connection, base, "disk-wrong", true)?;
    let occurrence = insert_disk(connection, owner, "wrong-source", None, "chd_header_sha1")?;
    insert_digest_assertion(
        connection,
        occurrence,
        SHA1,
        "chd_header_sha1",
        "source_declared",
    )?;
    insert_digest_assertion(
        connection,
        occurrence,
        OTHER_SHA1,
        "chd_header_sha1",
        "source_declared",
    )?;
    assert_native_mame_publication_rejects(
        connection,
        &key,
        "disk source assertion with wrong SHA1",
    )?;
    Ok(())
}

fn assert_disk_unknown_scope_allowed(connection: &mut SqliteConnection, base: &str) -> TestResult {
    let (key, owner) = pending_machine(connection, base, "disk-unknown", true)?;
    let occurrence = insert_disk(connection, owner, "unknown", None, "unknown")?;
    insert_digest_assertion(connection, occurrence, SHA1, "unknown", "source_declared")?;
    assert_eq!(publish_pending(connection, &key)?, 1);
    Ok(())
}

fn pending_machine(
    connection: &mut SqliteConnection,
    base: &str,
    label: &str,
    header: bool,
) -> TestResult<(String, i64)> {
    let key = format!("pending-{label}");
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalogs WHERE catalog_key='mame-evidence-publication'")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    if header {
        sql_query("INSERT INTO mame_document_facts(snapshot_key,build,debug,debug_specified,config_version,source_line,source_column) SELECT ?,build,debug,debug_specified,config_version,source_line,source_column FROM mame_document_facts WHERE snapshot_key=?")
            .bind::<Text, _>(&key)
            .bind::<Text, _>(base)
            .execute(connection)?;
    }
    let group = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0) \
         RETURNING set_group_id AS value",
    )
    .bind::<Text, _>(&key)
    .get_result::<IdRow>(connection)?
    .value;
    let owner = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES (?,'mame_machine',0,'pending',1,1) RETURNING set_id AS value")
        .bind::<BigInt, _>(group)
        .get_result::<IdRow>(connection)?
        .value;
    sql_query("INSERT INTO mame_machines(set_id,description,description_source_order,description_line,description_column,is_device,is_device_specified,runnable,runnable_specified,is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,attributes_line,attributes_column) VALUES (?,'Pending',0,1,1,0,0,1,0,0,0,0,0,1,1)")
        .bind::<BigInt, _>(owner)
        .execute(connection)?;
    Ok((key, owner))
}

fn insert_rom(
    connection: &mut SqliteConnection,
    owner: i64,
    name: &str,
    content_uuid: Option<&[u8]>,
    scope: &str,
) -> TestResult<i64> {
    let occurrence = insert_occurrence(connection, owner, "mame_rom", content_uuid)?;
    sql_query("INSERT INTO mame_rom_claims(occurrence_id,name,sha1_text,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,optional,optional_specified,source_line,source_column) VALUES (?,?,? ,?,'source_declared','good',0,1,0,0,2,1)")
        .bind::<BigInt, _>(occurrence)
        .bind::<Text, _>(name)
        .bind::<Text, _>(SHA1)
        .bind::<Text, _>(scope)
        .execute(connection)?;
    Ok(occurrence)
}

fn insert_disk(
    connection: &mut SqliteConnection,
    owner: i64,
    name: &str,
    content_uuid: Option<&[u8]>,
    scope: &str,
) -> TestResult<i64> {
    let occurrence = insert_occurrence(connection, owner, "mame_disk", content_uuid)?;
    sql_query("INSERT INTO mame_disk_claims(occurrence_id,name,sha1_text,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,optional,optional_specified,writable,writable_specified,source_line,source_column) VALUES (?,?,? ,?,'source_declared','good',0,1,0,0,0,0,2,1)")
        .bind::<BigInt, _>(occurrence)
        .bind::<Text, _>(name)
        .bind::<Text, _>(SHA1)
        .bind::<Text, _>(scope)
        .execute(connection)?;
    Ok(occurrence)
}

fn insert_occurrence(
    connection: &mut SqliteConnection,
    owner: i64,
    kind: &str,
    content_uuid: Option<&[u8]>,
) -> TestResult<i64> {
    Ok(sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) VALUES (?,0,?,?) RETURNING occurrence_id AS value")
        .bind::<BigInt, _>(owner)
        .bind::<Text, _>(kind)
        .bind::<Nullable<Binary>, _>(content_uuid)
        .get_result::<IdRow>(connection)?
        .value)
}

fn insert_digest_assertion(
    connection: &mut SqliteConnection,
    occurrence: i64,
    value: &str,
    scope: &str,
    provenance: &str,
) -> TestResult {
    let bytes = hex::decode(value)?;
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES ('sha1',?)")
        .bind::<Binary, _>(&bytes)
        .execute(connection)?;
    let digest = sql_query(
        "SELECT digest_id AS value FROM digest_values WHERE algorithm='sha1' AND digest=?",
    )
    .bind::<Binary, _>(&bytes)
    .get_result::<IdRow>(connection)?
    .value;
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES (?,?,?,?)")
        .bind::<BigInt, _>(occurrence)
        .bind::<BigInt, _>(digest)
        .bind::<Text, _>(scope)
        .bind::<Text, _>(provenance)
        .execute(connection)?;
    Ok(())
}

fn insert_dangling_assertion(
    connection: &mut SqliteConnection,
    occurrence: i64,
    scope: &str,
) -> TestResult {
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES (?,999,?,'source_declared')")
        .bind::<BigInt, _>(occurrence)
        .bind::<Text, _>(scope)
        .execute(connection)?;
    Ok(())
}

fn assert_native_mame_publication_rejects(
    connection: &mut SqliteConnection,
    key: &str,
    case: &str,
) -> TestResult {
    let error = publish_pending(connection, key)
        .err()
        .ok_or_else(|| format!("{case} passed MAME publication"))?;
    assert!(
        error.to_string().contains("native MAME publication"),
        "{case}: {error}"
    );
    Ok(())
}

fn publish_pending(connection: &mut SqliteConnection, key: &str) -> QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(key)
        .execute(connection)
}
