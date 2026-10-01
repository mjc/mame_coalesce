use std::{
    io,
    path::{Path, PathBuf},
};

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogRecordKind, CatalogRecordRef, CatalogScope, DocumentKey,
        ExternalRecordRef, ParserInterpretationKey, PublishingSourceKey, QualifiedCatalogSet,
        RelationshipClaim, RelationshipEndpoint, RelationshipOrigin, RelationshipType, SetName,
        SnapshotKey, SnapshotRecordStatus,
    },
};

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct SharedContentStats {
    #[diesel(sql_type = BigInt)]
    occurrences: i64,
    #[diesel(sql_type = BigInt)]
    linked_occurrences: i64,
    #[diesel(sql_type = BigInt)]
    identities: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    minimum_uuid_bytes: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    storage_class: Option<String>,
}

#[derive(QueryableByName)]
struct NullableBinaryRow {
    #[diesel(sql_type = Nullable<Binary>)]
    value: Option<Vec<u8>>,
}

#[derive(QueryableByName)]
struct MachineSwitchRow {
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = BigInt)]
    mask: i64,
}

#[derive(QueryableByName)]
struct MachineSwitchLocationRow {
    #[diesel(sql_type = BigInt)]
    location_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    number: String,
    #[diesel(sql_type = BigInt)]
    inverted: i64,
}

#[derive(QueryableByName)]
struct MachineSwitchValueRow {
    #[diesel(sql_type = BigInt)]
    value_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    value: i64,
    #[diesel(sql_type = BigInt)]
    is_default: i64,
}

#[derive(QueryableByName)]
struct MachineBiosSetRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = BigInt)]
    is_default: i64,
}

#[derive(QueryableByName)]
struct MameMachineFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type = BigInt)]
    is_device: i64,
    #[diesel(sql_type = BigInt)]
    runnable: i64,
    #[diesel(sql_type = BigInt)]
    is_bios: i64,
    #[diesel(sql_type = BigInt)]
    is_mechanical: i64,
    #[diesel(sql_type = BigInt)]
    is_consumable: i64,
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
struct TypedSourceAssertionRow {
    #[diesel(sql_type = Nullable<Text>, column_name = source_subject_a)]
    subject_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>, column_name = source_subject_b)]
    subject_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>, column_name = source_subject_c)]
    subject_c: Option<i64>,
    #[diesel(sql_type = Nullable<Text>, column_name = source_target_a)]
    target_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>, column_name = source_target_b)]
    target_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>, column_name = source_target_c)]
    target_c: Option<i64>,
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
    md5: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    sha1: Option<Vec<u8>>,
}

#[derive(QueryableByName)]
struct NoIntroGameFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    archive_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
}

#[derive(QueryableByName)]
struct NoIntroPcMetadataRow {
    #[diesel(sql_type = Nullable<Text>)]
    name_alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    bios_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    languages_present: i64,
}

#[derive(QueryableByName)]
struct NoIntroLanguageRow {
    #[diesel(sql_type = BigInt)]
    language_order: i64,
    #[diesel(sql_type = Text)]
    language: String,
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
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    source_line: i64,
}

#[derive(QueryableByName)]
struct SoftwareNamedValueRow {
    #[diesel(sql_type = BigInt)]
    value_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
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

#[derive(QueryableByName, PartialEq, Debug)]
struct ImportDiagnosticRow {
    #[diesel(sql_type = Text)]
    code: String,
    #[diesel(sql_type = Text)]
    message: String,
    #[diesel(sql_type = Nullable<Text>)]
    record_kind: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    record_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    field_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    raw_value_json: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
}

#[derive(QueryableByName)]
struct StoredExtensionRow {
    #[diesel(sql_type = Text)]
    field_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    namespace_uri: Option<String>,
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct LogiqxDocumentFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    debug: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    file_name: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    header_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    header_description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_email: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_category: Option<String>,
}

#[derive(QueryableByName)]
struct LogiqxSetFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    is_bios: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    board: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rebuild_to: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
}

fn setup() -> Result<(tempfile::TempDir, Database, SqliteConnection), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("catalog.sqlite");
    let database =
        Database::open(&Utf8PathBuf::from_path_buf(path.clone()).map_err(|_| "non-UTF8 db path")?)?;
    let connection = SqliteConnection::establish(path.to_str().ok_or("non-UTF8 db path")?)?;
    Ok((directory, database, connection))
}

#[test]
fn composite_key_catalog_tables_cluster_rows_by_their_primary_keys()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, _database, mut connection) = setup()?;
    let clustered = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_list \
         WHERE schema = 'main' AND type = 'table' AND wr = 1 AND name IN ( \
             'acquisition_transport_headers', 'acquisition_attempt_transport_headers', \
             'software_item_info', \
             'software_item_shared_features', 'software_part_features', \
             'software_item_dependencies', 'machine_switches', \
             'machine_switch_locations', 'machine_switch_values', 'mame_bios_sets', \
             'mame_machines', 'mame_machine_dependencies', \
             'software_part_dipswitches', 'software_part_dip_values', 'no_intro_pc_games', \
             'logiqx_games', 'mame_machine_input_controls', \
             'mame_machine_analogs', 'mame_machine_device_extensions', 'mame_machine_slot_options', \
             'relationship_assertion_evidence', 'relationship_assertion_support' \
         )",
    )
    .get_result::<CountRow>(&mut connection)?;

    assert_eq!(clustered.count, 22);

    let specification_tables = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_list \
         WHERE schema = 'main' AND type = 'table' AND wr = 1 AND name IN ( \
             'mame_machine_chips', 'mame_machine_displays', 'mame_machine_inputs', \
             'mame_machine_ports', 'mame_machine_sounds', 'mame_machine_adjusters', \
             'mame_machine_drivers', 'mame_machine_features', 'mame_machine_devices', \
             'mame_machine_device_instances', 'mame_machine_slots', \
             'mame_machine_software_lists', 'mame_machine_ram_options', 'mame_machine_samples' \
         )",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(specification_tables.count, 14);

    for (table, identity_column) in [
        ("catalog_set_groups", "set_group_id"),
        ("catalog_sets", "set_id"),
        ("asset_occurrences", "occurrence_id"),
        ("mame_rom_claims", "occurrence_id"),
        ("mame_disk_claims", "occurrence_id"),
        ("logiqx_rom_claims", "occurrence_id"),
        ("logiqx_disk_claims", "occurrence_id"),
        ("cmp_rom_claims", "occurrence_id"),
        ("no_intro_pc_file_claims", "occurrence_id"),
        ("software_lists", "namespace_id"),
        ("software_items", "record_id"),
        ("software_parts", "part_id"),
        ("software_areas", "area_id"),
        ("software_rom_entries", "occurrence_id"),
        ("software_disk_entries", "occurrence_id"),
        ("software_file_declarations", "occurrence_id"),
        ("software_file_uses", "occurrence_id"),
    ] {
        let integer_identity = sql_query(
            "SELECT COUNT(*) AS count FROM pragma_table_list AS layout \
             JOIN pragma_table_info(layout.name) AS column_info \
             WHERE layout.schema = 'main' AND layout.type = 'table' AND layout.wr = 0 \
             AND layout.name = ? AND column_info.name = ? \
             AND column_info.type = 'INTEGER' AND column_info.pk = 1 \
             AND (SELECT COUNT(*) FROM pragma_table_info(layout.name) WHERE pk > 0) = 1 \
             AND NOT EXISTS (SELECT 1 FROM pragma_index_list(layout.name) WHERE origin = 'pk')",
        )
        .bind::<Text, _>(table)
        .bind::<Text, _>(identity_column)
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(
            integer_identity.count, 1,
            "{table}.{identity_column} must be the integer rowid primary key"
        );
    }
    Ok(())
}

#[test]
fn mame_machine_dependencies_store_a_compact_set_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let (_directory, _database, mut connection) = setup()?;
    let columns = sql_query(
        "SELECT GROUP_CONCAT(name, ',') AS value FROM pragma_table_info('mame_machine_dependencies')",
    )
    .get_result::<TextRow>(&mut connection)?;

    assert!(columns.value.split(',').any(|column| column == "set_id"));
    assert!(
        !columns
            .value
            .split(',')
            .any(|column| { matches!(column, "snapshot_key" | "set_name" | "assertion_key") })
    );
    Ok(())
}

#[test]
fn mame_switch_facts_store_a_compact_set_identity() -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, _database, mut connection) = setup()?;
    for table in [
        "machine_switches",
        "machine_switch_locations",
        "machine_switch_values",
    ] {
        let columns = sql_query(format!(
            "SELECT GROUP_CONCAT(name, ',') AS value FROM pragma_table_info('{table}')"
        ))
        .get_result::<TextRow>(&mut connection)?;
        assert!(
            columns.value.split(',').any(|column| column == "set_id"),
            "{table} must use its compact parent identity"
        );
        assert!(
            !columns
                .value
                .split(',')
                .any(|column| matches!(column, "snapshot_key" | "set_name")),
            "{table} must not repeat the composite text identity"
        );
    }
    Ok(())
}

#[test]
fn asset_requirements_store_compact_set_identity() -> Result<(), Box<dyn std::error::Error>> {
    let (_directory, _database, mut connection) = setup()?;
    let columns = sql_query(
        "SELECT GROUP_CONCAT(name, ',') AS value FROM pragma_table_info('asset_requirement_rows')",
    )
    .get_result::<TextRow>(&mut connection)?;

    assert!(columns.value.split(',').any(|column| column == "set_id"));
    assert!(
        !columns
            .value
            .split(',')
            .any(|column| matches!(column, "snapshot_key" | "set_name"))
    );

    let compatibility_view = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_list \
         WHERE schema = 'main' AND type = 'view' AND name = 'asset_requirements'",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(compatibility_view.count, 1);
    let mame_view = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_list \
         WHERE schema = 'main' AND type = 'view' AND name = 'mame_asset_facts'",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(mame_view.count, 1);
    Ok(())
}

#[test]
fn mame_specification_facts_store_a_compact_set_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let (_directory, _database, mut connection) = setup()?;
    for table in [
        "mame_machine_spec_elements",
        "mame_machine_input_controls",
        "mame_machine_analogs",
        "mame_machine_device_extensions",
        "mame_machine_slot_options",
    ] {
        let columns = sql_query(format!(
            "SELECT GROUP_CONCAT(name, ',') AS value FROM pragma_table_info('{table}')"
        ))
        .get_result::<TextRow>(&mut connection)?;
        assert!(
            columns.value.split(',').any(|column| column == "set_id"),
            "{table} must use its compact parent identity"
        );
        assert!(
            !columns
                .value
                .split(',')
                .any(|column| matches!(column, "snapshot_key" | "set_name")),
            "{table} must not repeat the composite text identity"
        );
    }
    Ok(())
}

fn retained_document(
    directory: &tempfile::TempDir,
    key: &DocumentKey,
) -> mame_coalesce::Result<Vec<u8>> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    mame_coalesce::DocumentStore::open(path.as_str())?.load(key)
}

fn retained_document_for_run(
    directory: &tempfile::TempDir,
    connection: &mut SqliteConnection,
    run_key: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = sql_query("SELECT document_key AS value FROM import_runs WHERE run_key = ?")
        .bind::<Text, _>(run_key)
        .get_result::<TextRow>(connection)?
        .value
        .parse::<DocumentKey>()?;
    Ok(retained_document(directory, &key)?)
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
    request.scope =
        filtered_root_scope(&["Synthetic Cartridge (World)", "Synthetic Cartridge (Japan)"]);
    Ok(request)
}

fn filtered_root_scope(names: &[&str]) -> CatalogScope {
    CatalogScope::Filtered(
        names
            .iter()
            .map(|name| QualifiedCatalogSet::RootSet(SetName::new(*name)))
            .collect(),
    )
}

