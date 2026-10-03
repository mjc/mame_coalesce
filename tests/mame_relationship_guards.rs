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

const SEED_XML: &str =
    "<mame mameconfig='10'><machine name='seed'><description>Seed</description></machine></mame>";

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn seed_database() -> TestResult<(tempfile::TempDir, Utf8PathBuf, String)> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("seed.xml"))?;
    std::fs::write(&document_path, SEED_XML)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("mame-relationship-guards"),
            source_display_name: "MAME relationship guards".into(),
            catalog_key: CatalogKey::new("mame-relationship-guards"),
            catalog_display_name: "MAME relationship guards".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("seed snapshot missing")?;
    Ok((directory, database_path, snapshot.to_string()))
}

fn open_connection(database_path: &Utf8PathBuf) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    Ok(connection)
}

fn pending_snapshot(
    connection: &mut SqliteConnection,
    base: &str,
    label: &str,
) -> TestResult<String> {
    let key = format!("pending-{label}");
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT ?,source_key,? FROM catalogs WHERE catalog_key='mame-relationship-guards'")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT ?,?,document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    sql_query("INSERT INTO mame_document_facts(snapshot_key,build,debug,debug_specified,config_version,source_line,source_column) SELECT ?,build,debug,debug_specified,config_version,source_line,source_column FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(base)
        .execute(connection)?;
    sql_query("INSERT INTO mame_document_facts_attribute_positions(document_id,field_kind,source_order,source_line,source_column) SELECT document_id,2,0,1,7 FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text, _>(&key)
        .execute(connection)?;
    Ok(key)
}

fn pending_machine(
    connection: &mut SqliteConnection,
    base: &str,
    label: &str,
) -> TestResult<(String, i64)> {
    let key = pending_snapshot(connection, base, label)?;
    let group = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         VALUES (?,'root',0) RETURNING set_group_id AS value",
    )
    .bind::<Text, _>(&key)
    .get_result::<IdRow>(connection)?
    .value;
    let owner = insert_machine(connection, group, 0, "pending")?;
    Ok((key, owner))
}

fn insert_machine(
    connection: &mut SqliteConnection,
    group: i64,
    order: i64,
    name: &str,
) -> TestResult<i64> {
    let owner = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         VALUES (?,'mame_machine',?,?,1,1) RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group)
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(name)
    .get_result::<IdRow>(connection)?
    .value;
    sql_query("INSERT INTO mame_machines(set_id,description,description_source_order,description_line,description_column,is_device,is_device_specified,runnable,runnable_specified,is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,attributes_line,attributes_column) VALUES (?,'Pending',0,1,1,0,0,1,0,0,0,0,0,1,1)")
        .bind::<BigInt, _>(owner)
        .execute(connection)?;
    sql_query("INSERT INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES (?,0,0,1,1)")
        .bind::<BigInt, _>(owner)
        .execute(connection)?;
    Ok(owner)
}

fn second_machine(connection: &mut SqliteConnection, snapshot: &str) -> TestResult<i64> {
    let group = sql_query(
        "SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key=? AND kind='root'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<IdRow>(connection)?
    .value;
    insert_machine(connection, group, 1, "pending-second")
}

fn insert_identity(
    connection: &mut SqliteConnection,
    snapshot: &str,
    key: &str,
) -> TestResult<i64> {
    Ok(sql_query(
        "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) \
         VALUES (?,'source',?) RETURNING relationship_id AS value",
    )
    .bind::<Text, _>(key)
    .bind::<Text, _>(snapshot)
    .get_result::<IdRow>(connection)?
    .value)
}

fn insert_reported(
    connection: &mut SqliteConnection,
    identity: i64,
    kind: &str,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) \
         VALUES (?,?)",
    )
    .bind::<BigInt, _>(identity)
    .bind::<Text, _>(kind)
    .execute(connection)
}

#[allow(clippy::expect_used)]
fn insert_link(
    connection: &mut SqliteConnection,
    owner: i64,
    kind: &str,
    target: &str,
    identity: i64,
) -> diesel::QueryResult<usize> {
    let inserted = sql_query(
        "INSERT INTO mame_machine_links(set_id,link_kind,target_name,relationship_id,source_line,source_column) \
         VALUES (?,?,?,?,1,1)",
    )
    .bind::<BigInt, _>(owner)
    .bind::<Text, _>(kind)
    .bind::<Text, _>(target)
    .bind::<BigInt, _>(identity)
    .execute(connection)?;
    let field = match kind {
        "cloneof" => 6_i64,
        "romof" => 7,
        "sampleof" => 8,
        _ => return Err(diesel::result::Error::NotFound),
    };
    sql_query("INSERT INTO mame_machines_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES (?,?,?,1,?)")
        .bind::<BigInt, _>(owner).bind::<BigInt, _>(field)
        .bind::<BigInt, _>(field - 5).bind::<BigInt, _>(field).execute(connection)
        .expect("successful native link must accept its attribute witness");
    Ok(inserted)
}

