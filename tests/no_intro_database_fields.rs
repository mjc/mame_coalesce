#![allow(clippy::expect_used, clippy::panic)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};
use std::fmt::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Imported {
    _directory: tempfile::TempDir,
    _database: Database,
    connection: SqliteConnection,
}

impl Imported {
    fn new(xml: &str, mode: NoIntroDatabaseMode, key: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let document_path = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
        std::fs::write(&document_path, xml)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::NoIntroDatabase(mode),
                source_key: PublishingSourceKey::new(format!("no-intro-db-{key}")),
                source_display_name: format!("Synthetic No-Intro export {key}"),
                catalog_key: CatalogKey::new(format!("no-intro-db-{key}")),
                catalog_display_name: format!("Synthetic No-Intro export {key}"),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        Ok(Self {
            _directory: directory,
            _database: database,
            connection: SqliteConnection::establish(database_path.as_str())?,
        })
    }

    fn count(&mut self, query: &str) -> TestResult<i64> {
        Ok(sql_query(query)
            .get_result::<Count>(&mut self.connection)?
            .value)
    }

    fn text(&mut self, query: &str) -> TestResult<Option<String>> {
        Ok(sql_query(query)
            .get_result::<OptionalText>(&mut self.connection)?
            .value)
    }

    fn integer(&mut self, query: &str) -> TestResult<i64> {
        Ok(sql_query(query)
            .get_result::<Count>(&mut self.connection)?
            .value)
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct OptionalText {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

#[derive(QueryableByName)]
struct FieldPosition {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct Location {
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

fn assert_text_fields(
    imported: &mut Imported,
    table: &str,
    owner_column: &str,
    owner_id: i64,
    prefix: &str,
    fields: &[&str],
) -> TestResult {
    for field in fields {
        let query =
            format!("SELECT \"{field}\" AS value FROM {table} WHERE {owner_column} = {owner_id}");
        assert_eq!(
            imported.text(&query)?,
            Some(format!("{prefix}:{field}")),
            "{table}.{field} must keep its own declared value"
        );
    }
    Ok(())
}

fn assert_absent_fields(
    imported: &mut Imported,
    table: &str,
    owner_column: &str,
    owner_id: i64,
    fields: &[&str],
) -> TestResult {
    for field in fields {
        let query =
            format!("SELECT \"{field}\" AS value FROM {table} WHERE {owner_column} = {owner_id}");
        assert_eq!(
            imported.text(&query)?,
            None,
            "{table}.{field} was absent from the source and must remain NULL"
        );
    }
    Ok(())
}

fn assert_field_positions(
    imported: &mut Imported,
    table: &str,
    owner_column: &str,
    owner_id: i64,
    expected: &[(i64, i64)],
) -> TestResult {
    let rows = sql_query(format!(
        "SELECT field_kind, source_order FROM {table} WHERE {owner_column}={owner_id} ORDER BY source_order"
    ))
    .load::<FieldPosition>(&mut imported.connection)?;
    let actual = rows
        .into_iter()
        .map(|row| (row.field_kind, row.source_order))
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "field kinds retain their source attribute ordinals in {table}"
    );
    Ok(())
}

fn sequential_positions(count: usize) -> Vec<(i64, i64)> {
    (0..count)
        .map(|index| {
            let index = i64::try_from(index).expect("test field index fits i64");
            (index, index)
        })
        .collect()
}

fn assert_location(
    imported: &mut Imported,
    table: &str,
    key_column: &str,
    key: i64,
    line: i64,
    column: i64,
) -> TestResult {
    let actual = sql_query(format!(
        "SELECT source_line, source_column FROM {table} WHERE {key_column}={key}"
    ))
    .get_result::<Location>(&mut imported.connection)?;
    assert_eq!(
        (actual.source_line, actual.source_column),
        (line, column),
        "{table} location"
    );
    Ok(())
}

fn attributes(prefix: &str, fields: &[&str]) -> String {
    let mut result = String::new();
    for field in fields {
        write!(result, " {field}='{prefix}:{field}'").expect("String writes cannot fail");
    }
    result
}

fn fixture_location(xml: &str, marker: &str) -> (i64, i64) {
    assert_eq!(xml.matches(marker).count(), 1, "unique fixture marker");
    let offset = xml.find(marker).expect("fixture marker exists");
    let prefix = &xml[..offset];
    let line = i64::try_from(
        prefix
            .chars()
            .filter(|character| *character == '\n')
            .count(),
    )
    .expect("fixture line count fits i64")
        + 1;
    let column = i64::try_from(
        prefix
            .rsplit('\n')
            .next()
            .expect("prefix has a final line")
            .chars()
            .count(),
    )
    .expect("fixture column count fits i64")
        + 1;
    (line, column)
}

const ARCHIVE_FIELDS: &[&str] = &[
    "additional",
    "aftermarket",
    "alt",
    "bios",
    "categories",
    "complete",
    "dat",
    "datter_note",
    "description",
    "devstatus",
    "gameid1",
    "gameid2",
    "langchecked",
    "languages",
    "licensed",
    "listed",
    "mergename",
    "name",
    "name_alt",
    "number",
    "physical",
    "region",
    "regparent",
    "showlang",
    "special1",
    "special2",
    "sticky_note",
    "version1",
    "version2",
];

const DUMP_DETAILS_FIELDS: &[&str] = &[
    "comment1",
    "comment2",
    "d_date",
    "d_date_info",
    "dumper",
    "id",
    "link1",
    "link2",
    "link3",
    "media_title",
    "nodump",
    "origin",
    "originalformat",
    "project",
    "r_date",
    "r_date_info",
    "region",
    "rominfo",
    "section",
    "tool",
];

const DUMP_DETAILS_FIELDS_B: &[&str] = &[
    "comment1",
    "comment2",
    "d_date_info",
    "dumper",
    "link1",
    "link2",
    "link3",
    "media_title",
    "nodump",
    "origin",
    "originalformat",
    "project",
    "r_date",
    "r_date_info",
    "region",
    "rominfo",
    "section",
    "tool",
];

const DUMP_SERIAL_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "chip_serial",
    "digital_serial1",
    "digital_serial2",
    "lockout_serial",
    "media_serial1",
    "media_serial2",
    "media_serial3",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
    "romchip_serial2",
    "savechip_serial",
];

const DUMP_SERIAL_FIELDS_B: &[&str] = &[
    "box_barcode",
    "chip_serial",
    "digital_serial1",
    "digital_serial2",
    "lockout_serial",
    "media_serial1",
    "media_serial2",
    "media_serial3",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
    "romchip_serial2",
];

const DUMP_FILE_FIELDS: &[&str] = &[
    "bad",
    "date",
    "extension",
    "filter",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "item",
    "mia",
    "note",
    "origin_size",
    "serial",
    "unique",
    "update_type",
    "version",
];

const RELEASE_DETAILS_FIELDS: &[&str] = &[
    "archivename",
    "category",
    "comment",
    "date",
    "dirname",
    "group",
    "id",
    "nfo_size",
    "nfoname",
    "nfosize",
    "origin",
    "originalformat",
    "region",
    "rominfo",
    "tool",
];

const RELEASE_SERIAL_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "media_serial1",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
];

const RELEASE_FILE_FIELDS: &[&str] = &[
    "bad",
    "extension",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "id",
    "item",
    "note",
    "serial",
    "update_type",
    "version",
];

const HASHES_A: [&str; 5] = [
    "AaBbCcDd",
    "11111111111111111111111111111111",
    "2222222222222222222222222222222222222222",
    "3333333333333333333333333333333333333333333333333333333333333333",
    "4444444444444444444444444444444444444444444444444444444444444444",
];

const HASHES_B: [&str; 4] = [
    "8899Aabb",
    "99999999999999999999999999999999",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
];

const HASHES_RELEASE: [&str; 3] = [
    "55555555555555555555555555555555",
    "6666666666666666666666666666666666666666",
    "7777777777777777777777777777777777777777777777777777777777777777",
];

fn all_fields_xml() -> String {
    let archive = attributes("archive-a", ARCHIVE_FIELDS).replace(
        " additional='archive-a:additional'",
        " additional='archive-a:additional' adult=''",
    );
    let dump_details = attributes("dump-details-a", DUMP_DETAILS_FIELDS);
    let dump_serials = attributes("dump-serials-a", DUMP_SERIAL_FIELDS);
    let dump_file = attributes("dump-file-a", DUMP_FILE_FIELDS);
    let release_details = attributes("release-details-a", RELEASE_DETAILS_FIELDS);
    let release_serials = attributes("release-serials-a", RELEASE_SERIAL_FIELDS);
    let release_file = attributes("release-file-a", RELEASE_FILE_FIELDS);
    let other_dump_details = attributes("dump-details-b", DUMP_DETAILS_FIELDS)
        .replace(" id='dump-details-b:id'", " id=''")
        .replace(" d_date='dump-details-b:d_date'", "");
    let other_dump_serials = attributes("dump-serials-b", DUMP_SERIAL_FIELDS)
        .replace(" box_serial='dump-serials-b:box_serial'", " box_serial=''")
        .replace(" savechip_serial='dump-serials-b:savechip_serial'", "");
    let other_dump_file = attributes("dump-file-b", DUMP_FILE_FIELDS);
    format!(
        "<datafile>\n<header><author>header-author-a</author><piracy></piracy><author>header-author-b</author><url>header-url</url></header>\n\
         <game name='same-publisher-name'>\n\
         <archive{archive} clone='P' mergeof='opaque-merge-declaration'/>\n\
         <source>\n<details{dump_details}/>\n<serials{dump_serials}/>\n\
         <file{dump_file} id='same-publisher-id' size='0003' crc32='{}' md5='{}' sha1='{}' sha256='{}' origin_sha256='{}'/>\n</source>\n\
         <archive name='second-archive' clone='ambiguous-parent-number'/>\n\
         <release>\n<details{release_details} nfo_crc32='CcDdEeFf' nfocrc='not-a-hash'/>\n\
         <serials{release_serials}/>\n<file{release_file} size='0004' crc32='EeFf0011' md5='{}' sha1='{}' sha256='{}'/>\n</release>\n\
         <release>\n<details comment=''/>\n<serials media_serial1=''/>\n</release>\n\
         </game>\n\
         <game name='same-publisher-name'>\n<archive number='same-publisher-id'/>\n\
         <source>\n<details{other_dump_details}/>\n<serials{other_dump_serials}/>\n\
         <file{other_dump_file} id='same-publisher-id' size='0005' crc32='{}' md5='{}' sha1='{}' sha256='{}' origin_sha256='not-a-digest'/>\n\
         </source>\n</game>\n</datafile>",
        HASHES_A[0],
        HASHES_A[1],
        HASHES_A[2],
        HASHES_A[3],
        HASHES_A[4],
        HASHES_RELEASE[0],
        HASHES_RELEASE[1],
        HASHES_RELEASE[2],
        HASHES_B[0],
        HASHES_B[1],
        HASHES_B[2],
        HASHES_B[3],
    )
}

#[test]
fn single_and_sibling_framing_keep_repeated_header_presence_and_zero_sources() -> TestResult {
    for (key, xml, expected_envelope, expected_header_column) in [
        (
            "single-empty-header",
            "<datafile><header><author>first</author><author></author><url>last</url></header></datafile>",
            "single_datafile",
            11,
        ),
        (
            "sibling-empty-header",
            "<header><version>v1</version><version>v2</version></header><datafile></datafile>",
            "sibling_header_datafile",
            1,
        ),
    ] {
        let mut imported = Imported::new(xml, NoIntroDatabaseMode::ObservedCompatible, key)?;
        assert_eq!(
            imported.text("SELECT envelope_kind AS value FROM no_intro_exports LIMIT 1")?,
            Some(expected_envelope.into())
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_export_headers")?,
            1
        );
        assert_eq!(
            imported.integer("SELECT source_line AS value FROM no_intro_export_headers")?,
            1
        );
        assert_eq!(
            imported.integer("SELECT source_column AS value FROM no_intro_export_headers")?,
            expected_header_column
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_database_games")?,
            0
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_dump_sources")?,
            0
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_releases")?,
            0
        );
        assert_eq!(
            imported.integer("SELECT game_count AS value FROM no_intro_database_parse_counts")?,
            0
        );
        assert_eq!(
            imported
                .integer("SELECT dump_source_count AS value FROM no_intro_database_parse_counts")?,
            0
        );
        let fields =
            sql_query("SELECT value AS value FROM no_intro_header_fields ORDER BY source_order")
                .load::<OptionalText>(&mut imported.connection)?
                .into_iter()
                .map(|field| field.value.expect("header values are present text"))
                .collect::<Vec<_>>();
        match key {
            "single-empty-header" => {
                assert_eq!(
                    fields,
                    vec!["first".to_owned(), String::new(), "last".to_owned()]
                );
            }
            "sibling-empty-header" => {
                assert_eq!(fields, vec!["v1".to_owned(), "v2".to_owned()]);
            }
            _ => unreachable!(),
        }
    }

    let mut absent = Imported::new(
        "<datafile></datafile>",
        NoIntroDatabaseMode::ObservedCompatible,
        "absent-header",
    )?;
    assert_eq!(
        absent.integer("SELECT header_present AS value FROM no_intro_exports")?,
        0
    );
    assert_eq!(
        absent.count("SELECT COUNT(*) AS value FROM no_intro_header_fields")?,
        0
    );
    assert_eq!(
        absent.count("SELECT COUNT(*) AS value FROM no_intro_export_headers")?,
        0
    );
    Ok(())
}

#[test]
fn dump_size_presence_and_empty_spelling_do_not_block_publication() -> TestResult {
    for size in ["1", ""] {
        let xml = format!(
            "<datafile><game name='g'><source><file size='{size}'/></source></game></datafile>"
        );
        let mut imported = Imported::new(
            &xml,
            NoIntroDatabaseMode::ObservedCompatible,
            "size-witness",
        )?;
        assert_eq!(
            imported.text("SELECT source_size AS value FROM no_intro_dump_files")?,
            Some(size.into())
        );
        assert_eq!(imported.count("SELECT COUNT(*) AS value FROM no_intro_dump_file_field_positions WHERE field_kind=19")?,1);
    }
    Ok(())
}

#[test]
fn equal_current_and_origin_sha256_have_one_value_and_two_scoped_assertions() -> TestResult {
    let hash = "01".repeat(32);
    let xml = format!(
        "<datafile><game name='g'><source><file sha256='{hash}' origin_sha256='{hash}'/></source></game></datafile>"
    );
    let mut imported = Imported::new(
        &xml,
        NoIntroDatabaseMode::ObservedCompatible,
        "same-hash-scopes",
    )?;
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM digest_values")?,
        1
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM no_intro_dump_file_digests")?,
        2
    );
    assert_eq!(imported.count("SELECT COUNT(*) AS value FROM occurrence_digest_assertions WHERE scope IN ('unknown','source_origin') AND provenance='source_declared'")?,2);
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM asset_occurrences WHERE content_uuid IS NOT NULL"
        )?,
        0
    );
    Ok(())
}

