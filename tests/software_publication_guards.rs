use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
}

#[derive(QueryableByName)]
struct BaseSnapshotRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    catalog_key: String,
}

struct Seed {
    _directory: tempfile::TempDir,
    database_path: Utf8PathBuf,
    snapshot_key: String,
}

struct Candidate {
    snapshot_key: String,
    record_id: i64,
    part_id: Option<i64>,
}

#[derive(Clone, Copy)]
struct RomFixture<'a> {
    occurrence_order: i64,
    claim_kind: &'a str,
    crc_text: Option<&'a str>,
    sha1_text: Option<&'a str>,
    declare_file: bool,
}

struct RomSource<'a> {
    name: &'a str,
    status: &'a str,
    scope: &'a str,
    instruction: Option<&'a str>,
}

impl Default for RomSource<'_> {
    fn default() -> Self {
        Self {
            name: "game.bin",
            status: "good",
            scope: "whole_asset",
            instruction: None,
        }
    }
}

fn seed() -> TestResult<Seed> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("seed.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        r#"<softwarelist name="list"><software name="seed"><description>Seed</description><year>2000</year><publisher>Test</publisher></software></softwarelist>"#,
    )?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-publication-guards"),
            source_display_name: "Software publication guards".to_owned(),
            catalog_key: CatalogKey::new("software-publication-guards-seed"),
            catalog_display_name: "Software publication guards seed".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    let snapshot_key = report
        .snapshot_key
        .ok_or("successful import did not return a snapshot")?;
    Ok(Seed {
        _directory: directory,
        database_path,
        snapshot_key: snapshot_key.to_string(),
    })
}

fn connect(seed: &Seed, foreign_keys: bool) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(seed.database_path.as_str())?;
    sql_query("PRAGMA recursive_triggers = OFF").execute(&mut connection)?;
    sql_query(if foreign_keys {
        "PRAGMA foreign_keys = ON"
    } else {
        "PRAGMA foreign_keys = OFF"
    })
    .execute(&mut connection)?;
    Ok(connection)
}

fn candidate(
    connection: &mut SqliteConnection,
    base_snapshot: &str,
    suffix: &str,
    with_item: bool,
    missing_field: Option<i64>,
    with_part: bool,
) -> TestResult<Candidate> {
    let base = sql_query(
        "SELECT snapshot_key, catalog_key FROM snapshot_publications \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(base_snapshot)
    .get_result::<BaseSnapshotRow>(connection)?;
    let catalog_key = format!("software-publication-guards-{suffix}");
    let snapshot_key = format!("software-publication-guards-snapshot-{suffix}");
    sql_query(
        "INSERT INTO catalogs(catalog_key, source_key, display_name) \
         SELECT ?, source_key, display_name FROM catalogs WHERE catalog_key = ?",
    )
    .bind::<Text, _>(&catalog_key)
    .bind::<Text, _>(&base.catalog_key)
    .execute(connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots \
         (snapshot_key, catalog_key, document_key, interpretation_key, acquisition_key, \
          coverage_id, parent_snapshot_key) \
         SELECT ?, ?, document_key, interpretation_key, acquisition_key, \
                coverage_id, NULL FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(&snapshot_key)
    .bind::<Text, _>(&catalog_key)
    .bind::<Text, _>(&base.snapshot_key)
    .execute(connection)?;
    sql_query(
        "INSERT INTO software_documents(snapshot_key, envelope_kind) VALUES (?, 'single_list')",
    )
    .bind::<Text, _>(&snapshot_key)
    .execute(connection)?;
    let group_id = sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key, kind, list_order) \
         VALUES (?, 'software_list', 0) RETURNING set_group_id AS id",
    )
    .bind::<Text, _>(&snapshot_key)
    .get_result::<IdRow>(connection)?
    .id;
    sql_query(
        "INSERT INTO software_lists \
         (namespace_id, source_order, name, description, notes, source_line, source_column) \
         VALUES (?, 0, 'list', NULL, NULL, 1, 1)",
    )
    .bind::<BigInt, _>(group_id)
    .execute(connection)?;
    attribute_positions(
        connection,
        "software_list_attribute_positions",
        "namespace_id",
        group_id,
        &[0],
    )?;
    let record_id = sql_query(
        "INSERT INTO catalog_sets \
         (set_group_id, source_element_kind, list_order, set_name, source_line, source_column) \
         VALUES (?, 'software_item', 0, 'game', 1, 1) RETURNING set_id AS id",
    )
    .bind::<BigInt, _>(group_id)
    .get_result::<IdRow>(connection)?
    .id;
    if with_item {
        sql_query(
            "INSERT INTO software_items \
             (record_id, source_order, supported, supported_specified, \
              description, year, publisher, notes) \
             VALUES (?, 0, 'yes', 0, 'Game', '2000', 'Test', NULL)",
        )
        .bind::<BigInt, _>(record_id)
        .execute(connection)?;
        seed_item_positions(connection, record_id, missing_field)?;
    }
    let part_id = if with_part {
        Some(
            sql_query(
                "INSERT INTO software_parts \
                 (record_id, part_name, part_order, source_order, interface, source_line, source_column) \
                 VALUES (?, 'cart', 0, 3, 'cart', 1, 1) RETURNING part_id AS id",
            )
            .bind::<BigInt, _>(record_id)
            .get_result::<IdRow>(connection)?
            .id,
        )
    } else {
        None
    };
    if let Some(part) = part_id {
        attribute_positions(
            connection,
            "software_part_attribute_positions",
            "part_id",
            part,
            &[0, 1],
        )?;
    }
    Ok(Candidate {
        snapshot_key,
        record_id,
        part_id,
    })
}

