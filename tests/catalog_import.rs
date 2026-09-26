use std::{
    io,
    path::{Path, PathBuf},
};

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, DocumentKey, ParserInterpretationKey, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct NullableTextRow {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

#[derive(QueryableByName)]
struct BytesRow {
    #[diesel(sql_type = diesel::sql_types::Binary)]
    value: Vec<u8>,
}

#[derive(QueryableByName)]
struct IntegerRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    value: i64,
}

fn setup() -> Result<(tempfile::TempDir, Database, SqliteConnection), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("catalog.sqlite");
    let database =
        Database::open(&Utf8PathBuf::from_path_buf(path.clone()).map_err(|_| "non-UTF8 db path")?)?;
    let connection = SqliteConnection::establish(path.to_str().ok_or("non-UTF8 db path")?)?;
    Ok((directory, database, connection))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/catalog/logiqx")
        .join(name)
}

fn request(
    path: PathBuf,
    source: &str,
    catalog: &str,
    name: &str,
) -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let document_path = Utf8PathBuf::from_path_buf(path).map_err(|path| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("non-UTF-8 document path: {}", path.display()),
        )
    })?;
    Ok(CatalogImportRequest {
        document_path,
        source_key: PublishingSourceKey::new(source),
        source_display_name: format!("Publisher {source}"),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: name.to_owned(),
        scope: CatalogScope::Unknown,
    })
}

fn count(connection: &mut SqliteConnection, table: &str) -> Result<i64, diesel::result::Error> {
    sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
        .get_result::<CountRow>(connection)
        .map(|row| row.count)
}