#[test]
fn public_import_persists_each_native_field_owner_order_and_hash_scope() -> TestResult {
    let xml = all_fields_xml();
    let mut imported = Imported::new(&xml, NoIntroDatabaseMode::ObservedCompatible, "all-fields")?;
    assert_export_header_fields(&mut imported)?;
    assert_game_and_archive_fields(&mut imported, &xml)?;
    assert_dump_source_fields(&mut imported)?;
    assert_release_owner_fields(&mut imported)?;
    assert_dump_file_fields(&mut imported)?;
    assert_release_file_fields(&mut imported)?;
    assert_archive_relationship_fields(&mut imported)?;
    assert_occurrence_scope_fields(&mut imported)?;
    assert_digest_reference_fields(&mut imported)?;
    assert_digest_bytes(&mut imported)?;
    Ok(())
}

fn assert_export_header_fields(imported: &mut Imported) -> TestResult {
    let snapshot_key = imported
        .text("SELECT snapshot_key AS value FROM no_intro_exports")?
        .expect("native export has a snapshot key");
    let export_location =
        sql_query("SELECT source_line, source_column FROM no_intro_exports WHERE snapshot_key=?")
            .bind::<Text, _>(&snapshot_key)
            .get_result::<Location>(&mut imported.connection)?;
    assert_eq!(
        (export_location.source_line, export_location.source_column),
        (1, 1)
    );
    let header_location = sql_query(
        "SELECT source_line, source_column FROM no_intro_export_headers WHERE snapshot_key=?",
    )
    .bind::<Text, _>(&snapshot_key)
    .get_result::<Location>(&mut imported.connection)?;
    assert_eq!(
        (header_location.source_line, header_location.source_column),
        (2, 1)
    );
    assert_eq!(
        imported.text(&format!(
            "SELECT envelope_kind AS value FROM no_intro_exports WHERE snapshot_key='{snapshot_key}'"
        ))?,
        Some("single_datafile".into())
    );
    assert_eq!(
        imported.count(&format!(
            "SELECT COUNT(*) AS value FROM no_intro_header_fields WHERE snapshot_key='{snapshot_key}'"
        ))?,
        4
    );
    let header_values = sql_query(
        "SELECT value AS value FROM no_intro_header_fields WHERE snapshot_key=? ORDER BY source_order",
    )
    .bind::<Text, _>(&snapshot_key)
    .load::<OptionalText>(&mut imported.connection)?
    .into_iter()
    .map(|field| field.value.expect("header value is present text"))
    .collect::<Vec<_>>();
    assert_eq!(
        header_values,
        vec![
            "header-author-a".to_owned(),
            String::new(),
            "header-author-b".to_owned(),
            "header-url".to_owned(),
        ]
    );
    let field_kinds = sql_query(
        "SELECT field_kind AS value FROM no_intro_header_fields WHERE snapshot_key = ? ORDER BY source_order",
    )
    .bind::<Text, _>(&snapshot_key)
    .load::<Count>(&mut imported.connection)?
    .into_iter()
    .map(|row| row.value)
    .collect::<Vec<_>>();
    assert_eq!(field_kinds, [0, 1, 0, 3]);
    Ok(())
}

