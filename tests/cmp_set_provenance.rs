use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey,
        SnapshotRecordCorrespondence, SnapshotRecordStatus,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct SetFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rebuildto: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_year_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_month_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_day_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Text)]
    source_block: String,
    #[diesel(sql_type = BigInt)]
    document_order: i64,
}

#[derive(QueryableByName)]
struct SetDescriptionOrderRow {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = BigInt)]
    document_order: i64,
}

#[derive(QueryableByName)]
struct HeaderPositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    is_quoted: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct SetPositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    is_quoted: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct HeaderRow {
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    email: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    category: Option<String>,
    #[diesel(sql_type = Text)]
    source_block: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
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
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping_effective: Option<String>,
}

#[derive(QueryableByName)]
struct CommentRow {
    #[diesel(sql_type = BigInt)]
    comment_order: i64,
    #[diesel(sql_type = Text)]
    comment_text: String,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct SetRomPositionRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct CmpNativeLayoutRow {
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    child_order: i64,
    #[diesel(sql_type = BigInt)]
    payload_count: i64,
}

#[derive(QueryableByName)]
struct NullableTextRow {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

fn open_database(directory: &tempfile::TempDir) -> TestResult<(Utf8PathBuf, Database)> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&path)?;
    Ok((path, database))
}

fn import_cmp(
    database: &Database,
    directory: &tempfile::TempDir,
    file_name: &str,
    contents: &str,
) -> TestResult<SnapshotKey> {
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join(file_name))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::ClrMamePro,
            source_key: PublishingSourceKey::new("cmp-provenance"),
            source_display_name: "CMP provenance".into(),
            catalog_key: CatalogKey::new("cmp-provenance"),
            catalog_display_name: "CMP provenance".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    report
        .snapshot_key
        .ok_or_else(|| "CMP import did not produce a snapshot".into())
}

fn import_pair(previous: &str, current: &str) -> TestResult<CatalogSnapshotDiff> {
    let directory = tempfile::tempdir()?;
    let (_, database) = open_database(&directory)?;
    let previous = import_cmp(&database, &directory, "previous.dat", previous)?;
    let current = import_cmp(&database, &directory, "current.dat", current)?;
    Ok(app::diff_catalog_snapshots(&database, &previous, &current)?)
}

fn set_metadata(connection: &mut SqliteConnection) -> TestResult<SetFactsRow> {
    Ok(sql_query(
        "SELECT description, year, manufacturer, rebuildto, region, release_year_text, \
                release_month_text, release_day_text, serial, source_block, document_order \
         FROM cmp_set_facts",
    )
    .get_result(connection)?)
}