#[test]
fn imports_overlapping_catalogs_and_reimports_idempotently()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let catalog_a = request(
        fixture("catalog-a-v1.dat"),
        "publisher-a",
        "catalog-a",
        "Catalog A",
    )?;
    let first = app::import_catalog(&database, &catalog_a)?;
    let repeated = app::import_catalog(&database, &catalog_a)?;
    assert_eq!(first.snapshot_key, repeated.snapshot_key);

    let catalog_b = request(
        fixture("catalog-b.dat"),
        "publisher-b",
        "catalog-b",
        "Catalog B",
    )?;
    let overlap = app::import_catalog(&database, &catalog_b)?;
    assert_ne!(first.snapshot_key, overlap.snapshot_key);
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 2);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 2);
    assert_eq!(count(&mut connection, "asset_requirements")?, 4);
    assert_eq!(count(&mut connection, "import_runs")?, 3);
    assert_eq!(count(&mut connection, "snapshot_publications")?, 2);

    let unknown_attribute = sql_query(
        "SELECT field_name AS value FROM snapshot_extensions WHERE field_name = 'future-policy'",
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(unknown_attribute.value, "future-policy");
    let line = sql_query(
        "SELECT source_line AS value FROM snapshot_sets WHERE set_name = 'alpha' LIMIT 1",
    )
    .get_result::<IntegerRow>(&mut connection)?;
    assert!(line.value > 0);

    let location = sql_query(
        "SELECT source_column AS value FROM snapshot_sets WHERE set_name = 'alpha' LIMIT 1",
    )
    .get_result::<IntegerRow>(&mut connection)?;
    assert!(location.value > 0);

    let sparse_path = directory.path().join("sparse.dat");
    std::fs::write(
        &sparse_path,
        br#"<datafile xmlns:vendor="urn:vendor"><header><name>Sparse</name></header><game name="sparse"><rom name="unknown.bin" vendor:status="curated"/></game></datafile>"#,
    )?;
    let sparse = app::import_catalog(
        &database,
        &request(sparse_path, "publisher-sparse", "catalog-sparse", "Sparse")?,
    )?;
    assert_eq!(sparse.status.as_str(), "succeeded");
    let absent_size = sql_query(
        "SELECT CAST(size AS TEXT) AS value FROM asset_requirements WHERE asset_name = 'unknown.bin'",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert!(absent_size.value.is_none());
    let namespace = sql_query(
        "SELECT namespace_uri AS value FROM snapshot_extensions WHERE field_name = 'status'",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(namespace.value.as_deref(), Some("urn:vendor"));
    Ok(())
}

#[test]
fn publishing_completes_a_matching_identity_only_snapshot() -> Result<(), Box<dyn std::error::Error>>
{
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("identity-only.dat");
    let bytes = br#"<datafile><header><name>Legacy</name></header><game name="set"><rom name="asset.bin" crc="12345678"/></game></datafile>"#;
    std::fs::write(&path, bytes)?;
    let request = request(path, "legacy-publisher", "legacy-catalog", "Legacy")?;
    let document_key = DocumentKey::from_bytes(bytes).to_string();
    let interpretation = ParserInterpretationKey::logiqx_v1(&request.scope);
    sql_query("INSERT INTO publishing_sources (source_key, display_name) VALUES (?, ?)")
        .bind::<Text, _>(request.source_key.as_str())
        .bind::<Text, _>(&request.source_display_name)
        .execute(&mut connection)?;
    sql_query("INSERT INTO catalogs (catalog_key, source_key, display_name) VALUES (?, ?, ?)")
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(request.source_key.as_str())
        .bind::<Text, _>(&request.catalog_display_name)
        .execute(&mut connection)?;
    sql_query("INSERT INTO documents (document_key) VALUES (?)")
        .bind::<Text, _>(&document_key)
        .execute(&mut connection)?;
    sql_query(
        "INSERT INTO acquisitions (acquisition_key, source_key, document_key) \
         VALUES ('legacy-acquisition', ?, ?)",
    )
    .bind::<Text, _>(request.source_key.as_str())
    .bind::<Text, _>(&document_key)
    .execute(&mut connection)?;
    sql_query(
        "INSERT INTO parser_interpretations (interpretation_key, format) VALUES (?, 'logiqx')",
    )
    .bind::<Text, _>(interpretation.as_str())
    .execute(&mut connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots \
         (snapshot_key, catalog_key, document_key, interpretation_key, acquisition_key) \
         VALUES ('identity-only-snapshot', ?, ?, ?, 'legacy-acquisition')",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(&document_key)
    .bind::<Text, _>(interpretation.as_str())
    .execute(&mut connection)?;

    let report = app::import_catalog(&database, &request)?;
    assert_eq!(
        report
            .snapshot_key
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("identity-only-snapshot")
    );
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 1);
    assert_eq!(count(&mut connection, "asset_requirements")?, 1);
    assert_eq!(count(&mut connection, "snapshot_publications")?, 1);
    Ok(())
}

#[test]
fn stale_identity_only_metadata_is_not_published_as_current()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("stale-identity.dat");
    let bytes = br#"<datafile><header><name>Legacy</name><version>2.0</version></header><game name="set"/></datafile>"#;
    std::fs::write(&path, bytes)?;
    let request = request(path, "legacy-publisher", "legacy-catalog", "Legacy")?;
    let document_key = DocumentKey::from_bytes(bytes).to_string();
    let interpretation = ParserInterpretationKey::logiqx_v1(&request.scope);
    sql_query("INSERT INTO publishing_sources (source_key, display_name) VALUES (?, ?)")
        .bind::<Text, _>(request.source_key.as_str())
        .bind::<Text, _>(&request.source_display_name)
        .execute(&mut connection)?;
    sql_query("INSERT INTO catalogs (catalog_key, source_key, display_name) VALUES (?, ?, ?)")
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(request.source_key.as_str())
        .bind::<Text, _>(&request.catalog_display_name)
        .execute(&mut connection)?;
    sql_query("INSERT INTO documents (document_key) VALUES (?)")
        .bind::<Text, _>(&document_key)
        .execute(&mut connection)?;
    sql_query(
        "INSERT INTO parser_interpretations (interpretation_key, format) VALUES (?, 'logiqx')",
    )
    .bind::<Text, _>(interpretation.as_str())
    .execute(&mut connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots \
         (snapshot_key, catalog_key, document_key, interpretation_key, declared_version, scope_kind) \
         VALUES ('stale-identity-snapshot', ?, ?, ?, '1.0', 'complete')",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(&document_key)
    .bind::<Text, _>(interpretation.as_str())
    .execute(&mut connection)?;

    let report = app::import_catalog(&database, &request)?;
    let published_key = report
        .snapshot_key
        .as_ref()
        .map(ToString::to_string)
        .ok_or_else(|| io::Error::other("valid import publishes a snapshot"))?;
    assert_ne!(published_key, "stale-identity-snapshot");
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 2);
    assert_eq!(count(&mut connection, "snapshot_publications")?, 1);
    let version =
        sql_query("SELECT declared_version AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(published_key)
            .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(version.value.as_deref(), Some("2.0"));
    Ok(())
}

#[test]
fn changed_document_publishes_a_new_snapshot_and_preserves_the_previous_one()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let first = app::import_catalog(
        &database,
        &request(
            fixture("catalog-a-v1.dat"),
            "publisher-a",
            "catalog-a",
            "Catalog A",
        )?,
    )?;
    let second = app::import_catalog(
        &database,
        &request(
            fixture("catalog-a-v2.dat"),
            "publisher-a",
            "catalog-a",
            "Catalog A",
        )?,
    )?;
    assert_ne!(first.snapshot_key, second.snapshot_key);
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 2);
    let versions =
        sql_query("SELECT COUNT(*) AS count FROM snapshot_sets WHERE set_name = 'alpha'")
            .get_result::<CountRow>(&mut connection)?;
    assert_eq!(versions.count, 2);
    let expected_values = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirements \
         WHERE asset_name = 'disputed.bin' AND crc IN (X'11111111', X'22222222')",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(expected_values.count, 2);
    Ok(())
}