fn game_ids(imported: &mut Imported) -> TestResult<(i64, i64)> {
    let game_a = imported.integer(
        "SELECT n.set_id AS value FROM no_intro_database_games n \
         JOIN catalog_sets s USING(set_id) WHERE s.list_order=0",
    )?;
    let game_b = imported.integer(
        "SELECT n.set_id AS value FROM no_intro_database_games n \
         JOIN catalog_sets s USING(set_id) WHERE s.list_order=1",
    )?;
    Ok((game_a, game_b))
}

fn assert_game_and_archive_fields(imported: &mut Imported, xml: &str) -> TestResult {
    let (game_a, game_b) = game_ids(imported)?;
    assert_ne!(
        game_a, game_b,
        "duplicate publisher names retain separate game owners"
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM no_intro_database_games")?,
        2
    );
    let first_name_location = fixture_location(
        xml,
        "name='same-publisher-name'>\n<archive additional='archive-a:additional'",
    );
    let second_name_location = fixture_location(
        xml,
        "name='same-publisher-name'>\n<archive number='same-publisher-id'",
    );
    assert_eq!(
        imported.integer(
            "SELECT name_source_line AS value FROM no_intro_database_games \
             JOIN catalog_sets USING(set_id) WHERE list_order=0"
        )?,
        first_name_location.0
    );
    assert_eq!(
        imported.integer(
            "SELECT name_source_column AS value FROM no_intro_database_games \
             JOIN catalog_sets USING(set_id) WHERE list_order=0"
        )?,
        first_name_location.1
    );
    assert_eq!(
        imported.integer(
            "SELECT name_source_line AS value FROM no_intro_database_games \
             JOIN catalog_sets USING(set_id) WHERE list_order=1"
        )?,
        second_name_location.0
    );
    assert_eq!(
        imported.integer(
            "SELECT name_source_column AS value FROM no_intro_database_games \
             JOIN catalog_sets USING(set_id) WHERE list_order=1"
        )?,
        second_name_location.1
    );
    assert_archive_description_fields(imported, game_a, game_b)
}

