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

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Imported {
    _directory: tempfile::TempDir,
    _database: Database,
    connection: SqliteConnection,
}

impl Imported {
    fn new(document: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "database path is not UTF-8")?;
        let document_path = Utf8PathBuf::from_path_buf(directory.path().join("source.dat"))
            .map_err(|_| "document path is not UTF-8")?;
        std::fs::write(&document_path, document)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
                source_key: PublishingSourceKey::new("no-intro-native-publication"),
                source_display_name: "No-Intro native publication".into(),
                catalog_key: CatalogKey::new("no-intro-native-publication"),
                catalog_display_name: "No-Intro native publication".into(),
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
            .count)
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(Clone, Copy)]
enum BrokenPublication {
    MissingHeader,
    MissingClrMameProOwner,
    MissingHeaderPosition,
    IncorrectCounts,
}

#[derive(Clone, Copy)]
enum RomTamper {
    SourceDigestMismatch,
    NoncontiguousRank,
    LinkedEvidence,
}

fn stage_broken_publication(catalog: &mut Imported, broken: BrokenPublication) -> TestResult {
    sql_query(
        "INSERT INTO catalogs(catalog_key,source_key,display_name) \
         SELECT 'pending-native',catalogs.source_key,'Pending native' \
         FROM snapshot_publications JOIN catalogs USING(catalog_key) LIMIT 1",
    )
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
         SELECT 'pending-native','pending-native',document_key,interpretation_key,coverage_id \
         FROM catalog_snapshots WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
    )
    .execute(&mut catalog.connection)?;
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending-native','root',0)")
        .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_documents(snapshot_key,schema_location,source_line,source_column) \
         SELECT 'pending-native',schema_location,source_line,source_column \
         FROM no_intro_dat_documents",
    )
    .execute(&mut catalog.connection)?;
    if !matches!(broken, BrokenPublication::MissingHeader) {
        sql_query(
            "INSERT INTO no_intro_dat_headers(snapshot_key,source_order,source_line,source_column, \
             id_text,name,description,version_text,date,author,homepage,url,trademarks,piracy,subset,comment) \
             SELECT 'pending-native',source_order,source_line,source_column,id_text,name,description,version_text, \
             date,author,homepage,url,trademarks,piracy,subset,comment FROM no_intro_dat_headers",
        )
        .execute(&mut catalog.connection)?;
        if !matches!(broken, BrokenPublication::MissingClrMameProOwner) {
            sql_query(
                "INSERT INTO no_intro_dat_clrmamepro_options(snapshot_key,source_order,source_line,source_column,forcenodump_text,header_text) \
                 SELECT 'pending-native',source_order,source_line,source_column,forcenodump_text,header_text \
                 FROM no_intro_dat_clrmamepro_options",
            )
            .execute(&mut catalog.connection)?;
        }
        sql_query(
            "INSERT INTO no_intro_dat_romcenter_options(snapshot_key,source_order,source_line,source_column,plugin_text) \
             SELECT 'pending-native',source_order,source_line,source_column,plugin_text FROM no_intro_dat_romcenter_options",
        )
        .execute(&mut catalog.connection)?;
        sql_query(
            "INSERT INTO no_intro_dat_header_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) \
             SELECT 'pending-native',field_kind,source_order,source_line,source_column \
             FROM no_intro_dat_header_field_positions WHERE NOT (? AND field_kind=1)",
        )
        .bind::<BigInt, _>(i64::from(matches!(broken, BrokenPublication::MissingHeaderPosition)))
        .execute(&mut catalog.connection)?;
    }
    sql_query(
        "INSERT INTO no_intro_dat_parse_counts(snapshot_key,game_count,rom_count,category_count,identifier_count,release_count, \
         clrmamepro_option_count,romcenter_option_count,header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count) \
         SELECT 'pending-native',game_count,rom_count,category_count,identifier_count,release_count, \
         clrmamepro_option_count,romcenter_option_count,header_field_count+?,clrmamepro_field_count, \
         romcenter_field_count,game_field_count,rom_field_count FROM no_intro_dat_parse_counts",
    )
    .bind::<BigInt, _>(i64::from(matches!(broken, BrokenPublication::IncorrectCounts)))
    .execute(&mut catalog.connection)?;
    Ok(())
}

