#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    NoIntroDatMode, RestoreOutcome, RestorePolicy, app,
    app::{CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    create_backup,
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    restore_backup,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CRC: &str = "ABCDEF01";
const MD5: &str = "0123456789ABCDEF0123456789ABCDEF";
const SHA1: &str = "0123456789ABCDEF0123456789ABCDEF01234567";
const SHA256: &str = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";

struct Imported {
    directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    document_path: Utf8PathBuf,
    snapshot_key: SnapshotKey,
    connection: SqliteConnection,
}

impl Imported {
    fn new(document: &str, mode: NoIntroDatMode) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = utf8(directory.path().join("catalog.sqlite"))?;
        let document_path = utf8(directory.path().join("source.dat"))?;
        std::fs::write(&document_path, document.as_bytes())?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: document_path.clone(),
                format: CatalogDocumentFormat::NoIntroDat(mode),
                source_key: PublishingSourceKey::new("no-intro-full-specification"),
                source_display_name: "No-Intro full specification fixture".to_owned(),
                catalog_key: CatalogKey::new("no-intro-full-specification"),
                catalog_display_name: "No-Intro full specification fixture".to_owned(),
                scope: CatalogScope::Complete,
            },
        )?;
        let diagnostic = if report.status == CatalogImportStatus::Failed {
            Some(
                sql_query("SELECT message AS value FROM import_diagnostics LIMIT 1")
                    .get_result::<NullableText>(&mut SqliteConnection::establish(
                        database_path.as_str(),
                    )?)?
                    .value,
            )
        } else {
            None
        };
        assert_eq!(
            report.status,
            CatalogImportStatus::Succeeded,
            "strict import diagnostic count: {}; message: {diagnostic:?}",
            report.diagnostic_count
        );
        let snapshot_key = report
            .snapshot_key
            .ok_or("successful import has no snapshot")?;
        Ok(Self {
            directory,
            database,
            database_path: database_path.clone(),
            document_path,
            snapshot_key,
            connection: SqliteConnection::establish(database_path.as_str())?,
        })
    }

    fn count(&mut self, query: &str) -> TestResult<i64> {
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
struct HeaderFields {
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    trademarks: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    piracy: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    subset: Option<String>,
}

#[derive(QueryableByName)]
struct OptionsFields {
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_text: Option<String>,
    #[diesel(sql_type = Text)]
    forcenodump_effective: String,
    #[diesel(sql_type = Nullable<Text>)]
    header_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    plugin_text: Option<String>,
}

#[derive(QueryableByName)]
struct GameFields {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cloneof_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cloneofid_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description_text: Option<String>,
}

#[derive(QueryableByName)]
struct CategoryRow {
    #[diesel(sql_type = BigInt)]
    category_order: i64,
    #[diesel(sql_type = Text)]
    category: String,
}

#[derive(QueryableByName)]
struct ReleaseRow {
    #[diesel(sql_type = BigInt)]
    release_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    region: String,
    #[diesel(sql_type = BigInt)]
    name_source_order: i64,
    #[diesel(sql_type = BigInt)]
    name_source_line: i64,
    #[diesel(sql_type = BigInt)]
    name_source_column: i64,
    #[diesel(sql_type = BigInt)]
    region_source_order: i64,
    #[diesel(sql_type = BigInt)]
    region_source_line: i64,
    #[diesel(sql_type = BigInt)]
    region_source_column: i64,
}

#[derive(QueryableByName)]
struct RomFields {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    size_text: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
    #[diesel(sql_type = Nullable<Text>)]
    status_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_text: Option<String>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
}

#[derive(QueryableByName)]
struct Digest {
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
}

#[derive(QueryableByName)]
struct Position {
    #[diesel(sql_type = BigInt, column_name = source_order)]
    order: i64,
    #[diesel(sql_type = BigInt, column_name = source_line)]
    line: i64,
    #[diesel(sql_type = BigInt, column_name = source_column)]
    column: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct FieldPosition {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    position_domain: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct ReleasePosition {
    #[diesel(sql_type = Text)]
    field_kind: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct InvalidDigest {
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Text)]
    invalid_text: String,
}

#[derive(QueryableByName)]
struct NullableText {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

#[derive(QueryableByName)]
struct NullableInteger {
    #[diesel(sql_type = Nullable<BigInt>)]
    value: Option<i64>,
}

fn utf8(path: std::path::PathBuf) -> TestResult<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| path.display().to_string().into())
}

fn fixture_location(document: &str, marker: &str) -> (i64, i64) {
    assert_eq!(
        document.matches(marker).count(),
        1,
        "unique fixture marker: {marker:?}"
    );
    let offset = document.find(marker).expect("fixture marker exists");
    let prefix = &document[..offset];
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

fn full_field_document(mode: NoIntroDatMode) -> String {
    let v4_header = if matches!(mode, NoIntroDatMode::V4Strict) {
        "    <trademarks>Trademark text</trademarks>\n    <piracy>Piracy text</piracy>\n"
    } else {
        ""
    };
    format!(
        r#"<datafile>
  <header>
    <id>0007</id>
    <name>Fixture catalog</name>
    <description>Fixture description</description>
    <version>v1.2</version>
    <date>2026-09-30</date>
    <author>Fixture publisher</author>
    <homepage>https://home.example.test</homepage>
    <url>https://dat.example.test/source</url>
{v4_header}    <subset>Handheld subset</subset>
    <clrmamepro forcenodump="required" header="DAT header"/>
    <romcenter plugin="fixture-plugin"/>
  </header>
  <game
    name="fixture-game-a" id="0007" cloneof="parent-game" cloneofid="0001">
    <category></category>
    <category>Arcade</category>
    <description>First game description</description>
    <rom
      name="fixture-a.bin" size="4294967295" crc="{CRC}" md5="{MD5}"
      sha1="{SHA1}" sha256="{SHA256}" status="verified" serial="SER-001"/>
    <release
      name="First release" region="USA"/>
    <release name="Second release" region="Japan"/>
  </game>
  <game
    name="fixture-game-b" id="0007" cloneof="fixture-game-a" cloneofid="0007">
    <category>Console</category>
    <description>Second game description</description>
    <rom
      name="fixture-b.bin" size="4294967295" crc="{CRC}" md5="{MD5}"
      sha1="{SHA1}" sha256="{SHA256}" status="verified" serial="SER-002"
      header="console header"/>
    <release name="Third release" region="Europe"/>
  </game>
</datafile>"#
    )
}

#[test]
fn strict_v3_and_v4_publish_every_declared_schema_field_in_native_owners() -> TestResult {
    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let document = full_field_document(mode);
        let mut imported = Imported::new(&document, mode)?;
        assert_full_header(&mut imported.connection, mode, &document)?;
        assert_games_categories_releases(&mut imported.connection, &document)?;
        assert_roms_hashes_and_positions(&mut imported, &document)?;

        // Publisher IDs are lexical catalog fields, not globally unique keys.
        assert_eq!(
            imported
                .count("SELECT COUNT(*) AS value FROM no_intro_dat_games WHERE id_text='0007'")?,
            2,
            "mode={mode:?}"
        );
        assert_eq!(
            imported.count("SELECT declared_version IS NULL AS value FROM catalog_snapshots")?,
            1,
            "the source header version must not populate the unrelated snapshot field"
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM sqlite_master WHERE type='table' AND name LIKE '%spec_element%'")?,
            0,
            "No-Intro fields must remain in their typed native tables"
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM catalog_contents")?,
            0,
            "clrmamepro@header filters every ROM claim in the DAT"
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM asset_occurrences WHERE claim_kind='no_intro_dat_rom' AND content_uuid IS NULL")?,
            2,
            "the document-wide header filter prevents UUID assignment to both ROMs"
        );
        assert_unfiltered_whole_file_claims_link(mode)?;

        if matches!(mode, NoIntroDatMode::V4Strict) {
            assert_source_recovery_survives_backup_restore(imported, document.as_bytes())?;
        }
    }
    Ok(())
}

fn assert_full_header(
    connection: &mut SqliteConnection,
    mode: NoIntroDatMode,
    document: &str,
) -> TestResult {
    let header = sql_query(
        "SELECT id_text,name,description,version_text,date,author,homepage,url,trademarks,piracy,subset \
         FROM no_intro_dat_headers",
    )
    .get_result::<HeaderFields>(connection)?;
    assert_eq!(header.id_text.as_deref(), Some("0007"));
    assert_eq!(header.name.as_deref(), Some("Fixture catalog"));
    assert_eq!(header.description.as_deref(), Some("Fixture description"));
    assert_eq!(header.version_text.as_deref(), Some("v1.2"));
    assert_eq!(header.date.as_deref(), Some("2026-09-30"));
    assert_eq!(header.author.as_deref(), Some("Fixture publisher"));
    assert_eq!(
        header.homepage.as_deref(),
        Some("https://home.example.test")
    );
    assert_eq!(
        header.url.as_deref(),
        Some("https://dat.example.test/source")
    );
    if matches!(mode, NoIntroDatMode::V4Strict) {
        assert_eq!(header.trademarks.as_deref(), Some("Trademark text"));
        assert_eq!(header.piracy.as_deref(), Some("Piracy text"));
    } else {
        assert_eq!(header.trademarks, None);
        assert_eq!(header.piracy, None);
    }
    assert_eq!(header.subset.as_deref(), Some("Handheld subset"));

    assert_full_options(connection)?;
    assert_header_positions(connection, mode, document)
}

fn assert_full_options(connection: &mut SqliteConnection) -> TestResult {
    let options = sql_query(
        "SELECT clrmamepro.forcenodump_text,clrmamepro.forcenodump_effective, \
                clrmamepro.header_text,romcenter.plugin_text \
         FROM no_intro_dat_clrmamepro_options AS clrmamepro \
         JOIN no_intro_dat_romcenter_options AS romcenter USING(snapshot_key)",
    )
    .get_result::<OptionsFields>(connection)?;
    assert_eq!(options.forcenodump_text.as_deref(), Some("required"));
    assert_eq!(options.forcenodump_effective, "required");
    assert_eq!(options.header_text.as_deref(), Some("DAT header"));
    assert_eq!(options.plugin_text.as_deref(), Some("fixture-plugin"));
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_clrmamepro_declared_field_presence")
            .get_result::<Count>(connection)?
            .value,
        2
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_romcenter_declared_field_presence")
            .get_result::<Count>(connection)?
            .value,
        1
    );

    assert_option_positions(connection)
}

