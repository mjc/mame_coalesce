#![allow(clippy::expect_used)]

use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    database::Database, domain::CatalogContentId, storage::catalog_identity::OccurrenceId,
};
use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::{InstrumentationEvent, SimpleConnection},
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use super::super::*;
use super::load_facts;

#[derive(Debug, PartialEq, Eq, diesel::QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(Debug, PartialEq, Eq, diesel::QueryableByName)]
struct AssertionFactRow {
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

#[derive(Debug, diesel::QueryableByName)]
struct DigestPlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[derive(Default)]
struct DigestQueryTrace {
    started: Vec<String>,
    cache_queries: Vec<String>,
}

#[test]
#[ignore = "manual profiling-build measurement against a retained import database"]
fn profile_128_most_repeated_shared_files() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("use --profile profiling".into());
    }
    let path = std::env::var("MAME_COALESCE_FACT_BENCH_DB")?;
    let mut conn = SqliteConnection::establish(&path)?;
    conn.batch_execute("PRAGMA query_only=ON; PRAGMA cache_size=-65536; PRAGMA temp_store=FILE;")?;
    let ids = sql_query(
        "SELECT content_uuid FROM canonical_occurrence_content WHERE content_uuid IS NOT NULL \
         GROUP BY content_uuid ORDER BY COUNT(*) DESC,content_uuid LIMIT 128",
    )
    .load::<ContentUuidRow>(&mut conn)?
    .into_iter()
    .map(|row| content_id(row.content_uuid))
    .collect::<crate::Result<BTreeSet<_>>>()?;
    assert_eq!(ids.len(), 128, "the batch must not be reduced");
    let mut elapsed = Vec::new();
    for round in 0..11 {
        let start = std::time::Instant::now();
        let facts = load_facts(&mut conn, &ids)?;
        let duration = start.elapsed();
        assert_eq!(facts.len(), 128);
        let sizes: usize = facts.values().map(|facts| facts.sizes.len()).sum();
        let hashes: usize = facts
            .values()
            .flat_map(|facts| facts.digests.values())
            .map(BTreeSet::len)
            .sum();
        eprintln!(
            "shared_file_facts round={round} elapsed_us={} files={} sizes={sizes} hashes={hashes}",
            duration.as_micros(),
            facts.len()
        );
        if round > 0 {
            elapsed.push(duration);
        }
        std::hint::black_box(facts);
    }
    elapsed.sort_unstable();
    eprintln!(
        "shared_file_facts warm_median_us={}",
        elapsed[elapsed.len() / 2].as_micros()
    );
    Ok(())
}

#[test]
fn bulk_overlay_matches_sequential_digest_and_size_decisions() -> crate::Result<()> {
    let sha1_a = [0x11; 20];
    let sha1_b = [0x22; 20];
    let sha256_a = [0x33; 32];
    let sha1_c = [0x44; 20];
    let crc_a = [0x55; 4];
    let crc_b = [0x66; 4];
    let inputs = [
        input(Some(8), Some(&sha1_a), None, true),
        input(Some(8), None, Some(&sha256_a), true),
        input(Some(8), Some(&sha1_b), None, true),
        input(Some(3), Some(&sha1_b), None, true),
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "whole_file",
                Some(&crc_a),
                None,
                Some(&sha1_c),
                None,
            ),
            eligible: true,
        },
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "whole_file",
                Some(&crc_b),
                None,
                Some(&sha1_c),
                None,
            ),
            eligible: true,
        },
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new("whole_file", None, None, Some(&sha1_c), None),
            eligible: true,
        },
    ];

    let mut sequential = registry_connection()?;
    let expected = resolve_sequential(&mut sequential, &inputs)?;
    let mut bulk = registry_connection()?;
    let actual = resolve_content_identities(&mut bulk, &inputs)?;

    assert_eq!(decision_shape(&expected), decision_shape(&actual));
    assert_eq!(
        decision_shape(&actual),
        [
            "linked",
            "linked",
            "linked",
            "contradictory",
            "linked",
            "contradictory",
            "disputed",
        ]
    );
    persist_bulk(&mut bulk, &inputs, &actual)?;
    let expected_facts = sql_query(
        "SELECT assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance \
         FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING(digest_id) \
         ORDER BY assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance",
    )
    .load::<AssertionFactRow>(&mut sequential)?;
    let actual_facts = sql_query(
        "SELECT assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance \
         FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING(digest_id) \
         ORDER BY assertion.occurrence_id, digest.algorithm, digest.digest, assertion.scope, assertion.provenance",
    )
    .load::<AssertionFactRow>(&mut bulk)?;
    assert_eq!(expected_facts, actual_facts);
    Ok(())
}

