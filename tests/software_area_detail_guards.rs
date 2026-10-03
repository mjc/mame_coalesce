use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
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
    part_ids: Vec<i64>,
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
        r#"<softwarelist name="seed"><software name="seed"><description>Seed</description><year>2000</year><publisher>Test</publisher></software></softwarelist>"#,
    )?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-area-detail-guards"),
            source_display_name: "Software area detail guards".to_owned(),
            catalog_key: CatalogKey::new("software-area-detail-guards-seed"),
            catalog_display_name: "Software area detail guards seed".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot_key = report
        .snapshot_key
        .ok_or("seed import did not publish a snapshot")?;
    Ok(Seed {
        _directory: directory,
        database_path,
        snapshot_key: snapshot_key.to_string(),
    })
}

fn connect(seed: &Seed) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(seed.database_path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys = OFF; PRAGMA recursive_triggers = OFF;")?;
    Ok(connection)
}

fn candidate(
    connection: &mut SqliteConnection,
    seed: &Seed,
    suffix: &str,
    part_count: usize,
) -> TestResult<Candidate> {
    let base = sql_query(
        "SELECT snapshot_key, catalog_key FROM snapshot_publications WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(&seed.snapshot_key)
    .get_result::<BaseSnapshotRow>(connection)?;
    let catalog_key = format!("software-area-detail-guards-{suffix}");
    let snapshot_key = format!("software-area-detail-guards-snapshot-{suffix}");
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
    let record_id = sql_query(
        "INSERT INTO catalog_sets \
         (set_group_id, source_element_kind, list_order, set_name, source_line, source_column) \
         VALUES (?, 'software_item', 0, 'game', 1, 1) RETURNING set_id AS id",
    )
    .bind::<BigInt, _>(group_id)
    .get_result::<IdRow>(connection)?
    .id;
    sql_query(
        "INSERT INTO software_items \
         (record_id, source_order, supported, supported_specified, \
          description, year, publisher, notes) \
         VALUES (?, 0, 'yes', 0, 'Game', '2000', 'Test', NULL)",
    )
    .bind::<BigInt, _>(record_id)
    .execute(connection)?;
    for (field_kind, source_order) in [(0_i64, 0_i64), (1, 2), (2, 4)] {
        sql_query(
            "INSERT INTO software_item_text_positions \
             (record_id, field_kind, source_order, source_line, source_column) \
             VALUES (?, ?, ?, 1, 1)",
        )
        .bind::<BigInt, _>(record_id)
        .bind::<BigInt, _>(field_kind)
        .bind::<BigInt, _>(source_order)
        .execute(connection)?;
    }
    let mut part_ids = Vec::with_capacity(part_count);
    for part_order in 0..part_count {
        part_ids.push(
            sql_query(
                "INSERT INTO software_parts \
                 (record_id, part_name, part_order, source_order, interface, source_line, source_column) \
                 VALUES (?, ?, ?, ?, 'cart', 1, 1) RETURNING part_id AS id",
            )
            .bind::<BigInt, _>(record_id)
            .bind::<Text, _>(format!("cart{part_order}"))
            .bind::<BigInt, _>(i64::try_from(part_order)?)
            .bind::<BigInt, _>(10 + i64::try_from(part_order)?)
            .get_result::<IdRow>(connection)?
            .id,
        );
    }
    Ok(Candidate {
        snapshot_key,
        record_id,
        part_ids,
    })
}

fn add_area_anchor(
    connection: &mut SqliteConnection,
    candidate: &Candidate,
    part_id: i64,
    kind: &str,
    area_order: i64,
) -> TestResult<i64> {
    Ok(sql_query(
        "INSERT INTO software_areas(part_id, record_id, area_kind, area_order) \
         VALUES (?, ?, ?, ?) RETURNING area_id AS id",
    )
    .bind::<BigInt, _>(part_id)
    .bind::<BigInt, _>(candidate.record_id)
    .bind::<Text, _>(kind)
    .bind::<BigInt, _>(area_order)
    .get_result::<IdRow>(connection)?
    .id)
}

fn add_data_detail(
    connection: &mut SqliteConnection,
    area_id: i64,
    area_name: &str,
    source_order: i64,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO software_data_areas \
         (area_id, area_name, source_order, declared_size_text, width, width_specified, \
          endianness, endianness_specified, source_line, source_column) \
         VALUES (?, ?, ?, '1', 8, 0, 'little', 0, 1, 1)",
    )
    .bind::<BigInt, _>(area_id)
    .bind::<Text, _>(area_name)
    .bind::<BigInt, _>(source_order)
    .execute(connection)
}

