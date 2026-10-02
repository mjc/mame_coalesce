-- `requested(assertion_key)` is supplied by the API or trigger wrapper.
-- One predicate owns publication policy; native aliases seek their real owners.
SELECT requested.assertion_key,
    EXISTS (
        SELECT 1 FROM relationship_assertions AS assertion
        WHERE assertion.assertion_key=requested.assertion_key AND (
            (assertion.origin='source_assertion' AND EXISTS (
                SELECT 1 FROM snapshot_publications WHERE snapshot_key=assertion.source_snapshot_key))
            OR (assertion.origin<>'source_assertion' AND EXISTS (
                SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=assertion.assertion_key))
        )
    ) OR EXISTS (
        SELECT 1 FROM catalog_relationships AS registry
        JOIN reported_catalog_relationships AS reported USING(relationship_id)
        JOIN snapshot_publications AS publication ON publication.snapshot_key=registry.snapshot_key
        WHERE registry.assertion_key=requested.assertion_key AND registry.origin='source'
          AND EXISTS (SELECT 1 FROM reported_catalog_relationship_owners AS owner
              WHERE owner.relationship_id=registry.relationship_id
                AND owner.source_reference_kind=reported.source_reference_kind
                AND owner.snapshot_key=registry.snapshot_key
                AND owner.relationship_id IN (
                    SELECT scoped.relationship_id FROM catalog_relationships AS scoped
                    JOIN requested AS source ON scoped.assertion_key=source.assertion_key))
          AND (SELECT COUNT(*) FROM reported_catalog_relationship_owner_ids AS owner
               WHERE owner.relationship_id=registry.relationship_id
                 AND owner.relationship_id IN (
                     SELECT scoped.relationship_id FROM catalog_relationships AS scoped
                     JOIN requested AS source ON scoped.assertion_key=source.assertion_key))=1
    ) AS is_published
FROM requested WHERE requested.assertion_key IS NOT NULL