#[test]
fn ineligible_invalid_assertions_are_not_validated_and_empty_batches_are_noops() -> crate::Result<()>
{
    let mut connection = registry_connection()?;
    let invalid = ContentIdentityInput {
        size: Some(-1),
        assertions: ContentDigestAssertions::new("whole_file", None, None, Some(&[1, 2]), None),
        eligible: false,
    };
    assert_eq!(
        resolve_content_identities(&mut connection, &[invalid])?,
        [ContentIdentityResolution::NoEligibleEvidence]
    );
    assert!(resolve_content_identities(&mut connection, &[])?.is_empty());
    Ok(())
}

#[test]
fn unknown_size_rejects_candidates_with_inconsistent_stored_sizes() -> crate::Result<()> {
    let content_id = CatalogContentId::from_bytes([0x77; 16]);
    let sha1 = [0x78; 20];
    let assertions = ContentDigestAssertions::new("whole_file", None, None, Some(&sha1), None);
    let mut sequential = registry_connection()?;
    let mut bulk = registry_connection()?;
    for connection in [&mut sequential, &mut bulk] {
        sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
            .bind::<Binary, _>(content_id.as_bytes().as_slice())
            .execute(connection)?;
        for (occurrence, size) in [(1_i64, 8_i64), (2, 4)] {
            sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
                .bind::<BigInt, _>(occurrence)
                .bind::<Binary, _>(content_id.as_bytes().as_slice())
                .execute(connection)?;
            sql_query("INSERT INTO native_file_sizes(occurrence_id,size) VALUES (?,?)")
                .bind::<BigInt, _>(occurrence)
                .bind::<BigInt, _>(size)
                .execute(connection)?;
            record_occurrence_digest_assertions(
                connection,
                OccurrenceId::from_database(occurrence),
                assertions,
                "source_declared",
            )?;
        }
    }

    let expected = resolve_content_identity(&mut sequential, None, assertions)?;
    let actual = resolve_content_identities(
        &mut bulk,
        &[ContentIdentityInput {
            size: None,
            assertions,
            eligible: true,
        }],
    )?;
    assert!(matches!(
        expected,
        ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::ContradictoryAssertions,
            ..
        }
    ));
    assert_eq!(decision_shape(&actual), ["contradictory"]);
    Ok(())
}

#[test]
fn wide_candidate_set_returns_a_persisted_normal_prefix_boundary() -> crate::Result<()> {
    let candidate_a = CatalogContentId::from_bytes([0x7a; 16]);
    let candidate_b = CatalogContentId::from_bytes([0x7b; 16]);
    let sha1_first = [0x11; 20];
    let sha1_middle = [0x22; 20];
    let sha1_last = [0x33; 20];
    let sha256_other = [0x44; 32];
    let prefix_sha1 = [0x55; 20];
    let inputs = [
        input(None, Some(&prefix_sha1), None, true),
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "whole_file",
                None,
                None,
                Some(&sha1_first),
                Some(&sha256_other),
            ),
            eligible: true,
        },
        input(None, Some(&sha1_middle), None, true),
    ];
    let mut bulk = registry_connection()?;
    seed_candidate(
        &mut bulk,
        candidate_a,
        &[sha1_first, sha1_middle, sha1_last],
        &[],
    )?;
    seed_candidate(&mut bulk, candidate_b, &[], &[sha256_other])?;

    let prefix = resolve_content_prefix(&mut bulk, &inputs)?;
    assert!(prefix.has_wide_conflict);
    assert_eq!(prefix.resolutions.len(), 1);
    assert!(matches!(
        prefix.resolutions.as_slice(),
        [ContentIdentityResolution::Linked(_)]
    ));
    assert_eq!(
        inputs[prefix.resolutions.len()].assertions.sha1,
        Some(&sha1_first[..])
    );
    let linked_id = prefix.resolutions[0]
        .content_id()
        .expect("normal prefix identity is inserted before returning");
    let inserted =
        sql_query("SELECT COUNT(*) AS count FROM catalog_contents WHERE content_uuid = ?")
            .bind::<Binary, _>(linked_id.as_bytes().as_slice())
            .get_result::<CountRow>(&mut bulk)?;
    assert_eq!(inserted.count, 1);
    Ok(())
}