fn assert_archive_description_fields(
    imported: &mut Imported,
    game_a: i64,
    game_b: i64,
) -> TestResult {
    let archive_a = imported.integer(&format!(
        "SELECT archive_id AS value FROM no_intro_archive_descriptions WHERE set_id={game_a} AND source_order=0"
    ))?;
    let archive_a_second = imported.integer(&format!(
        "SELECT archive_id AS value FROM no_intro_archive_descriptions WHERE set_id={game_a} AND source_order=2"
    ))?;
    let archive_b = imported.integer(&format!(
        "SELECT archive_id AS value FROM no_intro_archive_descriptions WHERE set_id={game_b} AND source_order=0"
    ))?;
    assert_ne!(archive_a, archive_a_second);
    assert_ne!(archive_a, archive_b);
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_archive_descriptions WHERE archive_id={archive_a}"
        ))?,
        0
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_archive_descriptions WHERE archive_id={archive_a_second}"
        ))?,
        2,
        "archive ordinals are raw mixed game-child ordinals"
    );
    assert_location(
        imported,
        "no_intro_archive_descriptions",
        "archive_id",
        archive_a,
        4,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_archive_descriptions",
        "archive_id",
        archive_a_second,
        10,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_archive_descriptions",
        "archive_id",
        archive_b,
        22,
        1,
    )?;
    assert_text_fields(
        imported,
        "no_intro_archive_descriptions",
        "archive_id",
        archive_a,
        "archive-a",
        ARCHIVE_FIELDS,
    )?;
    assert_field_positions(
        imported,
        "no_intro_archive_field_positions",
        "archive_id",
        archive_a,
        &sequential_positions(32),
    )?;
    assert_eq!(
        imported.text(&format!(
            "SELECT adult AS value FROM no_intro_archive_descriptions WHERE archive_id={archive_a}"
        ))?,
        Some(String::new()),
        "present-empty differs from absent"
    );
    assert_eq!(
        imported.text(&format!(
            "SELECT name AS value FROM no_intro_archive_descriptions WHERE archive_id={archive_a_second}"
        ))?,
        Some("second-archive".into())
    );
    assert_eq!(
        imported.text(&format!(
            "SELECT number AS value FROM no_intro_archive_descriptions WHERE archive_id={archive_b}"
        ))?,
        Some("same-publisher-id".into())
    );
    assert_absent_fields(
        imported,
        "no_intro_archive_descriptions",
        "archive_id",
        archive_a_second,
        &["adult", "aftermarket", "description"],
    )?;
    Ok(())
}

