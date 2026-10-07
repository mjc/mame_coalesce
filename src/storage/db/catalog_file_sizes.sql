-- Validate only identities touched by the selected publication. The scalar
-- native lookup is deliberately keyed by occurrence, not a global size union.
-- Callers supply canonical UUIDs; only published reverse redirects participate.
CREATE VIEW canonical_file_size_consistency AS
SELECT identity.content_uuid,
    (WITH RECURSIVE component(content_uuid) AS (
        SELECT identity.content_uuid
        UNION
        SELECT redirect.old_content_uuid FROM component
        JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid
        JOIN file_match_decision_publications USING (decision_id)
    ) SELECT count(*) = 2 FROM (
        SELECT DISTINCT
            (SELECT size FROM source_file_size_assertions AS sizes
             WHERE sizes.occurrence_id = known.occurrence_id) AS size
        FROM component
        CROSS JOIN asset_occurrences AS known ON known.content_uuid = component.content_uuid
        WHERE size IS NOT NULL
        LIMIT 2
    )) AS inconsistent
FROM catalog_contents AS identity;

-- Unknown incoming size cannot hide disagreements among retained source facts.
-- Unlinked entries do not touch an identity and remain independently publishable.
-- Validate each touched canonical UUID once, not once per duplicate declaration.
-- MATERIALIZED prevents flattening the component check back into the owner loop.
CREATE TRIGGER catalog_linked_file_size_publication BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    WITH touched_identities(content_uuid) AS MATERIALIZED (
        SELECT DISTINCT canonical.content_uuid
        FROM catalog_set_groups AS groups
        CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
        CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id
        JOIN canonical_occurrence_content AS canonical USING (occurrence_id)
        WHERE groups.snapshot_key = NEW.snapshot_key
    )
    SELECT 1 FROM touched_identities
    CROSS JOIN canonical_file_size_consistency AS consistency
      ON consistency.content_uuid = touched_identities.content_uuid
    WHERE consistency.inconsistent
)
BEGIN SELECT RAISE(ABORT, 'catalog UUID has contradictory source whole-file lengths (source evidence)'); END;

-- A well-formed byte string is not an issued file identity. Validate both
-- immutable source assignments and their current canonical root independently
-- of the connection's foreign-key setting.
CREATE TRIGGER catalog_linked_file_uuid_publication BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups AS groups
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
    CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id
    LEFT JOIN canonical_occurrence_content AS canonical USING (occurrence_id)
    WHERE groups.snapshot_key = NEW.snapshot_key AND occurrence.content_uuid IS NOT NULL
      AND (
        NOT EXISTS (SELECT 1 FROM catalog_contents AS issued
                    WHERE issued.content_uuid = occurrence.content_uuid)
        OR canonical.content_uuid IS NULL
        OR NOT EXISTS (SELECT 1 FROM catalog_contents AS issued
                       WHERE issued.content_uuid = canonical.content_uuid)
      )
)
BEGIN SELECT RAISE(ABORT, 'catalog publication requires an issued file UUID and canonical root'); END;