fn add_disk_detail(
    connection: &mut SqliteConnection,
    area_id: i64,
    area_name: &str,
    source_order: i64,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO software_disk_areas(area_id, area_name, source_order, source_line, source_column) \
         VALUES (?, ?, ?, 1, 1)",
    )
    .bind::<BigInt, _>(area_id)
    .bind::<Text, _>(area_name)
    .bind::<BigInt, _>(source_order)
    .execute(connection)
}

fn part_at(candidate: &Candidate, index: usize) -> TestResult<i64> {
    candidate
        .part_ids
        .get(index)
        .copied()
        .ok_or_else(|| format!("candidate is missing part {index}").into())
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
fn area_detail_insert_guards_check_owner_kind_and_anchor_without_foreign_keys() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "area-owner", 1)?;
    let part_id = part_at(&candidate, 0)?;

    let mismatched_owner = sql_query(
        "INSERT INTO software_areas(part_id, record_id, area_kind, area_order) \
         VALUES (?, ?, 'data', 0)",
    )
    .bind::<BigInt, _>(part_id)
    .bind::<BigInt, _>(candidate.record_id + 500_000)
    .execute(&mut connection);
    assert!(
        mismatched_owner.is_err(),
        "accepted area with a foreign record owner"
    );

    let data_area = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
    let disk_area = add_area_anchor(&mut connection, &candidate, part_id, "disk", 1)?;
    assert!(
        add_data_detail(&mut connection, 900_001, "missing", 0).is_err(),
        "accepted data details without an area anchor"
    );
    assert!(
        add_disk_detail(&mut connection, 900_002, "missing", 0).is_err(),
        "accepted disk details without an area anchor"
    );
    assert!(
        add_disk_detail(&mut connection, data_area, "wrong-kind", 0).is_err(),
        "accepted disk details for a data area"
    );
    assert!(
        add_data_detail(&mut connection, disk_area, "wrong-kind", 0).is_err(),
        "accepted data details for a disk area"
    );
    add_data_detail(&mut connection, data_area, "rom", 0)?;
    add_disk_detail(&mut connection, disk_area, "media", 1)?;
    Ok(())
}

#[test]
fn publication_requires_exactly_one_matching_area_detail_and_recovers_after_restoration()
-> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "missing-area-detail", 1)?;
    let part_id = part_at(&candidate, 0)?;
    let data_area = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
    let disk_area = add_area_anchor(&mut connection, &candidate, part_id, "disk", 1)?;

    assert!(
        publish(&mut connection, &candidate.snapshot_key).is_err(),
        "published data and disk areas without details"
    );
    add_data_detail(&mut connection, data_area, "rom", 20)?;
    assert!(
        publish(&mut connection, &candidate.snapshot_key).is_err(),
        "published with a data detail but no disk detail"
    );
    add_disk_detail(&mut connection, disk_area, "media", 21)?;
    publish(&mut connection, &candidate.snapshot_key)?;
    Ok(())
}

#[test]
fn published_area_details_reject_update_delete_and_replacement() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "immutable-area-details", 1)?;
    let part_id = part_at(&candidate, 0)?;
    let data_area = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
    add_data_detail(&mut connection, data_area, "rom", 20)?;
    let disk_area = add_area_anchor(&mut connection, &candidate, part_id, "disk", 1)?;
    add_disk_detail(&mut connection, disk_area, "media", 21)?;
    publish(&mut connection, &candidate.snapshot_key)?;

    assert!(
        sql_query("UPDATE software_data_areas SET area_name = 'changed' WHERE area_id = ?")
            .bind::<BigInt, _>(data_area)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query("DELETE FROM software_data_areas WHERE area_id = ?")
            .bind::<BigInt, _>(data_area)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query(
            "INSERT OR REPLACE INTO software_data_areas \
         (area_id, area_name, source_order, declared_size_text, width, width_specified, \
          endianness, endianness_specified, source_line, source_column) \
         VALUES (?, 'replacement', 20, '1', 8, 0, 'little', 0, 1, 1)",
        )
        .bind::<BigInt, _>(data_area)
        .execute(&mut connection)
        .is_err()
    );

    assert!(
        sql_query("UPDATE software_disk_areas SET area_name = 'changed' WHERE area_id = ?")
            .bind::<BigInt, _>(disk_area)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query("DELETE FROM software_disk_areas WHERE area_id = ?")
            .bind::<BigInt, _>(disk_area)
            .execute(&mut connection)
            .is_err()
    );
    assert!(
        sql_query(
            "INSERT OR REPLACE INTO software_disk_areas \
         (area_id, area_name, source_order, source_line, source_column) \
         VALUES (?, 'replacement', 21, 1, 1)",
        )
        .bind::<BigInt, _>(disk_area)
        .execute(&mut connection)
        .is_err()
    );
    Ok(())
}

