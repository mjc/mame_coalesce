use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const HEADER: &str = "<header><id>1</id><name>Catalog</name><description>Catalog</description>\
    <version>1</version><author>Publisher</author></header>";
const ROM: &str = "<rom name='same.bin' size='4' crc='12345678' md5='' \
    sha1='0123456789012345678901234567890123456789'/>";

struct Catalog {
    _directory: tempfile::TempDir,
    database: Database,
    request: CatalogImportRequest,
    connection: SqliteConnection,
}

impl Catalog {
    fn new(body: &str, mode: NoIntroDatMode) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "database path is not UTF-8")?;
        let document_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.dat"))
            .map_err(|_| "document path is not UTF-8")?;
        std::fs::write(&document_path, body)?;
        let database = Database::open(&database_path)?;
        let connection = SqliteConnection::establish(database_path.as_str())?;
        let request = CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroDat(mode),
            source_key: PublishingSourceKey::new("no-intro-native"),
            source_display_name: "No-Intro native".into(),
            catalog_key: CatalogKey::new("no-intro-native"),
            catalog_display_name: "No-Intro native".into(),
            scope: CatalogScope::Complete,
        };
        Ok(Self {
            _directory: directory,
            database,
            request,
            connection,
        })
    }

    fn count(&mut self, query: &str) -> Result<i64, diesel::result::Error> {
        Ok(sql_query(query)
            .get_result::<Count>(&mut self.connection)?
            .count)
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TextFact {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

#[derive(QueryableByName)]
struct RomFacts {
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
}

#[test]
fn strict_zero_game_documents_publish_a_native_root() -> TestResult {
    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let mut catalog = Catalog::new(&format!("<datafile>{HEADER}</datafile>"), mode)?;
        let report = app::import_catalog(&catalog.database, &catalog.request)?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        assert_eq!(
            catalog.count("SELECT COUNT(*) AS count FROM catalog_sets")?,
            0
        );
        assert_eq!(
            catalog.count("SELECT COUNT(*) AS count FROM catalog_set_groups")?,
            1
        );
        assert_eq!(
            catalog.count("SELECT COUNT(*) AS count FROM snapshot_publications")?,
            1
        );
    }
    Ok(())
}

#[test]
fn strict_and_compatible_interpretations_do_not_share_snapshots() -> TestResult {
    let mut catalog = Catalog::new(
        &format!("<datafile>{HEADER}</datafile>"),
        NoIntroDatMode::V3Strict,
    )?;
    let mut keys = Vec::new();
    for mode in [
        NoIntroDatMode::V3Strict,
        NoIntroDatMode::V3Compatible,
        NoIntroDatMode::V4Strict,
        NoIntroDatMode::V4Compatible,
    ] {
        catalog.request.format = CatalogDocumentFormat::NoIntroDat(mode);
        let report = app::import_catalog(&catalog.database, &catalog.request)?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        keys.push(report.snapshot_key.ok_or("missing snapshot")?);
    }
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM parser_interpretations")?,
        4
    );
    keys.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    keys.dedup();
    assert_eq!(keys.len(), 4);
    Ok(())
}

#[test]
fn compatible_multiple_roms_are_native_occurrences_not_logiqx_games() -> TestResult {
    let body = format!(
        "<datafile>{HEADER}<game name='same' id='0001'>\
        <description>Game</description><game_id>0002</game_id>{ROM}{ROM}\
        <release name='' region=''/></game></datafile>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V3Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    assert_eq!(catalog.count("SELECT COUNT(*) AS count FROM catalog_sets WHERE source_element_kind = 'no_intro_dat_game'")?, 1);
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM asset_occurrences WHERE claim_kind = 'no_intro_dat_rom'"
        )?,
        2
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM logiqx_games")?,
        0
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM asset_requirement_rows")?,
        2
    );
    Ok(())
}

#[test]
fn strict_rejects_compatible_deviations_without_leaking_native_rows() -> TestResult {
    let game = format!("<game name='same'><description>Game</description>{ROM}{ROM}</game>");
    let mut catalog = Catalog::new(
        &format!("<datafile>{HEADER}{game}</datafile>"),
        NoIntroDatMode::V4Strict,
    )?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_snapshots")?,
        0
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_sets")?,
        0
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM asset_occurrences")?,
        0
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM import_diagnostics")?,
        1
    );
    Ok(())
}

#[test]
fn late_xml_failure_rolls_back_already_streamed_roms_and_file_identities() -> TestResult {
    let body = format!(
        "<datafile>{HEADER}<game name='same'><description>Game</description>\
        {ROM}</game></datafile><other/>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    for query in [
        "SELECT COUNT(*) AS count FROM catalog_snapshots",
        "SELECT COUNT(*) AS count FROM catalog_sets",
        "SELECT COUNT(*) AS count FROM asset_occurrences",
        "SELECT COUNT(*) AS count FROM catalog_contents",
        "SELECT COUNT(*) AS count FROM occurrence_digest_assertions",
    ] {
        assert_eq!(catalog.count(query)?, 0, "{query}");
    }
    Ok(())
}

