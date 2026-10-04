use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::database::Database;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn setup() -> TestResult<(tempfile::TempDir, Utf8PathBuf, String)> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&database_path)?);
    let mut connection = connect(&database_path)?;
    connection.batch_execute(
        "INSERT INTO publishing_sources(source_key,display_name) VALUES('guard-fixture','Guard fixture');\
         INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES('guard-fixture','guard-fixture','Guard fixture');\
         INSERT INTO documents(document_key,format_hint) VALUES('guard-document','logiqx');\
         INSERT INTO parser_interpretations(interpretation_key,format,parser_name,parser_version,rules_version)\
           VALUES('guard-parser','logiqx','fixture','1','logiqx-declared-text-compat-v2');\
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES(9001,'complete');\
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)\
           VALUES('guard-snapshot','guard-fixture','guard-document','guard-parser',9001);",
    )?;
    Ok((directory, database_path, "guard-snapshot".to_owned()))
}

fn connect(path: &Utf8PathBuf) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    Ok(connection)
}

fn draft_game(connection: &mut SqliteConnection, snapshot: &str) -> TestResult<i64> {
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT 'draft-attributes',source_key,'Draft attributes' FROM catalogs JOIN catalog_snapshots ON catalogs.catalog_key=catalog_snapshots.catalog_key WHERE catalog_snapshots.snapshot_key=?")
        .bind::<Text, _>(snapshot)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT 'draft-attributes','draft-attributes',document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot)
        .execute(connection)?;
    let group_id = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('draft-attributes','root',0) RETURNING set_group_id AS value",
    )
    .get_result::<IdRow>(connection)?
    .value;
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,'logiqx_game',0,'draft-game',1,1) RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group_id)
    .get_result::<IdRow>(connection)?
    .value;
    sql_query("INSERT INTO logiqx_games(set_id,source_file,is_bios,is_bios_was_present,board,rebuild_to,description,year,manufacturer) VALUES(?,'draft.zip','no',0,NULL,NULL,NULL,NULL,NULL)")
        .bind::<BigInt, _>(set_id)
        .execute(connection)?;
    Ok(set_id)
}

fn draft_set_without_native_game(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> TestResult<i64> {
    let group_id = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES(?,'root',0) RETURNING set_group_id AS value",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<IdRow>(connection)?
    .value;
    Ok(sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,'logiqx_game',0,'owner-without-game',1,1) RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group_id)
    .get_result::<IdRow>(connection)?
    .value)
}

#[test]
fn attribute_position_tables_use_closed_rowid_free_storage() -> TestResult {
    let (_directory, path, _) = setup()?;
    let mut connection = connect(&path)?;
    let tables = [
        "logiqx_document_attribute_positions",
        "logiqx_clrmamepro_attribute_positions",
        "logiqx_romcenter_attribute_positions",
        "logiqx_game_attribute_positions",
        "logiqx_release_attribute_positions",
        "logiqx_bios_attribute_positions",
        "logiqx_rom_attribute_positions",
        "logiqx_disk_attribute_positions",
        "logiqx_sample_attribute_positions",
        "logiqx_archive_attribute_positions",
        "logiqx_device_reference_attribute_positions",
    ];
    for table in tables {
        let sql = sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='table' AND name=?")
            .bind::<Text, _>(table)
            .get_result::<TextRow>(&mut connection)?
            .value;
        assert!(
            sql.contains("WITHOUT ROWID"),
            "{table} must not allocate rowids"
        );
        assert!(
            sql.contains("field_kind"),
            "{table} must identify a closed native field"
        );
    }
    let violations = sql_query(
        "SELECT name AS value FROM sqlite_schema WHERE type='view' AND name='logiqx_attribute_violations'",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(violations.value, "logiqx_attribute_violations");
    Ok(())
}

#[test]
fn publication_requires_document_facts_after_streamed_games() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut connection = connect(&path)?;
    let set_id = draft_game(&mut connection, &snapshot)?;
    for (field, order) in [(0, 0), (1, 1)] {
        sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,1,1)")
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(field)
            .bind::<BigInt, _>(order)
            .execute(&mut connection)?;
    }
    let publish = "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='draft-attributes'";
    assert!(
        sql_query(publish).execute(&mut connection).is_err(),
        "completed game rows cannot replace the required document owner"
    );
    sql_query("INSERT INTO logiqx_document_facts(snapshot_key) VALUES('draft-attributes')")
        .execute(&mut connection)?;
    sql_query(publish).execute(&mut connection)?;
    Ok(())
}

