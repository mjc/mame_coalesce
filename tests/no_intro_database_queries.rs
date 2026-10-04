#![allow(clippy::expect_used, clippy::panic)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{
        NoIntroDatabaseDigestValue, NoIntroDatabaseEvidenceScope, NoIntroDatabaseFilePayload,
        OccurrenceId, OccurrenceKind, occurrences_for_ids,
    },
    catalog_no_intro_database::{self, NoIntroDatabasePageLimit},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct OccurrenceRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[derive(QueryableByName)]
struct NativeOwnerRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct TriggerDefinition {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

fn reject_corrupt_archive_extent(update: &str, key: &str) -> TestResult {
    reject_corrupt_extent(
        "<datafile><game name='g'><archive name='a'/><archive name='b'/></game></datafile>",
        "no_intro_archive_descriptions",
        update,
        key,
    )
}

fn reject_corrupt_extent(xml: &str, table: &str, update: &str, key: &str) -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(&document_path, xml)?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new(key),
            source_display_name: "Extent corruption witness".into(),
            catalog_key: CatalogKey::new(key),
            catalog_display_name: "Extent corruption witness".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot: SnapshotKey = report.snapshot_key.ok_or("missing published snapshot")?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let triggers = sql_query(
        "SELECT name, sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
    )
    .bind::<Text, _>(table)
    .load::<TriggerDefinition>(&mut connection)?;
    assert!(!triggers.is_empty(), "native extent owner must be guarded");
    connection.batch_execute("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")?;
    for trigger in &triggers {
        connection.batch_execute(&format!("DROP TRIGGER \"{}\"", trigger.name))?;
    }
    let update_result = connection.batch_execute(update);
    for trigger in &triggers {
        connection.batch_execute(&trigger.sql)?;
    }
    connection.batch_execute("PRAGMA ignore_check_constraints=OFF; PRAGMA foreign_keys=ON;")?;
    update_result?;

    assert!(
        catalog_no_intro_database::games_for_snapshot(
            &database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )
        .is_err(),
        "reader accepted corrupt extent: {update}"
    );
    Ok(())
}

#[test]
fn native_extent_reader_checks_storage_classes_parentage_and_order() -> TestResult {
    for (key, update) in [
        (
            "extent-fractional",
            "UPDATE no_intro_archive_descriptions SET source_end_column=source_end_column + 0.5",
        ),
        (
            "extent-text",
            "UPDATE no_intro_archive_descriptions SET source_end_line='not-a-line'",
        ),
        (
            "extent-blob",
            "UPDATE no_intro_archive_descriptions SET source_end_column=x'02'",
        ),
        (
            "extent-outside-parent",
            "UPDATE no_intro_archive_descriptions SET source_end_line=(SELECT source_end_line FROM no_intro_database_games WHERE set_id=no_intro_archive_descriptions.set_id), source_end_column=(SELECT source_end_column + 1 FROM no_intro_database_games WHERE set_id=no_intro_archive_descriptions.set_id)",
        ),
        (
            "extent-siblings-out-of-order",
            "UPDATE no_intro_archive_descriptions SET source_end_line=(SELECT later.source_end_line FROM no_intro_archive_descriptions AS later WHERE later.set_id=no_intro_archive_descriptions.set_id AND later.source_order=1), source_end_column=(SELECT later.source_end_column FROM no_intro_archive_descriptions AS later WHERE later.set_id=no_intro_archive_descriptions.set_id AND later.source_order=1) WHERE source_order=0",
        ),
    ] {
        reject_corrupt_archive_extent(update, key)?;
    }
    Ok(())
}