#[allow(clippy::expect_used)]
fn insert_device_reference(
    connection: &mut SqliteConnection,
    owner: i64,
    order: i64,
    name: &str,
    tag: &str,
    identity: i64,
) -> diesel::QueryResult<usize> {
    let inserted = sql_query(
        "INSERT INTO mame_device_references(set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) \
         VALUES (?,?,?,?,?,?,1,1)",
    )
    .bind::<BigInt, _>(owner)
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(name)
    .bind::<Text, _>(tag)
    .bind::<BigInt, _>(order + 1)
    .bind::<BigInt, _>(identity)
    .execute(connection)?;
    sql_query("INSERT INTO mame_device_references_attribute_positions(set_id,reference_order,field_kind,source_order,source_line,source_column) VALUES (?,?,1,0,1,1),(?,?,0,1,1,2)")
        .bind::<BigInt, _>(owner).bind::<BigInt, _>(order)
        .bind::<BigInt, _>(owner).bind::<BigInt, _>(order).execute(connection)
        .expect("successful native device reference must accept its attribute witnesses");
    Ok(inserted)
}

fn insert_media_claim(
    connection: &mut SqliteConnection,
    owner: i64,
    order: i64,
    kind: &str,
) -> TestResult<i64> {
    insert_media_claim_at_source_order(connection, owner, order, kind, order + 1)
}

fn insert_media_claim_at_source_order(
    connection: &mut SqliteConnection,
    owner: i64,
    order: i64,
    kind: &str,
    source_order: i64,
) -> TestResult<i64> {
    let claim_kind = format!("mame_{kind}");
    let occurrence = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) \
         VALUES (?,?,?) RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(owner)
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(&claim_kind)
    .get_result::<IdRow>(connection)?
    .value;
    match kind {
        "rom" => {
            sql_query("INSERT INTO mame_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,source_line,source_column,optional,optional_specified) VALUES (?,'rom.bin','whole_asset','source_declared','good',0,?,1,1,0,0)")
                .bind::<BigInt, _>(occurrence)
                .bind::<BigInt, _>(source_order)
                .execute(connection)?;
        }
        "disk" => {
            sql_query("INSERT INTO mame_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,source_order,source_line,source_column,optional,optional_specified,writable,writable_specified) VALUES (?,'disk.chd','chd_header_sha1','source_declared','good',0,?,1,1,0,0,0,0)")
                .bind::<BigInt, _>(occurrence)
                .bind::<BigInt, _>(source_order)
                .execute(connection)?;
        }
        _ => return Err(format!("unsupported media kind: {kind}").into()),
    }
    sql_query(format!("INSERT INTO mame_{kind}_claims_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES (?,0,0,1,1)"))
        .bind::<BigInt, _>(occurrence).execute(connection)?;
    Ok(occurrence)
}

#[allow(clippy::expect_used)]
fn insert_native_merge(
    connection: &mut SqliteConnection,
    kind: &str,
    occurrence: i64,
    identity: i64,
    name: &str,
) -> diesel::QueryResult<usize> {
    let (table, field) = match kind {
        "rom" => ("mame_rom_merges", 5_i64),
        "disk" => ("mame_disk_merges", 2_i64),
        _ => return Err(diesel::result::Error::NotFound),
    };
    let inserted = sql_query(format!(
        "INSERT INTO {table}(occurrence_id,relationship_id,merge_name,source_line,source_column) \
         VALUES (?,?,?,1,1)"
    ))
    .bind::<BigInt, _>(occurrence)
    .bind::<BigInt, _>(identity)
    .bind::<Text, _>(name)
    .execute(connection)?;
    sql_query(format!("INSERT INTO mame_{kind}_claims_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES (?,?,1,1,2)"))
        .bind::<BigInt, _>(occurrence).bind::<BigInt, _>(field).execute(connection)
        .expect("successful native merge must accept its attribute witness");
    Ok(inserted)
}

fn publish(connection: &mut SqliteConnection, snapshot: &str) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot)
        .execute(connection)
}

fn assert_rejected(
    result: diesel::QueryResult<usize>,
    reason: &str,
    expected_error: &str,
) -> TestResult {
    let error = result
        .err()
        .ok_or_else(|| format!("{reason} unexpectedly succeeded"))?;
    assert!(
        error.to_string().contains(expected_error),
        "{reason}: expected error containing {expected_error:?}, got {error}"
    );
    Ok(())
}

