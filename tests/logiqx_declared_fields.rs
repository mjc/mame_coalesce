use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};
use std::{error::Error, fmt::Write};

type TestResult = Result<(), Box<dyn Error>>;

#[derive(QueryableByName)]
struct DeclaredRom {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    assertion_count: i64,
}

#[derive(QueryableByName)]
struct DeclaredDisk {
    #[diesel(sql_type = Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    assertion_count: i64,
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct ValueRow {
    #[diesel(sql_type = Text)]
    value: String,
}

fn stage_snapshot(connection: &mut SqliteConnection, original: &SnapshotKey) -> TestResult {
    sql_query(
        "INSERT INTO catalogs(catalog_key, source_key, display_name) \
         SELECT 'pending', source_key, 'Pending' FROM catalogs WHERE catalog_key = 'declared-fields'",
    )
    .execute(connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
         SELECT 'pending','pending',document_key,interpretation_key,coverage_id \
         FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(original.as_str())
    .execute(connection)?;
    Ok(())
}

fn publish_pending(connection: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(snapshot_key,catalog_key,document_key,interpretation_key) \
         SELECT snapshot_key,catalog_key,document_key,interpretation_key \
         FROM catalog_snapshots WHERE snapshot_key = 'pending'",
    )
    .execute(connection)
}

fn setup() -> Result<(tempfile::TempDir, Database, SqliteConnection), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "temporary path was not UTF-8")?;
    let database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    sql_query("PRAGMA foreign_keys = ON").execute(&mut connection)?;
    Ok((directory, database, connection))
}

fn import(
    directory: &tempfile::TempDir,
    database: &Database,
    revision: &str,
    xml: &str,
) -> Result<SnapshotKey, Box<dyn Error>> {
    let path = directory.path().join(format!("{revision}.dat"));
    std::fs::write(&path, xml)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: Utf8PathBuf::from_path_buf(path).map_err(|_| "non-UTF-8 path")?,
            format: CatalogDocumentFormat::Logiqx(
                mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
            ),
            source_key: PublishingSourceKey::new("declared-fields-source"),
            source_display_name: "Declared fields".into(),
            catalog_key: CatalogKey::new("declared-fields"),
            catalog_display_name: "Declared fields".into(),
            scope: CatalogScope::Unknown,
        },
    )?;
    assert_eq!(
        report.status,
        CatalogImportStatus::Succeeded,
        "specification CDATA must be retained even when it cannot be used as evidence"
    );
    report
        .snapshot_key
        .ok_or_else(|| "successful import lacked snapshot".into())
}

fn declared_roms(
    connection: &mut SqliteConnection,
) -> Result<Vec<DeclaredRom>, diesel::result::Error> {
    sql_query(
        "SELECT rom.name, rom.size_text, rom.size, rom.crc_text, rom.md5_text, rom.sha1_text, \
                occurrence.content_uuid, \
                (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion \
                 WHERE assertion.occurrence_id = rom.occurrence_id) AS assertion_count \
         FROM logiqx_rom_claims AS rom JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         ORDER BY rom.name",
    )
    .load(connection)
}

