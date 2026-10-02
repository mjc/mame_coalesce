use camino::Utf8PathBuf;
use diesel::sql_types::{BigInt, Binary, Nullable, Text};
use diesel::{Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn import(xml: &str) -> TestResult<(tempfile::TempDir, SqliteConnection)> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(&document, xml)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("declarations"),
            source_display_name: "Declarations".into(),
            catalog_key: CatalogKey::new("machines"),
            catalog_display_name: "Machines".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(
        report.status,
        CatalogImportStatus::Succeeded,
        "CDATA declarations must persist even when unusable as numeric/hash evidence"
    );
    let connection = SqliteConnection::establish(path.as_str())?;
    Ok((directory, connection))
}

#[derive(QueryableByName)]
struct Rom {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    offset_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[test]
fn source_declarations_survive_without_becoming_unusable_matching_evidence() -> TestResult {
    let (_directory, mut connection) = import(
        "<mame mameconfig='10'><machine name='system'><description>System</description>
         <rom name='spelled' size='00016' crc='Ab12Cd34' sha1='0123456789ABCDEF0123456789ABCDEF01234567' offset='00aB'/>
         <rom name='unusable' size='unknown' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' offset='not-a-number'/>
         <rom name='empty' size='' crc='' sha1='' offset=''/>
         <rom name='huge' size='18446744073709551615' sha1='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'/>
         <rom name='sparse'/>
         <disk name='disk' sha1='CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC'/>
         </machine></mame>",
    )?;
    let roms = sql_query(
        "SELECT rom.name,rom.size_text,rom.crc_text,rom.sha1_text,rom.offset_text,rom.size,occurrence.content_uuid
         FROM mame_rom_claims AS rom JOIN asset_occurrences AS occurrence USING(occurrence_id)
         ORDER BY occurrence.occurrence_order",
    ).load::<Rom>(&mut connection)?;
    assert_eq!(roms.len(), 5);
    assert_eq!(roms[0].name, "spelled");
    assert_eq!(roms[0].size_text.as_deref(), Some("00016"));
    assert_eq!(roms[0].crc_text.as_deref(), Some("Ab12Cd34"));
    assert_eq!(
        roms[0].sha1_text.as_deref(),
        Some("0123456789ABCDEF0123456789ABCDEF01234567")
    );
    assert_eq!(roms[0].offset_text.as_deref(), Some("00aB"));
    assert_eq!(roms[0].size, Some(16));
    assert_eq!(roms[0].content_uuid.as_ref().map(Vec::len), Some(16));
    assert_eq!(roms[1].size_text.as_deref(), Some("unknown"));
    assert_eq!(roms[1].offset_text.as_deref(), Some("not-a-number"));
    assert_eq!(
        roms[1].sha1_text.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(roms[1].size, None);
    assert_eq!(roms[1].content_uuid, None);
    for value in [
        &roms[2].size_text,
        &roms[2].crc_text,
        &roms[2].sha1_text,
        &roms[2].offset_text,
    ] {
        assert_eq!(value.as_deref(), Some(""));
    }
    assert_eq!(roms[2].size, None);
    assert_eq!(roms[2].content_uuid, None);
    assert_eq!(roms[3].size_text.as_deref(), Some("18446744073709551615"));
    assert_eq!(roms[3].size, None);
    assert_eq!(roms[3].content_uuid, None);
    assert_eq!(roms[4].size_text, None);
    assert_eq!(roms[4].offset_text, None);
    let disk = sql_query("SELECT count(*) AS count FROM mame_disk_claims AS disk JOIN asset_occurrences USING(occurrence_id) WHERE disk.sha1_text='CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC' AND content_uuid IS NULL")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(disk.count, 1);
    let failures = sql_query("SELECT count(*) AS count FROM pragma_foreign_key_check")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(failures.count, 0);
    Ok(())
}

#[test]
fn native_rom_disk_and_historical_fields_have_distinct_qualified_owners() -> TestResult {
    let (_directory, mut connection) = import(
        "<mame mameconfig='10'><machine name='system' isconsumable='no'><description>System</description>
         <rom name='legacy' size='16' md5='ABCDEF0123456789ABCDEF0123456789' soundonly='yes' dispose='no' loadflag='' value='0' inverted='no' ovha='' nothread='yes'/>
         <disk name='disk' writable='no' writeable='yes'/>
         </machine></mame>",
    )?;
    for (table, absent) in [
        (
            "mame_machines",
            vec!["is_consumable", "is_consumable_specified"],
        ),
        (
            "mame_rom_claims",
            vec![
                "size",
                "offset",
                "writable",
                "writeable",
                "disk_index",
                "sound_only",
                "dispose",
                "load_flag",
                "value",
                "inverted",
                "ovha",
                "no_thread",
            ],
        ),
        (
            "mame_disk_claims",
            vec![
                "size",
                "offset",
                "bios",
                "sound_only",
                "dispose",
                "load_flag",
                "value",
                "inverted",
                "ovha",
                "no_thread",
                "writeable",
            ],
        ),
    ] {
        for field in absent {
            // A VIRTUAL numeric size is a query convenience, not persisted data.
            let count = sql_query(format!("SELECT count(*) AS count FROM pragma_table_xinfo('{table}') WHERE name='{field}' AND hidden<>2"))
                .get_result::<Count>(&mut connection)?;
            assert_eq!(count.count, 0, "{table} still stores {field}");
        }
    }
    for query in [
        "SELECT count(*) AS count FROM mame_machine_compatibility WHERE is_consumable=0 AND is_consumable_specified=1",
        "SELECT count(*) AS count FROM mame_rom_compatibility WHERE md5_text='ABCDEF0123456789ABCDEF0123456789' AND sound_only=1 AND dispose=0 AND load_flag='' AND value='0' AND inverted=0 AND ovha='' AND no_thread=1",
        "SELECT count(*) AS count FROM mame_disk_claims JOIN mame_disk_compatibility USING(occurrence_id) WHERE writable=0 AND writable_specified=1 AND writeable=1",
        "SELECT count(*) AS count FROM parser_interpretations WHERE format='mame-listxml' AND rules_version='mame-observed-compat-declared-text-v1'",
    ] {
        assert_eq!(
            sql_query(query).get_result::<Count>(&mut connection)?.count,
            1
        );
    }
    Ok(())
}

#[test]
fn referenced_interpretations_reject_replacement_and_deletion_without_foreign_keys() -> TestResult {
    for statement in [
        "INSERT OR REPLACE INTO parser_interpretations(interpretation_key,format,parser_name,parser_version,rules_version) SELECT interpretation_key,'logiqx',parser_name,parser_version,rules_version FROM parser_interpretations WHERE format='mame-listxml'",
        "INSERT OR REPLACE INTO parser_interpretations(interpretation_key,format,parser_name,parser_version,rules_version) SELECT interpretation_key,format,parser_name,parser_version,'pinned-strict' FROM parser_interpretations WHERE format='mame-listxml'",
        "DELETE FROM parser_interpretations WHERE format='mame-listxml'",
    ] {
        let (_directory, mut connection) = import(
            "<mame mameconfig='10'><machine name='system' isconsumable='yes'><description>System</description></machine></mame>",
        )?;
        diesel::connection::SimpleConnection::batch_execute(
            &mut connection,
            "PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;",
        )?;
        let error = sql_query(statement)
            .execute(&mut connection)
            .err()
            .ok_or("referenced interpretation was rewritten beneath native compatibility facts")?;
        assert!(
            error
                .to_string()
                .contains("referenced parser interpretations are immutable"),
            "{error}"
        );
        sql_query("INSERT INTO parser_interpretations(interpretation_key,format,parser_name,parser_version,rules_version) SELECT interpretation_key,format,parser_name,parser_version,rules_version FROM parser_interpretations WHERE format='mame-listxml' ON CONFLICT(interpretation_key) DO NOTHING").execute(&mut connection)?;
    }
    Ok(())
}

#[test]
fn nodump_and_unproven_historical_loading_keep_literals_without_whole_file_identity() -> TestResult
{
    let (_directory,mut connection)=import(
        "<mame mameconfig='10'><machine name='system'><description>System</description>
         <rom name='normal' size='16' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/>
         <rom name='nodump' size='16' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' status='nodump'/>
         <rom name='legacy' size='16' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' loadflag='continue'/>
         <disk name='disk' sha1='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb' status='nodump'/>
         </machine></mame>",
    )?;
    let count=sql_query("SELECT count(*) AS count FROM mame_rom_claims JOIN asset_occurrences USING(occurrence_id) WHERE name IN ('nodump','legacy') AND sha1_text='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' AND evidence_scope='unknown' AND content_uuid IS NULL")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(
        count, 2,
        "unproven declarations received whole-file identity or scope"
    );
    let count=sql_query("SELECT count(*) AS count FROM mame_disk_claims JOIN asset_occurrences USING(occurrence_id) WHERE name='disk' AND evidence_scope='unknown' AND content_uuid IS NULL")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(count, 1);
    let count=sql_query("SELECT count(*) AS count FROM mame_rom_claims JOIN asset_occurrences USING(occurrence_id) WHERE name='normal' AND evidence_scope='whole_asset' AND length(content_uuid)=16")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(count, 1);
    let count=sql_query("SELECT count(*) AS count FROM occurrence_digest_assertions WHERE scope='unknown' AND provenance='source_declared'")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(
        count, 3,
        "valid literal metadata should remain queryable as scoped evidence"
    );
    Ok(())
}

#[test]
fn reinterning_a_digest_cannot_rekey_published_source_evidence() -> TestResult {
    let (_directory, mut connection) = import(
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='rom' size='16' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/></machine></mame>",
    )?;
    diesel::connection::SimpleConnection::batch_execute(
        &mut connection,
        "PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;",
    )?;
    let original = sql_query("SELECT digest_id AS count FROM digest_values WHERE algorithm='sha1'")
        .get_result::<Count>(&mut connection)?
        .count;
    sql_query("INSERT OR REPLACE INTO digest_values(digest_id,algorithm,digest) SELECT digest_id+1000,algorithm,digest FROM digest_values WHERE digest_id=?")
        .bind::<BigInt,_>(original).execute(&mut connection)?;
    let preserved = sql_query("SELECT count(*) AS count FROM digest_values WHERE digest_id=?")
        .bind::<BigInt, _>(original)
        .get_result::<Count>(&mut connection)?
        .count;
    assert_eq!(
        preserved, 1,
        "reinterning changed the owner ID of published evidence"
    );
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm,digest) VALUES ('sha1',X'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA')")
        .execute(&mut connection)?;
    let dangling=sql_query("SELECT count(*) AS count FROM occurrence_digest_assertions LEFT JOIN digest_values USING(digest_id) WHERE digest_values.digest_id IS NULL")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(dangling, 0);
    sql_query("INSERT INTO digest_values(algorithm,digest) VALUES ('sha1',X'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB')")
        .execute(&mut connection)?;
    let error=sql_query("INSERT OR REPLACE INTO digest_values(digest_id,algorithm,digest) VALUES (?,'sha1',X'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB')")
        .bind::<BigInt,_>(original).execute(&mut connection).err()
        .ok_or("conflicting existing digest ID was silently ignored rather than rejected")?;
    assert!(
        error
            .to_string()
            .contains("interned digest identities are immutable"),
        "{error}"
    );
    Ok(())
}
