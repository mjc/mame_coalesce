#![allow(clippy::expect_used)]

use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::{InstrumentationEvent, SimpleConnection},
    sql_types::{BigInt, Binary},
};

use super::{CandidateSummary, record_wide_conflict, staged_candidates};
use crate::{
    domain::{CatalogContentId, OccurrenceId},
    storage::catalog_content::{
        ContentDigestAssertions, ContentIdentityInput, record_occurrence_digest_assertions,
    },
};

#[derive(diesel::QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    detail: String,
}

fn trace_lookup_plan(
    conn: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
    hash: &[u8],
    request_indices: &[i64],
) -> Result<(Vec<CandidateSummary>, Vec<String>), Box<dyn std::error::Error>> {
    let lookup = std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured = std::sync::Arc::clone(&lookup);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if let InstrumentationEvent::StartQuery { query, .. } = event {
            let sql = query.to_string();
            if sql.contains("WITH incoming(request_index,algorithm,digest)") {
                *captured.lock().expect("staging query trace") =
                    Some(sql.split(" -- binds:").next().unwrap_or(&sql).to_owned());
            }
        }
    });
    let summaries = staged_candidates(conn, inputs);
    conn.set_instrumentation(|_: InstrumentationEvent<'_>| {});
    let summaries = summaries?;

    let lookup = lookup
        .lock()
        .expect("staging query trace")
        .clone()
        .expect("staging lookup was captured");
    let mut plan = diesel::sql_query(format!("EXPLAIN QUERY PLAN {lookup}"))
        .into_boxed::<diesel::sqlite::Sqlite>();
    for request_index in request_indices {
        plan = plan
            .bind::<BigInt, _>(*request_index)
            .bind::<diesel::sql_types::Text, _>("sha1")
            .bind::<Binary, _>(hash);
    }
    let details = plan
        .load::<ExplainRow>(conn)?
        .into_iter()
        .map(|row| row.detail)
        .collect();
    Ok((summaries, details))
}

fn repeated_digest_database(hash: &[u8]) -> Result<SqliteConnection, Box<dyn std::error::Error>> {
    let mut conn = SqliteConnection::establish(":memory:")?;
    conn.batch_execute(
        "CREATE TABLE catalog_contents(content_uuid BLOB PRIMARY KEY);
         CREATE TABLE asset_occurrences(
             occurrence_id INTEGER PRIMARY KEY,content_uuid BLOB,claim_kind TEXT);
         CREATE TABLE digest_values(
             digest_id INTEGER PRIMARY KEY,algorithm TEXT,digest BLOB,UNIQUE(algorithm,digest));
         CREATE TABLE occurrence_digest_assertions(
             occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,
             PRIMARY KEY(occurrence_id,digest_id,scope,provenance));
         CREATE TABLE native_sizes(occurrence_id INTEGER,size INTEGER);
         CREATE VIEW accepted_file_size_assertions AS
             SELECT occurrence_id,'test_size' AS size_field,size FROM native_sizes;
         CREATE TABLE merged_file_ids(old_content_uuid BLOB,kept_content_uuid BLOB,decision_id INTEGER);
         CREATE TABLE file_match_decision_publications(decision_id INTEGER PRIMARY KEY);
         CREATE TABLE file_match_hash_decisions(
             evidence_occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,
             disposition TEXT,decision_id INTEGER);
         CREATE TABLE disputed_file_hashes(digest_id INTEGER,candidate_content_uuid BLOB);
         CREATE TABLE shared_file_sizes(
             content_uuid BLOB NOT NULL,size INTEGER NOT NULL,
             PRIMARY KEY(content_uuid,size)) WITHOUT ROWID;
         CREATE TABLE shared_file_hashes(
             content_uuid BLOB NOT NULL,digest_id INTEGER NOT NULL,
             PRIMARY KEY(content_uuid,digest_id)) WITHOUT ROWID;
         CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(digest_id,content_uuid);
         CREATE VIEW canonical_occurrence_content AS
             SELECT occurrence_id,content_uuid FROM asset_occurrences WHERE content_uuid IS NOT NULL;
         CREATE VIEW catalog_content_digest_assertions AS
         SELECT canonical.content_uuid,assertion.*
         FROM occurrence_digest_assertions AS assertion
         JOIN asset_occurrences AS owner USING(occurrence_id)
         JOIN canonical_occurrence_content AS canonical USING(occurrence_id)
         WHERE owner.content_uuid IS NOT NULL
           AND owner.claim_kind='test_file'
           AND assertion.provenance='source_declared'
           AND assertion.scope IN ('whole_asset','whole_file')
           AND NOT EXISTS (
               SELECT 1 FROM file_match_hash_decisions AS review
               JOIN file_match_decision_publications USING(decision_id)
               WHERE review.evidence_occurrence_id=assertion.occurrence_id
                 AND review.digest_id=assertion.digest_id
                 AND review.scope=assertion.scope
                 AND review.provenance=assertion.provenance
                 AND review.disposition='reject');",
    )?;

    let candidates = [
        CatalogContentId::generate(),
        CatalogContentId::generate(),
        CatalogContentId::generate(),
        CatalogContentId::generate(),
    ];
    for (index, candidate) in candidates.iter().enumerate() {
        diesel::sql_query("INSERT INTO catalog_contents VALUES (?)")
            .bind::<Binary, _>(candidate.as_bytes().as_slice())
            .execute(&mut conn)?;
        diesel::sql_query("INSERT INTO asset_occurrences VALUES (?,?, 'test_file')")
            .bind::<BigInt, _>(i64::try_from(index + 1)?)
            .bind::<Binary, _>(candidate.as_bytes().as_slice())
            .execute(&mut conn)?;
    }

    let assertions = ContentDigestAssertions::new("whole_file", None, None, Some(hash), None);
    for index in 1_i64..=3 {
        record_occurrence_digest_assertions(
            &mut conn,
            OccurrenceId::from_database(index),
            assertions,
            "source_declared",
        )?;
    }
    let digest_id = diesel::sql_query("SELECT digest_id FROM digest_values WHERE algorithm='sha1'")
        .get_result::<DigestId>(&mut conn)?
        .digest_id;
    diesel::sql_query(
        "INSERT INTO file_match_hash_decisions
         VALUES (3,?,'whole_file','source_declared','reject',77)",
    )
    .bind::<BigInt, _>(digest_id)
    .execute(&mut conn)?;
    conn.batch_execute("INSERT INTO file_match_decision_publications VALUES (77)")?;
    // Mirror the production publication trigger after the fixture's review mutation.
    conn.batch_execute(
        "DELETE FROM shared_file_hashes;
         INSERT OR IGNORE INTO shared_file_hashes(content_uuid,digest_id)
         SELECT content_uuid,digest_id FROM catalog_content_digest_assertions;",
    )?;
    record_occurrence_digest_assertions(
        &mut conn,
        OccurrenceId::from_database(4),
        ContentDigestAssertions::new("member", None, None, Some(hash), None),
        "source_declared",
    )?;
    Ok(conn)
}

