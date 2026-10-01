#![allow(clippy::expect_used)]

use std::fs;

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[test]
fn repeated_name_vendor_extensions_are_recoverable_from_original_source()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| std::io::Error::other("non-UTF-8 database path"))?;
    let database = Database::open(&database_path)?;

    let source_bytes: &[u8] = b"<datafile>\n\
         <header><name>Repeated names</name></header>\n\
         <game name=\"shared\" vendor_game=\"first\">\n\
           <rom name=\"shared.rom\" size=\"1\" vendor_rom=\"first\"/>\n\
         </game>\n\
         <game name=\"shared\" vendor_game=\"second\">\n\
           <rom name=\"shared.rom\" size=\"2\" vendor_rom=\"second\"/>\n\
         </game>\n\
         </datafile>";
    let document_path = directory.path().join("repeated.xml");
    fs::write(&document_path, source_bytes)?;
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

    let recovered_source = app::load_snapshot_source(&database, &snapshot_key)?;
    assert_eq!(recovered_source.as_slice(), source_bytes);
    for vendor_value in [
        b"vendor_game=\"first\"".as_slice(),
        b"vendor_game=\"second\"".as_slice(),
        b"vendor_rom=\"first\"".as_slice(),
        b"vendor_rom=\"second\"".as_slice(),
    ] {
        assert!(
            recovered_source
                .windows(vendor_value.len())
                .any(|window| window == vendor_value),
            "missing vendor literal: {}",
            String::from_utf8_lossy(vendor_value)
        );
    }
    Ok(())
}
