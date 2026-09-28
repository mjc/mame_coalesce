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
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogRecordKind, CatalogRecordRef, CatalogScope, DocumentKey,
        ExternalRecordRef, ParserInterpretationKey, PublishingSourceKey, RelationshipClaim,
        RelationshipEndpoint, RelationshipOrigin, RelationshipType, SnapshotKey,
        SnapshotRecordStatus,
    },
};

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct QueryableAssertion {
    #[diesel(sql_type = Text)]
    relation_type: String,
    #[diesel(sql_type = Text)]
    origin: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_snapshot_key: Option<String>,
    #[diesel(sql_type = Text)]
    subject_key: String,
    #[diesel(sql_type = Text)]
    target_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rule_version: Option<String>,
}

#[derive(QueryableByName)]
struct RelationshipKeyLocationRow {
    #[diesel(sql_type = Text)]
    subject_key: String,
    #[diesel(sql_type = Text)]
    target_key: String,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
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
struct AssetHashesRow {
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    sha1: Option<Vec<u8>>,
}

#[derive(QueryableByName)]
struct IntegerRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct NullableIntegerRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    value: Option<i64>,
}

#[derive(QueryableByName)]
struct DiagnosticLocationRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
}

#[derive(QueryableByName)]
struct SoftwareItemRow {
    #[diesel(sql_type = Text)]
    supported: String,
    #[diesel(sql_type = Text)]
    info: String,
    #[diesel(sql_type = Text)]
    shared_features: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    source_line: i64,
}

#[derive(QueryableByName)]
struct SoftwareAreaRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    declared_size: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    width: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    endianness: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
}

#[derive(QueryableByName)]
struct SoftwareComponentRow {
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    load_instruction: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    writeable: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
}

#[derive(QueryableByName)]
struct FailedLocationRow {
    #[diesel(sql_type = Nullable<Text>)]
    record_kind: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    record_name: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
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
        format: CatalogDocumentFormat::Logiqx,
        source_key: PublishingSourceKey::new(source),
        source_display_name: format!("Publisher {source}"),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: name.to_owned(),
        scope: CatalogScope::Unknown,
    })
}

fn mame_request() -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let mut request = request(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/machine.xml"),
        "mame",
        "mame-machine-fixture",
        "Synthetic MAME machine catalog",
    )?;
    request.format = CatalogDocumentFormat::MameListXml;
    Ok(request)
}

fn mame_softwarelist_request() -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let mut request = request(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/software-list.xml"),
        "mame-softwarelists",
        "mame-softwarelists-fixture",
        "Synthetic MAME software lists",
    )?;
    request.format = CatalogDocumentFormat::MameSoftwareListXml;
    Ok(request)
}

fn clrmamepro_request() -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let mut request = request(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/clrmamepro/sample.dat"),
        "clrmamepro",
        "clrmamepro-fixture",
        "Synthetic ClrMamePro catalog",
    )?;
    request.format = CatalogDocumentFormat::ClrMamePro;
    Ok(request)
}

