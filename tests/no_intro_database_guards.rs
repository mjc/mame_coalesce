use diesel::{Connection, RunQueryDsl, SqliteConnection, connection::SimpleConnection};

#[path = "../src/storage/db/ddl.rs"]
mod bundled_ddl;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn fresh_database(header_present: bool) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")?;
    connection.batch_execute("PRAGMA foreign_keys = ON;")?;
    connection.batch_execute(bundled_ddl::SCHEMA)?;

    connection.batch_execute(
        "INSERT INTO publishing_sources(source_key,display_name) VALUES('source','source');
         INSERT INTO documents(document_key) VALUES('document');
         INSERT INTO parser_interpretations(interpretation_key,format)
             VALUES('interpretation','no-intro-database-xml-compatible');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES(1,'complete');
         INSERT INTO catalogs(catalog_key,source_key,display_name)
             VALUES('catalog','source','catalog');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
             VALUES('snapshot','catalog','document','interpretation',1);
         INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
             VALUES(1,'snapshot','root',0);",
    )?;
    let header_present = i32::from(header_present);
    connection.batch_execute(&format!(
        "INSERT INTO no_intro_exports(
             snapshot_key,envelope_kind,header_present,source_line,source_column,
             document_end_line,document_end_column)
         VALUES('snapshot','single_datafile',{header_present},1,1,30,1);"
    ))?;
    connection.batch_execute(
        "INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
             VALUES(1,1,'no_intro_database_game',0,'game-a',2,1),
                   (2,1,'no_intro_database_game',1,'game-b',20,1);
         INSERT INTO no_intro_database_games(
             set_id,name_source_order,name_source_line,name_source_column,source_end_line,source_end_column)
             VALUES(1,0,2,1,15,1),(2,0,20,1,25,1);
         INSERT INTO no_intro_dump_sources(
             dump_source_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column)
             VALUES(10,1,0,4,1,9,1);
         INSERT INTO no_intro_releases(
             release_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column)
             VALUES(20,1,1,9,1,12,1);",
    )?;
    Ok(connection)
}

fn add_occurrence(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
    set_id: i64,
    order: i64,
    claim_kind: &str,
) -> TestResult {
    let insert = diesel::sql_query(
        "INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind)
         VALUES(?,?,?,?)",
    )
    .bind::<diesel::sql_types::BigInt, _>(occurrence_id)
    .bind::<diesel::sql_types::BigInt, _>(set_id)
    .bind::<diesel::sql_types::BigInt, _>(order)
    .bind::<diesel::sql_types::Text, _>(claim_kind)
    .execute(connection);
    insert?;
    Ok(())
}

fn add_source_file(connection: &mut SqliteConnection, claim_kind: &str) -> TestResult {
    diesel::sql_query(
        "INSERT INTO no_intro_dump_files(
             occurrence_id,claim_kind,evidence_provenance,dump_source_id,set_id,
             source_order,source_line,source_column,source_end_line,source_end_column)
         VALUES(100,?,'source_declared',10,1,0,6,1,7,1)",
    )
    .bind::<diesel::sql_types::Text, _>(claim_kind)
    .execute(connection)?;
    Ok(())
}

fn add_release_file(connection: &mut SqliteConnection, claim_kind: &str) -> TestResult {
    diesel::sql_query(
        "INSERT INTO no_intro_release_files(
             occurrence_id,claim_kind,evidence_scope,evidence_provenance,release_id,set_id,
             source_order,source_line,source_column,source_end_line,source_end_column)
         VALUES(102,?,'unknown','source_declared',20,1,0,10,1,11,1)",
    )
    .bind::<diesel::sql_types::Text, _>(claim_kind)
    .execute(connection)?;
    Ok(())
}

fn add_archive_and_clone_identity(connection: &mut SqliteConnection) -> TestResult {
    connection.batch_execute(
        "INSERT INTO no_intro_archive_descriptions(
             archive_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column)
             VALUES(30,1,2,12,1,14,1);
         INSERT INTO no_intro_archive_field_positions(archive_id,field_kind,source_order,source_line,source_column)
             VALUES(30,30,0,12,2);
         INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key)
             VALUES(40,'clone-assertion','source','snapshot');
         INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind)
             VALUES(40,'no_intro_database_archive_clone');",
    )?;
    Ok(())
}