#[test]
fn matching_area_detail_cannot_be_added_late_to_a_published_anchor() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "late-area-detail", 1)?;
    let part_id = part_at(&candidate, 0)?;
    let area_id = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
    add_data_detail(&mut connection, area_id, "rom", 20)?;
    publish(&mut connection, &candidate.snapshot_key)?;

    // Remove only the published detail row after disabling its exact canonical delete guard.
    connection.batch_execute("DROP TRIGGER software_data_areas_immutable_delete")?;
    sql_query("DELETE FROM software_data_areas WHERE area_id = ?")
        .bind::<BigInt, _>(area_id)
        .execute(&mut connection)?;
    assert!(add_data_detail(&mut connection, area_id, "rom", 20).is_err());
    Ok(())
}

#[test]
fn area_source_order_is_unique_across_kinds_per_part_but_scoped_between_parts() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "area-source-order", 2)?;
    let first_part = part_at(&candidate, 0)?;
    let second_part = part_at(&candidate, 1)?;
    let data_area = add_area_anchor(&mut connection, &candidate, first_part, "data", 0)?;
    add_data_detail(&mut connection, data_area, "rom", 30)?;

    let collision_area = add_area_anchor(&mut connection, &candidate, first_part, "disk", 1)?;
    assert!(
        add_disk_detail(&mut connection, collision_area, "media", 30).is_err(),
        "accepted matching source order across data and disk areas in one part"
    );

    let other_part_area = add_area_anchor(&mut connection, &candidate, second_part, "disk", 0)?;
    add_disk_detail(&mut connection, other_part_area, "media", 30)?;
    Ok(())
}

#[derive(QueryableByName)]
struct DataDefaults {
    #[diesel(sql_type = BigInt)]
    width: i64,
    #[diesel(sql_type = BigInt)]
    width_specified: i64,
    #[diesel(sql_type = Text)]
    endianness: String,
    #[diesel(sql_type = BigInt)]
    endianness_specified: i64,
}

#[test]
fn data_area_defaults_and_invalid_required_fields_are_enforced() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "area-data-fields", 1)?;
    let part_id = part_at(&candidate, 0)?;
    let default_area = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
    add_data_detail(&mut connection, default_area, "defaulted", 40)?;
    let defaults = sql_query(
        "SELECT width, width_specified, endianness, endianness_specified \
         FROM software_data_areas WHERE area_id = ?",
    )
    .bind::<BigInt, _>(default_area)
    .get_result::<DataDefaults>(&mut connection)?;
    assert_eq!(defaults.width, 8);
    assert_eq!(defaults.width_specified, 0);
    assert_eq!(defaults.endianness, "little");
    assert_eq!(defaults.endianness_specified, 0);

    for (index, (size, width, width_specified, endianness, endian_specified)) in [
        (Some("1"), 7, 1, "little", 1),
        (Some("1"), 8, 2, "little", 1),
        (Some("1"), 8, 1, "middle", 1),
        (Some("1"), 8, 1, "little", 2),
        (Some("1"), 16, 0, "little", 0),
        (Some("1"), 8, 0, "big", 0),
        (None, 8, 0, "little", 0),
    ]
    .into_iter()
    .enumerate()
    {
        let area_id = add_area_anchor(
            &mut connection,
            &candidate,
            part_id,
            "data",
            i64::try_from(index + 1)?,
        )?;
        let result = sql_query(
            "INSERT INTO software_data_areas \
             (area_id, area_name, source_order, declared_size_text, width, width_specified, \
              endianness, endianness_specified, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, 1)",
        )
        .bind::<BigInt, _>(area_id)
        .bind::<Text, _>(format!("invalid-{index}"))
        .bind::<BigInt, _>(41 + i64::try_from(index)?)
        .bind::<Nullable<Text>, _>(size)
        .bind::<BigInt, _>(width)
        .bind::<BigInt, _>(width_specified)
        .bind::<Text, _>(endianness)
        .bind::<BigInt, _>(endian_specified)
        .execute(&mut connection);
        assert!(result.is_err(), "accepted invalid area detail case {index}");
    }
    Ok(())
}