fn assert_header_positions(
    connection: &mut SqliteConnection,
    mode: NoIntroDatMode,
    document: &str,
) -> TestResult {
    let header_positions = sql_query(
        "SELECT field_kind,CAST(NULL AS INTEGER) AS position_domain,source_order,source_line,source_column \
         FROM no_intro_dat_header_field_positions \
         WHERE field_kind IN (0,1,5,10) ORDER BY field_kind",
    )
    .load::<FieldPosition>(connection)?;
    let v4 = matches!(mode, NoIntroDatMode::V4Strict);
    let id_location = fixture_location(document, "<id>");
    let name_location = fixture_location(document, "<name>");
    let author_location = fixture_location(document, "<author>");
    let subset_location = fixture_location(document, "<subset>");
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_header_field_positions")
            .get_result::<Count>(connection)?
            .value,
        if v4 { 11 } else { 9 },
        "every declared header child retains a position"
    );
    assert_eq!(
        header_positions,
        [
            FieldPosition {
                field_kind: 0,
                position_domain: None,
                source_order: 0,
                source_line: id_location.0,
                source_column: id_location.1
            },
            FieldPosition {
                field_kind: 1,
                position_domain: None,
                source_order: 1,
                source_line: name_location.0,
                source_column: name_location.1
            },
            FieldPosition {
                field_kind: 5,
                position_domain: None,
                source_order: 5,
                source_line: author_location.0,
                source_column: author_location.1
            },
            FieldPosition {
                field_kind: 10,
                position_domain: None,
                source_order: if v4 { 10 } else { 8 },
                source_line: subset_location.0,
                source_column: subset_location.1
            },
        ],
        "header fields retain their declaration kind, child ordinal, and opening-tag location"
    );
    Ok(())
}