#[test]
fn cmp_set_fields_keep_values_and_document_provenance() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "GAME ( NAME set-a CLONEOF \"clone-parent\" DESCRIPTION \"A set\" YEAR 1984 MANUFACTURER \"Example\" REBUILDTO parent SAMPLEOF \"sample-parent\" REGION USA RELEASEYEAR 1982 RELEASEMONTH 01 RELEASEDAY 09 SERIAL \"\" rom ( name a.bin serial \"ROM serial\" ) )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    let fields = set_metadata(&mut connection)?;
    assert_eq!(fields.description.as_deref(), Some("A set"));
    assert_eq!(fields.year.as_deref(), Some("1984"));
    assert_eq!(fields.manufacturer.as_deref(), Some("Example"));
    assert_eq!(fields.rebuildto.as_deref(), Some("parent"));
    assert_eq!(fields.region.as_deref(), Some("USA"));
    assert_eq!(fields.release_year_text.as_deref(), Some("1982"));
    assert_eq!(fields.release_month_text.as_deref(), Some("01"));
    assert_eq!(fields.release_day_text.as_deref(), Some("09"));
    assert_eq!(fields.serial.as_deref(), Some(""));
    assert_eq!(fields.source_block, "GAME");
    assert_eq!(fields.document_order, 0);

    let positions = sql_query(
        "SELECT field_kind, source_field, source_order, is_quoted, source_line, source_column \
         FROM cmp_set_field_positions ORDER BY field_kind",
    )
    .load::<SetPositionRow>(&mut connection)?;
    assert_eq!(
        positions
            .iter()
            .map(|position| {
                (
                    position.field_kind,
                    position.source_field.as_str(),
                    position.source_order,
                    position.is_quoted,
                )
            })
            .collect::<Vec<_>>(),
        [
            (0, "NAME", 0, 0),
            (1, "CLONEOF", 1, 1),
            (2, "DESCRIPTION", 2, 1),
            (3, "YEAR", 3, 0),
            (4, "MANUFACTURER", 4, 1),
            (5, "REBUILDTO", 5, 0),
            (6, "SAMPLEOF", 6, 1),
            (7, "REGION", 7, 0),
            (8, "RELEASEYEAR", 8, 0),
            (9, "RELEASEMONTH", 9, 0),
            (10, "RELEASEDAY", 10, 0),
            (11, "SERIAL", 11, 1),
        ]
    );
    assert!(positions.iter().all(|position| {
        position.source_order >= 0 && position.source_line > 0 && position.source_column > 0
    }));

    let rom_serial = sql_query(
        "SELECT serial AS value FROM cmp_rom_claims \
         JOIN asset_occurrences USING (occurrence_id)",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(rom_serial.value.as_deref(), Some("ROM serial"));
    Ok(())
}

#[test]
fn cmp_header_fixed_fields_and_closed_positions_preserve_all_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "clrmamepro ( name \"Catalog\" description \"Description\" version \"v1\" date \"2024-01-02\" author \"Author\" email \"a@example.test\" homepage \"https://home.test\" url \"https://url.test\" comment \"header comment\" category \"Arcade\" header \"DAT\" forcemerging \"full\" forcezipping \"zip\" forcepacking \"split\" forcenodump \"required\" )\nGAME ( NAME set-a )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let header = sql_query(
        "SELECT name, description, version, date, author, email, homepage, url, comment, category, \
                source_block, source_order, source_line, source_column FROM cmp_header_facts",
    )
    .get_result::<HeaderRow>(&mut connection)?;
    assert_eq!(
        [
            header.name.as_deref(),
            header.description.as_deref(),
            header.version.as_deref(),
            header.date.as_deref(),
            header.author.as_deref(),
            header.email.as_deref(),
            header.homepage.as_deref(),
            header.url.as_deref(),
            header.comment.as_deref(),
            header.category.as_deref(),
        ],
        [
            Some("Catalog"),
            Some("Description"),
            Some("v1"),
            Some("2024-01-02"),
            Some("Author"),
            Some("a@example.test"),
            Some("https://home.test"),
            Some("https://url.test"),
            Some("header comment"),
            Some("Arcade"),
        ]
    );
    assert_eq!(header.source_block, "clrmamepro");
    assert_eq!(header.source_order, 0);
    assert!(header.source_line > 0 && header.source_column > 0);

    let positions = sql_query(
        "SELECT field_kind, source_field, source_order, is_quoted, source_line, source_column \
         FROM cmp_header_field_positions ORDER BY field_kind",
    )
    .load::<HeaderPositionRow>(&mut connection)?;
    assert_eq!(
        positions
            .iter()
            .map(|position| (
                position.field_kind,
                position.source_field.as_str(),
                position.source_order
            ))
            .collect::<Vec<_>>(),
        [
            (0, "name", 0),
            (1, "description", 1),
            (2, "version", 2),
            (3, "date", 3),
            (4, "author", 4),
            (5, "email", 5),
            (6, "homepage", 6),
            (7, "url", 7),
            (8, "comment", 8),
            (9, "category", 9),
            (10, "header", 10),
            (11, "forcemerging", 11),
            (12, "forcezipping", 12),
            (13, "forcepacking", 13),
            (14, "forcenodump", 14),
        ]
    );
    assert!(positions.iter().all(|position| {
        position.is_quoted == 1 && position.source_line > 0 && position.source_column > 0
    }));

    let directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
                forcenodump_effective, forcemerging_effective, forcezipping_effective \
         FROM cmp_header_directives",
    )
    .get_result::<DirectivesRow>(&mut connection)?;
    assert_eq!(directives.header_definition.as_deref(), Some("DAT"));
    assert_eq!(directives.forcemerging.as_deref(), Some("full"));
    assert_eq!(directives.forcezipping.as_deref(), Some("zip"));
    assert_eq!(directives.forcepacking.as_deref(), Some("split"));
    assert_eq!(directives.forcenodump.as_deref(), Some("required"));
    assert_eq!(
        directives.forcenodump_effective.as_deref(),
        Some("required")
    );
    assert_eq!(directives.forcemerging_effective.as_deref(), Some("full"));
    assert_eq!(directives.forcezipping_effective.as_deref(), Some("zip"));
    Ok(())
}

