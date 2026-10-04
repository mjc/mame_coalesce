use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_clrmamepro::{ClrMameProPageLimit, ClrMameProSetChild, sets_for_snapshot},
    catalog_files::{
        CatalogFileOccurrence, ClrMameProDumpStatus, ClrMameProEvidenceScope,
        ClrMameProFilePayload, ClrMameProRomField, ContentOccurrenceLimit, DigestProvenance,
        occurrences_for_content, occurrences_for_ids,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, OccurrenceId, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct MediaId {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[test]
fn existing_file_api_returns_the_complete_source_free_cmp_rom_payload() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "database path is not UTF-8")?;
    let input = Utf8PathBuf::from_path_buf(directory.path().join("catalog.dat"))
        .map_err(|_| "input path is not UTF-8")?;
    std::fs::write(
        &input,
        r#"set ( name "set" rom (
            NAME "native.bin" SIZE 00016 CRC AABBCCDD CRC32 aabbccdd
            MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
            SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
            DATE "" SERIAL "001" STATUS "nodump" NODUMP BADDUMP
        ) )"#,
    )?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: input.clone(),
            format: CatalogDocumentFormat::ClrMamePro,
            source_key: PublishingSourceKey::new("cmp-payload"),
            source_display_name: "CMP payload".into(),
            catalog_key: CatalogKey::new("cmp-payload"),
            catalog_display_name: "CMP payload".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let row = sql_query("SELECT occurrence_id FROM asset_occurrences")
        .get_result::<MediaId>(&mut connection)?;
    let occurrence = OccurrenceId::from_database(row.occurrence_id);
    std::fs::rename(&input, directory.path().join("unavailable.dat"))?;
    std::fs::rename(
        format!("{database_path}.documents"),
        directory.path().join("objects-unavailable"),
    )?;
    let files = occurrences_for_ids(&database, &[occurrence])?;
    let file = files.first().ok_or("missing file occurrence")?;
    let Some(ClrMameProFilePayload::Rom(payload)) = &file.clrmamepro_file else {
        return Err("missing CMP ROM payload".into());
    };
    assert_eq!(payload.name, "native.bin");
    assert_eq!(payload.size_text.as_deref(), Some("00016"));
    assert_eq!(payload.size, Some(16));
    assert_eq!(payload.crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(payload.crc32_text.as_deref(), Some("aabbccdd"));
    assert_eq!(payload.date.as_deref(), Some(""));
    assert_eq!(payload.serial.as_deref(), Some("001"));
    assert_eq!(payload.status_text.as_deref(), Some("nodump"));
    assert!(payload.nodump_present);
    assert!(payload.baddump_present);
    assert!(payload.dump_status.is_none());
    assert!(file.content_id.is_none());
    Ok(())
}

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    input: Utf8PathBuf,
    ids: Vec<OccurrenceId>,
    snapshot: SnapshotKey,
}

impl Fixture {
    fn new(source: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "database path is not UTF-8")?;
        let input = Utf8PathBuf::from_path_buf(directory.path().join("catalog.dat"))
            .map_err(|_| "input path is not UTF-8")?;
        std::fs::write(&input, source)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: input.clone(),
                format: CatalogDocumentFormat::ClrMamePro,
                source_key: PublishingSourceKey::new("cmp-payload"),
                source_display_name: "CMP payload".into(),
                catalog_key: CatalogKey::new("cmp-payload"),
                catalog_display_name: "CMP payload".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(
            report.status,
            CatalogImportStatus::Succeeded,
            "fixture must publish"
        );
        let snapshot = report.snapshot_key.ok_or("fixture snapshot is missing")?;
        let mut connection = SqliteConnection::establish(database_path.as_str())?;
        let ids = sql_query("SELECT occurrence_id FROM asset_occurrences ORDER BY occurrence_id")
            .load::<MediaId>(&mut connection)?
            .into_iter()
            .map(|row| OccurrenceId::from_database(row.occurrence_id))
            .collect();
        Ok(Self {
            directory,
            database,
            database_path,
            input,
            ids,
            snapshot,
        })
    }

    fn unavailable_sources(&self) -> TestResult {
        std::fs::rename(&self.input, self.directory.path().join("unavailable.dat"))?;
        std::fs::rename(
            format!("{}.documents", self.database_path),
            self.directory.path().join("objects-unavailable"),
        )?;
        Ok(())
    }

    fn files(&self) -> TestResult<Vec<CatalogFileOccurrence>> {
        Ok(occurrences_for_ids(&self.database, &self.ids)?)
    }

    fn mutate(&self, table: &str, statement: &str) -> TestResult {
        let mut connection = SqliteConnection::establish(self.database_path.as_str())?;
        let guards = sql_query(
            "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
        )
        .bind::<Text, _>(table)
        .load::<Guard>(&mut connection)?;
        connection.batch_execute("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")?;
        for guard in &guards {
            connection.batch_execute(&format!(
                "DROP TRIGGER \"{}\"",
                guard.name.replace('"', "\"\"")
            ))?;
        }
        connection.batch_execute(statement)?;
        for guard in guards {
            connection.batch_execute(&guard.sql)?;
        }
        connection.batch_execute("PRAGMA ignore_check_constraints=OFF; PRAGMA foreign_keys=ON;")?;
        Ok(())
    }
}

