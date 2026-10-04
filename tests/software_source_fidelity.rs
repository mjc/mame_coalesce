use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{
        self, DigestAlgorithm, DigestProvenance, OccurrenceDigest, OccurrenceId,
        SoftwareFileOperation, SoftwareFilePayload, SoftwareLoadInstruction,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LOWER_SHA1: &str = "abcdef0123abcdef0123abcdef0123abcdef0123";
const UPPER_SHA1: &str = "ABCDEF0123ABCDEF0123ABCDEF0123ABCDEF0123";

struct ImportedSoftware {
    _directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
    snapshot_key: String,
}

#[derive(QueryableByName)]
struct IdentitySourceRow {
    #[diesel(sql_type = BigInt)]
    list_order: i64,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_order: i64,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    offset_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    operation: String,
    #[diesel(sql_type = Nullable<Text>)]
    load_instruction: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_occurrence_id: Option<i64>,
    #[diesel(sql_type = BigInt)]
    declares_file: i64,
}

#[derive(QueryableByName)]
struct SourceDigestRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
    #[diesel(sql_type = Text)]
    scope: String,
    #[diesel(sql_type = Text)]
    provenance: String,
}

#[derive(QueryableByName)]
struct RegistryCounts {
    #[diesel(sql_type = BigInt)]
    issued: i64,
    #[diesel(sql_type = BigInt)]
    orphaned: i64,
    #[diesel(sql_type = BigInt)]
    malformed: i64,
}

fn import_software(xml: &str) -> TestResult<ImportedSoftware> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("identity.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-identity-source-fidelity"),
            source_display_name: "Software identity source fidelity".to_owned(),
            catalog_key: CatalogKey::new("software-identity-source-fidelity"),
            catalog_display_name: "Software identity source fidelity".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot_key = report
        .snapshot_key
        .ok_or("successful import did not return a snapshot")?
        .to_string();
    Ok(ImportedSoftware {
        _directory: directory,
        database,
        connection: SqliteConnection::establish(database_path.as_str())?,
        snapshot_key,
    })
}

fn identity_sources(imported: &mut ImportedSoftware) -> TestResult<Vec<IdentitySourceRow>> {
    Ok(sql_query(
        "SELECT groups.list_order, occurrence.record_id, occurrence.occurrence_id, \
                occurrence.occurrence_order, COALESCE(rom.component_order, disk.component_order) AS component_order, \
                COALESCE(rom.source_order, disk.source_order) AS source_order, \
                COALESCE(rom.name, disk.name) AS name, rom.crc_text, \
                COALESCE(rom.sha1_text, disk.sha1_text) AS sha1_text, rom.size_text, rom.offset_text, \
                rom.value, occurrence.claim_kind, occurrence.content_uuid, \
                COALESCE(rom.evidence_scope, disk.evidence_scope) AS evidence_scope, \
                file_use.operation, rom.load_instruction, file_use.declaration_occurrence_id, \
                EXISTS (SELECT 1 FROM software_file_declarations AS declaration \
                        WHERE declaration.occurrence_id = occurrence.occurrence_id) AS declares_file \
         FROM catalog_set_groups AS groups \
         JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id \
         JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id \
         LEFT JOIN software_rom_entries AS rom ON rom.occurrence_id = occurrence.occurrence_id \
         LEFT JOIN software_disk_entries AS disk ON disk.occurrence_id = occurrence.occurrence_id \
         JOIN software_file_uses AS file_use ON file_use.occurrence_id = occurrence.occurrence_id \
         WHERE groups.snapshot_key = ? ORDER BY groups.list_order, sets.list_order, occurrence.occurrence_order",
    )
    .bind::<Text, _>(&imported.snapshot_key)
    .load(&mut imported.connection)?)
}