#[test]
fn draft_native_game_replace_is_rejected_with_both_pragmas_off() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut connection = connect(&path)?;
    let set_id = draft_game(&mut connection, &snapshot)?;

    // This is the intentional red regression: native UPDATE/DELETE triggers
    // alone allow REPLACE to erase the old row when recursive triggers are off.
    let result = sql_query("INSERT OR REPLACE INTO logiqx_games(set_id,source_file,is_bios,is_bios_was_present,board,rebuild_to,description,year,manufacturer) VALUES(?,'replaced.zip','no',0,NULL,NULL,'Replacement',NULL,NULL)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        result.is_err(),
        "REPLACE must preserve the original immutable game payload"
    );
    let retained = sql_query("SELECT source_file AS value FROM logiqx_games WHERE set_id=?")
        .bind::<BigInt, _>(set_id)
        .get_result::<TextRow>(&mut connection)?;
    assert_eq!(retained.value, "draft.zip");
    Ok(())
}

#[test]
fn positions_enforce_publication_closure_presence_and_both_collision_keys() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut connection = connect(&path)?;
    let set_id = draft_game(&mut connection, &snapshot)?;

    let publication = sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='draft-attributes'")
        .execute(&mut connection);
    assert!(
        publication.is_err(),
        "the required game name position is missing"
    );

    sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection)?;
    let duplicate_field = sql_query("INSERT OR REPLACE INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,1,1,2)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        duplicate_field.is_err(),
        "the owner and field primary key is immutable"
    );
    let duplicate_ordinal = sql_query("INSERT OR REPLACE INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,0,1,2)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        duplicate_ordinal.is_err(),
        "the owner and source ordinal key is immutable"
    );

    // The game name is always present. Position insertion must also reject a
    // closed code that names a field absent from this owner's fixed columns.
    let absent_field = sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,6,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        absent_field.is_err(),
        "absent board value cannot acquire provenance"
    );
    let unknown_field = sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,99,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        unknown_field.is_err(),
        "field codes must stay within the Logiqx contract"
    );

    let update = sql_query("UPDATE logiqx_game_attribute_positions SET source_column=2 WHERE set_id=? AND field_kind=0")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(update.is_err(), "native attribute positions are immutable");
    let delete =
        sql_query("DELETE FROM logiqx_game_attribute_positions WHERE set_id=? AND field_kind=0")
            .bind::<BigInt, _>(set_id)
            .execute(&mut connection);
    assert!(
        delete.is_err(),
        "native attribute positions cannot be deleted"
    );
    Ok(())
}