fn seed_item_positions(
    connection: &mut SqliteConnection,
    record_id: i64,
    missing_field: Option<i64>,
) -> TestResult {
    attribute_positions(
        connection,
        "software_item_attribute_positions",
        "record_id",
        record_id,
        &[0],
    )?;
    for field_kind in 0_i64..3 {
        if missing_field == Some(field_kind) {
            continue;
        }
        sql_query(
            "INSERT INTO software_item_text_positions \
             (record_id, field_kind, source_order, source_line, source_column) \
             VALUES (?, ?, ?, 1, 1)",
        )
        .bind::<BigInt, _>(record_id)
        .bind::<BigInt, _>(field_kind)
        .bind::<BigInt, _>(field_kind)
        .execute(connection)?;
    }
    Ok(())
}

fn add_area(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    name: &str,
    kind: &str,
    area_order: i64,
    source_order: i64,
) -> TestResult<i64> {
    let area_id = sql_query(
        "INSERT INTO software_areas \
         (part_id, record_id, area_kind, area_order) \
         VALUES (?, ?, ?, ?) RETURNING area_id AS id",
    )
    .bind::<BigInt, _>(candidate.part_id.ok_or("candidate has no part")?)
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<Text, _>(kind)
    .bind::<BigInt, _>(area_order)
    .get_result::<IdRow>(connection)?
    .id;
    if kind == "data" {
        sql_query(
            "INSERT INTO software_data_areas \
             (area_id, area_name, source_order, declared_size_text, width, width_specified, \
              endianness, endianness_specified, source_line, source_column) \
             VALUES (?, ?, ?, '1', 8, 0, 'little', 0, 1, 1)",
        )
        .bind::<BigInt, _>(area_id)
        .bind::<Text, _>(name)
        .bind::<BigInt, _>(source_order)
        .execute(connection)?;
    } else {
        sql_query(
            "INSERT INTO software_disk_areas \
             (area_id, area_name, source_order, source_line, source_column) \
             VALUES (?, ?, ?, 1, 1)",
        )
        .bind::<BigInt, _>(area_id)
        .bind::<Text, _>(name)
        .bind::<BigInt, _>(source_order)
        .execute(connection)?;
    }
    let (table, fields): (&str, &[i64]) = if kind == "data" {
        ("software_data_area_attribute_positions", &[0, 1])
    } else {
        ("software_disk_area_attribute_positions", &[0])
    };
    attribute_positions(connection, table, "area_id", area_id, fields)?;
    Ok(area_id)
}