fn source_digests(imported: &mut ImportedSoftware) -> TestResult<Vec<SourceDigestRow>> {
    Ok(sql_query(
        "SELECT assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance \
         FROM catalog_set_groups AS groups \
         JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id \
         JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id \
         JOIN occurrence_digest_assertions AS assertion ON assertion.occurrence_id = occurrence.occurrence_id \
         JOIN digest_values AS digest ON digest.digest_id = assertion.digest_id \
         WHERE groups.snapshot_key = ? ORDER BY assertion.occurrence_id, digest.algorithm",
    )
    .bind::<Text, _>(&imported.snapshot_key)
    .load(&mut imported.connection)?)
}

fn assert_source_digests(
    digests: &[SourceDigestRow],
    occurrence_id: i64,
    scope: &str,
    expected: &[(&str, &str)],
) -> TestResult {
    let actual: Vec<_> = digests
        .iter()
        .filter(|digest| digest.occurrence_id == occurrence_id)
        .collect();
    assert_eq!(actual.len(), expected.len());
    for (actual, (algorithm, value)) in actual.into_iter().zip(expected) {
        assert_eq!(actual.algorithm, *algorithm);
        assert_eq!(actual.digest, hex::decode(value)?);
        assert_eq!(actual.scope, scope);
        assert_eq!(actual.provenance, "source_declared");
    }
    Ok(())
}

fn assert_registry_size(connection: &mut SqliteConnection, expected: i64) -> TestResult {
    let counts = sql_query(
        "SELECT COUNT(*) AS issued, \
                (SELECT COUNT(*) FROM catalog_contents AS content WHERE NOT EXISTS ( \
                    SELECT 1 FROM asset_occurrences AS occurrence WHERE occurrence.content_uuid = content.content_uuid \
                )) AS orphaned, \
                (SELECT COUNT(*) FROM catalog_contents WHERE length(content_uuid) <> 16) AS malformed \
         FROM catalog_contents",
    )
    .get_result::<RegistryCounts>(connection)?;
    assert_eq!(counts.issued, expected);
    assert_eq!(counts.orphaned, 0);
    assert_eq!(counts.malformed, 0);
    Ok(())
}

#[derive(QueryableByName)]
struct SourceFields {
    #[diesel(sql_type = Text)]
    envelope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
    #[diesel(sql_type = Text)]
    declared_size_text: String,
    #[diesel(sql_type = BigInt)]
    declared_size: i64,
    #[diesel(sql_type = Text)]
    size_text: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
    #[diesel(sql_type = Text)]
    offset_text: String,
    #[diesel(sql_type = BigInt)]
    offset: i64,
}

#[derive(QueryableByName)]
struct RawNumericFields {
    #[diesel(sql_type = Nullable<Text>)]
    declared_size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    declared_size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    offset_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    offset: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
}

#[test]
fn native_software_source_text_remains_the_single_numeric_source()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        r#"<softwarelist name="list"><software name="game">
          <description>Game</description><year>2000</year><publisher>Example</publisher>
          <part name="cart" interface="cart"><dataarea name="rom" size="020">
            <rom name="file.bin" size="010" offset="0x10"/>
          </dataarea></part></software></softwarelist>"#,
    )?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-source-fidelity"),
            source_display_name: "Software source fidelity".to_owned(),
            catalog_key: CatalogKey::new("software-source-fidelity"),
            catalog_display_name: "Software source fidelity".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let source = sql_query(
        "SELECT document.envelope_kind, header.build, data_area.declared_size_text, \
                data_area.declared_size, rom.size_text, rom.size, rom.offset_text, rom.offset \
         FROM software_documents AS document \
         LEFT JOIN software_wrapper_headers AS header USING (snapshot_key) \
         JOIN catalog_snapshots USING (snapshot_key) \
         JOIN catalog_sets ON catalog_sets.set_group_id = ( \
           SELECT namespace_id FROM software_lists WHERE namespace_id = catalog_sets.set_group_id) \
         JOIN software_parts AS part ON part.record_id = catalog_sets.set_id \
         JOIN software_areas AS area ON area.part_id = part.part_id AND area.area_kind = 'data' \
         JOIN software_data_areas AS data_area USING (area_id) \
         JOIN software_rom_entries AS rom USING (area_id, record_id) \
         LIMIT 1",
    )
    .get_result::<SourceFields>(&mut connection)?;

    assert_eq!(source.envelope_kind, "single_list");
    assert_eq!(source.build, None);
    assert_eq!(source.declared_size_text, "020");
    assert_eq!(source.declared_size, 16);
    assert_eq!(source.size_text, "010");
    assert_eq!(source.size, 8);
    assert_eq!(source.offset_text, "0x10");
    assert_eq!(source.offset, 16);
    Ok(())
}

