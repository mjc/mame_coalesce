use camino::Utf8PathBuf;
use diesel::connection::SimpleConnection;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct IntegerValue {
    #[diesel(sql_type=BigInt)]
    value: i64,
}

fn pending_game(
    conn: &mut SqliteConnection,
    source: &SnapshotKey,
    header: bool,
) -> TestResult<i64> {
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT 'pending-field-catalog',catalog.source_key,'Pending field witness' FROM catalog_snapshots AS snapshot JOIN catalogs AS catalog USING(catalog_key) WHERE snapshot.snapshot_key=?")
        .bind::<Text,_>(source.as_str()).execute(conn)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,acquisition_key,coverage_id) SELECT 'pending-field-owner','pending-field-catalog',document_key,interpretation_key,acquisition_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(source.as_str()).execute(conn)?;
    sql_query("INSERT INTO no_intro_pc_documents(snapshot_key,header_present,source_line,source_column) VALUES('pending-field-owner',?,1,1)")
        .bind::<diesel::sql_types::Bool,_>(header).execute(conn)?;
    let group=sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending-field-owner','root',0) RETURNING set_group_id AS value").get_result::<IntegerValue>(conn)?.value;
    let set=sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(?,'no_intro_pc_game',0,'pending',1,1) RETURNING set_id AS value")
        .bind::<BigInt,_>(group).get_result::<IntegerValue>(conn)?.value;
    sql_query("INSERT INTO no_intro_pc_games(set_id,document_order) VALUES(?,0)")
        .bind::<BigInt, _>(set)
        .execute(conn)?;
    Ok(set)
}

#[test]
fn native_pc_rom_null_id_cannot_allocate_another_owner() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) = import(
        &directory,
        "<datafile><game name='published'><rom name='published.bin'/></game></datafile>",
    )?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let set = pending_game(&mut conn, &snapshot, false)?;
    sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES(-1,?,0,'no_intro_pc_file')").bind::<BigInt,_>(set).execute(&mut conn)?;
    let result=sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(NULL,'wrong-owner',0,'whole_asset','source_declared',1,1)").execute(&mut conn);
    assert!(
        result.is_err(),
        "NULL FK payload allocated an unrelated occurrence ID"
    );
    sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(-1,'actual-owner',0,'whole_asset','source_declared',1,1)").execute(&mut conn)?;
    Ok(())
}

fn publish_pending(conn: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key='pending-field-owner'")
        .execute(conn)
}

#[test]
fn native_pc_publication_requires_each_rom_payload_before_sealing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) =
        import(&directory, "<datafile><game name='seed'/></datafile>")?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let set = pending_game(&mut conn, &snapshot, false)?;
    let occurrence=sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind) VALUES(?,0,'no_intro_pc_file') RETURNING occurrence_id AS value")
        .bind::<BigInt,_>(set).get_result::<IntegerValue>(&mut conn)?.value;
    assert!(
        publish_pending(&mut conn).is_err(),
        "published a P/C ROM occurrence without its native payload"
    );
    sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(?,'actual.bin',0,'whole_asset','source_declared',1,1)")
        .bind::<BigInt,_>(occurrence).execute(&mut conn)?;
    assert_eq!(publish_pending(&mut conn)?, 1);
    Ok(())
}

#[test]
fn native_pc_header_guards_preserve_actual_owners_positions_and_publication() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) =
        import(&directory, "<datafile><game name='seed'/></datafile>")?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    pending_game(&mut conn, &snapshot, true)?;
    assert!(
        publish_pending(&mut conn).is_err(),
        "missing declared header"
    );
    for statement in [
        "INSERT INTO no_intro_pc_headers(snapshot_key,source_order,source_line,source_column) VALUES('pending-field-owner',0,1,1)",
        "INSERT INTO no_intro_pc_headers(snapshot_key,source_order,source_line,source_column,version_text) VALUES('pending-field-owner',1,1,1,'incomplete')",
        "INSERT INTO no_intro_pc_headers(snapshot_key,source_order,source_line,source_column) VALUES('orphan',1,1,1)",
    ] {
        assert!(
            sql_query(statement).execute(&mut conn).is_err(),
            "accepted {statement}"
        );
    }
    sql_query("INSERT INTO no_intro_pc_headers(snapshot_key,source_order,source_line,source_column,version_text,version_order,version_line,version_column) VALUES('pending-field-owner',1,1,1,'v1',2,1,1)").execute(&mut conn)?;
    sql_query("INSERT INTO no_intro_pc_header_names(snapshot_key,source_order,name_text,source_line,source_column) VALUES('pending-field-owner',0,'name',1,1)").execute(&mut conn)?;
    sql_query("INSERT INTO no_intro_pc_header_descriptions(snapshot_key,source_order,description_text,source_line,source_column) VALUES('pending-field-owner',1,'',1,1)").execute(&mut conn)?;
    for statement in [
        "INSERT INTO no_intro_pc_header_descriptions(snapshot_key,source_order,description_text,source_line,source_column) VALUES('pending-field-owner',0,'collision',1,1)",
        "INSERT INTO no_intro_pc_header_names(snapshot_key,source_order,name_text,source_line,source_column) VALUES('pending-field-owner',2,'version collision',1,1)",
        "INSERT OR REPLACE INTO no_intro_pc_header_names(snapshot_key,source_order,name_text,source_line,source_column) VALUES('pending-field-owner',0,'replacement',1,1)",
        "INSERT INTO no_intro_pc_header_names(snapshot_key,source_order,name_text,source_line,source_column) VALUES('orphan',0,'orphan',1,1)",
        "UPDATE no_intro_pc_headers SET version_text='changed' WHERE snapshot_key='pending-field-owner'",
        "DELETE FROM no_intro_pc_header_descriptions WHERE snapshot_key='pending-field-owner'",
    ] {
        assert!(
            sql_query(statement).execute(&mut conn).is_err(),
            "accepted {statement}"
        );
    }
    assert_eq!(publish_pending(&mut conn)?, 1);
    assert!(sql_query("INSERT INTO no_intro_pc_header_names(snapshot_key,source_order,name_text,source_line,source_column) VALUES('pending-field-owner',3,'late',1,1)").execute(&mut conn).is_err());
    Ok(())
}