#[test]
fn cmp_header_directives_keep_explicit_and_effective_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "clrmamepro ( header DAT forcemerging none forcezipping unzip forcenodump required )\nGAME ( NAME set-a )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
                forcenodump_effective, forcemerging_effective, forcezipping_effective \
         FROM cmp_header_directives",
    )
    .get_result::<DirectivesRow>(&mut connection)?;
    assert_eq!(directives.header_definition.as_deref(), Some("DAT"));
    assert_eq!(directives.forcemerging.as_deref(), Some("none"));
    assert_eq!(directives.forcezipping.as_deref(), Some("unzip"));
    assert_eq!(directives.forcepacking, None);
    assert_eq!(directives.forcenodump.as_deref(), Some("required"));
    assert_eq!(
        directives.forcenodump_effective.as_deref(),
        Some("required")
    );
    assert_eq!(directives.forcemerging_effective.as_deref(), Some("none"));
    assert_eq!(directives.forcezipping_effective.as_deref(), Some("unzip"));
    Ok(())
}

#[test]
fn cmp_header_directives_accept_split_merging_and_ignore_nodump() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "clrmamepro ( forcemerging split forcezipping zip forcenodump ignore )\nGAME ( NAME set-a )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
                forcenodump_effective, forcemerging_effective, forcezipping_effective \
         FROM cmp_header_directives",
    )
    .get_result::<DirectivesRow>(&mut connection)?;
    assert_eq!(directives.forcemerging_effective.as_deref(), Some("split"));
    assert_eq!(directives.forcezipping_effective.as_deref(), Some("zip"));
    assert_eq!(directives.forcenodump_effective.as_deref(), Some("ignore"));
    Ok(())
}

#[test]
fn cmp_empty_and_absent_header_and_set_values_stay_distinct() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "clrmamepro ( name Catalog description \"\" )\nGAME ( NAME set-a SERIAL \"\" )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let header = sql_query("SELECT description AS value FROM cmp_header_facts")
        .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(header.value.as_deref(), Some(""));
    let set = set_metadata(&mut connection)?;
    assert_eq!(set.description, None);
    assert_eq!(set.year, None);
    assert_eq!(set.serial.as_deref(), Some(""));
    let absent_fields = sql_query(
        "SELECT field_kind, source_field, source_order, is_quoted, source_line, source_column \
         FROM cmp_set_field_positions ORDER BY field_kind",
    )
    .load::<SetPositionRow>(&mut connection)?;
    assert_eq!(
        absent_fields
            .iter()
            .map(|position| position.field_kind)
            .collect::<Vec<_>>(),
        [0, 11]
    );
    Ok(())
}