fn stage_digest_publication(connection: &mut SqliteConnection, stray_hash: bool) -> TestResult {
    connection.batch_execute(
        "INSERT INTO digest_values(digest_id,algorithm,digest)
             VALUES(1,'crc32',x'01234567'),(2,'md5',x'0123456789abcdef0123456789abcdef');
         INSERT INTO no_intro_dump_file_field_positions(occurrence_id,field_kind,source_order,source_line,source_column)
             VALUES(100,1,0,6,10);
         INSERT INTO no_intro_dump_file_digests(occurrence_id,field_kind,digest_id)
             VALUES(100,0,1);
         INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
             VALUES(100,1,'unknown','source_declared');
         INSERT INTO no_intro_database_parse_counts(
             snapshot_key,game_count,archive_count,dump_source_count,dump_details_count,
             dump_serials_count,dump_file_count,release_count,release_details_count,
             release_serials_count,release_file_count,header_field_count,archive_field_count,
             dump_details_field_count,dump_serials_field_count,dump_file_field_count,
             release_details_field_count,release_serials_field_count,release_file_field_count)
         VALUES('snapshot',2,0,1,0,0,1,1,0,0,0,0,0,0,0,1,0,0,0);",
    )?;
    if stray_hash {
        connection.batch_execute(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
                 VALUES(100,2,'unknown','source_declared');",
        )?;
    }
    Ok(())
}

fn publish(connection: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    diesel::sql_query(
        "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key)
         VALUES('catalog','document','interpretation','snapshot')",
    )
    .execute(connection)
}

#[test]
fn direct_sql_accepts_the_correct_source_file_owner() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 100, 1, 0, "no_intro_database_source_file")?;

    add_source_file(&mut connection, "no_intro_database_source_file")?;
    Ok(())
}

#[test]
fn direct_sql_accepts_the_distinct_release_file_owner() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 102, 1, 0, "no_intro_database_release_file")?;

    add_release_file(&mut connection, "no_intro_database_release_file")?;
    Ok(())
}

#[test]
fn direct_sql_rejects_an_unknown_header_field_code() -> TestResult {
    let mut connection = fresh_database(true)?;
    connection.batch_execute(
        "INSERT INTO no_intro_export_headers(
             snapshot_key,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',1,12,2,1);",
    )?;

    let result = connection.batch_execute(
        "INSERT INTO no_intro_header_fields(
             snapshot_key,source_order,field_kind,value,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',0,5,'unexpected',1,13,1,14);",
    );
    assert!(
        result.is_err(),
        "header field codes are a closed enumeration"
    );
    Ok(())
}

#[test]
fn empty_clone_target_is_a_scoped_link_not_a_parent_marker() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_archive_and_clone_identity(&mut connection)?;
    connection.batch_execute(
        "INSERT INTO no_intro_archive_clone_links(
             archive_id,declared_target_number,relationship_id)
         VALUES(30,'',40);",
    )?;

    let result = connection.batch_execute(
        "INSERT INTO no_intro_archive_clone_markers(archive_id,marker)
         VALUES(30,'P');",
    );
    assert!(
        result.is_err(),
        "a clone marker and scoped link are mutually exclusive"
    );
    Ok(())
}