#[test]
fn publication_requires_registry_reported_kind_and_native_owner() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;

    let (identity_only, _) = pending_machine(&mut connection, &base, "identity-only")?;
    insert_identity(&mut connection, &identity_only, "identity-only-key")?;
    assert_rejected(
        publish(&mut connection, &identity_only),
        "registry identity without a reported kind",
        "reported source relationships require complete native identity ownership",
    )?;

    let (reported_only, _) = pending_machine(&mut connection, &base, "reported-only")?;
    let reported_id = insert_identity(&mut connection, &reported_only, "reported-only-key")?;
    insert_reported(&mut connection, reported_id, "mame_cloneof")?;
    assert_rejected(
        publish(&mut connection, &reported_only),
        "reported kind without a native owner",
        "reported source relationships require complete native identity ownership",
    )?;

    let (complete, owner) = pending_machine(&mut connection, &base, "complete")?;
    let complete_id = insert_identity(&mut connection, &complete, "complete-key")?;
    insert_reported(&mut connection, complete_id, "mame_cloneof")?;
    insert_link(&mut connection, owner, "cloneof", "parent", complete_id)?;
    assert_eq!(publish(&mut connection, &complete)?, 1);
    Ok(())
}

#[test]
fn native_owner_must_match_reported_kind_and_exact_unpublished_edition() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "owner-validation")?;
    let (other_snapshot, other_edition_owner) =
        pending_machine(&mut connection, &base, "other-edition")?;

    let wrong_kind = insert_identity(&mut connection, &snapshot, "wrong-kind")?;
    insert_reported(&mut connection, wrong_kind, "mame_romof")?;
    assert_rejected(
        insert_link(&mut connection, owner, "cloneof", "parent", wrong_kind),
        "link kind differing from its reported source kind",
        "MAME link requires an unused source identity",
    )?;

    let foreign_edition = insert_identity(&mut connection, &snapshot, "foreign-edition")?;
    insert_reported(&mut connection, foreign_edition, "mame_cloneof")?;
    assert_rejected(
        insert_link(
            &mut connection,
            other_edition_owner,
            "cloneof",
            "parent",
            foreign_edition,
        ),
        "owner from another snapshot edition",
        "MAME link requires an unused source identity",
    )?;

    let orphan_identity = insert_identity(&mut connection, &snapshot, "orphan-owner")?;
    insert_reported(&mut connection, orphan_identity, "mame_cloneof")?;
    assert_rejected(
        insert_link(
            &mut connection,
            i64::MAX,
            "cloneof",
            "parent",
            orphan_identity,
        ),
        "orphan native owner ID",
        "MAME link requires an unused source identity",
    )?;

    let (valid_snapshot, valid_owner) = pending_machine(&mut connection, &base, "valid-owner")?;
    let valid_identity = insert_identity(&mut connection, &valid_snapshot, "valid-owner")?;
    insert_reported(&mut connection, valid_identity, "mame_cloneof")?;
    insert_link(
        &mut connection,
        valid_owner,
        "cloneof",
        "parent",
        valid_identity,
    )?;
    assert_eq!(publish(&mut connection, &valid_snapshot)?, 1);
    assert_eq!(publish(&mut connection, &other_snapshot)?, 1);
    Ok(())
}

#[test]
fn one_source_identity_cannot_be_reused_across_native_relationships_or_owners() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "identity-reuse")?;
    let other_owner = second_machine(&mut connection, &snapshot)?;

    let cross_family = insert_identity(&mut connection, &snapshot, "cross-family")?;
    insert_reported(&mut connection, cross_family, "mame_cloneof")?;
    insert_link(&mut connection, owner, "cloneof", "parent", cross_family)?;
    assert_rejected(
        insert_device_reference(&mut connection, owner, 0, "chip", "@chip", cross_family),
        "identity reused by a device reference after a link",
        "MAME device reference requires an unused source identity",
    )?;

    let second_owner_link = insert_identity(&mut connection, &snapshot, "second-owner-link")?;
    insert_reported(&mut connection, second_owner_link, "mame_romof")?;
    insert_link(&mut connection, owner, "romof", "parent", second_owner_link)?;
    assert_rejected(
        insert_link(
            &mut connection,
            other_owner,
            "romof",
            "parent",
            second_owner_link,
        ),
        "link identity reused by another machine owner",
        "MAME link requires an unused source identity",
    )?;

    let second_owner_device = insert_identity(&mut connection, &snapshot, "second-owner-device")?;
    insert_reported(&mut connection, second_owner_device, "mame_device_ref")?;
    insert_device_reference(
        &mut connection,
        owner,
        0,
        "chip",
        "@chip",
        second_owner_device,
    )?;
    assert_rejected(
        insert_device_reference(
            &mut connection,
            other_owner,
            0,
            "other-chip",
            "@other",
            second_owner_device,
        ),
        "device identity reused by another machine owner",
        "MAME device reference requires an unused source identity",
    )?;

    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    Ok(())
}