fn add_rom(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    area_id: i64,
    fixture: RomFixture<'_>,
) -> TestResult<i64> {
    add_rom_with_identity(connection, candidate, area_id, fixture, None)
}

fn add_rom_with_identity(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    area_id: i64,
    fixture: RomFixture<'_>,
    content_uuid: Option<&[u8]>,
) -> TestResult<i64> {
    add_rom_source(
        connection,
        candidate,
        area_id,
        fixture,
        content_uuid,
        &RomSource::default(),
    )
}

fn add_rom_source(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    area_id: i64,
    fixture: RomFixture<'_>,
    content_uuid: Option<&[u8]>,
    source: &RomSource<'_>,
) -> TestResult<i64> {
    let occurrence_id = sql_query(
        "INSERT INTO asset_occurrences(record_id, occurrence_order, claim_kind, content_uuid) \
         VALUES (?, ?, ?, ?) RETURNING occurrence_id AS id",
    )
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<BigInt, _>(fixture.occurrence_order)
    .bind::<Text, _>(fixture.claim_kind)
    .bind::<Nullable<diesel::sql_types::Binary>, _>(content_uuid)
    .get_result::<IdRow>(connection)?
    .id;
    sql_query(
        "INSERT INTO software_rom_entries \
         (occurrence_id, record_id, area_id, component_order, source_order, name, evidence_scope, \
          size_text, offset_text, value, crc_text, sha1_text, dump_status, status_specified, \
          load_instruction, source_line, source_column) \
         VALUES (?, ?, ?, ?, 0, ?, ?, '1', NULL, NULL, ?, ?, ?, ?, ?, 1, 1)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<BigInt, _>(area_id)
    .bind::<BigInt, _>(fixture.occurrence_order)
    .bind::<Text, _>(source.name)
    .bind::<Text, _>(source.scope)
    .bind::<Nullable<Text>, _>(fixture.crc_text)
    .bind::<Nullable<Text>, _>(fixture.sha1_text)
    .bind::<Text, _>(source.status)
    .bind::<BigInt, _>(i64::from(source.status != "good"))
    .bind::<Nullable<Text>, _>(source.instruction)
    .execute(connection)?;
    let fields = [
        Some(0),
        Some(1),
        fixture.crc_text.map(|_| 2),
        fixture.sha1_text.map(|_| 3),
        (source.status != "good").then_some(6),
        source.instruction.map(|_| 7),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    attribute_positions(
        connection,
        "software_rom_attribute_positions",
        "occurrence_id",
        occurrence_id,
        &fields,
    )?;
    if fixture.declare_file {
        sql_query(
            "INSERT INTO software_file_declarations(occurrence_id, record_id, declared_size) \
             VALUES (?, ?, NULL)",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(candidate.record_id)
        .execute(connection)?;
    }
    Ok(occurrence_id)
}

#[test]
fn publication_rejects_unproved_rom_scopes_and_uuids_with_foreign_keys_disabled() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;
    let uuid = [0x55_u8; 16];
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<diesel::sql_types::Binary, _>(uuid.as_slice())
        .execute(&mut connection)?;
    for (case, name, status, instruction) in [
        ("empty", "", "good", None),
        ("nodump", "undumped.bin", "nodump", None),
        ("continue", "operation.bin", "good", Some("continue")),
        ("reload", "operation.bin", "good", Some("reload")),
        (
            "reload-plain",
            "operation.bin",
            "good",
            Some("reload_plain"),
        ),
        ("ignore", "operation.bin", "good", Some("ignore")),
        ("fill", "operation.bin", "good", Some("fill")),
    ] {
        for (variant, scope, linked, allowed) in [
            ("whole", "whole_asset", false, false),
            ("control", "unknown", false, true),
            ("uuid", "unknown", true, false),
        ] {
            // Operation UUIDs are already forbidden by the occurrence table CHECK.
            if instruction.is_some() && linked {
                continue;
            }
            let owner = candidate(
                &mut connection,
                &seed.snapshot_key,
                &format!("{case}-{variant}"),
                true,
                None,
                true,
            )?;
            let area = add_area(&mut connection, &owner, "rom", "data", 0, 0)?;
            let declares_file = instruction.is_none();
            let rom = add_rom_source(
                &mut connection,
                &owner,
                area,
                RomFixture {
                    occurrence_order: 0,
                    claim_kind: if declares_file {
                        "software_rom_entry"
                    } else {
                        "software_rom_operation"
                    },
                    crc_text: None,
                    sha1_text: Some(SHA1),
                    declare_file: declares_file,
                },
                linked.then_some(uuid.as_slice()),
                &RomSource {
                    name,
                    status,
                    scope,
                    instruction,
                },
            )?;
            add_use(
                &mut connection,
                &owner,
                rom,
                declares_file.then_some(rom),
                instruction.unwrap_or("load"),
            )?;
            add_assertion(
                &mut connection,
                rom,
                "sha1",
                &hex::decode(SHA1)?,
                scope,
                "source_declared",
            )?;
            let result = publish(&mut connection, &owner.snapshot_key);
            assert_eq!(
                result.is_ok(),
                allowed,
                "publication {case}/{variant}: {result:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn publication_rejects_control_operations_mislabeled_as_file_declarations() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;
    let uuid = [0x66_u8; 16];
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<diesel::sql_types::Binary, _>(uuid.as_slice())
        .execute(&mut connection)?;
    for instruction in ["continue", "reload", "reload_plain", "ignore", "fill"] {
        for operation in ["load", instruction] {
            let owner = candidate(
                &mut connection,
                &seed.snapshot_key,
                &format!("forged-{instruction}-{operation}"),
                true,
                None,
                true,
            )?;
            let area = add_area(&mut connection, &owner, "rom", "data", 0, 0)?;
            let rom = add_rom_source(
                &mut connection,
                &owner,
                area,
                RomFixture {
                    occurrence_order: 0,
                    claim_kind: "software_rom_entry",
                    crc_text: None,
                    sha1_text: Some(SHA1),
                    declare_file: true,
                },
                Some(uuid.as_slice()),
                &RomSource {
                    instruction: Some(instruction),
                    ..RomSource::default()
                },
            )?;
            add_use(&mut connection, &owner, rom, Some(rom), operation)?;
            add_assertion(
                &mut connection,
                rom,
                "sha1",
                &hex::decode(SHA1)?,
                "whole_asset",
                "source_declared",
            )?;
            assert!(
                publish(&mut connection, &owner.snapshot_key).is_err(),
                "forged declaration {instruction}/{operation} published"
            );
        }
    }
    Ok(())
}

fn add_disk(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    area_id: i64,
    sha1_text: Option<&str>,
) -> TestResult<i64> {
    add_disk_with_identity(connection, candidate, area_id, sha1_text, None)
}

fn add_disk_with_identity(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    area_id: i64,
    sha1_text: Option<&str>,
    content_uuid: Option<&[u8]>,
) -> TestResult<i64> {
    let occurrence_id = sql_query(
        "INSERT INTO asset_occurrences(record_id, occurrence_order, claim_kind, content_uuid) \
         VALUES (?, 0, 'software_disk_entry', ?) RETURNING occurrence_id AS id",
    )
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<Nullable<diesel::sql_types::Binary>, _>(content_uuid)
    .get_result::<IdRow>(connection)?
    .id;
    sql_query(
        "INSERT INTO software_disk_entries \
         (occurrence_id, record_id, area_id, component_order, source_order, name, evidence_scope, \
          sha1_text, dump_status, status_specified, writeable, writeable_specified, source_line, source_column) \
         VALUES (?, ?, ?, 0, 0, 'disk.chd', 'chd_header_sha1', ?, 'good', 0, 0, 0, 1, 1)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<BigInt, _>(area_id)
    .bind::<Nullable<Text>, _>(sha1_text)
    .execute(connection)?;
    let fields = [Some(0), sha1_text.map(|_| 1)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    attribute_positions(
        connection,
        "software_disk_attribute_positions",
        "occurrence_id",
        occurrence_id,
        &fields,
    )?;
    sql_query(
        "INSERT INTO software_file_uses(occurrence_id, record_id, declaration_occurrence_id, operation) \
         VALUES (?, ?, NULL, 'disk')",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(candidate.record_id)
    .execute(connection)?;
    Ok(occurrence_id)
}

fn add_use(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    occurrence_id: i64,
    declaration_id: Option<i64>,
    operation: &str,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO software_file_uses \
         (occurrence_id, record_id, declaration_occurrence_id, operation) VALUES (?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<Nullable<BigInt>, _>(declaration_id)
    .bind::<Text, _>(operation)
    .execute(connection)
}

fn add_assertion(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
    algorithm: &str,
    digest: &[u8],
    scope: &str,
    provenance: &str,
) -> TestResult<()> {
    sql_query("INSERT OR IGNORE INTO digest_values(algorithm, digest) VALUES (?, ?)")
        .bind::<Text, _>(algorithm)
        .bind::<diesel::sql_types::Binary, _>(digest)
        .execute(connection)?;
    let digest_id =
        sql_query("SELECT digest_id AS id FROM digest_values WHERE algorithm = ? AND digest = ?")
            .bind::<Text, _>(algorithm)
            .bind::<diesel::sql_types::Binary, _>(digest)
            .get_result::<IdRow>(connection)?
            .id;
    sql_query(
        "INSERT INTO occurrence_digest_assertions(occurrence_id, digest_id, scope, provenance) \
         VALUES (?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(digest_id)
    .bind::<Text, _>(scope)
    .bind::<Text, _>(provenance)
    .execute(connection)?;
    Ok(())
}

fn attribute_positions(
    connection: &mut SqliteConnection,
    table: &str,
    key: &str,
    owner: i64,
    fields: &[i64],
) -> diesel::QueryResult<()> {
    for (order, field) in fields.iter().enumerate() {
        sql_query(format!("INSERT INTO {table}({key},field_kind,source_order,source_line,source_column) VALUES (?,?,?,1,1)"))
            .bind::<BigInt,_>(owner).bind::<BigInt,_>(*field)
            .bind::<BigInt,_>(i64::try_from(order).map_err(|error|diesel::result::Error::SerializationError(Box::new(error)))?)
            .execute(connection)?;
    }
    Ok(())
}

fn publish(connection: &mut SqliteConnection, snapshot_key: &str) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(catalog_key, document_key, interpretation_key, snapshot_key) \
         SELECT catalog_key, document_key, interpretation_key, snapshot_key \
         FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot_key)
    .execute(connection)
}

#[test]
fn publication_requires_native_item_and_scalar_positions_with_foreign_keys_disabled() -> TestResult
{
    let seed = seed()?;
    for (foreign_keys, suffix) in [(false, "off"), (true, "on")] {
        let mut connection = connect(&seed, foreign_keys)?;
        let missing_item = candidate(
            &mut connection,
            &seed.snapshot_key,
            &format!("missing-item-{suffix}"),
            false,
            None,
            false,
        )?;
        assert!(publish(&mut connection, &missing_item.snapshot_key).is_err());

        let missing_position = candidate(
            &mut connection,
            &seed.snapshot_key,
            &format!("missing-position-{suffix}"),
            true,
            Some(1),
            false,
        )?;
        assert!(publish(&mut connection, &missing_position.snapshot_key).is_err());

        let valid = candidate(
            &mut connection,
            &seed.snapshot_key,
            &format!("item-control-{suffix}"),
            true,
            None,
            false,
        )?;
        publish(&mut connection, &valid.snapshot_key)?;
    }
    Ok(())
}

#[test]
fn publication_requires_rom_use_and_rejects_absent_source_hashes() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;

    let missing_use = candidate(
        &mut connection,
        &seed.snapshot_key,
        "missing-use",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &missing_use, "rom", "data", 0, 0)?;
    let _rom = add_rom(
        &mut connection,
        &missing_use,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: None,
            declare_file: true,
        },
    )?;
    assert!(publish(&mut connection, &missing_use.snapshot_key).is_err());

    let absent_sha = candidate(
        &mut connection,
        &seed.snapshot_key,
        "absent-sha",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &absent_sha, "rom", "data", 0, 0)?;
    let rom = add_rom(
        &mut connection,
        &absent_sha,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: None,
            declare_file: true,
        },
    )?;
    add_use(&mut connection, &absent_sha, rom, Some(rom), "load")?;
    add_assertion(
        &mut connection,
        rom,
        "sha1",
        &hex::decode(SHA1)?,
        "whole_asset",
        "source_declared",
    )?;
    assert!(publish(&mut connection, &absent_sha.snapshot_key).is_err());

    Ok(())
}

#[test]
fn publication_requires_source_digest_scope_and_provenance() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;

    let wrong_scope = candidate(
        &mut connection,
        &seed.snapshot_key,
        "wrong-scope",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &wrong_scope, "rom", "data", 0, 0)?;
    let rom = add_rom(
        &mut connection,
        &wrong_scope,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: Some(SHA1),
            declare_file: true,
        },
    )?;
    add_use(&mut connection, &wrong_scope, rom, Some(rom), "load")?;
    add_assertion(
        &mut connection,
        rom,
        "sha1",
        &hex::decode(SHA1)?,
        "whole_file",
        "source_declared",
    )?;
    assert!(publish(&mut connection, &wrong_scope.snapshot_key).is_err());

    let wrong_provenance = candidate(
        &mut connection,
        &seed.snapshot_key,
        "wrong-provenance",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &wrong_provenance, "rom", "data", 0, 0)?;
    let rom = add_rom(
        &mut connection,
        &wrong_provenance,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: Some(SHA1),
            declare_file: true,
        },
    )?;
    add_use(&mut connection, &wrong_provenance, rom, Some(rom), "load")?;
    add_assertion(
        &mut connection,
        rom,
        "sha1",
        &hex::decode(SHA1)?,
        "whole_asset",
        "unknown",
    )?;
    assert!(publish(&mut connection, &wrong_provenance.snapshot_key).is_err());

    Ok(())
}

#[test]
fn publication_accepts_malformed_hash_text_and_matching_assertions() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    const CRC: &str = "12345678";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;

    let malformed = candidate(
        &mut connection,
        &seed.snapshot_key,
        "malformed",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &malformed, "rom", "data", 0, 0)?;
    let rom = add_rom(
        &mut connection,
        &malformed,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: Some(""),
            sha1_text: Some("bad-sha"),
            declare_file: true,
        },
    )?;
    add_use(&mut connection, &malformed, rom, Some(rom), "load")?;
    publish(&mut connection, &malformed.snapshot_key)?;

    let valid_hashes = candidate(
        &mut connection,
        &seed.snapshot_key,
        "valid-hashes",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &valid_hashes, "rom", "data", 0, 0)?;
    let rom = add_rom(
        &mut connection,
        &valid_hashes,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: Some(CRC),
            sha1_text: Some(SHA1),
            declare_file: true,
        },
    )?;
    add_use(&mut connection, &valid_hashes, rom, Some(rom), "load")?;
    add_assertion(
        &mut connection,
        rom,
        "crc32",
        &hex::decode(CRC)?,
        "whole_asset",
        "source_declared",
    )?;
    add_assertion(
        &mut connection,
        rom,
        "sha1",
        &hex::decode(SHA1)?,
        "whole_asset",
        "source_declared",
    )?;
    publish(&mut connection, &valid_hashes.snapshot_key)?;
    Ok(())
}