#[test]
fn archive_references_reject_wrong_owner_field_and_snapshot_even_without_foreign_keys() -> TestResult
{
    let mut connection = fresh_database(false)?;
    add_archive_and_clone_identity(&mut connection)?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    for sql in [
        "INSERT INTO relationship_assertions(assertion_key,relation_type,origin,source_snapshot_key,source_field,subject_kind,source_subject_c,target_kind,source_target_a) VALUES('missing','source_parent_clone','source_assertion','snapshot','archive_clone','no_intro_archive',999,'no_intro_archive_reference','0007')",
        "INSERT INTO relationship_assertions(assertion_key,relation_type,origin,source_snapshot_key,source_field,subject_kind,source_subject_c,target_kind,source_target_a) VALUES('wrong-snapshot','source_parent_clone','source_assertion','other','archive_clone','no_intro_archive',30,'no_intro_archive_reference','0007')",
        "INSERT INTO no_intro_archive_clone_links(archive_id,declared_target_number,relationship_id) VALUES(999,'other',40)",
        "INSERT INTO no_intro_archive_merge_links(archive_id,declared_mergeof,relationship_id) VALUES(30,'other',40)",
    ] {
        assert!(
            connection.batch_execute(sql).is_err(),
            "invalid typed archive reference accepted: {sql}"
        );
    }
    connection.batch_execute("INSERT INTO no_intro_archive_clone_links(archive_id,declared_target_number,relationship_id) VALUES(30,'0007',40)")?;
    connection.batch_execute(
        "INSERT INTO no_intro_archive_descriptions(
             archive_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column)
             VALUES(31,2,0,21,1,22,1);
         INSERT INTO no_intro_archive_field_positions(archive_id,field_kind,source_order,source_line,source_column)
             VALUES(31,30,0,21,2),(31,31,1,21,3);
         INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key)
             VALUES(41,'merge-assertion','source','snapshot');
         INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind)
             VALUES(41,'no_intro_database_archive_mergeof');",
    )?;
    assert!(connection.batch_execute("INSERT INTO no_intro_archive_clone_links(archive_id,declared_target_number,relationship_id) VALUES(31,'0007',40)").is_err(), "an issued native identity cannot be reused on another archive");
    assert!(connection.batch_execute("INSERT INTO no_intro_archive_clone_links(archive_id,declared_target_number,relationship_id) VALUES(31,'0007',41)").is_err(), "a merge identity cannot own a clone declaration");
    connection.batch_execute(
        "INSERT INTO documents(document_key) VALUES('other-document');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
             VALUES('other-snapshot','catalog','other-document','interpretation',1);
         INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key)
             VALUES(42,'other-edition-clone','source','other-snapshot');
         INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind)
             VALUES(42,'no_intro_database_archive_clone');",
    )?;
    assert!(connection.batch_execute("INSERT INTO no_intro_archive_clone_links(archive_id,declared_target_number,relationship_id) VALUES(31,'0007',42)").is_err(), "an unused matching-kind identity from another actual edition must not own this archive");
    connection.batch_execute("INSERT INTO no_intro_archive_merge_links(archive_id,declared_mergeof,relationship_id) VALUES(31,'0007',41)")?;
    Ok(())
}

#[test]
fn empty_header_element_keeps_its_own_location_and_publishes() -> TestResult {
    let mut connection = fresh_database(true)?;
    connection.batch_execute(
        "INSERT INTO no_intro_export_headers(
             snapshot_key,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',1,12,2,1);
         INSERT INTO no_intro_database_parse_counts(
             snapshot_key,game_count,archive_count,dump_source_count,dump_details_count,
             dump_serials_count,dump_file_count,release_count,release_details_count,
             release_serials_count,release_file_count,header_field_count,archive_field_count,
             dump_details_field_count,dump_serials_field_count,dump_file_field_count,
             release_details_field_count,release_serials_field_count,release_file_field_count)
         VALUES('snapshot',2,0,1,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0);",
    )?;

    publish(&mut connection)?;
    Ok(())
}

#[test]
fn absent_header_rejects_a_header_owner_row() -> TestResult {
    let mut connection = fresh_database(false)?;
    let result = connection.batch_execute(
        "INSERT INTO no_intro_export_headers(
             snapshot_key,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',1,12,2,1);",
    );
    assert!(
        result.is_err(),
        "an absent header must not invent a header owner"
    );
    Ok(())
}