#[test]
fn sha_bridge_wide_overlay_returns_prefix_then_restages_candidates() -> crate::Result<()> {
    let sha1 = [0x81; 20];
    let sha256 = [0x82; 32];
    let inputs = [
        input(None, Some(&sha1), None, true),
        input(None, None, Some(&sha256), true),
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "whole_file",
                None,
                None,
                Some(&sha1),
                Some(&sha256),
            ),
            eligible: true,
        },
    ];

    let mut sequential = registry_connection()?;
    let expected = resolve_sequential(&mut sequential, &inputs)?;
    assert_eq!(decision_shape(&expected), ["linked", "linked", "ambiguous"]);
    assert_ne!(expected[0].content_id(), expected[1].content_id());

    let mut bulk = registry_connection()?;
    let prefix = resolve_content_prefix(&mut bulk, &inputs)?;
    assert!(prefix.has_wide_conflict);
    assert_eq!(decision_shape(&prefix.resolutions), ["linked", "linked"]);
    assert_eq!(prefix.resolutions.len(), 2);

    persist_bulk(&mut bulk, &inputs[..2], &prefix.resolutions)?;
    let retried = resolve_content_prefix(&mut bulk, &inputs[2..])?;
    assert!(retried.has_wide_conflict);
    assert!(retried.resolutions.is_empty());
    Ok(())
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The fixture covers bounded size and digest extremes together"
)]
fn fact_hydration_returns_only_two_values_per_size_and_digest_kind() -> crate::Result<()> {
    let content_id = CatalogContentId::from_bytes([0x79; 16]);
    let digests = [[0x10; 20], [0x20; 20], [0x30; 20], [0x40; 20], [0x50; 20]];
    let mut connection = registry_connection()?;
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .execute(&mut connection)?;
    for (index, digest) in digests.iter().enumerate() {
        let occurrence = i64::try_from(index).expect("fixture index fits in SQLite INTEGER") + 1;
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Binary, _>(content_id.as_bytes().as_slice())
            .execute(&mut connection)?;
        sql_query("INSERT INTO native_file_sizes(occurrence_id,size) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<BigInt, _>(
                (i64::try_from(index).expect("fixture index fits in SQLite INTEGER") + 1) * 8,
            )
            .execute(&mut connection)?;
        record_occurrence_digest_assertions(
            &mut connection,
            OccurrenceId::from_database(occurrence),
            ContentDigestAssertions::new("whole_file", None, None, Some(digest), None),
            "source_declared",
        )?;
    }

    let facts = load_facts(&mut connection, &BTreeSet::from([content_id]))?;
    let candidate = facts.get(&content_id).expect("seeded candidate facts");
    assert_eq!(candidate.sizes.len(), 2);
    assert_eq!(candidate.digests.get("sha1").map(BTreeSet::len), Some(2));
    let resolution = resolve_content_identities(
        &mut connection,
        &[ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "whole_file",
                None,
                None,
                Some(&digests[0]),
                None,
            ),
            eligible: true,
        }],
    )?;
    assert!(matches!(
        resolution.as_slice(),
        [ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::ContradictoryAssertions,
            ..
        }]
    ));

    let inconsistent_id = CatalogContentId::from_bytes([0x7c; 16]);
    let shared_sha1 = [0x61; 20];
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<Binary, _>(inconsistent_id.as_bytes().as_slice())
        .execute(&mut connection)?;
    for (index, sha256) in [0x11_u8, 0x22, 0x33].into_iter().enumerate() {
        let occurrence = i64::try_from(index).expect("fixture index fits in SQLite INTEGER") + 20;
        let digest = [sha256; 32];
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Binary, _>(inconsistent_id.as_bytes().as_slice())
            .execute(&mut connection)?;
        record_occurrence_digest_assertions(
            &mut connection,
            OccurrenceId::from_database(occurrence),
            ContentDigestAssertions::new(
                "whole_file",
                None,
                None,
                Some(&shared_sha1),
                Some(&digest),
            ),
            "source_declared",
        )?;
    }
    let sparse_assertions =
        ContentDigestAssertions::new("whole_file", None, None, Some(&shared_sha1), None);
    let sequential = resolve_content_identity(&mut connection, None, sparse_assertions)?;
    assert!(matches!(
        sequential,
        ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::ContradictoryAssertions,
            ..
        }
    ));
    let resolution = resolve_content_identities(
        &mut connection,
        &[ContentIdentityInput {
            size: None,
            assertions: sparse_assertions,
            eligible: true,
        }],
    )?;
    assert!(matches!(
        resolution.as_slice(),
        [ContentIdentityResolution::Conflict {
            reason: ContentIdentityConflict::ContradictoryAssertions,
            ..
        }]
    ));
    Ok(())
}

