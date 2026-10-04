use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{self, OccurrenceId},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, CatalogSnapshotDiff, ExternalRecordRef, PublishingSourceKey,
        RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipOrigin,
        RelationshipType, SnapshotKey,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn import(
    database: &Database,
    directory: &tempfile::TempDir,
    file_name: &str,
    contents: &str,
) -> TestResult<SnapshotKey> {
    import_catalog(
        database,
        directory,
        file_name,
        contents,
        "no-intro-consumer-history",
    )
}

fn import_catalog(
    database: &Database,
    directory: &tempfile::TempDir,
    file_name: &str,
    contents: &str,
    catalog_key: &str,
) -> TestResult<SnapshotKey> {
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join(file_name))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
            source_key: PublishingSourceKey::new(catalog_key),
            source_display_name: format!("No-Intro source {catalog_key}"),
            catalog_key: CatalogKey::new(catalog_key),
            catalog_display_name: format!("No-Intro catalog {catalog_key}"),
            scope: CatalogScope::Complete,
        },
    )?;
    if report.status != app::CatalogImportStatus::Succeeded {
        let mut connection = SqliteConnection::establish(database_path.as_str())?;
        let diagnostics = sql_query(
            "SELECT code, message FROM import_diagnostics WHERE run_key = ? ORDER BY diagnostic_key",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .load::<ImportDiagnostic>(&mut connection)?
        .into_iter()
        .map(|row| format!("{}: {}", row.code, row.message))
        .collect::<Vec<_>>();
        return Err(format!("No-Intro consumer fixture import failed: {diagnostics:?}").into());
    }
    report
        .snapshot_key
        .ok_or_else(|| "No-Intro import did not produce a snapshot".into())
}

#[derive(QueryableByName)]
struct ImportDiagnostic {
    #[diesel(sql_type = Text)]
    code: String,
    #[diesel(sql_type = Text)]
    message: String,
}

#[derive(QueryableByName)]
struct OccurrenceRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[derive(QueryableByName)]
struct SetIdRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    assertion_key: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

struct NativeCloneofGuardFixture {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    native_key: String,
    cloneofid_only_key: String,
}

impl NativeCloneofGuardFixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        let database = Database::open(&database_path)?;
        let snapshot = import(
            &database,
            &directory,
            "relationship-guards.dat",
            &document(
                "<game name='native-child' cloneof='parent'><description>Child</description><rom name='child.bin'/></game>\
                 <game name='id-only' cloneofid='7'><description>ID only</description><rom name='id.bin'/></game>",
            ),
        )?;
        let mut connection = SqliteConnection::establish(database_path.as_str())?;
        let native = sql_query(
            "SELECT game.set_id AS set_id, registry.assertion_key \
             FROM no_intro_dat_games AS game \
             JOIN catalog_sets AS sets USING (set_id) \
             JOIN catalog_set_groups AS groups USING (set_group_id) \
             JOIN no_intro_dat_set_links AS link ON link.set_id=game.set_id AND link.link_kind='cloneof' \
             JOIN catalog_relationships AS registry USING(relationship_id) \
             JOIN no_intro_dat_game_field_positions AS position \
               ON position.set_id = game.set_id AND position.field_kind = 2 \
             WHERE groups.snapshot_key = ? AND sets.set_name = 'native-child'",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SetIdRow>(&mut connection)?;
        let id_only = sql_query(
            "SELECT game.set_id AS set_id, registry.assertion_key \
             FROM no_intro_dat_games AS game \
             JOIN catalog_sets AS sets USING (set_id) \
             JOIN catalog_set_groups AS groups USING (set_group_id) \
             JOIN no_intro_dat_set_links AS link ON link.set_id=game.set_id AND link.link_kind='cloneofid' \
             JOIN catalog_relationships AS registry USING(relationship_id) \
             WHERE groups.snapshot_key = ? AND sets.set_name = 'id-only'",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SetIdRow>(&mut connection)?;
        assert_ne!(native.set_id, id_only.set_id);
        let native_key = native.assertion_key;
        let cloneofid_only_key = id_only.assertion_key;
        let aliases = sql_query(
            "SELECT COUNT(*) AS count FROM no_intro_dat_game_field_positions \
             WHERE set_id = ? AND field_kind = 2",
        )
        .bind::<BigInt, _>(id_only.set_id)
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(aliases.count, 0);
        Ok(Self {
            _directory: directory,
            database,
            database_path,
            native_key,
            cloneofid_only_key,
        })
    }

    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.database_path.as_str())?)
    }

    fn invalid_keys(&self) -> [String; 3] {
        [
            "missing-native-relationship".to_owned(),
            format!("{}-suffix", self.native_key),
            format!("0{}", self.native_key),
        ]
    }
}

