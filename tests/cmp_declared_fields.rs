use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey, SnapshotRecordStatus},
};

#[derive(QueryableByName)]
struct RomClaims {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc32_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    status_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    nodump_present: i64,
    #[diesel(sql_type = BigInt)]
    baddump_present: i64,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
}

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct RomFieldPosition {
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

#[derive(QueryableByName, Debug, PartialEq, Eq)]
struct RomFieldKindOrder {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct ConflictingRom {
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc32_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    nodump_present: i64,
    #[diesel(sql_type = BigInt)]
    baddump_present: i64,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    assertion_count: i64,
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct IdValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(Clone, Copy)]
enum IdentityLink {
    Preserve,
    Omit,
}

fn pending_occurrence(
    connection: &mut SqliteConnection,
    original: &SnapshotKey,
    identity: IdentityLink,
) -> Result<i64, Box<dyn std::error::Error>> {
    sql_query("PRAGMA foreign_keys=ON").execute(connection)?;
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT 'pending',source_key,'Pending' FROM catalogs WHERE catalog_key='cmp-declared-fields-test'").execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT 'pending','pending',document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(original.as_str()).execute(connection)?;
    sql_query("INSERT INTO cmp_documents(snapshot_key,header_present,comment_count) VALUES('pending',0,0)").execute(connection)?;
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending','root',0)",
    )
    .execute(connection)?;
    let set_id = sql_query("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT set_group_id,'cmp_set',0,'pending',1,1 FROM catalog_set_groups WHERE snapshot_key='pending' RETURNING set_id AS value")
        .get_result::<IdValue>(connection)?.value;
    sql_query("INSERT INTO cmp_set_facts(record_id,source_block,document_order) VALUES(?,'set',0)")
        .bind::<BigInt, _>(set_id)
        .execute(connection)?;
    sql_query("INSERT INTO cmp_set_field_positions(record_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) VALUES(?,0,'name',0,0,1,1)").bind::<BigInt,_>(set_id).execute(connection)?;
    let occurrence_id = sql_query("INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) SELECT ?,0,'cmp_rom',CASE WHEN ? THEN content_uuid END FROM asset_occurrences WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM cmp_rom_claims) RETURNING occurrence_id AS value")
        .bind::<BigInt,_>(set_id).bind::<Bool,_>(matches!(identity, IdentityLink::Preserve)).get_result::<IdValue>(connection)?.value;
    Ok(occurrence_id)
}

fn pending_rom(
    connection: &mut SqliteConnection,
    original: &SnapshotKey,
    changed_digest_field: i64,
    identity: IdentityLink,
) -> Result<i64, Box<dyn std::error::Error>> {
    let occurrence_id = pending_occurrence(connection, original, identity)?;
    sql_query("INSERT INTO cmp_rom_claims(occurrence_id,name,size_text,crc_text,crc32_text,md5_text,sha1_text,evidence_scope,evidence_provenance,merge_name,date,serial,status_text,nodump_present,baddump_present,source_line,source_column) SELECT ?1,name,size_text,CASE WHEN ?2=2 THEN 'DEADBEEF' ELSE crc_text END,CASE WHEN ?2=3 THEN 'CAFEBABE' ELSE crc32_text END,CASE WHEN ?2=4 THEN 'cccccccccccccccccccccccccccccccc' ELSE md5_text END,CASE WHEN ?2=5 THEN 'dddddddddddddddddddddddddddddddddddddddd' ELSE sha1_text END,evidence_scope,evidence_provenance,merge_name,date,serial,status_text,nodump_present,baddump_present,source_line,source_column FROM cmp_rom_claims ORDER BY occurrence_id LIMIT 1")
        .bind::<BigInt,_>(occurrence_id).bind::<BigInt,_>(changed_digest_field).execute(connection)?;
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) SELECT ?,digest_id,scope,provenance FROM occurrence_digest_assertions WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM cmp_rom_claims)")
        .bind::<BigInt,_>(occurrence_id).execute(connection)?;
    sql_query("INSERT INTO cmp_set_rom_positions(occurrence_id,source_order) VALUES(?,1)")
        .bind::<BigInt, _>(occurrence_id)
        .execute(connection)?;
    Ok(occurrence_id)
}

fn copy_positions(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO cmp_rom_field_positions(occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_rom_field_positions WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM cmp_rom_claims)")
        .bind::<BigInt,_>(occurrence_id).execute(connection)
}