fn insert_coverage(
    connection: &mut SqliteConnection,
    kind: &str,
) -> Result<i64, diesel::result::Error> {
    sql_query("INSERT INTO catalog_coverage (kind) VALUES (?)")
        .bind::<Text, _>(kind)
        .execute(connection)?;
    sql_query("SELECT last_insert_rowid() AS value")
        .get_result::<IntegerRow>(connection)
        .map(|row| row.value)
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

    let shared_content = sql_query(
        "SELECT COUNT(*) AS occurrences, COUNT(content_uuid) AS linked_occurrences, \
         COUNT(DISTINCT content_uuid) AS identities, MIN(length(content_uuid)) AS minimum_uuid_bytes, \
         MIN(typeof(content_uuid)) AS storage_class FROM asset_requirements WHERE sha1 = ?",
    )
    .bind::<Binary, _>([0xaa; 20].as_slice())
    .get_result::<SharedContentStats>(&mut connection)?;
    assert_eq!(shared_content.occurrences, 2);
    assert_eq!(shared_content.linked_occurrences, 2);
    assert_eq!(shared_content.identities, 1);
    assert_eq!(shared_content.minimum_uuid_bytes, Some(16));
    assert_eq!(shared_content.storage_class.as_deref(), Some("blob"));
    let interned_sha1 = sql_query(
        "SELECT COUNT(*) AS count FROM digest_values \
         WHERE algorithm = 'sha1' AND digest = ?",
    )
    .bind::<Binary, _>([0xaa; 20].as_slice())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(interned_sha1.count, 1);
    let preserved_occurrence_assertions = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirement_digest_assertions AS assertion \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE digest.algorithm = 'sha1' AND digest.digest = ? \
           AND assertion.scope = 'whole_asset' AND assertion.provenance = 'source_declared'",
    )
    .bind::<Binary, _>([0xaa; 20].as_slice())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(preserved_occurrence_assertions.count, 2);
    for table in ["asset_requirement_rows", "software_component_occurrences"] {
        let duplicated_hash_columns = sql_query(format!(
            "SELECT COUNT(*) AS count FROM pragma_table_info('{table}') \
             WHERE name IN ('crc', 'md5', 'sha1')"
        ))
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(
            duplicated_hash_columns.count, 0,
            "{table} stores no digest bytes"
        );
    }

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
fn conflicting_catalog_digest_claims_remain_unlinked() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let baseline = request(
        fixture("catalog-a-v1.dat"),
        "publisher-a",
        "catalog-a",
        "Catalog A",
    )?;
    app::import_catalog(&database, &baseline)?;

    for (name, size, crc) in [
        ("contradictory-size.bin", 17, "12345678"),
        ("contradictory-crc.bin", 16, "99999999"),
    ] {
        let path = directory.path().join(format!("{name}.dat"));
        std::fs::write(
            &path,
            format!(
                "<datafile><header><name>Conflict</name></header><game name=\"conflict-{name}\"><rom name=\"{name}\" size=\"{size}\" crc=\"{crc}\" sha1=\"{}\"/></game></datafile>",
                "a".repeat(40)
            ),
        )?;
        app::import_catalog(
            &database,
            &request(
                path,
                "publisher-conflict",
                &format!("conflict-{name}"),
                "Conflict",
            )?,
        )?;
        let content_uuid =
            sql_query("SELECT content_uuid AS value FROM asset_requirements WHERE asset_name = ?")
                .bind::<Text, _>(name)
                .get_result::<NullableBinaryRow>(&mut connection)?;
        assert!(content_uuid.value.is_none());
        let conflicts = sql_query(
            "SELECT COUNT(*) AS count FROM asset_requirement_content_conflicts AS conflict \
             JOIN asset_requirement_rows AS occurrence \
               USING (set_id, component_order) WHERE occurrence.asset_name = ?",
        )
        .bind::<Text, _>(name)
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(conflicts.count, 1, "candidate conflict for {name}");
    }

    assert_ambiguous_digest_alias(&directory, &database, &mut connection)?;
    Ok(())
}

fn assert_ambiguous_digest_alias(
    directory: &tempfile::TempDir,
    database: &Database,
    connection: &mut SqliteConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let colliding_identity = [0x5a; 16];
    sql_query("INSERT INTO catalog_contents (content_uuid, expected_size) VALUES (?, 16)")
        .bind::<Binary, _>(colliding_identity.as_slice())
        .execute(connection)?;
    sql_query("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES ('sha1', ?)")
        .bind::<Binary, _>([0xaa; 20].as_slice())
        .execute(connection)?;
    let digest_id = sql_query(
        "SELECT digest_id AS value FROM digest_values \
         WHERE algorithm = 'sha1' AND digest = ?",
    )
    .bind::<Binary, _>([0xaa; 20].as_slice())
    .get_result::<IntegerRow>(connection)?
    .value;
    sql_query(
        "INSERT INTO catalog_content_digest_assertions (content_uuid, digest_id, scope) \
         VALUES (?, ?, 'whole_asset')",
    )
    .bind::<Binary, _>(colliding_identity.as_slice())
    .bind::<BigInt, _>(digest_id)
    .execute(connection)?;

    let ambiguous_path = directory.path().join("ambiguous-alias.dat");
    std::fs::write(
        &ambiguous_path,
        format!(
            "<datafile><header><name>Ambiguous</name></header><game name=\"ambiguous\"><rom name=\"ambiguous.bin\" size=\"16\" crc=\"12345678\" sha1=\"{}\"/></game></datafile>",
            "a".repeat(40)
        ),
    )?;
    app::import_catalog(
        database,
        &request(
            ambiguous_path,
            "publisher-ambiguous",
            "ambiguous-alias",
            "Ambiguous",
        )?,
    )?;
    let ambiguous_uuid = sql_query(
        "SELECT content_uuid AS value FROM asset_requirements WHERE asset_name = 'ambiguous.bin'",
    )
    .get_result::<NullableBinaryRow>(connection)?;
    assert!(ambiguous_uuid.value.is_none());
    let conflicts = sql_query(
        "SELECT COUNT(*) AS count FROM asset_requirement_content_conflicts AS conflict \
         JOIN asset_requirement_rows AS occurrence USING (set_id, component_order) \
         WHERE occurrence.asset_name = 'ambiguous.bin' AND conflict.reason = 'ambiguous_alias'",
    )
    .get_result::<CountRow>(connection)?;
    assert_eq!(conflicts.count, 2);
    Ok(())
}

#[test]
fn persists_logiqx_document_and_set_specification_fields_and_diffs_them()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let mut snapshots = Vec::new();
    for (revision, date, year) in [("one", "2026-01-01", "1994"), ("two", "2026-02-01", "1995")] {
        let path = directory.path().join(format!("{revision}.dat"));
        let xml = format!(
            r#"<datafile build="build-id" debug="yes"><header>
              <name>Complete DAT</name><description>Full description</description>
              <version>v1</version><date>{date}</date><author>Author</author>
              <email>author@example.test</email><homepage>https://example.test</homepage>
              <url>https://example.test/dat</url><comment>Comment</comment><category>Games</category>
            </header><file_name>declared.dat</file_name><sha1>0123456789012345678901234567890123456789</sha1>
            <game name="sample" sourcefile="driver.cpp" isbios="no" board="board-a" rebuildto="merged">
              <description>Sample game</description><year>{year}</year><manufacturer>Maker</manufacturer>
            </game></datafile>"#
        );
        std::fs::write(&path, xml)?;
        let import = app::import_catalog(
            &database,
            &request(path, "logiqx-spec", "logiqx-spec", "Logiqx specification")?,
        )?;
        snapshots.push(import.snapshot_key.ok_or("Logiqx snapshot missing")?);
    }

    let document = sql_query(
        "SELECT build, debug, file_name, sha1, header_name, header_description, header_version, \
         header_date, header_author, header_email, header_homepage, header_url, header_comment, \
         header_category FROM logiqx_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshots[0].as_str())
    .get_result::<LogiqxDocumentFactsRow>(&mut connection)?;
    assert_eq!(document.build.as_deref(), Some("build-id"));
    assert_eq!(document.debug.as_deref(), Some("yes"));
    assert!(document.file_name.is_some());
    assert_eq!(
        document.sha1.as_deref(),
        Some(b"\x01#Eg\x89\x01#Eg\x89\x01#Eg\x89\x01#Eg\x89".as_slice())
    );
    assert_eq!(document.header_name, "Complete DAT");
    assert_eq!(
        document.header_description.as_deref(),
        Some("Full description")
    );
    assert_eq!(document.header_version.as_deref(), Some("v1"));
    assert_eq!(document.header_date.as_deref(), Some("2026-01-01"));
    assert_eq!(document.header_author.as_deref(), Some("Author"));
    assert_eq!(
        document.header_email.as_deref(),
        Some("author@example.test")
    );
    assert_eq!(
        document.header_homepage.as_deref(),
        Some("https://example.test")
    );
    assert_eq!(
        document.header_url.as_deref(),
        Some("https://example.test/dat")
    );
    assert_eq!(document.header_comment.as_deref(), Some("Comment"));
    assert_eq!(document.header_category.as_deref(), Some("Games"));

    let set = sql_query(
        "SELECT source_file, is_bios, board, rebuild_to, description, year, manufacturer \
         FROM logiqx_set_facts WHERE snapshot_key = ? AND set_name = 'sample'",
    )
    .bind::<Text, _>(snapshots[0].as_str())
    .get_result::<LogiqxSetFactsRow>(&mut connection)?;
    assert_eq!(set.source_file.as_deref(), Some("driver.cpp"));
    assert_eq!(set.is_bios.as_deref(), Some("no"));
    assert_eq!(set.board.as_deref(), Some("board-a"));
    assert_eq!(set.rebuild_to.as_deref(), Some("merged"));
    assert_eq!(set.description.as_deref(), Some("Sample game"));
    assert_eq!(set.year.as_deref(), Some("1994"));
    assert_eq!(set.manufacturer.as_deref(), Some("Maker"));

    let diff = app::diff_catalog_snapshots(&database, &snapshots[0], &snapshots[1])?;
    assert!(diff.document_metadata_changed);
    assert!(diff.records[0].metadata_changed);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Changed);
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
    let coverage_id = insert_coverage(&mut connection, "unknown")?;
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
         (snapshot_key, catalog_key, document_key, interpretation_key, acquisition_key, coverage_id) \
         VALUES ('identity-only-snapshot', ?, ?, ?, 'legacy-acquisition', ?)",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(&document_key)
    .bind::<Text, _>(interpretation.as_str())
    .bind::<BigInt, _>(coverage_id)
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
    let (directory, database, mut connection) = setup()?;
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
    assert_no_intro_hashes(&mut connection, snapshot.as_str())?;

    assert_no_intro_game_facts(&mut connection, snapshot.as_str())?;

    let scope_members = sql_query(
        "SELECT covered.set_name AS value FROM catalog_snapshots AS snapshots \
         JOIN catalog_coverage AS coverage USING (coverage_id) \
         JOIN catalog_covered_sets AS covered USING (coverage_id) \
         WHERE snapshots.snapshot_key = ? AND coverage.kind = 'filtered' \
           AND covered.set_kind = 'root' AND covered.coverage = 'covered' \
         ORDER BY covered.list_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(
        scope_members
            .into_iter()
            .map(|row| row.value)
            .collect::<Vec<_>>(),
        [
            "Synthetic Cartridge (Japan)".to_owned(),
            "Synthetic Cartridge (World)".to_owned(),
        ]
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

    assert_no_intro_pc_native_metadata(&mut connection, snapshot.as_str())?;

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
        &directory,
        &mut connection,
        snapshot.as_str(),
        request.document_path.as_std_path(),
    )?;

    Ok(())
}