fn insert_stored_assertion(
    connection: &mut SqliteConnection,
    assertion_key: &str,
    origin: &str,
) -> diesel::QueryResult<usize> {
    let derived = origin == "derived_candidate";
    sql_query("INSERT INTO catalog_relationships(assertion_key,origin) VALUES(?,?)")
        .bind::<Text, _>(assertion_key)
        .bind::<Text, _>(if derived { "derived" } else { "user" })
        .execute(connection)?;
    for side in ["subject", "target"] {
        sql_query("INSERT INTO catalog_relationship_targets(kind) VALUES('external_record')")
            .execute(connection)?;
        sql_query("INSERT INTO external_catalog_targets(target_id,namespace,declared_key) VALUES(last_insert_rowid(),'guard-test',?)")
            .bind::<Text,_>(format!("{assertion_key}:{side}"))
            .execute(connection)?;
    }
    let (table, rule_join, rule_column, rule_value) = if derived {
        sql_query("INSERT INTO catalog_relationship_rules(rule_key,revision,description) SELECT 'guard','v1','Guard-test rule' WHERE NOT EXISTS(SELECT 1 FROM catalog_relationship_rules WHERE rule_key='guard' AND revision='v1')").execute(connection)?;
        (
            "inferred_catalog_relationships",
            "JOIN catalog_relationship_rules rule ON rule.rule_key='guard' AND rule.revision='v1'",
            ",rule_id",
            ",rule.rule_id",
        )
    } else {
        ("manual_catalog_relationships", "", "", "")
    };
    sql_query(format!("INSERT INTO {table}(relationship_id,relation_type,from_target_id,to_target_id{rule_column}) SELECT identity.relationship_id,'catalog_correction',source.target_id,target.target_id{rule_value} FROM catalog_relationships identity JOIN external_catalog_targets source ON source.namespace='guard-test' AND source.declared_key=? JOIN external_catalog_targets target ON target.namespace='guard-test' AND target.declared_key=? {rule_join} WHERE identity.assertion_key=?"))
        .bind::<Text,_>(format!("{assertion_key}:subject"))
        .bind::<Text,_>(format!("{assertion_key}:target"))
        .bind::<Text,_>(assertion_key).execute(connection)
}

fn insert_review(
    connection: &mut SqliteConnection,
    review_key: &str,
    assertion_key: &str,
    decision: &str,
    superseded_by: Option<&str>,
) -> diesel::QueryResult<usize> {
    connection.transaction(|connection| {
        sql_query("INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES (?,(SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?),?,'native cloneof guard test')")
            .bind::<Text,_>(review_key).bind::<Text,_>(assertion_key).bind::<Text,_>(decision).execute(connection)?;
        if let Some(successor) = superseded_by {
            sql_query("INSERT INTO replaced_catalog_relationships(review_id,replacement_relationship_id) VALUES((SELECT review_id FROM catalog_relationship_reviews WHERE review_key=?),(SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?))")
                .bind::<Text,_>(review_key).bind::<Text,_>(successor).execute(connection)?;
        }
        sql_query("INSERT INTO catalog_relationship_review_publications(review_id) SELECT review_id FROM catalog_relationship_reviews WHERE review_key=?")
            .bind::<Text,_>(review_key).execute(connection)
    })
}

fn insert_support(
    connection: &mut SqliteConnection,
    assertion_key: &str,
    position: i64,
    supported_assertion_key: &str,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO catalog_relationship_evidence \
         (relationship_id,list_order,supporting_relationship_id) \
         VALUES ((SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?), ?, \
                 (SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?))",
    )
    .bind::<Text, _>(assertion_key)
    .bind::<BigInt, _>(position)
    .bind::<Text, _>(supported_assertion_key)
    .execute(connection)
}

#[allow(clippy::expect_used)]
fn assert_guard_rejects(result: diesel::QueryResult<usize>, expected_message: &str) {
    let error = result.expect_err("invalid relationship key must be rejected by its guard");
    assert!(
        error.to_string().contains(expected_message),
        "expected {expected_message:?}, got {error}"
    );
}