#[test]
fn cmp_header_directives_default_and_reject_unknown_effective_values() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "clrmamepro ( forcemerging unsupported forcezipping unsupported forcenodump unsupported )\nGAME ( NAME set-a )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
                forcenodump_effective, forcemerging_effective, forcezipping_effective \
         FROM cmp_header_directives",
    )
    .get_result::<DirectivesRow>(&mut connection)?;
    assert_eq!(directives.forcemerging.as_deref(), Some("unsupported"));
    assert_eq!(directives.forcezipping.as_deref(), Some("unsupported"));
    assert_eq!(directives.forcenodump.as_deref(), Some("unsupported"));
    assert_eq!(directives.forcenodump_effective, None);
    assert_eq!(directives.forcemerging_effective, None);
    assert_eq!(directives.forcezipping_effective, None);

    let absent_dir = tempfile::tempdir()?;
    let (absent_path, absent_db) = open_database(&absent_dir)?;
    import_cmp(
        &absent_db,
        &absent_dir,
        "catalog.dat",
        "clrmamepro ( name Catalog )\nGAME ( NAME set-a )",
    )?;
    let mut absent_connection = SqliteConnection::establish(absent_path.as_str())?;
    let absent = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
                forcenodump_effective, forcemerging_effective, forcezipping_effective \
         FROM cmp_header_directives",
    )
    .get_result::<DirectivesRow>(&mut absent_connection)?;
    assert_eq!(absent.forcenodump, None);
    assert_eq!(absent.forcenodump_effective.as_deref(), Some("obsolete"));
    assert_eq!(absent.forcemerging_effective, None);
    assert_eq!(absent.forcezipping_effective, None);
    Ok(())
}

#[test]
fn cmp_set_keeps_clone_parent_and_sample_parent_as_distinct_relations() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "GAME ( NAME child CLONEOF clone-parent SAMPLEOF sample-parent )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let clone_parent = sql_query(
        "SELECT target_name AS value FROM clrmamepro_set_links \
         JOIN records ON records.record_id = clrmamepro_set_links.set_id \
         JOIN record_namespaces USING (namespace_id) \
         WHERE records.source_name = 'child' AND link_kind='cloneof'",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(clone_parent.value.as_deref(), Some("clone-parent"));
    let sample_parent = sql_query(
        "SELECT target_name AS value FROM cmp_sample_parent_links \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE records.source_name = 'child'",
    )
    .get_result::<NullableTextRow>(&mut connection)?;
    assert_eq!(sample_parent.value.as_deref(), Some("sample-parent"));
    Ok(())
}

#[test]
fn cmp_set_rom_positions_order_rom_forms_with_scalars_and_samples() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "GAME ( NAME ordered description first rom ( name a.bin ) sample intro year 1982 rom ( name b.bin ) sample click )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let roms = sql_query(
        "SELECT claims.name, positions.source_order FROM cmp_set_rom_positions AS positions \
         JOIN cmp_rom_claims AS claims USING (occurrence_id) ORDER BY positions.source_order",
    )
    .load::<SetRomPositionRow>(&mut connection)?;
    assert_eq!(
        roms.iter()
            .map(|rom| (rom.name.as_str(), rom.source_order))
            .collect::<Vec<_>>(),
        [("a.bin", 2), ("b.bin", 5)]
    );

    let facts = set_metadata(&mut connection)?;
    assert_eq!(facts.description.as_deref(), Some("first"));
    assert_eq!(facts.year.as_deref(), Some("1982"));
    let samples = sql_query(
        "SELECT samples.sample_name AS value FROM cmp_samples AS samples \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         ORDER BY occurrence.occurrence_order",
    )
    .load::<NullableTextRow>(&mut connection)?;
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.value.as_deref())
            .collect::<Vec<_>>(),
        [Some("intro"), Some("click")]
    );
    let media_layout = sql_query(
        "SELECT layout.kind, layout.source_order, layout.child_order, \
                CASE WHEN layout.kind='sample' \
                     THEN (SELECT COUNT(*) FROM cmp_samples WHERE occurrence_id=layout.child_order) \
                     ELSE (SELECT COUNT(*) FROM cmp_rom_claims WHERE occurrence_id=layout.child_order) \
                END AS payload_count \
         FROM cmp_set_native_layout AS layout \
         WHERE layout.record_id=(SELECT record_id FROM cmp_set_facts LIMIT 1) \
           AND layout.kind IN ('rom','sample') \
         ORDER BY layout.source_order",
    )
    .load::<CmpNativeLayoutRow>(&mut connection)?;
    assert_eq!(
        media_layout
            .iter()
            .map(|entry| (entry.kind.as_str(), entry.source_order, entry.payload_count))
            .collect::<Vec<_>>(),
        [
            ("rom", 2, 1),
            ("sample", 3, 1),
            ("rom", 5, 1),
            ("sample", 6, 1)
        ]
    );
    assert!(media_layout.iter().all(|entry| entry.child_order > 0));
    Ok(())
}