#[test]
fn raw_numeric_hash_text_survives_unresolved_and_empty_interpretations()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("source.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        r#"<softwarelist name="list"><software name="game"><description>Game</description><year>2000</year><publisher>Test</publisher><part name="cart" interface="cart"><dataarea name="rom" size="0x8000000000000000"><rom name="bad.bin" size="0x8000000000000000" offset="09" crc="bad-crc" sha1="1111111111111111111111111111111111111111"/><rom name="empty.bin" size="+0000000000000008" offset="0x0000000000000008" crc="" sha1=""/><rom name="octal.bin" size="00010" offset="00011"/></dataarea></part></software></softwarelist>"#,
    )?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-source-raw-numeric"),
            source_display_name: "Software raw numeric fidelity".to_owned(),
            catalog_key: CatalogKey::new("software-source-raw-numeric"),
            catalog_display_name: "Software raw numeric fidelity".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot_key = report
        .snapshot_key
        .ok_or("successful import did not return a snapshot")?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let rows = sql_query(
        "SELECT data_area.declared_size_text, data_area.declared_size, rom.name, rom.size_text, rom.size, \
                rom.offset_text, rom.offset, rom.crc_text, rom.sha1_text, occurrence.content_uuid \
         FROM snapshot_publications AS publication \
         JOIN catalog_set_groups AS groups ON groups.snapshot_key = publication.snapshot_key \
         JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id \
         JOIN software_items AS items ON items.record_id = sets.set_id \
         JOIN software_parts AS parts ON parts.record_id = items.record_id \
         JOIN software_areas AS area ON area.part_id = parts.part_id AND area.area_kind = 'data' \
         JOIN software_data_areas AS data_area USING (area_id) \
         JOIN software_rom_entries AS rom ON rom.area_id = area.area_id \
         JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id = rom.occurrence_id \
         WHERE publication.snapshot_key = ? ORDER BY rom.component_order",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .load::<RawNumericFields>(&mut connection)?;

    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].declared_size_text.as_deref(),
        Some("0x8000000000000000")
    );
    assert_eq!(rows[0].declared_size, None);
    assert_eq!(rows[0].name.as_deref(), Some("bad.bin"));
    assert_eq!(rows[0].size_text.as_deref(), Some("0x8000000000000000"));
    assert_eq!(rows[0].size, None);
    assert_eq!(rows[0].offset_text.as_deref(), Some("09"));
    assert_eq!(rows[0].offset, None);
    assert_eq!(rows[0].crc_text.as_deref(), Some("bad-crc"));
    assert_eq!(
        rows[0].sha1_text.as_deref(),
        Some("1111111111111111111111111111111111111111")
    );
    assert_eq!(rows[0].content_uuid, None);
    assert_eq!(rows[1].crc_text.as_deref(), Some(""));
    assert_eq!(rows[1].sha1_text.as_deref(), Some(""));
    assert_eq!(rows[1].size_text.as_deref(), Some("+0000000000000008"));
    assert_eq!(rows[1].size, None);
    assert_eq!(rows[1].offset, Some(8));
    assert_eq!(rows[1].content_uuid, None);
    assert_eq!(rows[2].size_text.as_deref(), Some("00010"));
    assert_eq!(rows[2].size, Some(8));
    assert_eq!(rows[2].offset_text.as_deref(), Some("00011"));
    assert_eq!(rows[2].offset, Some(9));
    Ok(())
}