#[test]
fn sibling_extents_are_checked_across_mixed_native_families() -> TestResult {
    let xml = "<datafile><header><author>A</author><version>V</version></header><game name='g'><archive/><source><details/><serials/><file/></source><release><details/><serials/><file/></release></game></datafile>";
    for (table, later, parent_column, first_order) in [
        (
            "no_intro_archive_descriptions",
            "no_intro_dump_sources",
            "set_id",
            0,
        ),
        ("no_intro_dump_sources", "no_intro_releases", "set_id", 1),
        (
            "no_intro_dump_details",
            "no_intro_dump_serials",
            "dump_source_id",
            0,
        ),
        (
            "no_intro_dump_serials",
            "no_intro_dump_files",
            "dump_source_id",
            1,
        ),
        (
            "no_intro_release_details",
            "no_intro_release_serials",
            "release_id",
            0,
        ),
        (
            "no_intro_release_serials",
            "no_intro_release_files",
            "release_id",
            1,
        ),
        (
            "no_intro_header_fields",
            "no_intro_header_fields",
            "snapshot_key",
            0,
        ),
    ] {
        let later_order = first_order + 1;
        let update = format!(
            "UPDATE {table} SET source_end_line=(SELECT later.source_end_line FROM {later} AS later WHERE later.{parent_column}={table}.{parent_column} AND later.source_order={later_order}), source_end_column=(SELECT later.source_end_column FROM {later} AS later WHERE later.{parent_column}={table}.{parent_column} AND later.source_order={later_order}) WHERE source_order={first_order}"
        );
        reject_corrupt_extent(xml, table, &update, table)?;
    }
    Ok(())
}

#[test]
fn a_game_page_rejects_overlap_with_its_lookahead_game() -> TestResult {
    reject_corrupt_extent(
        "<datafile><game name='a'/><game name='b'/></datafile>",
        "no_intro_database_games",
        "UPDATE no_intro_database_games SET source_end_line=(SELECT later.source_end_line FROM no_intro_database_games AS later JOIN catalog_sets AS sets ON sets.set_id=later.set_id WHERE sets.list_order=1), source_end_column=(SELECT later.source_end_column FROM no_intro_database_games AS later JOIN catalog_sets AS sets ON sets.set_id=later.set_id WHERE sets.list_order=1) WHERE set_id=(SELECT set_id FROM catalog_sets WHERE list_order=0)",
        "game-overlap",
    )
}

fn assert_source_payload(
    occurrence: &mame_coalesce::catalog_files::CatalogFileOccurrence,
    expected: &NativeOwnerRow,
) {
    let source = match occurrence
        .no_intro_database_file
        .as_ref()
        .expect("source file payload")
    {
        NoIntroDatabaseFilePayload::Source(source) => source,
        NoIntroDatabaseFilePayload::Release(_) => panic!("source occurrence returned release"),
    };
    assert_eq!(source.dump_source_id.as_i64(), expected.owner_id);
    assert_eq!(source.source_order, expected.source_order);
    assert_eq!(
        source.location,
        mame_coalesce::catalog_files::SourceLocation {
            line: expected.source_line,
            column: expected.source_column,
        }
    );
    assert_eq!(
        occurrence.provenance.native_occurrence_location,
        Some(source.location)
    );
    assert_eq!(source.source_size.as_deref(), Some("0007"));
    assert_eq!(source.size, Some(7));
    assert_eq!(source.forcename.as_deref(), Some("source-forced.bin"));
    assert_eq!(occurrence.provenance.asset_name, None);
    assert_eq!(occurrence.content_id, None);
    assert_eq!(source.evidence_scope, NoIntroDatabaseEvidenceScope::Unknown);
    assert_eq!(
        source.digests.crc32,
        Some(NoIntroDatabaseDigestValue::Invalid("not-crc32".to_owned()))
    );
    assert_eq!(
        source.digests.sha256,
        Some(NoIntroDatabaseDigestValue::Invalid("not-sha256".to_owned()))
    );
    assert_eq!(
        source.origin_sha256,
        Some(NoIntroDatabaseDigestValue::Invalid(
            "not-origin-sha256".to_owned()
        ))
    );
    assert!(occurrence.digests.is_empty());
}

