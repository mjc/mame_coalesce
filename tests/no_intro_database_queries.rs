#![allow(clippy::expect_used, clippy::panic)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{
        NoIntroDatabaseDigestValue, NoIntroDatabaseEvidenceScope, NoIntroDatabaseFilePayload,
        OccurrenceId, OccurrenceKind, occurrences_for_ids,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
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