#[test]
fn composite_release_position_requires_the_actual_native_game_owner() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut connection = connect(&path)?;
    let set_id = draft_game(&mut connection, &snapshot)?;
    sql_query("INSERT INTO logiqx_releases(set_id,release_order,source_order,name,region,language,date,\"default\",default_was_present,source_line,source_column) VALUES(?,0,0,'World','US',NULL,NULL,'no',0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection)?;

    remove_native_game_in_corrupt_fixture(&mut connection, set_id)?;

    let position = sql_query("INSERT INTO logiqx_release_attribute_positions(set_id,release_order,field_kind,source_order,source_line,source_column) VALUES(?,0,0,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection);
    assert!(
        position.is_err(),
        "a release row without its native Logiqx game is not a valid position owner"
    );
    Ok(())
}

#[test]
fn global_violations_report_a_corrupt_missing_native_game_owner() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut connection = connect(&path)?;
    let set_id = draft_game(&mut connection, &snapshot)?;
    sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection)?;

    // Corrupt only this disposable fixture so the global backup audit must
    // detect a position stranded after its native value owner disappears.
    remove_native_game_in_corrupt_fixture(&mut connection, set_id)?;
    let reason = sql_query("SELECT reason AS value FROM logiqx_attribute_violations WHERE owner_kind='game' AND owner_a=? AND field_kind=0 LIMIT 1")
        .bind::<BigInt, _>(set_id)
        .get_result::<TextRow>(&mut connection)?
        .value;
    assert_eq!(reason, "extraneous_or_misplaced_position");
    Ok(())
}

fn remove_native_game_in_corrupt_fixture(conn: &mut SqliteConnection, set_id: i64) -> TestResult {
    corrupt_fixture_with_guard_removed(
        conn,
        "logiqx_games_native_immutable_delete",
        &format!("DELETE FROM logiqx_games WHERE set_id={set_id}"),
    )
}

fn corrupt_fixture_with_guard_removed(
    conn: &mut SqliteConnection,
    trigger_name: &str,
    statement: &str,
) -> TestResult {
    let guard = sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='trigger' AND name=?")
        .bind::<Text, _>(trigger_name)
        .get_result::<TextRow>(conn)?
        .value;
    conn.batch_execute(&format!("DROP TRIGGER {trigger_name};"))?;
    let result = conn.batch_execute(statement);
    conn.batch_execute(&guard)?;
    result?;
    Ok(())
}

#[test]
fn every_attribute_value_owner_rejects_replace_before_positions_are_inserted() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    let set = draft_game(&mut conn, &snapshot)?;
    conn.batch_execute("INSERT INTO logiqx_document_facts(snapshot_key) VALUES('draft-attributes');
        INSERT INTO logiqx_clrmamepro_options(snapshot_key,source_order,source_line,source_column,header_was_present,forcemerging_was_present,forcenodump_was_present,forcepacking_was_present) VALUES('draft-attributes',0,1,1,0,0,0,0);
        INSERT INTO logiqx_romcenter_options(snapshot_key,source_order,source_line,source_column,plugin_was_present,rommode_was_present,biosmode_was_present,samplemode_was_present,lockrommode_was_present,lockbiosmode_was_present,locksamplemode_was_present) VALUES('draft-attributes',1,1,1,0,0,0,0,0,0,0);")?;
    for (kind, id, order) in [
        ("logiqx_rom", 3001, 0),
        ("logiqx_disk", 3002, 1),
        ("logiqx_sample", 3003, 2),
    ] {
        sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(?,?,?,?)").bind::<BigInt,_>(id).bind::<BigInt,_>(set).bind::<BigInt,_>(order).bind::<Text,_>(kind).execute(&mut conn)?;
        assert_eq!(
            sql_query("SELECT count(*) AS value FROM asset_occurrences WHERE occurrence_id=? AND record_id=? AND claim_kind=?")
                .bind::<BigInt, _>(id)
                .bind::<BigInt, _>(set)
                .bind::<Text, _>(kind)
                .get_result::<IdRow>(&mut conn)?
                .value,
            1,
            "each native media claim fixture needs its matching game-owned occurrence first"
        );
    }
    sql_query("INSERT INTO logiqx_releases(set_id,release_order,source_order,name,region,default_was_present,source_line,source_column) VALUES(?,0,0,'World','US',0,1,1)").bind::<BigInt,_>(set).execute(&mut conn)?;
    sql_query("INSERT INTO logiqx_bios_sets(set_id,bios_order,source_order,name,description,default_was_present,source_line,source_column) VALUES(?,0,1,'base','Base',0,1,1)").bind::<BigInt,_>(set).execute(&mut conn)?;
    sql_query("INSERT INTO logiqx_archive_references(set_id,archive_order,source_order,archive_name,source_line,source_column) VALUES(?,0,2,'archive',1,1)").bind::<BigInt,_>(set).execute(&mut conn)?;
    conn.batch_execute("INSERT INTO logiqx_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(3001,'rom','whole_asset','source_declared',1,1);
        INSERT INTO logiqx_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(3002,'disk','disk_data','source_declared',1,1);
        INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column) VALUES(3003,'sample',0,1,1);")?;
    for table in [
        "logiqx_document_facts",
        "logiqx_clrmamepro_options",
        "logiqx_romcenter_options",
        "logiqx_games",
        "logiqx_releases",
        "logiqx_bios_sets",
        "logiqx_archive_references",
        "logiqx_rom_claims",
        "logiqx_disk_claims",
        "logiqx_sample_claims",
    ] {
        let before = sql_query(format!("SELECT count(*) AS value FROM {table}"))
            .get_result::<IdRow>(&mut conn)?
            .value;
        assert_eq!(before, 1, "fixture must exercise {table}");
        // Copy every stored column, excluding SQLite's generated interpretations.
        let columns = sql_query(
            "SELECT name AS value FROM pragma_table_xinfo(?) WHERE hidden=0 ORDER BY cid",
        )
        .bind::<Text, _>(table)
        .load::<TextRow>(&mut conn)?
        .into_iter()
        .map(|row| format!("\"{}\"", row.value))
        .collect::<Vec<_>>()
        .join(",");
        let replaced = sql_query(format!(
            "INSERT OR REPLACE INTO {table}({columns}) SELECT {columns} FROM {table}"
        ))
        .execute(&mut conn);
        assert!(
            replaced.is_err(),
            "REPLACE bypassed {table} immutability with recursive triggers off"
        );
        assert_eq!(
            sql_query(format!("SELECT count(*) AS value FROM {table}"))
                .get_result::<IdRow>(&mut conn)?
                .value,
            before
        );
    }
    Ok(())
}

