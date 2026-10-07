//! Candidate fan-out stays in SQLite, not in a catalog-sized Rust collection.
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
    sqlite::Sqlite,
};

use super::super::{ContentDigestAssertions, ContentIdentityInput, content_id};
use crate::{
    domain::{CatalogContentId, OccurrenceId},
    storage::cached_sql::cached_sql,
};

#[cfg(test)]
mod tests;

pub(super) struct CandidateSummary {
    pub request_index: usize,
    pub candidate: Option<CatalogContentId>,
    pub count: i64,
    pub disputed: bool,
}

#[derive(QueryableByName)]
struct SummaryRow {
    #[diesel(sql_type = BigInt)]
    request_index: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    candidate_count: i64,
    #[diesel(sql_type = BigInt)]
    disputed: i64,
}

const CLEAR: &str = "DELETE FROM temp.import_identity_matches;
    DELETE FROM temp.import_identity_paths;
    DELETE FROM temp.import_identity_candidates";

pub(super) fn staged_candidates(
    conn: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
) -> crate::Result<Vec<CandidateSummary>> {
    let summaries = stage(conn, inputs)?;
    conn.batch_execute(CLEAR)?;
    Ok(summaries)
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep the staged SQL dependency order visible together"
)]
fn stage(
    conn: &mut SqliteConnection,
    inputs: &[ContentIdentityInput<'_>],
) -> crate::Result<Vec<CandidateSummary>> {
    #[derive(QueryableByName)]
    struct Invalid {
        #[diesel(sql_type = Binary)]
        issued_uuid: Vec<u8>,
    }
    conn.batch_execute("CREATE TEMP TABLE IF NOT EXISTS import_identity_matches (
        request_index INTEGER NOT NULL, issued_uuid BLOB NOT NULL, disputed INTEGER NOT NULL,
        PRIMARY KEY(request_index,issued_uuid,disputed)) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS import_identity_paths (
        request_index INTEGER NOT NULL, issued_uuid BLOB NOT NULL, content_uuid BLOB NOT NULL, disputed INTEGER NOT NULL,
        PRIMARY KEY(request_index,issued_uuid,content_uuid,disputed)) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS import_identity_candidates (
        request_index INTEGER NOT NULL, content_uuid BLOB NOT NULL, disputed INTEGER NOT NULL,
        PRIMARY KEY(request_index,content_uuid)) WITHOUT ROWID;")?;
    conn.batch_execute(CLEAR)?;
    let assertions = inputs
        .iter()
        .enumerate()
        .filter(|(_, input)| input.eligible)
        .flat_map(|(index, input)| {
            input
                .assertions
                .iter()
                .filter(|assertion| assertion.identifies_whole_file())
                .map(move |assertion| (index, assertion))
        })
        .collect::<Vec<_>>();
    // Even callers with a larger slice use bounded parameter pages. Membership
    // is accumulated on disk by SQLite, never loaded as one candidate vector.
    for page in assertions.chunks(128) {
        let values = std::iter::repeat_n("(?,?,?)", page.len())
            .collect::<Vec<_>>()
            .join(",");
        // Accepted aliases use the shared source/review qualification view.
        // Its redirect traversal relies on the schema's merged_file_cycle
        // trigger; arbitrary tampering with that graph is not supported here.
        let statement = format!(
            "WITH incoming(request_index,algorithm,digest) AS (VALUES {values})
            INSERT OR IGNORE INTO temp.import_identity_matches(request_index,issued_uuid,disputed)
            SELECT incoming.request_index,assertion.content_uuid,0 FROM incoming
            JOIN digest_values AS digest USING(algorithm,digest)
            JOIN catalog_content_digest_assertions AS assertion USING(digest_id)
            UNION ALL
            SELECT incoming.request_index,dispute.candidate_content_uuid,1 FROM incoming
            JOIN digest_values AS digest USING(algorithm,digest)
            JOIN disputed_file_hashes AS dispute USING(digest_id)"
        );
        let mut query = sql_query(statement).into_boxed::<Sqlite>();
        for (index, assertion) in page {
            let index = i64::try_from(*index).map_err(|_| {
                crate::Error::InvalidPath("bulk input index exceeds SQLite integer".into())
            })?;
            query = query
                .bind::<BigInt, _>(index)
                .bind::<Text, _>(assertion.algorithm.as_str())
                .bind::<Binary, _>(assertion.value);
        }
        query.execute(conn)?;
    }
    cached_sql(
        "WITH RECURSIVE path(request_index,issued_uuid,content_uuid,disputed) AS (
        SELECT request_index,issued_uuid,issued_uuid,disputed FROM temp.import_identity_matches
        UNION SELECT path.request_index,path.issued_uuid,redirect.kept_content_uuid,path.disputed
        FROM path JOIN merged_file_ids AS redirect ON redirect.old_content_uuid=path.content_uuid
        JOIN file_match_decision_publications USING(decision_id))
        INSERT INTO temp.import_identity_paths
        SELECT request_index,issued_uuid,content_uuid,disputed FROM path
        WHERE NOT EXISTS (SELECT 1 FROM merged_file_ids AS redirect
            JOIN file_match_decision_publications USING(decision_id)
            WHERE redirect.old_content_uuid=path.content_uuid)",
    )
    .execute(conn)?;
    let invalid = cached_sql(
        "SELECT matches.issued_uuid FROM temp.import_identity_matches AS matches
        LEFT JOIN temp.import_identity_paths AS paths USING(request_index,issued_uuid,disputed)
        GROUP BY matches.request_index,matches.issued_uuid,matches.disputed
        HAVING COUNT(paths.content_uuid)<>1 LIMIT 1",
    )
    .get_result::<Invalid>(conn)
    .optional()?;
    if invalid.is_some() {
        return Err(crate::storage::file_match_reviews::ReviewError::CorruptRedirects.into());
    }
    let unknown = cached_sql("SELECT paths.issued_uuid FROM temp.import_identity_paths AS paths
        WHERE NOT EXISTS (SELECT 1 FROM catalog_contents WHERE content_uuid=paths.content_uuid) LIMIT 1")
        .get_result::<Invalid>(conn).optional()?;
    if let Some(unknown) = unknown {
        return Err(
            crate::storage::file_match_reviews::ReviewError::UnknownFile(content_id(
                unknown.issued_uuid,
            )?)
            .into(),
        );
    }
    cached_sql(
        "INSERT INTO temp.import_identity_candidates
        SELECT request_index,content_uuid,MAX(disputed) FROM temp.import_identity_paths
        GROUP BY request_index,content_uuid",
    )
    .execute(conn)?;
    let summaries = cached_sql(
        "SELECT request_index,COUNT(*) AS candidate_count,
        CASE WHEN COUNT(*)=1 THEN MIN(content_uuid) ELSE NULL END AS content_uuid,
        MAX(disputed) AS disputed FROM temp.import_identity_candidates GROUP BY request_index",
    )
    .load::<SummaryRow>(conn)?;
    summaries
        .into_iter()
        .map(|row| {
            Ok(CandidateSummary {
                request_index: usize::try_from(row.request_index)
                    .map_err(|_| crate::Error::InvalidPath("negative bulk input index".into()))?,
                candidate: row.content_uuid.map(content_id).transpose()?,
                count: row.candidate_count,
                disputed: row.disputed != 0,
            })
        })
        .collect()
}

pub(super) fn record_wide_conflict(
    conn: &mut SqliteConnection,
    occurrence: OccurrenceId,
    assertions: ContentDigestAssertions<'_>,
) -> crate::Result<()> {
    // Restage after the source prefix has been written. Its new aliases and
    // disputes must participate, and the incoming unlinked occurrence must not.
    let summaries = stage(
        conn,
        &[ContentIdentityInput {
            size: None,
            assertions,
            eligible: true,
        }],
    )?;
    let Some(summary) = summaries.first().filter(|summary| summary.count > 1) else {
        return Err(crate::Error::DatabaseSchema(
            "wide identity conflict lost its candidates".into(),
        ));
    };
    let reason = if summary.disputed {
        "disputed_alias"
    } else {
        "ambiguous_alias"
    };
    cached_sql(
        "INSERT INTO occurrence_content_conflicts(occurrence_id,candidate_content_uuid,reason)
        SELECT ?,content_uuid,? FROM temp.import_identity_candidates WHERE request_index=0",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<Text, _>(reason)
    .execute(conn)?;
    cached_sql("INSERT INTO occurrence_content_conflict_hashes
        (occurrence_id,candidate_content_uuid,evidence_occurrence_id,digest_id,scope,provenance,role)
        SELECT ?,candidate.content_uuid,assertion.occurrence_id,assertion.digest_id,assertion.scope,assertion.provenance,'incoming'
        FROM temp.import_identity_candidates AS candidate CROSS JOIN occurrence_digest_assertions AS assertion
        WHERE candidate.request_index=0 AND assertion.occurrence_id=?
        AND assertion.scope IN ('whole_asset','whole_file') AND assertion.provenance='source_declared'")
        .bind::<BigInt,_>(occurrence.database_value()).bind::<BigInt,_>(occurrence.database_value()).execute(conn)?;
    let component = "WITH RECURSIVE component(candidate_uuid,content_uuid) AS (
        SELECT content_uuid,content_uuid FROM temp.import_identity_candidates WHERE request_index=0
        UNION SELECT component.candidate_uuid,redirect.old_content_uuid FROM component
        JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid=component.content_uuid
        JOIN file_match_decision_publications USING(decision_id))";
    sql_query(format!("{component} INSERT INTO occurrence_content_conflict_hashes
        (occurrence_id,candidate_content_uuid,evidence_occurrence_id,digest_id,scope,provenance,role)
        SELECT ?,component.candidate_uuid,assertion.occurrence_id,assertion.digest_id,assertion.scope,assertion.provenance,'candidate'
        FROM component CROSS JOIN asset_occurrences AS entry ON entry.content_uuid=component.content_uuid
        CROSS JOIN catalog_content_digest_assertions AS assertion ON assertion.occurrence_id=entry.occurrence_id"))
        .bind::<BigInt,_>(occurrence.database_value()).execute(conn)?;
    cached_sql("INSERT INTO occurrence_content_conflict_sizes
        (occurrence_id,candidate_content_uuid,evidence_occurrence_id,size_field,role)
        SELECT ?,candidate.content_uuid,sizes.occurrence_id,sizes.size_field,'incoming'
        FROM temp.import_identity_candidates AS candidate CROSS JOIN accepted_file_size_assertions AS sizes
        WHERE candidate.request_index=0 AND sizes.occurrence_id=?")
        .bind::<BigInt,_>(occurrence.database_value()).bind::<BigInt,_>(occurrence.database_value()).execute(conn)?;
    sql_query(format!("{component} INSERT INTO occurrence_content_conflict_sizes
        (occurrence_id,candidate_content_uuid,evidence_occurrence_id,size_field,role)
        SELECT ?,component.candidate_uuid,sizes.occurrence_id,sizes.size_field,'candidate'
        FROM component CROSS JOIN asset_occurrences AS entry ON entry.content_uuid=component.content_uuid
        CROSS JOIN accepted_file_size_assertions AS sizes ON sizes.occurrence_id=entry.occurrence_id"))
        .bind::<BigInt,_>(occurrence.database_value()).execute(conn)?;
    conn.batch_execute(CLEAR)?;
    Ok(())
}