fn dump_source_ids(imported: &mut Imported) -> TestResult<(i64, i64)> {
    let (game_a, game_b) = game_ids(imported)?;
    let source_a = imported.integer(&format!(
        "SELECT dump_source_id AS value FROM no_intro_dump_sources WHERE set_id={game_a}"
    ))?;
    let source_b = imported.integer(&format!(
        "SELECT dump_source_id AS value FROM no_intro_dump_sources WHERE set_id={game_b}"
    ))?;
    Ok((source_a, source_b))
}

fn assert_dump_source_fields(imported: &mut Imported) -> TestResult {
    let (source_a, source_b) = dump_source_ids(imported)?;
    assert_ne!(source_a, source_b);
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_dump_sources WHERE dump_source_id={source_a}"
        ))?,
        1
    );
    assert_location(
        imported,
        "no_intro_dump_sources",
        "dump_source_id",
        source_a,
        5,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_dump_sources",
        "dump_source_id",
        source_b,
        23,
        1,
    )?;
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_dump_details WHERE dump_source_id={source_a}"
        ))?,
        0
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_dump_serials WHERE dump_source_id={source_a}"
        ))?,
        1
    );
    assert_dump_source_a_fields(imported, source_a)?;
    assert_dump_source_b_fields(imported, source_b)
}

fn assert_dump_source_a_fields(imported: &mut Imported, source_a: i64) -> TestResult {
    assert_text_fields(
        imported,
        "no_intro_dump_details",
        "dump_source_id",
        source_a,
        "dump-details-a",
        DUMP_DETAILS_FIELDS,
    )?;
    assert_text_fields(
        imported,
        "no_intro_dump_serials",
        "dump_source_id",
        source_a,
        "dump-serials-a",
        DUMP_SERIAL_FIELDS,
    )?;
    assert_field_positions(
        imported,
        "no_intro_dump_details_field_positions",
        "dump_source_id",
        source_a,
        &sequential_positions(20),
    )?;
    assert_field_positions(
        imported,
        "no_intro_dump_serials_field_positions",
        "dump_source_id",
        source_a,
        &sequential_positions(14),
    )?;
    assert_location(
        imported,
        "no_intro_dump_details",
        "dump_source_id",
        source_a,
        6,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_dump_serials",
        "dump_source_id",
        source_a,
        7,
        1,
    )
}

fn assert_dump_source_b_fields(imported: &mut Imported, source_b: i64) -> TestResult {
    assert_text_fields(
        imported,
        "no_intro_dump_details",
        "dump_source_id",
        source_b,
        "dump-details-b",
        DUMP_DETAILS_FIELDS_B,
    )?;
    assert_text_fields(
        imported,
        "no_intro_dump_serials",
        "dump_source_id",
        source_b,
        "dump-serials-b",
        DUMP_SERIAL_FIELDS_B,
    )?;
    assert_eq!(
        imported.text(&format!(
            "SELECT id AS value FROM no_intro_dump_details WHERE dump_source_id={source_b}"
        ))?,
        Some(String::new())
    );
    assert_absent_fields(
        imported,
        "no_intro_dump_details",
        "dump_source_id",
        source_b,
        &["d_date"],
    )?;
    assert_absent_fields(
        imported,
        "no_intro_dump_serials",
        "dump_source_id",
        source_b,
        &["savechip_serial"],
    )?;
    assert_eq!(
        imported.text(&format!(
            "SELECT box_serial AS value FROM no_intro_dump_serials WHERE dump_source_id={source_b}"
        ))?,
        Some(String::new())
    );
    let (source_a, _) = dump_source_ids(imported)?;
    assert_eq!(
        imported.text(&format!(
            "SELECT id AS value FROM no_intro_dump_details WHERE dump_source_id={source_a}"
        ))?,
        Some("dump-details-a:id".into())
    );
    Ok(())
}

fn release_ids(imported: &mut Imported) -> TestResult<(i64, i64)> {
    let (game_a, _) = game_ids(imported)?;
    let release_a = imported.integer(&format!(
        "SELECT release_id AS value FROM no_intro_releases WHERE set_id={game_a} AND source_order=3"
    ))?;
    let empty_release = imported.integer(&format!(
        "SELECT release_id AS value FROM no_intro_releases WHERE set_id={game_a} AND source_order=4"
    ))?;
    Ok((release_a, empty_release))
}

fn assert_release_owner_fields(imported: &mut Imported) -> TestResult {
    let (release_a, empty_release) = release_ids(imported)?;
    assert_location(
        imported,
        "no_intro_releases",
        "release_id",
        release_a,
        11,
        1,
    )?;
    assert_text_fields(
        imported,
        "no_intro_release_details",
        "release_id",
        release_a,
        "release-details-a",
        RELEASE_DETAILS_FIELDS,
    )?;
    assert_field_positions(
        imported,
        "no_intro_release_details_field_positions",
        "release_id",
        release_a,
        &[
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (6, 6),
            (8, 7),
            (10, 8),
            (11, 9),
            (12, 10),
            (13, 11),
            (14, 12),
            (15, 13),
            (16, 14),
            (7, 15),
            (9, 16),
        ],
    )?;
    assert_text_fields(
        imported,
        "no_intro_release_serials",
        "release_id",
        release_a,
        "release-serials-a",
        RELEASE_SERIAL_FIELDS,
    )?;
    assert_field_positions(
        imported,
        "no_intro_release_serials_field_positions",
        "release_id",
        release_a,
        &sequential_positions(6),
    )?;
    assert_empty_release_fields(imported, empty_release)
}