#[test]
fn source_and_manual_assertions_share_one_non_shadowable_identity_registry() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, _) = pending_machine(&mut connection, &base, "key-shadow")?;

    sql_query(
        "INSERT INTO catalog_relationships(assertion_key,origin) VALUES ('assertion-first','user')",
    )
    .execute(&mut connection)?;
    assert_rejected(
        sql_query("INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) VALUES ('assertion-first','derived',NULL)")
            .execute(&mut connection),
        "once-issued user identity cannot be reissued as derived",
        "catalog relationship identity",
    )?;

    insert_identity(&mut connection, &snapshot, "registry-first")?;
    assert_rejected(
        sql_query("INSERT INTO relationship_assertions(assertion_key,relation_type,origin,subject_kind,generic_subject_a,generic_subject_b,target_kind,generic_target_a,generic_target_b) VALUES ('registry-first','runtime_dependency','user_conclusion','external_record','registry-test','left','external_record','registry-test','right')")
            .execute(&mut connection),
        "general assertion key already owned by the registry",
        "relationship assertions are immutable",
    )?;
    Ok(())
}

#[test]
fn published_relationship_records_reject_late_writes_mutations_and_replacement() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "immutability")?;
    let link_id = insert_identity(&mut connection, &snapshot, "published-link")?;
    insert_reported(&mut connection, link_id, "mame_cloneof")?;
    insert_link(&mut connection, owner, "cloneof", "parent", link_id)?;
    let device_id = insert_identity(&mut connection, &snapshot, "published-device")?;
    insert_reported(&mut connection, device_id, "mame_device_ref")?;
    insert_device_reference(&mut connection, owner, 0, "chip", "@chip", device_id)?;
    assert_eq!(publish(&mut connection, &snapshot)?, 1);

    assert_rejected(
        sql_query("INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) VALUES ('late-registry','source',?)")
            .bind::<Text, _>(&snapshot)
            .execute(&mut connection),
        "late registry insert into a published snapshot",
        "catalog relationship identity",
    )?;
    assert_rejected(
        sql_query(
            "UPDATE catalog_relationships SET assertion_key='changed' WHERE relationship_id=?",
        )
        .bind::<BigInt, _>(link_id)
        .execute(&mut connection),
        "published registry update",
        "identities are immutable",
    )?;
    assert_rejected(
        sql_query("DELETE FROM catalog_relationships WHERE relationship_id=?")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "published registry delete",
        "identities are immutable",
    )?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key) VALUES (?,'published-link','source',?)")
            .bind::<BigInt, _>(link_id)
            .bind::<Text, _>(&snapshot)
            .execute(&mut connection),
        "registry replacement of a published identity",
        "catalog relationship identity",
    )?;

    assert_rejected(
        insert_reported(&mut connection, i64::MAX, "mame_cloneof"),
        "late reported-kind insert without an identity",
        "reported relationship requires an unused unpublished source identity",
    )?;
    assert_rejected(
        sql_query("UPDATE reported_catalog_relationships SET source_reference_kind='mame_romof' WHERE relationship_id=?")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "published reported-kind update",
        "reported catalog relationships are immutable",
    )?;
    assert_rejected(
        sql_query("DELETE FROM reported_catalog_relationships WHERE relationship_id=?")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "published reported-kind delete",
        "reported catalog relationships are immutable",
    )?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (?,'mame_cloneof')")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "reported-kind replacement after publication",
        "reported relationship requires an unused unpublished source identity",
    )?;

    assert_rejected(
        insert_link(&mut connection, owner, "romof", "parent", i64::MAX),
        "native-link insert without an issued identity",
        "MAME link requires an unused source identity",
    )?;
    assert_rejected(
        sql_query("UPDATE mame_machine_links SET target_name='changed' WHERE relationship_id=?")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "published native-link update",
        "MAME machine links are immutable",
    )?;
    assert_rejected(
        sql_query("DELETE FROM mame_machine_links WHERE relationship_id=?")
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "published native-link delete",
        "MAME machine links are immutable",
    )?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO mame_machine_links(set_id,link_kind,target_name,relationship_id,source_line,source_column) VALUES (?,'cloneof','replacement',?,1,1)")
            .bind::<BigInt, _>(owner)
            .bind::<BigInt, _>(link_id)
            .execute(&mut connection),
        "native-link replacement after publication",
        "MAME link requires an unused source identity",
    )?;

    assert_published_device_immutable(&mut connection, owner, device_id)?;
    Ok(())
}