fn occurrence_ids(
    connection: &mut SqliteConnection,
    catalog_key: &str,
) -> TestResult<Vec<OccurrenceId>> {
    Ok(sql_query(
        "SELECT occurrence.occurrence_id FROM catalog_snapshots AS snapshot \
         JOIN catalog_set_groups AS groups USING (snapshot_key) \
         JOIN catalog_sets AS sets USING (set_group_id) \
         JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id \
         WHERE snapshot.catalog_key = ? AND occurrence.claim_kind = 'no_intro_dat_rom' \
         ORDER BY sets.list_order, occurrence.occurrence_order",
    )
    .bind::<Text, _>(catalog_key)
    .load::<OccurrenceRow>(connection)?
    .into_iter()
    .map(|row| OccurrenceId::from_database(row.occurrence_id))
    .collect())
}

fn diff_documents(previous: &str, current: &str) -> TestResult<CatalogSnapshotDiff> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let previous = import(&database, &directory, "previous.dat", previous)?;
    let current = import(&database, &directory, "current.dat", current)?;
    Ok(app::diff_catalog_snapshots(&database, &previous, &current)?)
}

fn document(game: &str) -> String {
    document_with_header("", game)
}

fn document_with_header(options: &str, game: &str) -> String {
    format!(
        "<datafile><header><id>1</id><name>Consumer test</name>\
         <description>Consumer fixture</description><version>1</version>{options}</header>{game}</datafile>"
    )
}

