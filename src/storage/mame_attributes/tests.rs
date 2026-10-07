#![allow(clippy::expect_used)]

use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{
    EDITION_VALIDATION_QUERY, Family, PositionBatch, SnapshotKey, Text, insert, validate_edition,
};
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl,
    connection::{InstrumentationEvent, SimpleConnection},
    sql_query,
    sql_types::BigInt,
    sqlite::SqliteConnection,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, QueryableByName)]
struct PositionData {
    #[diesel(sql_type = BigInt)]
    owner_a: i64,
    #[diesel(sql_type = BigInt)]
    owner_b: i64,
    #[diesel(sql_type = BigInt)]
    owner_c: i64,
    #[diesel(sql_type = BigInt)]
    owner_d: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

const ALL_FAMILIES: [Family; 33] = [
    Family::Document,
    Family::Machine,
    Family::Bios,
    Family::Rom,
    Family::Disk,
    Family::DeviceReference,
    Family::Sample,
    Family::Chip,
    Family::Display,
    Family::Sound,
    Family::Input,
    Family::Control,
    Family::Switch,
    Family::SwitchLocation,
    Family::SwitchValue,
    Family::SwitchCondition,
    Family::SwitchValueCondition,
    Family::AdjusterCondition,
    Family::Port,
    Family::Analog,
    Family::Adjuster,
    Family::Driver,
    Family::Feature,
    Family::Device,
    Family::Instance,
    Family::Extension,
    Family::Slot,
    Family::SlotOption,
    Family::SoftwareList,
    Family::RamOption,
    Family::MachineCompatibility,
    Family::RomCompatibility,
    Family::DiskCompatibility,
];

fn create_position_table(conn: &mut SqliteConnection, family: Family) -> crate::Result<()> {
    let columns = family
        .keys()
        .iter()
        .map(|key| format!("{key} INTEGER NOT NULL"))
        .chain(
            [
                "field_kind INTEGER NOT NULL",
                "source_order INTEGER NOT NULL",
                "source_line INTEGER NOT NULL",
                "source_column INTEGER NOT NULL",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .collect::<Vec<_>>()
        .join(", ");
    conn.batch_execute(&format!("CREATE TABLE {} ({columns})", family.table()))?;
    Ok(())
}

fn read_positions(conn: &mut SqliteConnection, family: Family) -> crate::Result<Vec<PositionData>> {
    let columns = family
        .keys()
        .iter()
        .map(ToString::to_string)
        .chain(std::iter::repeat_n("0".to_owned(), 4 - family.keys().len()))
        .zip(["owner_a", "owner_b", "owner_c", "owner_d"])
        .map(|(key, alias)| format!("{key} AS {alias}"))
        .chain(
            ["field_kind", "source_order", "source_line", "source_column"]
                .into_iter()
                .map(str::to_owned),
        )
        .collect::<Vec<_>>()
        .join(", ");
    Ok(sql_query(format!(
        "SELECT {columns} FROM {} ORDER BY source_order",
        family.table()
    ))
    .load(conn)?)
}

fn positions(count: usize, offset: usize) -> Vec<crate::xml_reader::AttributePosition<u8>> {
    use crate::xml_reader::{AttributeLocation, AttributePosition};

    (0..count)
        .map(|index| AttributePosition {
            field: u8::try_from((index + offset) % 250).expect("field code fits in u8"),
            source_order: index,
            location: AttributeLocation {
                line: 100 + i64::try_from(index).expect("line fits in i64"),
                column: 200 + i64::try_from(index + offset).expect("column fits in i64"),
            },
        })
        .collect()
}

#[test]
fn mame_attribute_positions_insert_for_every_owner_key_arity() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    let families = [
        (Family::Document, [101, 0, 0, 0], &[101][..]),
        (Family::Bios, [102, 3, 0, 0], &[102, 3][..]),
        (Family::Control, [103, 4, 5, 0], &[103, 4, 5][..]),
        (
            Family::SwitchValueCondition,
            [104, 6, 7, 8],
            &[104, 6, 7, 8][..],
        ),
    ];

    for (family, owner_key, expected_key) in families {
        create_position_table(&mut conn, family)?;
        let row = positions(
            1,
            usize::try_from(family.code()).expect("family code is positive"),
        );
        insert(&mut conn, family, expected_key, &row, i64::from)?;
        let stored = read_positions(&mut conn, family)?;
        assert_eq!(stored.len(), 1);
        assert_eq!(
            [
                stored[0].owner_a,
                stored[0].owner_b,
                stored[0].owner_c,
                stored[0].owner_d
            ],
            owner_key
        );
        assert_eq!(stored[0].field_kind, i64::from(row[0].field));
        assert_eq!(stored[0].source_order, 0);
        assert_eq!(stored[0].source_line, row[0].location.line);
        assert_eq!(stored[0].source_column, row[0].location.column);
    }

    let before_empty = read_positions(&mut conn, Family::Document)?.len();
    insert::<u8>(&mut conn, Family::Document, &[101], &[], i64::from)?;
    assert_eq!(
        read_positions(&mut conn, Family::Document)?.len(),
        before_empty
    );
    assert!(insert::<u8>(&mut conn, Family::Document, &[], &[], i64::from).is_err());
    Ok(())
}

#[test]
fn mame_attribute_positions_batch_with_bounded_cacheable_sql() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    create_position_table(&mut conn, Family::SwitchValueCondition)?;

    let executions = Arc::new(AtomicUsize::new(0));
    let cache_misses = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_executions = Arc::clone(&executions);
    let observed_cache_misses = Arc::clone(&cache_misses);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| match event {
        InstrumentationEvent::StartQuery { .. } => {
            observed_executions.fetch_add(1, Ordering::Relaxed);
        }
        InstrumentationEvent::CacheQuery { sql, .. } => observed_cache_misses
            .lock()
            .expect("cache event lock")
            .push(sql.to_owned()),
        _ => {}
    });