#[test]
fn redirected_alias_uses_shared_facts_without_occurrence_lookup()
-> Result<(), Box<dyn std::error::Error>> {
    let mut conn = SqliteConnection::establish(":memory:")?;
    conn.batch_execute(
        "CREATE TABLE catalog_contents(content_uuid BLOB PRIMARY KEY);
         CREATE TABLE asset_occurrences(occurrence_id INTEGER PRIMARY KEY,content_uuid BLOB);
         CREATE TABLE digest_values(digest_id INTEGER PRIMARY KEY,algorithm TEXT,digest BLOB,UNIQUE(algorithm,digest));
         CREATE TABLE occurrence_digest_assertions(occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,PRIMARY KEY(occurrence_id,digest_id,scope,provenance));
         CREATE TABLE native_sizes(occurrence_id INTEGER,size INTEGER);
         CREATE VIEW accepted_file_size_assertions AS SELECT occurrence_id,'test_size' AS size_field,size FROM native_sizes;
         CREATE TABLE merged_file_ids(old_content_uuid BLOB,kept_content_uuid BLOB,decision_id INTEGER);
         CREATE TABLE file_match_decision_publications(decision_id INTEGER);
         CREATE VIEW canonical_occurrence_content AS
         SELECT occurrence.occurrence_id,
             (WITH RECURSIVE path(content_uuid) AS (
                 SELECT occurrence.content_uuid
                 UNION ALL
                 SELECT redirect.kept_content_uuid FROM path
                 JOIN merged_file_ids AS redirect ON redirect.old_content_uuid=path.content_uuid
                 JOIN file_match_decision_publications USING(decision_id))
              SELECT content_uuid FROM path WHERE NOT EXISTS (
                 SELECT 1 FROM merged_file_ids AS redirect
                 JOIN file_match_decision_publications USING(decision_id)
                 WHERE redirect.old_content_uuid=path.content_uuid)) AS content_uuid
         FROM asset_occurrences AS occurrence WHERE occurrence.content_uuid IS NOT NULL;
         CREATE VIEW catalog_content_digest_assertions AS
         SELECT canonical.content_uuid,assertion.*
         FROM occurrence_digest_assertions AS assertion
         JOIN asset_occurrences AS owner USING(occurrence_id)
         JOIN canonical_occurrence_content AS canonical USING(occurrence_id)
         WHERE owner.content_uuid IS NOT NULL
           AND assertion.provenance='source_declared'
           AND assertion.scope IN ('whole_asset','whole_file');
         CREATE TABLE shared_file_sizes(content_uuid BLOB NOT NULL,size INTEGER NOT NULL,PRIMARY KEY(content_uuid,size)) WITHOUT ROWID;
         CREATE TABLE shared_file_hashes(content_uuid BLOB NOT NULL,digest_id INTEGER NOT NULL,PRIMARY KEY(content_uuid,digest_id)) WITHOUT ROWID;
         CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(digest_id,content_uuid);
         CREATE TABLE disputed_file_hashes(digest_id INTEGER,candidate_content_uuid BLOB);",
    )?;

    let issued = CatalogContentId::generate();
    let root = CatalogContentId::generate();
    for content in [issued, root] {
        diesel::sql_query("INSERT INTO catalog_contents VALUES (?)")
            .bind::<Binary, _>(content.as_bytes().as_slice())
            .execute(&mut conn)?;
    }
    diesel::sql_query("INSERT INTO asset_occurrences VALUES (1,?),(2,?)")
        .bind::<Binary, _>(issued.as_bytes().as_slice())
        .bind::<Binary, _>(root.as_bytes().as_slice())
        .execute(&mut conn)?;
    conn.batch_execute(
        "INSERT INTO merged_file_ids VALUES (
             (SELECT content_uuid FROM asset_occurrences WHERE occurrence_id=1),
             (SELECT content_uuid FROM asset_occurrences WHERE occurrence_id=2),77);
         INSERT INTO file_match_decision_publications VALUES (77);",
    )?;

    let hash = [9_u8; 20];
    let assertions = ContentDigestAssertions::new("whole_file", None, None, Some(&hash), None);
    record_occurrence_digest_assertions(
        &mut conn,
        OccurrenceId::from_database(1),
        assertions,
        "source_declared",
    )?;

    let (summaries, details) = trace_lookup_plan(
        &mut conn,
        &[ContentIdentityInput {
            size: None,
            assertions,
            eligible: true,
        }],
        &hash,
        &[0],
    )?;

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].candidate, Some(root));
    assert_eq!(summaries[0].count, 1);
    assert!(
        !details
            .iter()
            .any(|detail| detail.contains("CORRELATED SCALAR SUBQUERY")),
        "alias lookup should leave redirect normalization to the staged path: {details:#?}"
    );
    Ok(())
}

