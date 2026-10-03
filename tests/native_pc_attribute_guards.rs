use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct IntegerValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct AttributePosition {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

fn pending_game(conn: &mut SqliteConnection, source: &SnapshotKey) -> TestResult<i64> {
    pending_game_with_id(conn, source, false)
}

fn pending_game_with_id(
    conn: &mut SqliteConnection,
    source: &SnapshotKey,
    with_id: bool,
) -> TestResult<i64> {
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT 'pending-attribute-catalog',catalog.source_key,'Pending attribute witness' FROM catalog_snapshots AS snapshot JOIN catalogs AS catalog USING(catalog_key) WHERE snapshot.snapshot_key=?")
        .bind::<Text, _>(source.as_str())
        .execute(conn)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,acquisition_key,coverage_id) SELECT 'pending-attribute-owner','pending-attribute-catalog',document_key,interpretation_key,acquisition_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(source.as_str())
        .execute(conn)?;
    sql_query("INSERT INTO no_intro_pc_documents(snapshot_key,header_present,source_line,source_column) VALUES('pending-attribute-owner',0,1,1)")
        .execute(conn)?;
    let group = sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending-attribute-owner','root',0) RETURNING set_group_id AS value")
        .get_result::<IntegerValue>(conn)?
        .value;
    let set = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,'no_intro_pc_game',0,'pending',1,1) RETURNING set_id AS value")
        .bind::<BigInt, _>(group)
        .get_result::<IntegerValue>(conn)?
        .value;
    if with_id {
        sql_query("INSERT INTO no_intro_pc_games(set_id,document_order,archive_id,name_alt) VALUES(?,0,'1','alternate')")
            .bind::<BigInt, _>(set)
            .execute(conn)?;
    } else {
        sql_query("INSERT INTO no_intro_pc_games(set_id,document_order) VALUES(?,0)")
            .bind::<BigInt, _>(set)
            .execute(conn)?;
    }
    sql_query("INSERT INTO no_intro_pc_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
        .bind::<BigInt, _>(set)
        .execute(conn)?;
    if with_id {
        sql_query("INSERT INTO no_intro_pc_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,1,1,13)")
            .bind::<BigInt, _>(set)
            .execute(conn)?;
    }
    Ok(set)
}

fn import_source(
    directory: &tempfile::TempDir,
    source: &str,
) -> TestResult<(Database, SqliteConnection, SnapshotKey)> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("seed.xml"))
        .map_err(|_| "non-UTF-8 input path")?;
    let db_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    std::fs::write(&path, source)?;
    let database = Database::open(&db_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("synthetic-pc-attribute-guards"),
            source_display_name: "Synthetic P/C attribute guards".into(),
            catalog_key: CatalogKey::new("synthetic-pc-attribute-guards"),
            catalog_display_name: "Synthetic P/C attribute guards".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("successful import has no snapshot")?;
    let conn = SqliteConnection::establish(db_path.as_str())?;
    Ok((database, conn, snapshot))
}

fn import_seed(
    directory: &tempfile::TempDir,
) -> TestResult<(Database, SqliteConnection, SnapshotKey)> {
    import_source(directory, "<datafile><game name='seed'/></datafile>")
}

fn pending_rom(conn: &mut SqliteConnection, source: &SnapshotKey) -> TestResult<i64> {
    let set = pending_game(conn, source)?;
    pending_rom_for_set(conn, set)
}

fn pending_rom_for_set(conn: &mut SqliteConnection, set: i64) -> TestResult<i64> {
    pending_rom_for_set_with_size(conn, set, false)
}

