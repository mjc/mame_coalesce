SELECT requested.assertion_key, result.relationship_id, result.origin,
       COALESCE(result.is_complete,0) AS is_complete,
       COALESCE(result.is_published,0) AS is_published,
       COALESCE(result.owners_published,0) AS owners_published
FROM requested
LEFT JOIN (
    WITH scoped_identity AS MATERIALIZED (
        SELECT identity.*
        FROM requested
        CROSS JOIN catalog_relationships AS identity USING(assertion_key)
    ), completeness AS (
        -- Keep the bounded materialized rows distinct from registry aliases in query plans.
        SELECT requested_identity.relationship_id, requested_identity.assertion_key, requested_identity.origin,
          CASE requested_identity.origin
            WHEN 'source' THEN (
              (SELECT COUNT(*) FROM reported_catalog_relationship_owners AS owner
               WHERE owner.relationship_id=requested_identity.relationship_id
                 AND owner.relationship_id IN (SELECT relationship_id FROM scoped_identity)
                 AND owner.source_reference_kind=(SELECT source_reference_kind
                     FROM reported_catalog_relationships
                     WHERE relationship_id=requested_identity.relationship_id)
                 AND owner.snapshot_key=requested_identity.snapshot_key)=1
              AND (SELECT COUNT(*) FROM reported_catalog_relationship_owner_ids AS owner
                   WHERE owner.relationship_id=requested_identity.relationship_id
                     AND owner.relationship_id IN (SELECT relationship_id FROM scoped_identity))=1
            )
            WHEN 'derived' THEN (
              EXISTS (SELECT 1 FROM inferred_catalog_relationships AS inferred
                      JOIN catalog_relationship_target_details AS source ON source.target_id=inferred.from_target_id
                      JOIN catalog_relationship_target_details AS target ON target.target_id=inferred.to_target_id
                      JOIN catalog_relationship_rules AS rule USING(rule_id)
                      WHERE inferred.relationship_id=requested_identity.relationship_id)
              AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships
                              WHERE relationship_id=requested_identity.relationship_id)
              AND NOT EXISTS (SELECT 1 FROM manual_catalog_relationships
                              WHERE relationship_id=requested_identity.relationship_id)
            )
            WHEN 'user' THEN (
              EXISTS (SELECT 1 FROM manual_catalog_relationships AS manual
                      JOIN catalog_relationship_target_details AS source ON source.target_id=manual.from_target_id
                      JOIN catalog_relationship_target_details AS target ON target.target_id=manual.to_target_id
                      WHERE manual.relationship_id=requested_identity.relationship_id)
              AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships
                              WHERE relationship_id=requested_identity.relationship_id)
              AND NOT EXISTS (SELECT 1 FROM inferred_catalog_relationships
                              WHERE relationship_id=requested_identity.relationship_id)
            )
            ELSE 0
          END AS is_complete
        FROM scoped_identity AS requested_identity
    ), ownership AS (
        SELECT complete.*,
          complete.is_complete AND CASE complete.origin
            WHEN 'source' THEN EXISTS (
              SELECT 1 FROM snapshot_publications AS publication
              WHERE publication.snapshot_key=(SELECT identity.snapshot_key
                    FROM catalog_relationships AS identity
                    WHERE identity.relationship_id=complete.relationship_id)
            )
            WHEN 'derived' THEN EXISTS (
              SELECT 1 FROM inferred_catalog_relationships AS inferred
              JOIN catalog_relationship_target_details AS source ON source.target_id=inferred.from_target_id
              JOIN catalog_relationship_target_details AS target ON target.target_id=inferred.to_target_id
              WHERE inferred.relationship_id=complete.relationship_id
                AND source.is_published AND target.is_published
            )
            WHEN 'user' THEN EXISTS (
              SELECT 1 FROM manual_catalog_relationships AS manual
              JOIN catalog_relationship_target_details AS source ON source.target_id=manual.from_target_id
              JOIN catalog_relationship_target_details AS target ON target.target_id=manual.to_target_id
              WHERE manual.relationship_id=complete.relationship_id
                AND source.is_published AND target.is_published
            )
            ELSE 0
          END AS owners_published
        FROM completeness AS complete
    )
    SELECT ownership.*,
      ownership.is_complete AND CASE ownership.origin
        WHEN 'source' THEN ownership.owners_published
        ELSE ownership.owners_published AND EXISTS (
          SELECT 1 FROM catalog_relationship_evidence_publications AS publication
          WHERE publication.relationship_id=ownership.relationship_id
        )
      END AS is_published
    FROM ownership
) AS result USING(assertion_key)
WHERE requested.assertion_key IS NOT NULL