fn assert_option_positions(connection: &mut SqliteConnection) -> TestResult {
    for (table, expected) in [
        ("no_intro_dat_clrmamepro_field_positions", 2),
        ("no_intro_dat_romcenter_field_positions", 1),
    ] {
        let positions = sql_query(format!(
            "SELECT source_order,source_line,source_column FROM {table} ORDER BY source_order"
        ))
        .load::<Position>(connection)?;
        assert_eq!(positions.len(), expected, "{table}");
        assert_positions_are_source_ordered(&positions);
    }
    Ok(())
}

fn assert_games_categories_releases(
    connection: &mut SqliteConnection,
    document: &str,
) -> TestResult {
    let games = sql_query(
        "SELECT sets.set_name,game.id_text,clone.target_literal AS cloneof_text,parent_id.target_literal AS cloneofid_text,game.description_text \
         FROM no_intro_dat_games AS game JOIN catalog_sets AS sets USING(set_id) \
         LEFT JOIN no_intro_dat_set_links AS clone ON clone.set_id=game.set_id AND clone.link_kind='cloneof' \
         LEFT JOIN no_intro_dat_set_links AS parent_id ON parent_id.set_id=game.set_id AND parent_id.link_kind='cloneofid' \
         ORDER BY sets.list_order",
    )
    .load::<GameFields>(connection)?;
    assert_eq!(games.len(), 2);
    assert_eq!(games[0].set_name, "fixture-game-a");
    assert_eq!(games[0].id_text.as_deref(), Some("0007"));
    assert_eq!(games[0].cloneof_text.as_deref(), Some("parent-game"));
    assert_eq!(games[0].cloneofid_text.as_deref(), Some("0001"));
    assert_eq!(
        games[0].description_text.as_deref(),
        Some("First game description")
    );
    assert_eq!(games[1].set_name, "fixture-game-b");
    assert_eq!(games[1].id_text.as_deref(), Some("0007"));
    assert_eq!(games[1].cloneof_text.as_deref(), Some("fixture-game-a"));
    assert_eq!(games[1].cloneofid_text.as_deref(), Some("0007"));
    assert_eq!(
        games[1].description_text.as_deref(),
        Some("Second game description")
    );

    assert_categories(connection)?;
    assert_releases(connection, document)
}

fn assert_categories(connection: &mut SqliteConnection) -> TestResult {
    let categories = sql_query(
        "SELECT category_order,category FROM no_intro_dat_categories \
         WHERE set_id=(SELECT game.set_id FROM no_intro_dat_games AS game \
                       JOIN catalog_sets AS sets USING(set_id) \
                       WHERE sets.set_name='fixture-game-a') \
         ORDER BY category_order",
    )
    .load::<CategoryRow>(connection)?;
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].category_order, 0);
    assert_eq!(categories[0].category, "");
    assert_eq!(categories[1].category_order, 1);
    assert_eq!(categories[1].category, "Arcade");
    assert_eq!(
        sql_query(
            "SELECT category_order,category FROM no_intro_dat_categories \
             WHERE set_id=(SELECT set_id FROM catalog_sets WHERE set_name='fixture-game-b')",
        )
        .get_result::<CategoryRow>(connection)?
        .category,
        "Console"
    );

    Ok(())
}