fn assert_empty_release_fields(imported: &mut Imported, empty_release: i64) -> TestResult {
    assert_eq!(
        imported.text(&format!(
            "SELECT comment AS value FROM no_intro_release_details WHERE release_id={empty_release}"
        ))?,
        Some(String::new())
    );
    assert_absent_fields(
        imported,
        "no_intro_release_details",
        "release_id",
        empty_release,
        &["category"],
    )?;
    assert_eq!(
        imported.text(&format!(
            "SELECT media_serial1 AS value FROM no_intro_release_serials WHERE release_id={empty_release}"
        ))?,
        Some(String::new())
    );
    assert_absent_fields(
        imported,
        "no_intro_release_serials",
        "release_id",
        empty_release,
        &["box_serial"],
    )
}

fn dump_file_ids(imported: &mut Imported) -> TestResult<(i64, i64)> {
    let (source_a, source_b) = dump_source_ids(imported)?;
    let dump_file_a = imported.integer(&format!(
        "SELECT occurrence_id AS value FROM no_intro_dump_files WHERE dump_source_id={source_a}"
    ))?;
    let dump_file_b = imported.integer(&format!(
        "SELECT occurrence_id AS value FROM no_intro_dump_files WHERE dump_source_id={source_b}"
    ))?;
    Ok((dump_file_a, dump_file_b))
}

fn assert_dump_file_fields(imported: &mut Imported) -> TestResult {
    let (dump_file_a, dump_file_b) = dump_file_ids(imported)?;
    assert_ne!(dump_file_a, dump_file_b);
    for (occurrence_id, prefix, expected_size) in [
        (dump_file_a, "dump-file-a", 3),
        (dump_file_b, "dump-file-b", 5),
    ] {
        assert_text_fields(
            imported,
            "no_intro_dump_files",
            "occurrence_id",
            occurrence_id,
            prefix,
            DUMP_FILE_FIELDS,
        )?;
        assert_eq!(
            imported.text(&format!(
                "SELECT source_size AS value FROM no_intro_dump_files WHERE occurrence_id={occurrence_id}"
            ))?,
            Some(format!("{expected_size:04}"))
        );
        assert_eq!(
            imported.integer(&format!(
                "SELECT size AS value FROM no_intro_dump_files WHERE occurrence_id={occurrence_id}"
            ))?,
            expected_size
        );
        assert_eq!(
            imported.text(&format!(
                "SELECT id AS value FROM no_intro_dump_files WHERE occurrence_id={occurrence_id}"
            ))?,
            Some("same-publisher-id".into())
        );
    }
    assert_dump_file_positions(imported, dump_file_a, dump_file_b)
}

fn assert_dump_file_positions(
    imported: &mut Imported,
    dump_file_a: i64,
    dump_file_b: i64,
) -> TestResult {
    let positions = [
        (0, 0),
        (2, 1),
        (3, 2),
        (4, 3),
        (5, 4),
        (6, 5),
        (7, 6),
        (8, 7),
        (10, 8),
        (12, 9),
        (13, 10),
        (15, 11),
        (16, 12),
        (20, 13),
        (21, 14),
        (22, 15),
        (9, 16),
        (19, 17),
        (1, 18),
        (11, 19),
        (17, 20),
        (18, 21),
        (14, 22),
    ];
    for occurrence_id in [dump_file_a, dump_file_b] {
        assert_field_positions(
            imported,
            "no_intro_dump_file_field_positions",
            "occurrence_id",
            occurrence_id,
            &positions,
        )?;
    }
    assert_location(
        imported,
        "no_intro_dump_files",
        "occurrence_id",
        dump_file_a,
        8,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_dump_files",
        "occurrence_id",
        dump_file_b,
        26,
        1,
    )
}

fn assert_release_file_fields(imported: &mut Imported) -> TestResult {
    let (release_a, _) = release_ids(imported)?;
    let release_file = imported.integer(&format!(
        "SELECT occurrence_id AS value FROM no_intro_release_files WHERE release_id={release_a}"
    ))?;
    assert_location(
        imported,
        "no_intro_release_details",
        "release_id",
        release_a,
        12,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_release_serials",
        "release_id",
        release_a,
        13,
        1,
    )?;
    assert_location(
        imported,
        "no_intro_release_files",
        "occurrence_id",
        release_file,
        14,
        1,
    )?;
    assert_text_fields(
        imported,
        "no_intro_release_files",
        "occurrence_id",
        release_file,
        "release-file-a",
        RELEASE_FILE_FIELDS,
    )?;
    assert_field_positions(
        imported,
        "no_intro_release_file_field_positions",
        "occurrence_id",
        release_file,
        &[
            (0, 0),
            (2, 1),
            (3, 2),
            (4, 3),
            (5, 4),
            (6, 5),
            (7, 6),
            (8, 7),
            (10, 8),
            (11, 9),
            (15, 10),
            (16, 11),
            (14, 12),
            (1, 13),
            (9, 14),
            (12, 15),
            (13, 16),
        ],
    )?;
    assert_release_source_order(imported, release_file, release_a)
}

fn assert_release_source_order(
    imported: &mut Imported,
    release_file: i64,
    release_a: i64,
) -> TestResult {
    assert_eq!(
        imported.text(&format!(
            "SELECT source_size AS value FROM no_intro_release_files WHERE occurrence_id={release_file}"
        ))?,
        Some("0004".into())
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT size AS value FROM no_intro_release_files WHERE occurrence_id={release_file}"
        ))?,
        4
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_release_files WHERE occurrence_id={release_file}"
        ))?,
        2
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_release_details WHERE release_id={release_a}"
        ))?,
        0
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_release_serials WHERE release_id={release_a}"
        ))?,
        1
    );
    assert_eq!(
        imported.integer(&format!(
            "SELECT source_order AS value FROM no_intro_releases WHERE release_id={release_a}"
        ))?,
        3
    );
    Ok(())
}