#[test]
fn repeated_digest_matches_expand_to_eligible_whole_file_requests()
-> Result<(), Box<dyn std::error::Error>> {
    let hash = [11_u8; 20];
    let mut conn = repeated_digest_database(&hash)?;
    let assertions = ContentDigestAssertions::new("whole_file", None, None, Some(&hash), None);
    let inputs = [
        ContentIdentityInput {
            size: None,
            assertions,
            eligible: true,
        },
        ContentIdentityInput {
            size: None,
            assertions,
            eligible: true,
        },
        ContentIdentityInput {
            size: None,
            assertions,
            eligible: false,
        },
        ContentIdentityInput {
            size: None,
            assertions: ContentDigestAssertions::new(
                "archive_member",
                None,
                None,
                Some(&hash),
                None,
            ),
            eligible: true,
        },
    ];
    let (summaries, details) = trace_lookup_plan(&mut conn, &inputs, &hash, &[0, 1])?;

    assert_eq!(summaries.len(), 2);
    assert!(summaries.iter().all(|row| {
        matches!(row.request_index, 0 | 1)
            && row.count == 2
            && row.candidate.is_none()
            && !row.disputed
    }));
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("MATERIALIZE requested")),
        "repeated digest lookup should materialize distinct requests once: {details:#?}"
    );
    Ok(())
}

#[derive(diesel::QueryableByName)]
struct DigestId {
    #[diesel(sql_type = BigInt, column_name = digest_id)]
    digest_id: i64,
}