fn assert_releases(connection: &mut SqliteConnection, document: &str) -> TestResult {
    let releases = sql_query(
        "SELECT release_order,name,region,name_source_order,name_source_line,name_source_column, \
                region_source_order,region_source_line,region_source_column \
         FROM no_intro_dat_releases \
         WHERE set_id=(SELECT set_id FROM catalog_sets WHERE set_name='fixture-game-a') \
         ORDER BY release_order",
    )
    .load::<ReleaseRow>(connection)?;
    assert_eq!(releases.len(), 2);
    assert_eq!(
        (
            releases[0].release_order,
            releases[0].name.as_str(),
            releases[0].region.as_str()
        ),
        (0, "First release", "USA")
    );
    assert_eq!(
        (
            releases[1].release_order,
            releases[1].name.as_str(),
            releases[1].region.as_str()
        ),
        (1, "Second release", "Japan")
    );
    let release_positions = sql_query(
        "SELECT 'name' AS field_kind,name_source_order AS source_order, \
                name_source_line AS source_line,name_source_column AS source_column \
         FROM no_intro_dat_releases WHERE set_id=(SELECT set_id FROM catalog_sets WHERE set_name='fixture-game-a') \
         AND release_order=0 \
         UNION ALL \
         SELECT 'region',region_source_order,region_source_line,region_source_column \
         FROM no_intro_dat_releases WHERE set_id=(SELECT set_id FROM catalog_sets WHERE set_name='fixture-game-a') \
         AND release_order=0 ORDER BY field_kind",
    )
    .load::<ReleasePosition>(connection)?;
    let release_location = fixture_location(document, "<release\n      name=\"First release\"");
    assert_eq!(
        (
            releases[0].name_source_order,
            releases[0].name_source_line,
            releases[0].name_source_column,
            releases[0].region_source_order,
            releases[0].region_source_line,
            releases[0].region_source_column,
        ),
        (
            0,
            release_location.0,
            release_location.1,
            1,
            release_location.0,
            release_location.1
        ),
        "release name and region retain their distinct source ordinals and owner location"
    );
    assert_eq!(
        release_positions,
        [
            ReleasePosition {
                field_kind: "name".to_owned(),
                source_order: 0,
                source_line: release_location.0,
                source_column: release_location.1
            },
            ReleasePosition {
                field_kind: "region".to_owned(),
                source_order: 1,
                source_line: release_location.0,
                source_column: release_location.1
            },
        ],
        "release attributes keep their field identity, attribute ordinal, and opening-tag location"
    );
    assert_eq!(
        sql_query(
            "SELECT release_order,name,region,name_source_order,name_source_line,name_source_column, \
                    region_source_order,region_source_line,region_source_column \
             FROM no_intro_dat_releases \
             WHERE set_id=(SELECT set_id FROM catalog_sets WHERE set_name='fixture-game-b')",
        )
        .get_result::<ReleaseRow>(connection)?
        .region,
        "Europe"
    );
    Ok(())
}

fn assert_roms_hashes_and_positions(imported: &mut Imported, document: &str) -> TestResult {
    let roms = sql_query(
        "SELECT rom.name,rom.size_text,rom.size,rom.status_text,rom.serial_text, \
                rom.header_text,rom.evidence_scope \
         FROM no_intro_dat_rom_claims AS rom \
         JOIN asset_occurrences AS occurrence USING(occurrence_id) \
         JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id \
         ORDER BY sets.list_order",
    )
    .load::<RomFields>(&mut imported.connection)?;
    assert_eq!(roms.len(), 2);
    for rom in &roms {
        assert_eq!(rom.size_text, "4294967295");
        assert_eq!(rom.size, 4_294_967_295);
        assert_eq!(rom.status_text.as_deref(), Some("verified"));
    }
    assert_eq!(roms[0].name, "fixture-a.bin");
    assert_eq!(roms[0].serial_text.as_deref(), Some("SER-001"));
    assert_eq!(roms[0].header_text, None);
    assert_eq!(roms[0].evidence_scope, "unknown");
    assert_eq!(roms[1].name, "fixture-b.bin");
    assert_eq!(roms[1].serial_text.as_deref(), Some("SER-002"));
    assert_eq!(roms[1].header_text.as_deref(), Some("console header"));
    assert_eq!(roms[1].evidence_scope, "unknown");

    assert_hashes(imported)?;
    assert_game_positions(imported, document)?;
    assert_rom_positions(imported, document)?;
    assert_child_positions(imported)
}

fn assert_hashes(imported: &mut Imported) -> TestResult {
    let digests = sql_query(
        "SELECT digest_value.algorithm,digest_value.digest FROM digest_values AS digest_value \
         ORDER BY digest_value.algorithm",
    )
    .load::<Digest>(&mut imported.connection)?;
    assert_eq!(digests.len(), 4);
    assert_eq!(
        digests
            .iter()
            .map(|digest| digest.algorithm.as_str())
            .collect::<Vec<_>>(),
        ["crc32", "md5", "sha1", "sha256"]
    );
    for (digest, spelling) in digests.iter().zip([CRC, MD5, SHA1, SHA256]) {
        assert_eq!(digest.digest, decode_hex(spelling));
    }
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM no_intro_dat_rom_digest_fields WHERE digest_id IS NOT NULL AND invalid_text IS NULL")?,
        8,
        "the repeated fixed hashes share four canonical interned values"
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM ( \
             SELECT occurrence_id FROM no_intro_dat_rom_digest_fields \
             GROUP BY occurrence_id HAVING COUNT(*)=4 \
             )",
        )?,
        2,
        "each ROM owns CRC, MD5, SHA-1, and SHA-256 declarations"
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM occurrence_digest_assertions")?,
        8
    );

    Ok(())
}