#[test]
fn native_value_owners_require_actual_parents_with_both_pragmas_off() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    let orphan_set = draft_set_without_native_game(&mut conn, &snapshot)?;
    let mut accepted_without_parent = Vec::new();

    let clrmamepro = sql_query("INSERT INTO logiqx_clrmamepro_options(snapshot_key,source_order,source_line,source_column,header_was_present,forcemerging_was_present,forcenodump_was_present,forcepacking_was_present) VALUES(?,0,1,1,0,0,0,0)")
        .bind::<Text, _>(&snapshot)
        .execute(&mut conn);
    if clrmamepro.is_ok() {
        accepted_without_parent.push("ClrMamePro options without document facts");
    }

    let romcenter = sql_query("INSERT INTO logiqx_romcenter_options(snapshot_key,source_order,source_line,source_column,plugin_was_present,rommode_was_present,biosmode_was_present,samplemode_was_present,lockrommode_was_present,lockbiosmode_was_present,locksamplemode_was_present) VALUES(?,0,1,1,0,0,0,0,0,0,0)")
        .bind::<Text, _>(&snapshot)
        .execute(&mut conn);
    if romcenter.is_ok() {
        accepted_without_parent.push("RomCenter options without document facts");
    }

    let release = sql_query("INSERT INTO logiqx_releases(set_id,release_order,source_order,name,region,default_was_present,source_line,source_column) VALUES(?,0,0,'World','US',0,1,1)")
        .bind::<BigInt, _>(orphan_set)
        .execute(&mut conn);
    if release.is_ok() {
        accepted_without_parent.push("release without a native game");
    }

    let bios = sql_query("INSERT INTO logiqx_bios_sets(set_id,bios_order,source_order,name,description,default_was_present,source_line,source_column) VALUES(?,0,0,'bios','BIOS',0,1,1)")
        .bind::<BigInt, _>(orphan_set)
        .execute(&mut conn);
    if bios.is_ok() {
        accepted_without_parent.push("BIOS set without a native game");
    }

    let archive = sql_query("INSERT INTO logiqx_archive_references(set_id,archive_order,source_order,archive_name,source_line,source_column) VALUES(?,0,0,'archive',1,1)")
        .bind::<BigInt, _>(orphan_set)
        .execute(&mut conn);
    if archive.is_ok() {
        accepted_without_parent.push("archive reference without a native game");
    }

    let device = sql_query("INSERT INTO logiqx_device_references(set_id,reference_order,source_order,source_line,source_column,target_name,relationship_id) VALUES(?,0,0,1,1,'device',7901)")
        .bind::<BigInt, _>(orphan_set)
        .execute(&mut conn);
    if device.is_ok() {
        accepted_without_parent.push("device reference without a native game");
    }

    let rom = sql_query("INSERT INTO logiqx_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(7902,'rom','whole_asset','source_declared',1,1)")
        .execute(&mut conn);
    if rom.is_ok() {
        accepted_without_parent.push("ROM claim without an asset occurrence");
    }
    let rom_auto_id = sql_query("INSERT INTO logiqx_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(NULL,'rom','whole_asset','source_declared',1,1)")
        .execute(&mut conn);
    if rom_auto_id.is_ok() {
        accepted_without_parent.push("ROM claim with an auto-allocated occurrence ID");
    }

    let disk = sql_query("INSERT INTO logiqx_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(7903,'disk','disk_data','source_declared',1,1)")
        .execute(&mut conn);
    if disk.is_ok() {
        accepted_without_parent.push("disk claim without an asset occurrence");
    }
    let disk_auto_id = sql_query("INSERT INTO logiqx_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(NULL,'disk','disk_data','source_declared',1,1)")
        .execute(&mut conn);
    if disk_auto_id.is_ok() {
        accepted_without_parent.push("disk claim with an auto-allocated occurrence ID");
    }

    let sample = sql_query("INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column) VALUES(7904,'sample',0,1,1)")
        .execute(&mut conn);
    if sample.is_ok() {
        accepted_without_parent.push("sample claim without an asset occurrence");
    }
    let sample_auto_id = sql_query("INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column) VALUES(NULL,'sample',0,1,1)")
        .execute(&mut conn);
    if sample_auto_id.is_ok() {
        accepted_without_parent.push("sample claim with an auto-allocated occurrence ID");
    }

    // Game insertion is allowed before document facts: the application writes
    // game rows before it stores the root facts, while the generic document
    // ancestry must already be real.
    let valid_game = draft_game(&mut conn, &snapshot)?;
    sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1),(?,1,1,1,2)")
        .bind::<BigInt, _>(valid_game)
        .bind::<BigInt, _>(valid_game)
        .execute(&mut conn)?;
    sql_query("INSERT INTO logiqx_document_facts(snapshot_key) VALUES('draft-attributes')")
        .execute(&mut conn)?;
    let publication = sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='draft-attributes'")
        .execute(&mut conn);
    assert!(
        publication.is_ok(),
        "a valid published owner closure remains accepted"
    );
    assert!(
        accepted_without_parent.is_empty(),
        "native value-owner parent guards are missing for: {accepted_without_parent:?}"
    );
    Ok(())
}

