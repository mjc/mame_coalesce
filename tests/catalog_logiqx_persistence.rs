use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct RomFields {
    #[diesel(sql_type = Text)]
    size_text: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
    #[diesel(sql_type = Text)]
    dump_status: String,
    #[diesel(sql_type = Bool)]
    status_was_present: bool,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[test]
fn logiqx_rom_disk_and_sample_each_have_a_native_source_occurrence()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.xml"))
        .map_err(|_| "non-UTF-8 path")?;
    std::fs::write(
        &document_path,
        "<datafile><game name='all'><description>All media</description><rom name='rom' size='0004'/><disk name='disk' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/><sample name='sample'/></game></datafile>",
    )?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("all-media"),
            source_display_name: "All media".into(),
            catalog_key: CatalogKey::new("all-media"),
            catalog_display_name: "All media".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let occurrences = sql_query("SELECT count(*) AS count FROM asset_occurrences")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(
        occurrences.count, 3,
        "ROM, disk and sample are distinct source claims"
    );
    let disk_identity = sql_query("SELECT count(*) AS count FROM asset_occurrences WHERE claim_kind = 'logiqx_disk' AND content_uuid IS NOT NULL")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(
        disk_identity.count, 0,
        "logical disk hashes are not whole-container identity"
    );
    let rom = sql_query("SELECT size_text, size, dump_status, status_was_present, source_order FROM logiqx_rom_claims")
        .get_result::<RomFields>(&mut connection)?;
    assert_eq!(rom.size_text, "0004");
    assert_eq!(rom.size, 4);
    assert_eq!(rom.dump_status, "good");
    assert!(!rom.status_was_present);
    assert_eq!(rom.source_order, 1);
    assert!(
        sql_query("INSERT INTO asset_occurrences(record_id, occurrence_order, claim_kind) SELECT record_id, 3, 'logiqx_sample' FROM asset_occurrences LIMIT 1")
            .execute(&mut connection)
            .is_err(),
        "published source owners must not accept new media occurrences"
    );
    Ok(())
}
