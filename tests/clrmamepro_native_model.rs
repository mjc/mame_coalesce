use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct HeaderFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
}

#[derive(QueryableByName)]
struct DirectivesRow {
    #[diesel(sql_type = Nullable<Text>)]
    header_definition: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcepacking: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump: Option<String>,
}

#[derive(QueryableByName)]
struct SetFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
}

#[derive(QueryableByName)]
struct SampleRow {
    #[diesel(sql_type = BigInt)]
    occurrence_order: i64,
    #[diesel(sql_type = Text)]
    sample_name: String,
}

#[derive(QueryableByName)]
struct RomFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    status_explicit_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    status_source_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    status_quoted: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    status_source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    status_source_column: Option<i64>,
}

#[derive(QueryableByName)]
struct NullableTextRow {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

fn assert_sample_occurrences(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let samples = sql_query(
        "SELECT occurrence.occurrence_order, samples.sample_name FROM cmp_samples AS samples \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         JOIN records ON records.record_id = occurrence.record_id \
         JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ? AND records.source_name = 'native-set' \
         ORDER BY occurrence.occurrence_order",
    )
    .bind::<Text, _>(snapshot_key)
    .load::<SampleRow>(connection)?;
    assert_eq!(
        samples
            .iter()
            .map(|sample| (sample.occurrence_order, sample.sample_name.as_str()))
            .collect::<Vec<_>>(),
        [(0, "intro"), (1, "click")]
    );
    Ok(())
}

#[test]
fn public_import_persists_native_header_set_sample_and_rom_facts()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = directory.path().join("native.dat");
    std::fs::write(
        &document_path,
        "clrmamepro ( name \"Native catalog\" version \"native-1\" header \"DAT\" forcemerging \"full\" forcezipping \"zip\" forcepacking \"split\" forcenodump \"obsolete\" )\n\
         game ( name \"native-set\" description \"Native set\" year 1999 manufacturer \"Example Works\" sampleof \"sample-parent\" sample \"intro\" sample \"click\" rom ( name \"native.bin\" size 4 date \"1999-01-02\" serial \"SER-42\" status \"verified\" ) )\n",
    )?;
    let request = CatalogImportRequest {
        document_path: Utf8PathBuf::from_path_buf(document_path)
            .map_err(|_| "non-UTF-8 document path")?,
        format: CatalogDocumentFormat::ClrMamePro,
        source_key: PublishingSourceKey::new("native-cmp-test"),
        source_display_name: "Native CMP test".to_owned(),
        catalog_key: CatalogKey::new("native-cmp-test"),
        catalog_display_name: "Native CMP test".to_owned(),
        scope: CatalogScope::Unknown,
    };

    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot_key = report.snapshot_key.ok_or("snapshot missing")?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    let header = sql_query("SELECT name, version FROM cmp_header_facts WHERE snapshot_key = ?")
        .bind::<Text, _>(snapshot_key.as_str())
        .get_result::<HeaderFactsRow>(&mut connection)?;
    assert_eq!(header.name.as_deref(), Some("Native catalog"));
    assert_eq!(header.version.as_deref(), Some("native-1"));

    let directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump \
         FROM cmp_header_directives WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<DirectivesRow>(&mut connection)?;
    assert_eq!(directives.header_definition.as_deref(), Some("DAT"));
    assert_eq!(directives.forcemerging.as_deref(), Some("full"));
    assert_eq!(directives.forcezipping.as_deref(), Some("zip"));
    assert_eq!(directives.forcepacking.as_deref(), Some("split"));
    assert_eq!(directives.forcenodump.as_deref(), Some("obsolete"));

    let set = sql_query(
        "SELECT description, year, manufacturer FROM cmp_set_facts \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ? AND records.source_name = 'native-set'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<SetFactsRow>(&mut connection)?;
    assert_eq!(set.description.as_deref(), Some("Native set"));
    assert_eq!(set.year.as_deref(), Some("1999"));
    assert_eq!(set.manufacturer.as_deref(), Some("Example Works"));

    let sample_parent = sql_query(
        "SELECT target_name AS value FROM cmp_sample_parent_links \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ? AND records.source_name = 'native-set'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(sample_parent.value.as_deref(), Some("sample-parent"));
    assert_sample_occurrences(&mut connection, snapshot_key.as_str())?;

    let rom = sql_query(
        "SELECT date, serial, position.source_order AS status_explicit_order, \
                position.source_field AS status_source_field, position.is_quoted AS status_quoted, \
                position.source_line AS status_source_line, position.source_column AS status_source_column \
         FROM cmp_rom_claims JOIN cmp_rom_field_positions AS position USING (occurrence_id) \
         JOIN asset_occurrences USING (occurrence_id) \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ? AND records.source_name = 'native-set' AND position.field_kind = 9",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<RomFactsRow>(&mut connection)?;
    assert_eq!(rom.date.as_deref(), Some("1999-01-02"));
    assert_eq!(rom.serial.as_deref(), Some("SER-42"));
    assert_eq!(rom.status_explicit_order, Some(4));
    assert_eq!(rom.status_source_field.as_deref(), Some("status"));
    assert_eq!(rom.status_quoted, Some(1));
    assert!(rom.status_source_line.is_some_and(|line| line > 0));
    assert!(rom.status_source_column.is_some_and(|column| column > 0));
    let status = sql_query(
        "SELECT dump_status AS value FROM cmp_rom_claims \
         JOIN asset_occurrences USING (occurrence_id) JOIN records USING (record_id) \
         JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ? AND records.source_name = 'native-set'",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(status.value.as_deref(), Some("verified"));
    Ok(())
}