#[test]
fn disk_source_sha_and_use_are_owned_by_the_disk_entry() -> TestResult {
    const SHA1: &str = "2222222222222222222222222222222222222222";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;

    let absent_sha = candidate(
        &mut connection,
        &seed.snapshot_key,
        "disk-absent-sha",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &absent_sha, "media", "disk", 0, 0)?;
    let disk = add_disk(&mut connection, &absent_sha, area, None)?;
    add_assertion(
        &mut connection,
        disk,
        "sha1",
        &hex::decode(SHA1)?,
        "chd_header_sha1",
        "source_declared",
    )?;
    assert!(publish(&mut connection, &absent_sha.snapshot_key).is_err());

    let valid = candidate(
        &mut connection,
        &seed.snapshot_key,
        "disk-valid",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &valid, "media", "disk", 0, 0)?;
    let disk = add_disk(&mut connection, &valid, area, Some(SHA1))?;
    add_assertion(
        &mut connection,
        disk,
        "sha1",
        &hex::decode(SHA1)?,
        "chd_header_sha1",
        "source_declared",
    )?;
    publish(&mut connection, &valid.snapshot_key)?;
    Ok(())
}

#[test]
fn replace_and_cross_area_file_uses_are_rejected() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;
    let owner = candidate(
        &mut connection,
        &seed.snapshot_key,
        "replace",
        true,
        None,
        true,
    )?;
    let part_id = owner.part_id.ok_or("candidate has no part")?;
    assert!(sql_query(
        "INSERT OR REPLACE INTO software_parts \
         (part_id, record_id, part_name, part_order, source_order, interface, source_line, source_column) \
         VALUES (?, ?, 'replacement', 0, 1, 'cart', 1, 1)",
    )
    .bind::<BigInt, _>(part_id)
    .bind::<BigInt, _>(owner.record_id)
    .execute(&mut connection)
    .is_err());
    assert!(
        sql_query(
            "INSERT OR REPLACE INTO software_item_text_positions \
         (record_id, field_kind, source_order, source_line, source_column) VALUES (?, 0, 99, 1, 1)",
        )
        .bind::<BigInt, _>(owner.record_id)
        .execute(&mut connection)
        .is_err()
    );

    let cross_area = candidate(
        &mut connection,
        &seed.snapshot_key,
        "cross-area",
        true,
        None,
        true,
    )?;
    let first_area = add_area(&mut connection, &cross_area, "first", "data", 0, 0)?;
    let second_area = add_area(&mut connection, &cross_area, "second", "data", 1, 1)?;
    let declaration = add_rom(
        &mut connection,
        &cross_area,
        first_area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: None,
            declare_file: true,
        },
    )?;
    let operation = add_rom(
        &mut connection,
        &cross_area,
        second_area,
        RomFixture {
            occurrence_order: 1,
            claim_kind: "software_rom_operation",
            crc_text: None,
            sha1_text: None,
            declare_file: false,
        },
    )?;
    assert!(
        add_use(
            &mut connection,
            &cross_area,
            operation,
            Some(declaration),
            "continue"
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn valid_publication_also_succeeds_with_foreign_keys_enabled() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed, true)?;
    let valid = candidate(
        &mut connection,
        &seed.snapshot_key,
        "foreign-keys-on",
        true,
        None,
        false,
    )?;
    publish(&mut connection, &valid.snapshot_key)?;
    Ok(())
}