fn assert_published_device_immutable(
    connection: &mut SqliteConnection,
    owner: i64,
    device_id: i64,
) -> TestResult {
    assert_rejected(
        insert_device_reference(connection, owner, 1, "late", "@late", i64::MAX),
        "native-device insert without an issued identity",
        "MAME device reference requires an unused source identity",
    )?;
    assert_rejected(
        sql_query("UPDATE mame_device_references SET tag='@changed' WHERE relationship_id=?")
            .bind::<BigInt, _>(device_id)
            .execute(connection),
        "published native-device update",
        "MAME device references are immutable",
    )?;
    assert_rejected(
        sql_query("DELETE FROM mame_device_references WHERE relationship_id=?")
            .bind::<BigInt, _>(device_id)
            .execute(connection),
        "published native-device delete",
        "MAME device references are immutable",
    )?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO mame_device_references(set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) VALUES (?,0,'replacement','@replacement',0,?,1,1)")
            .bind::<BigInt, _>(owner)
            .bind::<BigInt, _>(device_id)
            .execute(connection),
        "native-device replacement after publication",
        "MAME device reference requires an unused source identity",
    )?;
    Ok(())
}

#[test]
fn native_merge_owners_require_exact_issued_kind_edition_and_media_claim() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "merge-owner")?;
    let (other_snapshot, other_owner) =
        pending_machine(&mut connection, &base, "merge-other-edition")?;
    let rom = insert_media_claim(&mut connection, owner, 0, "rom")?;
    let disk = insert_media_claim(&mut connection, owner, 1, "disk")?;
    let foreign_rom = insert_media_claim(&mut connection, other_owner, 0, "rom")?;

    assert!(insert_native_merge(&mut connection, "rom", rom, i64::MAX, "missing.bin").is_err());

    let wrong_kind = insert_identity(&mut connection, &snapshot, "merge-wrong-kind")?;
    insert_reported(&mut connection, wrong_kind, "mame_disk_merge")?;
    assert!(insert_native_merge(&mut connection, "rom", rom, wrong_kind, "wrong.bin").is_err());

    let foreign = insert_identity(&mut connection, &snapshot, "merge-cross-edition")?;
    insert_reported(&mut connection, foreign, "mame_rom_merge")?;
    assert!(
        insert_native_merge(&mut connection, "rom", foreign_rom, foreign, "foreign.bin").is_err()
    );

    let wrong_payload = insert_identity(&mut connection, &snapshot, "merge-wrong-payload")?;
    insert_reported(&mut connection, wrong_payload, "mame_rom_merge")?;
    assert!(
        insert_native_merge(&mut connection, "rom", disk, wrong_payload, "disk-as-rom").is_err()
    );

    let rom_identity = insert_identity(&mut connection, &snapshot, "merge-valid-rom")?;
    insert_reported(&mut connection, rom_identity, "mame_rom_merge")?;
    insert_native_merge(&mut connection, "rom", rom, rom_identity, "")?;
    assert!(insert_native_merge(&mut connection, "disk", disk, rom_identity, "reuse.chd").is_err());

    let disk_identity = insert_identity(&mut connection, &snapshot, "merge-valid-disk")?;
    insert_reported(&mut connection, disk_identity, "mame_disk_merge")?;
    insert_native_merge(&mut connection, "disk", disk, disk_identity, "")?;

    let (unclosed_snapshot, _) =
        pending_machine(&mut connection, &base, "merge-publication-unclosed")?;
    let unclosed_identity = insert_identity(&mut connection, &unclosed_snapshot, "unclosed-merge")?;
    insert_reported(&mut connection, unclosed_identity, "mame_rom_merge")?;
    assert!(publish(&mut connection, &unclosed_snapshot).is_err());

    let (closed_snapshot, closed_owner) =
        pending_machine(&mut connection, &base, "merge-publication-closure")?;
    let closed_rom = insert_media_claim(&mut connection, closed_owner, 0, "rom")?;
    let closed_identity = insert_identity(&mut connection, &closed_snapshot, "merge-closed")?;
    insert_reported(&mut connection, closed_identity, "mame_rom_merge")?;
    insert_native_merge(
        &mut connection,
        "rom",
        closed_rom,
        closed_identity,
        "closed.bin",
    )?;
    assert_eq!(publish(&mut connection, &closed_snapshot)?, 1);
    assert!(publish(&mut connection, &other_snapshot).is_ok());
    Ok(())
}

