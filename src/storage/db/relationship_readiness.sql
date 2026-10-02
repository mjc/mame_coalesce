SELECT * FROM (
WITH scoped_identity AS MATERIALIZED (
    SELECT identity.* FROM requested
    CROSS JOIN catalog_relationships AS identity USING(assertion_key)
), completeness AS (
SELECT selected.relationship_id, selected.assertion_key, selected.origin,
       CASE selected.origin
         WHEN 'source' THEN (
             (SELECT COUNT(*) FROM reported_catalog_relationship_owners AS owner
              WHERE owner.relationship_id=selected.relationship_id
                AND owner.relationship_id IN (SELECT relationship_id FROM scoped_identity)
                AND owner.source_reference_kind=(SELECT source_reference_kind
                    FROM reported_catalog_relationships WHERE relationship_id=selected.relationship_id)
                AND owner.snapshot_key=selected.snapshot_key)=1
             AND (SELECT COUNT(*) FROM reported_catalog_relationship_owner_ids AS owner
                  WHERE owner.relationship_id=selected.relationship_id
                    AND owner.relationship_id IN (SELECT relationship_id FROM scoped_identity))=1
         )
         WHEN 'derived' THEN (
             EXISTS (SELECT 1 FROM inferred_catalog_relationships AS inferred
                     JOIN catalog_relationship_target_details AS source ON source.target_id=inferred.from_target_id
                     JOIN catalog_relationship_target_details AS target ON target.target_id=inferred.to_target_id
                     JOIN catalog_relationship_rules AS rule USING(rule_id)
                     WHERE inferred.relationship_id=selected.relationship_id)
             AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id=selected.relationship_id)
             AND NOT EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id=selected.relationship_id)
         )
         WHEN 'user' THEN (
             EXISTS (SELECT 1 FROM manual_catalog_relationships AS manual
                     JOIN catalog_relationship_target_details AS source ON source.target_id=manual.from_target_id
                     JOIN catalog_relationship_target_details AS target ON target.target_id=manual.to_target_id
                     WHERE manual.relationship_id=selected.relationship_id)
             AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id=selected.relationship_id)
             AND NOT EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id=selected.relationship_id)
         )
         ELSE 0
       END AS is_complete
FROM scoped_identity AS selected
), closure AS (
SELECT complete.*,
       complete.is_complete AND CASE complete.origin
         WHEN 'source' THEN EXISTS (
             SELECT 1 FROM catalog_relationships AS identity
             JOIN snapshot_publications AS publication USING(snapshot_key)
             WHERE identity.relationship_id=complete.relationship_id)
         ELSE EXISTS (
             SELECT 1 FROM relationship_evidence_publications AS evidence
             WHERE evidence.assertion_key=complete.assertion_key)
           AND EXISTS (
             SELECT 1 FROM inferred_catalog_relationships AS inferred
             JOIN catalog_relationship_target_details AS source ON source.target_id=inferred.from_target_id
             JOIN catalog_relationship_target_details AS target ON target.target_id=inferred.to_target_id
             WHERE inferred.relationship_id=complete.relationship_id AND source.is_published AND target.is_published
             UNION ALL
             SELECT 1 FROM manual_catalog_relationships AS manual
             JOIN catalog_relationship_target_details AS source ON source.target_id=manual.from_target_id
             JOIN catalog_relationship_target_details AS target ON target.target_id=manual.to_target_id
             WHERE manual.relationship_id=complete.relationship_id AND source.is_published AND target.is_published)
       END AS is_published
FROM completeness AS complete
)
SELECT requested.assertion_key, closure.relationship_id, closure.origin,
       COALESCE(closure.is_complete,0) AS is_complete,
       COALESCE(closure.is_published,0) AS is_published
FROM requested LEFT JOIN closure USING(assertion_key)
WHERE requested.assertion_key IS NOT NULL
)