fn connect(directory: &tempfile::TempDir) -> Result<SqliteConnection, Box<dyn std::error::Error>> {
    Ok(SqliteConnection::establish(
        directory
            .path()
            .join("catalog.sqlite")
            .to_str()
            .ok_or("non-UTF-8 database path")?,
    )?)
}

#[test]
fn publication_requires_native_claim_and_claims_cannot_be_added_after_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database) = setup()?;
    let original = import(
        &directory,
        &database,
        "original",
        "set ( name s rom ( name r ) )",
    )?;
    let mut connection = connect(&directory)?;
    pending_occurrence(&mut connection, &original, IdentityLink::Omit)?;
    let error = publish_pending(&mut connection)
        .err()
        .ok_or("a ROM owner without its native claim published")?;
    assert!(
        error
            .to_string()
            .contains("CMP ROM fields require complete position ownership"),
        "{error}"
    );
    let error = sql_query("INSERT INTO cmp_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,nodump_present,baddump_present,source_line,source_column) SELECT MIN(occurrence_id),'late','whole_asset','source_declared',0,0,1,1 FROM cmp_rom_claims")
        .execute(&mut connection).err().ok_or("published native owner accepted a claim insert")?;
    assert!(
        error
            .to_string()
            .contains("CMP ROM claims require an unpublished native owner"),
        "{error}"
    );
    Ok(())
}