fn try_publish_pending(catalog: &mut Imported) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) \
         SELECT catalog_key,document_key,interpretation_key,snapshot_key \
         FROM catalog_snapshots WHERE snapshot_key='pending-native'",
    )
    .execute(&mut catalog.connection)
}

fn stage_pending_rom(catalog: &mut Imported, tamper: RomTamper) -> TestResult {
    sql_query(
        "INSERT INTO catalogs(catalog_key,source_key,display_name) \
         SELECT 'pending-native',catalogs.source_key,'Pending native' \
         FROM snapshot_publications JOIN catalogs USING(catalog_key) LIMIT 1",
    )
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
         SELECT 'pending-native','pending-native',document_key,interpretation_key,coverage_id \
         FROM catalog_snapshots WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
    )
    .execute(&mut catalog.connection)?;
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending-native','root',0)")
        .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_documents(snapshot_key,schema_location,source_line,source_column) \
         SELECT 'pending-native',schema_location,source_line,source_column FROM no_intro_dat_documents",
    )
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_headers(snapshot_key,source_order,source_line,source_column, \
         id_text,name,description,version_text,date,author,homepage,url,trademarks,piracy,subset,comment) \
         SELECT 'pending-native',source_order,source_line,source_column,id_text,name,description,version_text, \
         date,author,homepage,url,trademarks,piracy,subset,comment FROM no_intro_dat_headers",
    )
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_header_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) \
         SELECT 'pending-native',field_kind,source_order,source_line,source_column FROM no_intro_dat_header_field_positions",
    )
    .execute(&mut catalog.connection)?;
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         SELECT (SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key='pending-native' AND kind='root'), \
         source_element_kind,list_order,set_name,source_line,source_column FROM catalog_sets LIMIT 1 \
         RETURNING set_id AS count",
    )
    .get_result::<Count>(&mut catalog.connection)?
    .count;
    sql_query(
        "INSERT INTO no_intro_dat_games(set_id,source_order,id_text,description_text) \
         SELECT ?,source_order,id_text,description_text FROM no_intro_dat_games",
    )
    .bind::<BigInt, _>(set_id)
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_game_field_positions(set_id,field_kind,source_order,source_line,source_column) \
         SELECT ?,field_kind,source_order,source_line,source_column FROM no_intro_dat_game_field_positions",
    )
    .bind::<BigInt, _>(set_id)
    .execute(&mut catalog.connection)?;
    let occurrence_order = i64::from(matches!(tamper, RomTamper::NoncontiguousRank));
    if matches!(tamper, RomTamper::LinkedEvidence) {
        sql_query("INSERT INTO catalog_contents(content_uuid) SELECT x'00000000000000000000000000000000' WHERE NOT EXISTS(SELECT 1 FROM catalog_contents)")
            .execute(&mut catalog.connection)?;
    }
    let occurrence_id = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         SELECT ?,(SELECT occurrence_order FROM asset_occurrences LIMIT 1)+?,'no_intro_dat_rom', \
         CASE WHEN ? THEN (SELECT content_uuid FROM catalog_contents LIMIT 1) END \
         RETURNING occurrence_id AS count",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(occurrence_order)
    .bind::<BigInt, _>(i64::from(matches!(tamper, RomTamper::LinkedEvidence)))
    .get_result::<Count>(&mut catalog.connection)?
    .count;
    copy_pending_rom_payload(catalog, occurrence_id)?;
    sql_query(
        "INSERT INTO no_intro_dat_parse_counts(snapshot_key,game_count,rom_count,category_count,identifier_count,release_count, \
         clrmamepro_option_count,romcenter_option_count,header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count) \
         SELECT 'pending-native',game_count,rom_count,category_count,identifier_count,release_count,clrmamepro_option_count, \
         romcenter_option_count,header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count \
         FROM no_intro_dat_parse_counts",
    )
    .execute(&mut catalog.connection)?;
    if matches!(tamper, RomTamper::SourceDigestMismatch) {
        sql_query("INSERT INTO digest_values(algorithm,digest) VALUES('md5',x'00000000000000000000000000000000')")
            .execute(&mut catalog.connection)?;
        sql_query(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
             SELECT ?,digest_id,'whole_file','source_declared' FROM digest_values \
             WHERE algorithm='md5' AND digest=x'00000000000000000000000000000000'",
        )
        .bind::<BigInt, _>(occurrence_id)
        .execute(&mut catalog.connection)?;
    }
    Ok(())
}