#[derive(QueryableByName)]
struct Guard {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

const COMPLETE_SOURCE: &str = r#"set (
 NAME "" vendor "ignored" SaMpLe ""
 ROM ( NAME "" vendor "gap" SIZE 0009223372036854775807 CRC AABBCCDD CRC32 aabbccdd
       MD5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
       MERGE "" DATE "" SERIAL "" STATUS "mystery" NODUMP BADDUMP )
 sample click
 ROM ( NAME lone SHA1 bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb NODUMP )
 ROM ( NAME aliases CRC AABBCCDD CRC32 11223344 SHA1 cccccccccccccccccccccccccccccccccccccccc )
 ROM ( NAME unknown STATUS "mystery" )
 ROM ( NAME linked SHA1 dddddddddddddddddddddddddddddddddddddddd )
)"#;

fn rom(
    file: &CatalogFileOccurrence,
) -> TestResult<&mame_coalesce::catalog_files::ClrMameProRomPayload> {
    match &file.clrmamepro_file {
        Some(ClrMameProFilePayload::Rom(payload)) => Ok(payload),
        _ => Err("expected native CMP ROM".into()),
    }
}

fn at<T>(values: &[T], index: usize) -> TestResult<&T> {
    values.get(index).ok_or_else(|| {
        format!(
            "missing element {index} from {} returned values",
            values.len()
        )
        .into()
    })
}