#[test]
fn digest_fact_hydration_uses_rooted_plan_and_bounded_cached_shapes() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let mut connection = database.pool().get()?;
    let ids = (0_u8..8)
        .map(|tag| CatalogContentId::from_bytes([tag; 16]))
        .collect::<Vec<_>>();
    let trace = Arc::new(Mutex::new(DigestQueryTrace::default()));
    let observed = Arc::clone(&trace);
    connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        let (is_cache_query, sql) = match event {
            InstrumentationEvent::CacheQuery { sql, .. } => (true, sql.to_string()),
            InstrumentationEvent::StartQuery { query, .. } => (false, query.to_string()),
            _ => return,
        };
        let sql = sql.split(" -- binds:").next().unwrap_or(&sql).to_owned();
        if sql.contains("shared_file_hashes AS assertion") {
            let mut trace = observed.lock().expect("digest query trace");
            if is_cache_query {
                trace.cache_queries.push(sql);
            } else {
                trace.started.push(sql);
            }
        }
    });

    for unique_ids in [3, 4, 5, 8] {
        let ids = ids
            .iter()
            .take(unique_ids)
            .copied()
            .collect::<BTreeSet<_>>();
        assert!(load_facts(&mut connection, &ids)?.is_empty());
    }

    let (started, cache_queries) = {
        let trace = trace.lock().expect("digest query trace");
        assert_eq!(trace.started.len(), 4);
        assert_eq!(trace.started[0], trace.started[1]);
        assert_eq!(trace.started[2], trace.started[3]);
        assert_ne!(trace.started[1], trace.started[2]);
        assert_eq!(
            trace
                .started
                .iter()
                .map(|sql| sql.matches('?').count())
                .collect::<Vec<_>>(),
            [4, 4, 8, 8],
            "3 and 4 IDs share a four-bind statement; 5 and 8 share an eight-bind statement"
        );
        let cache_shapes = trace.cache_queries.iter().collect::<BTreeSet<_>>();
        assert_eq!(
            cache_shapes.len(),
            2,
            "only the padded SQL shapes are admitted"
        );
        let started_shapes = trace.started.iter().collect::<BTreeSet<_>>();
        assert_eq!(
            cache_shapes, started_shapes,
            "cached SQL fields must match executed SQL"
        );
        assert!(
            trace.cache_queries.len() <= 4,
            "cache checks remain batch-shaped"
        );
        (trace.started.clone(), trace.cache_queries.clone())
    };
    connection.set_instrumentation(|_: InstrumentationEvent<'_>| {});

    for sql in &started {
        assert_rooted_digest_plan(&mut connection, sql, &ids)?;
    }
    assert_eq!(cache_queries.iter().collect::<BTreeSet<_>>().len(), 2);
    Ok(())
}