#[test]
fn native_pc_rom_rejects_orphan_owners_with_foreign_keys_disabled() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, _snapshot) =
        import(&directory, "<datafile><game name='game'/></datafile>")?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let result = sql_query("INSERT INTO no_intro_pc_file_claims(occurrence_id,name,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES(999999,'orphan',0,'whole_asset','source_declared',1,1)").execute(&mut conn);
    assert!(
        result.is_err(),
        "native ROM silently accepted a nonexistent occurrence owner"
    );
    Ok(())
}

fn import(
    directory: &tempfile::TempDir,
    source: &str,
) -> TestResult<(Database, SqliteConnection, SnapshotKey)> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.xml"))
        .map_err(|_| "non-UTF-8 input")?;
    let db_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database")?;
    std::fs::write(&path, source)?;
    let database = Database::open(&db_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("synthetic-pc-fields"),
            source_display_name: "Synthetic P/C".into(),
            catalog_key: CatalogKey::new("synthetic-pc-fields"),
            catalog_display_name: "Synthetic P/C".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    Ok((
        database,
        SqliteConnection::establish(db_path.as_str())?,
        report.snapshot_key.ok_or("missing snapshot")?,
    ))
}

#[derive(Debug, QueryableByName)]
struct HeaderText {
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct RomSize {
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[test]
fn repeated_native_header_fields_retain_text_empty_values_and_cross_family_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, mut conn, snapshot) = import(
        &directory,
        r#"<datafile>
  <game name="game"/>
  <header>
    <name>  first &amp; name  </name>
    <vendor/>
    <description/>
    <name/>
    <version>  v1  </version>
    <description> second description </description>
  </header>
</datafile>"#,
    )?;
    let names = sql_query("SELECT name_text AS value, source_order, source_line, source_column FROM no_intro_pc_header_names WHERE snapshot_key=? ORDER BY source_order")
        .bind::<Text, _>(snapshot.as_str()).load::<HeaderText>(&mut conn)?;
    assert_eq!(
        names
            .iter()
            .map(|row| (row.value.as_str(), row.source_order))
            .collect::<Vec<_>>(),
        [("  first & name  ", 0), ("", 3)]
    );
    assert!(
        names
            .iter()
            .all(|row| row.source_line > 0 && row.source_column > 0)
    );
    let descriptions = sql_query("SELECT description_text AS value, source_order, source_line, source_column FROM no_intro_pc_header_descriptions WHERE snapshot_key=? ORDER BY source_order")
        .bind::<Text, _>(snapshot.as_str()).load::<HeaderText>(&mut conn)?;
    assert_eq!(
        descriptions
            .iter()
            .map(|row| (row.value.as_str(), row.source_order))
            .collect::<Vec<_>>(),
        [("", 2), (" second description ", 5)]
    );
    Ok(())
}

#[test]
fn native_rom_size_preserves_declared_spelling_and_mixed_child_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database, mut conn, snapshot) = import(
        &directory,
        r#"<datafile><game name="game">
      <rom name="first" size="0004"/>
      <description/>
      <vendor/>
      <rom name="second"/>
    </game></datafile>"#,
    )?;
    let rows = sql_query("SELECT rom.size_text, rom.size, rom.source_order FROM no_intro_pc_file_claims AS rom JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? ORDER BY occurrence.occurrence_order")
        .bind::<Text, _>(snapshot.as_str()).load::<RomSize>(&mut conn)?;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].size_text.as_deref(), Some("0004"));
    assert_eq!(rows[0].size, Some(4));
    assert_eq!(rows[0].source_order, 0);
    assert_eq!(rows[1].size_text, None);
    assert_eq!(rows[1].size, None);
    assert_eq!(rows[1].source_order, 3);
    let ids=sql_query("SELECT rom.occurrence_id AS value FROM no_intro_pc_file_claims AS rom JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? ORDER BY occurrence.occurrence_order")
        .bind::<Text,_>(snapshot.as_str()).load::<IntegerValue>(&mut conn)?.into_iter()
        .map(|row| mame_coalesce::catalog_files::OccurrenceId::from_database(row.value)).collect::<Vec<_>>();
    let files = mame_coalesce::catalog_files::occurrences_for_ids(&database, &ids)?;
    assert_eq!(files.len(), 2);
    let first = files[0]
        .no_intro_pc_rom
        .as_ref()
        .ok_or("missing native P/C ROM payload")?;
    assert_eq!(first.size_text.as_deref(), Some("0004"));
    assert_eq!(first.size, Some(4));
    assert_eq!(first.source_order, 0);
    let second = files[1]
        .no_intro_pc_rom
        .as_ref()
        .ok_or("missing native P/C ROM payload")?;
    assert_eq!(second.size_text, None);
    assert_eq!(second.size, None);
    assert_eq!(second.source_order, 3);
    Ok(())
}
