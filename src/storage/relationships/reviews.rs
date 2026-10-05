//! Review hydration starts from the numeric owners already selected by the
//! explanation reader, not a second expansion of every native source family.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    query_builder::{BoxedSqlQuery, SqlQuery},
    sql_query,
    sql_types::BigInt,
    sqlite::Sqlite,
};

use super::{
    ExplanationRow, REVIEW_READINESS_SQL, RelationshipId, RelationshipReviewId, ReviewRow,
};

// A SQL parameter batch, not a limit on relationships or review history.
const IDS_PER_QUERY: usize = 128;

#[derive(QueryableByName)]
struct ReviewCandidate {
    #[diesel(sql_type = BigInt)]
    review_id: i64,
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn candidates_query(count: usize) -> String {
    let values = placeholders(count);
    format!(
        "WITH requested_relationships(relationship_id) AS (VALUES {values}) \
         SELECT review.review_id \
         FROM requested_relationships AS request \
         CROSS JOIN catalog_relationship_reviews AS review USING(relationship_id) \
         CROSS JOIN catalog_relationship_review_publications AS publication USING(review_id)"
    )
}

pub(super) fn readiness_query(count: usize) -> String {
    let values = placeholders(count);
    format!(
        "WITH requested_reviews(review_id) AS (VALUES {values}), \
         review_readiness AS ({REVIEW_READINESS_SQL}) \
         SELECT owner.assertion_key, review.decision, review.note, \
                successor.assertion_key AS superseded_by_assertion_key, \
                review.reviewed_at AS created_at \
         FROM requested_reviews AS request \
         CROSS JOIN catalog_relationship_reviews AS review USING(review_id) \
         JOIN catalog_relationships AS owner USING(relationship_id) \
         JOIN review_readiness AS ready ON ready.review_id=review.review_id \
         LEFT JOIN replaced_catalog_relationships AS replacement ON replacement.review_id=review.review_id \
         LEFT JOIN catalog_relationships AS successor ON successor.relationship_id=replacement.replacement_relationship_id \
         WHERE ready.is_complete AND ready.is_published ORDER BY review.review_id"
    )
}

fn bind_ids<T>(
    sql: String,
    ids: &[T],
    value: impl Fn(&T) -> i64,
) -> BoxedSqlQuery<'static, Sqlite, SqlQuery> {
    let mut query = sql_query(sql).into_boxed::<Sqlite>();
    for id in ids {
        query = query.bind::<BigInt, _>(value(id));
    }
    query
}

pub(super) fn load(
    conn: &mut SqliteConnection,
    rows: &[ExplanationRow],
) -> crate::Result<Vec<ReviewRow>> {
    let mut relationship_ids = rows
        .iter()
        .map(|row| RelationshipId(row.relationship_id))
        .collect::<Vec<_>>();
    relationship_ids.sort_unstable();
    relationship_ids.dedup();
    let mut review_ids = Vec::new();
    for ids in relationship_ids.chunks(IDS_PER_QUERY) {
        review_ids.extend(
            bind_ids(candidates_query(ids.len()), ids, |id| id.0)
                .load::<ReviewCandidate>(conn)?
                .into_iter()
                .map(|row| RelationshipReviewId(row.review_id)),
        );
    }
    // A relationship can appear in several projections. Preserve one history,
    // in publication order, even when an owner's reviews cross SQL batches.
    review_ids.sort_unstable();
    review_ids.dedup();
    let mut reviews = Vec::new();
    for ids in review_ids.chunks(IDS_PER_QUERY) {
        reviews
            .extend(bind_ids(readiness_query(ids.len()), ids, |id| id.0).load::<ReviewRow>(conn)?);
    }
    Ok(reviews)
}