fn pending_rom_for_set_with_size(
    conn: &mut SqliteConnection,
    set: i64,
    with_size: bool,
) -> TestResult<i64> {
    let occurrence = sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES(?,0,'no_intro_pc_file') RETURNING occurrence_id AS value")
        .bind::<BigInt, _>(set)
        .get_result::<IntegerValue>(conn)?
        .value;
    if with_size {
        sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,size_text,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(?,'pending.bin','4',0,'whole_asset','source_declared',1,1)")
            .bind::<BigInt, _>(occurrence)
            .execute(conn)?;
    } else {
        sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(?,'pending.bin',0,'whole_asset','source_declared',1,1)")
            .bind::<BigInt, _>(occurrence)
            .execute(conn)?;
    }
    sql_query("INSERT INTO no_intro_pc_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
        .bind::<BigInt, _>(occurrence)
        .execute(conn)?;
    Ok(occurrence)
}

fn add_crc_assertion(conn: &mut SqliteConnection, occurrence: i64, value: u8) -> TestResult<()> {
    add_crc_assertion_with_scope(conn, occurrence, value, "whole_asset", "source_declared")
}

fn add_crc_assertion_with_scope(
    conn: &mut SqliteConnection,
    occurrence: i64,
    value: u8,
    scope: &str,
    provenance: &str,
) -> TestResult<()> {
    let digest = [0, 0, 0, value];
    let digest_id = sql_query("INSERT INTO digest_values(algorithm,digest) VALUES('crc32',?) RETURNING digest_id AS value")
        .bind::<diesel::sql_types::Binary, _>(&digest[..])
        .get_result::<IntegerValue>(conn)?
        .value;
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES(?,?,?,?)")
        .bind::<BigInt, _>(occurrence)
        .bind::<BigInt, _>(digest_id)
        .bind::<Text, _>(scope)
        .bind::<Text, _>(provenance)
        .execute(conn)?;
    Ok(())
}

fn add_crc_position(conn: &mut SqliteConnection, occurrence: i64) -> TestResult<()> {
    sql_query("INSERT INTO no_intro_pc_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,2,1,1,12)")
        .bind::<BigInt, _>(occurrence)
        .execute(conn)?;
    Ok(())
}

fn publish_pending(conn: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='pending-attribute-owner'")
        .execute(conn)
}

fn publication_count(conn: &mut SqliteConnection) -> TestResult<i64> {
    Ok(sql_query("SELECT COUNT(*) AS value FROM snapshot_publications WHERE snapshot_key='pending-attribute-owner'")
        .get_result::<IntegerValue>(conn)?
        .value)
}

#[test]
fn publication_rejects_two_source_crcs_sharing_one_rom_position() -> TestResult {
    let single_directory = tempfile::tempdir()?;
    let (_single_database, mut single_conn, single_snapshot) = import_seed(&single_directory)?;
    let single_occurrence = pending_rom(&mut single_conn, &single_snapshot)?;
    add_crc_assertion(&mut single_conn, single_occurrence, 1)?;
    add_crc_position(&mut single_conn, single_occurrence)?;
    assert_eq!(publish_pending(&mut single_conn)?, 1);

    let preposition_directory = tempfile::tempdir()?;
    let (_preposition_database, mut preposition_conn, preposition_snapshot) =
        import_seed(&preposition_directory)?;
    let preposition_occurrence = pending_rom(&mut preposition_conn, &preposition_snapshot)?;
    add_crc_assertion(&mut preposition_conn, preposition_occurrence, 1)?;
    add_crc_assertion(&mut preposition_conn, preposition_occurrence, 2)?;
    assert!(
        add_crc_position(&mut preposition_conn, preposition_occurrence).is_err(),
        "two source CRC values satisfied one attribute position during insertion"
    );
    assert!(
        publish_pending(&mut preposition_conn).is_err(),
        "published two source CRC assertions already present when their one attribute position was added"
    );
    assert_eq!(publication_count(&mut preposition_conn)?, 0);

    let late_directory = tempfile::tempdir()?;
    let (_late_database, mut late_conn, late_snapshot) = import_seed(&late_directory)?;
    let late_occurrence = pending_rom(&mut late_conn, &late_snapshot)?;
    add_crc_assertion(&mut late_conn, late_occurrence, 1)?;
    add_crc_position(&mut late_conn, late_occurrence)?;
    add_crc_assertion(&mut late_conn, late_occurrence, 2)?;
    assert!(
        publish_pending(&mut late_conn).is_err(),
        "published a second source CRC added after its single native position"
    );
    assert_eq!(publication_count(&mut late_conn)?, 0);

    let unpositioned_directory = tempfile::tempdir()?;
    let (_unpositioned_database, mut unpositioned_conn, unpositioned_snapshot) =
        import_seed(&unpositioned_directory)?;
    let unpositioned_occurrence = pending_rom(&mut unpositioned_conn, &unpositioned_snapshot)?;
    add_crc_assertion(&mut unpositioned_conn, unpositioned_occurrence, 1)?;
    assert!(
        publish_pending(&mut unpositioned_conn).is_err(),
        "published a source CRC assertion without its attribute position"
    );
    assert_eq!(publication_count(&mut unpositioned_conn)?, 0);

    let computed_directory = tempfile::tempdir()?;
    let (_computed_database, mut computed_conn, computed_snapshot) =
        import_seed(&computed_directory)?;
    let computed_occurrence = pending_rom(&mut computed_conn, &computed_snapshot)?;
    add_crc_assertion_with_scope(
        &mut computed_conn,
        computed_occurrence,
        1,
        "whole_asset",
        "computed",
    )?;
    assert!(
        add_crc_position(&mut computed_conn, computed_occurrence).is_err(),
        "a computed CRC satisfied a source CRC attribute position"
    );
    assert_eq!(publish_pending(&mut computed_conn)?, 1);

    let unrelated_directory = tempfile::tempdir()?;
    let (_unrelated_database, mut unrelated_conn, unrelated_snapshot) =
        import_seed(&unrelated_directory)?;
    let unrelated_occurrence = pending_rom(&mut unrelated_conn, &unrelated_snapshot)?;
    add_crc_assertion_with_scope(
        &mut unrelated_conn,
        unrelated_occurrence,
        1,
        "whole_file",
        "source_declared",
    )?;
    assert!(
        add_crc_position(&mut unrelated_conn, unrelated_occurrence).is_err(),
        "an unrelated-scope CRC satisfied a source CRC attribute position"
    );
    assert_eq!(publish_pending(&mut unrelated_conn)?, 1);
    Ok(())
}

#[test]
fn attribute_position_guards_reject_bad_domains_owners_and_mutation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) = import_seed(&directory)?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let set = pending_game_with_id(&mut conn, &snapshot, true)?;
    let occurrence = pending_rom_for_set_with_size(&mut conn, set, true)?;

    let game_bad_rows = [
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},-1,1,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},9,1,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},1.5,1,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,-1,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,2.5,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,0,1,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,2,0,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,2,1.5,1)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,2,1,0)"),
        format!("INSERT INTO no_intro_pc_game_attribute_positions VALUES({set},2,2,1,1.5)"),
        "INSERT INTO no_intro_pc_game_attribute_positions VALUES(NULL,0,1,1,1)".into(),
        "INSERT INTO no_intro_pc_game_attribute_positions VALUES(987654321,0,1,1,1)".into(),
        format!(
            "INSERT OR REPLACE INTO no_intro_pc_game_attribute_positions VALUES({set},0,4,1,1)"
        ),
    ];
    for statement in game_bad_rows {
        assert!(
            sql_query(&statement).execute(&mut conn).is_err(),
            "accepted game position guard case: {statement}"
        );
    }
    sql_query("INSERT INTO no_intro_pc_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,2,2,1,15)")
        .bind::<BigInt, _>(set)
        .execute(&mut conn)?;

    let rom_bad_rows = [
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},-1,1,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},5,1,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1.5,1,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,0,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,-1,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,1.5,1,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,1,0,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,1,1.5,1)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,1,1,0)"),
        format!("INSERT INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},1,1,1,1.5)"),
        "INSERT INTO no_intro_pc_rom_attribute_positions VALUES(NULL,0,1,1,1)".into(),
        "INSERT INTO no_intro_pc_rom_attribute_positions VALUES(987654321,0,1,1,1)".into(),
        format!(
            "INSERT OR REPLACE INTO no_intro_pc_rom_attribute_positions VALUES({occurrence},0,4,1,1)"
        ),
    ];
    for statement in rom_bad_rows {
        assert!(
            sql_query(&statement).execute(&mut conn).is_err(),
            "accepted ROM position guard case: {statement}"
        );
    }
    sql_query("INSERT INTO no_intro_pc_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,1,1,1,12)")
        .bind::<BigInt, _>(occurrence)
        .execute(&mut conn)?;

    for statement in [
        format!(
            "UPDATE no_intro_pc_game_attribute_positions SET source_order=7 WHERE set_id={set} AND field_kind=0"
        ),
        format!(
            "DELETE FROM no_intro_pc_game_attribute_positions WHERE set_id={set} AND field_kind=0"
        ),
        format!(
            "UPDATE no_intro_pc_rom_attribute_positions SET source_order=7 WHERE occurrence_id={occurrence} AND field_kind=0"
        ),
        format!(
            "DELETE FROM no_intro_pc_rom_attribute_positions WHERE occurrence_id={occurrence} AND field_kind=0"
        ),
    ] {
        assert!(
            sql_query(&statement).execute(&mut conn).is_err(),
            "accepted immutable position mutation: {statement}"
        );
    }

    assert_eq!(publish_pending(&mut conn)?, 1);
    assert!(sql_query("INSERT INTO no_intro_pc_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,4,1,1)")
        .bind::<BigInt, _>(set).execute(&mut conn).is_err());
    assert!(sql_query("INSERT INTO no_intro_pc_rom_attribute_positions(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,0,4,1,1)")
        .bind::<BigInt, _>(occurrence).execute(&mut conn).is_err());
    Ok(())
}