#[test]
fn specification_cdata_stays_queryable_without_fabricating_evidence() -> TestResult {
    let (directory, database, mut connection) = setup()?;
    import(
        &directory,
        &database,
        "reported",
        r#"<datafile><header><name>CDATA</name></header><game name="g">
           <description>Game</description>
           <rom name="absent"/>
           <rom name="empty" size="" crc="" md5="" sha1=""/>
           <rom name="invalid" size="null" crc="broken" md5="x" sha1="not-hex"/>
           <rom name="oversized" size="9223372036854775808" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/>
           <rom name="partial" size="1" crc="bad" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/>
           <rom name="valid" size="00016" crc="AABBCCDD" md5="BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB" sha1="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"/>
           <disk name="disk" md5="" sha1="invalid"/>
         </game></datafile>"#,
    )?;
    let rows = declared_roms(&mut connection)?;
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].name, "absent");
    assert_eq!(rows[0].size_text, None);
    assert_eq!(rows[0].crc_text, None);
    assert_eq!(rows[0].md5_text, None);
    assert_eq!(rows[0].sha1_text, None);
    assert_eq!(rows[1].size_text.as_deref(), Some(""));
    assert_eq!(rows[1].crc_text.as_deref(), Some(""));
    assert_eq!(rows[1].md5_text.as_deref(), Some(""));
    assert_eq!(rows[1].sha1_text.as_deref(), Some(""));
    assert_eq!(rows[2].size_text.as_deref(), Some("null"));
    assert_eq!(rows[2].crc_text.as_deref(), Some("broken"));
    assert_eq!(rows[2].md5_text.as_deref(), Some("x"));
    assert_eq!(rows[2].sha1_text.as_deref(), Some("not-hex"));
    assert_eq!(rows[3].size_text.as_deref(), Some("9223372036854775808"));
    for row in &rows[..4] {
        assert_eq!(row.size, None, "{} invented a numeric size", row.name);
    }
    for row in &rows[..5] {
        assert_eq!(
            row.content_uuid, None,
            "{} invented file identity",
            row.name
        );
    }
    for row in &rows[..3] {
        assert_eq!(
            row.assertion_count, 0,
            "{} invented digest evidence",
            row.name
        );
    }
    assert_eq!(rows[3].assertion_count, 1);
    assert_eq!(rows[4].assertion_count, 1);
    assert_eq!(rows[5].size, Some(16));
    assert_eq!(rows[5].size_text.as_deref(), Some("00016"));
    assert_eq!(rows[5].crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(
        rows[5].md5_text.as_deref(),
        Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
    );
    assert_eq!(
        rows[5].sha1_text.as_deref(),
        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
    );
    assert_eq!(rows[5].assertion_count, 3);
    assert!(rows[5].content_uuid.is_some());
    let disk = sql_query(
        "SELECT md5_text, sha1_text, \
         (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion \
          WHERE assertion.occurrence_id = disk.occurrence_id) AS assertion_count \
         FROM logiqx_disk_claims AS disk",
    )
    .get_result::<DeclaredDisk>(&mut connection)?;
    assert_eq!(disk.md5_text.as_deref(), Some(""));
    assert_eq!(disk.sha1_text.as_deref(), Some("invalid"));
    assert_eq!(disk.assertion_count, 0);
    Ok(())
}

#[test]
fn changing_hash_spelling_or_uninterpreted_text_changes_native_history() -> TestResult {
    let (directory, database, _) = setup()?;
    let before = import(
        &directory,
        &database,
        "before",
        r#"<datafile><game name="g"><description>Game</description>
           <rom name="r" size="null" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" crc="broken"/>
           </game></datafile>"#,
    )?;
    let after = import(
        &directory,
        &database,
        "after",
        r#"<datafile><game name="g"><description>Game</description>
           <rom name="r" size="NULL" sha1="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" crc=""/>
           </game></datafile>"#,
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(!diff.records[0].requirement_changes.is_empty());
    assert!(!diff.records[0].metadata_changed);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum LinkedClaimCase {
    Valid,
    InvalidSize,
    InvalidCrc,
    EmptyMd5,
    MissingSha1,
    EmptySha1,
    MismatchedDigest,
    ScopedDigest,
    ComputedDigest,
    DiskData,
}

fn stage_logiqx_set(connection: &mut SqliteConnection) -> diesel::QueryResult<i64> {
    // These game/claim witnesses use headerless sources. Their positive controls
    // still need the one native document owner required by publication.
    sql_query("INSERT INTO logiqx_document_facts(snapshot_key) VALUES('pending')")
        .execute(connection)?;
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending','root',0)",
    )
    .execute(connection)?;
    Ok(sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         SELECT set_group_id,'logiqx_game',0,'pending',1,1 FROM catalog_set_groups WHERE snapshot_key='pending' \
         RETURNING set_id AS value",
    ).get_result::<IdRow>(connection)?.value)
}

fn stage_game_name_position(connection: &mut SqliteConnection, set_id: i64) -> TestResult {
    sql_query("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)")
        .bind::<BigInt,_>(set_id).execute(connection)?;
    Ok(())
}