#[test]
fn global_audit_reports_draft_value_owners_without_native_parents() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    corrupt_fixture_with_guard_removed(
        &mut conn,
        "logiqx_clrmamepro_options_attribute_parent_insert_guard",
        "INSERT INTO logiqx_clrmamepro_options(snapshot_key,source_order,source_line,source_column,header_was_present,forcemerging_was_present,forcenodump_was_present,forcepacking_was_present) VALUES('guard-snapshot',0,1,1,0,0,0,0);",
    )?;

    let reason = sql_query("SELECT reason AS value FROM logiqx_attribute_violations WHERE owner_kind='clrmamepro' AND owner_a=? AND reason='orphan_or_wrong_ancestry' LIMIT 1")
        .bind::<Text, _>(&snapshot)
        .load::<TextRow>(&mut conn)?
        .into_iter()
        .next();
    assert_eq!(
        reason.map(|row| row.value).as_deref(),
        Some("orphan_or_wrong_ancestry"),
        "the global audit includes invalid native value owners in drafts"
    );
    Ok(())
}

#[test]
fn publication_rejects_reachable_native_children_without_positions() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    let orphan_set = draft_set_without_native_game(&mut conn, &snapshot)?;
    corrupt_fixture_with_guard_removed(
        &mut conn,
        "logiqx_releases_actual_game_owner_insert_guard",
        &format!(
            "INSERT INTO logiqx_releases(set_id,release_order,source_order,name,region,default_was_present,source_line,source_column) VALUES({orphan_set},0,0,'World','US',0,1,1);"
        ),
    )?;
    sql_query("INSERT INTO logiqx_document_facts(snapshot_key) VALUES(?)")
        .bind::<Text, _>(&snapshot)
        .execute(&mut conn)?;

    let publication = sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&snapshot)
        .execute(&mut conn);
    assert!(
        publication.is_err(),
        "publication must find native child rows even when no positions qualify them"
    );
    Ok(())
}