fn no_intro_request() -> Result<CatalogImportRequest, Box<dyn std::error::Error>> {
    let mut request = request(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/catalog/no-intro/pc-xml-synthetic.xml"),
        "no-intro",
        "no-intro-pc-xml-fixture",
        "Synthetic No-Intro P/C XML",
    )?;
    request.format = CatalogDocumentFormat::NoIntroPcXml;
    request.scope = CatalogScope::Filtered(serde_json::json!({"fixture": "synthetic"}));
    Ok(request)
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
fn imports_no_intro_pc_xml_metadata_without_inventing_title_relationships()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let request = no_intro_request()?;
    let report = app::import_catalog(&database, &request)?;
    let repeated = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert_eq!(report.snapshot_key, repeated.snapshot_key);
    let snapshot = report.snapshot_key.ok_or("No-Intro snapshot missing")?;

    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 2);
    assert_eq!(count(&mut connection, "asset_requirements")?, 2);
    let hashed_requirements = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirements \
         WHERE snapshot_key = ? AND size = 4 AND crc IS NOT NULL AND sha1 IS NOT NULL",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(hashed_requirements.count, 1);
    let hashes = sql_query(
        "SELECT crc, sha1 FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'synthetic-cartridge.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<AssetHashesRow>(&mut connection)?;
    assert_eq!(hashes.crc, Some(vec![0x12, 0x34, 0x56, 0x78]));
    assert_eq!(
        hashes.sha1,
        Some(vec![
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
        ])
    );

    let snapshot_scope =
        sql_query("SELECT scope_json AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&snapshot_scope.value)?,
        serde_json::json!({"fixture": "synthetic"})
    );
    let declared_version =
        sql_query("SELECT declared_version AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(&mut connection)?;
    assert_eq!(declared_version.value, "synthetic-2026-09-24");

    let source_lineage =
        sql_query("SELECT source_key AS value FROM catalogs WHERE catalog_key = ?")
            .bind::<Text, _>("no-intro-pc-xml-fixture")
            .get_result::<TextRow>(&mut connection)?;
    assert_eq!(source_lineage.value, "no-intro");

    let metadata = sql_query(
        "SELECT metadata_json AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata.value)?;
    assert_eq!(metadata["name_alt"], "合成カートリッジ (日本)");
    assert_eq!(metadata["region"], "Japan");
    assert_eq!(metadata["languages"], serde_json::json!(["Ja"]));
    assert_eq!(metadata["version"], "1.0");
    assert_eq!(metadata["bios"], serde_json::Value::Null);
    assert_eq!(metadata["clone"], "1041");
    assert_eq!(metadata["mergeof"], "1042");
    let source_line = sql_query(
        "SELECT source_line AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(&mut connection)?;
    assert!(source_line.value > 0);

    let parent_metadata = sql_query(
        "SELECT metadata_json AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (World)'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    let parent_metadata: serde_json::Value = serde_json::from_str(&parent_metadata.value)?;
    assert_eq!(parent_metadata["clone"], "P");
    assert_eq!(parent_metadata["bios"], "0");

    assert_no_intro_source_assertions(
        &mut connection,
        snapshot.as_str(),
        &report.run_key.to_string(),
    )?;

    let retained_unknown = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE snapshot_key = ? AND field_name = 'future-field'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(retained_unknown.value, "\"retained\"");

    assert_no_intro_retained_document(
        &mut connection,
        snapshot.as_str(),
        request.document_path.as_std_path(),
    )?;

    Ok(())
}

fn assert_no_intro_retained_document(
    connection: &mut SqliteConnection,
    snapshot: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let document = sql_query(
        "SELECT payload AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<BytesRow>(connection)?;
    assert_eq!(document.value, std::fs::read(path)?);
    let format_hint = sql_query(
        "SELECT documents.format_hint AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<NullableTextRow>(connection)?;
    assert_eq!(format_hint.value.as_deref(), Some("no-intro-pc-xml"));
    Ok(())
}

fn assert_no_intro_source_assertions(
    connection: &mut SqliteConnection,
    snapshot: &str,
    run_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let relationships = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_sets \
         WHERE snapshot_key = ? AND parent_name IS NOT NULL",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<CountRow>(connection)?;
    assert_eq!(relationships.count, 0);
    let merge_links = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'synthetic-cartridge-jp.bin' \
         AND merge_name IS NOT NULL",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<CountRow>(connection)?;
    assert_eq!(merge_links.count, 0);
    let raw_reference = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE snapshot_key = ? AND field_name = 'clone' AND record_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<TextRow>(connection)?;
    assert_eq!(raw_reference.value, "\"1041\"");
    let diagnostics = sql_query(
        "SELECT COUNT(*) AS count FROM import_diagnostics \
         WHERE run_key = ? AND field_name = 'clone' AND code = 'unsupported_attribute'",
    )
    .bind::<Text, _>(run_key)
    .get_result::<CountRow>(connection)?;
    assert_eq!(diagnostics.count, 2);
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
fn malformed_no_intro_xml_does_not_publish_a_snapshot() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let malformed_path = directory.path().join("malformed.xml");
    std::fs::write(&malformed_path, b"<datafile><game name=\"incomplete\">")?;
    let mut request = no_intro_request()?;
    request.document_path = Utf8PathBuf::from_path_buf(malformed_path)
        .map_err(|_| "non-UTF8 malformed fixture path")?;

    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    assert_eq!(count(&mut connection, "import_diagnostics")?, 1);
    let location = sql_query("SELECT source_line, source_column FROM import_diagnostics")
        .get_result::<DiagnosticLocationRow>(&mut connection)?;
    assert!(location.source_line.is_some_and(|line| line > 0));
    assert!(location.source_column.is_some_and(|column| column > 0));
    Ok(())
}

#[test]
fn structured_no_intro_header_fails_with_location_and_no_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    for (file_name, xml) in [
        (
            "attributed-header.xml",
            br#"<datafile><header><version source="export">v1</version></header><game name="set"/></datafile>"#.as_slice(),
        ),
        (
            "nested-header.xml",
            br#"<datafile><header><description><revision>v2</revision></description></header><game name="set"/></datafile>"#.as_slice(),
        ),
    ] {
        let path = directory.path().join(file_name);
        std::fs::write(&path, xml)?;
        let mut request = no_intro_request()?;
        request.document_path = Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 fixture path")?;
        let report = app::import_catalog(&database, &request)?;
        assert_eq!(report.status, app::CatalogImportStatus::Failed);
        assert!(report.snapshot_key.is_none());
    }
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    let located = sql_query(
        "SELECT COUNT(*) AS count FROM import_diagnostics \
         WHERE record_kind = 'header' AND record_name IS NOT NULL \
         AND source_line IS NOT NULL AND source_column IS NOT NULL",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(located.count, 2);
    Ok(())
}

#[test]
fn no_intro_unknown_rom_fields_keep_the_rom_record_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("rom-extensions.xml");
    std::fs::write(
        &path,
        br#"<datafile><game name="set"><rom name="a.bin" future="A"><future-child value="a"/></rom><rom name="b.bin" future="B"/></game></datafile>"#,
    )?;
    let mut request = no_intro_request()?;
    request.document_path =
        Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 fixture path")?;
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let first = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE record_kind = 'rom' AND record_name = 'a.bin' AND field_name = 'future'",
    )
    .get_result::<TextRow>(&mut connection)?;
    let second = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE record_kind = 'rom' AND record_name = 'b.bin' AND field_name = 'future'",
    )
    .get_result::<TextRow>(&mut connection)?;
    let nested = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions \
         WHERE record_kind = 'rom' AND record_name = 'a.bin' AND field_name = 'element:future-child'",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(first.value, "\"A\"");
    assert_eq!(second.value, "\"B\"");
    assert_eq!(nested.count, 1);
    Ok(())
}

#[test]
fn no_intro_record_text_fails_instead_of_disappearing() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    for (file_name, xml, kind) in [
        (
            "root-text.xml",
            br#"<datafile>unexpected<game name="set"/></datafile>"#.as_slice(),
            "document",
        ),
        (
            "header-text.xml",
            br#"<datafile><header>unexpected</header><game name="set"/></datafile>"#.as_slice(),
            "header",
        ),
        (
            "game-text.xml",
            br#"<datafile><game name="set">unexpected</game></datafile>"#.as_slice(),
            "game",
        ),
        (
            "rom-text.xml",
            br#"<datafile><game name="set"><rom name="a.bin">unexpected</rom></game></datafile>"#
                .as_slice(),
            "rom",
        ),
    ] {
        let path = directory.path().join(file_name);
        std::fs::write(&path, xml)?;
        let mut request = no_intro_request()?;
        request.document_path =
            Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 fixture path")?;
        let report = app::import_catalog(&database, &request)?;
        assert_eq!(report.status, app::CatalogImportStatus::Failed);
        assert!(report.snapshot_key.is_none());
        let located = sql_query(
            "SELECT COUNT(*) AS count FROM import_diagnostics \
             WHERE run_key = ? AND record_kind = ? \
             AND source_line IS NOT NULL AND source_column IS NOT NULL",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .bind::<Text, _>(kind)
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(located.count, 1, "missing located diagnostic for {kind}");
    }
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    Ok(())
}

#[test]
fn malformed_no_intro_source_records_are_located_and_never_published()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let invalid_reference_path = directory.path().join("invalid-reference.xml");
    std::fs::write(
        &invalid_reference_path,
        b"<datafile>\n  <game name=\"bad-reference\" clone=\"not-an-id\"/>\n</datafile>",
    )?;
    let mut invalid_reference = no_intro_request()?;
    invalid_reference.document_path = Utf8PathBuf::from_path_buf(invalid_reference_path)
        .map_err(|_| "non-UTF8 invalid-reference path")?;
    let report = app::import_catalog(&database, &invalid_reference)?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());

    let duplicate_path = directory.path().join("duplicate-archives.xml");
    std::fs::write(
        &duplicate_path,
        b"<datafile>\n  <game name=\"same-archive\"/>\n  <game name=\"same-archive\"/>\n</datafile>",
    )?;
    let mut duplicate = no_intro_request()?;
    duplicate.document_path = Utf8PathBuf::from_path_buf(duplicate_path)
        .map_err(|_| "non-UTF8 duplicate-archive path")?;
    let report = app::import_catalog(&database, &duplicate)?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    assert_eq!(count(&mut connection, "import_diagnostics")?, 2);

    let located = sql_query(
        "SELECT COUNT(*) AS count FROM import_diagnostics \
         WHERE source_line IS NOT NULL AND source_column IS NOT NULL",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(located.count, 2);
    Ok(())
}

#[test]
fn imports_mame_machine_rom_disk_bios_and_device_semantics_loss_aware()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let report = app::import_catalog(&database, &mame_request()?)?;
    let repeated = app::import_catalog(&database, &mame_request()?)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;
    assert_eq!(Some(snapshot.clone()), repeated.snapshot_key);
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);

    let machine = sql_query("SELECT metadata_json AS value FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'demo_machine'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert!(machine.value.contains("Synthetic Machine"));
    assert!(machine.value.contains("demo_bios"));
    assert!(machine.value.contains("demo_sound"));
    assert!(machine.value.contains("2000"));
    let device = sql_query("SELECT metadata_json AS value FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'demo_sound'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert!(device.value.contains("isdevice"));
    assert!(device.value.contains("yes"));

    let assets = sql_query("SELECT GROUP_CONCAT(role || ':' || asset_name, ',') AS value FROM (SELECT role, asset_name FROM asset_requirements WHERE snapshot_key = ? AND set_name = 'demo_machine' ORDER BY component_order)")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        assets.value,
        "rom:demo_bios.bin,rom:demo_video.bin,disk:demo_disk"
    );
    let region = sql_query("SELECT metadata_json AS value FROM asset_requirements WHERE snapshot_key = ? AND asset_name = 'demo_bios.bin'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert!(region.value.contains("maincpu"));
    assert!(region.value.contains("demo_bios"));
    let disk_scope = sql_query("SELECT evidence_scope AS value FROM asset_requirements WHERE snapshot_key = ? AND asset_name = 'demo_disk'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert_eq!(disk_scope.value, "chd_header_sha1");
    let disk_identity = sql_query("SELECT sha1 AS value FROM asset_requirements WHERE snapshot_key = ? AND asset_name = 'demo_disk'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<BytesRow>(&mut connection)?;
    assert_eq!(
        disk_identity.value,
        hex::decode("1123456789abcdef0123456789abcdef01234567")?
    );
    let parent_disk = sql_query("SELECT merge_name AS value FROM asset_requirements WHERE snapshot_key = ? AND asset_name = 'demo_disk'")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert_eq!(parent_disk.value, "parent_disk");
    let version =
        sql_query("SELECT declared_version AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(&mut connection)?;
    assert_eq!(version.value, "0.216-synthetic");
    assert!(count(&mut connection, "snapshot_extensions")? >= 2);
    assert!(report.diagnostic_count >= 2);
    let namespace = sql_query(
        "SELECT namespace_uri AS value FROM snapshot_extensions WHERE field_name = 'flag'",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(namespace.value.as_deref(), Some("urn:mame:future"));
    let feature = sql_query("SELECT raw_value_json AS value FROM snapshot_extensions WHERE field_name = 'element:feature'")
        .get_result::<TextRow>(&mut connection)?;
    assert!(feature.value.contains("protection"));
    assert!(sql_query("SELECT COUNT(*) AS count FROM import_diagnostics WHERE run_key = ? AND code = 'unsupported_element'")
        .bind::<Text, _>(report.run_key.to_string())
        .get_result::<CountRow>(&mut connection)?.count > 0);
    Ok(())
}

#[test]
fn imports_mame_relationship_asset_fields_extensions_and_format_hint()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let mut request = request(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/semantics.xml"),
        "mame-semantics",
        "mame-semantics",
        "MAME semantics",
    )?;
    request.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &request)?;
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;

    let parent = sql_query(
        "SELECT parent_name AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'clone'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(parent.value.as_deref(), Some("parent"));

    let rom_fields = sql_query(
        "SELECT merge_name || ':' || dump_status AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'clone.rom'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(rom_fields.value, "parent.rom:baddump");
    let disk_fields = sql_query(
        "SELECT merge_name || ':' || dump_status AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'clone.disk'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(disk_fields.value, "parent.disk:nodump");

    let asset_extension_count = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions \
         WHERE snapshot_key = ? AND record_kind = 'rom' AND field_name = 'flag'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?
    .count;
    assert_eq!(asset_extension_count, 1);
    let asset_metadata = sql_query(
        "SELECT metadata_json AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'clone.rom'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert!(!asset_metadata.value.contains("future:flag"));

    let format_hint = sql_query(
        "SELECT documents.format_hint AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(format_hint.value.as_deref(), Some("mame-listxml"));
    Ok(())
}

#[test]
fn mame_relationships_resolve_component_keys_and_keep_device_locations()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("mame-relationship-identity.xml");
    std::fs::write(
        &path,
        "<mame>\n<machine name=\"clone-parent\"/>\n<machine name=\"rom-parent\">\n<rom name=\"shared.bin\" size=\"1\" crc=\"12345678\"/>\n</machine>\n<machine name=\"clone\" cloneof=\"clone-parent\" romof=\"rom-parent\">\n<device_ref name=\"sound\"/>\n<rom name=\"shared.bin\" merge=\"shared.bin\" size=\"1\" crc=\"12345678\"/>\n</machine>\n<machine name=\"sound\"/>\n</mame>",
    )?;
    let mut request = request(
        path,
        "publisher-mame-relations",
        "mame-relations",
        "MAME relations",
    )?;
    request.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &request)?;
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;

    let merge = sql_query(
        "SELECT subject_key, target_key, source_line FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'merge'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RelationshipKeyLocationRow>(&mut connection)?;
    assert_eq!(merge.subject_key, "[\"clone\",\"shared.bin\",0]");
    assert_eq!(merge.target_key, "[\"rom-parent\",\"shared.bin\",0]");
    assert_eq!(merge.source_line, 8);

    let device = sql_query(
        "SELECT subject_key, target_key, source_line FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'device_ref'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RelationshipKeyLocationRow>(&mut connection)?;
    assert_eq!(device.subject_key, "clone");
    assert_eq!(device.target_key, "sound");
    assert_eq!(device.source_line, 7);

    let dependencies = sql_query(
        "SELECT source_field || ':' || target_key AS value FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND subject_kind = 'catalog_set' AND subject_key = 'clone' \
         AND relation_type = 'runtime_dependency' ORDER BY source_field",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(
        dependencies
            .iter()
            .map(|dependency| dependency.value.as_str())
            .collect::<Vec<_>>(),
        ["device_ref:sound", "romof:rom-parent"]
    );
    Ok(())
}

#[test]
fn software_item_relationship_keys_do_not_collide_on_slashes()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("software-list-slash-identities.xml");
    std::fs::write(
        &path,
        "<softwarelists><softwarelist name=\"a/b\"><software name=\"c\" cloneof=\"parent\"><description>c</description><year>2000</year><publisher>example</publisher><part name=\"cart\" interface=\"cart\"/></software><software name=\"parent\"><description>parent</description><year>2000</year><publisher>example</publisher><part name=\"cart\" interface=\"cart\"/></software></softwarelist><softwarelist name=\"a\"><software name=\"b/c\" cloneof=\"parent\"><description>b/c</description><year>2000</year><publisher>example</publisher><part name=\"cart\" interface=\"cart\"/></software><software name=\"parent\"><description>parent</description><year>2000</year><publisher>example</publisher><part name=\"cart\" interface=\"cart\"/></software></softwarelist></softwarelists>",
    )?;
    let mut request = request(path, "publisher-slash-keys", "slash-keys", "Slash keys")?;
    request.format = CatalogDocumentFormat::MameSoftwareListXml;
    let report = app::import_catalog(&database, &request)?;
    let Some(snapshot) = report.snapshot_key.as_ref() else {
        let diagnostic =
            sql_query("SELECT message AS value FROM import_diagnostics WHERE run_key = ?")
                .bind::<Text, _>(report.run_key.to_string())
                .get_result::<TextRow>(&mut connection)?;
        return Err(io::Error::other(diagnostic.value).into());
    };
    let keys = sql_query(
        "SELECT subject_key AS value FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'cloneof' ORDER BY subject_key",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].value, "[\"a\",\"b/c\"]");
    assert_eq!(keys[1].value, "[\"a/b\",\"c\"]");
    Ok(())
}

#[test]
fn imports_mame_softwarelists_with_nested_order_dependencies_and_load_claims()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let request = mame_softwarelist_request()?;
    let report = app::import_catalog(&database, &request)?;
    let repeated = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("software-list snapshot missing")?;
    assert_eq!(Some(snapshot.clone()), repeated.snapshot_key);
    let format_hint = sql_query(
        "SELECT documents.format_hint AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(format_hint.value.as_deref(), Some("mame-softwarelist-xml"));
    let version =
        sql_query("SELECT declared_version AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(version.value.as_deref(), Some("0.289-synthetic"));

    assert_eq!(count(&mut connection, "software_lists")?, 2);
    assert_eq!(count(&mut connection, "software_items")?, 3);
    assert_eq!(count(&mut connection, "software_parts")?, 4);
    assert_eq!(count(&mut connection, "software_areas")?, 5);
    assert_eq!(count(&mut connection, "software_components")?, 6);
    assert_eq!(count(&mut connection, "software_item_dependencies")?, 1);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 0);
    assert_eq!(count(&mut connection, "asset_requirements")?, 0);

    assert_software_list_identity_and_dependencies(&mut connection, &snapshot)?;
    assert_software_list_nesting(&mut connection, &snapshot)?;
    assert_software_list_components(&mut connection, &snapshot)?;
    assert_software_list_extensions(&mut connection, &snapshot)?;
    assert_malformed_software_list_fails(directory.path(), &database, &mut connection, &request)?;
    Ok(())
}

#[test]
fn imports_metadata_only_software_without_parts() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("metadata-only-softwarelist.xml");
    std::fs::write(
        &path,
        br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher></software></softwarelist>"#,
    )?;
    let mut request = mame_softwarelist_request()?;
    request.document_path =
        Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 metadata fixture path")?;
    request.catalog_key = CatalogKey::new("metadata-only-softwarelist");
    request.catalog_display_name = "Metadata-only software list".into();
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert!(report.snapshot_key.is_some());
    assert_eq!(count(&mut connection, "software_lists")?, 1);
    assert_eq!(count(&mut connection, "software_items")?, 1);
    assert_eq!(count(&mut connection, "software_parts")?, 0);
    let supported = sql_query("SELECT supported AS value FROM software_items")
        .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(supported.value, None);
    Ok(())
}

#[test]
fn imports_repeated_area_names_without_merging_components_or_defaults()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("repeated-software-areas.xml");
    std::fs::write(
        &path,
        br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="first.bin"/></dataarea><dataarea name="rom" size="2"><rom name="second.bin"/></dataarea><diskarea name="media"><disk name="implicit"/><disk name="explicit" writeable="no"/></diskarea></part></software></softwarelist>"#,
    )?;
    let mut request = mame_softwarelist_request()?;
    request.document_path =
        Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 fixture path")?;
    request.catalog_key = CatalogKey::new("repeated-software-areas");
    request.catalog_display_name = "Repeated software areas".into();
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert_eq!(count(&mut connection, "software_areas")?, 3);
    assert_eq!(count(&mut connection, "software_components")?, 4);
    let first =
        sql_query("SELECT component_name AS value FROM software_components WHERE area_order = 0")
            .get_result::<NullableTextRow>(&mut connection)?;
    let second =
        sql_query("SELECT component_name AS value FROM software_components WHERE area_order = 1")
            .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(first.value.as_deref(), Some("first.bin"));
    assert_eq!(second.value.as_deref(), Some("second.bin"));
    let absent = sql_query(
        "SELECT writeable AS value FROM software_components WHERE component_name = 'implicit'",
    )
    .get_result::<NullableIntegerRow>(&mut connection)?;
    let explicit = sql_query(
        "SELECT writeable AS value FROM software_components WHERE component_name = 'explicit'",
    )
    .get_result::<NullableIntegerRow>(&mut connection)?;
    assert_eq!(absent.value, None);
    assert_eq!(explicit.value, Some(0));
    Ok(())
}

#[test]
fn imports_mame_numeric_bases_and_empty_nodump_hashes() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("octal-software-nodump.xml");
    std::fs::write(
        &path,
        br#"<softwarelist name="one"><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="010"><rom name="missing.bin" size="010" offset="010" status="nodump" crc="" sha1=""/></dataarea></part></software></softwarelist>"#,
    )?;
    let mut request = mame_softwarelist_request()?;
    request.document_path =
        Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 fixture path")?;
    request.catalog_key = CatalogKey::new("octal-software-nodump");
    request.catalog_display_name = "Octal software sizes".into();
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let area_size = sql_query("SELECT declared_size AS value FROM software_areas")
        .get_result::<NullableIntegerRow>(&mut connection)?;
    let rom_size = sql_query("SELECT size AS value FROM software_components")
        .get_result::<NullableIntegerRow>(&mut connection)?;
    let offset = sql_query("SELECT offset AS value FROM software_components")
        .get_result::<NullableIntegerRow>(&mut connection)?;
    let no_hash =
        sql_query("SELECT crc IS NULL AND sha1 IS NULL AS value FROM software_components")
            .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(area_size.value, Some(8));
    assert_eq!(rom_size.value, Some(8));
    assert_eq!(offset.value, Some(8));
    assert_eq!(no_hash.value, 1);
    Ok(())
}

#[test]
fn imports_empty_software_list_export_as_an_empty_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("empty-softwarelists.xml");
    std::fs::write(&path, b"<softwarelists/>")?;
    let mut request = mame_softwarelist_request()?;
    request.document_path =
        Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 empty fixture path")?;
    request.catalog_key = CatalogKey::new("empty-softwarelists");
    request.catalog_display_name = "Empty software-list export".into();
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert!(report.snapshot_key.is_some());
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    assert_eq!(count(&mut connection, "software_lists")?, 0);
    assert_eq!(count(&mut connection, "software_items")?, 0);
    let repeated = app::import_catalog(&database, &request)?;
    assert_eq!(repeated.snapshot_key, report.snapshot_key);
    Ok(())
}

fn assert_software_list_identity_and_dependencies(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let list_order = sql_query(
        "SELECT GROUP_CONCAT(list_name, ',') AS value \
         FROM (SELECT list_name FROM software_lists WHERE snapshot_key = ? ORDER BY list_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(list_order.value, "demo_cart,demo_flop");

    let item_order = sql_query(
        "SELECT GROUP_CONCAT(item_name, ',') AS value FROM ( \
         SELECT item_name FROM software_items WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' ORDER BY item_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(item_order.value, "demo_game,demo_original");

    let scoped_duplicates = sql_query(
        "SELECT COUNT(*) AS count FROM software_items \
         WHERE snapshot_key = ? AND item_name = 'demo_game'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(connection)?;
    assert_eq!(scoped_duplicates.count, 2);

    let dependency = sql_query(
        "SELECT target_item_name AS value FROM software_item_dependencies \
         WHERE snapshot_key = ? AND list_name = 'demo_cart' AND item_name = 'demo_game' \
         AND dependency_kind = 'clone_of'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(dependency.value, "demo_original");
    Ok(())
}

fn assert_software_list_nesting(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let item = sql_query(
        "SELECT supported, info_json AS info, shared_features_json AS shared_features, source_line \
         FROM software_items WHERE snapshot_key = ? AND list_name = 'demo_cart' \
         AND item_name = 'demo_game'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareItemRow>(connection)?;
    assert_eq!(item.supported, "partial");
    assert_eq!(item.info.matches("language").count(), 2);
    let english = item
        .info
        .find("English")
        .ok_or("English metadata missing")?;
    let french = item.info.find("French").ok_or("French metadata missing")?;
    assert!(english < french);
    assert!(item.shared_features.contains("compatibility"));

    let parts = sql_query(
        "SELECT GROUP_CONCAT(part_name, ',') AS value FROM ( \
         SELECT part_name FROM software_parts WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' ORDER BY part_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(parts.value, "cart,manual");

    let part_interfaces = sql_query(
        "SELECT GROUP_CONCAT(interface, ',') AS value FROM ( \
         SELECT interface FROM software_parts WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' ORDER BY part_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(part_interfaces.value, "demo_cart,document");

    let area_order = sql_query(
        "SELECT GROUP_CONCAT(area_kind || ':' || area_name, ',') AS value FROM ( \
         SELECT area_kind, area_name FROM software_areas WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' AND part_name = 'cart' \
         ORDER BY area_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(area_order.value, "data:program,disk:media");

    let area = sql_query(
        "SELECT declared_size, width, endianness, source_line FROM software_areas \
         WHERE snapshot_key = ? AND list_name = 'demo_cart' AND item_name = 'demo_game' \
         AND part_name = 'cart' AND area_name = 'program' AND area_kind = 'data'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareAreaRow>(connection)?;
    assert_eq!(area.declared_size, Some(32));
    assert_eq!(area.width, Some(16));
    assert_eq!(area.endianness.as_deref(), Some("big"));
    assert!(item.source_line < area.source_line);

    let sparse_area = sql_query(
        "SELECT declared_size, width, endianness, source_line FROM software_areas \
         WHERE snapshot_key = ? AND list_name = 'demo_cart' AND item_name = 'demo_game' \
         AND part_name = 'manual' AND area_name = 'text' AND area_kind = 'data'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareAreaRow>(connection)?;
    assert_eq!(sparse_area.declared_size, Some(16));
    assert_eq!(sparse_area.width, None);
    assert_eq!(sparse_area.endianness, None);
    Ok(())
}

fn assert_software_list_components(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let components = sql_query(
        "SELECT GROUP_CONCAT(component_name, ',') AS value \
         FROM (SELECT component_name FROM software_components WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' AND part_name = 'cart' \
         AND area_name = 'program' AND area_kind = 'data' ORDER BY component_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(components.value, "program.bin,missing.bin");

    let component_roles = sql_query(
        "SELECT GROUP_CONCAT(component_kind || ':' || component_name, ',') AS value \
         FROM (SELECT component_kind, component_name FROM software_components \
         WHERE snapshot_key = ? AND list_name = 'demo_cart' AND item_name = 'demo_game' \
         AND part_name = 'cart' AND area_kind = 'data' AND area_name = 'program' \
         ORDER BY component_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(component_roles.value, "rom:program.bin,rom:missing.bin");

    let load_claim = sql_query(
        "SELECT load_instruction AS value FROM software_components WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' AND component_name = 'program.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(load_claim.value, "load16_word_swap");

    let no_dump = sql_query(
        "SELECT dump_status, sha1, load_instruction, writeable, source_line \
         FROM software_components \
         WHERE snapshot_key = ? AND component_name = 'missing.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareComponentRow>(connection)?;
    assert_eq!(no_dump.dump_status.as_deref(), Some("nodump"));
    assert!(no_dump.sha1.is_none());
    assert_eq!(no_dump.load_instruction.as_deref(), Some("continue"));
    let parent_area_line = sql_query(
        "SELECT source_line AS value FROM software_areas WHERE snapshot_key = ? \
         AND list_name = 'demo_cart' AND item_name = 'demo_game' AND part_name = 'cart' \
         AND area_name = 'program' AND area_kind = 'data'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(connection)?;
    assert!(parent_area_line.value < no_dump.source_line);

    let disk = sql_query(
        "SELECT dump_status, sha1, load_instruction, writeable, source_line \
         FROM software_components WHERE snapshot_key = ? AND component_name = 'demo-disk'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareComponentRow>(connection)?;
    assert!(disk.dump_status.is_none());
    assert!(disk.sha1.is_some());
    assert_eq!(disk.writeable, Some(1));
    let disk_scope = sql_query(
        "SELECT evidence_scope AS value FROM software_components \
         WHERE snapshot_key = ? AND component_name = 'demo-disk'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(disk_scope.value, "chd_header_sha1");

    let absent_status = sql_query(
        "SELECT dump_status, NULL AS sha1, NULL AS load_instruction, writeable, source_line \
         FROM software_components WHERE snapshot_key = ? \
         AND component_name = 'original.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareComponentRow>(connection)?;
    assert!(absent_status.dump_status.is_none());
    Ok(())
}

fn assert_software_list_extensions(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let unknown_element = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions WHERE snapshot_key = ? \
         AND field_name = 'element:future-policy'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert!(unknown_element.value.contains("vendor payload"));

    let unknown_attribute = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions WHERE snapshot_key = ? \
         AND field_name = '@future-flag'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert!(unknown_attribute.value.contains("retained"));
    Ok(())
}

fn assert_malformed_software_list_fails(
    temp_dir: &Path,
    database: &Database,
    connection: &mut SqliteConnection,
    request: &CatalogImportRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let malformed_path = temp_dir.join("malformed-softwarelist.xml");
    std::fs::write(
        &malformed_path,
        b"<softwarelist name=\"broken\"><software name=\"game\"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name=\"cart\"/></software></softwarelist>",
    )?;
    let mut malformed_request = request.clone();
    malformed_request.document_path = Utf8PathBuf::from_path_buf(malformed_path)
        .map_err(|_| "non-UTF8 malformed fixture path")?;
    malformed_request.catalog_key = CatalogKey::new("malformed-softwarelist");
    malformed_request.catalog_display_name = "Malformed software list".into();
    let failed = app::import_catalog(database, &malformed_request)?;
    assert_eq!(failed.status, app::CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(count(connection, "catalog_snapshots")?, 1);
    assert!(count(connection, "import_diagnostics")? > 0);
    let failure_location =
        sql_query("SELECT source_line AS value FROM import_diagnostics WHERE run_key = ?")
            .bind::<Text, _>(failed.run_key.to_string())
            .get_result::<IntegerRow>(connection)?;
    assert!(failure_location.value > 0);

    let oversized_path = temp_dir.join("oversized-softwarelist-value.xml");
    std::fs::write(
        &oversized_path,
        b"<softwarelist name=\"range\"><software name=\"game\"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name=\"cart\" interface=\"cart\"><dataarea name=\"rom\" size=\"9223372036854775808\"><rom name=\"game.bin\"/></dataarea></part></software></softwarelist>",
    )?;
    let mut oversized_request = request.clone();
    oversized_request.document_path = Utf8PathBuf::from_path_buf(oversized_path)
        .map_err(|_| "non-UTF8 oversized fixture path")?;
    oversized_request.catalog_key = CatalogKey::new("oversized-softwarelist");
    oversized_request.catalog_display_name = "Oversized software-list value".into();
    let failed = app::import_catalog(database, &oversized_request)?;
    assert_eq!(failed.status, app::CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(count(connection, "catalog_snapshots")?, 1);

    let fixture = include_str!("../fixtures/catalog/mame/software-list.xml");
    for (field, original, replacement, record_kind) in [
        (
            "load",
            "loadflag=\"continue\"",
            "loadflag=\"unknown\"",
            "rom",
        ),
        ("width", "width=\"16\"", "width=\"7\"", "dataarea"),
        (
            "endianness",
            "endianness=\"big\"",
            "endianness=\"unknown\"",
            "dataarea",
        ),
        ("size", "size=\"0x20\"", "size=\"unknown\"", "dataarea"),
        ("crc", "crc=\"12345678\"", "crc=\"unknown\"", "rom"),
    ] {
        let invalid_path = temp_dir.join(format!("invalid-softwarelist-{field}.xml"));
        assert!(fixture.contains(original));
        std::fs::write(&invalid_path, fixture.replacen(original, replacement, 1))?;
        let mut invalid_request = request.clone();
        invalid_request.document_path = Utf8PathBuf::from_path_buf(invalid_path)
            .map_err(|_| "non-UTF8 invalid fixture path")?;
        invalid_request.catalog_key = CatalogKey::new(format!("invalid-softwarelist-{field}"));
        invalid_request.catalog_display_name = format!("Invalid software-list {field}");
        let failed = app::import_catalog(database, &invalid_request)?;
        assert_eq!(failed.status, app::CatalogImportStatus::Failed);
        assert!(failed.snapshot_key.is_none());
        assert_eq!(count(connection, "catalog_snapshots")?, 1);
        let location = sql_query(
            "SELECT record_kind, record_name, source_line, source_column \
             FROM import_diagnostics WHERE run_key = ?",
        )
        .bind::<Text, _>(failed.run_key.to_string())
        .get_result::<FailedLocationRow>(connection)?;
        assert_eq!(location.record_kind.as_deref(), Some(record_kind));
        assert_eq!(
            location.record_name.as_deref(),
            Some("demo_cart:demo_game:cart:data:program")
        );
        assert!(location.source_line.is_some_and(|line| line > 0));
        assert!(location.source_column.is_some_and(|column| column > 0));
    }
    Ok(())
}

#[test]
fn imports_clrmamepro_sets_rom_statuses_and_retained_source_tokens()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let request = clrmamepro_request()?;
    let report = app::import_catalog(&database, &request)?;
    let repeated = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("ClrMamePro snapshot missing")?;
    assert_eq!(Some(snapshot.clone()), repeated.snapshot_key);
    let format_hint = sql_query(
        "SELECT documents.format_hint AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(format_hint.value.as_deref(), Some("clrmamepro-text"));
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    assert_eq!(count(&mut connection, "snapshot_sets")?, 2);
    assert_eq!(count(&mut connection, "asset_requirements")?, 6);
    assert_clrmamepro_set_metadata(&mut connection, &snapshot)?;
    assert_clrmamepro_rom_facts(&mut connection, &snapshot)?;
    assert_clrmamepro_retained_tokens(&mut connection, &snapshot)?;
    assert!(report.diagnostic_count >= 3);
    Ok(())
}

fn assert_clrmamepro_set_metadata(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let clone = sql_query(
        "SELECT parent_name AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'clone_set'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(connection)?;
    assert_eq!(clone.value.as_deref(), Some("demo_set"));
    let metadata = sql_query(
        "SELECT metadata_json AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'demo_set'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert!(metadata.value.contains("Synthetic parent set"));
    assert!(metadata.value.contains("Example Works"));
    assert!(metadata.value.contains("1998"));
    let clone_metadata = sql_query(
        "SELECT metadata_json AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'clone_set'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    let clone_metadata: serde_json::Value = serde_json::from_str(&clone_metadata.value)?;
    assert_eq!(clone_metadata["description"], "Clone \"quoted\" set");
    Ok(())
}

fn assert_clrmamepro_rom_facts(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let primary_rom = sql_query(
        "SELECT CAST(size AS TEXT) AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'demo.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(connection)?;
    assert_eq!(primary_rom.value.as_deref(), Some("16"));
    let primary_crc = sql_query(
        "SELECT crc AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'demo.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<BytesRow>(connection)?;
    assert_eq!(primary_crc.value, [0x12, 0x34, 0x56, 0x78]);
    let primary_sha1 = sql_query(
        "SELECT sha1 AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'demo.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<BytesRow>(connection)?;
    assert_eq!(
        primary_sha1.value,
        hex::decode("0123456789abcdef0123456789abcdef01234567")?
    );

    let merged = sql_query(
        "SELECT merge_name AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'shared.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NullableTextRow>(connection)?;
    assert_eq!(merged.value.as_deref(), Some("demo.bin"));
    let statuses = sql_query(
        "SELECT GROUP_CONCAT(asset_name || ':' || dump_status, ',') AS value \
         FROM (SELECT asset_name, dump_status FROM asset_requirements \
               WHERE snapshot_key = ? AND dump_status IS NOT NULL ORDER BY component_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(statuses.value, "no_dump.bin:nodump,bad_dump.bin:baddump");
    let partial_hash = sql_query(
        "SELECT sha1 AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'partial.bin'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<BytesRow>(connection)?;
    assert_eq!(partial_hash.value, vec![0xaa; 20]);
    Ok(())
}

fn assert_clrmamepro_retained_tokens(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let unknown = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE snapshot_key = ? AND field_name = 'futureflag'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert!(unknown.value.contains("future-token"));
    let header_tokens = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         WHERE snapshot_key = ? AND record_kind = 'clrmamepro' AND field_name = 'source_tokens'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert!(header_tokens.value.contains("Synthetic Catalog"));
    let comment_count = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions \
         WHERE snapshot_key = ? AND field_name = 'comment'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(connection)?;
    assert!(comment_count.count >= 2);
    let version =
        sql_query("SELECT declared_version AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(connection)?;
    assert_eq!(version.value, "synthetic-1");
    Ok(())
}

#[test]
fn malformed_clrmamepro_record_has_location_and_publishes_no_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("malformed.dat");
    std::fs::write(
        &path,
        b"clrmamepro ( version 1 )\ngame ( name broken rom ( name bad.bin sha1 invalid ) )",
    )?;
    let mut request = request(
        path,
        "clrmamepro-bad",
        "malformed-clrmamepro",
        "Malformed DAT",
    )?;
    request.format = CatalogDocumentFormat::ClrMamePro;
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    let diagnostic = sql_query(
        "SELECT record_kind || ':' || record_name AS value FROM import_diagnostics \
         WHERE run_key = ?",
    )
    .bind::<Text, _>(report.run_key.to_string())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(diagnostic.value, "rom:bad.bin");
    let location =
        sql_query("SELECT source_line AS value FROM import_diagnostics WHERE run_key = ?")
            .bind::<Text, _>(report.run_key.to_string())
            .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(location.value, 2);
    Ok(())
}

#[test]
fn clrmamepro_and_logiqx_normalize_shared_set_and_rom_facts_equivalently()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let logiqx_path = directory.path().join("equivalent.xml");
    let clrmamepro_path = directory.path().join("equivalent.dat");
    std::fs::write(
        &logiqx_path,
        br#"<datafile><header><name>Equivalent</name></header><game name="equiv"><description>Equivalent set</description><year>1998</year><manufacturer>Example Works</manufacturer><rom name="equiv.bin" size="16" crc="12345678"/></game></datafile>"#,
    )?;
    std::fs::write(
        &clrmamepro_path,
        b"clrmamepro ( version v1 ) game ( name equiv description \"Equivalent set\" year 1998 manufacturer \"Example Works\" rom ( name equiv.bin size 16 crc 12345678 ) )",
    )?;
    let logiqx = app::import_catalog(
        &database,
        &request(logiqx_path, "equiv-logiqx", "equiv-logiqx", "Equivalent")?,
    )?;
    let mut dat_request = request(
        clrmamepro_path,
        "equiv-clrmamepro",
        "equiv-clrmamepro",
        "Equivalent",
    )?;
    dat_request.format = CatalogDocumentFormat::ClrMamePro;
    let clrmamepro = app::import_catalog(&database, &dat_request)?;
    let logiqx_snapshot = logiqx.snapshot_key.ok_or("Logiqx snapshot missing")?;
    let clrmamepro_snapshot = clrmamepro
        .snapshot_key
        .ok_or("ClrMamePro snapshot missing")?;

    for field in ["description", "year", "manufacturer"] {
        let logiqx_metadata = sql_query(
            "SELECT metadata_json AS value FROM snapshot_sets \
             WHERE snapshot_key = ? AND set_name = 'equiv'",
        )
        .bind::<Text, _>(logiqx_snapshot.as_str())
        .get_result::<TextRow>(&mut connection)?;
        let dat_metadata = sql_query(
            "SELECT metadata_json AS value FROM snapshot_sets \
             WHERE snapshot_key = ? AND set_name = 'equiv'",
        )
        .bind::<Text, _>(clrmamepro_snapshot.as_str())
        .get_result::<TextRow>(&mut connection)?;
        let logiqx_value: serde_json::Value = serde_json::from_str(&logiqx_metadata.value)?;
        let dat_value: serde_json::Value = serde_json::from_str(&dat_metadata.value)?;
        assert_eq!(logiqx_value[field], dat_value[field], "{field}");
    }
    let logiqx_size = sql_query(
        "SELECT CAST(size AS TEXT) AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'equiv.bin'",
    )
    .bind::<Text, _>(logiqx_snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    let dat_size = sql_query(
        "SELECT CAST(size AS TEXT) AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'equiv.bin'",
    )
    .bind::<Text, _>(clrmamepro_snapshot.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(logiqx_size.value, dat_size.value);
    let logiqx_crc = sql_query(
        "SELECT crc AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'equiv.bin'",
    )
    .bind::<Text, _>(logiqx_snapshot.as_str())
    .get_result::<BytesRow>(&mut connection)?;
    let dat_crc = sql_query(
        "SELECT crc AS value FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'equiv.bin'",
    )
    .bind::<Text, _>(clrmamepro_snapshot.as_str())
    .get_result::<BytesRow>(&mut connection)?;
    assert_eq!(logiqx_crc.value, dat_crc.value);
    Ok(())
}

#[test]
fn malformed_mame_xml_records_failed_run_without_snapshot() -> Result<(), Box<dyn std::error::Error>>
{
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("malformed.xml");
    std::fs::write(&path, b"<mame build='broken'><machine name='unfinished'>")?;
    let mut request = request(path, "mame", "malformed-mame", "Malformed MAME")?;
    request.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
    assert_eq!(count(&mut connection, "import_runs")?, 1);
    assert_eq!(count(&mut connection, "import_diagnostics")?, 1);
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
fn snapshot_diff_separates_hash_changes_from_metadata_and_regrouping()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, _connection) = setup()?;
    let mut first_request = request(
        fixture("catalog-a-v1.dat"),
        "publisher-a",
        "catalog-a",
        "Catalog A",
    )?;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        fixture("catalog-a-v2.dat"),
        "publisher-a",
        "catalog-a",
        "Catalog A",
    )?;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let previous = first.snapshot_key.ok_or("first snapshot missing")?;
    let current = second.snapshot_key.ok_or("second snapshot missing")?;

    app::record_relationship(
        &database,
        &RelationshipClaim {
            relation_type: RelationshipType::CatalogContinuity,
            subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "publisher-note",
                "alpha-history",
            )),
            target: RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
                current.clone(),
                CatalogRecordKind::Set,
                "alpha",
            )),
            origin: RelationshipOrigin::UserConclusion,
            evidence: serde_json::json!({"reason": "explicitly linked"}),
        },
    )?;

    let history = app::catalog_snapshot_history(&database, &CatalogKey::new("catalog-a"))?;
    assert_eq!(history.len(), 2);
    assert!(history.windows(2).all(|pair| {
        (pair[0].document_key.as_str(), pair[0].snapshot.as_str())
            <= (pair[1].document_key.as_str(), pair[1].snapshot.as_str())
    }));
    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let alpha = diff
        .records
        .iter()
        .find(|record| record.set_name == "alpha")
        .ok_or("alpha diff missing")?;
    assert_eq!(alpha.status, SnapshotRecordStatus::Changed);
    assert!(!alpha.metadata_changed);
    assert!(!alpha.regrouped);
    assert!(alpha.relationship_evidence.iter().any(|evidence| matches!(
        &evidence.claim.origin,
        mame_coalesce::domain::RelationshipOrigin::SourceAssertion { .. }
    )));
    assert!(alpha.relationship_evidence.iter().any(|evidence| matches!(
        &evidence.claim.subject,
        mame_coalesce::domain::RelationshipEndpoint::ExternalRecord(_)
    )));
    let disputed = alpha
        .requirement_changes
        .iter()
        .find(|change| change.asset_name == "disputed.bin")
        .ok_or("changed requirement missing")?;
    assert!(disputed.hash_changed);
    assert!(!disputed.size_changed);
    assert!(disputed.other_evidence_changed);
    assert_eq!(
        disputed
            .previous
            .as_ref()
            .and_then(|v| v[0]["crc"].as_str()),
        Some("11111111")
    );
    assert_eq!(
        disputed.current.as_ref().and_then(|v| v[0]["crc"].as_str()),
        Some("22222222")
    );
    Ok(())
}

#[test]
fn snapshot_diff_treats_filtered_absence_as_unknown_and_complete_absence_as_removal()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let complete_path = directory.path().join("complete.dat");
    let filtered_path = directory.path().join("filtered.dat");
    std::fs::write(
        &complete_path,
        br#"<datafile><header><name>Scope</name></header><game name="alpha"/><game name="beta"/></datafile>"#,
    )?;
    std::fs::write(
        &filtered_path,
        br#"<datafile><header><name>Scope</name></header><game name="alpha"/></datafile>"#,
    )?;
    let mut complete_request = request(complete_path, "publisher-scope", "scope", "Scope")?;
    complete_request.scope = CatalogScope::Complete;
    let complete = app::import_catalog(&database, &complete_request)?;
    let mut filtered_request = request(filtered_path, "publisher-scope", "scope", "Scope")?;
    filtered_request.scope = CatalogScope::Filtered(serde_json::json!({"sets": ["alpha"]}));
    let filtered = app::import_catalog(&database, &filtered_request)?;
    let complete_key = complete.snapshot_key.ok_or("complete snapshot missing")?;
    let filtered_key = filtered.snapshot_key.ok_or("filtered snapshot missing")?;

    let diff = app::diff_catalog_snapshots(&database, &complete_key, &filtered_key)?;
    assert!(!diff.same_scope);
    let beta = diff
        .records
        .iter()
        .find(|record| record.set_name == "beta")
        .ok_or("beta diff missing")?;
    assert_eq!(beta.status, SnapshotRecordStatus::OutOfScope);

    let unknown_request = request(
        filtered_request.document_path.into_std_path_buf(),
        "publisher-scope",
        "scope",
        "Scope",
    )?;
    let unknown = app::import_catalog(&database, &unknown_request)?;
    let unknown_key = unknown
        .snapshot_key
        .ok_or("unknown-scope snapshot missing")?;
    let unknown_diff = app::diff_catalog_snapshots(&database, &complete_key, &unknown_key)?;
    assert_eq!(
        unknown_diff
            .records
            .iter()
            .find(|record| record.set_name == "beta")
            .map(|record| record.status),
        Some(SnapshotRecordStatus::Unknown)
    );

    let mut next_complete_request = request(
        fixture("catalog-a-filtered.dat"),
        "publisher-scope",
        "scope",
        "Scope",
    )?;
    next_complete_request.scope = CatalogScope::Complete;
    let next_complete = app::import_catalog(&database, &next_complete_request)?;
    let next_key = next_complete.snapshot_key.ok_or("next snapshot missing")?;
    let removal = app::diff_catalog_snapshots(&database, &complete_key, &next_key)?;
    assert!(removal.same_scope);
    assert_eq!(
        removal
            .records
            .iter()
            .find(|record| record.set_name == "beta")
            .map(|record| record.status),
        Some(SnapshotRecordStatus::RemovedWithinScope)
    );

    let included_path = directory.path().join("included-filter.dat");
    std::fs::write(
        &included_path,
        br#"<datafile><header><name>Scope</name></header><game name="alpha"/><game name="beta"/><game name="gamma"/></datafile>"#,
    )?;
    let mut included_request = request(included_path, "publisher-scope", "scope", "Scope")?;
    included_request.scope = CatalogScope::Filtered(serde_json::json!({
        "sets": ["alpha", "beta", "gamma"]
    }));
    let included = app::import_catalog(&database, &included_request)?;
    let included_key = included.snapshot_key.ok_or("included snapshot missing")?;
    let addition = app::diff_catalog_snapshots(&database, &complete_key, &included_key)?;
    assert_eq!(
        addition
            .records
            .iter()
            .find(|record| record.set_name == "gamma")
            .map(|record| record.status),
        Some(SnapshotRecordStatus::AddedWithinScope)
    );
    Ok(())
}

#[test]
fn snapshot_diff_requires_known_filtered_set_membership_to_compare_scopes()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let mut snapshots = Vec::new();
    for (filename, games) in [
        ("filtered-alpha.dat", "<game name=\"alpha\"/>"),
        (
            "filtered-alpha-beta.dat",
            "<game name=\"alpha\"/><game name=\"beta\"/>",
        ),
    ] {
        let path = directory.path().join(filename);
        std::fs::write(
            &path,
            format!("<datafile><header><name>Scope</name></header>{games}</datafile>"),
        )?;
        let mut import = request(path, "publisher-scope", "scope", "Scope")?;
        import.scope = CatalogScope::Filtered(serde_json::json!({"source": "manual"}));
        snapshots.push(
            app::import_catalog(&database, &import)?
                .snapshot_key
                .ok_or("filtered snapshot without set names missing")?,
        );
    }
    let diff = app::diff_catalog_snapshots(&database, &snapshots[0], &snapshots[1])?;
    assert!(!diff.same_scope);
    Ok(())
}

#[test]
fn snapshot_diff_tracks_unknown_extensions_and_does_not_call_missing_hashes_changed()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("machine-v1.xml");
    let second_path = directory.path().join("machine-v2.xml");
    std::fs::write(
        &first_path,
        br#"<mame><machine name="thing"><description>Thing</description><future value="one"/></machine></mame>"#,
    )?;
    std::fs::write(
        &second_path,
        br#"<mame><machine name="thing"><description>Thing</description><future value="two"/><rom name="undumped.bin"/></machine></mame>"#,
    )?;
    let mut first_request = request(first_path, "publisher-machines", "machines", "Machines")?;
    first_request.format = CatalogDocumentFormat::MameListXml;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(second_path, "publisher-machines", "machines", "Machines")?;
    second_request.format = CatalogDocumentFormat::MameListXml;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        &first.snapshot_key.ok_or("first snapshot missing")?,
        &second.snapshot_key.ok_or("second snapshot missing")?,
    )?;
    let thing = diff
        .records
        .iter()
        .find(|record| record.set_name == "thing")
        .ok_or("machine diff missing")?;
    assert!(thing.metadata_changed);
    let undumped = thing
        .requirement_changes
        .iter()
        .find(|change| change.asset_name == "undumped.bin")
        .ok_or("new undumped requirement missing")?;
    assert!(!undumped.size_changed);
    assert!(!undumped.hash_changed);
    Ok(())
}

#[test]
fn snapshot_diff_tracks_logiqx_game_extensions() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let logiqx_v1 = directory.path().join("logiqx-extension-v1.dat");
    let logiqx_v2 = directory.path().join("logiqx-extension-v2.dat");
    std::fs::write(
        &logiqx_v1,
        br#"<datafile><header><name>Extension test</name></header><game name="thing" future="one"/></datafile>"#,
    )?;
    std::fs::write(
        &logiqx_v2,
        br#"<datafile><header><name>Extension test</name></header><game name="thing" future="two"/></datafile>"#,
    )?;
    let mut logiqx_first = request(
        logiqx_v1,
        "publisher-logiqx-extensions",
        "logiqx-extensions",
        "Extension test",
    )?;
    logiqx_first.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &logiqx_first)?;
    let mut logiqx_second = request(
        logiqx_v2,
        "publisher-logiqx-extensions",
        "logiqx-extensions",
        "Extension test",
    )?;
    logiqx_second.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &logiqx_second)?;
    let logiqx_diff = app::diff_catalog_snapshots(
        &database,
        &first.snapshot_key.ok_or("Logiqx first snapshot missing")?,
        &second
            .snapshot_key
            .ok_or("Logiqx second snapshot missing")?,
    )?;
    assert!(logiqx_diff.records[0].metadata_changed);
    Ok(())
}

#[test]
fn snapshot_diff_attributes_logiqx_extensions_to_their_asset_and_game()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("logiqx-owned-extensions-v1.dat");
    let second_path = directory.path().join("logiqx-owned-extensions-v2.dat");
    for (path, value) in [(&first_path, "one"), (&second_path, "two")] {
        std::fs::write(
            path,
            format!(
                "<datafile><header><name>Owned extensions</name></header><game name=\"owner\"><rom name=\"shared.rom\" future=\"{value}\"/><device_ref name=\"target\" future=\"{value}\"/></game><game name=\"target\"/></datafile>"
            ),
        )?;
    }
    let mut first_request = request(
        first_path,
        "publisher-logiqx-owned-extensions",
        "logiqx-owned-extensions",
        "Owned extensions",
    )?;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-logiqx-owned-extensions",
        "logiqx-owned-extensions",
        "Owned extensions",
    )?;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        &first.snapshot_key.ok_or("first snapshot missing")?,
        &second.snapshot_key.ok_or("second snapshot missing")?,
    )?;
    let owner = diff
        .records
        .iter()
        .find(|record| record.set_name == "owner")
        .ok_or("owner diff missing")?;
    assert_eq!(owner.status, SnapshotRecordStatus::Changed);
    assert!(owner.metadata_changed);
    let asset = owner
        .requirement_changes
        .iter()
        .find(|change| change.asset_name == "shared.rom")
        .ok_or("owned asset extension change missing")?;
    assert!(asset.other_evidence_changed);

    let target = diff
        .records
        .iter()
        .find(|record| record.set_name == "target")
        .ok_or("target diff missing")?;
    assert_eq!(target.status, SnapshotRecordStatus::Unchanged);
    assert!(!target.metadata_changed);
    Ok(())
}

#[test]
fn snapshot_diff_attributes_device_ref_extensions_to_the_owning_set()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let device_ref_v1 = directory.path().join("device-ref-v1.xml");
    let device_ref_v2 = directory.path().join("device-ref-v2.xml");
    for (path, value) in [(&device_ref_v1, "one"), (&device_ref_v2, "two")] {
        std::fs::write(
            path,
            format!(
                "<mame><machine name=\"owner\"><description>Owner</description><device_ref name=\"target\" future=\"{value}\"/></machine><machine name=\"target\"><description>Target</description></machine></mame>"
            ),
        )?;
    }
    let mut device_ref_first = request(
        device_ref_v1,
        "publisher-device-ref",
        "device-refs",
        "Device refs",
    )?;
    device_ref_first.format = CatalogDocumentFormat::MameListXml;
    device_ref_first.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &device_ref_first)?;
    let mut device_ref_second = request(
        device_ref_v2,
        "publisher-device-ref",
        "device-refs",
        "Device refs",
    )?;
    device_ref_second.format = CatalogDocumentFormat::MameListXml;
    device_ref_second.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &device_ref_second)?;
    let device_ref_diff = app::diff_catalog_snapshots(
        &database,
        &first
            .snapshot_key
            .ok_or("device-ref first snapshot missing")?,
        &second
            .snapshot_key
            .ok_or("device-ref second snapshot missing")?,
    )?;
    let owner = device_ref_diff
        .records
        .iter()
        .find(|record| record.set_name == "owner")
        .ok_or("owner diff missing")?;
    assert_eq!(owner.status, SnapshotRecordStatus::Changed);
    assert!(owner.metadata_changed);

    let target = device_ref_diff
        .records
        .iter()
        .find(|record| record.set_name == "target")
        .ok_or("target diff missing")?;
    assert_eq!(target.status, SnapshotRecordStatus::Unchanged);
    assert!(!target.metadata_changed);
    Ok(())
}

#[test]
fn snapshot_diff_keeps_asset_extensions_with_their_owning_set()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let first_path = directory.path().join("asset-extension-v1.xml");
    let second_path = directory.path().join("asset-extension-v2.xml");
    for (path, changed_value) in [(&first_path, "one"), (&second_path, "two")] {
        std::fs::write(
            path,
            format!(
                "<mame><machine name=\"alpha\"><description>Alpha</description><rom name=\"shared.rom\" future=\"{changed_value}\"/></machine><machine name=\"beta\"><description>Beta</description><rom name=\"shared.rom\"/></machine></mame>"
            ),
        )?;
    }
    let mut first_request = request(
        first_path,
        "publisher-asset-extensions",
        "asset-extensions",
        "Asset extensions",
    )?;
    first_request.format = CatalogDocumentFormat::MameListXml;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-asset-extensions",
        "asset-extensions",
        "Asset extensions",
    )?;
    second_request.format = CatalogDocumentFormat::MameListXml;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        first
            .snapshot_key
            .as_ref()
            .ok_or("first asset-extension snapshot missing")?,
        second
            .snapshot_key
            .as_ref()
            .ok_or("second asset-extension snapshot missing")?,
    )?;

    let alpha = diff
        .records
        .iter()
        .find(|record| record.set_name == "alpha")
        .ok_or("alpha diff missing")?;
    let alpha_asset = alpha
        .requirement_changes
        .iter()
        .find(|change| change.asset_name == "shared.rom")
        .ok_or("alpha asset extension change missing")?;
    assert!(alpha_asset.other_evidence_changed);

    let beta = diff
        .records
        .iter()
        .find(|record| record.set_name == "beta")
        .ok_or("beta diff missing")?;
    assert_eq!(beta.status, SnapshotRecordStatus::Unchanged);
    assert!(beta.requirement_changes.is_empty());

    let metadata = sql_query(
        "SELECT metadata_json AS value FROM asset_requirements WHERE snapshot_key = ? LIMIT 1",
    )
    .bind::<Text, _>(
        second
            .snapshot_key
            .as_ref()
            .ok_or("second snapshot missing")?
            .as_str(),
    )
    .get_result::<TextRow>(&mut connection)?;
    assert!(!metadata.value.contains("__mame_coalesce_extensions"));
    Ok(())
}

#[test]
fn snapshot_diff_attributes_no_intro_rom_extensions_to_their_asset()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("no-intro-asset-extension-v1.xml");
    let second_path = directory.path().join("no-intro-asset-extension-v2.xml");
    for (path, changed_value) in [(&first_path, "one"), (&second_path, "two")] {
        std::fs::write(
            path,
            format!(
                "<datafile><header><name>Extensions</name></header><game name=\"alpha\"><rom name=\"shared.bin\" future=\"{changed_value}\"/></game><game name=\"beta\"><rom name=\"shared.bin\"/></game></datafile>"
            ),
        )?;
    }
    let mut first_request = request(
        first_path,
        "publisher-no-intro-extensions",
        "no-intro-extensions",
        "No-Intro extensions",
    )?;
    first_request.format = CatalogDocumentFormat::NoIntroPcXml;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-no-intro-extensions",
        "no-intro-extensions",
        "No-Intro extensions",
    )?;
    second_request.format = CatalogDocumentFormat::NoIntroPcXml;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        first
            .snapshot_key
            .as_ref()
            .ok_or("first snapshot missing")?,
        second
            .snapshot_key
            .as_ref()
            .ok_or("second snapshot missing")?,
    )?;
    assert!(diff.records.iter().any(|record| {
        record.set_name == "alpha"
            && record
                .requirement_changes
                .iter()
                .any(|change| change.asset_name == "shared.bin" && change.other_evidence_changed)
    }));
    assert!(diff.records.iter().any(|record| {
        record.set_name == "beta"
            && record.status == SnapshotRecordStatus::Unchanged
            && record.requirement_changes.is_empty()
    }));
    Ok(())
}

#[test]
fn snapshot_diff_does_not_treat_unpublished_complete_identity_as_removal()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let published_path = directory.path().join("published.dat");
    let identity_path = directory.path().join("identity.dat");
    let published_bytes =
        br#"<datafile><header><name>Published</name></header><game name="set"/></datafile>"#;
    let identity_bytes = b"unpublished identity bytes";
    std::fs::write(&published_path, published_bytes)?;
    std::fs::write(&identity_path, identity_bytes)?;

    let mut published_request =
        request(published_path, "publisher-identity", "identity", "Identity")?;
    published_request.scope = CatalogScope::Complete;
    let published = app::import_catalog(&database, &published_request)?;
    let published_key = published.snapshot_key.ok_or("published snapshot missing")?;
    let identity_document_key = DocumentKey::from_bytes(identity_bytes);
    let identity_document = identity_document_key.to_string();
    let interpretation = ParserInterpretationKey::logiqx_v1(&CatalogScope::Complete);
    let identity_snapshot = SnapshotKey::new(
        &published_request.catalog_key,
        &identity_document_key,
        &interpretation,
    );
    sql_query("INSERT INTO documents (document_key) VALUES (?)")
        .bind::<Text, _>(&identity_document)
        .execute(&mut connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots \
         (snapshot_key, catalog_key, document_key, interpretation_key, scope_kind) \
         VALUES (?, ?, ?, ?, 'complete')",
    )
    .bind::<Text, _>(identity_snapshot.as_str())
    .bind::<Text, _>(published_request.catalog_key.as_str())
    .bind::<Text, _>(&identity_document)
    .bind::<Text, _>(interpretation.as_str())
    .execute(&mut connection)?;

    let diff = app::diff_catalog_snapshots(&database, &published_key, &identity_snapshot)?;
    assert!(!diff.same_scope);
    assert_eq!(diff.records.len(), 1);
    assert_eq!(diff.records[0].set_name, "set");
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unknown);
    Ok(())
}

#[test]
fn snapshot_diff_preserves_duplicate_asset_extension_occurrences()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("duplicate-extension-v1.xml");
    let second_path = directory.path().join("duplicate-extension-v2.xml");
    for (path, count) in [(&first_path, 2), (&second_path, 1)] {
        let children = "<future/>".repeat(count);
        std::fs::write(
            path,
            format!(
                "<mame><machine name=\"alpha\"><description>Alpha</description><rom name=\"shared.rom\">{children}</rom></machine></mame>"
            ),
        )?;
    }
    let mut first_request = request(
        first_path,
        "publisher-duplicate-extensions",
        "duplicate-extensions",
        "Duplicate extensions",
    )?;
    first_request.format = CatalogDocumentFormat::MameListXml;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-duplicate-extensions",
        "duplicate-extensions",
        "Duplicate extensions",
    )?;
    second_request.format = CatalogDocumentFormat::MameListXml;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        first
            .snapshot_key
            .as_ref()
            .ok_or("first snapshot missing")?,
        second
            .snapshot_key
            .as_ref()
            .ok_or("second snapshot missing")?,
    )?;
    assert!(diff.records.iter().any(|record| {
        record.set_name == "alpha"
            && record
                .requirement_changes
                .iter()
                .any(|change| change.asset_name == "shared.rom" && change.other_evidence_changed)
    }));
    Ok(())
}

#[test]
fn snapshot_diff_compares_duplicate_asset_fields_as_unordered_multisets()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("duplicates-v1.dat");
    let second_path = directory.path().join("duplicates-v2.dat");
    std::fs::write(
        &first_path,
        br#"<datafile><header><name>Duplicates</name></header><game name="set"><rom name="same.bin" size="1" crc="11111111" status="good"/><rom name="same.bin" size="2" crc="11111111" status="bad"/></game></datafile>"#,
    )?;
    std::fs::write(
        &second_path,
        br#"<datafile><header><name>Duplicates</name></header><game name="set"><rom name="same.bin" size="2" crc="11111111" status="good"/><rom name="same.bin" size="1" crc="11111111" status="bad"/></game></datafile>"#,
    )?;
    let mut first_request = request(
        first_path,
        "publisher-duplicates",
        "duplicates",
        "Duplicates",
    )?;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-duplicates",
        "duplicates",
        "Duplicates",
    )?;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        &first.snapshot_key.ok_or("first snapshot missing")?,
        &second.snapshot_key.ok_or("second snapshot missing")?,
    )?;
    let duplicate = diff.records[0]
        .requirement_changes
        .first()
        .ok_or("duplicate requirement change missing")?;
    assert!(!duplicate.size_changed);
    assert!(!duplicate.hash_changed);
    assert!(duplicate.other_evidence_changed);
    Ok(())
}

