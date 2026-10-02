evaluated AS (
    SELECT selected.*,
           predecessor.is_complete AS predecessor_complete,
           predecessor.is_published AS predecessor_published,
           successor.is_complete AS successor_complete,
           successor.is_published AS successor_published
    FROM selected
    LEFT JOIN catalog_relationships AS predecessor_identity
      ON predecessor_identity.relationship_id=selected.relationship_id
    LEFT JOIN readiness AS predecessor
      ON predecessor.assertion_key=predecessor_identity.assertion_key
    LEFT JOIN catalog_relationships AS successor_identity
      ON successor_identity.relationship_id=selected.replacement_relationship_id
    LEFT JOIN readiness AS successor
      ON successor.assertion_key=successor_identity.assertion_key
)
SELECT request.review_id, evaluated.review_key, evaluated.relationship_id,
       COALESCE(
         evaluated.review_key IS NOT NULL
         AND evaluated.predecessor_published
         AND ((evaluated.decision='superseded')=(evaluated.replacement_relationship_id IS NOT NULL))
         AND (evaluated.replacement_relationship_id IS NULL OR evaluated.successor_published), 0
       ) AS is_complete,
       COALESCE(
         evaluated.review_key IS NOT NULL
         AND evaluated.predecessor_published
         AND ((evaluated.decision='superseded')=(evaluated.replacement_relationship_id IS NOT NULL))
         AND (evaluated.replacement_relationship_id IS NULL OR evaluated.successor_published)
         AND EXISTS (SELECT 1 FROM catalog_relationship_review_publications AS publication
                     WHERE publication.review_id=evaluated.review_id), 0
       ) AS is_published
FROM requested_reviews AS request
LEFT JOIN evaluated USING(review_id)