#[test]
fn published_native_merges_reject_late_facts_mutations_deletes_and_replacements() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "merge-immutable")?;
    let rom = insert_media_claim(&mut connection, owner, 0, "rom")?;
    let rom_identity = insert_identity(&mut connection, &snapshot, "published-rom-merge")?;
    insert_reported(&mut connection, rom_identity, "mame_rom_merge")?;
    insert_native_merge(&mut connection, "rom", rom, rom_identity, "parent.bin")?;
    let disk = insert_media_claim(&mut connection, owner, 1, "disk")?;
    let disk_identity = insert_identity(&mut connection, &snapshot, "published-disk-merge")?;
    insert_reported(&mut connection, disk_identity, "mame_disk_merge")?;
    insert_native_merge(&mut connection, "disk", disk, disk_identity, "parent.chd")?;
    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;

    assert!(insert_identity(&mut connection, &snapshot, "late-merge-identity").is_err());
    assert!(insert_native_merge(&mut connection, "rom", rom, i64::MAX, "late.bin").is_err());
    assert!(insert_native_merge(&mut connection, "disk", disk, i64::MAX, "late.chd").is_err());
    assert!(
        sql_query("UPDATE mame_rom_merges SET merge_name='changed.bin' WHERE relationship_id=?")
            .bind::<BigInt, _>(rom_identity)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query("DELETE FROM mame_rom_merges WHERE relationship_id=?")
            .bind::<BigInt, _>(rom_identity)
            .execute(&mut connection)
            .is_err()
    );
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO mame_rom_merges(occurrence_id,relationship_id,merge_name,source_line,source_column) VALUES (?,?,'replacement.bin',1,1)")
            .bind::<BigInt, _>(rom)
            .bind::<BigInt, _>(rom_identity)
            .execute(&mut connection),
        "native ROM merge replacement",
        "MAME ROM merge requires an unused source identity",
    )?;
    assert!(
        sql_query("UPDATE mame_disk_merges SET merge_name='changed.chd' WHERE relationship_id=?")
            .bind::<BigInt, _>(disk_identity)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query("DELETE FROM mame_disk_merges WHERE relationship_id=?")
            .bind::<BigInt, _>(disk_identity)
            .execute(&mut connection)
            .is_err()
    );
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO mame_disk_merges(occurrence_id,relationship_id,merge_name,source_line,source_column) VALUES (?,?,'replacement.chd',1,1)")
            .bind::<BigInt, _>(disk)
            .bind::<BigInt, _>(disk_identity)
            .execute(&mut connection),
        "native disk merge replacement",
        "MAME disk merge requires an unused source identity",
    )?;
    let originals = sql_query("SELECT count(*) AS value FROM (SELECT occurrence_id FROM mame_rom_merges WHERE occurrence_id=? AND relationship_id=? AND merge_name='parent.bin' UNION ALL SELECT occurrence_id FROM mame_disk_merges WHERE occurrence_id=? AND relationship_id=? AND merge_name='parent.chd')")
        .bind::<BigInt, _>(rom)
        .bind::<BigInt, _>(rom_identity)
        .bind::<BigInt, _>(disk)
        .bind::<BigInt, _>(disk_identity)
        .get_result::<IdRow>(&mut connection)?;
    assert_eq!(originals.value, 2);
    assert!(sql_query("UPDATE reported_catalog_relationships SET source_reference_kind='mame_disk_merge' WHERE relationship_id=?")
        .bind::<BigInt, _>(rom_identity)
        .execute(&mut connection)
        .is_err());
    Ok(())
}