#[test]
fn publication_rejects_a_gap_in_header_field_order() -> TestResult {
    let mut connection = fresh_database(true)?;
    connection.batch_execute(
        "INSERT INTO no_intro_export_headers(
             snapshot_key,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',1,12,2,1);
         INSERT INTO no_intro_header_fields(
             snapshot_key,source_order,field_kind,value,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',0,0,'author',1,13,1,14),('snapshot',2,1,'piracy',1,14,1,15);
         INSERT INTO no_intro_database_parse_counts(
             snapshot_key,game_count,archive_count,dump_source_count,dump_details_count,
             dump_serials_count,dump_file_count,release_count,release_details_count,
             release_serials_count,release_file_count,header_field_count,archive_field_count,
             dump_details_field_count,dump_serials_field_count,dump_file_field_count,
             release_details_field_count,release_serials_field_count,release_file_field_count)
         VALUES('snapshot',2,0,1,0,0,0,1,0,0,0,2,0,0,0,0,0,0,0);",
    )?;

    let result = publish(&mut connection);
    assert!(
        result.is_err(),
        "field positions must be contiguous within each owner"
    );
    Ok(())
}

#[test]
fn direct_sql_rejects_a_file_attached_to_another_games_source() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 100, 2, 0, "no_intro_database_source_file")?;

    let result = diesel::sql_query(
        "INSERT INTO no_intro_dump_files(
             occurrence_id,claim_kind,evidence_scope,evidence_provenance,dump_source_id,set_id,
             source_order,source_line,source_column,source_end_line,source_end_column)
         VALUES(100,'no_intro_database_source_file','unknown','source_declared',10,2,0,6,1,7,1)",
    )
    .execute(&mut connection);
    assert!(
        result.is_err(),
        "a source file must stay with its source's game"
    );
    Ok(())
}

#[test]
fn direct_sql_rejects_the_old_shared_file_discriminator() -> TestResult {
    let mut connection = fresh_database(false)?;
    let result = add_occurrence(&mut connection, 100, 1, 0, "no_intro_database_file");
    assert!(
        result.is_err(),
        "source and release files need distinct claim kinds"
    );
    Ok(())
}

#[test]
fn direct_sql_rejects_replace_on_an_existing_native_file() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 100, 1, 0, "no_intro_database_source_file")?;
    add_source_file(&mut connection, "no_intro_database_source_file")?;

    let result = diesel::sql_query(
        "INSERT OR REPLACE INTO no_intro_dump_files(
             occurrence_id,claim_kind,evidence_scope,evidence_provenance,dump_source_id,set_id,
             source_order,source_line,source_column,source_end_line,source_end_column)
         VALUES(100,'no_intro_database_source_file','whole_file','source_declared',10,1,0,6,1,7,1)",
    )
    .execute(&mut connection);
    assert!(
        result.is_err(),
        "REPLACE must not bypass native immutability guards"
    );
    Ok(())
}

#[test]
fn archive_primary_key_collision_cannot_replace_another_games_owner() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_archive_and_clone_identity(&mut connection)?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let replacement = connection.batch_execute("INSERT OR REPLACE INTO no_intro_archive_descriptions(archive_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(30,2,0,21,1,22,1)");
    assert!(
        replacement.is_err(),
        "archive PK collisions cannot bypass owner immutability"
    );
    connection.batch_execute("INSERT INTO no_intro_archive_descriptions(archive_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(31,2,1,22,1,23,1)")?;
    Ok(())
}

#[test]
fn publication_accepts_matching_typed_source_hash_assertions() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 100, 1, 0, "no_intro_database_source_file")?;
    add_source_file(&mut connection, "no_intro_database_source_file")?;
    stage_digest_publication(&mut connection, false)?;

    publish(&mut connection)?;
    Ok(())
}

#[test]
fn publication_rejects_a_stray_source_hash_assertion() -> TestResult {
    let mut connection = fresh_database(false)?;
    add_occurrence(&mut connection, 100, 1, 0, "no_intro_database_source_file")?;
    add_source_file(&mut connection, "no_intro_database_source_file")?;
    stage_digest_publication(&mut connection, true)?;

    let result = publish(&mut connection);
    assert!(
        result.is_err(),
        "unowned source hash assertions must block publication"
    );
    Ok(())
}