fn bios_position_rejects_missing_parent(
    delete_trigger: &str,
    delete_statement: &str,
) -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    let set_id = draft_game(&mut conn, &snapshot)?;
    sql_query("INSERT INTO logiqx_bios_sets(set_id,bios_order,source_order,name,description,default_was_present,source_line,source_column) VALUES(?,0,0,'base','Base',0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut conn)?;

    corrupt_fixture_with_guard_removed(&mut conn, delete_trigger, delete_statement)?;
    let position = sql_query("INSERT INTO logiqx_bios_attribute_positions(set_id,bios_order,field_kind,source_order,source_line,source_column) VALUES(?,0,0,0,1,1)")
        .bind::<BigInt, _>(set_id)
        .execute(&mut conn);
    assert!(
        position.is_err(),
        "BIOS position insertion must require the actual {delete_trigger} parent"
    );
    Ok(())
}

#[test]
fn bios_position_guard_requires_actual_catalog_and_document_rows() -> TestResult {
    bios_position_rejects_missing_parent(
        "catalogs_stable_delete",
        "DELETE FROM catalogs WHERE catalog_key='draft-attributes';",
    )?;
    bios_position_rejects_missing_parent(
        "documents_are_immutable_delete",
        "DELETE FROM documents WHERE document_key='guard-document';",
    )?;
    Ok(())
}

struct MediaProbe {
    table: &'static str,
    kind: &'static str,
    positions: &'static str,
    insert: &'static str,
}

const MEDIA: [MediaProbe; 3] = [
    MediaProbe {
        table: "logiqx_rom_claims",
        kind: "logiqx_rom",
        positions: "logiqx_rom_attribute_positions",
        insert: "INSERT INTO logiqx_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(?,'rom','whole_asset','source_declared',1,1)",
    },
    MediaProbe {
        table: "logiqx_disk_claims",
        kind: "logiqx_disk",
        positions: "logiqx_disk_attribute_positions",
        insert: "INSERT INTO logiqx_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column) VALUES(?,'disk','disk_data','source_declared',1,1)",
    },
    MediaProbe {
        table: "logiqx_sample_claims",
        kind: "logiqx_sample",
        positions: "logiqx_sample_attribute_positions",
        insert: "INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column) VALUES(?,'sample',0,1,1)",
    },
];