    let full_batches = positions(130, 0);
    insert(
        &mut conn,
        Family::SwitchValueCondition,
        &[201, 2, 3, 4],
        &full_batches,
        i64::from,
    )?;
    let changed_binds = positions(64, 37);
    insert(
        &mut conn,
        Family::SwitchValueCondition,
        &[202, 5, 6, 7],
        &changed_binds,
        i64::from,
    )?;

    assert_eq!(executions.load(Ordering::Relaxed), 4);
    let cached_sql = cache_misses.lock().expect("cache event lock").clone();
    assert_eq!(
        cached_sql.len(),
        2,
        "one cached SQL per row count: {cached_sql:?}"
    );
    assert_ne!(cached_sql[0], cached_sql[1]);

    let stored = read_positions(&mut conn, Family::SwitchValueCondition)?;
    assert_eq!(stored.len(), 194);
    for row in &stored {
        let (expected_owner, expected_positions) =
            match [row.owner_a, row.owner_b, row.owner_c, row.owner_d] {
                [201, 2, 3, 4] => ([201, 2, 3, 4], &full_batches),
                [202, 5, 6, 7] => ([202, 5, 6, 7], &changed_binds),
                key => {
                    return Err(crate::Error::XmlValidation(format!(
                        "unexpected owner key: {key:?}"
                    )));
                }
            };
        let expected = &expected_positions
            [usize::try_from(row.source_order).expect("source order fits in usize")];
        assert_eq!(
            [row.owner_a, row.owner_b, row.owner_c, row.owner_d],
            expected_owner
        );
        assert_eq!(row.field_kind, i64::from(expected.field));
        assert_eq!(
            row.source_order,
            i64::try_from(expected.source_order).expect("order fits")
        );
        assert_eq!(row.source_line, expected.location.line);
        assert_eq!(row.source_column, expected.location.column);
    }
    Ok(())
}