fn copy_pending_rom_payload(catalog: &mut Imported, occurrence_id: i64) -> TestResult {
    sql_query(
        "INSERT INTO no_intro_dat_rom_claims(occurrence_id,name,size_text, \
         status_text,serial_text,header_text,date_text,mia_text,evidence_scope,evidence_provenance,source_order,source_line,source_column) \
         SELECT ?,name,size_text,status_text,serial_text,header_text,date_text,mia_text, \
         evidence_scope,evidence_provenance,source_order,source_line,source_column FROM no_intro_dat_rom_claims",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
         SELECT ?,digest_id,scope,provenance FROM occurrence_digest_assertions \
         WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM asset_occurrences)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_rom_digest_fields(occurrence_id,field_kind,digest_id,invalid_text) \
         SELECT ?,field_kind,digest_id,invalid_text FROM no_intro_dat_rom_digest_fields \
         WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM asset_occurrences)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(&mut catalog.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_rom_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) \
         SELECT ?,field_kind,source_order,source_line,source_column FROM no_intro_dat_rom_field_positions",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(&mut catalog.connection)?;
    Ok(())
}

#[derive(QueryableByName)]
struct RomEvidence {
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Nullable<Text>)]
    content_uuid: Option<String>,
}

#[test]
fn published_no_intro_rows_reject_replace_with_recursive_triggers_disabled() -> TestResult {
    let mut catalog = Imported::new(
        "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version></header>\
         <game name='Game'><description>Game</description><category>Arcade</category>\
         <rom name='file.bin' size='4'/></game></datafile>",
    )?;
    sql_query("PRAGMA recursive_triggers=OFF").execute(&mut catalog.connection)?;

    let secondary_replacement = sql_query(
        "INSERT OR REPLACE INTO no_intro_dat_categories \
         (set_id,category_order,category,source_order,source_line,source_column) \
         SELECT set_id,99,'replacement',source_order,source_line,source_column \
         FROM no_intro_dat_categories WHERE category_order=0",
    )
    .execute(&mut catalog.connection);
    assert!(secondary_replacement.is_err());

    let primary_replacement = sql_query(
        "INSERT OR REPLACE INTO no_intro_dat_rom_claims \
         (occurrence_id,name,size_text,evidence_scope,source_order,source_line,source_column) \
         SELECT occurrence_id,'replacement',size_text,evidence_scope,source_order,source_line,source_column \
         FROM no_intro_dat_rom_claims",
    )
    .execute(&mut catalog.connection);
    assert!(primary_replacement.is_err());
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM no_intro_dat_categories WHERE category='Arcade'"
        )?,
        1
    );
    assert_eq!(
        catalog
            .count("SELECT COUNT(*) AS count FROM no_intro_dat_rom_claims WHERE name='file.bin'")?,
        1
    );
    Ok(())
}

#[test]
fn publication_rejects_missing_native_owner_position_and_parser_count_mismatch() -> TestResult {
    for broken in [
        BrokenPublication::MissingHeader,
        BrokenPublication::MissingClrMameProOwner,
        BrokenPublication::MissingHeaderPosition,
        BrokenPublication::IncorrectCounts,
    ] {
        let mut catalog = Imported::new(
            "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version>\
             <clrmamepro/><romcenter/></header></datafile>",
        )?;
        stage_broken_publication(&mut catalog, broken)?;
        assert!(try_publish_pending(&mut catalog).is_err());
        assert_eq!(catalog.count(
            "SELECT COUNT(*) AS count FROM snapshot_publications WHERE snapshot_key='pending-native'",
        )?, 0);
    }
    Ok(())
}

#[test]
fn publication_rejects_unowned_hash_assertions_and_noncontiguous_rom_ranks() -> TestResult {
    for tamper in [
        RomTamper::SourceDigestMismatch,
        RomTamper::NoncontiguousRank,
    ] {
        let mut catalog = Imported::new(
            "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version></header>\
             <game name='Game'><description>Game</description><rom name='file.bin'/></game></datafile>",
        )?;
        stage_pending_rom(&mut catalog, tamper)?;
        assert!(try_publish_pending(&mut catalog).is_err());
        assert_eq!(catalog.count(
            "SELECT COUNT(*) AS count FROM snapshot_publications WHERE snapshot_key='pending-native'",
        )?, 0);
    }
    Ok(())
}