#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn area_source_order_guard_plan_seeks_part_and_area_indexes() -> TestResult {
    let seed = seed()?;
    let mut connection = connect(&seed)?;
    let candidate = candidate(&mut connection, &seed, "area-query-plan", 64)?;
    let mut representative_area_ids = Vec::new();
    for index in 0..64 {
        let part_id = part_at(&candidate, index)?;
        let data_area = add_area_anchor(&mut connection, &candidate, part_id, "data", 0)?;
        add_data_detail(&mut connection, data_area, "rom", 0)?;
        let disk_area = add_area_anchor(&mut connection, &candidate, part_id, "disk", 1)?;
        add_disk_detail(&mut connection, disk_area, "media", 1)?;
        if index == 0 {
            representative_area_ids.extend([data_area, disk_area]);
        }
    }
    connection.batch_execute("ANALYZE")?;
    let data_area_id = representative_area_ids
        .first()
        .copied()
        .ok_or("missing representative data area")?;
    let disk_area_id = representative_area_ids
        .get(1)
        .copied()
        .ok_or("missing representative disk area")?;

    for (table, trigger, area_id, source_order) in [
        (
            "software_data_areas",
            "software_data_areas_native_owner_insert",
            data_area_id,
            0_i64,
        ),
        (
            "software_disk_areas",
            "software_disk_areas_native_owner_insert",
            disk_area_id,
            1_i64,
        ),
    ] {
        let trigger_sql = sql_query(
            "SELECT sql AS detail FROM sqlite_master WHERE type = 'trigger' AND name = ?",
        )
        .bind::<Text, _>(trigger)
        .get_result::<PlanRow>(&mut connection)?
        .detail;
        let normalized_trigger_sql = trigger_sql.split_whitespace().collect::<Vec<_>>().join(" ");
        let predicate = normalized_trigger_sql
            .split_once("WHEN ")
            .and_then(|(_, guard)| guard.split_once("BEGIN "))
            .map(|(guard, _)| {
                guard
                    .replace("NEW.area_id", "?1")
                    .replace("NEW.source_order", "?2")
            })
            .ok_or_else(|| format!("could not extract {trigger} WHEN predicate"))?;
        let plan = sql_query(format!(
            "EXPLAIN QUERY PLAN SELECT ({predicate}) AS guard_result"
        ))
        .bind::<BigInt, _>(area_id)
        .bind::<BigInt, _>(source_order)
        .load::<PlanRow>(&mut connection)?;
        let plan_text = plan
            .iter()
            .map(|row| row.detail.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            plan.iter().any(|row| {
                row.detail.contains("SEARCH area USING")
                    && row.detail.contains("software_areas_part_order")
            }),
            "{trigger} on {table} lost its bounded part-index seek: {plan_text}"
        );
        assert!(
            plan.iter().any(|row| {
                row.detail
                    .contains("SEARCH detail USING INTEGER PRIMARY KEY")
            }),
            "{trigger} on {table} lost its subtype area-key seek: {plan_text}"
        );
        assert!(
            plan.iter().any(|row| {
                row.detail
                    .contains("SEARCH software_areas USING INTEGER PRIMARY KEY")
            }),
            "{trigger} on {table} lost its actual area-owner key seek: {plan_text}"
        );
        assert!(
            plan.iter().all(|row| {
                !row.detail.starts_with("SCAN ") || row.detail == "SCAN CONSTANT ROW"
            }),
            "{trigger} on {table} uses an unbounded scan: {plan_text}"
        );
    }
    Ok(())
}