#[test]
fn publication_requires_declarations_even_without_a_shared_uuid() -> TestResult {
    for foreign_keys in [false, true] {
        let seed = seed()?;
        let mut connection = connect(&seed, foreign_keys)?;
        let owner = candidate(
            &mut connection,
            &seed.snapshot_key,
            "missing-declaration",
            true,
            None,
            true,
        )?;
        let area = add_area(&mut connection, &owner, "rom", "data", 0, 0)?;
        let rom = add_rom(
            &mut connection,
            &owner,
            area,
            RomFixture {
                occurrence_order: 0,
                claim_kind: "software_rom_entry",
                crc_text: None,
                sha1_text: None,
                declare_file: false,
            },
        )?;
        add_use(&mut connection, &owner, rom, None, "load")?;
        assert!(
            publish(&mut connection, &owner.snapshot_key).is_err(),
            "a ROM declaration claim cannot disappear from requirement queries (FK={foreign_keys})"
        );
        let valid = candidate(
            &mut connection,
            &seed.snapshot_key,
            "declaration-control",
            true,
            None,
            true,
        )?;
        let area = add_area(&mut connection, &valid, "rom", "data", 0, 0)?;
        let rom = add_rom(
            &mut connection,
            &valid,
            area,
            RomFixture {
                occurrence_order: 0,
                claim_kind: "software_rom_entry",
                crc_text: None,
                sha1_text: None,
                declare_file: true,
            },
        )?;
        add_use(&mut connection, &valid, rom, Some(rom), "load")?;
        publish(&mut connection, &valid.snapshot_key)?;
    }
    Ok(())
}