#[test]
fn publication_rejects_extra_source_hashes_but_keeps_computed_evidence_separate()
-> Result<(), Box<dyn std::error::Error>> {
    for provenance in ["source_declared", "computed"] {
        let (directory, database) = setup()?;
        let original = import(
            &directory,
            &database,
            "original",
            "set ( name s rom ( name r sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ) )",
        )?;
        let mut connection = connect(&directory)?;
        let pending = pending_rom(&mut connection, &original, -1, IdentityLink::Omit)?;
        copy_positions(&mut connection, pending)?;
        let digest = sql_query("INSERT INTO digest_values(digest_id,algorithm,digest) VALUES(0,'sha1',X'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb') RETURNING digest_id AS value").get_result::<IdValue>(&mut connection)?.value;
        sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES(?,?,'whole_asset',?)")
            .bind::<BigInt,_>(pending).bind::<BigInt,_>(digest).bind::<Text,_>(provenance).execute(&mut connection)?;
        if provenance == "source_declared" {
            let error = publish_pending(&mut connection)
                .err()
                .ok_or("extra source hash published without a UUID")?;
            assert!(
                error
                    .to_string()
                    .contains("CMP source assertions must match declared ROM fields"),
                "{error}"
            );
        } else {
            assert_eq!(publish_pending(&mut connection)?, 1);
            let first = sql_query("SELECT hex(digest) AS value FROM occurrence_digest_assertions JOIN digest_values USING(digest_id) WHERE occurrence_id=? ORDER BY digest_id LIMIT 1")
                .bind::<BigInt,_>(pending).get_result::<TextValue>(&mut connection)?;
            assert_eq!(
                first.value, "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "computed witness must sort before native source evidence"
            );
            let projected = sql_query(
                "SELECT hex(sha1) AS value FROM asset_requirements WHERE snapshot_key='pending'",
            )
            .get_result::<TextValue>(&mut connection)?;
            assert_eq!(projected.value, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        }
    }
    Ok(())
}

fn publish_pending(connection: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(snapshot_key,catalog_key,document_key,interpretation_key) SELECT snapshot_key,catalog_key,document_key,interpretation_key FROM catalog_snapshots WHERE snapshot_key='pending'").execute(connection)
}

#[test]
fn publication_requires_every_present_cmp_rom_field_position()
-> Result<(), Box<dyn std::error::Error>> {
    for omitted in 0..12_i64 {
        let (directory, database) = setup()?;
        let original = import(
            &directory,
            &database,
            "original",
            "set ( name s rom ( name r size 0001 crc aabbccdd crc32 AABBCCDD md5 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa merge \"\" date \"\" serial \"\" status \"\" nodump baddump ) )",
        )?;
        let mut connection = SqliteConnection::establish(
            directory
                .path()
                .join("catalog.sqlite")
                .to_str()
                .ok_or("nonUTF8path")?,
        )?;
        let pending = pending_rom(&mut connection, &original, -1, IdentityLink::Preserve)?;
        let copy = "INSERT INTO cmp_rom_field_positions(occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_rom_field_positions WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM cmp_rom_claims) AND field_kind <> ?";
        sql_query(copy)
            .bind::<BigInt, _>(pending)
            .bind::<BigInt, _>(omitted)
            .execute(&mut connection)?;
        let error = publish_pending(&mut connection)
            .err()
            .ok_or_else(|| format!("field {omitted} published without its position"))?;
        assert!(
            error
                .to_string()
                .contains("CMP ROM fields require complete position ownership"),
            "{error}"
        );
        sql_query("INSERT INTO cmp_rom_field_positions(occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_rom_field_positions WHERE occurrence_id=(SELECT MIN(occurrence_id) FROM cmp_rom_claims) AND field_kind=?")
            .bind::<BigInt,_>(pending).bind::<BigInt,_>(omitted).execute(&mut connection)?;
        assert_eq!(publish_pending(&mut connection)?, 1);
        assert!(
            sql_query("DELETE FROM cmp_rom_field_positions WHERE occurrence_id=?")
                .bind::<BigInt, _>(pending)
                .execute(&mut connection)
                .is_err()
        );
        assert!(sql_query("UPDATE cmp_rom_field_positions SET source_order=source_order+1 WHERE occurrence_id=?").bind::<BigInt,_>(pending).execute(&mut connection).is_err());
    }
    Ok(())
}

#[test]
fn publication_rejects_digest_assertions_that_disagree_with_native_text()
-> Result<(), Box<dyn std::error::Error>> {
    for changed in 2..6_i64 {
        let (directory, database) = setup()?;
        let original = import(
            &directory,
            &database,
            "original",
            "set ( name s rom ( name r size 1 crc aabbccdd crc32 AABBCCDD md5 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ) )",
        )?;
        let mut connection = SqliteConnection::establish(
            directory
                .path()
                .join("catalog.sqlite")
                .to_str()
                .ok_or("nonUTF8path")?,
        )?;
        let pending = pending_rom(&mut connection, &original, changed, IdentityLink::Preserve)?;
        copy_positions(&mut connection, pending)?;
        let error = publish_pending(&mut connection).err().ok_or_else(|| {
            format!("field {changed} published with a different normalized assertion")
        })?;
        assert!(
            error
                .to_string()
                .contains("CMP source assertions must match declared ROM fields"),
            "{error}"
        );
    }
    Ok(())
}

#[test]
fn native_cmp_claims_reject_non_file_scope_and_non_source_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    for (scope, provenance) in [
        ("rom_segment", "source_declared"),
        ("whole_asset", "computed"),
        ("whole_asset", "unknown"),
    ] {
        let (directory, database) = setup()?;
        let original = import(
            &directory,
            &database,
            "original",
            "set ( name s rom ( name r ) )",
        )?;
        let mut connection = connect(&directory)?;
        let pending = pending_occurrence(&mut connection, &original, IdentityLink::Omit)?;
        let result = sql_query("INSERT INTO cmp_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,nodump_present,baddump_present,source_line,source_column) VALUES(?,'r',?,?,0,0,1,1)")
            .bind::<BigInt,_>(pending).bind::<Text,_>(scope).bind::<Text,_>(provenance).execute(&mut connection);
        assert!(
            result.is_err(),
            "native CMP source claim accepted {scope}/{provenance}"
        );
    }
    Ok(())
}

fn setup() -> Result<(tempfile::TempDir, Database), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    Ok((directory, database))
}

fn import(
    directory: &tempfile::TempDir,
    database: &Database,
    revision: &str,
    source: &str,
) -> Result<SnapshotKey, Box<dyn std::error::Error>> {
    let document_path =
        Utf8PathBuf::from_path_buf(directory.path().join(format!("{revision}.dat")))
            .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, source)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::ClrMamePro,
            source_key: PublishingSourceKey::new("cmp-declared-fields-test"),
            source_display_name: "CMP declared fields test".to_owned(),
            catalog_key: CatalogKey::new("cmp-declared-fields-test"),
            catalog_display_name: "CMP declared fields test".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    report
        .snapshot_key
        .ok_or_else(|| "successful import lacked snapshot".into())
}