#[test]
fn cmp_semicolon_comments_keep_lexical_order_and_locations() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "; before header\nclrmamepro ( name \"Catalog\" comment \"header comment\" )\n; between blocks\nGAME ( NAME set-a )\n; after set\n",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let comments = sql_query(
        "SELECT comment_order, text AS comment_text, source_line, source_column \
         FROM cmp_comments ORDER BY comment_order",
    )
    .load::<CommentRow>(&mut connection)?;
    assert_eq!(
        comments
            .iter()
            .map(|comment| (comment.comment_order, comment.comment_text.as_str()))
            .collect::<Vec<_>>(),
        [
            (0, "; before header"),
            (1, "; between blocks"),
            (2, "; after set"),
        ]
    );
    assert!(
        comments
            .iter()
            .all(|comment| comment.source_line > 0 && comment.source_column > 0)
    );
    Ok(())
}

#[test]
fn cmp_repeated_set_names_keep_separate_native_fact_rows() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database_path, database) = open_database(&directory)?;
    import_cmp(
        &database,
        &directory,
        "catalog.dat",
        "GAME ( NAME repeated DESCRIPTION first )\nGAME ( NAME repeated DESCRIPTION second )",
    )?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let descriptions = sql_query(
        "SELECT facts.description AS value, facts.document_order FROM cmp_set_facts AS facts \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE records.source_name = 'repeated' ORDER BY record_namespaces.source_order",
    )
    .load::<SetDescriptionOrderRow>(&mut connection)?;
    assert_eq!(
        descriptions
            .iter()
            .map(|row| (row.value.as_deref(), row.document_order))
            .collect::<Vec<_>>(),
        [(Some("first"), 0), (Some("second"), 1)]
    );
    Ok(())
}