#[test]
fn mame_attribute_position_cache_isolated_for_all_families() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    let families = ALL_FAMILIES;
    for family in families {
        create_position_table(&mut conn, family)?;
    }

    let cache_misses = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_cache_misses = Arc::clone(&cache_misses);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if let InstrumentationEvent::CacheQuery { sql, .. } = event {
            observed_cache_misses
                .lock()
                .expect("cache event lock")
                .push(sql.to_owned());
        }
    });

    for family in families {
        let owner_key = [501 + family.code(), 11, 12, 13];
        let key = &owner_key[..family.keys().len()];
        let rows = positions(2, usize::try_from(family.code()).expect("family code fits"));
        insert(&mut conn, family, key, &rows, i64::from)?;
    }

    let first_pass = cache_misses.lock().expect("cache event lock").clone();
    assert_eq!(first_pass.len(), families.len());
    assert_eq!(
        first_pass.iter().collect::<BTreeSet<_>>().len(),
        families.len()
    );

    for family in families {
        let owner_key = [1501 + family.code(), 21, 22, 23];
        let key = &owner_key[..family.keys().len()];
        let changed_rows = positions(
            2,
            usize::try_from(family.code() + 71).expect("field offset fits"),
        );
        insert(&mut conn, family, key, &changed_rows, i64::from)?;
    }

    assert_eq!(
        cache_misses.lock().expect("cache event lock").len(),
        families.len(),
        "changed binds should reuse all 33 family statements"
    );
    Ok(())
}

#[test]
fn variable_position_tails_admit_only_canonical_cached_shapes() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    create_position_table(&mut conn, Family::Rom)?;
    let admissions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&admissions);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if let InstrumentationEvent::CacheQuery { .. } = event {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    });
    for pass in 0..2 {
        for count in 1..=64 {
            insert(
                &mut conn,
                Family::Rom,
                &[pass * 64 + i64::try_from(count).expect("bounded count")],
                &positions(count, 0),
                i64::from,
            )?;
        }
    }
    assert_eq!(admissions.load(Ordering::Relaxed), 7);
    assert_eq!(read_positions(&mut conn, Family::Rom)?.len(), 4160);
    Ok(())
}

#[test]
#[allow(clippy::too_many_lines)]
fn mame_attribute_positions_batch_across_owners_and_field_types() -> crate::Result<()> {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    for family in ALL_FAMILIES {
        create_position_table(&mut conn, family)?;
    }
    let executions = Arc::new(AtomicUsize::new(0));
    let cache_misses = Arc::new(Mutex::new(Vec::<String>::new()));
    let observed_executions = Arc::clone(&executions);
    let observed_cache_misses = Arc::clone(&cache_misses);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| match event {
        InstrumentationEvent::StartQuery { .. } => {
            observed_executions.fetch_add(1, Ordering::Relaxed);
        }
        InstrumentationEvent::CacheQuery { sql, .. } => observed_cache_misses
            .lock()
            .expect("cache event lock")
            .push(sql.to_owned()),
        _ => {}
    });

    let mut batch = PositionBatch::default();
    let mut expected = std::collections::BTreeMap::<Family, Vec<PositionData>>::new();
    for pass in 0..2 {
        for owner in 0..97 {
            for family in ALL_FAMILIES {
                let id = pass * 1000 + owner + 1;
                let mut key = [id, id * 2, id * 3, id * 4];
                key[family.keys().len()..].fill(0);
                let mut rows = positions(2, usize::try_from(id).expect("positive owner"));
                for row in &mut rows {
                    row.source_order += usize::try_from(owner * 3).expect("positive order");
                    row.location.line += id;
                    row.location.column += family.code();
                }
                let field_offset = if owner % 2 == 0 { 0 } else { 1000 };
                if owner % 2 == 0 {
                    batch.queue(
                        &mut conn,
                        family,
                        &key[..family.keys().len()],
                        &rows,
                        i64::from,
                    )?;
                } else {
                    let typed_rows = rows
                        .iter()
                        .map(|row| crate::xml_reader::AttributePosition {
                            field: u16::from(row.field) + 1000,
                            source_order: row.source_order,
                            location: row.location,
                        })
                        .collect::<Vec<_>>();
                    batch.queue(
                        &mut conn,
                        family,
                        &key[..family.keys().len()],
                        &typed_rows,
                        i64::from,
                    )?;
                }
                expected
                    .entry(family)
                    .or_default()
                    .extend(rows.iter().map(|row| PositionData {
                        owner_a: key[0],
                        owner_b: key[1],
                        owner_c: key[2],
                        owner_d: key[3],
                        field_kind: i64::from(row.field) + field_offset,
                        source_order: i64::try_from(row.source_order).expect("order fits"),
                        source_line: row.location.line,
                        source_column: row.location.column,
                    }));
            }
        }
        batch.flush(&mut conn)?;
        assert!(batch.0.values().all(Vec::is_empty));
        assert!(
            batch.0.values().map(Vec::capacity).sum::<usize>()
                * std::mem::size_of::<super::PositionBindValues>()
                <= 132 * 1024,
            "row storage stays bounded across all owners and families"
        );
        assert_eq!(
            executions.load(Ordering::Relaxed),
            usize::try_from(pass + 1).expect("positive pass") * ALL_FAMILIES.len() * 4,
            "194 rows per family must take ceil(194/64) executions, across 97 owners"
        );
        assert_eq!(
            cache_misses.lock().expect("cache event lock").len(),
            ALL_FAMILIES.len() * 2,
            "cache isolates each family and row count, then reuses changed binds"
        );
    }
    for (family, mut expected_rows) in expected {
        let mut stored = read_positions(&mut conn, family)?;
        stored.sort();
        expected_rows.sort();
        assert_eq!(stored, expected_rows, "all source facts for {family:?}");
        let invalid_types = family
            .keys()
            .iter()
            .copied()
            .chain(["field_kind", "source_order", "source_line", "source_column"])
            .map(|column| format!("typeof({column}) <> 'integer'"))
            .collect::<Vec<_>>()
            .join(" OR ");
        let invalid = sql_query(format!(
            "SELECT count(*) AS count FROM {} WHERE {invalid_types}",
            family.table()
        ))
        .get_result::<Count>(&mut conn)?;
        assert_eq!(invalid.count, 0, "integer binds for {family:?}");
    }
    Ok(())
}