#[test]
fn equivalent_sha1_spellings_share_identity_across_distinct_list_owners() -> TestResult {
    let xml = format!(
        r#"<softwarelists>
          <softwarelist name="upper"><software name="game"><description>Upper</description><year>2000</year><publisher>Test</publisher>
            <part name="cart" interface="cart"><dataarea name="rom" size="010">
              <rom name="upper.bin" size="02" offset="0X0" sha1="{UPPER_SHA1}"/>
            </dataarea></part></software></softwarelist>
          <softwarelist name="lower"><software name="game"><description>Lower</description><year>2000</year><publisher>Test</publisher>
            <part name="cart" interface="cart"><dataarea name="rom" size="010">
              <rom name="lower.bin" size="02" offset="0X0" sha1="{LOWER_SHA1}"/>
            </dataarea></part></software></softwarelist>
        </softwarelists>"#
    );
    let mut imported = import_software(&xml)?;
    let rows = identity_sources(&mut imported)?;
    let digests = source_digests(&mut imported)?;
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].record_id, rows[1].record_id);
    assert_ne!(rows[0].occurrence_id, rows[1].occurrence_id);
    assert_eq!(rows[0].list_order, 0);
    assert_eq!(rows[1].list_order, 1);
    assert_eq!(rows[0].sha1_text.as_deref(), Some(UPPER_SHA1));
    assert_eq!(rows[1].sha1_text.as_deref(), Some(LOWER_SHA1));
    let shared_uuid = rows[0].content_uuid.as_ref().ok_or("missing shared UUID")?;
    assert_eq!(shared_uuid.len(), 16);
    assert_eq!(rows[1].content_uuid.as_ref(), Some(shared_uuid));
    for row in &rows {
        assert_eq!(row.occurrence_order, 0);
        assert_eq!(row.component_order, 0);
        assert_eq!(row.source_order, 0);
        assert_eq!(row.size_text.as_deref(), Some("02"));
        assert_eq!(row.offset_text.as_deref(), Some("0X0"));
        assert_eq!(row.crc_text, None);
        assert_eq!(row.claim_kind, "software_rom_entry");
        assert_eq!(row.evidence_scope, "whole_asset");
        assert_eq!(row.declares_file, 1);
        assert_source_digests(
            &digests,
            row.occurrence_id,
            "whole_asset",
            &[("sha1", LOWER_SHA1)],
        )?;
    }
    assert_eq!(digests.len(), 2);
    assert_registry_size(&mut imported.connection, 1)?;
    Ok(())
}