fn assert_rooted_digest_plan(
    connection: &mut SqliteConnection,
    sql: &str,
    ids: &[CatalogContentId],
) -> crate::Result<()> {
    assert!(sql.contains("shared_file_hashes AS assertion"));
    assert!(sql.contains("assertion.content_uuid"));
    assert!(sql.contains("roots.root_uuid"));
    assert!(sql.contains("assertion.digest_id"));
    assert!(sql.contains("digest.algorithm"));
    assert!(sql.contains("digest.digest"));

    let mut explain =
        sql_query(format!("EXPLAIN QUERY PLAN {sql}")).into_boxed::<diesel::sqlite::Sqlite>();
    let bind_count = sql.matches('?').count();
    for id in ids.iter().take(bind_count) {
        explain = explain.bind::<Binary, _>(id.as_bytes().as_slice());
    }
    let details = explain
        .load::<DigestPlanRow>(connection)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    assert!(
        details.iter().any(|detail| {
            detail.contains("SEARCH assertion") && detail.contains("content_uuid=?")
        }),
        "digest hydration should seek shared facts by content UUID: {details:#?}"
    );
    assert!(
        !details.iter().any(|detail| detail.contains("occurrence")),
        "digest hydration should read shared facts without scanning occurrences: {details:#?}"
    );
    Ok(())
}

#[test]
fn lookup_and_assertion_statements_scale_by_chunks_not_rows() -> crate::Result<()> {
    let mut connection = registry_connection()?;
    let mut inputs = (0_u8..63)
        .map(|index| {
            let digest = [index; 20];
            input(None, Some(Box::leak(Box::new(digest))), None, true)
        })
        .collect::<Vec<_>>();
    inputs.push(input(
        None,
        Some(
            inputs[0]
                .assertions
                .sha1
                .expect("first fixture input has a SHA-1 assertion"),
        ),
        None,
        true,
    ));
    inputs.push(input(
        None,
        Some(
            inputs[0]
                .assertions
                .sha1
                .expect("first fixture input has a SHA-1 assertion"),
        ),
        None,
        true,
    ));
    let lookup_queries = Arc::new(AtomicUsize::new(0));
    let lookup_observed = Arc::clone(&lookup_queries);
    let assertion_queries = Arc::new(AtomicUsize::new(0));
    let assertion_observed = Arc::clone(&assertion_queries);
    connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if let InstrumentationEvent::StartQuery { query, .. } = event {
            let sql = query.to_string();
            if sql.contains("INSERT OR IGNORE INTO temp.import_identity_matches") {
                lookup_observed.fetch_add(1, Ordering::Relaxed);
            }
            if sql.contains("WITH incoming(occurrence_id")
                && sql.contains("occurrence_digest_assertions")
            {
                assertion_observed.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let first_resolutions = resolve_content_identities(&mut connection, &inputs[..64])?;
    assert_eq!(first_resolutions.len(), 64);
    assert_eq!(
        first_resolutions[0].content_id(),
        first_resolutions[63].content_id()
    );
    persist_bulk(&mut connection, &inputs[..64], &first_resolutions)?;
    let tail_resolutions = resolve_content_identities(&mut connection, &inputs[64..])?;
    assert_eq!(tail_resolutions.len(), 1);
    assert_eq!(
        first_resolutions[0].content_id(),
        tail_resolutions[0].content_id()
    );
    persist_bulk_from(&mut connection, &inputs[64..], &tail_resolutions, 65)?;
    assert_eq!(lookup_queries.load(Ordering::Relaxed), 2);
    assert_eq!(assertion_queries.load(Ordering::Relaxed), 2);
    let count = sql_query("SELECT COUNT(*) AS count FROM occurrence_digest_assertions")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(count.count, 65);
    Ok(())
}

fn input<'a>(
    size: Option<i64>,
    sha1: Option<&'a [u8]>,
    sha256: Option<&'a [u8]>,
    eligible: bool,
) -> ContentIdentityInput<'a> {
    ContentIdentityInput {
        size,
        assertions: ContentDigestAssertions::new("whole_file", None, None, sha1, sha256),
        eligible,
    }
}