#[test]
fn native_media_null_ids_cannot_validate_against_minus_one_parents() -> TestResult {
    let (_directory, path, snapshot) = setup()?;
    let mut conn = connect(&path)?;
    let set = draft_game(&mut conn, &snapshot)?;
    let mut accepted = Vec::new();
    for media in &MEDIA {
        conn.batch_execute("SAVEPOINT media_probe")?;
        sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(-1,?,0,?)")
            .bind::<BigInt, _>(set).bind::<Text, _>(media.kind).execute(&mut conn)?;
        if sql_query(media.insert)
            .bind::<Nullable<BigInt>, _>(None::<i64>)
            .execute(&mut conn)
            .is_ok()
        {
            accepted.push(media.kind);
        }
        conn.batch_execute(
            "ROLLBACK TO media_probe; RELEASE media_probe; SAVEPOINT positive_control",
        )?;
        sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(1,?,0,?)")
            .bind::<BigInt, _>(set).bind::<Text, _>(media.kind).execute(&mut conn)?;
        sql_query(media.insert)
            .bind::<Nullable<BigInt>, _>(Some(1_i64))
            .execute(&mut conn)?;
        conn.batch_execute("ROLLBACK TO positive_control; RELEASE positive_control")?;
    }
    assert!(
        accepted.is_empty(),
        "automatic IDs validated the wrong parent: {accepted:?}"
    );
    Ok(())
}

#[test]
fn publication_checks_unpositioned_native_media_values_after_kind_corruption() -> TestResult {
    let mut accepted = Vec::new();
    for (index, original) in MEDIA.iter().enumerate() {
        let replacement = &MEDIA[if index == 2 { 0 } else { 2 }];
        let (_directory, path, snapshot) = setup()?;
        let mut conn = connect(&path)?;
        let set = draft_game(&mut conn, &snapshot)?;
        sql_query("INSERT INTO logiqx_document_facts(snapshot_key) VALUES('draft-attributes')")
            .execute(&mut conn)?;
        sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1),(?,1,1,1,2)")
            .bind::<BigInt,_>(set).bind::<BigInt,_>(set).execute(&mut conn)?;
        sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(1,?,0,?)")
            .bind::<BigInt,_>(set).bind::<Text,_>(original.kind).execute(&mut conn)?;
        sql_query(original.insert)
            .bind::<Nullable<BigInt>, _>(Some(1_i64))
            .execute(&mut conn)?;
        corrupt_fixture_with_guard_removed(
            &mut conn,
            "asset_occurrences_are_immutable_update",
            &format!(
                "UPDATE asset_occurrences SET claim_kind='{}' WHERE occurrence_id=1",
                replacement.kind
            ),
        )?;
        sql_query(replacement.insert)
            .bind::<Nullable<BigInt>, _>(Some(1_i64))
            .execute(&mut conn)?;
        sql_query(format!("INSERT INTO {}(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(1,0,0,1,1)",replacement.positions)).execute(&mut conn)?;
        let violations = sql_query("SELECT count(*) AS value FROM logiqx_attribute_violations WHERE reason='orphan_or_wrong_ancestry' AND owner_kind=?")
            .bind::<Text,_>(original.kind.strip_prefix("logiqx_").ok_or("fixture media kind")?).get_result::<IdRow>(&mut conn)?.value;
        assert!(
            violations > 0,
            "global audit must independently find the wrong-kind native value"
        );
        let publish = "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='draft-attributes'";
        conn.batch_execute("SAVEPOINT publication_probe")?;
        if conn.batch_execute(publish).is_ok() {
            accepted.push(original.kind);
        }
        conn.batch_execute("ROLLBACK TO publication_probe; RELEASE publication_probe")?;
        corrupt_fixture_with_guard_removed(
            &mut conn,
            &format!("{}_immutable_delete", original.table),
            &format!("DELETE FROM {} WHERE occurrence_id=1", original.table),
        )?;
        conn.batch_execute(publish)?;
    }
    assert!(
        accepted.is_empty(),
        "publication overlooked wrong-kind native values: {accepted:?}"
    );
    Ok(())
}
