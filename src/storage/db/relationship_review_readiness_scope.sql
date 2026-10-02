selected AS MATERIALIZED (
    SELECT request.review_id, review.review_key, review.relationship_id, review.decision,
           replacement.replacement_relationship_id
    FROM requested_reviews AS request
    LEFT JOIN catalog_relationship_reviews AS review USING(review_id)
    LEFT JOIN replaced_catalog_relationships AS replacement USING(review_id)
), requested(assertion_key) AS (
    SELECT identity.assertion_key FROM selected
    CROSS JOIN catalog_relationships AS identity ON identity.relationship_id=selected.relationship_id
    UNION
    SELECT identity.assertion_key FROM selected
    CROSS JOIN catalog_relationships AS identity ON identity.relationship_id=selected.replacement_relationship_id
), readiness AS (