fn assert_game_positions(imported: &mut Imported, document: &str) -> TestResult {
    let first_game = imported.count(
        "SELECT game.set_id AS value FROM no_intro_dat_games AS game \
         JOIN catalog_sets AS sets USING(set_id) WHERE sets.set_name='fixture-game-a'",
    )?;
    let game_location = fixture_location(document, "<game\n    name=\"fixture-game-a\"");
    let description_location = fixture_location(document, "<description>First game description");
    for position_domain in [0, 1] {
        let positions = sql_query(
            "SELECT source_order,source_line,source_column \
             FROM no_intro_dat_game_field_positions WHERE set_id=? AND position_domain=? \
             ORDER BY source_order",
        )
        .bind::<BigInt, _>(first_game)
        .bind::<BigInt, _>(position_domain)
        .load::<Position>(&mut imported.connection)?;
        assert!(
            !positions.is_empty(),
            "game position domain {position_domain}"
        );
        assert_positions_are_source_ordered(&positions);
    }
    let game_positions = sql_query(
        "SELECT field_kind,position_domain,source_order,source_line,source_column \
         FROM no_intro_dat_game_field_positions WHERE set_id=? AND field_kind IN (0,1,2,3,4) \
         ORDER BY field_kind",
    )
    .bind::<BigInt, _>(first_game)
    .load::<FieldPosition>(&mut imported.connection)?;
    assert_eq!(
        game_positions,
        [
            FieldPosition {
                field_kind: 0,
                position_domain: Some(0),
                source_order: 0,
                source_line: game_location.0,
                source_column: game_location.1
            },
            FieldPosition {
                field_kind: 1,
                position_domain: Some(0),
                source_order: 1,
                source_line: game_location.0,
                source_column: game_location.1
            },
            FieldPosition {
                field_kind: 2,
                position_domain: Some(0),
                source_order: 2,
                source_line: game_location.0,
                source_column: game_location.1
            },
            FieldPosition {
                field_kind: 3,
                position_domain: Some(0),
                source_order: 3,
                source_line: game_location.0,
                source_column: game_location.1
            },
            FieldPosition {
                field_kind: 4,
                position_domain: Some(1),
                source_order: 2,
                source_line: description_location.0,
                source_column: description_location.1
            },
        ],
        "game attributes and child description use distinct position domains"
    );
    Ok(())
}

fn assert_rom_positions(imported: &mut Imported, document: &str) -> TestResult {
    let rom_positions = sql_query(
        "SELECT position.field_kind,CAST(NULL AS INTEGER) AS position_domain, \
                position.source_order,position.source_line,position.source_column \
         FROM no_intro_dat_rom_field_positions AS position \
         JOIN no_intro_dat_rom_claims AS rom USING(occurrence_id) \
         WHERE rom.name='fixture-a.bin' AND position.field_kind IN (0,1,5,6,7) \
         ORDER BY position.field_kind",
    )
    .load::<FieldPosition>(&mut imported.connection)?;
    let all_rom_positions = sql_query(
        "SELECT position.source_order,position.source_line,position.source_column \
         FROM no_intro_dat_rom_field_positions AS position \
         JOIN no_intro_dat_rom_claims AS rom USING(occurrence_id) \
         WHERE rom.name='fixture-a.bin' ORDER BY position.source_order",
    )
    .load::<Position>(&mut imported.connection)?;
    assert!(!all_rom_positions.is_empty());
    assert_positions_are_source_ordered(&all_rom_positions);
    let rom_location = fixture_location(document, "<rom\n      name=\"fixture-a.bin\"");
    assert_eq!(
        rom_positions,
        [
            FieldPosition {
                field_kind: 0,
                position_domain: None,
                source_order: 0,
                source_line: rom_location.0,
                source_column: rom_location.1
            },
            FieldPosition {
                field_kind: 1,
                position_domain: None,
                source_order: 1,
                source_line: rom_location.0,
                source_column: rom_location.1
            },
            FieldPosition {
                field_kind: 5,
                position_domain: None,
                source_order: 5,
                source_line: rom_location.0,
                source_column: rom_location.1
            },
            FieldPosition {
                field_kind: 6,
                position_domain: None,
                source_order: 6,
                source_line: rom_location.0,
                source_column: rom_location.1
            },
            FieldPosition {
                field_kind: 7,
                position_domain: None,
                source_order: 7,
                source_line: rom_location.0,
                source_column: rom_location.1
            },
        ],
        "ROM attributes share their opening-tag location and retain attribute ordinals"
    );
    Ok(())
}

fn assert_child_positions(imported: &mut Imported) -> TestResult {
    for table in ["no_intro_dat_categories", "no_intro_dat_releases"] {
        let positions = sql_query(format!(
            "SELECT source_order,source_line,source_column FROM {table} \
             WHERE set_id=(SELECT game.set_id FROM no_intro_dat_games AS game \
                           JOIN catalog_sets AS sets USING(set_id) \
                           WHERE sets.set_name='fixture-game-a') \
             ORDER BY source_order"
        ))
        .load::<Position>(&mut imported.connection)?;
        assert_eq!(positions.len(), 2, "{table} on the first game");
        assert_positions_are_source_ordered(&positions);
    }
    Ok(())
}