fn resolve_sequential(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
) -> crate::Result<Vec<ContentIdentityResolution>> {
    let mut resolutions = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let resolution = if input.eligible {
            resolve_content_identity(connection, input.size, input.assertions)?
        } else {
            ContentIdentityResolution::NoEligibleEvidence
        };
        let occurrence = i64::try_from(index).expect("fixture index fits in SQLite INTEGER") + 1;
        let content_uuid = resolution.content_id().map(|id| id.as_bytes().to_vec());
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Nullable<Binary>, _>(content_uuid)
            .execute(connection)?;
        if let Some(size) = input.size {
            sql_query("INSERT INTO native_file_sizes(occurrence_id,size) VALUES (?,?)")
                .bind::<BigInt, _>(occurrence)
                .bind::<BigInt, _>(size)
                .execute(connection)?;
        }
        record_occurrence_digest_assertions(
            connection,
            OccurrenceId::from_database(occurrence),
            input.assertions,
            "source_declared",
        )?;
        if let ContentIdentityResolution::Conflict { candidates, .. } = &resolution {
            for candidate in candidates {
                sql_query(
                    "INSERT OR IGNORE INTO disputed_file_hashes(digest_id,candidate_content_uuid) \
                     SELECT assertion.digest_id, ? FROM occurrence_digest_assertions AS assertion \
                     JOIN digest_values AS digest USING(digest_id) \
                     WHERE assertion.occurrence_id = ? AND digest.algorithm IN ('sha1','sha256') \
                       AND assertion.scope IN ('whole_asset','whole_file') \
                       AND assertion.provenance='source_declared'",
                )
                .bind::<Binary, _>(candidate.as_bytes().as_slice())
                .bind::<BigInt, _>(occurrence)
                .execute(connection)?;
                sql_query(
                    "INSERT OR IGNORE INTO disputed_file_hashes(digest_id,candidate_content_uuid) \
                     SELECT assertion.digest_id, ? FROM catalog_content_digest_assertions AS assertion \
                     JOIN digest_values AS digest USING(digest_id) \
                     WHERE assertion.content_uuid = ? AND digest.algorithm IN ('sha1','sha256')",
                )
                .bind::<Binary, _>(candidate.as_bytes().as_slice())
                .bind::<Binary, _>(candidate.as_bytes().as_slice())
                .execute(connection)?;
            }
        }
        resolutions.push(resolution);
    }
    Ok(resolutions)
}

fn seed_candidate(
    connection: &mut SqliteConnection,
    content_id: CatalogContentId,
    sha1_digests: &[[u8; 20]],
    sha256_digests: &[[u8; 32]],
) -> crate::Result<()> {
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .execute(connection)?;
    let mut occurrence =
        sql_query("SELECT COALESCE(MAX(occurrence_id), 99) + 1 AS count FROM asset_occurrences")
            .get_result::<CountRow>(connection)?
            .count;
    for digest in sha1_digests {
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Binary, _>(content_id.as_bytes().as_slice())
            .execute(connection)?;
        record_occurrence_digest_assertions(
            connection,
            OccurrenceId::from_database(occurrence),
            ContentDigestAssertions::new("whole_file", None, None, Some(digest), None),
            "source_declared",
        )?;
        occurrence += 1;
    }
    for digest in sha256_digests {
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Binary, _>(content_id.as_bytes().as_slice())
            .execute(connection)?;
        record_occurrence_digest_assertions(
            connection,
            OccurrenceId::from_database(occurrence),
            ContentDigestAssertions::new("whole_file", None, None, None, Some(digest)),
            "source_declared",
        )?;
        occurrence += 1;
    }
    Ok(())
}

fn persist_bulk(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
    resolutions: &[ContentIdentityResolution],
) -> crate::Result<()> {
    persist_bulk_from(connection, inputs, resolutions, 1)
}