#[test]
fn cmp_repeated_same_name_owner_permutation_ignores_document_order() -> TestResult {
    let previous =
        "clrmamepro ( name Catalog )\nset ( name same region A )\nset ( name same region B )";
    let current =
        "clrmamepro ( name Catalog )\nset ( name same region B )\nset ( name same region A )";
    let diff = import_pair(previous, current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated CMP owner missing from history diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn cmp_header_values_and_dialect_options_participate_in_history() -> TestResult {
    let cases = [
        (
            "header value",
            "clrmamepro ( name Catalog description Stable forcemerging none )\nGAME ( NAME set-a DESCRIPTION Stable )",
            "clrmamepro ( name Catalog description Revised forcemerging none )\nGAME ( NAME set-a DESCRIPTION Stable )",
        ),
        (
            "dialect option",
            "clrmamepro ( name Catalog description Stable forcemerging none )\nGAME ( NAME set-a DESCRIPTION Stable )",
            "clrmamepro ( name Catalog description Stable forcemerging full )\nGAME ( NAME set-a DESCRIPTION Stable )",
        ),
        (
            "header order",
            "clrmamepro ( name Catalog version v1 )\nGAME ( NAME set-a DESCRIPTION Stable )",
            "clrmamepro ( version v1 name Catalog )\nGAME ( NAME set-a DESCRIPTION Stable )",
        ),
        (
            "header quote provenance",
            "clrmamepro ( name Catalog )\nGAME ( NAME set-a DESCRIPTION Stable )",
            "clrmamepro ( name \"Catalog\" )\nGAME ( NAME set-a DESCRIPTION Stable )",
        ),
    ];
    for (label, previous, changed) in cases {
        let diff = import_pair(previous, changed)?;
        assert!(diff.document_metadata_changed, "{label}");
    }
    Ok(())
}

#[test]
fn cmp_set_block_case_field_order_and_quotes_participate_in_history() -> TestResult {
    let pairs = [
        (
            "block case",
            "GAME ( NAME set-a DESCRIPTION Stable )",
            "game ( NAME set-a DESCRIPTION Stable )",
        ),
        (
            "field order",
            "GAME ( NAME set-a DESCRIPTION Stable YEAR 1982 )",
            "GAME ( NAME set-a YEAR 1982 DESCRIPTION Stable )",
        ),
        (
            "quote provenance",
            "GAME ( NAME set-a DESCRIPTION Stable )",
            "GAME ( NAME set-a DESCRIPTION \"Stable\" )",
        ),
    ];
    for (label, previous_set, current_set) in pairs {
        let previous = format!("clrmamepro ( name Catalog )\n{previous_set}");
        let current = format!("clrmamepro ( name Catalog )\n{current_set}");
        let diff = import_pair(&previous, &current)?;
        let record = diff
            .records
            .iter()
            .find(|record| record.set_name == "set-a")
            .ok_or("CMP set missing from history diff")?;
        assert!(record.metadata_changed, "{label}");
    }
    Ok(())
}

#[test]
fn cmp_vendor_only_gaps_and_reindentation_do_not_change_history() -> TestResult {
    let previous = "clrmamepro ( name Catalog version v1 )\nGAME ( NAME set-a DESCRIPTION Stable YEAR 1982 rom ( name a.bin ) sample intro )";
    let current = "clrmamepro (\n  name Catalog\n  vendorheader extension\n  version v1\n)\n\nGAME (\n  NAME set-a\n  vendorfield extension\n  DESCRIPTION Stable\n  YEAR 1982\n  rom ( name a.bin )\n  sample intro\n)";
    let diff = import_pair(previous, current)?;
    assert!(!diff.document_metadata_changed);
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "set-a")
        .ok_or("CMP set missing from history diff")?;
    assert!(!record.metadata_changed);
    Ok(())
}

#[test]
fn cmp_swapping_rom_and_scalar_or_sample_order_changes_history() -> TestResult {
    let pairs = [
        (
            "ROM and scalar order",
            "GAME ( NAME set-a DESCRIPTION Stable rom ( name a.bin ) )",
            "GAME ( NAME set-a rom ( name a.bin ) DESCRIPTION Stable )",
        ),
        (
            "ROM and sample order",
            "GAME ( NAME set-a rom ( name a.bin ) sample intro )",
            "GAME ( NAME set-a sample intro rom ( name a.bin ) )",
        ),
    ];
    for (label, previous_set, current_set) in pairs {
        let previous = format!("clrmamepro ( name Catalog )\n{previous_set}");
        let current = format!("clrmamepro ( name Catalog )\n{current_set}");
        let diff = import_pair(&previous, &current)?;
        let record = diff
            .records
            .iter()
            .find(|record| record.set_name == "set-a")
            .ok_or("CMP set missing from history diff")?;
        assert!(record.metadata_changed, "{label}");
    }
    Ok(())
}

#[test]
fn cmp_header_crossing_repeated_sets_changes_document_layout() -> TestResult {
    let first = "GAME ( NAME repeated DESCRIPTION First )";
    let second = "GAME ( NAME repeated DESCRIPTION Second )";
    let header = "clrmamepro ( name Catalog )";
    let previous = format!("{header}\n{first}\n{second}");
    let current = format!("{first}\n{header}\n{second}");
    let diff = import_pair(&previous, &current)?;
    assert!(diff.document_metadata_changed);
    let record = diff.records.first().ok_or("missing repeated set group")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(!record.metadata_changed);
    Ok(())
}