fn assert_unfiltered_whole_file_claims_link(mode: NoIntroDatMode) -> TestResult {
    let shared_rom = format!(
        "<rom name='shared.bin' size='0' crc='{CRC}' md5='{MD5}' \
         sha1='{SHA1}' sha256='{SHA256}'/>"
    );
    let first_game = minimal_document(
        "",
        "",
        &format!("size='0' crc='{CRC}' md5='{MD5}' sha1='{SHA1}' sha256='{SHA256}'"),
    );
    let document = first_game.replace(
        "</datafile>",
        &format!(
            "<game name='second-game'><description>Second owner</description>{shared_rom}</game>\
             </datafile>"
        ),
    );
    let mut imported = Imported::new(&document, mode)?;
    let roms = sql_query(
        "SELECT rom.name,rom.size_text,rom.size,rom.status_text,rom.serial_text, \
                rom.header_text,rom.evidence_scope \
         FROM no_intro_dat_rom_claims AS rom ORDER BY rom.source_order",
    )
    .load::<RomFields>(&mut imported.connection)?;
    assert_eq!(roms.len(), 2);
    assert!(roms.iter().all(|rom| rom.evidence_scope == "whole_file"));
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM asset_occurrences \
             WHERE claim_kind='no_intro_dat_rom' AND content_uuid IS NOT NULL"
        )?,
        2
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(DISTINCT content_uuid) AS value FROM asset_occurrences \
             WHERE claim_kind='no_intro_dat_rom' AND content_uuid IS NOT NULL"
        )?,
        1,
        "matching whole-file claims across games share one content identity"
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM catalog_contents")?,
        1
    );
    Ok(())
}

fn decode_hex(spelling: &str) -> Vec<u8> {
    let (pairs, remainder) = spelling.as_bytes().as_chunks::<2>();
    assert!(
        remainder.is_empty(),
        "hex fixture contains complete byte pairs"
    );
    pairs
        .iter()
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex fixture is ASCII");
            u8::from_str_radix(pair, 16).expect("hex fixture contains valid digits")
        })
        .collect()
}

fn assert_positions_are_source_ordered(positions: &[Position]) {
    assert!(
        positions
            .iter()
            .all(|position| { position.order >= 0 && position.line > 0 && position.column > 0 })
    );
    assert!(
        positions
            .windows(2)
            .all(|pair| pair[0].order < pair[1].order)
    );
}

fn assert_source_recovery_survives_backup_restore(
    imported: Imported,
    original: &[u8],
) -> TestResult {
    let source_on_disk = std::fs::read(&imported.document_path)?;
    assert_eq!(source_on_disk.as_slice(), original);
    let snapshot = imported.snapshot_key.clone();
    let retained_before_backup = app::load_snapshot_source(&imported.database, &snapshot)?;
    assert_eq!(retained_before_backup.as_slice(), original);

    let directory = imported.directory.path().to_path_buf();
    let backup_path = utf8(directory.join("catalog-backup.sqlite"))?;
    let restored_path = utf8(directory.join("catalog-restored.sqlite"))?;
    drop(imported.connection);
    drop(imported.database);
    create_backup(&imported.database_path, &backup_path)?;
    assert!(matches!(
        restore_backup(&backup_path, &restored_path, RestorePolicy::CreateNew)?,
        RestoreOutcome::Published | RestoreOutcome::PublishedDurabilityUnconfirmed { .. }
    ));
    let restored_database = Database::open(&restored_path)?;
    let retained_after_restore = app::load_snapshot_source(&restored_database, &snapshot)?;
    assert_eq!(retained_after_restore.as_slice(), original);
    let mut restored_connection = SqliteConnection::establish(restored_path.as_str())?;
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_headers WHERE trademarks='Trademark text' AND piracy='Piracy text'")
            .get_result::<Count>(&mut restored_connection)?.value,
        1
    );
    Ok(())
}

fn minimal_document(
    date_before_author: &str,
    header_children_after_author: &str,
    rom_attributes: &str,
) -> String {
    format!(
        "<datafile><header><id>1</id><name>Minimal</name><description></description>\
         <version>v1</version>{date_before_author}<author>Publisher</author>\
         {header_children_after_author}</header>\
         <game name='minimal-game'><description></description>\
         <rom name='minimal.bin' {rom_attributes}/></game></datafile>"
    )
}

#[test]
fn strict_modes_preserve_empty_presence_defaults_invalid_hashes_and_absence() -> TestResult {
    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let document = minimal_document(
            "<date/>",
            "<homepage/><url/><subset/><clrmamepro/><romcenter/>",
            "size='0' crc='' md5='' sha1='' status='' serial='' header=''",
        );
        let mut imported = Imported::new(&document, mode)?;
        assert_empty_fields(&mut imported)?;
        assert_empty_hashes(&mut imported)?;
        assert_absent_fields(mode)?;
    }
    Ok(())
}