#[test]
fn reordered_attributes_with_vendor_field_do_not_shift_child_ordinals() -> TestResult {
    let first = "<game name='same' id='0001'><description>Stable</description><game_id>0001</game_id><category>A</category><rom name='a.bin'/></game>";
    // Reorder recognized attributes around a vendor attribute; child content is unchanged.
    let second = "<game id='0001' vendor='x' name='same'><description>Stable</description><game_id>0001</game_id><category>A</category><rom name='a.bin'/></game>";
    let diff = diff_documents(&document(first), &document(second))?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    let record = &diff.records[0];
    assert!(record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn game_vendor_attribute_gaps_preserve_separate_history_position_domains() -> TestResult {
    let first = "<game name='same' id='0001'><category>A</category><description>Stable</description><rom name='same.bin'/></game>";
    let second = "<game name='same' vendor='x' id='0001'><category>A</category><description>Stable</description><rom name='same.bin'/></game>";
    let diff = diff_documents(&document(first), &document(second))?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn reordering_native_child_fields_is_visible_independent_of_game_attributes() -> TestResult {
    let first = "<game name='same' id='0001'><description>Stable</description><category>A</category><game_id>id-1</game_id><rom name='a.bin'/></game>";
    let second = "<game name='same' id='0001'><category>A</category><description>Stable</description><game_id>id-1</game_id><rom name='a.bin'/></game>";
    let diff = diff_documents(&document(first), &document(second))?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    let record = &diff.records[0];
    assert!(record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

fn assert_rom_reorder_history(previous_roms: &str, current_roms: &str) -> TestResult {
    let previous = document(&format!(
        "<game name='same'><description>Stable</description>{previous_roms}</game>"
    ));
    let current = document(&format!(
        "<game name='same'><description>Stable</description>{current_roms}</game>"
    ));
    let diff = diff_documents(&previous, &current)?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    assert!(diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn history_detects_rom_sibling_reordering_without_requirement_changes() -> TestResult {
    assert_rom_reorder_history(
        "<rom name='a.bin' size='1'/><rom name='b.bin' size='2'/>",
        "<rom name='b.bin' size='2'/><rom name='a.bin' size='1'/>",
    )
}

#[test]
fn history_detects_reordering_repeated_rom_names_with_different_facts() -> TestResult {
    assert_rom_reorder_history(
        "<rom name='same.bin' size='1' serial='A'/><rom name='same.bin' size='2' serial='B'/>",
        "<rom name='same.bin' size='2' serial='B'/><rom name='same.bin' size='1' serial='A'/>",
    )
}

#[test]
fn history_detects_release_attribute_reordering() -> TestResult {
    let previous = document(
        "<game name='same'><description>Stable</description><release name='v1' region='US'/><rom name='same.bin'/></game>",
    );
    let current = document(
        "<game name='same'><description>Stable</description><release region='US' name='v1'/><rom name='same.bin'/></game>",
    );
    let diff = diff_documents(&previous, &current)?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    assert!(diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn release_vendor_attribute_gaps_do_not_change_history() -> TestResult {
    let previous = document(
        "<game name='same'><description>Stable</description><release name='v1' region='US'/><rom name='same.bin'/></game>",
    );
    let current = document(
        "<game name='same'><description>Stable</description><release name='v1' vendor='x' region='US'/><rom name='same.bin'/></game>",
    );
    let diff = diff_documents(&previous, &current)?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    Ok(())
}

#[test]
fn header_option_attributes_keep_a_local_rank_domain() -> TestResult {
    let game = "<game name='same'><description>Stable</description><rom name='a.bin'/></game>";
    let original = "<clrmamepro forcenodump='obsolete' header='filter'/>";
    let vendor_gap = "<clrmamepro forcenodump='obsolete' vendor='x' header='filter'/>";
    let reordered = "<clrmamepro header='filter' vendor='x' forcenodump='obsolete'/>";

    let unchanged = diff_documents(
        &document_with_header(original, game),
        &document_with_header(vendor_gap, game),
    )?;
    assert!(!unchanged.document_metadata_changed);
    assert!(!unchanged.records[0].metadata_changed);

    let reordered = diff_documents(
        &document_with_header(original, game),
        &document_with_header(reordered, game),
    )?;
    assert!(reordered.document_metadata_changed);
    assert!(!reordered.records[0].metadata_changed);

    let first_header = "<id>1</id><name>Consumer test</name><description>Consumer fixture</description><version>1</version><clrmamepro header='filter'/><romcenter plugin='plugin'/>";
    let reordered_header = "<clrmamepro header='filter'/><id>1</id><name>Consumer test</name><description>Consumer fixture</description><romcenter plugin='plugin'/><version>1</version>";
    let child_order = diff_documents(
        &format!("<datafile><header>{first_header}</header>{game}</datafile>"),
        &format!("<datafile><header>{reordered_header}</header>{game}</datafile>"),
    )?;
    assert!(child_order.document_metadata_changed);
    assert!(!child_order.records[0].metadata_changed);
    Ok(())
}

#[test]
fn public_occurrence_query_returns_lossless_native_rom_payloads_in_bulk() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let sha256 = "A".repeat(64);
    let canonical_sha256 = sha256.to_ascii_lowercase();
    import(
        &database,
        &directory,
        "payload.dat",
        &format!(
            "<datafile><header><id>1</id><name>Payloads</name><description>Payload fixture</description><version>1</version></header>\
             <game name='same'><description>Game</description>\
             <rom name='repeat.bin' size='0004' crc='AABBCCDD' \
                  md5='00112233445566778899AABBCCDDEEFF' \
                  sha1='00112233445566778899AABBCCDDEEFF00112233' \
                  sha256='{sha256}' status='' serial='' date='0000' mia='yes'/>\
             <rom name='repeat.bin' size='' crc='' sha256='Not-Hex' header='local'/>\
             </game></datafile>"
        ),
    )?;
    let ids = occurrence_ids(&mut connection, "no-intro-consumer-history")?;
    assert_eq!(ids.len(), 2);
    let occurrences = catalog_files::occurrences_for_ids(&database, &ids)?;
    assert_eq!(occurrences.len(), 2);
    let first = occurrences[0]
        .no_intro_dat_rom
        .as_ref()
        .ok_or("first native ROM payload missing")?;
    assert_eq!(first.name, "repeat.bin");
    assert_eq!(first.size_text.as_deref(), Some("0004"));
    assert_eq!(first.size, Some(4));
    assert_eq!(
        first.evidence_scope,
        catalog_files::NoIntroDatEvidenceScope::WholeFile
    );
    assert_eq!(first.crc_text.as_deref(), Some("aabbccdd"));
    assert_eq!(
        first.md5_text.as_deref(),
        Some("00112233445566778899aabbccddeeff")
    );
    assert_eq!(
        first.sha1_text.as_deref(),
        Some("00112233445566778899aabbccddeeff00112233")
    );
    assert_eq!(
        first.sha256_text.as_deref(),
        Some(canonical_sha256.as_str())
    );
    assert_eq!(first.status_text.as_deref(), Some(""));
    assert_eq!(first.serial_text.as_deref(), Some(""));
    assert_eq!(first.date_text.as_deref(), Some("0000"));
    assert_eq!(first.mia_text.as_deref(), Some("yes"));
    assert_eq!(first.source_order, 1);
    assert!(occurrences[0].digests.iter().any(|digest| {
        digest.algorithm == catalog_files::DigestAlgorithm::Sha256 && digest.scope == "whole_file"
    }));
    assert!(first.location.line > 0);
    let second = occurrences[1]
        .no_intro_dat_rom
        .as_ref()
        .ok_or("second native ROM payload missing")?;
    assert_eq!(second.size_text.as_deref(), Some(""));
    assert_eq!(second.size, None);
    assert_eq!(
        second.evidence_scope,
        catalog_files::NoIntroDatEvidenceScope::Unknown
    );
    assert_eq!(second.crc_text.as_deref(), Some(""));
    assert_eq!(second.md5_text, None);
    assert_eq!(second.sha1_text, None);
    assert_eq!(second.sha256_text.as_deref(), Some("Not-Hex"));
    assert_eq!(second.header_text.as_deref(), Some("local"));
    assert_eq!(second.source_order, 2);
    assert_ne!(occurrences[0].occurrence_id, occurrences[1].occurrence_id);
    Ok(())
}

#[test]
fn sha256_identity_crosses_catalogs_but_global_header_filter_blocks_uuid() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let sha256 = "b".repeat(64);
    for (key, header) in [
        ("no-intro-list-a", ""),
        ("no-intro-list-b", ""),
        ("no-intro-global-filter", "<clrmamepro header=''/>"),
    ] {
        let body = format!(
            "<datafile><header><id>1</id><name>List</name><description>List fixture</description><version>1</version>{header}</header>\
             <game name='same'><description>Game</description>\
             <rom name='shared.bin' size='4' sha256='{sha256}'/></game></datafile>"
        );
        import_catalog(&database, &directory, &format!("{key}.dat"), &body, key)?;
    }
    let a_ids = occurrence_ids(&mut connection, "no-intro-list-a")?;
    let b_ids = occurrence_ids(&mut connection, "no-intro-list-b")?;
    let filtered_ids = occurrence_ids(&mut connection, "no-intro-global-filter")?;
    let mut requested = a_ids.clone();
    requested.extend(b_ids.clone());
    requested.extend(filtered_ids.clone());
    let occurrences = catalog_files::occurrences_for_ids(&database, &requested)?;
    assert_eq!(occurrences.len(), 3);
    let a = occurrences
        .iter()
        .find(|occurrence| occurrence.occurrence_id == a_ids[0])
        .ok_or("first cross-catalog occurrence missing")?;
    let b = occurrences
        .iter()
        .find(|occurrence| occurrence.occurrence_id == b_ids[0])
        .ok_or("second cross-catalog occurrence missing")?;
    let filtered = occurrences
        .iter()
        .find(|occurrence| occurrence.occurrence_id == filtered_ids[0])
        .ok_or("global-filter occurrence missing")?;
    assert!(a.content_id.is_some());
    assert_eq!(
        a.content_id, b.content_id,
        "equal SHA-256 claims should bridge catalogs"
    );
    assert_eq!(
        a.no_intro_dat_rom
            .as_ref()
            .ok_or("first native ROM payload missing")?
            .header_text,
        None
    );
    assert_eq!(filtered.content_id, None);
    assert_eq!(
        filtered
            .digests
            .iter()
            .find(|digest| digest.algorithm == catalog_files::DigestAlgorithm::Sha256)
            .map(|digest| digest.scope.as_str()),
        Some("unknown"),
        "document-level clrmamepro header filters ROMs without a local header attribute"
    );
    Ok(())
}

#[test]
fn history_preserves_no_intro_game_fields_and_document_options() -> TestResult {
    let base = "<game name='same' id='1' cloneof='parent' cloneofid='0007'>\
        <description>Stable</description><category>A</category><category>A</category>\
        <game_id>one</game_id><game_id>one</game_id>\
        <release name='v1' region='US'/><release name='v1' region='US'/>\
        <rom name='same.bin' size='4'/></game>";
    for current in [
        base.replace("cloneofid='0007'", "cloneofid='0008'"),
        base.replace("<category>A</category>", "<category>B</category>"),
        base.replacen(
            "<category>A</category><category>A</category>",
            "<category>A</category>",
            1,
        ),
        base.replace("<game_id>one</game_id>", "<game_id>two</game_id>"),
        base.replace("name='v1'", "name='v2'"),
        base.replace("region='US'", "region='EU'"),
        base.replacen(
            "<release name='v1' region='US'/><release name='v1' region='US'/>",
            "<release name='v1' region='US'/>",
            1,
        ),
    ] {
        let diff = diff_documents(&document(base), &document(&current))?;
        assert!(!diff.document_metadata_changed);
        assert_eq!(diff.records.len(), 1);
        assert!(diff.records[0].metadata_changed);
        assert!(diff.records[0].requirement_changes.is_empty());
    }

    for (previous, current) in [
        ("", "<clrmamepro/>"),
        (
            "<clrmamepro/>",
            "<clrmamepro header='' forcenodump='obsolete'/>",
        ),
        ("", "<romcenter plugin=''/>"),
    ] {
        let before = format!(
            "<datafile><header><id>1</id><name>DAT</name><description>Header fixture</description><version>1</version>{previous}</header>{base}</datafile>"
        );
        let after = format!(
            "<datafile><header><id>1</id><name>DAT</name><description>Header fixture</description><version>1</version>{current}</header>{base}</datafile>"
        );
        let diff = diff_documents(&before, &after)?;
        assert!(diff.document_metadata_changed);
        assert_eq!(diff.records.len(), 1);
        assert!(!diff.records[0].metadata_changed);
    }
    Ok(())
}

#[test]
fn history_derives_declared_version_from_native_header() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let first = import(
        &database,
        &directory,
        "v1.dat",
        &document("<game name='one'><description>One</description><rom name='one.bin'/></game>")
            .replace("<version>1</version>", "<version>native-v1</version>"),
    )?;
    let second = import(
        &database,
        &directory,
        "v2.dat",
        &document("<game name='one'><description>One</description><rom name='one.bin'/></game>")
            .replace("<version>1</version>", "<version>native-v2</version>"),
    )?;
    let history =
        app::catalog_snapshot_history(&database, &CatalogKey::new("no-intro-consumer-history"))?;
    assert!(
        history.iter().any(|entry| entry.snapshot == first
            && entry.declared_version.as_deref() == Some("native-v1"))
    );
    assert!(
        history.iter().any(|entry| entry.snapshot == second
            && entry.declared_version.as_deref() == Some("native-v2"))
    );
    Ok(())
}

#[test]
fn relationship_provenance_derives_version_from_the_native_dat_header() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    import(
        &database,
        &directory,
        "relationships.dat",
        &document("<game name='child' cloneof='parent'><description>Child</description><rom name='child.bin'/></game>").replace(
            "<version>1</version>",
            "<version>native-relations</version>",
        ),
    )?;
    let relationships = app::explain_relationships(&database)?;
    assert_eq!(relationships.len(), 1);
    assert_eq!(
        relationships[0]
            .source
            .as_ref()
            .and_then(|source| source.declared_version.as_deref()),
        Some("native-relations")
    );
    Ok(())
}

#[test]
fn native_cloneof_names_are_not_archive_ids_or_cloneofid_references() -> TestResult {
    use mame_coalesce::domain::RelationshipEndpoint;

    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(
        &database,
        &directory,
        "literal-parents.dat",
        &document(
            "<game name='numeric' cloneof='0007'><description>Numeric</description><rom name='numeric.bin'/></game>\
             <game name='empty' cloneof=''><description>Empty</description><rom name='empty.bin'/></game>\
             <game name='id-only' cloneofid='0007'><description>ID only</description><rom name='id.bin'/></game>",
        ),
    )?;
    let relationships = app::explain_relationships(&database)?;
    assert_eq!(relationships.len(), 3);
    let mut names = Vec::new();
    for relationship in relationships {
        let RelationshipEndpoint::CatalogRecord(subject) = relationship.claim.subject else {
            return Err("DAT clone subject must be its actual catalog set".into());
        };
        assert_eq!(subject.snapshot, snapshot);
        assert!(subject.owner_set_id.is_some());
        if let RelationshipEndpoint::NoIntroDatIdReference {
            snapshot: target_snapshot,
            declaring_set,
            declared_id,
        } = &relationship.claim.target
        {
            assert_eq!(target_snapshot, &snapshot);
            assert_eq!(Some(*declaring_set), subject.owner_set_id);
            assert_eq!(declared_id, "0007");
            assert_eq!(relationship.source_field.as_deref(), Some("cloneofid"));
            continue;
        }
        let RelationshipEndpoint::CatalogRecord(target) = relationship.claim.target else {
            return Err("DAT cloneof names must remain name-based selectors".into());
        };
        assert_eq!(target.snapshot, snapshot);
        assert!(target.owner_set_id.is_none());
        assert_eq!(relationship.source_field.as_deref(), Some("cloneof"));
        names.push(target.key.as_str().to_owned());
    }
    names.sort();
    assert_eq!(names, ["", "0007"]);
    Ok(())
}

#[test]
fn native_cloneof_assertion_review_guard_checks_owned_positioned_rows() -> TestResult {
    let fixture = NativeCloneofGuardFixture::new()?;
    let mut connection = fixture.connection()?;
    insert_review(
        &mut connection,
        "review-native-cloneof-primary",
        &fixture.native_key,
        "accepted",
        None,
    )?;

    insert_review(
        &mut connection,
        "review-native-cloneofid-primary",
        &fixture.cloneofid_only_key,
        "accepted",
        None,
    )?;
    for (index, key) in fixture.invalid_keys().into_iter().enumerate() {
        assert_guard_rejects(
            insert_review(
                &mut connection,
                &format!("review-native-cloneof-invalid-{index}"),
                &key,
                "accepted",
                None,
            ),
            "relationship review references an unknown or unpublished assertion",
        );
    }
    Ok(())
}

#[test]
fn native_cloneof_assertion_supersession_guard_checks_owned_positioned_rows() -> TestResult {
    let fixture = NativeCloneofGuardFixture::new()?;
    let mut connection = fixture.connection()?;
    insert_stored_assertion(&mut connection, "guard-test:review-base", "user_conclusion")?;
    sql_query(
        "INSERT INTO catalog_relationship_rationales (relationship_id, reason) \
         SELECT relationship_id,'Published guard-test conclusion' FROM catalog_relationships WHERE assertion_key='guard-test:review-base'",
    )
    .execute(&mut connection)?;
    sql_query(
        "INSERT INTO catalog_relationship_evidence_publications (relationship_id, evidence_kind) \
         SELECT relationship_id,'rationale' FROM catalog_relationships WHERE assertion_key='guard-test:review-base'",
    )
    .execute(&mut connection)?;
    insert_review(
        &mut connection,
        "review-native-cloneof-superseding",
        "guard-test:review-base",
        "superseded",
        Some(&fixture.native_key),
    )?;

    insert_review(
        &mut connection,
        "review-native-cloneofid-superseding",
        "guard-test:review-base",
        "superseded",
        Some(&fixture.cloneofid_only_key),
    )?;
    for (index, key) in fixture.invalid_keys().into_iter().enumerate() {
        assert_guard_rejects(
            insert_review(
                &mut connection,
                &format!("review-native-cloneof-invalid-successor-{index}"),
                "guard-test:review-base",
                "superseded",
                Some(&key),
            ),
            "relationship review references an unknown or unpublished assertion",
        );
    }
    Ok(())
}

#[test]
fn native_cloneof_assertion_support_guard_checks_owned_positioned_rows() -> TestResult {
    let fixture = NativeCloneofGuardFixture::new()?;
    let mut connection = fixture.connection()?;
    insert_stored_assertion(
        &mut connection,
        "guard-test:derived-candidate",
        "derived_candidate",
    )?;
    insert_support(
        &mut connection,
        "guard-test:derived-candidate",
        0,
        &fixture.native_key,
    )?;

    insert_support(
        &mut connection,
        "guard-test:derived-candidate",
        1,
        &fixture.cloneofid_only_key,
    )?;
    for (index, key) in fixture.invalid_keys().into_iter().enumerate() {
        assert_guard_rejects(
            insert_support(
                &mut connection,
                "guard-test:derived-candidate",
                i64::try_from(index + 2)?,
                &key,
            ),
            "support requires published relationship evidence",
        );
    }
    Ok(())
}

#[test]
fn public_relationship_api_accepts_native_cloneof_as_support() -> TestResult {
    let fixture = NativeCloneofGuardFixture::new()?;
    let supporting = RelationshipAssertionKey::new(fixture.native_key.clone());
    let claim = RelationshipClaim {
        relation_type: RelationshipType::CatalogCorrection,
        subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "guard-test",
            "subject",
        )),
        target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "guard-test",
            "target",
        )),
        origin: RelationshipOrigin::DerivedCandidate {
            rule: mame_coalesce::domain::RelationshipRule::new(
                "native-cloneof-support-test",
                "v1",
                "Native clone support witness",
            )?,
            supporting_assertions: vec![supporting.clone()],
        },
        evidence: mame_coalesce::domain::RelationshipEvidence::Rationale {
            reason: "native cloneof source evidence".to_owned(),
        },
    };
    let candidate = app::record_relationship(&fixture.database, &claim)?;
    let mut connection = fixture.connection()?;
    let found = sql_query(
        "SELECT COUNT(*) AS count FROM catalog_relationship_evidence \
         WHERE relationship_id=(SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?) \
           AND supporting_relationship_id=(SELECT relationship_id FROM catalog_relationships WHERE assertion_key=?)",
    )
    .bind::<Text, _>(candidate.as_str())
    .bind::<Text, _>(supporting.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(found.count, 1);
    Ok(())
}

#[test]
fn native_cloneof_key_namespace_cannot_be_shadowed_by_stored_assertions() -> TestResult {
    let fixture = NativeCloneofGuardFixture::new()?;
    let mut connection = fixture.connection()?;
    insert_stored_assertion(
        &mut connection,
        "guard-test:stored-control",
        "user_conclusion",
    )?;
    assert_guard_rejects(
        insert_stored_assertion(&mut connection, &fixture.native_key, "user_conclusion"),
        "catalog relationship identity",
    );

    let stored =
        sql_query("SELECT COUNT(*) AS count FROM relationship_assertions WHERE assertion_key = ?")
            .bind::<Text, _>(&fixture.native_key)
            .get_result::<CountRow>(&mut connection)?;
    assert_eq!(stored.count, 0);
    Ok(())
}

#[test]
fn history_hash_fields_canonicalize_valid_and_preserve_invalid_or_absent_values() -> TestResult {
    let valid_upper = "AB".repeat(32);
    let valid_lower = valid_upper.to_ascii_lowercase();
    let upper_xml = document(&format!(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4' sha256='{valid_upper}'/></game>"
    ));
    let lower_xml = document(&format!(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4' sha256='{valid_lower}'/></game>"
    ));
    let canonical = diff_documents(&upper_xml, &lower_xml)?;
    assert!(!canonical.document_metadata_changed);
    assert_eq!(canonical.records.len(), 1);
    assert!(!canonical.records[0].metadata_changed);
    assert!(canonical.records[0].requirement_changes.is_empty());

    let invalid_before = document(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4' sha256='not-a-hash'/></game>",
    );
    let invalid_after = document(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4' sha256='NOT-A-HASH'/></game>",
    );
    let invalid = diff_documents(&invalid_before, &invalid_after)?;
    assert_eq!(invalid.records.len(), 1);
    assert!(invalid.records[0].requirement_changes.iter().any(|change| {
        change.other_evidence_changed && !change.hash_changed && !change.size_changed
    }));

    let absent = document(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4'/></game>",
    );
    let explicitly_empty = document(
        "<game name='same'><description>Game</description><rom name='same.bin' size='4' crc=''/></game>",
    );
    let presence = diff_documents(&absent, &explicitly_empty)?;
    assert_eq!(presence.records.len(), 1);
    assert!(
        presence.records[0]
            .requirement_changes
            .iter()
            .any(|change| {
                change.other_evidence_changed && !change.hash_changed && !change.size_changed
            })
    );
    Ok(())
}

#[test]
fn history_classifies_usable_sha256_changes_as_hash_changes() -> TestResult {
    let before_hash = "a".repeat(64);
    let after_hash = "b".repeat(64);
    let previous = document(&format!(
        "<game name='same'><description>Stable</description><rom name='same.bin' size='4' sha256='{before_hash}'/></game>"
    ));
    let current = document(&format!(
        "<game name='same'><description>Stable</description><rom name='same.bin' size='4' sha256='{after_hash}'/></game>"
    ));
    let diff = diff_documents(&previous, &current)?;
    assert_eq!(diff.records.len(), 1);
    let changes = &diff.records[0].requirement_changes;
    assert_eq!(changes.len(), 1);
    assert!(changes[0].hash_changed);
    assert!(!changes[0].size_changed);
    Ok(())
}

#[test]
fn history_keeps_unknown_scope_sha256_changes_as_other_evidence() -> TestResult {
    let before_hash = "a".repeat(64);
    let after_hash = "b".repeat(64);
    let previous = document_with_header(
        "<clrmamepro header=''/>",
        &format!(
            "<game name='same'><description>Stable</description><rom name='same.bin' size='4' sha256='{before_hash}'/></game>"
        ),
    );
    let current = document_with_header(
        "<clrmamepro header=''/>",
        &format!(
            "<game name='same'><description>Stable</description><rom name='same.bin' size='4' sha256='{after_hash}'/></game>"
        ),
    );
    let diff = diff_documents(&previous, &current)?;
    assert_eq!(diff.records.len(), 1);
    let changes = &diff.records[0].requirement_changes;
    assert_eq!(changes.len(), 1);
    assert!(!changes[0].hash_changed);
    assert!(!changes[0].size_changed);
    assert!(changes[0].other_evidence_changed);
    Ok(())
}

#[test]
fn repeated_set_names_reorder_without_ordinal_or_producer_id_continuity() -> TestResult {
    let first = "<game name='same' id='duplicate' cloneofid='same-id'><description>A</description><rom name='a.bin'/></game>\
        <game name='same' id='duplicate' cloneofid='same-id'><description>B</description><rom name='b.bin'/></game>";
    let second = "<game name='same' id='duplicate' cloneofid='same-id'><description>B</description><rom name='b.bin'/></game>\
        <game name='same' id='duplicate' cloneofid='same-id'><description>A</description><rom name='a.bin'/></game>";
    let diff = diff_documents(&document(first), &document(second))?;
    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), 1);
    let record = &diff.records[0];
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    assert_eq!(
        record.correspondence,
        mame_coalesce::domain::SnapshotRecordCorrespondence::ExactFacts
    );
    Ok(())
}