#[test]
fn global_filter_keeps_hash_evidence_unknown_and_prevents_content_identity() -> TestResult {
    let mut catalog = Imported::new(
        "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version>\
         <clrmamepro header=''/></header><game name='Game'><description>Game</description>\
         <rom name='file.bin' sha1='0123456789012345678901234567890123456789'/></game></datafile>",
    )?;
    let evidence = sql_query(
        "SELECT rom.size_text,rom.size,rom.evidence_scope, \
         CASE WHEN occurrence.content_uuid IS NOT NULL THEN hex(occurrence.content_uuid) END AS content_uuid \
         FROM no_intro_dat_rom_claims AS rom JOIN asset_occurrences AS occurrence USING(occurrence_id)",
    )
    .get_result::<RomEvidence>(&mut catalog.connection)?;
    assert_eq!(evidence.evidence_scope, "unknown");
    assert_eq!(evidence.content_uuid, None);
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM occurrence_digest_assertions AS assertion \
         JOIN digest_values USING(digest_id) WHERE algorithm='sha1' AND scope='unknown' \
         AND provenance='source_declared'",
        )?,
        1
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM catalog_contents")?,
        0
    );
    Ok(())
}

#[test]
fn invalid_size_lexeme_is_retained_without_a_truncated_numeric_claim() -> TestResult {
    let mut catalog = Imported::new(
        "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version></header>\
         <game name='Game'><description>Game</description>\
         <rom name='overflow.bin' size=' +9223372036854775808 '/></game></datafile>",
    )?;
    let evidence = sql_query(
        "SELECT size_text,size,evidence_scope,NULL AS content_uuid FROM no_intro_dat_rom_claims",
    )
    .get_result::<RomEvidence>(&mut catalog.connection)?;
    assert_eq!(
        evidence.size_text.as_deref(),
        Some(" +9223372036854775808 ")
    );
    assert_eq!(evidence.size, None);
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM snapshot_publications")?,
        1
    );
    Ok(())
}

#[test]
fn root_rank_source_position_and_mixed_game_field_domains_stay_distinct() -> TestResult {
    let mut catalog = Imported::new(
        "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version></header>\
         <game name='Game'><description>Game description</description>\
         <rom name='first.bin'/><rom name='second.bin'/></game></datafile>",
    )?;
    let order = sql_query(
        "SELECT COUNT(*) AS count FROM catalog_sets AS sets \
         JOIN no_intro_dat_games AS game USING(set_id) \
         WHERE sets.list_order=0 AND game.source_order=1",
    )
    .get_result::<Count>(&mut catalog.connection)?;
    assert_eq!(order.count, 1);
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM no_intro_dat_game_field_positions \
         WHERE set_id=(SELECT set_id FROM no_intro_dat_games) \
         AND position_domain=0 AND field_kind=0",
        )?,
        1
    );
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM no_intro_dat_game_field_positions \
         WHERE set_id=(SELECT set_id FROM no_intro_dat_games) \
         AND position_domain=1 AND field_kind=4",
        )?,
        1
    );
    assert_eq!(
        catalog.count(
            "SELECT COUNT(*) AS count FROM asset_occurrences \
         WHERE claim_kind='no_intro_dat_rom' AND occurrence_order IN (0,1)",
        )?,
        2
    );
    assert_eq!(
        catalog.count("SELECT COUNT(*) AS count FROM snapshot_publications")?,
        1
    );
    Ok(())
}

#[test]
fn publication_rejects_linked_uninterpretable_evidence_with_a_valid_link_control() -> TestResult {
    for declaration in [
        "size=''",
        "size='9223372036854775808'",
        "crc='bad'",
        "md5=''",
        "sha256='bad'",
        "",
    ] {
        let document = format!(
            "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version></header>\
            <game name='Game'><description>Game</description>\
            <rom name='file.bin' sha1='0123456789012345678901234567890123456789' {declaration}/>\
            </game></datafile>"
        );
        let mut catalog = Imported::new(&document)?;
        stage_pending_rom(&mut catalog, RomTamper::LinkedEvidence)?;
        let result = try_publish_pending(&mut catalog);
        if declaration.is_empty() {
            assert!(result.is_ok(), "valid control: {result:?}");
        } else {
            assert!(result.is_err(), "accepted {declaration}");
        }
    }
    Ok(())
}
