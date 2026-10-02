use camino::Utf8PathBuf;
use diesel::{Connection, QueryableByName, RunQueryDsl, sql_query, sql_types::BigInt};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[test]
fn every_declared_merge_has_one_native_identity_even_without_a_parent() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document_path,
        r#"<mame mameconfig="10"><machine name="child"><description>Child</description>
        <rom name="a.bin" size="1" merge="parent.bin"/>
        <rom name="b.bin" size="1" merge=""/>
        <rom name="unmerged.bin" size="1"/>
        <disk name="a.chd" merge="parent.chd"/>
        <disk name="b.chd" merge=""/>
        </machine></mame>"#,
    )?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("native-merge-owners"),
            source_display_name: "Native merge owners".into(),
            catalog_key: CatalogKey::new("native-merge-owners"),
            catalog_display_name: "Native merge owners".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = diesel::SqliteConnection::establish(database_path.as_str())?;
    let registered = sql_query(
        "SELECT count(*) AS count FROM catalog_relationships AS identity \
         JOIN reported_catalog_relationships AS reported USING(relationship_id) \
         WHERE identity.origin='source' AND reported.source_reference_kind \
             IN ('mame_rom_merge','mame_disk_merge')",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        registered.count, 4,
        "unresolved declarations still own identities"
    );
    let native = sql_query(
        "SELECT (SELECT count(*) FROM mame_rom_merges) \
              + (SELECT count(*) FROM mame_disk_merges) AS count",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(native.count, registered.count);
    let copies = sql_query(
        "SELECT (SELECT count(*) FROM pragma_table_info('mame_rom_claims') \
                 WHERE name='merge_name') \
              + (SELECT count(*) FROM pragma_table_info('mame_disk_claims') \
                 WHERE name='merge_name') AS count",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        copies.count, 0,
        "merge literals have no copied media-payload columns"
    );
    let empty = sql_query(
        "SELECT (SELECT count(*) FROM mame_rom_merges WHERE merge_name='') \
              + (SELECT count(*) FROM mame_disk_merges WHERE merge_name='') AS count",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(empty.count, 2);
    let generic = sql_query(
        "SELECT count(*) AS count FROM relationship_assertions WHERE origin='source_assertion'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(generic.count, 0);
    Ok(())
}