fn assert_empty_fields(imported: &mut Imported) -> TestResult {
    let options = sql_query(
        "SELECT clrmamepro.forcenodump_text,clrmamepro.forcenodump_effective, \
                    clrmamepro.header_text,romcenter.plugin_text \
             FROM no_intro_dat_clrmamepro_options AS clrmamepro \
             JOIN no_intro_dat_romcenter_options AS romcenter USING(snapshot_key)",
    )
    .get_result::<OptionsFields>(&mut imported.connection)?;
    assert_eq!(options.forcenodump_text, None);
    assert_eq!(options.forcenodump_effective, "obsolete");
    assert_eq!(options.header_text, None);
    assert_eq!(options.plugin_text, None);
    let empty_header = sql_query(
        "SELECT id_text,name,description,version_text,date,author,homepage,url, \
                    trademarks,piracy,subset FROM no_intro_dat_headers",
    )
    .get_result::<HeaderFields>(&mut imported.connection)?;
    assert_eq!(empty_header.date.as_deref(), Some(""));
    assert_eq!(empty_header.homepage.as_deref(), Some(""));
    assert_eq!(empty_header.url.as_deref(), Some(""));
    assert_eq!(empty_header.subset.as_deref(), Some(""));
    assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_dat_header_declared_field_presence WHERE field_kind IN (4,6,7,10)")?,
            4
        );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_dat_clrmamepro_declared_field_presence"
        )?,
        0
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM no_intro_dat_romcenter_declared_field_presence"
        )?,
        0
    );
    assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM no_intro_dat_rom_declared_field_presence WHERE field_kind IN (6,7,8)")?,
            3,
            "ROM status is declared and serial/header remain explicitly present"
        );
    assert_eq!(
        sql_query("SELECT status_text AS value FROM no_intro_dat_rom_claims")
            .get_result::<NullableText>(&mut imported.connection)?
            .value
            .as_deref(),
        Some("")
    );

    Ok(())
}

fn assert_empty_hashes(imported: &mut Imported) -> TestResult {
    let bad_hashes = sql_query(
            "SELECT CASE field_kind WHEN 2 THEN 'crc32' WHEN 3 THEN 'md5' WHEN 4 THEN 'sha1' END AS algorithm, \
                    invalid_text FROM no_intro_dat_rom_digest_fields WHERE invalid_text IS NOT NULL \
             ORDER BY field_kind",
        )
        .load::<InvalidDigest>(&mut imported.connection)?;
    assert_eq!(bad_hashes.len(), 3);
    assert_eq!(
        bad_hashes
            .iter()
            .map(|field| (field.algorithm.as_str(), field.invalid_text.as_str()))
            .collect::<Vec<_>>(),
        [("crc32", ""), ("md5", ""), ("sha1", "")]
    );
    assert_eq!(
        imported.count("SELECT COUNT(*) AS value FROM catalog_contents")?,
        0
    );
    assert_eq!(
        imported.count(
            "SELECT COUNT(*) AS value FROM asset_occurrences WHERE content_uuid IS NOT NULL"
        )?,
        0
    );

    Ok(())
}

fn assert_absent_fields(mode: NoIntroDatMode) -> TestResult {
    let mut absent = Imported::new(
        &minimal_document("", "", "size='0' crc='' md5='' sha1=''"),
        mode,
    )?;
    let mut absent_connection = SqliteConnection::establish(absent.database_path.as_str())?;
    let header = sql_query(
        "SELECT id_text,name,description,version_text,date,author,homepage,url, \
                    trademarks,piracy,subset FROM no_intro_dat_headers",
    )
    .get_result::<HeaderFields>(&mut absent_connection)?;
    assert_eq!(header.date, None);
    assert_eq!(header.homepage, None);
    assert_eq!(header.url, None);
    assert_eq!(header.subset, None);
    let game = sql_query(
            "SELECT sets.set_name,game.id_text,clone.target_literal AS cloneof_text,parent_id.target_literal AS cloneofid_text,game.description_text \
             FROM no_intro_dat_games AS game JOIN catalog_sets AS sets USING(set_id) \
             LEFT JOIN no_intro_dat_set_links AS clone ON clone.set_id=game.set_id AND clone.link_kind='cloneof' \
             LEFT JOIN no_intro_dat_set_links AS parent_id ON parent_id.set_id=game.set_id AND parent_id.link_kind='cloneofid'",
        )
        .get_result::<GameFields>(&mut absent_connection)?;
    assert_eq!(game.set_name, "minimal-game");
    assert_eq!(game.id_text, None);
    assert_eq!(game.cloneof_text, None);
    assert_eq!(game.cloneofid_text, None);
    assert_eq!(game.description_text.as_deref(), Some(""));
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_categories")
            .get_result::<Count>(&mut absent_connection)?
            .value,
        0
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM no_intro_dat_releases")
            .get_result::<Count>(&mut absent_connection)?
            .value,
        0
    );
    assert_eq!(
        sql_query("SELECT status_text AS value FROM no_intro_dat_rom_claims")
            .get_result::<NullableText>(&mut absent_connection)?
            .value,
        None
    );
    assert_eq!(
        sql_query("SELECT size AS value FROM no_intro_dat_rom_claims")
            .get_result::<NullableInteger>(&mut absent_connection)?
            .value,
        Some(0)
    );
    assert_eq!(
        absent.count(
            "SELECT COUNT(*) AS value FROM no_intro_dat_header_field_positions \
                 WHERE field_kind IN (4,6,7,8,9,10)"
        )?,
        0,
        "absent optional header fields do not manufacture source positions"
    );
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS value FROM no_intro_dat_rom_declared_field_presence \
                 WHERE field_kind IN (6,7,8,9,10)",
        )
        .get_result::<Count>(&mut absent_connection)?
        .value,
        0,
        "absent optional ROM fields do not manufacture source presence"
    );
    Ok(())
}