fn assert_archive_relationship_fields(imported: &mut Imported) -> TestResult {
    let (game_a, _) = game_ids(imported)?;
    let archive_a = imported.integer(&format!(
        "SELECT archive_id AS value FROM no_intro_archive_descriptions WHERE set_id={game_a} AND source_order=0"
    ))?;
    let archive_a_second = imported.integer(&format!(
        "SELECT archive_id AS value FROM no_intro_archive_descriptions WHERE set_id={game_a} AND source_order=2"
    ))?;
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM no_intro_archive_clone_markers")?,
        1
    );
    assert_eq!(
        imported.text("SELECT marker AS value FROM no_intro_archive_clone_markers LIMIT 1")?,
        Some("P".into())
    );
    assert_eq!(
        imported.text(&format!(
            "SELECT declared_target_number AS value FROM no_intro_archive_clone_links WHERE archive_id={archive_a_second}"
        ))?,
        Some("ambiguous-parent-number".into())
    );
    assert_eq!(
        imported.text(&format!(
            "SELECT declared_mergeof AS value FROM no_intro_archive_merge_links WHERE archive_id={archive_a}"
        ))?,
        Some("opaque-merge-declaration".into())
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_archive_clone_links c \
             JOIN reported_catalog_relationships r USING(relationship_id) \
             JOIN no_intro_archive_descriptions a USING(archive_id)"
        )?,
        1
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_archive_merge_links m \
             JOIN reported_catalog_relationships r USING(relationship_id) \
             JOIN no_intro_archive_descriptions a USING(archive_id)"
        )?,
        1
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_archive_clone_markers m \
             JOIN no_intro_archive_descriptions a USING(archive_id)"
        )?,
        1,
        "the P marker owns its archive independently of clone and merge links"
    );
    assert_eq!(
        imported.count(&format!(
            "SELECT COUNT(*) AS value FROM no_intro_archive_clone_links l \
             JOIN catalog_relationships registry USING(relationship_id) \
             JOIN no_intro_database_source_relationships r ON r.assertion_key=registry.assertion_key \
             WHERE l.archive_id={archive_a_second} \
             AND l.declared_target_number='ambiguous-parent-number' \
             AND r.source_target_a='ambiguous-parent-number' AND r.target_set_id IS NULL \
             AND r.source_subject_c=l.archive_id AND r.target_kind='no_intro_archive_reference'"
        ))?,
        1,
        "ambiguous raw archive targets are retained without name resolution"
    );
    Ok(())
}

fn assert_occurrence_scope_fields(imported: &mut Imported) -> TestResult {
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_dump_files WHERE evidence_scope='unknown'"
        )?,
        2
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_release_files WHERE evidence_scope='unknown'"
        )?,
        1
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM asset_occurrences \
             WHERE claim_kind IN ('no_intro_database_source_file','no_intro_database_release_file') \
             AND content_uuid IS NULL"
        )?,
        3
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM no_intro_release_nfo_hashes")?,
        2,
        "NFO digests belong to release details, not file occurrences"
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_release_nfo_hashes \
             WHERE presence='present' AND scope='nfo_companion'"
        )?,
        2
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM asset_occurrences \
             WHERE claim_kind='no_intro_database_file'"
        )?,
        0,
        "NFO hashes do not create synthetic file occurrences"
    );
    Ok(())
}

fn assert_digest_reference_fields(imported: &mut Imported) -> TestResult {
    let valid_digest_references = imported.count(
        "SELECT COUNT(*) AS value FROM ( \
           SELECT digest_id FROM no_intro_dump_file_digests WHERE digest_id IS NOT NULL \
           UNION ALL SELECT digest_id FROM no_intro_release_file_digests WHERE digest_id IS NOT NULL \
           UNION ALL SELECT hash_id AS digest_id FROM no_intro_release_nfo_hashes WHERE hash_id IS NOT NULL)"
    )?;
    assert_eq!(valid_digest_references, 14);
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM digest_values")?,
        14,
        "each distinct valid byte sequence is interned exactly once"
    );
    Ok(())
}

fn assert_digest_bytes(imported: &mut Imported) -> TestResult {
    for (algorithm, literal) in [
        ("crc32", HASHES_A[0]),
        ("md5", HASHES_A[1]),
        ("sha1", HASHES_A[2]),
        ("sha256", HASHES_A[3]),
        ("sha256", HASHES_A[4]),
        ("crc32", "EeFf0011"),
        ("md5", HASHES_RELEASE[0]),
        ("sha1", HASHES_RELEASE[1]),
        ("sha256", HASHES_RELEASE[2]),
        ("crc32", HASHES_B[0]),
        ("md5", HASHES_B[1]),
        ("sha1", HASHES_B[2]),
        ("sha256", HASHES_B[3]),
        ("crc32", "CcDdEeFf"),
    ] {
        let count = sql_query(
            "SELECT COUNT(*) AS value FROM digest_values \
             WHERE algorithm=? AND upper(hex(digest))=upper(?)",
        )
        .bind::<Text, _>(algorithm)
        .bind::<Text, _>(literal)
        .get_result::<Count>(&mut imported.connection)?
        .value;
        assert_eq!(
            count, 1,
            "{algorithm} {literal} must be stored as bytes once"
        );
    }
    let (dump_file_a, _) = dump_file_ids(imported)?;
    for (field_kind, expected_hex) in [(3, HASHES_A[3]), (4, HASHES_A[4])] {
        assert_eq!(
            imported.text(&format!(
                "SELECT upper(hex(digest)) AS value FROM digest_values \
                 WHERE digest_id=(SELECT digest_id FROM no_intro_dump_file_digests \
                 WHERE occurrence_id={dump_file_a} AND field_kind={field_kind})"
            ))?,
            Some(expected_hex.to_ascii_uppercase()),
            "file SHA256 and origin SHA256 stay separate typed fields"
        );
    }
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM (SELECT algorithm,digest FROM digest_values \
             GROUP BY algorithm,digest HAVING COUNT(*) > 1)"
        )?,
        0
    );
    assert_digest_invalid_literals(imported)
}