#[test]
fn empty_native_text_is_preserved_while_null_text_is_rejected() -> TestResult {
    let (_directory, database_path, base) = seed_database()?;
    let mut connection = open_connection(&database_path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "empty-text")?;

    let empty_target = insert_identity(&mut connection, &snapshot, "empty-target")?;
    insert_reported(&mut connection, empty_target, "mame_romof")?;
    insert_link(&mut connection, owner, "romof", "", empty_target)?;
    let (null_target_snapshot, null_target_owner) =
        pending_machine(&mut connection, &base, "null-target")?;
    let null_target = insert_identity(&mut connection, &null_target_snapshot, "null-target")?;
    insert_reported(&mut connection, null_target, "mame_sampleof")?;
    assert_rejected(
        sql_query("INSERT INTO mame_machine_links(set_id,link_kind,target_name,relationship_id,source_line,source_column) VALUES (?,'sampleof',NULL,?,1,1)")
            .bind::<BigInt, _>(null_target_owner)
            .bind::<BigInt, _>(null_target)
            .execute(&mut connection),
        "NULL link target",
        "NOT NULL constraint failed: mame_machine_links.target_name",
    )?;

    let empty_device = insert_identity(&mut connection, &snapshot, "empty-device")?;
    insert_reported(&mut connection, empty_device, "mame_device_ref")?;
    insert_device_reference(&mut connection, owner, 0, "", "", empty_device)?;
    let (null_name_snapshot, null_name_owner) =
        pending_machine(&mut connection, &base, "null-device-name")?;
    let null_name = insert_identity(&mut connection, &null_name_snapshot, "null-device-name")?;
    insert_reported(&mut connection, null_name, "mame_device_ref")?;
    assert_rejected(
        sql_query("INSERT INTO mame_device_references(set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) VALUES (?,1,NULL,'@chip',1,?,1,1)")
            .bind::<BigInt, _>(null_name_owner)
            .bind::<BigInt, _>(null_name)
            .execute(&mut connection),
        "NULL device name",
        "NOT NULL constraint failed: mame_device_references.name",
    )?;
    let (null_tag_snapshot, null_tag_owner) =
        pending_machine(&mut connection, &base, "null-device-tag")?;
    let null_tag = insert_identity(&mut connection, &null_tag_snapshot, "null-device-tag")?;
    insert_reported(&mut connection, null_tag, "mame_device_ref")?;
    assert_rejected(
        sql_query("INSERT INTO mame_device_references(set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) VALUES (?,2,'chip',NULL,2,?,1,1)")
            .bind::<BigInt, _>(null_tag_owner)
            .bind::<BigInt, _>(null_tag)
            .execute(&mut connection),
        "NULL device tag",
        "NOT NULL constraint failed: mame_device_references.tag",
    )?;

    let text_rows = sql_query("SELECT (SELECT target_name FROM mame_machine_links WHERE relationship_id=?) || ':' || (SELECT name || ':' || tag FROM mame_device_references WHERE relationship_id=?) AS value")
        .bind::<BigInt, _>(empty_target)
        .bind::<BigInt, _>(empty_device)
        .get_result::<TextRow>(&mut connection)?;
    assert_eq!(text_rows.value, "::");
    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    Ok(())
}

#[test]
fn published_machine_owners_cannot_be_replaced_into_an_unpublished_edition() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "owner-seal")?;
    let identity = insert_identity(&mut connection, &snapshot, "owner-seal")?;
    insert_reported(&mut connection, identity, "mame_cloneof")?;
    insert_link(&mut connection, owner, "cloneof", "parent", identity)?;
    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    let (pending, pending_owner) = pending_machine(&mut connection, &base, "owner-retarget")?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT ?,set_group_id,source_element_kind,2,'retargeted',1,1 FROM catalog_sets WHERE set_id=?")
            .bind::<BigInt, _>(owner)
            .bind::<BigInt, _>(pending_owner)
            .execute(&mut connection),
        "published machine owner moved into an unpublished edition",
        "catalog sets are immutable",
    )?;
    assert_eq!(publish(&mut connection, &pending)?, 1);
    Ok(())
}

#[test]
fn published_machine_groups_cannot_be_replaced_into_an_unpublished_edition() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    let group =
        sql_query("SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key=?")
            .bind::<Text, _>(&base)
            .get_result::<IdRow>(&mut connection)?
            .value;
    let pending = pending_snapshot(&mut connection, &base, "group-retarget")?;
    assert_rejected(
        sql_query("INSERT OR REPLACE INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order) VALUES (?,?,'root',0)")
            .bind::<BigInt, _>(group)
            .bind::<Text, _>(&pending)
            .execute(&mut connection),
        "published machine group moved into an unpublished edition",
        "catalog groups are immutable",
    )?;
    let new_group = sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES (?,'root',0) RETURNING set_group_id AS value")
        .bind::<Text, _>(&pending)
        .get_result::<IdRow>(&mut connection)?.value;
    insert_machine(&mut connection, new_group, 0, "control")?;
    assert_eq!(publish(&mut connection, &pending)?, 1);
    Ok(())
}

#[test]
fn publication_rejects_sparse_device_reference_ordinals() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    for (label, orders, valid) in [
        ("missing-first", &[1_i64][..], false),
        ("middle-gap", &[0, 2][..], false),
        ("dense-control", &[0, 1][..], true),
    ] {
        let (snapshot, owner) = pending_machine(&mut connection, &base, label)?;
        for order in orders {
            let identity =
                insert_identity(&mut connection, &snapshot, &format!("{label}-{order}"))?;
            insert_reported(&mut connection, identity, "mame_device_ref")?;
            insert_device_reference(&mut connection, owner, *order, "chip", "@chip", identity)?;
        }
        let result = publish(&mut connection, &snapshot);
        if valid {
            assert_eq!(result?, 1);
        } else {
            assert_rejected(
                result,
                "device reference ordinal gap",
                "MAME device-reference positions must be dense",
            )?;
        }
    }
    Ok(())
}