#[test]
fn strict_v3_rejects_v4_header_extensions_and_both_revisions_enforce_integer_widths() -> TestResult
{
    let v4_extensions = minimal_document(
        "",
        "<trademarks>Marks</trademarks><piracy>Notice</piracy>",
        "size='0' crc='' md5='' sha1=''",
    );
    let mut imported = Imported::new(&v4_extensions, NoIntroDatMode::V4Strict)?;
    assert_eq!(imported.count("SELECT COUNT(*) AS value FROM no_intro_dat_headers WHERE trademarks='Marks' AND piracy='Notice'")?, 1);

    let v3_extensions = v4_extensions;
    let directory = tempfile::tempdir()?;
    let database_path = utf8(directory.path().join("v3-reject.sqlite"))?;
    let source_path = utf8(directory.path().join("v3-reject.dat"))?;
    std::fs::write(&source_path, v3_extensions)?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: source_path,
            format: CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V3Strict),
            source_key: PublishingSourceKey::new("no-intro-v3-extension-rejection"),
            source_display_name: "No-Intro v3 extension rejection".to_owned(),
            catalog_key: CatalogKey::new("no-intro-v3-extension-rejection"),
            catalog_display_name: "No-Intro v3 extension rejection".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);

    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let valid = minimal_document("", "", "size='0' crc='' md5='' sha1=''")
            .replace("size='0'", "size='4294967295'")
            .replace("<id>1</id>", "<id>2147483647</id>");
        let imported = Imported::new(&valid, mode)?;
        let mut connection = SqliteConnection::establish(imported.database_path.as_str())?;
        assert_eq!(
            sql_query("SELECT id_text AS value FROM no_intro_dat_headers")
                .get_result::<NullableText>(&mut connection)?
                .value
                .as_deref(),
            Some("2147483647"),
            "mode={mode:?} accepts xs:int maximum"
        );
        assert_eq!(
            sql_query("SELECT size AS value FROM no_intro_dat_rom_claims")
                .get_result::<NullableInteger>(&mut connection)?
                .value,
            Some(4_294_967_295),
            "mode={mode:?} accepts xs:unsignedInt maximum"
        );

        for (header_id, size) in [("2147483648", "0"), ("1", "4294967296")] {
            let invalid = minimal_document("", "", "size='0' crc='' md5='' sha1=''")
                .replace("<id>1</id>", &format!("<id>{header_id}</id>"))
                .replace("size='0'", &format!("size='{size}'"));
            let directory = tempfile::tempdir()?;
            let database_path = utf8(directory.path().join("range-reject.sqlite"))?;
            let source_path = utf8(directory.path().join("range-reject.dat"))?;
            std::fs::write(&source_path, invalid)?;
            let database = Database::open(&database_path)?;
            let report = app::import_catalog(
                &database,
                &CatalogImportRequest {
                    document_path: source_path,
                    format: CatalogDocumentFormat::NoIntroDat(mode),
                    source_key: PublishingSourceKey::new(format!(
                        "range-reject-{}-{header_id}-{size}",
                        mode.as_str()
                    )),
                    source_display_name: "No-Intro numeric boundary rejection".to_owned(),
                    catalog_key: CatalogKey::new(format!(
                        "range-reject-{}-{header_id}-{size}",
                        mode.as_str()
                    )),
                    catalog_display_name: "No-Intro numeric boundary rejection".to_owned(),
                    scope: CatalogScope::Complete,
                },
            )?;
            assert_eq!(
                report.status,
                CatalogImportStatus::Failed,
                "mode={mode:?}, id={header_id}, size={size}"
            );
        }
    }
    Ok(())
}

#[test]
fn strict_modes_accept_unconstrained_invalid_hash_lexemes_without_matching_them() -> TestResult {
    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let invalid = minimal_document(
            "",
            "",
            "size='0' crc='bad-crc' md5='bad md5' sha1='bad-sha1'",
        );
        let mut imported = Imported::new(&invalid, mode)?;
        let bad_hashes = sql_query(
            "SELECT CASE field_kind WHEN 2 THEN 'crc32' WHEN 3 THEN 'md5' WHEN 4 THEN 'sha1' END AS algorithm, \
                    invalid_text FROM no_intro_dat_rom_digest_fields WHERE invalid_text IS NOT NULL \
             ORDER BY field_kind",
        )
        .load::<InvalidDigest>(&mut imported.connection)?;
        assert_eq!(bad_hashes.len(), 3);
        assert_eq!(bad_hashes[0].invalid_text, "bad-crc");
        assert_eq!(bad_hashes[1].invalid_text, "bad md5");
        assert_eq!(bad_hashes[2].invalid_text, "bad-sha1");
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM occurrence_digest_assertions")?,
            0
        );
        assert_eq!(
            imported.count("SELECT COUNT(*) AS value FROM catalog_contents")?,
            0
        );
        assert_eq!(
            imported.count(
                "SELECT COUNT(*) AS value FROM asset_occurrences WHERE content_uuid IS NOT NULL"
            )?,
            0
        );
    }
    Ok(())
}