#[test]
fn mame_attribute_position_batch_validates_input_and_splits_large_owners() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    create_position_table(&mut conn, Family::Document)?;
    let executions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&executions);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            observed.fetch_add(1, Ordering::Relaxed);
        }
    });
    let mut batch = PositionBatch::default();
    batch.flush(&mut conn)?;
    batch.queue::<u8>(&mut conn, Family::Document, &[1], &[], i64::from)?;
    assert!(
        batch
            .queue::<u8>(&mut conn, Family::Document, &[], &[], i64::from)
            .is_err()
    );
    assert!(
        batch
            .queue(
                &mut conn,
                Family::Document,
                &[1, 2],
                &positions(1, 0),
                i64::from
            )
            .is_err()
    );
    let mut invalid_order = positions(1, 0);
    invalid_order[0].source_order = usize::MAX;
    if i64::try_from(usize::MAX).is_err() {
        assert!(
            batch
                .queue(&mut conn, Family::Document, &[1], &invalid_order, i64::from)
                .is_err()
        );
    }
    batch.flush(&mut conn)?;
    assert_eq!(executions.load(Ordering::Relaxed), 0);

    let rows = positions(130, 5);
    batch.queue(&mut conn, Family::Document, &[1], &rows, i64::from)?;
    assert_eq!(executions.load(Ordering::Relaxed), 2);
    batch.flush(&mut conn)?;
    batch.flush(&mut conn)?;
    assert_eq!(executions.load(Ordering::Relaxed), 3);
    let stored = read_positions(&mut conn, Family::Document)?;
    assert_eq!(stored.len(), rows.len());
    for (stored, expected) in stored.iter().zip(&rows) {
        assert_eq!(stored.owner_a, 1);
        assert_eq!(stored.field_kind, i64::from(expected.field));
        assert_eq!(
            stored.source_order,
            i64::try_from(expected.source_order).expect("order fits")
        );
        assert_eq!(stored.source_line, expected.location.line);
        assert_eq!(stored.source_column, expected.location.column);
    }
    Ok(())
}

