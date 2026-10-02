proposed AS (
    SELECT request.review_id, review.relationship_id,
           replacement.replacement_relationship_id
    FROM requested_reviews AS request
    LEFT JOIN catalog_relationship_reviews AS review USING(review_id)
    LEFT JOIN replaced_catalog_relationships AS replacement USING(review_id)
), review_walk(start_relationship_id, relationship_id) AS (
    SELECT replacement_relationship_id, replacement_relationship_id
    FROM proposed WHERE replacement_relationship_id IS NOT NULL
    UNION
    SELECT walk.start_relationship_id, replacement.replacement_relationship_id
    FROM review_walk AS walk
    JOIN catalog_relationship_reviews AS latest
      ON latest.review_id=(
          SELECT candidate.review_id
          FROM catalog_relationship_reviews AS candidate
          WHERE candidate.relationship_id=walk.relationship_id
            AND EXISTS (SELECT 1 FROM catalog_relationship_review_publications AS seal
                        WHERE seal.review_id=candidate.review_id)
          ORDER BY candidate.review_id DESC
          LIMIT 1
      )
    JOIN replaced_catalog_relationships AS replacement USING(review_id)
    WHERE latest.decision='superseded'
), cycle_results AS (
    SELECT proposed.review_id,
           proposed.relationship_id AS predecessor_relationship_id,
           proposed.replacement_relationship_id,
           proposed.replacement_relationship_id=proposed.relationship_id
             OR EXISTS (
                 SELECT 1 FROM review_walk AS walk
                 WHERE walk.start_relationship_id=proposed.replacement_relationship_id
                   AND walk.relationship_id=proposed.relationship_id
             ) AS creates_cycle
    FROM proposed
)