fn assert_digest_invalid_literals(imported: &mut Imported) -> TestResult {
    assert_eq!(
        imported.text(
            "SELECT invalid_literal AS value FROM no_intro_release_nfo_hashes \
             WHERE source_hash_field='nfocrc'"
        )?,
        Some("not-a-hash".into())
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_release_nfo_hashes \
             WHERE source_hash_field='nfo_crc32' AND hash_id IS NOT NULL \
             AND invalid_literal IS NULL"
        )?,
        1
    );
    let (_, dump_file_b) = dump_file_ids(imported)?;
    assert_eq!(
        imported.text(&format!(
            "SELECT invalid_literal AS value FROM no_intro_dump_file_digests \
             WHERE occurrence_id={dump_file_b} AND field_kind=4"
        ))?,
        Some("not-a-digest".into())
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_dump_file_digests \
             WHERE invalid_literal='not-a-digest'"
        )?,
        1
    );
    Ok(())
}

#[test]
fn null_recovery_persists_each_reader_warning_with_its_run_and_document_owner() -> TestResult {
    let prefix = "<datafile>\r\n<game name=\"g\"><source><details comment1=\"";
    let suffix = "\"/></source></game></datafile>";
    let xml = format!("{prefix}a\0\0\0\0\0\0b{suffix}");
    let mut imported = Imported::new(
        &xml,
        NoIntroDatabaseMode::NullRecoveryCompatible,
        "nul-recovery",
    )?;
    assert_eq!(
        imported.text("SELECT comment1 AS value FROM no_intro_dump_details")?,
        Some("a������b".into())
    );
    let warnings = sql_query(
        "SELECT d.source_line, d.source_column FROM import_diagnostics d \
         JOIN import_runs r USING(run_key) JOIN documents doc USING(document_key) \
         WHERE d.severity='warning' AND r.status='succeeded' ORDER BY d.source_column",
    )
    .load::<Location>(&mut imported.connection)?;
    assert_eq!(warnings.len(), 6);
    let first_null_column = i64::try_from(
        prefix
            .rsplit_once('\n')
            .ok_or("line break")?
            .1
            .chars()
            .count(),
    )? + 2;
    for (index, warning) in warnings.into_iter().enumerate() {
        assert_eq!(warning.source_line, 2);
        assert_eq!(
            warning.source_column,
            first_null_column + i64::try_from(index)?
        );
    }
    Ok(())
}

#[test]
fn late_xml_failure_rolls_back_every_native_catalog_fact_but_keeps_diagnostics() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("late-error.xml"))?;
    std::fs::write(
        &document_path,
        "<datafile><header><author>synthetic</author></header>\
         <game name='synthetic'><archive number='0001'/><source><file id='x' crc32='AABBCCDD'/></source></game>\
         </datafile><trailing-root/>",
    )?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("no-intro-db-late-error"),
            source_display_name: "Synthetic malformed No-Intro export".into(),
            catalog_key: CatalogKey::new("no-intro-db-late-error"),
            catalog_display_name: "Synthetic malformed No-Intro export".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    for table in [
        "catalog_snapshots",
        "snapshot_publications",
        "catalog_set_groups",
        "catalog_sets",
        "asset_occurrences",
        "occurrence_digest_assertions",
        "catalog_contents",
        "relationship_assertions",
        "no_intro_exports",
        "no_intro_export_headers",
        "no_intro_database_games",
        "no_intro_header_fields",
        "no_intro_archive_descriptions",
        "no_intro_archive_field_positions",
        "no_intro_archive_clone_markers",
        "no_intro_archive_clone_links",
        "no_intro_archive_merge_links",
        "no_intro_dump_sources",
        "no_intro_dump_details",
        "no_intro_dump_details_field_positions",
        "no_intro_dump_serials",
        "no_intro_dump_serials_field_positions",
        "no_intro_dump_files",
        "no_intro_dump_file_field_positions",
        "no_intro_releases",
        "no_intro_release_details",
        "no_intro_release_details_field_positions",
        "no_intro_release_serials",
        "no_intro_release_serials_field_positions",
        "no_intro_release_files",
        "no_intro_release_file_field_positions",
        "no_intro_dump_file_digests",
        "no_intro_release_nfo_hashes",
        "no_intro_release_file_digests",
        "no_intro_database_parse_counts",
        "digest_values",
    ] {
        let query = format!("SELECT COUNT(*) AS value FROM {table}");
        let count = sql_query(query).get_result::<Count>(&mut connection)?.value;
        assert_eq!(count, 0, "late XML failure leaked rows into {table}");
    }
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM import_runs WHERE status='failed'")
            .get_result::<Count>(&mut connection)?
            .value,
        1
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM documents")
            .get_result::<Count>(&mut connection)?
            .value,
        1,
        "the failed acquisition keeps its document record"
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM import_diagnostics")
            .get_result::<Count>(&mut connection)?
            .value,
        1
    );
    Ok(())
}