#[test]
fn identical_reimport_is_idempotent_but_records_each_acquisition() -> TestResult {
    let body = format!(
        "<datafile>{HEADER}<game name='same'><description>Game</description>{ROM}</game></datafile>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Strict)?;
    let first = app::import_catalog(&catalog.database, &catalog.request)?;
    let second = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(first.status, CatalogImportStatus::Succeeded);
    assert_eq!(second.status, CatalogImportStatus::Succeeded);
    assert_eq!(first.snapshot_key, second.snapshot_key);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_sets")?,
        1
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM acquisitions")?,
        2
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM import_runs")?,
        2
    );
    Ok(())
}

#[test]
fn compatible_native_fields_keep_lexemes_presence_and_document_filter_scope() -> TestResult {
    let body = "<datafile><header><id>0007</id><name> Catalog </name>\
        <description><![CDATA[ Description ]]></description><version> v4 </version>\
        <date/><author>Author</author><homepage/><url>URL</url><trademarks/>\
        <piracy>Piracy</piracy><subset>Subset</subset><comment>Comment</comment>\
        <clrmamepro forcenodump=' required ' header=''/><romcenter plugin=''/>\
        </header><game name='Game' id='0008' cloneof='Parent' cloneofid='0001'>\
        <category/><category>Category</category><description>Game description</description>\
        <game_id>0009</game_id><game_id>0010</game_id>\
        <rom name='file.bin' size=' +4294967296 ' crc='12345678' md5='invalid' \
         sha1='0123456789012345678901234567890123456789' sha256='' status='custom' \
         serial='' date='Date' mia='yes'/><release name='' region='Region'/>\
        </game></datafile>";
    let mut catalog = Catalog::new(body, NoIntroDatMode::V4Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(
        report.status,
        CatalogImportStatus::Succeeded,
        "{:?}",
        sql_query("SELECT message AS value FROM import_diagnostics")
            .load::<TextFact>(&mut catalog.connection)?
            .into_iter()
            .map(|fact| fact.value)
            .collect::<Vec<_>>()
    );
    assert_native_text_fields(&mut catalog)?;
    let rom = sql_query("SELECT size_text,size,invalid_text AS md5_text,evidence_scope FROM no_intro_dat_rom_claims LEFT JOIN no_intro_dat_rom_digest_fields USING(occurrence_id) WHERE field_kind=3")
        .get_result::<RomFacts>(&mut catalog.connection)?;
    assert_eq!(rom.size_text.as_deref(), Some(" +4294967296 "));
    assert_eq!(rom.size, Some(4_294_967_296));
    assert_eq!(rom.md5_text.as_deref(), Some("invalid"));
    assert_eq!(rom.evidence_scope, "unknown");
    for (query, expected) in [
        ("SELECT COUNT(*) AS count FROM catalog_contents", 0),
        (
            "SELECT COUNT(*) AS count FROM digest_values WHERE algorithm = 'md5'",
            0,
        ),
        ("SELECT COUNT(*) AS count FROM no_intro_dat_categories", 2),
        ("SELECT COUNT(*) AS count FROM no_intro_dat_identifiers", 2),
        ("SELECT COUNT(*) AS count FROM no_intro_dat_releases", 1),
    ] {
        assert_eq!(catalog.count(query)?, expected, "{query}");
    }
    Ok(())
}

fn assert_native_text_fields(catalog: &mut Catalog) -> TestResult {
    for (query, expected) in [
        (
            "SELECT id_text AS value FROM no_intro_dat_headers",
            Some("0007"),
        ),
        (
            "SELECT description AS value FROM no_intro_dat_headers",
            Some(" Description "),
        ),
        (
            "SELECT version_text AS value FROM no_intro_dat_headers",
            Some(" v4 "),
        ),
        (
            "SELECT declared_version AS value FROM catalog_snapshots",
            None,
        ),
        (
            "SELECT trademarks AS value FROM no_intro_dat_headers",
            Some(""),
        ),
        (
            "SELECT comment AS value FROM no_intro_dat_headers",
            Some("Comment"),
        ),
        (
            "SELECT header_text AS value FROM no_intro_dat_clrmamepro_options",
            Some(""),
        ),
        (
            "SELECT forcenodump_text AS value FROM no_intro_dat_clrmamepro_options",
            Some(" required "),
        ),
        (
            "SELECT plugin_text AS value FROM no_intro_dat_romcenter_options",
            Some(""),
        ),
        (
            "SELECT cloneofid_text AS value FROM no_intro_dat_games",
            Some("0001"),
        ),
        (
            "SELECT serial_text AS value FROM no_intro_dat_rom_claims",
            Some(""),
        ),
        (
            "SELECT header_text AS value FROM no_intro_dat_rom_claims",
            None,
        ),
        (
            "SELECT mia_text AS value FROM no_intro_dat_rom_claims",
            Some("yes"),
        ),
    ] {
        let fact = sql_query(query).get_result::<TextFact>(&mut catalog.connection)?;
        assert_eq!(fact.value.as_deref(), expected, "{query}");
    }
    Ok(())
}

#[test]
fn compatible_empty_size_does_not_become_a_fabricated_zero() -> TestResult {
    let body = format!(
        "<datafile>{HEADER}<game name='Game'><description>Game</description>\
        <rom name='empty' size='' crc='' md5='' sha1=''/></game></datafile>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let rom = sql_query("SELECT size_text,size,invalid_text AS md5_text,evidence_scope FROM no_intro_dat_rom_claims LEFT JOIN no_intro_dat_rom_digest_fields USING(occurrence_id) WHERE field_kind=3")
        .get_result::<RomFacts>(&mut catalog.connection)?;
    assert_eq!(rom.size_text.as_deref(), Some(""));
    assert_eq!(rom.size, None);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_contents")?,
        0
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM occurrence_digest_assertions")?,
        0
    );
    Ok(())
}

#[test]
fn uninterpretable_supplied_evidence_does_not_issue_a_file_uuid() -> TestResult {
    for declaration in [
        "size=''",
        "size='9223372036854775808'",
        "crc='nope'",
        "md5=''",
        "sha256='not-a-hash'",
    ] {
        let body = format!(
            "<datafile>{HEADER}<game name='Game'><description>Game</description>\
            <rom name='file.bin' sha1='0123456789012345678901234567890123456789' {declaration}/>\
            </game></datafile>"
        );
        let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Compatible)?;
        let report = app::import_catalog(&catalog.database, &catalog.request)?;
        assert_eq!(
            report.status,
            CatalogImportStatus::Succeeded,
            "{declaration}"
        );
        assert_eq!(
            catalog.count("SELECT COUNT(*) AS count FROM catalog_contents")?,
            0,
            "{declaration}"
        );
        assert_eq!(
            catalog.count(
                "SELECT COUNT(*) AS count FROM asset_occurrences WHERE content_uuid IS NOT NULL"
            )?,
            0,
            "{declaration}"
        );
        assert_eq!(catalog.count("SELECT COUNT(*) AS count FROM occurrence_digest_assertions JOIN digest_values USING(digest_id) WHERE algorithm='sha1'")?, 1, "{declaration}");
    }
    // Missing optional declarations are not uninterpretable supplied evidence.
    let body = format!(
        "<datafile>{HEADER}<game name='Game'><description>Game</description>\
        <rom name='file.bin' sha1='0123456789012345678901234567890123456789'/>\
        </game></datafile>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_contents")?,
        1
    );
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM asset_occurrences WHERE content_uuid IS NOT NULL"
        )?,
        1
    );
    Ok(())
}