#[test]
fn malformed_or_empty_source_hashes_never_issue_identity_even_with_other_valid_evidence()
-> TestResult {
    let mut imported = import_software(
        r#"<softwarelist name="list"><software name="game"><description>Game</description><year>2000</year><publisher>Test</publisher>
          <part name="cart" interface="cart"><dataarea name="rom" size="8">
            <rom name="bad-crc.bin" size="2" crc="BAD-CRC" sha1="1111111111111111111111111111111111111111"/>
            <rom name="empty-crc.bin" size="2" crc="" sha1="2222222222222222222222222222222222222222"/>
            <rom name="bad-sha.bin" size="2" crc="ABCDEF01" sha1="bad-sha"/>
            <rom name="empty-sha.bin" size="2" crc="1234ABCD" sha1=""/>
          </dataarea></part></software></softwarelist>"#,
    )?;
    let rows = identity_sources(&mut imported)?;
    let digests = source_digests(&mut imported)?;
    let expected = [
        (
            "bad-crc.bin",
            "BAD-CRC",
            "1111111111111111111111111111111111111111",
            "sha1",
            "1111111111111111111111111111111111111111",
        ),
        (
            "empty-crc.bin",
            "",
            "2222222222222222222222222222222222222222",
            "sha1",
            "2222222222222222222222222222222222222222",
        ),
        ("bad-sha.bin", "ABCDEF01", "bad-sha", "crc32", "abcdef01"),
        ("empty-sha.bin", "1234ABCD", "", "crc32", "1234abcd"),
    ];
    assert_eq!(rows.len(), expected.len());
    for (order, (row, (name, crc, sha1, algorithm, digest))) in
        rows.iter().zip(expected).enumerate()
    {
        assert_eq!(row.name, name);
        assert_eq!(row.crc_text.as_deref(), Some(crc));
        assert_eq!(row.sha1_text.as_deref(), Some(sha1));
        assert_eq!(row.occurrence_order, i64::try_from(order)?);
        assert_eq!(row.component_order, i64::try_from(order)?);
        assert_eq!(row.source_order, i64::try_from(order)?);
        assert_eq!(row.claim_kind, "software_rom_entry");
        assert_eq!(row.content_uuid, None);
        assert_source_digests(
            &digests,
            row.occurrence_id,
            "whole_asset",
            &[(algorithm, digest)],
        )?;
    }
    assert_eq!(digests.len(), 4);
    assert_registry_size(&mut imported.connection, 0)?;
    Ok(())
}

#[test]
fn named_hash_bearing_rom_operations_keep_their_own_assertions_without_identity() -> TestResult {
    let xml = format!(
        r#"<softwarelist name="list"><software name="game"><description>Game</description><year>2000</year><publisher>Test</publisher>
          <part name="cart" interface="cart"><dataarea name="rom" size="020">
            <rom name="program.bin" size="8" offset="0" crc="12345678" sha1="1111111111111111111111111111111111111111"/>
            <rom name="reload.bin" size="02" offset="0x8" loadflag="reload" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
            <rom name="continue.bin" size="02" offset="010" loadflag="continue" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
            <rom name="fill.bin" size="02" offset="+8" loadflag="fill" value="fF" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
            <rom name="plain.bin" size="02" offset="+8" loadflag="reload_plain" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
            <rom name="ignore.bin" size="02" offset="+8" loadflag="ignore" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
          </dataarea></part></software></softwarelist>"#
    );
    let mut imported = import_software(&xml)?;
    let rows = identity_sources(&mut imported)?;
    let digests = source_digests(&mut imported)?;
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].declares_file, 1);
    assert_source_digests(
        &digests,
        rows[0].occurrence_id,
        "whole_asset",
        &[
            ("crc32", "12345678"),
            ("sha1", "1111111111111111111111111111111111111111"),
        ],
    )?;
    assert_eq!(rows[0].content_uuid.as_ref().map(Vec::len), Some(16));
    for (order, (row, (name, operation, offset))) in rows[1..]
        .iter()
        .zip([
            ("reload.bin", "reload", "0x8"),
            ("continue.bin", "continue", "010"),
            ("fill.bin", "fill", "+8"),
            ("plain.bin", "reload_plain", "+8"),
            ("ignore.bin", "ignore", "+8"),
        ])
        .enumerate()
    {
        let order = i64::try_from(order)? + 1;
        assert_eq!(row.name, name);
        assert_eq!(row.operation, operation);
        assert_eq!(row.load_instruction.as_deref(), Some(operation));
        assert_eq!(row.size_text.as_deref(), Some("02"));
        assert_eq!(row.offset_text.as_deref(), Some(offset));
        assert_eq!(row.crc_text.as_deref(), Some("AB12CD34"));
        assert_eq!(row.sha1_text.as_deref(), Some(UPPER_SHA1));
        assert_eq!(row.occurrence_order, order);
        assert_eq!(row.component_order, order);
        assert_eq!(row.source_order, order);
        assert_eq!(row.claim_kind, "software_rom_operation");
        assert_eq!(row.declares_file, 0);
        assert_eq!(row.content_uuid, None);
        assert_eq!(row.evidence_scope, "unknown");
        assert_eq!(
            row.declaration_occurrence_id,
            if operation == "fill" {
                None
            } else {
                Some(rows[0].occurrence_id)
            }
        );
        assert_source_digests(
            &digests,
            row.occurrence_id,
            "unknown",
            &[("crc32", "ab12cd34"), ("sha1", LOWER_SHA1)],
        )?;
    }
    assert_eq!(rows[3].value.as_deref(), Some("fF"));
    assert_eq!(digests.len(), 12);
    assert_registry_size(&mut imported.connection, 1)?;
    assert_queried_rom_operations(&imported, &rows)
}

