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
    domain::{CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey},
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