#[test]
fn snapshot_diff_is_order_independent_and_rejects_cross_catalog_name_matching()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _connection) = setup()?;
    let first_path = directory.path().join("first-order.dat");
    let second_path = directory.path().join("second-order.dat");
    std::fs::write(
        &first_path,
        br#"<datafile><header><name>Order</name></header><game name="one"><rom name="one.rom" crc="11111111"/></game><game name="two"><rom name="two.rom" crc="22222222"/></game></datafile>"#,
    )?;
    std::fs::write(
        &second_path,
        br#"<datafile><header><name>Order</name></header><game name="two"><rom name="two.rom" crc="22222222"/></game><game name="one"><rom name="one.rom" crc="11111111"/></game></datafile>"#,
    )?;
    let mut first_request = request(first_path, "publisher-order", "order", "Order")?;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(second_path, "publisher-order", "order", "Order")?;
    second_request.scope = CatalogScope::Complete;
    let second = app::import_catalog(&database, &second_request)?;
    let first_key = first.snapshot_key.ok_or("first snapshot missing")?;
    let second_key = second.snapshot_key.ok_or("second snapshot missing")?;
    let diff = app::diff_catalog_snapshots(&database, &first_key, &second_key)?;
    assert_eq!(diff.records.len(), 2);
    assert!(
        diff.records
            .iter()
            .all(|record| record.status == SnapshotRecordStatus::Unchanged)
    );

    let other_catalog = app::import_catalog(
        &database,
        &request(
            fixture("catalog-b.dat"),
            "publisher-b",
            "another-order",
            "Another catalog",
        )?,
    )?;
    let other_key = other_catalog.snapshot_key.ok_or("other snapshot missing")?;
    assert!(app::diff_catalog_snapshots(&database, &first_key, &other_key).is_err());
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
fn source_relationship_assertions_keep_snapshot_and_field_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let imported = app::import_catalog(
        &database,
        &request(
            fixture("catalog-a-v1.dat"),
            "publisher-a",
            "catalog-a",
            "Catalog A",
        )?,
    )?;
    let snapshot_key = imported
        .snapshot_key
        .as_ref()
        .ok_or_else(|| io::Error::other("successful import has no snapshot key"))?;
    let assertion = sql_query(
        "SELECT relation_type, origin, source_snapshot_key, subject_key, target_key, \
                source_field, source_line, source_column, rule_version \
         FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'cloneof'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<QueryableAssertion>(&mut connection)?;
    assert_eq!(assertion.relation_type, "source_parent_clone");
    assert_eq!(assertion.origin, "source_assertion");
    assert_eq!(assertion.subject_key, "alpha");
    assert_eq!(assertion.target_key, "parent");
    assert_eq!(assertion.source_field.as_deref(), Some("cloneof"));
    assert_eq!(assertion.source_line, Some(4));
    assert_eq!(assertion.source_column, Some(3));
    assert!(assertion.rule_version.is_none());
    assert_eq!(
        assertion.source_snapshot_key.as_deref(),
        Some(snapshot_key.as_str())
    );

    let runtime_claims = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND relation_type = 'runtime_dependency' \
           AND source_field IN ('romof', 'sampleof', 'device_ref')",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(runtime_claims.count, 3);
    let device_claim = sql_query(
        "SELECT target_key AS value FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'device_ref'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(device_claim.value, "fixture-sound");

    // An identical reimport reuses the immutable snapshot rather than duplicating its claims.
    app::import_catalog(
        &database,
        &request(
            fixture("catalog-a-v1.dat"),
            "publisher-a",
            "catalog-a",
            "Catalog A",
        )?,
    )?;
    let claims = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'cloneof'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(claims.count, 1);
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