#[test]
fn mame_attribute_position_batch_rolls_back_full_batches_on_tail_failure() -> crate::Result<()> {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
    create_position_table(&mut conn, Family::Document)?;
    conn.batch_execute(&format!(
        "CREATE TRIGGER reject_position BEFORE INSERT ON {} WHEN NEW.field_kind=255 \
         BEGIN SELECT RAISE(ABORT, 'rejected position'); END",
        Family::Document.table()
    ))?;
    let result = conn.transaction::<(), crate::Error, _>(|conn| {
        let mut batch = PositionBatch::default();
        for owner in 1..=33 {
            batch.queue(
                conn,
                Family::Document,
                &[owner],
                &positions(2, usize::try_from(owner).expect("positive owner")),
                i64::from,
            )?;
        }
        assert_eq!(read_positions(conn, Family::Document)?.len(), 64);
        let mut rejected = positions(1, 0);
        rejected[0].field = 255;
        batch.queue(conn, Family::Document, &[34], &rejected, i64::from)?;
        let result = batch.flush(conn);
        assert!(result.is_err(), "tail must hit the database constraint");
        assert_eq!(
            batch.0[&Family::Document].len(),
            3,
            "failed rows remain buffered"
        );
        result
    });
    assert!(result.is_err());
    assert!(read_positions(&mut conn, Family::Document)?.is_empty());
    Ok(())
}

fn import_edition(
    database: &Database,
    directory: &tempfile::TempDir,
    catalog: &str,
    repetitions: usize,
) -> crate::Result<SnapshotKey> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{catalog}.xml")))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
    let xml = include_str!("../../../fixtures/specifications/mame-machine-fields.xml");
    let (header, rest) = xml
        .split_once("\t<machine")
        .expect("synthetic machine header");
    let (machine, _) = rest
        .split_once("</machine>")
        .expect("synthetic complete machine");
    std::fs::write(
        &path,
        format!(
            "{header}{}\n</mame>",
            format!("<machine{machine}</machine>").repeat(repetitions)
        ),
    )?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("machine-attribute-query-plan"),
            source_display_name: "Machine query plan".into(),
            catalog_key: CatalogKey::new(catalog),
            catalog_display_name: catalog.into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.expect("successful publication"))
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn edition_validation_searches_requested_owners_in_a_populated_database() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let requested = import_edition(&database, &directory, "requested", 2)?;
    let unrelated = import_edition(&database, &directory, "unrelated", 48)?;
    let mut conn = database.pool().get()?;
    validate_edition(&mut conn, &requested)?;
    validate_edition(&mut conn, &unrelated)?;
    conn.batch_execute("ANALYZE")?;
    let plan = sql_query(format!("EXPLAIN QUERY PLAN {EDITION_VALIDATION_QUERY}"))
        .bind::<Text, _>(requested.as_str())
        .load::<ExplainRow>(&mut conn)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    let scans = plan
        .iter()
        .filter(|detail| {
            detail.strip_prefix("SCAN ").is_some_and(|scan| {
                scan.split_whitespace().next().is_some_and(|alias| {
                    matches!(
                        alias,
                        "owner"
                            | "position"
                            | "native"
                            | "compat"
                            | "groups"
                            | "sets"
                            | "set_owner"
                            | "occurrence"
                            | "document"
                            | "catalog_sets"
                            | "asset_occurrences"
                            | "catalog_set_groups"
                    ) || (alias.starts_with("mame_")
                        && !matches!(
                            alias,
                            "mame_expected_attribute_positions" | "mame_actual_attribute_positions"
                        ))
                        || alias.starts_with("machine_switch")
                })
            })
        })
        .collect::<Vec<_>>();
    assert!(
        scans.is_empty(),
        "unbounded native scans: {scans:?}; plan: {plan:?}"
    );
    conn.batch_execute("DROP TRIGGER mame_machines_attribute_positions_attribute_position_delete")?;
    sql_query("DELETE FROM mame_machines_attribute_positions WHERE field_kind=0 AND set_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=?)")
        .bind::<Text,_>(unrelated.as_str()).execute(&mut conn)?;
    validate_edition(&mut conn, &requested)?;
    assert!(validate_edition(&mut conn, &unrelated).is_err());
    Ok(())
}