#[test]
fn wide_candidates_stay_in_sql_and_all_evidence_is_recorded()
-> Result<(), Box<dyn std::error::Error>> {
    #[derive(diesel::QueryableByName)]
    struct Counts {
        #[diesel(sql_type=BigInt)]
        candidates: i64,
        #[diesel(sql_type=BigInt)]
        hashes: i64,
        #[diesel(sql_type=BigInt)]
        sizes: i64,
        #[diesel(sql_type=BigInt)]
        staged: i64,
    }
    let mut conn = SqliteConnection::establish(":memory:")?;
    conn.batch_execute("CREATE TABLE catalog_contents(content_uuid BLOB PRIMARY KEY);
        CREATE TABLE asset_occurrences(occurrence_id INTEGER PRIMARY KEY,content_uuid BLOB);
        CREATE TABLE digest_values(digest_id INTEGER PRIMARY KEY,algorithm TEXT,digest BLOB,UNIQUE(algorithm,digest));
        CREATE TABLE occurrence_digest_assertions(occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,PRIMARY KEY(occurrence_id,digest_id,scope,provenance));
        CREATE VIEW canonical_occurrence_content AS SELECT occurrence_id,content_uuid FROM asset_occurrences;
        CREATE VIEW catalog_content_digest_assertions AS SELECT entry.content_uuid,assertion.* FROM asset_occurrences AS entry JOIN occurrence_digest_assertions AS assertion USING(occurrence_id) WHERE content_uuid IS NOT NULL;
        CREATE TABLE shared_file_sizes(content_uuid BLOB NOT NULL,size INTEGER NOT NULL,PRIMARY KEY(content_uuid,size)) WITHOUT ROWID;
        CREATE TABLE shared_file_hashes(content_uuid BLOB NOT NULL,digest_id INTEGER NOT NULL,PRIMARY KEY(content_uuid,digest_id)) WITHOUT ROWID;
        CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(digest_id,content_uuid);
        CREATE TABLE merged_file_ids(old_content_uuid BLOB,kept_content_uuid BLOB,decision_id INTEGER);
        CREATE TABLE file_match_decision_publications(decision_id INTEGER);
        CREATE TABLE native_sizes(occurrence_id INTEGER,size INTEGER);
        CREATE VIEW accepted_file_size_assertions AS SELECT occurrence_id,'test_size' AS size_field,size FROM native_sizes;
        CREATE TABLE occurrence_content_conflicts(occurrence_id INTEGER,candidate_content_uuid BLOB,reason TEXT,PRIMARY KEY(occurrence_id,candidate_content_uuid));
        CREATE TABLE occurrence_content_conflict_hashes(occurrence_id INTEGER,candidate_content_uuid BLOB,evidence_occurrence_id INTEGER,digest_id INTEGER,scope TEXT,provenance TEXT,role TEXT);
        CREATE TABLE occurrence_content_conflict_sizes(occurrence_id INTEGER,candidate_content_uuid BLOB,evidence_occurrence_id INTEGER,size_field TEXT,role TEXT);
        CREATE VIEW disputed_file_hashes AS SELECT digest_id,candidate_content_uuid FROM occurrence_content_conflict_hashes;")?;
    let hash = [7_u8; 20];
    let assertions = ContentDigestAssertions::new("whole_file", None, None, Some(&hash), None);
    for index in 1_i64..=257 {
        let content = CatalogContentId::generate();
        diesel::sql_query("INSERT INTO catalog_contents VALUES (?)")
            .bind::<Binary, _>(content.as_bytes().as_slice())
            .execute(&mut conn)?;
        diesel::sql_query("INSERT INTO asset_occurrences VALUES (?,?)")
            .bind::<BigInt, _>(index)
            .bind::<Binary, _>(content.as_bytes().as_slice())
            .execute(&mut conn)?;
        diesel::sql_query("INSERT INTO native_sizes VALUES (?,8)")
            .bind::<BigInt, _>(index)
            .execute(&mut conn)?;
        record_occurrence_digest_assertions(
            &mut conn,
            OccurrenceId::from_database(index),
            assertions,
            "source_declared",
        )?;
    }
    let inputs = [ContentIdentityInput {
        size: Some(8),
        assertions,
        eligible: true,
    }; 64];
    let summaries = staged_candidates(&mut conn, &inputs)?;
    assert_eq!(summaries.len(), 64);
    assert!(
        summaries
            .iter()
            .all(|row| row.count == 257 && row.candidate.is_none() && !row.disputed)
    );
    conn.batch_execute("INSERT INTO asset_occurrences VALUES (1000,NULL); INSERT INTO native_sizes VALUES (1000,8)")?;
    let occurrence = OccurrenceId::from_database(1000);
    record_occurrence_digest_assertions(&mut conn, occurrence, assertions, "source_declared")?;
    record_wide_conflict(&mut conn, occurrence, assertions)?;
    let counts = diesel::sql_query("SELECT (SELECT count(*) FROM occurrence_content_conflicts) AS candidates,(SELECT count(*) FROM occurrence_content_conflict_hashes) AS hashes,(SELECT count(*) FROM occurrence_content_conflict_sizes) AS sizes,(SELECT count(*) FROM temp.import_identity_candidates) AS staged").get_result::<Counts>(&mut conn)?;
    assert_eq!(
        (
            counts.candidates,
            counts.hashes,
            counts.sizes,
            counts.staged
        ),
        (257, 514, 514, 0)
    );
    let summaries = staged_candidates(&mut conn, &inputs[..1])?;
    assert_eq!(summaries.len(), 1);
    assert!(summaries[0].disputed);
    Ok(())
}