fn stage_linked_claim(connection: &mut SqliteConnection, case: LinkedClaimCase) -> TestResult {
    let set_id = stage_logiqx_set(connection)?;
    sql_query("INSERT INTO logiqx_games(set_id) VALUES(?)")
        .bind::<BigInt, _>(set_id)
        .execute(connection)?;
    stage_game_name_position(connection, set_id)?;
    let kind = if matches!(case, LinkedClaimCase::DiskData) {
        "logiqx_disk"
    } else {
        "logiqx_rom"
    };
    let occurrence_id = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         SELECT ?,0,?,content_uuid FROM asset_occurrences WHERE content_uuid IS NOT NULL LIMIT 1 \
         RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Text, _>(kind)
    .get_result::<IdRow>(connection)?
    .value;
    if matches!(case, LinkedClaimCase::DiskData) {
        sql_query(
            "INSERT INTO logiqx_disk_claims(occurrence_id,name,md5_text,sha1_text,evidence_scope,evidence_provenance,source_line,source_column) \
             VALUES(?,'pending','invalid','invalid','disk_data','source_declared',1,1)",
        ).bind::<BigInt,_>(occurrence_id).execute(connection)?;
    } else {
        let size = if matches!(case, LinkedClaimCase::InvalidSize) {
            "invalid"
        } else {
            "1"
        };
        let crc = matches!(case, LinkedClaimCase::InvalidCrc).then_some("invalid");
        let md5 = matches!(case, LinkedClaimCase::EmptyMd5).then_some("");
        let sha1 = match case {
            LinkedClaimCase::MissingSha1 => None,
            LinkedClaimCase::EmptySha1 => Some(""),
            _ => Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        };
        sql_query(
            "INSERT INTO logiqx_rom_claims(occurrence_id,name,size_text,crc_text,md5_text,sha1_text,evidence_scope,evidence_provenance,source_line,source_column) \
             VALUES(?,'pending',?,?,?,?,'whole_asset','source_declared',1,1)",
        ).bind::<BigInt,_>(occurrence_id).bind::<Text,_>(size)
            .bind::<Nullable<Text>,_>(crc).bind::<Nullable<Text>,_>(md5)
            .bind::<Nullable<Text>,_>(sha1).execute(connection)?;
    }
    let (table, fields) = if matches!(case, LinkedClaimCase::DiskData) {
        ("logiqx_disk_attribute_positions", vec![0, 1, 2])
    } else {
        let mut fields = vec![0, 1];
        if matches!(case, LinkedClaimCase::InvalidCrc) {
            fields.push(2);
        }
        if !matches!(case, LinkedClaimCase::MissingSha1) {
            fields.push(3);
        }
        if matches!(case, LinkedClaimCase::EmptyMd5) {
            fields.push(4);
        }
        ("logiqx_rom_attribute_positions", fields)
    };
    for field in fields {
        sql_query(format!("INSERT INTO {table}(occurrence_id,field_kind,source_order,source_line,source_column) VALUES(?,?,?,1,?)"))
            .bind::<BigInt,_>(occurrence_id).bind::<BigInt,_>(field).bind::<BigInt,_>(field).bind::<BigInt,_>(field+1).execute(connection)?;
    }
    let digest = if matches!(case, LinkedClaimCase::MismatchedDigest) {
        [0xbb; 20]
    } else {
        [0xaa; 20]
    };
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES('sha1',?)")
        .bind::<Binary, _>(digest.as_slice())
        .execute(connection)?;
    let scope = if matches!(
        case,
        LinkedClaimCase::ScopedDigest | LinkedClaimCase::DiskData
    ) {
        "disk_data"
    } else {
        "whole_asset"
    };
    let provenance = if matches!(case, LinkedClaimCase::ComputedDigest) {
        "computed"
    } else {
        "source_declared"
    };
    sql_query(
        "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
         SELECT ?,digest_id,?,? FROM digest_values WHERE algorithm='sha1' AND digest=?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Text, _>(scope)
    .bind::<Text, _>(provenance)
    .bind::<Binary, _>(digest.as_slice())
    .execute(connection)?;
    Ok(())
}

#[test]
fn publication_rejects_uuid_linked_to_an_uninterpretable_native_declaration() -> TestResult {
    for case in [
        LinkedClaimCase::InvalidSize,
        LinkedClaimCase::InvalidCrc,
        LinkedClaimCase::EmptyMd5,
        LinkedClaimCase::MissingSha1,
        LinkedClaimCase::EmptySha1,
        LinkedClaimCase::MismatchedDigest,
        LinkedClaimCase::ScopedDigest,
        LinkedClaimCase::ComputedDigest,
        LinkedClaimCase::DiskData,
        LinkedClaimCase::Valid,
    ] {
        let (directory, database, mut connection) = setup()?;
        let original = import(
            &directory,
            &database,
            "original",
            r#"<datafile><game name="g"><rom name="valid" size="1" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/></game></datafile>"#,
        )?;
        stage_snapshot(&mut connection, &original)?;
        stage_linked_claim(&mut connection, case)?;
        let result = publish_pending(&mut connection);
        if matches!(case, LinkedClaimCase::Valid) {
            assert_eq!(result?, 1);
        } else {
            let error = result
                .err()
                .ok_or_else(|| format!("publication accepted {case:?}"))?;
            assert!(
                error
                    .to_string()
                    .contains("shared file UUID requires interpretable whole-file source evidence"),
                "{case:?}: {error}"
            );
        }
    }
    Ok(())
}
#[test]
fn publication_requires_positions_for_every_present_scalar() -> TestResult {
    let (directory, database, mut connection) = setup()?;
    let original = import(
        &directory,
        &database,
        "original",
        "<datafile><header><name>Original</name></header></datafile>",
    )?;
    stage_snapshot(&mut connection, &original)?;
    sql_query("INSERT INTO logiqx_document_facts(snapshot_key,header_name) VALUES('pending','')")
        .execute(&mut connection)?;
    assert!(
        publish_pending(&mut connection).is_err(),
        "empty present native value lost its position"
    );
    assert!(sql_query(
        "INSERT INTO logiqx_header_text_positions(snapshot_key,field_kind,source_order,source_line,source_column) \
         VALUES('pending',1,0,1,1)",
    ).execute(&mut connection).is_err(), "absent description acquired a position");
    sql_query(
        "INSERT INTO logiqx_header_text_positions(snapshot_key,field_kind,source_order,source_line,source_column) \
         VALUES('pending',0,0,1,1)",
    ).execute(&mut connection)?;
    assert_eq!(publish_pending(&mut connection)?, 1);
    assert!(
        sql_query("DELETE FROM logiqx_header_text_positions WHERE snapshot_key='pending'")
            .execute(&mut connection)
            .is_err()
    );
    Ok(())
}

#[test]
fn publication_requires_positions_for_each_present_game_scalar() -> TestResult {
    for (kind, column) in [(0_i64, "description"), (1, "year"), (2, "manufacturer")] {
        let (directory, database, mut connection) = setup()?;
        let original = import(&directory, &database, "original", "<datafile/>")?;
        stage_snapshot(&mut connection, &original)?;
        let set_id = stage_logiqx_set(&mut connection)?;
        sql_query(format!(
            "INSERT INTO logiqx_games(set_id,{column}) VALUES(?,'')"
        ))
        .bind::<BigInt, _>(set_id)
        .execute(&mut connection)?;
        stage_game_name_position(&mut connection, set_id)?;
        let error = publish_pending(&mut connection)
            .err()
            .ok_or("present game scalar published without a position")?;
        assert!(
            error
                .to_string()
                .contains("native scalar values require complete position ownership")
        );
        let query = "INSERT INTO logiqx_game_text_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(?,?,0,1,1)";
        assert!(
            sql_query(query)
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>((kind + 1) % 3)
                .execute(&mut connection)
                .is_err(),
            "absent game scalar acquired a position"
        );
        sql_query(query)
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(kind)
            .execute(&mut connection)?;
        assert_eq!(publish_pending(&mut connection)?, 1);
        assert!(
            sql_query("UPDATE logiqx_game_text_positions SET source_order=1 WHERE set_id=?")
                .bind::<BigInt, _>(set_id)
                .execute(&mut connection)
                .is_err()
        );
        assert!(
            sql_query("DELETE FROM logiqx_game_text_positions WHERE set_id=?")
                .bind::<BigInt, _>(set_id)
                .execute(&mut connection)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn native_history_ranks_text_with_media_and_header_options() -> TestResult {
    let (directory, database, _) = setup()?;
    let before = import(
        &directory,
        &database,
        "before",
        "<datafile><header><name>Catalog</name><clrmamepro/></header><game name='g'><description>Game</description><rom name='r' size='1'/></game></datafile>",
    )?;
    let after = import(
        &directory,
        &database,
        "after",
        "<datafile><header><clrmamepro/><name>Catalog</name></header><game name='g'><rom name='r' size='1'/><description>Game</description></game></datafile>",
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(
        diff.document_metadata_changed,
        "header text/options crossing was hidden"
    );
    assert!(
        diff.records[0].metadata_changed,
        "scalar/media crossing was hidden"
    );
    assert_eq!(diff.records[0].requirement_changes.len(), 1);
    Ok(())
}

#[test]
fn native_history_ignores_vendor_gaps_but_preserves_pcdata_spaces() -> TestResult {
    let (directory, database, mut connection) = setup()?;
    let before = import(
        &directory,
        &database,
        "before",
        "<datafile><header><name></name><description> About </description><clrmamepro/></header><game name='g'><description> Game </description><year/><rom name='r' size='1'/></game></datafile>",
    )?;
    let after = import(
        &directory,
        &database,
        "after",
        "<datafile>\n<header><vendor/><name/><vendor/><description> About </description><vendor/><clrmamepro/></header>\n<game name='g'><vendor/><description> Game </description><vendor/><year/><rom name='r' size='1'/></game></datafile>",
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &after)?;
    assert!(!diff.document_metadata_changed);
    assert!(!diff.records[0].metadata_changed);
    assert!(diff.records[0].requirement_changes.is_empty());
    let value = sql_query("SELECT description AS value FROM logiqx_games LIMIT 1")
        .get_result::<ValueRow>(&mut connection)?;
    assert_eq!(value.value, " Game ");
    Ok(())
}

#[test]
fn device_reference_crossings_change_native_history_but_layout_does_not() -> TestResult {
    let (directory, database, _) = setup()?;
    let before = import(
        &directory,
        &database,
        "device-before",
        "<datafile><game name='g'><device_ref name='sound'/><rom name='r' size='1'/></game></datafile>",
    )?;
    let crossed = import(
        &directory,
        &database,
        "device-crossed",
        "<datafile><game name='g'><rom name='r' size='1'/><device_ref name='sound'/></game></datafile>",
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &crossed)?;
    assert!(
        diff.records
            .first()
            .ok_or("history record missing")?
            .metadata_changed,
        "device/media crossing was lost"
    );
    let reindented = import(
        &directory,
        &database,
        "device-layout",
        "<datafile>\n<game name='g'>\n<vendor/><device_ref name='sound'/>\n<vendor/><rom name='r' size='1'/>\n</game></datafile>",
    )?;
    let diff = app::diff_catalog_snapshots(&database, &before, &reindented)?;
    let record = diff.records.first().ok_or("history record missing")?;
    assert!(
        !record.metadata_changed,
        "layout/vendor gap invented a metadata change"
    );
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn sqlite_size_interpretation_matches_rust_at_decimal_boundaries() -> TestResult {
    let (directory, database, mut connection) = setup()?;
    let texts = [
        "+1".to_owned(),
        " 1 ".to_owned(),
        "1.0".to_owned(),
        "١".to_owned(),
        "0".repeat(500),
        "00016".to_owned(),
        i64::MAX.to_string(),
        "9223372036854775808".to_owned(),
    ];
    let mut declarations = String::new();
    for (index, text) in texts.iter().enumerate() {
        write!(declarations, "<rom name='r{index}' size='{text}'/>")?;
    }
    let xml = format!("<datafile><game name='g'>{declarations}</game></datafile>");
    import(&directory, &database, "sizes", &xml)?;
    let parsed = mame_coalesce::logiqx::DataFile::from_reader(xml.as_bytes())?;
    let rows = declared_roms(&mut connection)?;
    for ((row, rom), text) in rows.iter().zip(parsed.games()[0].roms()).zip(texts) {
        assert_eq!(row.size_text.as_deref(), Some(text.as_str()));
        assert_eq!(row.size, rom.size().map(i64::try_from).transpose()?);
    }
    Ok(())
}