#[test]
fn all_twelve_fields_samples_and_identity_boundaries_are_source_free() -> TestResult {
    let fixture = Fixture::new(COMPLETE_SOURCE)?;
    fixture.unavailable_sources()?;
    let files = fixture.files()?;
    assert_eq!(files.len(), 7);
    let empty_sample = at(&files, 0)?;
    let declared = at(&files, 1)?;
    let lone_nodump = at(&files, 3)?;
    let aliases = at(&files, 4)?;
    let unknown_status = at(&files, 5)?;
    let linked = at(&files, 6)?;
    let Some(ClrMameProFilePayload::Sample(sample)) = &empty_sample.clrmamepro_file else {
        return Err("missing empty scalar sample".into());
    };
    assert_eq!(sample.name, "");
    assert_eq!(sample.position.source_field, "SaMpLe");
    assert_eq!(sample.position.source_order, 2);
    assert!(sample.position.is_quoted);
    assert!(empty_sample.content_id.is_none());
    let payload = rom(declared)?;
    assert_eq!(payload.name, "");
    assert_eq!(payload.source_order, 3);
    assert_eq!(payload.size, Some(i64::MAX));
    assert_eq!(payload.size_text.as_deref(), Some("0009223372036854775807"));
    assert_eq!(payload.evidence_scope, ClrMameProEvidenceScope::WholeAsset);
    assert_eq!(payload.field_positions.len(), 12);
    assert_eq!(
        payload
            .field_positions
            .iter()
            .map(|field| field.field as i64)
            .collect::<Vec<_>>(),
        (0..12).collect::<Vec<_>>()
    );
    assert_eq!(
        payload
            .field_positions
            .iter()
            .map(|field| field.position.source_order)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(
        at(&payload.field_positions, 1)?.position.source_field,
        "SIZE"
    );
    assert!(at(&payload.field_positions, 0)?.position.is_quoted);
    assert!(!at(&payload.field_positions, 10)?.position.is_quoted);
    assert_eq!(
        at(&payload.field_positions, 6)?.field,
        ClrMameProRomField::Merge
    );
    let merge = payload
        .merge
        .as_ref()
        .ok_or("missing empty merge declaration")?;
    assert_eq!(merge.merge_name, "");
    assert!(merge.relationship_id.database_value() > 0);
    assert!(declared.content_id.is_none());
    assert_eq!(
        rom(lone_nodump)?.dump_status,
        Some(ClrMameProDumpStatus::NoDump)
    );
    assert!(
        lone_nodump.content_id.is_some(),
        "lone nodump must not ban eligible SHA1"
    );
    assert!(
        aliases.content_id.is_none(),
        "unequal CRC aliases suppress UUID despite SHA1"
    );
    assert_eq!(
        aliases.digests.len(),
        3,
        "both CRC declarations and SHA1 stay visible"
    );
    assert_eq!(rom(unknown_status)?.status_text.as_deref(), Some("mystery"));
    assert_eq!(rom(unknown_status)?.dump_status, None);
    let id = linked.content_id.ok_or("eligible ROM missing identity")?;
    let page =
        occurrences_for_content(&fixture.database, id, ContentOccurrenceLimit::new(1)?, None)?;
    assert_eq!(page.occurrences.as_slice(), std::slice::from_ref(linked));
    Ok(())
}

#[test]
fn computed_sample_evidence_is_separate_and_does_not_assign_identity() -> TestResult {
    let fixture = Fixture::new("set ( name s sample x rom ( name r crc AABBCCDD ) )")?;
    let _control = fixture.files()?;
    fixture.mutate(
        "occurrence_digest_assertions",
        "INSERT INTO digest_values(digest_id,algorithm,digest) VALUES (90000,'sha1',zeroblob(20));
         INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
         SELECT MIN(occurrence_id),90000,'test_computed_scope','computed' FROM cmp_samples;",
    )?;
    let files = fixture.files()?;
    let sample = at(&files, 0)?;
    assert!(sample.content_id.is_none());
    assert_eq!(sample.digests.len(), 1);
    let digest = sample
        .digests
        .first()
        .ok_or("missing computed sample digest")?;
    assert_eq!(digest.provenance, DigestProvenance::Computed);
    assert_eq!(digest.scope, "test_computed_scope");
    Ok(())
}

const CORRUPTION_SOURCE: &str = "set ( name s sample x rom ( name r size 0004 crc AABBCCDD sha1 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa merge parent ) )";

fn assert_corruption_rejected(table: &str, mutation: &str) -> TestResult {
    let fixture = Fixture::new(CORRUPTION_SOURCE)?;
    assert_eq!(
        fixture.files()?.len(),
        2,
        "public control must load before mutation"
    );
    fixture.mutate(table, mutation)?;
    assert!(
        occurrences_for_ids(&fixture.database, &fixture.ids).is_err(),
        "accepted {mutation}"
    );
    Ok(())
}

#[test]
fn native_payload_reader_rejects_coercible_storage_and_missing_owners() -> TestResult {
    for (table, mutation) in [
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET size_text='4junk'",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET size_text='9223372036854775808'",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET crc_text='not-a-hash'",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET nodump_present=0.5",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET source_line=1.5",
        ),
        ("cmp_samples", "UPDATE cmp_samples SET is_quoted=0.5"),
        ("cmp_samples", "UPDATE cmp_samples SET source_order=2.5"),
        (
            "cmp_set_rom_positions",
            "UPDATE cmp_set_rom_positions SET source_order=1.5",
        ),
        (
            "cmp_rom_field_positions",
            "UPDATE cmp_rom_field_positions SET field_kind=2.5 WHERE field_kind=2",
        ),
        (
            "cmp_rom_field_positions",
            "DELETE FROM cmp_rom_field_positions WHERE field_kind=1",
        ),
        ("cmp_rom_claims", "DELETE FROM cmp_rom_claims"),
        ("cmp_samples", "DELETE FROM cmp_samples"),
    ] {
        assert_corruption_rejected(table, mutation)?;
    }
    Ok(())
}