fn persist_bulk_from(
    connection: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
    resolutions: &[ContentIdentityResolution],
    first_occurrence: i64,
) -> crate::Result<()> {
    let mut assertions = Vec::with_capacity(inputs.len());
    for (index, (input, resolution)) in inputs.iter().zip(resolutions).enumerate() {
        let occurrence =
            first_occurrence + i64::try_from(index).expect("fixture index fits in SQLite INTEGER");
        let content_uuid = resolution.content_id().map(|id| id.as_bytes().to_vec());
        sql_query("INSERT INTO asset_occurrences(occurrence_id,content_uuid) VALUES (?,?)")
            .bind::<BigInt, _>(occurrence)
            .bind::<Nullable<Binary>, _>(content_uuid)
            .execute(connection)?;
        if let Some(size) = input.size {
            sql_query("INSERT INTO native_file_sizes(occurrence_id,size) VALUES (?,?)")
                .bind::<BigInt, _>(occurrence)
                .bind::<BigInt, _>(size)
                .execute(connection)?;
        }
        assertions.push((OccurrenceId::from_database(occurrence), input.assertions));
    }
    record_occurrence_digest_assertions_bulk(connection, &assertions, "source_declared")
}

fn decision_shape(resolutions: &[ContentIdentityResolution]) -> Vec<&'static str> {
    resolutions
        .iter()
        .map(|resolution| match resolution {
            ContentIdentityResolution::Linked(_) => "linked",
            ContentIdentityResolution::NoEligibleEvidence => "none",
            ContentIdentityResolution::Conflict { reason, .. } => match reason {
                ContentIdentityConflict::AmbiguousAlias => "ambiguous",
                ContentIdentityConflict::ContradictoryAssertions => "contradictory",
                ContentIdentityConflict::DisputedAlias => "disputed",
            },
        })
        .collect()
}

fn registry_connection() -> crate::Result<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
    connection.batch_execute(
        "CREATE TABLE catalog_contents (content_uuid BLOB PRIMARY KEY NOT NULL);
         CREATE TABLE digest_values (digest_id INTEGER PRIMARY KEY, algorithm TEXT NOT NULL, digest BLOB NOT NULL, UNIQUE(algorithm,digest));
         CREATE TABLE asset_occurrences (occurrence_id INTEGER PRIMARY KEY,content_uuid BLOB);
         CREATE TABLE occurrence_digest_assertions (occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,PRIMARY KEY(occurrence_id,digest_id,scope,provenance));
         CREATE TABLE native_file_sizes (occurrence_id INTEGER PRIMARY KEY,size INTEGER);
         CREATE VIEW accepted_file_size_assertions AS SELECT occurrence_id,'test_size' AS size_field,size FROM native_file_sizes;
         CREATE VIEW canonical_occurrence_content AS SELECT occurrence_id,content_uuid FROM asset_occurrences;
         CREATE VIEW catalog_content_digest_assertions AS SELECT occurrence.content_uuid,assertion.* FROM occurrence_digest_assertions AS assertion JOIN asset_occurrences AS occurrence USING(occurrence_id) WHERE occurrence.content_uuid IS NOT NULL AND scope IN ('whole_asset','whole_file') AND provenance='source_declared';
         CREATE TABLE shared_file_sizes(content_uuid BLOB NOT NULL,size INTEGER NOT NULL,PRIMARY KEY(content_uuid,size)) WITHOUT ROWID;
         CREATE TABLE shared_file_hashes(content_uuid BLOB NOT NULL,digest_id INTEGER NOT NULL,PRIMARY KEY(content_uuid,digest_id)) WITHOUT ROWID;
         CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(digest_id,content_uuid);
         CREATE TABLE merged_file_ids(old_content_uuid BLOB PRIMARY KEY,kept_content_uuid BLOB,decision_id INTEGER);
         CREATE TABLE file_match_decision_publications(decision_id INTEGER PRIMARY KEY);
         CREATE TABLE disputed_file_hashes(digest_id INTEGER,candidate_content_uuid BLOB);",
    )?;
    Ok(connection)
}