fn assert_no_intro_pc_native_metadata(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> Result<(), diesel::result::Error> {
    let japan_facts = sql_query(
        "SELECT name_alt, region, version, bios_text, languages_present \
         FROM no_intro_game_facts \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<NoIntroPcMetadataRow>(connection)?;
    assert_eq!(
        japan_facts.name_alt.as_deref(),
        Some("合成カートリッジ (日本)")
    );
    assert_eq!(japan_facts.region.as_deref(), Some("Japan"));
    assert_eq!(japan_facts.version.as_deref(), Some("1.0"));
    assert_eq!(japan_facts.bios_text, None);
    assert_eq!(japan_facts.languages_present, 1);
    let japan_languages = sql_query(
        "SELECT languages.language_order, languages.language \
         FROM no_intro_pc_languages AS languages \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'Synthetic Cartridge (Japan)' \
         ORDER BY languages.language_order",
    )
    .bind::<Text, _>(snapshot)
    .load::<NoIntroLanguageRow>(connection)?;
    assert_eq!(
        japan_languages
            .into_iter()
            .map(|row| (row.language_order, row.language))
            .collect::<Vec<_>>(),
        [(0, "Ja".to_owned())]
    );
    let source_line = sql_query(
        "SELECT source_line AS value FROM snapshot_sets \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<IntegerRow>(connection)?;
    assert!(source_line.value > 0);

    let parent_facts = sql_query(
        "SELECT name_alt, region, version, bios_text, languages_present FROM no_intro_game_facts \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (World)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<NoIntroPcMetadataRow>(connection)?;
    assert_eq!(parent_facts.name_alt, None);
    assert_eq!(parent_facts.bios_text.as_deref(), Some("0"));
    assert_eq!(parent_facts.languages_present, 1);
    let parent_clone_marker = sql_query(
        "SELECT COUNT(*) AS count FROM no_intro_pc_clone_markers AS markers \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'Synthetic Cartridge (World)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<CountRow>(connection)?;
    assert_eq!(parent_clone_marker.count, 1);
    let parent_languages = sql_query(
        "SELECT languages.language_order, languages.language \
         FROM no_intro_pc_languages AS languages \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'Synthetic Cartridge (World)' \
         ORDER BY languages.language_order",
    )
    .bind::<Text, _>(snapshot)
    .load::<NoIntroLanguageRow>(connection)?;
    assert_eq!(
        parent_languages
            .into_iter()
            .map(|row| (row.language_order, row.language))
            .collect::<Vec<_>>(),
        [(0, "En".to_owned()), (1, "Ja".to_owned())]
    );
    Ok(())
}

fn assert_no_intro_game_facts(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> Result<(), diesel::result::Error> {
    let facts = sql_query(
        "SELECT archive_id, description FROM no_intro_game_facts \
         WHERE snapshot_key = ? AND set_name = 'Synthetic Cartridge (World)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<NoIntroGameFactsRow>(connection)?;
    assert_eq!(facts.archive_id.as_deref(), Some("1040"));
    assert_eq!(
        facts.description.as_deref(),
        Some("Synthetic Cartridge (World)")
    );
    Ok(())
}

fn assert_no_intro_hashes(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> Result<(), diesel::result::Error> {
    let hashes = sql_query(
        "SELECT crc, md5, sha1 FROM asset_requirements \
         WHERE snapshot_key = ? AND asset_name = 'synthetic-cartridge.bin'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<AssetHashesRow>(connection)?;
    assert_eq!(hashes.crc, Some(vec![0x12, 0x34, 0x56, 0x78]));
    assert_eq!(
        hashes.md5,
        Some(vec![
            0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff
        ])
    );
    assert_eq!(
        hashes.sha1,
        Some(vec![
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
        ])
    );
    Ok(())
}

fn assert_no_intro_retained_document(
    directory: &tempfile::TempDir,
    connection: &mut SqliteConnection,
    snapshot: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let document = sql_query(
        "SELECT document_key AS value FROM documents \
         JOIN catalog_snapshots USING (document_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<TextRow>(connection)?;
    let key = document.value.parse::<DocumentKey>()?;
    assert_eq!(retained_document(directory, &key)?, std::fs::read(path)?);
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
    let clone_reference = sql_query(
        "SELECT links.target_archive_id AS value FROM no_intro_pc_clone_links AS links \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<TextRow>(connection)?;
    assert_eq!(clone_reference.value, "1041");
    let merge_reference = sql_query(
        "SELECT links.target_archive_id AS value FROM no_intro_pc_merge_links AS links \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'Synthetic Cartridge (Japan)'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<TextRow>(connection)?;
    assert_eq!(merge_reference.value, "1042");
    let diagnostics = sql_query(
        "SELECT COUNT(*) AS count FROM import_diagnostics \
         WHERE run_key = ? AND field_name = 'clone' AND code = 'unsupported_attribute'",
    )
    .bind::<Text, _>(run_key)
    .get_result::<CountRow>(connection)?;
    assert_eq!(diagnostics.count, 0);
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
    let coverage_id = insert_coverage(&mut connection, "complete")?;
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
         (snapshot_key, catalog_key, document_key, interpretation_key, declared_version, coverage_id) \
         VALUES ('stale-identity-snapshot', ?, ?, ?, '1.0', ?)",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(&document_key)
    .bind::<Text, _>(interpretation.as_str())
    .bind::<BigInt, _>(coverage_id)
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
fn xml10_forbidden_characters_never_publish_catalogs() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    for (name, forbidden) in [
        ("nul", "\u{0000}"),
        ("control", "\u{0001}"),
        ("vertical-tab", "\u{000b}"),
        ("noncharacter-fffe", "\u{fffe}"),
        ("noncharacter-ffff", "\u{ffff}"),
    ] {
        let path = directory.path().join(format!("{name}.xml"));
        let xml = format!(
            "<datafile>\n<!-- {forbidden} -->\n<game name=\"valid\"><rom name=\"a.bin\"/></game></datafile>"
        );
        std::fs::write(&path, xml.as_bytes())?;
        let mut request = no_intro_request()?;
        request.document_path =
            Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF8 XML fixture path")?;

        let report = app::import_catalog(&database, &request)?;
        assert_eq!(report.status, app::CatalogImportStatus::Failed, "{name}");
        assert!(report.snapshot_key.is_none(), "{name}");
        let diagnostic =
            sql_query("SELECT message AS value FROM import_diagnostics WHERE run_key = ?")
                .bind::<Text, _>(report.run_key.to_string())
                .get_result::<TextRow>(&mut connection)?;
        assert!(diagnostic.value.contains("XML 1.0 forbids"), "{name}");
        let location = sql_query(
            "SELECT source_line, source_column FROM import_diagnostics WHERE run_key = ?",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .get_result::<DiagnosticLocationRow>(&mut connection)?;
        assert!(location.source_line.is_some_and(|line| line > 0), "{name}");
        assert!(
            location.source_column.is_some_and(|column| column > 0),
            "{name}"
        );
    }
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 0);
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

    let machine = sql_query(
        "SELECT source_file, description, year, manufacturer, is_device, runnable, is_bios, \
         is_mechanical, is_consumable FROM mame_machine_facts \
         WHERE snapshot_key = ? AND set_name = 'demo_machine'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<MameMachineFactsRow>(&mut connection)?;
    assert_eq!(machine.source_file.as_deref(), Some("demo.cpp"));
    assert_eq!(machine.description, "Synthetic Machine");
    assert_eq!(machine.year.as_deref(), Some("2000"));
    assert_eq!(machine.manufacturer.as_deref(), Some("Example"));
    assert_eq!(
        (
            machine.is_device,
            machine.runnable,
            machine.is_bios,
            machine.is_mechanical,
            machine.is_consumable
        ),
        (0, 1, 0, 0, 0)
    );
    let bios_sets = sql_query(
        "SELECT name, description, is_default FROM machine_bios_sets \
         WHERE snapshot_key = ? AND set_name = 'demo_machine' ORDER BY bios_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<MachineBiosSetRow>(&mut connection)?;
    assert_eq!(bios_sets.len(), 1);
    assert_eq!(bios_sets[0].name, "demo_bios");
    assert_eq!(bios_sets[0].description.as_deref(), Some("Demo BIOS"));
    assert_eq!(bios_sets[0].is_default, 1);
    let device = sql_query(
        "SELECT source_file, description, year, manufacturer, is_device, runnable, is_bios, \
         is_mechanical, is_consumable FROM mame_machine_facts \
         WHERE snapshot_key = ? AND set_name = 'demo_sound'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<MameMachineFactsRow>(&mut connection)?;
    assert_eq!((device.is_device, device.runnable), (1, 0));

    let assets = sql_query("SELECT GROUP_CONCAT(role || ':' || asset_name, ',') AS value FROM (SELECT role, asset_name FROM asset_requirements WHERE snapshot_key = ? AND set_name = 'demo_machine' ORDER BY component_order)")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        assets.value,
        "rom:demo_bios.bin,rom:demo_video.bin,disk:demo_disk"
    );
    let region = sql_query("SELECT region || ':' || bios AS value FROM mame_asset_facts WHERE snapshot_key = ? AND set_name = 'demo_machine' AND component_order = 0")
        .bind::<Text, _>(snapshot.as_str()).get_result::<TextRow>(&mut connection)?;
    assert_eq!(region.value, "maincpu:demo_bios");
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
    assert!(count(&mut connection, "snapshot_extensions")? >= 1);
    let source = app::load_snapshot_source(&database, &snapshot)?;
    assert!(
        source
            .windows(b"urn:mame:future".len())
            .any(|window| window == b"urn:mame:future")
    );
    assert!(
        source
            .windows(b"<feature".len())
            .any(|window| window == b"<feature")
    );
    Ok(())
}

#[test]
fn imports_mame_switch_specification_fields_as_ordered_query_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("machine-switches.xml");
    std::fs::write(
        &path,
        br#"<mame build="0.289"><machine name="switches"><description>Switch machine</description><dipswitch name="Difficulty" tag=":DSW" mask="0x03"><diplocation name="SW1" number="1A" inverted="yes"/><dipvalue name="Easy" value="0x02"/><dipvalue name="Hard" value="0x01" default="yes"/></dipswitch><configuration name="Video" tag=":CFG" mask="4"><conflocation name="JP1" number="2"/><confsetting name="Raster" value="4" default="yes"/></configuration></machine></mame>"#,
    )?;
    let mut import = request(path, "mame-switches-source", "mame-switches", "Switches")?;
    import.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &import)?;
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;

    let switches = sql_query(
        "SELECT switches.switch_order, switches.kind, switches.name, switches.tag, switches.mask \
         FROM machine_switches AS switches JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'switches' \
         ORDER BY switches.switch_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<MachineSwitchRow>(&mut connection)?;
    assert_eq!(switches.len(), 2);
    assert_eq!(
        (
            switches[0].switch_order,
            switches[0].kind.as_str(),
            switches[0].name.as_str(),
            switches[0].tag.as_str(),
            switches[0].mask
        ),
        (0, "dipswitch", "Difficulty", ":DSW", 3)
    );
    assert_eq!(
        (
            switches[1].switch_order,
            switches[1].kind.as_str(),
            switches[1].name.as_str(),
            switches[1].tag.as_str(),
            switches[1].mask
        ),
        (1, "configuration", "Video", ":CFG", 4)
    );

    let locations = sql_query(
        "SELECT locations.location_order, locations.name, locations.number, locations.inverted \
         FROM machine_switch_locations AS locations JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'switches' \
           AND locations.switch_order = 0 ORDER BY locations.location_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<MachineSwitchLocationRow>(&mut connection)?;
    assert_eq!(locations.len(), 1);
    assert_eq!(
        (
            locations[0].location_order,
            locations[0].name.as_str(),
            locations[0].number.as_str(),
            locations[0].inverted
        ),
        (0, "SW1", "1A", 1)
    );

    let values = sql_query(
        "SELECT switch_values.value_order, switch_values.name, switch_values.value, switch_values.is_default \
         FROM machine_switch_values AS switch_values \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'switches' \
           AND switch_values.switch_order = 0 ORDER BY switch_values.value_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<MachineSwitchValueRow>(&mut connection)?;
    assert_eq!(values.len(), 2);
    assert_eq!(
        (
            values[0].value_order,
            values[0].name.as_str(),
            values[0].value,
            values[0].is_default
        ),
        (0, "Easy", 2, 0)
    );
    assert_eq!(
        (
            values[1].value_order,
            values[1].name.as_str(),
            values[1].value,
            values[1].is_default
        ),
        (1, "Hard", 1, 1)
    );

    let switch_extensions = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions \
         WHERE snapshot_key = ? AND record_kind IN ('dipswitch', 'diplocation', 'dipvalue', 'configuration', 'conflocation', 'confsetting')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(switch_extensions.count, 0);
    Ok(())
}

#[test]
// One end-to-end DTD fixture intentionally exercises every field family in a
// single import so missing persistence is caught at the real storage boundary.
#[allow(clippy::too_many_lines)]
fn imports_every_mame_machine_dtd_family_as_typed_query_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("all-machine-dtd-fields.xml");
    std::fs::write(
        &path,
        br#"<mame build="0.289"><machine name="complete"><description>Complete spec</description>
          <device_ref tag=":cpu" name="cpu_device"/>
          <sample name="sample-set"/><chip name="CPU" tag=":maincpu" type="cpu" clock="4000000"/>
          <chip name="Speaker" type="audio"/><display tag=":screen" type="svg" rotate="90" width="320" height="240" refresh="60.0" pixclock="12000000" htotal="400" hbend="10" hbstart="330" vtotal="260" vbend="5" vbstart="245" flipx="yes"/>
          <sound channels="2"/><input players="2" coins="1" service="yes" tilt="yes"><control type="joy" player="1" buttons="2" minimum="0" maximum="255" sensitivity="50" keydelta="10" reverse="yes" ways="8" ways2="4" ways3="2"/></input>
          <dipswitch name="Mode" tag=":DSW" mask="3"><condition tag=":CFG" mask="1" relation="eq" value="1"/><diplocation name="SW1" number="1"/><dipvalue name="On" value="1"><condition tag=":CFG" mask="1" relation="ne" value="0"/></dipvalue></dipswitch>
          <configuration name="Video" tag=":CFG" mask="4"><condition tag=":CFG" mask="2" relation="gt" value="0"/><conflocation name="JP1" number="2"/><confsetting name="" value="4"><condition tag=":CFG" mask="2" relation="le" value="1"/></confsetting></configuration>
          <port tag=":IN0"><analog mask="255"/></port><adjuster name="Volume" default="80"><condition tag=":CFG" mask="1" relation="lt" value="2"/></adjuster>
          <driver status="imperfect" emulation="good" cocktail="preliminary" savestate="supported" requiresartwork="yes" unofficial="yes" nosoundhardware="yes" incomplete="yes"/>
          <feature type="graphics" status="imperfect" overall="unemulated"/><device type="floppy" tag=":fd" fixed_image="disk" mandatory="yes" interface="floppy"><instance name="floppy1" briefname="FDD"/><extension name="dsk"/><extension name=""/></device>
          <slot name=":cartslot"><slotoption name="game" devname="cart" default="yes"/></slot><softwarelist tag=":software" name="softlist" status="compatible" filter="original"/>
          <ramoption name="ram" default="2M">2M</ramoption>
        </machine><machine name="defaults"><description>Defaults</description>
          <display type="unknown" refresh="0"/><input players="1"><control type="button"/></input>
          <driver status="good" emulation="good" savestate="supported"/><slot name="empty"><slotoption name="default-option" devname="device"/></slot>
        </machine></mame>"#,
    )?;
    let mut import = request(
        path,
        "mame-complete-fields",
        "mame-complete-fields",
        "Complete fields",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &import)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;

    let kinds = sql_query(
        "SELECT GROUP_CONCAT(element_type, ',') AS value FROM (SELECT element_type FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') ORDER BY element_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME element types: {error}")))?;
    assert_eq!(
        kinds.value,
        "sample,chip,chip,display,sound,input,port,adjuster,driver,feature,device,slot,softwarelist,ramoption"
    );

    let display = sql_query(
        "SELECT display_type AS value FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'display'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME display type: {error}")))?;
    assert_eq!(display.value, "svg");
    let chip_fields = sql_query(
        "SELECT GROUP_CONCAT(chip_name || ':' || COALESCE(chip_tag, '') || ':' || chip_type || ':' || COALESCE(chip_clock, ''), ',') AS value FROM (SELECT chip_name, chip_tag, chip_type, chip_clock FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'chip' ORDER BY element_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        chip_fields.value,
        "CPU::maincpu:cpu:4000000,Speaker::audio:"
    );
    let display_fields = sql_query(
        "SELECT display_tag || ':' || display_type || ':' || display_rotate || ':' || flipx || ':' || display_width || ':' || display_height || ':' || display_refresh || ':' || display_pixclock || ':' || display_htotal || ':' || display_hbend || ':' || display_hbstart || ':' || display_vtotal || ':' || display_vbend || ':' || display_vbstart AS value FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'display'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        display_fields.value,
        ":screen:svg:90:1:320:240:60.0:12000000:400:10:330:260:5:245"
    );
    let input_fields = sql_query(
        "SELECT input_service || ':' || input_tilt || ':' || input_players || ':' || input_coins AS value FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'input'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(input_fields.value, "1:1:2:1");
    let controls = sql_query(
        "SELECT control_type || ':' || player || ':' || buttons || ':' || minimum || ':' || maximum || ':' || sensitivity || ':' || keydelta || ':' || reverse || ':' || ways || ':' || ways2 || ':' || ways3 AS value FROM mame_machine_input_controls WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(controls.value, "joy:1:2:0:255:50:10:1:8:4:2");
    let analog = sql_query(
        "SELECT port_tag || ':' || analog.mask AS value FROM mame_machine_spec_elements AS port JOIN mame_machine_analogs AS analog USING (set_id, element_order) WHERE port.set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(analog.value, ":IN0:255");
    let scalar_families = sql_query(
        "SELECT (SELECT sound_channels FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'sound') || ':' || (SELECT adjuster_name || ':' || adjuster_default FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'adjuster') || ':' || (SELECT driver_status || ':' || driver_emulation || ':' || cocktail || ':' || savestate || ':' || requiresartwork || ':' || unofficial || ':' || nosoundhardware || ':' || incomplete FROM (SELECT driver_status, driver_emulation, driver_cocktail AS cocktail, driver_savestate AS savestate, driver_requiresartwork AS requiresartwork, driver_unofficial AS unofficial, driver_nosoundhardware AS nosoundhardware, driver_incomplete AS incomplete FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'driver')) AS value",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        scalar_families.value,
        "2:Volume:80:imperfect:good:preliminary:supported:1:1:1:1"
    );
    let spec_children = sql_query(
        "SELECT (SELECT feature_type || ':' || feature_status || ':' || feature_overall FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'feature') || ':' || (SELECT device_type || ':' || device_tag || ':' || device_fixed_image || ':' || device_mandatory || ':' || device_interface || ':' || device_instance_name || ':' || device_instance_briefname FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'device') || ':' || (SELECT name || ':' || devname || ':' || is_default FROM mame_machine_slot_options WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')) || ':' || (SELECT softwarelist_tag || ':' || softwarelist_name || ':' || softwarelist_status || ':' || softwarelist_filter FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'softwarelist') || ':' || (SELECT ramoption_name || ':' || ramoption_default || ':' || ramoption_text FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'ramoption') AS value",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        spec_children.value,
        "graphics:imperfect:unemulated:floppy::fd:disk:yes:floppy:floppy1:FDD:game:cart:1::software:softlist:compatible:original:ram:2M:2M"
    );
    let device_extension = sql_query(
        "SELECT GROUP_CONCAT(name, ',') AS value FROM (SELECT name FROM mame_machine_device_extensions WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') ORDER BY extension_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(device_extension.value, "dsk,");
    let device_instance_location = sql_query(
        "SELECT device_instance_line > 0 AND device_instance_column > 0 AS value FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'device'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(device_instance_location.value, 1);
    let defaults = sql_query(
        "SELECT flipx AS value FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND element_type = 'display'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME display default: {error}")))?;
    assert_eq!(defaults.value, 1);

    let nested_counts = sql_query(
        "SELECT (SELECT COUNT(*) FROM mame_machine_input_controls WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')) + (SELECT COUNT(*) FROM mame_machine_analogs WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')) + (SELECT COUNT(*) FROM mame_machine_device_extensions WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')) + (SELECT COUNT(*) FROM mame_machine_slot_options WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete')) + (SELECT COUNT(*) FROM mame_machine_conditions AS conditions JOIN snapshot_sets AS sets USING (set_id) WHERE sets.snapshot_key = ? AND sets.set_name = 'complete') AS value",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME nested facts: {error}")))?;
    assert_eq!(nested_counts.value, 1 + 1 + 2 + 1 + 5);
    let empty_extension = sql_query(
        "SELECT COUNT(*) AS value FROM mame_machine_device_extensions WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'complete') AND name = ''",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IntegerRow>(&mut connection)?;
    assert_eq!(empty_extension.value, 1);
    let empty_setting_name = sql_query(
        "SELECT COUNT(*) AS count FROM machine_switch_values AS switch_values \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'complete' AND switch_values.name = ''",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(empty_setting_name.count, 1);
    let conditions = sql_query(
        "SELECT GROUP_CONCAT(owner_kind || ':' || relation || ':' || mask || ':' || value, ',') AS value \
         FROM (SELECT conditions.owner_kind, conditions.relation, conditions.mask, conditions.value \
               FROM mame_machine_conditions AS conditions JOIN snapshot_sets AS sets USING (set_id) \
               WHERE sets.snapshot_key = ? AND sets.set_name = 'complete' \
               ORDER BY conditions.owner_kind, conditions.owner_switch_order, conditions.owner_child_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        conditions.value,
        "element:lt:1:2,switch:eq:1:1,switch:gt:2:0,switch_value:ne:1:0,switch_value:le:2:1"
    );
    let default_values = sql_query(
        "SELECT (SELECT flipx FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'defaults') AND element_type = 'display') || ':' || (SELECT input_service || ':' || input_tilt FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'defaults') AND element_type = 'input') || ':' || (SELECT reverse FROM mame_machine_input_controls WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'defaults')) || ':' || (SELECT driver_requiresartwork || ':' || driver_unofficial || ':' || driver_nosoundhardware || ':' || driver_incomplete FROM mame_machine_spec_elements WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'defaults') AND element_type = 'driver') || ':' || (SELECT is_default FROM mame_machine_slot_options WHERE set_id = (SELECT set_id FROM snapshot_sets WHERE snapshot_key = ? AND set_name = 'defaults')) AS value",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(default_values.value, "0:0:0:0:0:0:0:0:0");

    let reference_tag = sql_query(
        "SELECT dependency.reference_tag AS value FROM mame_machine_dependencies AS dependency \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = 'complete' \
           AND dependency.dependency_kind = 'device_ref'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME dependency tag: {error}")))?;
    assert_eq!(reference_tag.value, ":cpu");

    let opaque_spec_fields = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions WHERE snapshot_key = ? AND record_kind IN ('sample', 'chip', 'display', 'sound', 'input', 'control', 'port', 'analog', 'adjuster', 'driver', 'feature', 'device', 'instance', 'extension', 'slot', 'slotoption', 'softwarelist', 'ramoption')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)
    .map_err(|error| io::Error::other(format!("read MAME extensions: {error}")))?;
    assert_eq!(opaque_spec_fields.count, 0);
    Ok(())
}

#[test]
fn snapshot_diff_detects_mame_machine_specification_and_nested_fact_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    let previous_path = directory.path().join("specification-v1.xml");
    let current_path = directory.path().join("specification-v2.xml");
    let xml = |refresh: &str, control_reverse: &str, reference_tag: &str| {
        format!(
            "<mame><machine name=\"spec\"><description>Spec</description><device_ref tag=\"{reference_tag}\" name=\"sound\"/><display type=\"raster\" refresh=\"{refresh}\"/><input players=\"2\"><control type=\"joy\" reverse=\"{control_reverse}\"/></input></machine></mame>"
        )
    };
    std::fs::write(&previous_path, xml("60", "no", ":sound"))?;
    std::fs::write(&current_path, xml("59.94", "yes", ":sound"))?;
    let mut previous_request = request(
        previous_path,
        "mame-specification-history",
        "mame-specification-history",
        "MAME specification history",
    )?;
    previous_request.format = CatalogDocumentFormat::MameListXml;
    let mut current_request = previous_request.clone();
    current_request.document_path =
        Utf8PathBuf::from_path_buf(current_path).map_err(|_| "non-UTF8 fixture path")?;
    let previous = app::import_catalog(&database, &previous_request)?
        .snapshot_key
        .ok_or("previous MAME snapshot missing")?;
    let current = app::import_catalog(&database, &current_request)?
        .snapshot_key
        .ok_or("current MAME snapshot missing")?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "spec")
        .ok_or("MAME specification diff missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Changed);
    assert!(machine.metadata_changed);

    let tag_path = directory.path().join("specification-tag.xml");
    std::fs::write(&tag_path, xml("60", "no", ":speaker"))?;
    let mut tag_request = previous_request.clone();
    tag_request.document_path =
        Utf8PathBuf::from_path_buf(tag_path).map_err(|_| "non-UTF8 fixture path")?;
    let tag_snapshot = app::import_catalog(&database, &tag_request)?
        .snapshot_key
        .ok_or("tag-only MAME snapshot missing")?;
    let diff = app::diff_catalog_snapshots(&database, &previous, &tag_snapshot)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "spec")
        .ok_or("device reference tag diff missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Changed);
    assert!(machine.metadata_changed);

    let shifted_path = directory.path().join("specification-shifted.xml");
    std::fs::write(&shifted_path, format!("\n{}", xml("60", "no", ":sound")))?;
    let mut shifted_request = previous_request;
    shifted_request.document_path =
        Utf8PathBuf::from_path_buf(shifted_path).map_err(|_| "non-UTF8 fixture path")?;
    let shifted = app::import_catalog(&database, &shifted_request)?
        .snapshot_key
        .ok_or("shifted MAME snapshot missing")?;
    let diff = app::diff_catalog_snapshots(&database, &previous, &shifted)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "spec")
        .ok_or("source-location-only diff missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Unchanged);
    assert!(!machine.metadata_changed);
    Ok(())
}

#[test]
fn mame_rejects_values_outside_the_dtd_and_duplicate_singletons()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    for (index, machine_fields) in [
        "<display type=\"plasma\" refresh=\"60\"/>",
        "<sound channels=\"1\"/><sound channels=\"2\"/>",
        "<input players=\"1\"/><input players=\"2\"/>",
        "<driver status=\"good\" emulation=\"good\" savestate=\"supported\"/><driver status=\"good\" emulation=\"good\" savestate=\"supported\"/>",
    ]
    .into_iter()
    .enumerate()
    {
        let path = directory.path().join(format!("invalid-spec-{index}.xml"));
        std::fs::write(
            &path,
            format!("<mame><machine name=\"invalid\"><description>Invalid</description>{machine_fields}</machine></mame>"),
        )?;
        let mut import = request(
            path,
            &format!("mame-invalid-spec-{index}"),
            &format!("mame-invalid-spec-{index}"),
            "Invalid MAME specification test",
        )?;
        import.format = CatalogDocumentFormat::MameListXml;
        let result = app::import_catalog(&database, &import)?;
        assert!(
            result.snapshot_key.is_none(),
            "accepted invalid MAME DTD fixture {index}"
        );
    }
    Ok(())
}

#[test]
fn snapshot_diff_detects_mame_switch_fact_changes() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    let previous_path = directory.path().join("switches-v1.xml");
    let current_path = directory.path().join("switches-v2.xml");
    let xml = |mask: &str| {
        format!(
            "<mame build=\"0.289\"><machine name=\"switches\"><description>Switch machine</description><dipswitch name=\"Difficulty\" tag=\":DSW\" mask=\"{mask}\"><dipvalue name=\"Easy\" value=\"1\"/></dipswitch></machine></mame>"
        )
    };
    std::fs::write(&previous_path, xml("1"))?;
    std::fs::write(&current_path, xml("3"))?;
    let mut previous_request = request(
        previous_path,
        "mame-switch-history",
        "mame-switch-history",
        "Switch history",
    )?;
    previous_request.format = CatalogDocumentFormat::MameListXml;
    let mut current_request = previous_request.clone();
    current_request.document_path =
        Utf8PathBuf::from_path_buf(current_path).map_err(|_| "non-UTF8 fixture path")?;
    let previous = app::import_catalog(&database, &previous_request)?
        .snapshot_key
        .ok_or("previous snapshot missing")?;
    let current = app::import_catalog(&database, &current_request)?
        .snapshot_key
        .ok_or("current snapshot missing")?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "switches")
        .ok_or("switches diff missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Changed);
    assert!(machine.metadata_changed);
    Ok(())
}

#[test]
fn snapshot_diff_detects_mame_bios_set_fact_changes() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    let previous_path = directory.path().join("bios-v1.xml");
    let current_path = directory.path().join("bios-v2.xml");
    let xml = |description: &str| {
        format!(
            "<mame build=\"0.289\"><machine name=\"bios-machine\"><description>BIOS machine</description><biosset name=\"base\" description=\"{description}\" default=\"yes\"/></machine></mame>"
        )
    };
    std::fs::write(&previous_path, xml("Original"))?;
    std::fs::write(&current_path, xml("Revised"))?;
    let mut previous_request = request(
        previous_path,
        "mame-bios-history",
        "mame-bios-history",
        "BIOS history",
    )?;
    previous_request.format = CatalogDocumentFormat::MameListXml;
    let mut current_request = previous_request.clone();
    current_request.document_path =
        Utf8PathBuf::from_path_buf(current_path).map_err(|_| "non-UTF8 fixture path")?;
    let previous = app::import_catalog(&database, &previous_request)?
        .snapshot_key
        .ok_or("previous snapshot missing")?;
    let current = app::import_catalog(&database, &current_request)?
        .snapshot_key
        .ok_or("current snapshot missing")?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "bios-machine")
        .ok_or("BIOS machine diff missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Changed);
    assert!(machine.metadata_changed);
    Ok(())
}

#[test]
fn snapshot_diff_detects_mame_document_fact_changes() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    let previous_path = directory.path().join("document-v1.xml");
    let current_path = directory.path().join("document-v2.xml");
    let xml = |debug: &str, config| {
        format!(
            "<mame debug=\"{debug}\" mameconfig=\"{config}\"><machine name=\"system\"><description>System</description></machine></mame>"
        )
    };
    std::fs::write(&previous_path, xml("no", 10))?;
    std::fs::write(&current_path, xml("yes", 11))?;
    let mut previous_request = request(
        previous_path,
        "mame-document-history",
        "mame-document-history",
        "MAME document history",
    )?;
    previous_request.format = CatalogDocumentFormat::MameListXml;
    let mut current_request = previous_request.clone();
    current_request.document_path =
        Utf8PathBuf::from_path_buf(current_path).map_err(|_| "non-UTF8 fixture path")?;
    let previous = app::import_catalog(&database, &previous_request)?
        .snapshot_key
        .ok_or("previous MAME snapshot missing")?;
    let current = app::import_catalog(&database, &current_request)?
        .snapshot_key
        .ok_or("current MAME snapshot missing")?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    assert!(diff.document_metadata_changed);
    assert_eq!(diff.records[0].status, SnapshotRecordStatus::Unchanged);
    Ok(())
}

#[test]
fn imports_mame_asset_facts_and_recovers_extensions_from_source()
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

    let document_facts = sql_query(
        "SELECT CAST(debug AS TEXT) || ':' || config_version AS value \
         FROM mame_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(document_facts.value, "1:10");

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
    let source = app::load_snapshot_source(&database, &snapshot)?;
    assert!(
        source
            .windows(b"future:flag".len())
            .any(|window| window == b"future:flag")
    );
    let asset_extension = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         JOIN catalog_sets ON catalog_sets.set_id = snapshot_extensions.owner_set_id \
         WHERE snapshot_key = ? AND record_kind = 'rom' AND record_name = 'clone.rom' \
           AND catalog_sets.set_name = 'clone' AND field_name = 'flag' \
           AND namespace_uri = 'urn:future'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(asset_extension.value, "\"retained\"");

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
fn persists_mame_rom_and_disk_spec_attributes_as_relational_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("mame-asset-facts.xml");
    std::fs::write(
        &path,
        br#"<mame><machine name="facts"><description>Facts</description><rom name="boot.bin" region="maincpu" bios="rev-a" offset="a000" optional="yes" soundonly="no" dispose="yes" loadflag="LOAD16_BYTE" value="0x42" inverted="no" ovha="0x80" nothread="yes"/><disk name="media.chd" region="cdrom" index="2" writeable="yes"/></machine></mame>"#,
    )?;
    let mut request = request(
        path.clone(),
        "mame-asset-facts",
        "mame-asset-facts",
        "MAME facts",
    )?;
    request.format = CatalogDocumentFormat::MameListXml;
    let snapshot = app::import_catalog(&database, &request)?
        .snapshot_key
        .ok_or("MAME snapshot missing")?;

    let rom = sql_query(
        "SELECT region || ':' || bios || ':' || offset || ':' || optional || ':' || sound_only || ':' || \
         dispose || ':' || load_flag || ':' || value || ':' || inverted || ':' || ovha || ':' || no_thread AS value \
         FROM mame_asset_facts WHERE snapshot_key = ? AND set_name = 'facts' AND component_order = 0",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        rom.value,
        "maincpu:rev-a:40960:1:0:1:LOAD16_BYTE:0x42:0:0x80:1"
    );

    let disk = sql_query(
        "SELECT region || ':' || disk_index || ':' || writeable AS value FROM mame_asset_facts \
         WHERE snapshot_key = ? AND set_name = 'facts' AND component_order = 1",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(disk.value, "cdrom:2:1");
    let dtd_defaults = sql_query(
        "SELECT GROUP_CONCAT(role || ':' || dump_status || ':' || optional || ':' || COALESCE(writable, -1), ',') AS value FROM (SELECT asset.role, asset.dump_status, facts.optional, facts.writable FROM asset_requirements AS asset JOIN mame_asset_facts AS facts USING (snapshot_key, set_name, component_order) WHERE asset.snapshot_key = ? AND asset.set_name = 'facts' ORDER BY asset.component_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(dtd_defaults.value, "rom:good:1:-1,disk:good:0:0");

    std::fs::write(
        &path,
        br#"<mame><machine name="facts"><description>Facts</description><rom name="boot.bin" region="graphics" bios="rev-a" offset="a000" optional="yes" soundonly="no" dispose="yes" loadflag="LOAD16_BYTE" value="0x42" inverted="no" ovha="0x80" nothread="yes"/><disk name="media.chd" region="cdrom" index="2" writeable="yes"/></machine></mame>"#,
    )?;
    let changed_snapshot = app::import_catalog(&database, &request)?
        .snapshot_key
        .ok_or("changed MAME snapshot missing")?;
    let diff = app::diff_catalog_snapshots(&database, &snapshot, &changed_snapshot)?;
    let changed = diff
        .records
        .iter()
        .find(|record| record.set_name == "facts")
        .ok_or("MAME asset diff missing")?;
    assert!(
        changed
            .requirement_changes
            .iter()
            .any(|change| { change.asset_name == "boot.bin" && change.other_evidence_changed })
    );
    Ok(())
}

#[test]
fn failed_late_mame_duplicate_does_not_publish_streamed_records()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("late-duplicate-machine.xml");
    let document = br#"<mame xmlns:vendor="urn:vendor">
      <machine name="alpha" cloneof="parent"><description>Alpha</description>
        <rom name="alpha.rom" size="4" crc="12345678"/>
        <vendor:extra mode="preserve">unknown</vendor:extra>
      </machine>
      <machine name="alpha"><description>duplicate</description></machine>
      <machine name="parent"><description>Parent</description><rom name="parent.rom" size="4" crc="12345678"/></machine>
    </mame>"#;
    std::fs::write(&path, document)?;
    let mut import = request(
        path,
        "mame-late-duplicate",
        "mame-late-duplicate",
        "MAME late duplicate",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;

    let failed = app::import_catalog(&database, &import)?;
    assert_eq!(failed.status, app::CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert!(failed.diagnostic_count > 0);
    for table in [
        "catalog_snapshots",
        "snapshot_publications",
        "snapshot_sets",
        "asset_requirements",
        "snapshot_extensions",
        "relationship_assertions",
    ] {
        assert_eq!(
            count(&mut connection, table)?,
            0,
            "unexpected rows in {table}"
        );
    }
    let run_diagnostic = sql_query(
        "SELECT diagnostic AS value FROM import_runs WHERE run_key = ? AND status = 'failed'",
    )
    .bind::<Text, _>(failed.run_key.to_string())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert!(run_diagnostic.value.is_some());
    assert_eq!(
        retained_document_for_run(&directory, &mut connection, &failed.run_key.to_string())?,
        document,
    );
    Ok(())
}

#[test]
fn mame_merge_rom_resolves_when_parent_machine_follows_child()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let mut snapshots = Vec::new();
    for (file, machines) in [
        (
            "parent-first.xml",
            "<machine name=\"parent\"><description>Parent</description><rom name=\"shared.rom\" size=\"4\" crc=\"12345678\"/></machine><machine name=\"child\" romof=\"parent\"><description>Child</description><rom name=\"child.rom\" merge=\"shared.rom\" size=\"4\" crc=\"12345678\"/></machine>",
        ),
        (
            "child-first.xml",
            "<machine name=\"child\" romof=\"parent\"><description>Child</description><rom name=\"child.rom\" merge=\"shared.rom\" size=\"4\" crc=\"12345678\"/></machine><machine name=\"parent\"><description>Parent</description><rom name=\"shared.rom\" size=\"4\" crc=\"12345678\"/></machine>",
        ),
    ] {
        let path = directory.path().join(file);
        std::fs::write(&path, format!("<mame>{machines}</mame>"))?;
        let mut import = request(
            path,
            "mame-merge-order",
            "mame-merge-order",
            "MAME merge order",
        )?;
        import.format = CatalogDocumentFormat::MameListXml;
        let report = app::import_catalog(&database, &import)?;
        snapshots.push(report.snapshot_key.ok_or("MAME snapshot missing")?);
    }
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 2);

    let mut resolved = Vec::new();
    for snapshot in &snapshots {
        let asset = sql_query(
            "SELECT asset_name || ':' || merge_name || ':' || hex(crc) AS value \
             FROM asset_requirements WHERE snapshot_key = ? AND set_name = 'child'",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<TextRow>(&mut connection)
        .map_err(|error| format!("merged asset lookup failed: {error}"))?;
        let relation = sql_query(
            "SELECT source_target_a AS value FROM relationship_assertions \
             WHERE source_snapshot_key = ? AND source_field = 'merge'",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<TextRow>(&mut connection)
        .map_err(|error| format!("merge relationship lookup failed: {error}"))?;
        resolved.push((asset.value, relation.value));
    }
    assert_eq!(
        resolved[0],
        ("child.rom:shared.rom:12345678".into(), "parent".into())
    );
    assert_eq!(resolved[1], resolved[0]);
    Ok(())
}

#[test]
fn mame_forward_merges_resolve_across_asset_pagination() -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write as _;

    const ASSET_COUNT: usize = 300;

    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("forward-merges-across-pages.xml");
    let mut document = String::from(
        "<mame><machine name=\"child\" romof=\"parent\"><description>Child</description>",
    );
    for index in 0..ASSET_COUNT {
        write!(
            document,
            "<rom name=\"child-{index:03}.rom\" merge=\"parent-{index:03}.rom\" size=\"4\" crc=\"12345678\"/>"
        )?;
    }
    document.push_str("</machine><machine name=\"parent\"><description>Parent</description>");
    for index in 0..ASSET_COUNT {
        write!(
            document,
            "<rom name=\"parent-{index:03}.rom\" size=\"4\" crc=\"12345678\"/>"
        )?;
    }
    document.push_str("</machine></mame>");
    std::fs::write(&path, document)?;

    let mut import = request(
        path,
        "mame-paginated-merges",
        "mame-paginated-merges",
        "MAME paginated merges",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;
    let report = app::import_catalog(&database, &import)?;
    let snapshot = report.snapshot_key.ok_or("MAME snapshot missing")?;

    let assertion_counts = sql_query(
        "SELECT COUNT(*) || ':' || \
                COUNT(DISTINCT source_subject_a || ':' || source_subject_b || ':' || source_subject_c) || ':' || \
                COUNT(DISTINCT source_target_a || ':' || source_target_b || ':' || source_target_c) AS value \
         FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND relation_type = 'exact_content_identity' \
           AND source_field = 'merge'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(
        assertion_counts.value,
        format!("{ASSET_COUNT}:{ASSET_COUNT}:{ASSET_COUNT}")
    );
    Ok(())
}

#[test]
fn mame_asset_storage_error_rolls_back_staged_import_and_keeps_document()
-> Result<(), Box<dyn std::error::Error>> {
    use diesel::connection::SimpleConnection;

    let (directory, database, mut connection) = setup()?;
    connection.batch_execute(
        "CREATE TRIGGER reject_asset_insert BEFORE INSERT ON mame_rom_claims \
         BEGIN SELECT RAISE(ABORT, 'injected asset insert failure'); END;",
    )?;
    let path = directory.path().join("asset-insert-storage-error.xml");
    let document = br#"<mame xmlns:vendor="urn:rollback" vendor:flag="staged"><machine name="set"><description>Set</description><rom name="set.rom" size="4" crc="12345678"/></machine></mame>"#;
    std::fs::write(&path, document)?;
    let mut import = request(
        path,
        "mame-storage-error",
        "mame-storage-error",
        "MAME storage error",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;

    let error = match app::import_catalog(&database, &import) {
        Ok(report) => {
            return Err(io::Error::other(format!(
                "injected SQLite failure returned import report with status {}",
                report.status.as_str()
            ))
            .into());
        }
        Err(error) => error,
    };
    assert!(matches!(
        &error,
        mame_coalesce::Error::Diesel(diesel::result::Error::DatabaseError(..))
    ));
    assert!(error.to_string().contains("injected asset insert failure"));

    for table in [
        "catalog_snapshots",
        "snapshot_publications",
        "snapshot_sets",
        "asset_requirements",
        "record_namespaces",
        "records",
        "asset_occurrences",
        "mame_rom_claims",
        "snapshot_extensions",
        "relationship_assertions",
        "import_runs",
        "import_diagnostics",
    ] {
        assert_eq!(
            count(&mut connection, table)?,
            0,
            "unexpected rows in {table}"
        );
    }
    assert_eq!(count(&mut connection, "documents")?, 1);
    let document_key = sql_query("SELECT document_key AS value FROM documents")
        .get_result::<TextRow>(&mut connection)?
        .value
        .parse::<DocumentKey>()?;
    // The injected trigger is not part of the authoritative schema. Remove it
    // before reopening the store to verify the independently retained source.
    connection.batch_execute("DROP TRIGGER reject_asset_insert;")?;
    assert_eq!(retained_document(&directory, &document_key)?, document);
    Ok(())
}

#[test]
fn mame_nested_unknown_extension_and_reimport_keep_semantics_and_diagnostics()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("nested-extension.xml");
    std::fs::write(
        &path,
        br#"<mame xmlns:v="urn:vendor"><machine name="alpha"><description>Alpha</description><v:payload code="A &amp; B">left &lt;middle&gt;<v:part code="x">inside &amp; out</v:part> right</v:payload></machine></mame>"#,
    )?;
    let mut import = request(
        path,
        "mame-nested-extension",
        "mame-nested-extension",
        "MAME nested extension",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;

    let first = app::import_catalog(&database, &import)?;
    let snapshot = first.snapshot_key.ok_or("MAME snapshot missing")?;
    assert_eq!(
        first.diagnostic_count, 0,
        "unknown source XML is not a failed import"
    );
    let first_diagnostics = sql_query(
        "SELECT code, message, record_kind, record_name, field_name, raw_value_json, source_line, source_column \
         FROM import_diagnostics WHERE run_key = ? \
         ORDER BY code, message, record_kind, record_name, field_name, source_line, source_column",
    )
    .bind::<Text, _>(first.run_key.to_string())
    .load::<ImportDiagnosticRow>(&mut connection)?;
    assert!(first_diagnostics.is_empty());

    let extension = sql_query(
        "SELECT field_name, namespace_uri, raw_value_json AS value FROM snapshot_extensions \
         WHERE snapshot_key = ? AND record_kind = 'machine' AND record_name = 'alpha' \
           AND field_name = 'element:payload'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<StoredExtensionRow>(&mut connection)?;
    assert_eq!(extension.field_name, "element:payload");
    assert_eq!(extension.namespace_uri.as_deref(), Some("urn:vendor"));
    let extension_json: serde_json::Value = serde_json::from_str(&extension.value)?;
    assert_eq!(extension_json["name"], "{urn:vendor}payload");
    assert_eq!(extension_json["attributes"]["code"], "A & B");
    assert_eq!(
        extension_json["content"],
        serde_json::json!([
            {"kind": "text", "value": "left "},
            {"kind": "text", "value": "<"},
            {"kind": "text", "value": "middle"},
            {"kind": "text", "value": ">"},
            {"kind": "element", "value": {
                "name": "{urn:vendor}part", "attributes": {"code": "x"},
                "content": [
                    {"kind": "text", "value": "inside "},
                    {"kind": "text", "value": "&"},
                    {"kind": "text", "value": " out"}
                ]
            }},
            {"kind": "text", "value": " right"}
        ])
    );

    let repeated = app::import_catalog(&database, &import)?;
    assert_eq!(repeated.snapshot_key.as_ref(), Some(&snapshot));
    assert_eq!(repeated.diagnostic_count, 0);
    assert_eq!(count(&mut connection, "catalog_snapshots")?, 1);
    let repeated_diagnostics = sql_query(
        "SELECT code, message, record_kind, record_name, field_name, raw_value_json, source_line, source_column \
         FROM import_diagnostics WHERE run_key = ? \
         ORDER BY code, message, record_kind, record_name, field_name, source_line, source_column",
    )
    .bind::<Text, _>(repeated.run_key.to_string())
    .load::<ImportDiagnosticRow>(&mut connection)?;
    assert!(repeated_diagnostics.is_empty());
    Ok(())
}

#[test]
fn mame_relationships_resolve_component_keys_and_keep_device_locations()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("mame-relationship-identity.xml");
    std::fs::write(
        &path,
        "<mame>\n<machine name=\"clone-parent\"><description>Clone parent</description></machine>\n<machine name=\"rom-parent\">\n<description>ROM parent</description><rom name=\"shared.bin\" size=\"1\" crc=\"12345678\"/>\n</machine>\n<machine name=\"clone\" cloneof=\"clone-parent\" romof=\"rom-parent\">\n<description>Clone</description><device_ref tag=\":sound\" name=\"sound\"/>\n<rom name=\"shared.bin\" merge=\"shared.bin\" size=\"1\" crc=\"12345678\"/>\n</machine>\n<machine name=\"sound\"><description>Sound</description></machine>\n</mame>",
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

    assert_mame_merge_relationships(&database, &mut connection, &snapshot)?;
    review_mame_device_relationship(&database)?;
    assert_mame_dependency_projections(&mut connection, &snapshot)?;
    assert_mame_relationship_invariants(&mut connection, &snapshot)?;
    Ok(())
}

fn assert_mame_merge_relationships(
    database: &Database,
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let merge = sql_query(
        "SELECT source_subject_a AS subject_key, source_target_a AS target_key, source_line FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'merge'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RelationshipKeyLocationRow>(connection)?;
    assert_eq!(merge.subject_key, "clone");
    assert_eq!(merge.target_key, "rom-parent");
    assert_eq!(merge.source_line, 8);

    let stored_merge = sql_query(
        "SELECT source_subject_a, source_subject_b, source_subject_c, \
                source_target_a, source_target_b, source_target_c \
         FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'merge'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TypedSourceAssertionRow>(connection)?;
    assert_eq!(stored_merge.subject_a.as_deref(), Some("clone"));
    assert_eq!(stored_merge.subject_b.as_deref(), Some("shared.bin"));
    assert_eq!(stored_merge.subject_c, Some(0));
    assert_eq!(stored_merge.target_a.as_deref(), Some("rom-parent"));
    assert_eq!(stored_merge.target_b.as_deref(), Some("shared.bin"));
    assert_eq!(stored_merge.target_c, Some(0));

    let merge_explanation = app::explain_relationships(database)?
        .into_iter()
        .find(|explanation| explanation.source_field.as_deref() == Some("merge"))
        .ok_or("typed merge assertion was not explainable")?;
    assert_eq!(
        merge_explanation.claim.evidence["declared_merge_name"],
        "shared.bin"
    );
    assert_eq!(
        merge_explanation.claim.evidence["parent_set_name"],
        "rom-parent"
    );
    assert_eq!(merge_explanation.claim.evidence["expected_crc"], "12345678");
    assert_eq!(merge_explanation.claim.evidence["size"], 1);
    Ok(())
}

fn review_mame_device_relationship(database: &Database) -> Result<(), Box<dyn std::error::Error>> {
    let device_explanation = app::explain_relationships(database)?
        .into_iter()
        .find(|explanation| explanation.source_field.as_deref() == Some("device_ref"))
        .ok_or("typed device dependency was not explainable")?;
    assert_eq!(
        device_explanation.claim.relation_type,
        RelationshipType::RuntimeDependency
    );
    assert_eq!(
        device_explanation
            .source_location
            .map(|location| location.line),
        Some(7)
    );
    let device_assertion_key = device_explanation.assertion_key;
    app::review_relationship(
        database,
        &device_assertion_key,
        &mame_coalesce::domain::RelationshipReview {
            decision: mame_coalesce::domain::RelationshipReviewDecision::Accepted,
            note: "typed dependency review remains available".to_owned(),
            superseded_by: None,
        },
    )?;
    let reviewed_device = app::explain_relationships(database)?
        .into_iter()
        .find(|explanation| explanation.assertion_key == device_assertion_key)
        .ok_or("reviewed typed dependency was not explainable")?;
    assert!(reviewed_device.latest_review.is_some());
    Ok(())
}

fn assert_mame_dependency_projections(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let device = sql_query(
        "SELECT source_subject_a AS subject_key, source_target_a AS target_key, source_line FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND source_field = 'device_ref'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RelationshipKeyLocationRow>(connection)?;
    assert_eq!(device.subject_key, "clone");
    assert_eq!(device.target_key, "sound");
    assert_eq!(device.source_line, 7);

    let dependencies = sql_query(
        "SELECT source_field || ':' || source_target_a AS value FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND subject_kind = 'catalog_set' AND source_subject_a = 'clone' \
         AND relation_type = 'runtime_dependency' ORDER BY source_field",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<TextRow>(connection)?;
    assert_eq!(
        dependencies
            .iter()
            .map(|dependency| dependency.value.as_str())
            .collect::<Vec<_>>(),
        ["device_ref:sound", "romof:rom-parent"]
    );

    Ok(())
}

fn assert_mame_relationship_invariants(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let duplicate_runtime_assertions = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND relation_type = 'runtime_dependency'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(connection)?;
    assert_eq!(duplicate_runtime_assertions.count, 0);
    Ok(())
}

#[test]
fn resolves_machine_runtime_closure_without_traversing_clone_ancestry()
-> Result<(), Box<dyn std::error::Error>> {
    use mame_coalesce::machine_dependencies::{DependencyDiagnostic, MachineDependencyKind};

    let (directory, database, mut connection) = setup()?;
    let document = directory.path().join("machine-dependencies.xml");
    std::fs::write(
        &document,
        br#"<mame build="fixture">
          <machine name="game" cloneof="parent" romof="bios" sampleof="samples">
            <description>Game</description>
            <device_ref tag=":sound" name="sound"/>
          </machine>
          <machine name="bios" isbios="yes"><description>BIOS</description></machine>
          <machine name="sound" isdevice="yes"><description>Sound</description></machine>
          <machine name="parent"><description>Parent</description></machine>
          <machine name="samples"><description>Samples</description></machine>
        </mame>"#,
    )?;
    let mut import = request(
        document,
        "mame-fixture",
        "machine-dependencies",
        "Machine dependencies",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;
    import.scope = CatalogScope::Complete;
    let imported = app::import_catalog(&database, &import)?;
    let snapshot = imported.snapshot_key.ok_or("machine snapshot missing")?;

    let closure = app::resolve_machine_dependencies(
        &database,
        &snapshot,
        &mame_coalesce::domain::SetName::new("game"),
    )?;
    assert_eq!(
        closure
            .sets
            .iter()
            .map(mame_coalesce::domain::SetName::as_str)
            .collect::<Vec<_>>(),
        ["bios", "game", "sound"]
    );
    assert!(closure.edges.iter().any(|edge| {
        edge.kind == MachineDependencyKind::RomOf
            && edge.to.as_str() == "bios"
            && edge.target_is_bios
    }));
    assert!(closure.edges.iter().any(|edge| {
        edge.kind == MachineDependencyKind::DeviceReference
            && edge.to.as_str() == "sound"
            && edge.target_is_device
    }));
    assert!(
        closure
            .diagnostics
            .contains(&DependencyDiagnostic::UnsupportedRelationship {
                set: mame_coalesce::domain::SetName::new("game"),
                field: "sampleof".into(),
                target: mame_coalesce::domain::SetName::new("samples"),
            })
    );
    assert!(
        !closure
            .sets
            .contains(&mame_coalesce::domain::SetName::new("parent"))
    );

    let source_assertions = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND source_subject_a = 'game' \
           AND source_field IN ('cloneof', 'romof', 'device_ref', 'sampleof')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(source_assertions.count, 4);
    let persisted_runtime_assertions = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field IN ('romof', 'device_ref', 'sampleof')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(persisted_runtime_assertions.count, 0);
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
        "SELECT source_subject_a || ':' || source_subject_b AS value FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND source_field = 'cloneof' \
         ORDER BY source_subject_a, source_subject_b",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0].value, "a:b/c");
    assert_eq!(keys[1].value, "a/b:c");
    Ok(())
}

#[test]
fn machine_dependency_loader_rejects_software_list_snapshots()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, _) = setup()?;
    let requests = [
        mame_softwarelist_request()?,
        no_intro_request()?,
        clrmamepro_request()?,
    ];
    for request in requests {
        let format = request.format.as_str();
        let imported = app::import_catalog(&database, &request)?;
        let snapshot = imported.snapshot_key.ok_or("catalog snapshot missing")?;
        let result = app::resolve_machine_dependencies(
            &database,
            &snapshot,
            &mame_coalesce::domain::SetName::new("game"),
        );
        assert!(matches!(
            result,
            Err(mame_coalesce::Error::UnsupportedMachineDependencyFormat(actual))
                if actual == format
        ));
    }
    Ok(())
}

#[test]
fn machine_dependency_loader_accepts_logiqx_set_snapshots() -> Result<(), Box<dyn std::error::Error>>
{
    let (_directory, database, _) = setup()?;
    let mut request = request(
        fixture("catalog-a-v1.dat"),
        "logiqx-machine",
        "logiqx-machine-catalog",
        "Logiqx machine fixture",
    )?;
    request.scope = CatalogScope::Complete;
    let imported = app::import_catalog(&database, &request)?;
    let snapshot = imported.snapshot_key.ok_or("Logiqx snapshot missing")?;
    let closure = app::resolve_machine_dependencies(
        &database,
        &snapshot,
        &mame_coalesce::domain::SetName::new("alpha"),
    )?;
    assert_eq!(closure.root.as_str(), "alpha");
    Ok(())
}

#[test]
fn dependency_absence_respects_filtered_snapshot_scope() -> Result<(), Box<dyn std::error::Error>> {
    use mame_coalesce::machine_dependencies::DependencyDiagnostic;

    let (directory, database, _) = setup()?;
    let document = directory.path().join("filtered-machine.xml");
    std::fs::write(
        &document,
        br#"<mame build="fixture"><machine name="root" romof="outside"><description>Root</description></machine></mame>"#,
    )?;
    let mut import = request(
        document,
        "mame-filtered",
        "filtered-machines",
        "Filtered machines",
    )?;
    import.format = CatalogDocumentFormat::MameListXml;
    import.scope = filtered_root_scope(&["root"]);
    let imported = app::import_catalog(&database, &import)?;
    let snapshot = imported.snapshot_key.ok_or("filtered snapshot missing")?;
    let closure = app::resolve_machine_dependencies(
        &database,
        &snapshot,
        &mame_coalesce::domain::SetName::new("root"),
    )?;
    assert!(
        closure
            .diagnostics
            .contains(&DependencyDiagnostic::OutOfScopeSet {
                name: mame_coalesce::domain::SetName::new("outside"),
                required_by: Some(mame_coalesce::domain::SetName::new("root")),
            })
    );
    assert_eq!(
        closure.completeness,
        mame_coalesce::machine_dependencies::SnapshotCompleteness::Filtered(Some(
            std::collections::BTreeSet::from([mame_coalesce::domain::SetName::new("root")])
        ))
    );
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
    assert_eq!(count(&mut connection, "software_item_info")?, 4);
    assert_eq!(count(&mut connection, "software_item_shared_features")?, 1);
    assert_eq!(count(&mut connection, "software_part_features")?, 1);
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
fn software_list_occurrences_share_binary_content_identity_without_collapsing_rows()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, database, mut connection) = setup()?;
    let first_request = mame_softwarelist_request()?;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = mame_softwarelist_request()?;
    second_request.catalog_key = CatalogKey::new("second-software-list-catalog");
    second_request.catalog_display_name = "Second software-list catalog".to_owned();
    let second = app::import_catalog(&database, &second_request)?;
    assert_ne!(first.snapshot_key, second.snapshot_key);

    let shared_content = sql_query(
        "SELECT COUNT(*) AS occurrences, COUNT(content_uuid) AS linked_occurrences, \
         COUNT(DISTINCT content_uuid) AS identities, MIN(length(content_uuid)) AS minimum_uuid_bytes, \
         MIN(typeof(content_uuid)) AS storage_class FROM software_components \
         WHERE sha1 = ? AND evidence_scope = 'whole_asset'",
    )
    .bind::<Binary, _>(
        hex::decode("0123456789abcdef0123456789abcdef01234567")?.as_slice(),
    )
    .get_result::<SharedContentStats>(&mut connection)?;
    assert_eq!(shared_content.occurrences, 2);
    assert_eq!(shared_content.linked_occurrences, 2);
    assert_eq!(shared_content.identities, 1);
    assert_eq!(shared_content.minimum_uuid_bytes, Some(16));
    assert_eq!(shared_content.storage_class.as_deref(), Some("blob"));

    let names = sql_query(
        "SELECT component_name AS value FROM software_components \
         WHERE sha1 = ? ORDER BY snapshot_key",
    )
    .bind::<Binary, _>(hex::decode("0123456789abcdef0123456789abcdef01234567")?.as_slice())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(names.len(), 2);
    assert_eq!(names[0].value, "program.bin");
    assert_eq!(names[1].value, "program.bin");
    Ok(())
}

#[test]
fn operation_only_software_rom_digest_is_not_promoted_to_file_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("software-list-operation-digest.xml");
    std::fs::write(
        &path,
        r#"<softwarelists><softwarelist name="operations"><software name="sample">
          <description>Sample</description><year>2000</year><publisher>Example</publisher>
          <part name="cart" interface="cart"><dataarea name="rom" size="32">
            <rom name="program.bin" size="16" sha1="0123456789abcdef0123456789abcdef01234567" loadflag="load16_word"/>
            <rom name="program.bin" size="16" sha1="0123456789abcdef0123456789abcdef01234567" loadflag="continue"/>
          </dataarea></part></software></softwarelist></softwarelists>"#,
    )?;
    let mut request = request(
        path,
        "publisher-operations",
        "operation-digests",
        "Operations",
    )?;
    request.format = CatalogDocumentFormat::MameSoftwareListXml;
    app::import_catalog(&database, &request)?;

    let continuation_identity = sql_query(
        "SELECT content_uuid AS value FROM software_components \
         WHERE load_instruction = 'continue'",
    )
    .get_result::<NullableBinaryRow>(&mut connection)?;
    assert!(continuation_identity.value.is_none());
    assert_eq!(
        sql_query("SELECT COUNT(*) AS count FROM software_components WHERE sha1 IS NOT NULL")
            .get_result::<CountRow>(&mut connection)?
            .count,
        2,
        "operation digest source facts remain queryable"
    );
    assert_eq!(count(&mut connection, "catalog_contents")?, 1);
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM occurrence_digest_assertions AS assertion \
             JOIN software_components USING (occurrence_id) \
             WHERE assertion.scope = 'whole_asset' AND assertion.provenance = 'source_declared'",
        )
        .get_result::<CountRow>(&mut connection)?
        .count,
        2,
        "operation digests remain attached to their own source occurrences"
    );
    Ok(())
}

#[test]
fn interned_digest_bytes_keep_occurrence_scope_and_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let digest = "0123456789abcdef0123456789abcdef01234567";
    let path = directory.path().join("same-digest-different-scopes.xml");
    std::fs::write(
        &path,
        format!(
            "<softwarelist name=\"scopes\"><software name=\"item\"><description>Item</description><year>2000</year><publisher>Example</publisher><part name=\"cart\" interface=\"cart\"><dataarea name=\"rom\" size=\"1\"><rom name=\"program.bin\" size=\"1\" sha1=\"{digest}\"/></dataarea><diskarea name=\"media\"><disk name=\"media.chd\" sha1=\"{digest}\"/></diskarea></part></software></softwarelist>"
        ),
    )?;
    let mut import = request(path, "publisher-scopes", "digest-scopes", "Digest scopes")?;
    import.format = CatalogDocumentFormat::MameSoftwareListXml;
    app::import_catalog(&database, &import)?;
    let digest_bytes = hex::decode(digest)?;

    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM digest_values WHERE algorithm = 'sha1' AND digest = ?"
        )
        .bind::<Binary, _>(digest_bytes.as_slice())
        .get_result::<CountRow>(&mut connection)?
        .count,
        1,
        "identical algorithm/bytes are stored once regardless of scope"
    );
    let scopes = sql_query(
        "SELECT assertion.scope AS value FROM occurrence_digest_assertions AS assertion \
         JOIN software_components USING (occurrence_id) \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE digest.algorithm = 'sha1' AND digest.digest = ? ORDER BY assertion.scope",
    )
    .bind::<Binary, _>(digest_bytes.as_slice())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(
        scopes
            .iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>(),
        ["chd_header_sha1", "whole_asset"]
    );
    let digest_owners = sql_query(
        "SELECT DISTINCT assertion.provenance AS value FROM occurrence_digest_assertions AS assertion \
         JOIN software_components USING (occurrence_id) \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE digest.algorithm = 'sha1' AND digest.digest = ?",
    )
    .bind::<Binary, _>(digest_bytes.as_slice())
    .load::<TextRow>(&mut connection)?;
    assert_eq!(digest_owners.len(), 1);
    assert_eq!(digest_owners[0].value, "source_declared");
    let disk_identity = sql_query(
        "SELECT content_uuid AS value FROM software_components WHERE component_kind = 'disk'",
    )
    .get_result::<NullableBinaryRow>(&mut connection)?;
    assert!(disk_identity.value.is_none());
    assert_eq!(count(&mut connection, "catalog_contents")?, 1);
    Ok(())
}

#[test]
fn persists_softwarelist_dtd_notes_dipswitches_and_defaults_relationally()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let path = directory.path().join("softwarelist-spec-facts.xml");
    std::fs::write(
        &path,
        br#"<softwarelist name="list"><notes>list notes</notes><software name="game"><description>Game</description><year>2000</year><publisher>Pub</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="game.bin"/></dataarea><diskarea name="media"><disk name="game.chd"/></diskarea><dipswitch name="Difficulty" tag=":DSW" mask="0x03"><dipvalue name="Easy" value="0x01" default="yes"/><dipvalue name="Hard" value="0x02"/></dipswitch></part></software></softwarelist>"#,
    )?;
    let mut request = request(
        path,
        "softwarelist-spec-facts",
        "softwarelist-spec-facts",
        "Software-list spec facts",
    )?;
    request.format = CatalogDocumentFormat::MameSoftwareListXml;
    let snapshot = app::import_catalog(&database, &request)?
        .snapshot_key
        .ok_or("software-list snapshot missing")?;

    let notes = sql_query(
        "SELECT software_lists.notes AS value FROM software_lists \
         JOIN record_namespaces USING (namespace_id) \
         WHERE record_namespaces.snapshot_key = ? AND record_namespaces.source_name = 'list'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(notes.value, "list notes");
    let defaults = sql_query(
        "SELECT software_items.supported || ':' || software_areas.width || ':' || software_areas.endianness AS value \
         FROM software_items \
         JOIN records USING (record_id) \
         JOIN record_namespaces USING (namespace_id) \
         JOIN software_areas USING (record_id) \
         WHERE record_namespaces.snapshot_key = ? AND record_namespaces.source_name = 'list' \
         AND records.source_name = 'game' AND software_areas.area_order = 0",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(defaults.value, "yes:8:little");
    let components = sql_query(
        "SELECT GROUP_CONCAT(component_kind || ':' || dump_status || ':' || COALESCE(writeable, -1), ',') AS value \
         FROM (SELECT component_kind, dump_status, writeable FROM software_components \
               WHERE snapshot_key = ? AND list_name = 'list' ORDER BY component_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(components.value, "rom:good:-1,disk:good:0");
    let switch = sql_query(
        "SELECT dipswitch.name || ':' || dipswitch.tag || ':' || dipswitch.mask || ':' || \
         GROUP_CONCAT(value.name || '=' || value.value || ':' || value.is_default, ',') AS value \
         FROM software_part_dipswitches AS dipswitch JOIN software_part_dip_values AS value \
         USING (part_id, dipswitch_order) \
         JOIN software_parts USING (part_id) \
         JOIN records USING (record_id) \
         JOIN record_namespaces USING (namespace_id) \
         WHERE record_namespaces.snapshot_key = ? AND record_namespaces.source_name = 'list' \
         AND records.source_name = 'game' \
         GROUP BY dipswitch.part_id, dipswitch.dipswitch_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(switch.value, "Difficulty::DSW:0x03:Easy=0x01:1,Hard=0x02:0");
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
    let supported = sql_query(
        "SELECT software_items.supported AS value FROM software_items \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE record_namespaces.snapshot_key = ? AND record_namespaces.source_name = 'one' \
         AND records.source_name = 'game'",
    )
    .bind::<Text, _>(
        report
            .snapshot_key
            .as_ref()
            .ok_or("software-list snapshot missing")?
            .as_str(),
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(supported.value.as_deref(), Some("yes"));
    Ok(())
}

#[test]
fn imports_repeated_area_names_and_materializes_component_defaults()
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
    assert_eq!(absent.value, Some(0));
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
    let area_size = sql_query(
        "SELECT software_areas.declared_size AS value FROM software_areas \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE record_namespaces.snapshot_key = ? AND record_namespaces.source_name = 'one' \
         AND records.source_name = 'game' AND software_areas.area_kind = 'data'",
    )
    .bind::<Text, _>(
        report
            .snapshot_key
            .as_ref()
            .ok_or("software-list snapshot missing")?
            .as_str(),
    )
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
         FROM (SELECT namespaces.source_name AS list_name FROM software_lists \
               JOIN record_namespaces AS namespaces USING (namespace_id) \
               WHERE namespaces.snapshot_key = ? ORDER BY namespaces.source_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(list_order.value, "demo_cart,demo_flop");

    let item_order = sql_query(
        "SELECT GROUP_CONCAT(item_name, ',') AS value FROM ( \
         SELECT records.source_name AS item_name FROM software_items \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         ORDER BY records.source_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(item_order.value, "demo_game,demo_original");

    let scoped_duplicates = sql_query(
        "SELECT COUNT(*) AS count FROM software_items \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND records.source_name = 'demo_game'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(connection)?;
    assert_eq!(scoped_duplicates.count, 2);

    let dependency = sql_query(
        "SELECT dependency.target_name AS value FROM software_item_dependencies AS dependency \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' \
         AND dependency.dependency_kind = 'clone_of'",
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
    let item_line = assert_software_list_fields(connection, snapshot)?;
    assert_software_list_parts(connection, snapshot, item_line)
}

fn assert_software_list_fields(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
) -> Result<i64, Box<dyn std::error::Error>> {
    let item = sql_query(
        "SELECT software_items.supported, records.source_line \
         FROM software_items JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareItemRow>(connection)?;
    assert_eq!(item.supported, "partial");
    let info = sql_query(
        "SELECT info.value_order, info.name, info.value, info.source_line, info.source_column \
         FROM software_item_info AS info JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' ORDER BY info.value_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SoftwareNamedValueRow>(connection)?;
    assert_eq!(
        info.iter()
            .map(|row| (row.value_order, row.name.as_str(), row.value.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (0, "serial", Some("SYN-001")),
            (1, "language", Some("English")),
            (2, "language", Some("French")),
        ]
    );
    assert_eq!((info[0].source_line, info[0].source_column), (9, 7));
    assert_eq!((info[2].source_line, info[2].source_column), (11, 7));

    let shared_feature = sql_query(
        "SELECT feature.value_order, feature.name, feature.value, feature.source_line, feature.source_column \
         FROM software_item_shared_features AS feature JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' ORDER BY feature.value_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SoftwareNamedValueRow>(connection)?;
    assert_eq!(shared_feature.len(), 1);
    assert_eq!(shared_feature[0].value_order, 0);
    assert_eq!(shared_feature[0].name, "compatibility");
    assert_eq!(shared_feature[0].value.as_deref(), Some("PAL"));
    assert_eq!(
        (
            shared_feature[0].source_line,
            shared_feature[0].source_column
        ),
        (12, 7)
    );

    let part_feature = sql_query(
        "SELECT feature.value_order, feature.name, feature.value, feature.source_line, feature.source_column \
         FROM software_part_features AS feature JOIN software_parts USING (part_id) \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' AND software_parts.part_name = 'cart' \
         ORDER BY feature.value_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SoftwareNamedValueRow>(connection)?;
    assert_eq!(part_feature.len(), 1);
    assert_eq!(part_feature[0].value_order, 0);
    assert_eq!(part_feature[0].name, "pcb");
    assert_eq!(part_feature[0].value.as_deref(), Some("standard"));
    assert_eq!(
        (part_feature[0].source_line, part_feature[0].source_column),
        (14, 9)
    );

    let absent_value = sql_query(
        "SELECT info.value_order, info.name, info.value, info.source_line, info.source_column \
         FROM software_item_info AS info JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_original' ORDER BY info.value_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareNamedValueRow>(connection)?;
    assert_eq!(absent_value.value_order, 0);
    assert_eq!(absent_value.name, "region");
    assert_eq!(absent_value.value, None);

    let removed_columns = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('software_items') \
         WHERE name IN ('info_json', 'shared_features_json')",
    )
    .get_result::<CountRow>(connection)?;
    assert_eq!(removed_columns.count, 0);
    let removed_part_column = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('software_parts') \
         WHERE name = 'features_json'",
    )
    .get_result::<CountRow>(connection)?;
    assert_eq!(removed_part_column.count, 0);
    Ok(item.source_line)
}

fn assert_software_list_parts(
    connection: &mut SqliteConnection,
    snapshot: &mame_coalesce::domain::SnapshotKey,
    item_line: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let parts = sql_query(
        "SELECT GROUP_CONCAT(part_name, ',') AS value FROM ( \
         SELECT software_parts.part_name FROM software_parts \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' ORDER BY software_parts.part_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(parts.value, "cart,manual");

    let part_interfaces = sql_query(
        "SELECT GROUP_CONCAT(interface, ',') AS value FROM ( \
         SELECT software_parts.interface FROM software_parts \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' ORDER BY software_parts.part_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(part_interfaces.value, "demo_cart,document");

    let area_order = sql_query(
        "SELECT GROUP_CONCAT(area_kind || ':' || area_name, ',') AS value FROM ( \
         SELECT areas.area_kind, areas.area_name FROM software_areas AS areas \
         JOIN software_parts AS parts ON parts.part_id = areas.part_id \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' AND parts.part_name = 'cart' \
         ORDER BY areas.area_order)",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextRow>(connection)?;
    assert_eq!(area_order.value, "data:program,disk:media");

    let area = sql_query(
        "SELECT areas.declared_size, areas.width, areas.endianness, areas.source_line \
         FROM software_areas AS areas \
         JOIN software_parts AS parts ON parts.part_id = areas.part_id \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' AND parts.part_name = 'cart' \
         AND areas.area_name = 'program' AND areas.area_kind = 'data'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareAreaRow>(connection)?;
    assert_eq!(area.declared_size, Some(32));
    assert_eq!(area.width, Some(16));
    assert_eq!(area.endianness.as_deref(), Some("big"));
    assert!(item_line < area.source_line);

    let sparse_area = sql_query(
        "SELECT areas.declared_size, areas.width, areas.endianness, areas.source_line \
         FROM software_areas AS areas \
         JOIN software_parts AS parts ON parts.part_id = areas.part_id \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' AND parts.part_name = 'manual' \
         AND areas.area_name = 'text' AND areas.area_kind = 'data'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SoftwareAreaRow>(connection)?;
    assert_eq!(sparse_area.declared_size, Some(16));
    assert_eq!(sparse_area.width, Some(8));
    assert_eq!(sparse_area.endianness.as_deref(), Some("little"));
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
        "SELECT areas.source_line AS value FROM software_areas AS areas \
         JOIN software_parts AS parts ON parts.part_id = areas.part_id \
         JOIN records USING (record_id) \
         JOIN record_namespaces AS namespaces USING (namespace_id) \
         WHERE namespaces.snapshot_key = ? AND namespaces.source_name = 'demo_cart' \
         AND records.source_name = 'demo_game' AND parts.part_name = 'cart' \
         AND areas.area_name = 'program' AND areas.area_kind = 'data'",
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
    assert_eq!(disk.dump_status.as_deref(), Some("good"));
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
    assert_eq!(absent_status.dump_status.as_deref(), Some("good"));
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
    let (directory, database, mut connection) = setup()?;
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
    let document_key =
        sql_query("SELECT document_key AS value FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(&mut connection)?
            .value
            .parse::<DocumentKey>()?;
    assert_eq!(
        retained_document(&directory, &document_key)?,
        std::fs::read(&request.document_path)?
    );
    assert_eq!(report.diagnostic_count, 0);
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
    for (set_name, field, expected) in [
        ("demo_set", "description", "Synthetic parent set"),
        ("demo_set", "manufacturer", "Example Works"),
        ("demo_set", "year", "1998"),
        ("clone_set", "description", "Clone \"quoted\" set"),
    ] {
        let fact = sql_query(format!(
            "SELECT facts.{field} AS value FROM cmp_set_facts AS facts \
             JOIN snapshot_sets AS sets ON sets.set_id = facts.record_id \
             WHERE sets.snapshot_key = ? AND sets.set_name = '{set_name}'"
        ))
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<NullableTextRow>(connection)?;
        assert_eq!(fact.value.as_deref(), Some(expected), "{set_name}.{field}");
    }
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
    let header_name =
        sql_query("SELECT name AS value FROM cmp_header_facts WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<TextRow>(connection)?;
    assert_eq!(header_name.value, "Synthetic Catalog");
    let source_copy_count = sql_query(
        "SELECT COUNT(*) AS count FROM snapshot_extensions \
         WHERE snapshot_key = ? AND field_name = 'source_tokens'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(connection)?;
    assert_eq!(source_copy_count.count, 0);
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
        let logiqx_value = sql_query(format!(
            "SELECT {field} AS value FROM logiqx_set_facts \
             WHERE snapshot_key = ? AND set_name = 'equiv'"
        ))
        .bind::<Text, _>(logiqx_snapshot.as_str())
        .get_result::<NullableTextRow>(&mut connection)?;
        let dat_value = sql_query(format!(
            "SELECT facts.{field} AS value FROM cmp_set_facts AS facts \
             JOIN snapshot_sets AS sets ON sets.set_id = facts.record_id \
             WHERE sets.snapshot_key = ? AND sets.set_name = 'equiv'"
        ))
        .bind::<Text, _>(clrmamepro_snapshot.as_str())
        .get_result::<NullableTextRow>(&mut connection)?;
        assert_eq!(
            logiqx_value.value.as_deref(),
            dat_value.value.as_deref(),
            "{field}"
        );
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
fn snapshot_diff_rejects_software_list_catalogs_instead_of_reporting_empty_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, _) = setup()?;
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/software-list.xml");
    let original = std::fs::read_to_string(fixture_path)?;
    let changed = original
        .replace("0.289-synthetic", "0.290-synthetic")
        .replace("crc=\"12345678\"", "crc=\"87654321\"");
    let previous_path = directory.path().join("software-list-v1.xml");
    let current_path = directory.path().join("software-list-v2.xml");
    std::fs::write(&previous_path, original)?;
    std::fs::write(&current_path, &changed)?;

    let mut previous_request = request(
        previous_path,
        "software-list-history",
        "software-list-history",
        "Software-list history",
    )?;
    previous_request.format = CatalogDocumentFormat::MameSoftwareListXml;
    previous_request.scope = CatalogScope::Complete;
    let mut current_request = previous_request.clone();
    current_request.document_path =
        Utf8PathBuf::from_path_buf(current_path).map_err(|_| "non-UTF8 fixture path")?;
    let previous = app::import_catalog(&database, &previous_request)?
        .snapshot_key
        .ok_or("previous snapshot missing")?;
    let current = app::import_catalog(&database, &current_request)?
        .snapshot_key
        .ok_or("current snapshot missing")?;

    let Err(error) = app::diff_catalog_snapshots(&database, &previous, &current) else {
        return Err("software-list diff unexpectedly succeeded".into());
    };
    assert!(
        error
            .to_string()
            .contains("does not yet support MAME software-list")
    );

    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, changed.as_bytes())?;
    let gzip_path = directory.path().join("software-list-v2.xml.gz");
    std::fs::write(&gzip_path, encoder.finish()?)?;
    let mut gzip_request = current_request;
    gzip_request.document_path =
        Utf8PathBuf::from_path_buf(gzip_path).map_err(|_| "non-UTF8 gzip path")?;
    let gzip_snapshot = app::import_catalog(&database, &gzip_request)?
        .snapshot_key
        .ok_or("gzip snapshot missing")?;
    let Err(error) = app::diff_catalog_snapshots(&database, &previous, &gzip_snapshot) else {
        return Err("compressed software-list diff unexpectedly succeeded".into());
    };
    assert!(
        error
            .to_string()
            .contains("does not yet support MAME software-list")
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
    filtered_request.scope = filtered_root_scope(&["alpha"]);
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
    included_request.scope = filtered_root_scope(&["alpha", "beta", "gamma"]);
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
        import.scope = filtered_root_scope(if filename == "filtered-alpha.dat" {
            &["alpha"]
        } else {
            &["alpha", "beta"]
        });
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
                "<mame><machine name=\"owner\"><description>Owner</description><device_ref tag=\":target\" name=\"target\" future=\"{value}\"/></machine><machine name=\"target\"><description>Target</description></machine></mame>"
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

    let extension = sql_query(
        "SELECT raw_value_json AS value FROM snapshot_extensions \
         JOIN catalog_sets ON catalog_sets.set_id = snapshot_extensions.owner_set_id \
         WHERE snapshot_key = ? AND record_kind = 'rom' AND catalog_sets.set_name = 'alpha' \
           AND field_name = 'future'",
    )
    .bind::<Text, _>(
        second
            .snapshot_key
            .as_ref()
            .ok_or("second snapshot missing")?
            .as_str(),
    )
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(extension.value, "\"two\"");
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
fn snapshot_diff_includes_no_intro_game_specification_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database, mut connection) = setup()?;
    let first_path = directory.path().join("no-intro-game-facts-v1.xml");
    let second_path = directory.path().join("no-intro-game-facts-v2.xml");
    std::fs::write(
        &first_path,
        br#"<datafile><game name="alpha" id="0640"><description>Alpha</description></game></datafile>"#,
    )?;
    std::fs::write(
        &second_path,
        br#"<datafile><game name="alpha" id="0641"><description>Alpha II</description></game></datafile>"#,
    )?;
    let mut first_request = request(
        first_path,
        "publisher-no-intro-facts",
        "no-intro-game-facts",
        "No-Intro game facts",
    )?;
    first_request.format = CatalogDocumentFormat::NoIntroPcXml;
    first_request.scope = CatalogScope::Complete;
    let first = app::import_catalog(&database, &first_request)?;
    let mut second_request = request(
        second_path,
        "publisher-no-intro-facts",
        "no-intro-game-facts",
        "No-Intro game facts",
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
    let alpha = diff.records.first().ok_or("alpha record diff missing")?;
    assert_eq!(alpha.status, SnapshotRecordStatus::Changed);
    assert!(alpha.metadata_changed);
    let facts = sql_query(
        "SELECT archive_id, description FROM no_intro_game_facts \
         WHERE snapshot_key = ? AND set_name = 'alpha'",
    )
    .bind::<Text, _>(
        second
            .snapshot_key
            .ok_or("second snapshot missing")?
            .as_str(),
    )
    .get_result::<NoIntroGameFactsRow>(&mut connection)?;
    assert_eq!(facts.archive_id.as_deref(), Some("0641"));
    assert_eq!(facts.description.as_deref(), Some("Alpha II"));
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
    let coverage_id = insert_coverage(&mut connection, "complete")?;
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
         (snapshot_key, catalog_key, document_key, interpretation_key, coverage_id) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(identity_snapshot.as_str())
    .bind::<Text, _>(published_request.catalog_key.as_str())
    .bind::<Text, _>(&identity_document)
    .bind::<Text, _>(interpretation.as_str())
    .bind::<BigInt, _>(coverage_id)
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
        br#"<datafile><header><name>Duplicates</name></header><game name="set"><rom name="same.bin" size="1" crc="11111111" status="good"/><rom name="same.bin" size="2" crc="11111111" status="baddump"/></game></datafile>"#,
    )?;
    std::fs::write(
        &second_path,
        br#"<datafile><header><name>Duplicates</name></header><game name="set"><rom name="same.bin" size="2" crc="11111111" status="good"/><rom name="same.bin" size="1" crc="11111111" status="baddump"/></game></datafile>"#,
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
        "SELECT relation_type, origin, source_snapshot_key, source_subject_a AS subject_key, \
                source_target_a AS target_key, \
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
        "SELECT COUNT(*) AS count FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND relation_type = 'runtime_dependency' \
           AND source_field IN ('romof', 'sampleof', 'device_ref')",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(runtime_claims.count, 3);
    let persisted_runtime_claims = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND relation_type = 'runtime_dependency' \
           AND source_field IN ('romof', 'sampleof', 'device_ref')",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(persisted_runtime_claims.count, 3);
    let device_claim = sql_query(
        "SELECT source_target_a AS value FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND source_field = 'device_ref'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<TextRow>(&mut connection)?;
    assert_eq!(device_claim.value, "fixture-sound");

    let device_claim = sql_query(
        "SELECT relation_type, origin, source_snapshot_key, source_subject_a AS subject_key, \
                source_target_a AS target_key, \
                source_field, source_line, source_column, rule_version \
         FROM relationship_assertion_explanations \
         WHERE source_snapshot_key = ? AND source_field = 'device_ref'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<QueryableAssertion>(&mut connection)?;
    assert_eq!(device_claim.source_line, Some(8));
    assert_eq!(device_claim.source_column, Some(5));

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
    let failed_run_key =
        sql_query("SELECT run_key AS value FROM import_runs WHERE status = 'failed'")
            .get_result::<TextRow>(&mut connection)?
            .value;
    let retained = retained_document_for_run(&directory, &mut connection, &failed_run_key)?;
    assert!(String::from_utf8_lossy(&retained).contains("not-hex"));
    Ok(())
}