#[test]
fn imported_attribute_positions_cover_all_fourteen_closed_field_codes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) = import_source(
        &directory,
        "<datafile><game name='parent' id='1'/><game name='complete' vendor='gap' id='2' namealt='' region='' languages='English,French' version='' bios='' clone='1' mergeof='1'><rom name='complete.bin' vendor='gap' size='0004' crc='12345678' md5='0123456789abcdef0123456789abcdef' sha1='0123456789abcdef0123456789abcdef01234567'/></game><game name='marker' clone='P'/><game name='no-hash'><rom name='nohash.bin'/></game></datafile>",
    )?;
    let game_positions = sql_query("SELECT position.field_kind,position.source_order FROM no_intro_pc_game_attribute_positions AS position JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? AND sets.set_name='complete' ORDER BY position.field_kind")
        .bind::<Text, _>(snapshot.as_str())
        .load::<AttributePosition>(&mut conn)?;
    let game_codes = game_positions
        .iter()
        .map(|row| row.field_kind)
        .collect::<Vec<_>>();
    let game_orders = game_positions
        .iter()
        .map(|row| row.source_order)
        .collect::<Vec<_>>();
    let rom_positions = sql_query("SELECT position.field_kind,position.source_order FROM no_intro_pc_rom_attribute_positions AS position JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? AND sets.set_name='complete' ORDER BY position.field_kind")
        .bind::<Text, _>(snapshot.as_str())
        .load::<AttributePosition>(&mut conn)?;
    let rom_codes = rom_positions
        .iter()
        .map(|row| row.field_kind)
        .collect::<Vec<_>>();
    let rom_orders = rom_positions
        .iter()
        .map(|row| row.source_order)
        .collect::<Vec<_>>();
    let parent_codes = sql_query("SELECT position.field_kind AS value FROM no_intro_pc_game_attribute_positions AS position JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? AND sets.set_name='parent' ORDER BY position.field_kind")
        .bind::<Text, _>(snapshot.as_str())
        .load::<IntegerValue>(&mut conn)?
        .into_iter()
        .map(|row| row.value)
        .collect::<Vec<_>>();
    let no_hash_rom_codes = sql_query("SELECT position.field_kind AS value FROM no_intro_pc_rom_attribute_positions AS position JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? AND sets.set_name='no-hash' ORDER BY position.field_kind")
        .bind::<Text, _>(snapshot.as_str())
        .load::<IntegerValue>(&mut conn)?
        .into_iter()
        .map(|row| row.value)
        .collect::<Vec<_>>();
    let marker_codes = sql_query("SELECT position.field_kind AS value FROM no_intro_pc_game_attribute_positions AS position JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? AND sets.set_name='marker' ORDER BY position.field_kind")
        .bind::<Text, _>(snapshot.as_str())
        .load::<IntegerValue>(&mut conn)?
        .into_iter()
        .map(|row| row.value)
        .collect::<Vec<_>>();
    assert_eq!(game_codes, (0_i64..=8).collect::<Vec<_>>());
    assert_eq!(game_orders, [0, 2, 3, 4, 5, 6, 7, 8, 9]);
    assert_eq!(rom_codes, (0_i64..=4).collect::<Vec<_>>());
    assert_eq!(rom_orders, [0, 2, 3, 4, 5]);
    assert_eq!(parent_codes, [0, 1]);
    assert_eq!(no_hash_rom_codes, [0]);
    assert_eq!(marker_codes, [0, 7]);
    Ok(())
}
