#![allow(clippy::expect_used)]

use std::error::Error;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
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

fn request(source: &camino::Utf8Path, catalog: &str) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: source.to_owned(),
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("test"),
        source_display_name: "test".into(),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: catalog.into(),
        scope: CatalogScope::Unknown,
    }
}

fn count(conn: &mut SqliteConnection, table: &str) -> diesel::QueryResult<i64> {
    sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
        .get_result::<Count>(conn)
        .map(|row| row.count)
}

#[test]
fn repeated_lists_share_file_facts_without_losing_declarations() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("UTF-8 test path");
    let path = root.join("catalog.sqlite");
    let database = Database::open(&path)?;
    let source = root.join("roms.xml");
    std::fs::write(
        &source,
        r#"<mame build="0.289" debug="no" mameconfig="10"><machine name="game"><description>Game</description><rom name="rom" size="8" crc="11111111" sha1="2222222222222222222222222222222222222222" offset="0"/></machine></mame>"#,
    )?;
    for catalog in ["first", "second"] {
        let report = app::import_catalog(&database, &request(&source, catalog))?;
        assert_eq!(
            report.status,
            app::CatalogImportStatus::Succeeded,
            "{report:?}"
        );
    }
    let mut conn = SqliteConnection::establish(path.as_str())?;
    for (table, expected) in [
        ("asset_occurrences", 2),
        ("catalog_contents", 1),
        ("occurrence_digest_assertions", 4),
        ("shared_file_sizes", 1),
        ("shared_file_hashes", 2),
    ] {
        assert_eq!(count(&mut conn, table)?, expected, "{table}");
    }
    Ok(())
}

#[test]
fn failed_import_rolls_back_shared_facts_and_connection_reuse_is_clean()
-> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("UTF-8 test path");
    let path = root.join("catalog.sqlite");
    let database = Database::open(&path)?;
    let source = root.join("roms.xml");
    let mut xml = String::from(r#"<mame build="0.289" debug="no" mameconfig="10">"#);
    for index in 0..65 {
        use std::fmt::Write;
        write!(
            xml,
            r#"<machine name="game{index}"><description>Game</description><rom name="rom" size="8" sha1="2222222222222222222222222222222222222222"/></machine>"#
        )?;
    }
    // The first 64 machines have flushed before this malformed tail is read.
    std::fs::write(&source, format!("{xml}</wrong>"))?;
    let report = app::import_catalog(&database, &request(&source, "rollback"))?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    let mut conn = SqliteConnection::establish(path.as_str())?;
    for table in [
        "asset_occurrences",
        "catalog_contents",
        "shared_file_sizes",
        "shared_file_hashes",
    ] {
        assert_eq!(count(&mut conn, table)?, 0, "{table} must roll back");
    }
    std::fs::write(&source, format!("{xml}</mame>"))?;
    let report = app::import_catalog(&database, &request(&source, "rollback"))?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert_eq!(count(&mut conn, "asset_occurrences")?, 65);
    assert_eq!(count(&mut conn, "catalog_contents")?, 1);
    assert_eq!(count(&mut conn, "shared_file_sizes")?, 1);
    assert_eq!(count(&mut conn, "shared_file_hashes")?, 1);
    Ok(())
}