#[test]
fn publication_rejects_uuids_without_usable_whole_file_source_evidence() -> TestResult {
    const SHA1: &str = "1111111111111111111111111111111111111111";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;
    let uuid = [0x33_u8; 16];
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<diesel::sql_types::Binary, _>(uuid.as_slice())
        .execute(&mut connection)?;
    for (suffix, crc, sha1) in [
        ("hashless-uuid", None, None),
        ("malformed-crc-uuid", Some("bad-crc"), Some(SHA1)),
        ("empty-crc-uuid", Some(""), Some(SHA1)),
        ("malformed-sha-uuid", None, Some("bad-sha")),
        ("empty-sha-uuid", None, Some("")),
    ] {
        let owner = candidate(
            &mut connection,
            &seed.snapshot_key,
            suffix,
            true,
            None,
            true,
        )?;
        let area = add_area(&mut connection, &owner, "rom", "data", 0, 0)?;
        let rom = add_rom_with_identity(
            &mut connection,
            &owner,
            area,
            RomFixture {
                occurrence_order: 0,
                claim_kind: "software_rom_entry",
                crc_text: crc,
                sha1_text: sha1,
                declare_file: true,
            },
            Some(uuid.as_slice()),
        )?;
        add_use(&mut connection, &owner, rom, Some(rom), "load")?;
        if sha1 == Some(SHA1) {
            add_assertion(
                &mut connection,
                rom,
                "sha1",
                &hex::decode(SHA1)?,
                "whole_asset",
                "source_declared",
            )?;
        }
        assert!(
            publish(&mut connection, &owner.snapshot_key).is_err(),
            "invalid identity eligibility was published: {suffix}"
        );
    }
    let valid = candidate(
        &mut connection,
        &seed.snapshot_key,
        "uuid-control",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &valid, "rom", "data", 0, 0)?;
    let rom = add_rom_with_identity(
        &mut connection,
        &valid,
        area,
        RomFixture {
            occurrence_order: 0,
            claim_kind: "software_rom_entry",
            crc_text: None,
            sha1_text: Some(SHA1),
            declare_file: true,
        },
        Some(uuid.as_slice()),
    )?;
    add_use(&mut connection, &valid, rom, Some(rom), "load")?;
    add_assertion(
        &mut connection,
        rom,
        "sha1",
        &hex::decode(SHA1)?,
        "whole_asset",
        "source_declared",
    )?;
    publish(&mut connection, &valid.snapshot_key)?;
    Ok(())
}

#[test]
fn chd_header_hashes_cannot_publish_a_whole_container_uuid() -> TestResult {
    const SHA1: &str = "2222222222222222222222222222222222222222";
    let seed = seed()?;
    let mut connection = connect(&seed, false)?;
    let uuid = [0x44_u8; 16];
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<diesel::sql_types::Binary, _>(uuid.as_slice())
        .execute(&mut connection)?;
    let owner = candidate(
        &mut connection,
        &seed.snapshot_key,
        "logical-disk-uuid",
        true,
        None,
        true,
    )?;
    let area = add_area(&mut connection, &owner, "media", "disk", 0, 0)?;
    let disk = add_disk_with_identity(
        &mut connection,
        &owner,
        area,
        Some(SHA1),
        Some(uuid.as_slice()),
    )?;
    add_assertion(
        &mut connection,
        disk,
        "sha1",
        &hex::decode(SHA1)?,
        "chd_header_sha1",
        "source_declared",
    )?;
    assert!(publish(&mut connection, &owner.snapshot_key).is_err());
    Ok(())
}