#[test]
fn public_import_retains_cmp_rom_declared_text() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, database) = setup()?;
    let snapshot = import(
        &directory,
        &database,
        "native",
        r#"clrmamepro ( name "Declared text" )
           GAME (
             NAME "native-set"
             ROM (
               NAME "native.bin"
               SIZE 00016
               CRC AABBCCDD
               MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
               SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
               MERGE "parent.bin"
               DATE ""
               SERIAL ""
               STATUS "nodump"
               NODUMP
             )
           )"#,
    )?;
    let mut connection = connect(&directory)?;
    let rom = sql_query(
        "SELECT name, size, size_text, crc_text, crc32_text, md5_text, sha1_text, merge_name, \
                date, serial, status_text, nodump_present, baddump_present, dump_status \
         FROM cmp_rom_claims JOIN asset_occurrences USING (occurrence_id) \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RomClaims>(&mut connection)?;
    let interpretation = sql_query(
        "SELECT rules_version AS value FROM catalog_snapshots \
         JOIN parser_interpretations USING (interpretation_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<TextValue>(&mut connection)?;
    assert_eq!(interpretation.value, "clrmamepro-declared-text-compat-v1");
    assert_eq!(rom.name, "native.bin");
    assert_eq!(rom.size, Some(16));
    assert_eq!(rom.size_text.as_deref(), Some("00016"));
    assert_eq!(rom.crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(rom.crc32_text, None);
    assert_eq!(
        rom.md5_text.as_deref(),
        Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
    );
    assert_eq!(
        rom.sha1_text.as_deref(),
        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
    );
    assert_eq!(rom.merge_name.as_deref(), Some("parent.bin"));
    assert_eq!(rom.date.as_deref(), Some(""));
    assert_eq!(rom.serial.as_deref(), Some(""));
    assert_eq!(rom.status_text.as_deref(), Some("nodump"));
    assert_eq!(rom.nodump_present, 1);
    assert_eq!(rom.baddump_present, 0);
    assert_eq!(
        rom.dump_status, None,
        "explicit status plus a dump flag is retained as a conflict"
    );

    let positions = sql_query(
        "SELECT position.field_kind, position.source_field, position.source_order, position.is_quoted, position.source_line, position.source_column \
         FROM cmp_rom_field_positions AS position JOIN asset_occurrences USING (occurrence_id) \
         JOIN catalog_sets ON record_id=set_id JOIN catalog_set_groups USING (set_group_id) \
         WHERE snapshot_key = ? \
         ORDER BY field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<RomFieldPosition>(&mut connection)?;
    assert_eq!(
        positions
            .iter()
            .map(|position| (
                position.field_kind,
                position.source_field.as_str(),
                position.source_order,
                position.is_quoted,
            ))
            .collect::<Vec<_>>(),
        [
            (0, "NAME", 0, 1),
            (1, "SIZE", 1, 0),
            (2, "CRC", 2, 0),
            (4, "MD5", 3, 0),
            (5, "SHA1", 4, 0),
            (6, "MERGE", 5, 1),
            (7, "DATE", 6, 1),
            (8, "SERIAL", 7, 1),
            (9, "STATUS", 8, 1),
            (10, "NODUMP", 9, 0),
        ]
    );
    assert!(
        positions
            .iter()
            .all(|position| position.source_line > 0 && position.source_column > 0)
    );
    Ok(())
}

#[test]
fn conflicting_crc_and_dump_flags_are_retained_without_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database) = setup()?;
    let snapshot = import(
        &directory,
        &database,
        "conflicts",
        r#"clrmamepro ( name "Conflicts" )
           GAME ( NAME "conflicts"
             ROM ( NAME "conflict.bin" SIZE 8 CRC AABBCCDD CRC32 11223344 NODUMP BADDUMP )
           )"#,
    )?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let rom = sql_query(
        "SELECT crc_text, crc32_text, nodump_present, baddump_present, dump_status, \
                occurrence.content_uuid, \
                (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion \
                 WHERE assertion.occurrence_id = occurrence.occurrence_id) AS assertion_count \
         FROM cmp_rom_claims AS claim \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         JOIN records USING (record_id) JOIN record_namespaces USING (namespace_id) \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<ConflictingRom>(&mut connection)?;
    assert_eq!(rom.crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(rom.crc32_text.as_deref(), Some("11223344"));
    assert_eq!(rom.nodump_present, 1);
    assert_eq!(rom.baddump_present, 1);
    assert_eq!(rom.dump_status, None);
    assert_eq!(rom.content_uuid, None);
    assert_eq!(
        rom.assertion_count, 2,
        "both valid CRC declarations remain queryable evidence"
    );

    let positions = sql_query(
        "SELECT field_kind, source_order FROM cmp_rom_field_positions \
         WHERE occurrence_id = (SELECT occurrence_id FROM cmp_rom_claims \
                                JOIN asset_occurrences USING (occurrence_id) \
                                JOIN records USING (record_id) \
                                JOIN record_namespaces USING (namespace_id) \
                                WHERE snapshot_key = ?) ORDER BY field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<RomFieldKindOrder>(&mut connection)?;
    assert_eq!(
        positions,
        [
            RomFieldKindOrder {
                field_kind: 0,
                source_order: 0,
            },
            RomFieldKindOrder {
                field_kind: 1,
                source_order: 1,
            },
            RomFieldKindOrder {
                field_kind: 2,
                source_order: 2,
            },
            RomFieldKindOrder {
                field_kind: 3,
                source_order: 3,
            },
            RomFieldKindOrder {
                field_kind: 10,
                source_order: 4,
            },
            RomFieldKindOrder {
                field_kind: 11,
                source_order: 5,
            },
        ]
    );
    Ok(())
}

#[test]
fn snapshot_diff_tracks_declared_spelling_but_ignores_vendor_only_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database) = setup()?;
    let baseline = r#"clrmamepro ( name "History" )
        GAME ( NAME "history-set"
          ROM ( NAME "history.bin" SIZE 00016 CRC AABBCCDD MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA DATE "" SERIAL "" STATUS "verified" )
        )"#;
    let base = import(&directory, &database, "base", baseline)?;
    let cases = [
        (
            "hash-case",
            r#"clrmamepro ( name "History" )
              GAME ( NAME "history-set"
                ROM ( NAME "history.bin" SIZE 00016 CRC aabbccdd MD5 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb SHA1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa DATE "" SERIAL "" STATUS "verified" )
              )"#,
        ),
        (
            "leading-zero",
            r#"clrmamepro ( name "History" )
              GAME ( NAME "history-set"
                ROM ( NAME "history.bin" SIZE 16 CRC AABBCCDD MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA DATE "" SERIAL "" STATUS "verified" )
              )"#,
        ),
        (
            "quote-state",
            r#"clrmamepro ( name "History" )
              GAME ( NAME "history-set"
                ROM ( NAME "history.bin" SIZE 00016 CRC AABBCCDD MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA DATE "" SERIAL "" STATUS verified )
              )"#,
        ),
        (
            "field-spelling",
            r#"clrmamepro ( name "History" )
              GAME ( NAME "history-set"
                ROM ( name "history.bin" SIZE 00016 CRC AABBCCDD MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA DATE "" SERIAL "" STATUS "verified" )
              )"#,
        ),
        (
            "field-order",
            r#"clrmamepro ( name "History" )
              GAME ( NAME "history-set"
                ROM ( NAME "history.bin" SIZE 00016 CRC AABBCCDD MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA SERIAL "" DATE "" STATUS "verified" )
              )"#,
        ),
    ];
    for (revision, source) in cases {
        let current = import(&directory, &database, revision, source)?;
        let diff = app::diff_catalog_snapshots(&database, &base, &current)?;
        let record = diff
            .records
            .iter()
            .find(|record| record.set_name == "history-set")
            .ok_or("history set missing from snapshot diff")?;
        assert_eq!(
            record.status,
            SnapshotRecordStatus::Changed,
            "{revision} source declaration change was lost"
        );
    }

    let vendor_only = import(
        &directory,
        &database,
        "vendor-only",
        r#"clrmamepro ( name "History" )
           GAME ( NAME "history-set"
             ROM ( NAME "history.bin" SIZE 00016 CRC AABBCCDD FUTURE "vendor-value" MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA DATE "" SERIAL "" STATUS "verified" )
           )"#,
    )?;
    let diff = app::diff_catalog_snapshots(&database, &base, &vendor_only)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "history-set")
        .ok_or("history set missing from vendor-only snapshot diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn snapshot_diff_reports_declared_changes_alongside_interpreted_size_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, database) = setup()?;
    let base = import(
        &directory,
        &database,
        "before",
        "set ( name s rom ( name r size 0001 ) )",
    )?;
    let current = import(
        &directory,
        &database,
        "after",
        "set ( name s rom ( name r SIZE 0002 ) )",
    )?;
    let diff = app::diff_catalog_snapshots(&database, &base, &current)?;
    let change = diff
        .records
        .first()
        .and_then(|record| record.requirement_changes.first())
        .ok_or("missing requirement change")?;
    assert!(change.size_changed);
    assert!(change.other_evidence_changed);
    Ok(())
}
