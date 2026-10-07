use diesel::{
    Connection, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_types::{BigInt, Binary},
};

use super::{record_wide_conflict, staged_candidates};
use crate::{
    domain::{CatalogContentId, OccurrenceId},
    storage::catalog_content::{
        ContentDigestAssertions, ContentIdentityInput, record_occurrence_digest_assertions,
    },
};

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
        CREATE VIEW catalog_content_digest_assertions AS SELECT entry.content_uuid,assertion.* FROM asset_occurrences AS entry JOIN occurrence_digest_assertions AS assertion USING(occurrence_id) WHERE content_uuid IS NOT NULL;
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
