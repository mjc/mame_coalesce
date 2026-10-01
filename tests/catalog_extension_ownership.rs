#![allow(clippy::expect_used)]

use std::fs;

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    prelude::Connection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct ExtensionOwnerRow {
    #[diesel(sql_type = Text)]
    record_kind: String,
    #[diesel(sql_type = Text)]
    field_name: String,
    #[diesel(sql_type = BigInt)]
    owner_set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_occurrence_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    occurrence_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    asset_name: Option<String>,
}

#[test]
fn repeated_name_extensions_keep_their_numeric_set_and_asset_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| std::io::Error::other("non-UTF-8 database path"))?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    let document_path = directory.path().join("repeated.xml");
    fs::write(
        &document_path,
        "<datafile>\n\
         <header><name>Repeated names</name></header>\n\
         <game name=\"shared\" vendor_game=\"first\">\n\
           <rom name=\"shared.rom\" size=\"1\" vendor_rom=\"first\"/>\n\
         </game>\n\
         <game name=\"shared\" vendor_game=\"second\">\n\
           <rom name=\"shared.rom\" size=\"2\" vendor_rom=\"second\"/>\n\
         </game>\n\
         </datafile>",
    )?;
    let document_path = Utf8PathBuf::from_path_buf(document_path)
        .map_err(|_| std::io::Error::other("non-UTF-8 document path"))?;
    let request = CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::Logiqx,
        source_key: PublishingSourceKey::new("extension-owner-source"),
        source_display_name: "Extension owner source".to_owned(),
        catalog_key: CatalogKey::new("extension-owner-catalog"),
        catalog_display_name: "Extension owner catalog".to_owned(),
        scope: CatalogScope::Unknown,
    };
    let report = app::import_catalog(&database, &request)?;
    let snapshot_key = report
        .snapshot_key
        .ok_or("import did not publish a snapshot")?;

    let rows = sql_query(
        "SELECT extension.record_kind, extension.field_name, extension.owner_set_id, \
                owner_set.set_name, extension.owner_occurrence_id, occurrence.occurrence_order, \
                claim.name AS asset_name \
         FROM snapshot_extensions AS extension \
         JOIN catalog_sets AS owner_set \
           ON owner_set.set_id = extension.owner_set_id \
         LEFT JOIN asset_occurrences AS occurrence \
           ON occurrence.occurrence_id = extension.owner_occurrence_id \
          AND occurrence.record_id = extension.owner_set_id \
         LEFT JOIN logiqx_rom_claims AS claim \
           ON claim.occurrence_id = occurrence.occurrence_id \
         WHERE extension.snapshot_key = ? \
           AND extension.field_name IN ('vendor_game', 'vendor_rom') \
         ORDER BY extension.record_kind, extension.source_line",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .load::<ExtensionOwnerRow>(&mut connection)?;

    let game_rows: Vec<_> = rows
        .iter()
        .filter(|row| row.record_kind == "game")
        .collect();
    let rom_rows: Vec<_> = rows.iter().filter(|row| row.record_kind == "rom").collect();
    assert_eq!(game_rows.len(), 2);
    assert_eq!(rom_rows.len(), 2);
    assert!(game_rows.iter().all(|row| {
        row.field_name == "vendor_game"
            && row.set_name == "shared"
            && row.owner_occurrence_id.is_none()
    }));
    assert_ne!(game_rows[0].owner_set_id, game_rows[1].owner_set_id);

    assert!(rom_rows.iter().all(|row| {
        row.field_name == "vendor_rom"
            && row.set_name == "shared"
            && row.occurrence_order == Some(0)
            && row.asset_name.as_deref() == Some("shared.rom")
            && row.owner_occurrence_id.is_some()
    }));
    assert_ne!(rom_rows[0].owner_set_id, rom_rows[1].owner_set_id);
    assert_ne!(
        rom_rows[0].owner_occurrence_id,
        rom_rows[1].owner_occurrence_id
    );
    assert_eq!(
        rom_rows
            .iter()
            .map(|row| row.owner_set_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );

    Ok(())
}