fn assert_release_payload(
    occurrence: &mame_coalesce::catalog_files::CatalogFileOccurrence,
    expected: &NativeOwnerRow,
) {
    let release = match occurrence
        .no_intro_database_file
        .as_ref()
        .expect("release file payload")
    {
        NoIntroDatabaseFilePayload::Release(release) => release,
        NoIntroDatabaseFilePayload::Source(_) => panic!("release occurrence returned source"),
    };
    assert_eq!(release.release_id.as_i64(), expected.owner_id);
    assert_eq!(release.source_order, expected.source_order);
    assert_eq!(
        release.location,
        mame_coalesce::catalog_files::SourceLocation {
            line: expected.source_line,
            column: expected.source_column,
        }
    );
    assert_eq!(
        occurrence.provenance.native_occurrence_location,
        Some(release.location)
    );
    assert_eq!(release.source_size.as_deref(), Some("0009"));
    assert_eq!(release.size, Some(9));
    assert_eq!(release.forcename.as_deref(), Some("release-forced.cue"));
    assert_eq!(occurrence.provenance.asset_name, None);
    assert_eq!(occurrence.content_id, None);
    assert_eq!(
        release.evidence_scope,
        NoIntroDatabaseEvidenceScope::Unknown
    );
    assert_eq!(
        release.digests.md5,
        Some(NoIntroDatabaseDigestValue::Invalid("not-md5".to_owned()))
    );
}

#[test]
fn public_bulk_lookup_keeps_no_intro_source_and_release_file_owners_distinct() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(
        &document_path,
        r#"<datafile><game name="native-game">
          <source><file bad="source-bad" date="source-date" extension="bin"
            filter="source-filter" forcename="source-forced.bin"
            forcescenename="source-scene" format="source-format" header="source-header"
            id="source-id" item="source-item" mia="source-mia" note="source-note"
            origin_size="origin-size" serial="source-serial" size="0007"
            unique="source-unique" update_type="source-update" version="source-version"
            crc32="not-crc32" sha256="not-sha256" origin_sha256="not-origin-sha256"/>
          </source>
          <release><file bad="release-bad" extension="cue"
            forcename="release-forced.cue" forcescenename="release-scene"
            format="release-format" header="release-header" id="release-id"
            item="release-item" note="release-note" serial="release-serial"
            size="0009" update_type="release-update" version="release-version"
            md5="not-md5"/></release>
        </game></datafile>"#,
    )?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("native-query-source"),
            source_display_name: "Native query source".to_owned(),
            catalog_key: CatalogKey::new("native-query-catalog"),
            catalog_display_name: "Native query catalog".to_owned(),
            scope: CatalogScope::Unknown,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let ids = sql_query("SELECT occurrence_id FROM asset_occurrences ORDER BY occurrence_id")
        .load::<OccurrenceRow>(&mut connection)?
        .into_iter()
        .map(|row| OccurrenceId::from_database(row.occurrence_id))
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    let expected_owners = sql_query(
        "SELECT occurrence_id, dump_source_id AS owner_id, source_order, source_line, source_column \
           FROM no_intro_dump_files \
         UNION ALL \
         SELECT occurrence_id, release_id AS owner_id, source_order, source_line, source_column \
           FROM no_intro_release_files \
         ORDER BY occurrence_id",
    )
    .load::<NativeOwnerRow>(&mut connection)?;

    let occurrences = occurrences_for_ids(&database, &[ids[0], ids[1], ids[0]])?;
    assert_eq!(occurrences.len(), 2, "repeated requests retain one owner");
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.provenance.occurrence_kind)
            .collect::<Vec<_>>(),
        vec![
            OccurrenceKind::NoIntroDatabaseSourceFile,
            OccurrenceKind::NoIntroDatabaseReleaseFile,
        ]
    );
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.occurrence_id.database_value())
            .collect::<Vec<_>>(),
        expected_owners
            .iter()
            .map(|row| row.occurrence_id)
            .collect::<Vec<_>>()
    );

    assert_source_payload(&occurrences[0], &expected_owners[0]);
    assert_release_payload(&occurrences[1], &expected_owners[1]);
    Ok(())
}