fn assert_queried_rom_operations(
    imported: &ImportedSoftware,
    rows: &[IdentitySourceRow],
) -> TestResult {
    let files = catalog_files::occurrences_for_ids(
        &imported.database,
        &rows
            .iter()
            .map(|row| OccurrenceId::from_database(row.occurrence_id))
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(files.len(), rows.len());
    for (row, (operation, instruction)) in rows.iter().zip([
        (SoftwareFileOperation::Load, None),
        (
            SoftwareFileOperation::Reload,
            Some(SoftwareLoadInstruction::Reload),
        ),
        (
            SoftwareFileOperation::Continue,
            Some(SoftwareLoadInstruction::Continue),
        ),
        (
            SoftwareFileOperation::Fill,
            Some(SoftwareLoadInstruction::Fill),
        ),
        (
            SoftwareFileOperation::ReloadPlain,
            Some(SoftwareLoadInstruction::ReloadPlain),
        ),
        (
            SoftwareFileOperation::Ignore,
            Some(SoftwareLoadInstruction::Ignore),
        ),
    ]) {
        let file = files
            .iter()
            .find(|file| file.occurrence_id.database_value() == row.occurrence_id)
            .ok_or("queried operation missing")?;
        assert_eq!(file.content_id.is_some(), row.declares_file == 1);
        assert_eq!(file.canonical_content_id.is_some(), row.declares_file == 1);
        assert_eq!(file.provenance.set_id.as_i64(), row.record_id);
        let Some(SoftwareFilePayload::Rom(payload)) = &file.software_file else {
            return Err("ROM payload missing".into());
        };
        assert_eq!(payload.name.as_deref(), Some(row.name.as_str()));
        assert_eq!(payload.evidence_scope, row.evidence_scope);
        assert_eq!(payload.operation, operation);
        assert_eq!(payload.load_instruction, instruction);
        assert_eq!(
            payload
                .declaration_occurrence_id
                .map(OccurrenceId::database_value),
            row.declaration_occurrence_id
        );
        assert!(
            file.digests
                .iter()
                .all(|digest| digest.scope == row.evidence_scope)
        );
        assert_eq!(file.digests.len(), 2);
        let expected = if row.declares_file == 1 {
            [
                (DigestAlgorithm::Crc32, "12345678"),
                (
                    DigestAlgorithm::Sha1,
                    "1111111111111111111111111111111111111111",
                ),
            ]
        } else {
            [
                (DigestAlgorithm::Crc32, "ab12cd34"),
                (DigestAlgorithm::Sha1, LOWER_SHA1),
            ]
        };
        for (algorithm, digest) in expected {
            assert!(file.digests.contains(&OccurrenceDigest {
                algorithm,
                value: hex::decode(digest)?,
                scope: row.evidence_scope.clone(),
                provenance: DigestProvenance::SourceDeclared
            }));
        }
    }
    Ok(())
}

#[test]
fn empty_name_and_nodump_hashes_remain_unproved_source_assertions() -> TestResult {
    for (name, status) in [("", "good"), ("undumped.bin", "nodump")] {
        let xml = format!(
            r#"<softwarelist name="list"><software name="game"><description>Game</description><year>2000</year><publisher>Test</publisher>
              <part name="cart" interface="cart"><dataarea name="rom" size="8">
                <rom name="{name}" size="8" status="{status}" crc="AB12CD34" sha1="{UPPER_SHA1}"/>
              </dataarea></part></software></softwarelist>"#
        );
        let mut imported = import_software(&xml)?;
        let rows = identity_sources(&mut imported)?;
        assert_eq!(rows.len(), 1);
        let row = rows.first().ok_or("ROM row missing")?;
        assert_eq!(row.name, name);
        assert_eq!(row.crc_text.as_deref(), Some("AB12CD34"));
        assert_eq!(row.sha1_text.as_deref(), Some(UPPER_SHA1));
        assert_eq!(row.content_uuid, None, "unproved ROM {name:?}/{status}");
        assert_eq!(row.evidence_scope, "unknown");
        assert_source_digests(
            &source_digests(&mut imported)?,
            row.occurrence_id,
            "unknown",
            &[("crc32", "ab12cd34"), ("sha1", LOWER_SHA1)],
        )?;
        assert_registry_size(&mut imported.connection, 0)?;
        let files = catalog_files::occurrences_for_ids(
            &imported.database,
            &[OccurrenceId::from_database(row.occurrence_id)],
        )?;
        let file = files.first().ok_or("queried unproved ROM missing")?;
        assert!(file.content_id.is_none());
        assert_eq!(file.digests.len(), 2);
        assert!(file.digests.iter().all(|digest| digest.scope == "unknown"));
        let Some(SoftwareFilePayload::Rom(payload)) = &file.software_file else {
            return Err("ROM payload missing".into());
        };
        assert_eq!(payload.name.as_deref(), Some(name));
        assert_eq!(payload.evidence_scope, "unknown");
        assert_eq!(payload.status.as_str(), status);
    }
    Ok(())
}

#[test]
fn disk_chd_header_assertions_preserve_source_case_without_whole_file_identity() -> TestResult {
    let xml = format!(
        r#"<softwarelist name="list"><software name="game"><description>Game</description><year>2000</year><publisher>Test</publisher>
          <part name="drive" interface="drive"><diskarea name="media">
            <disk name="upper" sha1="{UPPER_SHA1}"/>
            <disk name="lower" sha1="{LOWER_SHA1}"/>
          </diskarea></part></software></softwarelist>"#
    );
    let mut imported = import_software(&xml)?;
    let rows = identity_sources(&mut imported)?;
    let digests = source_digests(&mut imported)?;
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].occurrence_id, rows[1].occurrence_id);
    for (order, (row, (name, sha1))) in rows
        .iter()
        .zip([("upper", UPPER_SHA1), ("lower", LOWER_SHA1)])
        .enumerate()
    {
        assert_eq!(row.name, name);
        assert_eq!(row.sha1_text.as_deref(), Some(sha1));
        assert_eq!(row.occurrence_order, i64::try_from(order)?);
        assert_eq!(row.component_order, i64::try_from(order)?);
        assert_eq!(row.source_order, i64::try_from(order)?);
        assert_eq!(row.claim_kind, "software_disk_entry");
        assert_eq!(row.evidence_scope, "chd_header_sha1");
        assert_eq!(row.operation, "disk");
        assert_eq!(row.declares_file, 0);
        assert_eq!(row.declaration_occurrence_id, None);
        assert_eq!(row.content_uuid, None);
        assert_eq!(row.size_text, None);
        assert_eq!(row.offset_text, None);
        assert_source_digests(
            &digests,
            row.occurrence_id,
            "chd_header_sha1",
            &[("sha1", LOWER_SHA1)],
        )?;
    }
    assert_eq!(digests.len(), 2);
    assert_registry_size(&mut imported.connection, 0)?;
    Ok(())
}