#[test]
fn native_assertion_reader_checks_dictionary_and_bidirectional_source_agreement() -> TestResult {
    for (table, mutation) in [
        (
            "occurrence_digest_assertions",
            "UPDATE occurrence_digest_assertions SET digest_id=999999999 WHERE digest_id=(SELECT MIN(digest_id) FROM occurrence_digest_assertions)",
        ),
        (
            "occurrence_digest_assertions",
            "UPDATE occurrence_digest_assertions SET scope='rom_segment'",
        ),
        (
            "occurrence_digest_assertions",
            "UPDATE occurrence_digest_assertions SET provenance='computed'",
        ),
        (
            "occurrence_digest_assertions",
            "DELETE FROM occurrence_digest_assertions",
        ),
        (
            "digest_values",
            "UPDATE digest_values SET digest=zeroblob(3) WHERE algorithm='crc32'",
        ),
        (
            "digest_values",
            "UPDATE digest_values SET digest='abcd' WHERE algorithm='crc32'",
        ),
        (
            "digest_values",
            "UPDATE digest_values SET algorithm='unknown' WHERE algorithm='crc32'",
        ),
        (
            "digest_values",
            "UPDATE digest_values SET digest=zeroblob(4) WHERE algorithm='crc32'",
        ),
        (
            "occurrence_digest_assertions",
            "INSERT INTO digest_values(digest_id,algorithm,digest) VALUES (90000,'md5',zeroblob(16)); INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) SELECT MIN(occurrence_id),90000,'whole_asset','source_declared' FROM cmp_rom_claims",
        ),
        (
            "occurrence_digest_assertions",
            "INSERT INTO digest_values(digest_id,algorithm,digest) VALUES (90000,'md5',zeroblob(16)); INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) SELECT MIN(occurrence_id),90000,'whole_asset','source_declared' FROM cmp_samples",
        ),
    ] {
        assert_corruption_rejected(table, mutation)?;
    }
    Ok(())
}

#[test]
fn native_merge_and_uuid_reader_requires_actual_source_owners() -> TestResult {
    for (table, mutation) in [
        (
            "clrmamepro_rom_merges",
            "UPDATE clrmamepro_rom_merges SET relationship_id=999999999",
        ),
        (
            "catalog_relationships",
            "UPDATE catalog_relationships SET origin='user',snapshot_key=NULL",
        ),
        (
            "reported_catalog_relationships",
            "UPDATE reported_catalog_relationships SET source_reference_kind='clrmamepro_cloneof'",
        ),
        (
            "asset_occurrences",
            "UPDATE asset_occurrences SET content_uuid=zeroblob(16) WHERE claim_kind='cmp_rom'",
        ),
        (
            "catalog_contents",
            "UPDATE catalog_contents SET registry_id=999999999",
        ),
        (
            "file_id_registries",
            "UPDATE file_id_registries SET registry_uuid='abcdefghijklmnop'",
        ),
        (
            "asset_occurrences",
            "UPDATE asset_occurrences SET content_uuid=(SELECT content_uuid FROM catalog_contents LIMIT 1) WHERE claim_kind='cmp_sample'",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET crc32_text='11223344'",
        ),
        (
            "cmp_rom_claims",
            "UPDATE cmp_rom_claims SET nodump_present=1,baddump_present=1",
        ),
    ] {
        assert_corruption_rejected(table, mutation)?;
    }
    Ok(())
}

#[test]
fn samples_reject_orphan_rom_form_positions_in_both_public_queries() -> TestResult {
    let fixture = Fixture::new("set ( name s sample x )")?;
    fixture.unavailable_sources()?;
    let limit = ClrMameProPageLimit::new(1)?;
    let read_page = || sets_for_snapshot(&fixture.database, &fixture.snapshot, None, limit);
    let files = fixture.files()?;
    assert_eq!(files.len(), 1);
    assert!(matches!(
        at(&files, 0)?.clrmamepro_file,
        Some(ClrMameProFilePayload::Sample(_))
    ));
    let page = read_page()?;
    assert_eq!(page.sets.len(), 1);
    assert!(
        at(&page.sets, 0)?
            .children
            .iter()
            .any(|child| matches!(child, ClrMameProSetChild::Sample(_)))
    );
    fixture.mutate(
        "cmp_set_rom_positions",
        "INSERT INTO cmp_set_rom_positions (occurrence_id,source_order)
         SELECT occurrence_id,2 FROM cmp_samples",
    )?;
    let bulk_rejected = fixture.files().is_err();
    let metadata_rejected = read_page().is_err();
    assert_eq!(
        (bulk_rejected, metadata_rejected),
        (true, true),
        "samples cannot own ROM-form positions in either public API"
    );
    Ok(())
}