#[test]
fn valid_hash_payloads_are_interned_once_and_invalid_literals_have_native_owners() -> TestResult {
    let rom = "<rom name='file.bin' crc='ABCDEF01' md5='0123456789ABCDEF0123456789ABCDEF' \
        sha1='0123456789ABCDEF0123456789ABCDEF01234567' \
        sha256='0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF'/>";
    let body = format!(
        "<datafile>{HEADER}<game name='Game'><description>Game</description>\
        {rom}{rom}<rom name='invalid.bin' md5='not a hash'/></game></datafile>"
    );
    let mut catalog = Catalog::new(&body, NoIntroDatMode::V4Compatible)?;
    let report = app::import_catalog(&catalog.database, &catalog.request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    assert_eq!(catalog.count("SELECT COUNT(*) AS count FROM pragma_table_info('no_intro_dat_rom_claims') WHERE name IN ('crc_text','md5_text','sha1_text','sha256_text')")?, 0);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM digest_values")?,
        4
    );
    assert_eq!(catalog.count("SELECT COUNT(*) AS count FROM no_intro_dat_rom_digest_fields WHERE digest_id IS NOT NULL AND invalid_text IS NULL")?, 8);
    assert_eq!(catalog.count("SELECT COUNT(*) AS count FROM no_intro_dat_rom_digest_fields WHERE digest_id IS NULL AND invalid_text='not a hash'")?, 1);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM occurrence_digest_assertions")?,
        8
    );
    Ok(())
}