#[test]
fn permuting_set_records_does_not_change_source_attributed_requirements()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let original_path = directory.path().join("original-order.dat");
    let permuted_path = directory.path().join("permuted-order.dat");
    let first = br#"<datafile><header><name>Order Test</name></header><game name="one"><rom name="one.rom" crc="11111111"/></game><game name="two"><rom name="two.rom" crc="22222222"/></game></datafile>"#;
    let second = br#"<datafile><header><name>Order Test</name></header><game name="two"><rom name="two.rom" crc="22222222"/></game><game name="one"><rom name="one.rom" crc="11111111"/></game></datafile>"#;
    std::fs::write(&original_path, first)?;
    std::fs::write(&permuted_path, second)?;

    let original = app::import_catalog(
        &database,
        &request(
            original_path,
            "publisher-order",
            "catalog-order",
            "Order Test",
        )?,
    )?;
    let permuted = app::import_catalog(
        &database,
        &request(
            permuted_path,
            "publisher-order",
            "catalog-order",
            "Order Test",
        )?,
    )?;
    assert_ne!(original.snapshot_key, permuted.snapshot_key);

    let preserved_claims = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirements \
         WHERE (set_name = 'one' AND asset_name = 'one.rom' AND crc = X'11111111') \
            OR (set_name = 'two' AND asset_name = 'two.rom' AND crc = X'22222222')",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(preserved_claims.count, 4);
    Ok(())
}

#[test]
fn malformed_record_creates_failed_run_without_hiding_prior_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let valid = app::import_catalog(
        &database,
        &request(
            fixture("catalog-a-v1.dat"),
            "publisher-a",
            "catalog-a",
            "Catalog A",
        )?,
    )?;
    let malformed_path = directory.path().join("malformed-record.dat");
    std::fs::write(
        &malformed_path,
        br#"<datafile><header><name>Catalog A</name></header><game name="broken"><rom name="bad.bin" size="8" sha1="not-hex"/></game></datafile>"#,
    )?;
    let failed = app::import_catalog(
        &database,
        &request(malformed_path, "publisher-a", "catalog-a", "Catalog A")?,
    )?;
    assert_eq!(failed.status.as_str(), "failed");
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 1);
    let visible = sql_query("SELECT snapshot_key AS value FROM catalog_snapshots")
        .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        visible.value,
        valid
            .snapshot_key
            .ok_or_else(|| io::Error::other("successful import publishes a snapshot"))?
            .to_string()
    );
    let diagnostic =
        sql_query("SELECT diagnostic AS value FROM import_runs WHERE status = 'failed'")
            .get_result::<NullableTextRow>(&mut connection)?;
    assert!(diagnostic.value.is_some());
    let retained_payload = sql_query(
        "SELECT payload AS value FROM documents \
         WHERE document_key = (SELECT document_key FROM import_runs WHERE status = 'failed')",
    )
    .get_result::<BytesRow>(&mut connection)?;
    assert!(String::from_utf8_lossy(&retained_payload.value).contains("not-hex"));
    Ok(())
}