#[test]
fn readiness_checks_actual_owner_edition_even_if_parent_rows_are_corrupt() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "readiness-owner")?;
    let link = insert_identity(&mut connection, &snapshot, "ready-link")?;
    insert_reported(&mut connection, link, "mame_cloneof")?;
    insert_link(&mut connection, owner, "cloneof", "parent", link)?;
    let device = insert_identity(&mut connection, &snapshot, "ready-device")?;
    insert_reported(&mut connection, device, "mame_device_ref")?;
    insert_device_reference(&mut connection, owner, 0, "chip", "@chip", device)?;
    for (order, kind, key) in [
        (0, "rom", "ready-rom-merge"),
        (1, "disk", "ready-disk-merge"),
    ] {
        let occurrence =
            insert_media_claim_at_source_order(&mut connection, owner, order, kind, order + 2)?;
        let identity = insert_identity(&mut connection, &snapshot, key)?;
        insert_reported(&mut connection, identity, &format!("mame_{kind}_merge"))?;
        insert_native_merge(&mut connection, kind, occurrence, identity, "parent")?;
    }
    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    let lookup = concat!(
        "SELECT is_published AS value FROM (WITH requested(assertion_key) AS (VALUES (?)) ",
        include_str!("../src/storage/db/relationship_readiness.sql"),
        ")"
    );
    for key in [
        "ready-link",
        "ready-device",
        "ready-rom-merge",
        "ready-disk-merge",
    ] {
        assert_eq!(
            sql_query(lookup)
                .bind::<Text, _>(key)
                .get_result::<IdRow>(&mut connection)?
                .value,
            1
        );
    }
    let (_pending, pending_owner) = pending_machine(&mut connection, &base, "readiness-corrupt")?;
    // Deliberately bypass the replacement seal in this isolated fixture to prove
    // readiness independently validates closure, rather than trusting that seal.
    connection.batch_execute("DROP TRIGGER catalog_sets_reject_replacement")?;
    sql_query("INSERT OR REPLACE INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT ?,set_group_id,source_element_kind,1,'corrupt-owner',1,1 FROM catalog_sets WHERE set_id=?")
        .bind::<BigInt, _>(owner)
        .bind::<BigInt, _>(pending_owner)
        .execute(&mut connection)?;
    for key in [
        "ready-link",
        "ready-device",
        "ready-rom-merge",
        "ready-disk-merge",
    ] {
        assert_eq!(
            sql_query(lookup)
                .bind::<Text, _>(key)
                .get_result::<IdRow>(&mut connection)?
                .value,
            0
        );
    }
    Ok(())
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[test]
fn native_merge_occurrences_cannot_be_retargeted_to_another_edition() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    let (snapshot, owner) = pending_machine(&mut connection, &base, "merge-occurrence-seal")?;
    let occurrence = insert_media_claim(&mut connection, owner, 0, "rom")?;
    let identity = insert_identity(&mut connection, &snapshot, "merge-occurrence-seal")?;
    insert_reported(&mut connection, identity, "mame_rom_merge")?;
    insert_native_merge(&mut connection, "rom", occurrence, identity, "parent.bin")?;
    assert_eq!(publish(&mut connection, &snapshot)?, 1);
    let (_pending, pending_owner) = pending_machine(&mut connection, &base, "merge-retarget")?;
    assert!(
        sql_query("INSERT OR REPLACE INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES (?,?,0,'mame_rom')")
            .bind::<BigInt, _>(occurrence)
            .bind::<BigInt, _>(pending_owner)
            .execute(&mut connection)
            .is_err(),
        "a merge's actual media ID cannot move to another machine/edition"
    );
    let pending_occurrence = insert_media_claim(&mut connection, pending_owner, 0, "rom")?;
    assert_ne!(occurrence, pending_occurrence);
    Ok(())
}

#[test]
fn native_merge_occurrence_positions_cannot_be_replaced() -> TestResult {
    let (_directory, path, base) = seed_database()?;
    let mut connection = open_connection(&path)?;
    let (_snapshot, owner) = pending_machine(&mut connection, &base, "merge-position-seal")?;
    let occurrence = insert_media_claim(&mut connection, owner, 0, "rom")?;
    assert!(
        sql_query("INSERT OR REPLACE INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES (?,0,'mame_rom')")
            .bind::<BigInt, _>(owner)
            .execute(&mut connection)
            .is_err(),
        "an existing owner/position cannot silently acquire another media ID"
    );
    let control = insert_media_claim(&mut connection, owner, 1, "rom")?;
    assert_ne!(occurrence, control);
    Ok(())
}
