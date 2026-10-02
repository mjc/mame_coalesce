-- Collection payloads require the native owner even when FK enforcement is off.
CREATE TRIGGER no_intro_dat_set_links_native_owner_insert
BEFORE INSERT ON no_intro_dat_set_links
WHEN EXISTS (SELECT 1 FROM no_intro_dat_set_links
             WHERE (set_id, link_kind) = (NEW.set_id, NEW.link_kind)
                OR relationship_id = NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids
            WHERE relationship_id = NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1
    FROM no_intro_dat_games AS native
    JOIN catalog_sets AS sets ON sets.set_id = native.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
    JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = groups.snapshot_key
    JOIN parser_interpretations AS interpretation
      ON interpretation.interpretation_key = snapshot.interpretation_key
    JOIN catalog_relationships AS identity ON identity.relationship_id = NEW.relationship_id
    JOIN reported_catalog_relationships AS reported
      ON reported.relationship_id = identity.relationship_id
     AND reported.source_reference_kind = NEW.source_reference_kind
    WHERE native.set_id = NEW.set_id AND sets.source_element_kind = 'no_intro_dat_game'
      AND groups.kind = 'root'
      AND interpretation.format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                                     'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
      AND identity.origin = 'source' AND identity.snapshot_key = groups.snapshot_key
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications AS publication
                      WHERE publication.snapshot_key = groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT clone link requires an unused source identity on an unpublished game'); END;
CREATE TRIGGER no_intro_dat_set_links_native_immutable_update BEFORE UPDATE ON no_intro_dat_set_links
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT clone links are immutable'); END;
CREATE TRIGGER no_intro_dat_set_links_native_immutable_delete BEFORE DELETE ON no_intro_dat_set_links
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT clone links are immutable'); END;

CREATE TRIGGER no_intro_dat_categories_unpublished_owner_insert
BEFORE INSERT ON no_intro_dat_categories
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_dat_games AS native_game
    JOIN catalog_sets AS game ON game.set_id = native_game.set_id
    JOIN catalog_set_groups AS grouping ON grouping.set_group_id = game.set_group_id
    JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = grouping.snapshot_key
    JOIN parser_interpretations AS parser ON parser.interpretation_key = snapshot.interpretation_key
    WHERE native_game.set_id = NEW.set_id AND game.source_element_kind = 'no_intro_dat_game'
      AND grouping.kind = 'root'
      AND parser.format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                            'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications AS publication
                      WHERE publication.snapshot_key = grouping.snapshot_key)
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT category requires an unpublished native game'); END;

CREATE TRIGGER no_intro_dat_identifiers_unpublished_owner_insert
BEFORE INSERT ON no_intro_dat_identifiers
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_dat_games AS native_game
    JOIN catalog_sets AS game ON game.set_id = native_game.set_id
    JOIN catalog_set_groups AS grouping ON grouping.set_group_id = game.set_group_id
    JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = grouping.snapshot_key
    JOIN parser_interpretations AS parser ON parser.interpretation_key = snapshot.interpretation_key
    WHERE native_game.set_id = NEW.set_id AND game.source_element_kind = 'no_intro_dat_game'
      AND grouping.kind = 'root'
      AND parser.format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                            'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications AS publication
                      WHERE publication.snapshot_key = grouping.snapshot_key)
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT identifier requires an unpublished native game'); END;

CREATE TRIGGER no_intro_dat_releases_unpublished_owner_insert
BEFORE INSERT ON no_intro_dat_releases
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_dat_games AS native_game
    JOIN catalog_sets AS game ON game.set_id = native_game.set_id
    JOIN catalog_set_groups AS grouping ON grouping.set_group_id = game.set_group_id
    JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = grouping.snapshot_key
    JOIN parser_interpretations AS parser ON parser.interpretation_key = snapshot.interpretation_key
    WHERE native_game.set_id = NEW.set_id AND game.source_element_kind = 'no_intro_dat_game'
      AND grouping.kind = 'root'
      AND parser.format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                            'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications AS publication
                      WHERE publication.snapshot_key = grouping.snapshot_key)
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT release requires an unpublished native game'); END;

-- Linking requires agreement with applicable source evidence on every owner of
-- the UUID. Computed hashes and hashes of unknown scope do not contradict it.
CREATE VIEW no_intro_dat_source_file_size_assertions AS
SELECT size.occurrence_id, size.size
FROM accepted_file_size_assertions AS size
JOIN (
    SELECT occurrence_id, evidence_provenance FROM mame_rom_claims
    UNION ALL SELECT occurrence_id, evidence_provenance FROM logiqx_rom_claims
    UNION ALL SELECT occurrence_id, evidence_provenance FROM cmp_rom_claims
    UNION ALL SELECT occurrence_id, evidence_provenance FROM no_intro_pc_file_claims
    UNION ALL SELECT occurrence_id, evidence_provenance FROM no_intro_dat_rom_claims
) AS provenance ON provenance.occurrence_id = size.occurrence_id
WHERE provenance.evidence_provenance = 'source_declared';

-- Check disputes when a linked claim first supplies its source hash. A later
-- claim can dispute an earlier association in the same streaming import; that
-- does not erase the earlier source or make the document unpublishable. The
-- immutable conflict evidence quarantines the alias for subsequent resolution.
CREATE TRIGGER no_intro_dat_disputed_source_link_insert
BEFORE INSERT ON occurrence_digest_assertions
WHEN NEW.provenance = 'source_declared' AND NEW.scope IN ('whole_asset', 'whole_file')
  AND EXISTS (
      SELECT 1 FROM asset_occurrences
      WHERE occurrence_id = NEW.occurrence_id AND claim_kind = 'no_intro_dat_rom'
        AND content_uuid IS NOT NULL
  ) AND EXISTS (
      SELECT 1 FROM disputed_file_hashes WHERE digest_id = NEW.digest_id
  )
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT cannot link an already disputed source hash'); END;

CREATE TRIGGER no_intro_dat_linked_identity_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM no_intro_dat_rom_claims AS claim
    JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id = claim.occurrence_id
    JOIN canonical_occurrence_content AS canonical ON canonical.occurrence_id = occurrence.occurrence_id
    JOIN catalog_sets AS game ON game.set_id = occurrence.record_id
    JOIN catalog_set_groups AS grouping ON grouping.set_group_id = game.set_group_id
    WHERE grouping.snapshot_key = NEW.snapshot_key AND occurrence.content_uuid IS NOT NULL
      AND (
        EXISTS (
            WITH RECURSIVE component(content_uuid) AS (
                SELECT canonical.content_uuid
                UNION
                SELECT redirect.old_content_uuid FROM component
                JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid
                JOIN file_match_decision_publications USING (decision_id)
            )
            SELECT 1 FROM occurrence_digest_assertions AS incoming
            JOIN digest_values AS incoming_digest ON incoming_digest.digest_id = incoming.digest_id
            CROSS JOIN component
            CROSS JOIN asset_occurrences AS known_owner ON known_owner.content_uuid = component.content_uuid
            CROSS JOIN catalog_content_digest_assertions AS known ON known.occurrence_id = known_owner.occurrence_id
            CROSS JOIN digest_values AS known_digest ON known_digest.digest_id = known.digest_id
            WHERE incoming.occurrence_id = occurrence.occurrence_id
              AND incoming.provenance = 'source_declared' AND incoming.scope IN ('whole_asset', 'whole_file')
              AND incoming_digest.algorithm = known_digest.algorithm
              AND incoming_digest.digest <> known_digest.digest
        ) OR EXISTS (
            WITH RECURSIVE component(content_uuid) AS (
                SELECT canonical.content_uuid
                UNION
                SELECT redirect.old_content_uuid FROM component
                JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid
                JOIN file_match_decision_publications USING (decision_id)
            )
            SELECT 1 FROM component
            CROSS JOIN asset_occurrences AS known_owner ON known_owner.content_uuid = component.content_uuid
            CROSS JOIN no_intro_dat_source_file_size_assertions AS known_size ON known_size.occurrence_id = known_owner.occurrence_id
            WHERE claim.evidence_scope = 'whole_file' AND claim.size IS NOT NULL
              AND claim.evidence_provenance = 'source_declared'
              AND known_size.size <> claim.size
        )
      )
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT linked identity has contradictory source evidence'); END;

-- These views project XML sibling positions from their sole native owners.
CREATE VIEW no_intro_dat_root_child_source_positions AS
SELECT snapshot_key, source_order FROM no_intro_dat_headers
UNION ALL
SELECT grouping.snapshot_key, game.source_order
FROM no_intro_dat_games AS game
JOIN catalog_sets AS owner ON owner.set_id = game.set_id
JOIN catalog_set_groups AS grouping ON grouping.set_group_id = owner.set_group_id;

CREATE VIEW no_intro_dat_header_child_source_positions AS
SELECT snapshot_key, source_order FROM no_intro_dat_header_field_positions
UNION ALL SELECT snapshot_key, source_order FROM no_intro_dat_clrmamepro_options
UNION ALL SELECT snapshot_key, source_order FROM no_intro_dat_romcenter_options;

-- Family 0 is description, 1 category, 2 identifier, 3 ROM, and 4 release.
-- Attribute ordinals are a different XML domain and never enter this view.
-- Expose snapshot_key in every branch so the caller's snapshot predicate is
-- pushed into each indexed group lookup. CROSS JOIN keeps owner seeks before
-- child seeks rather than materializing collections from unrelated snapshots.
CREATE VIEW no_intro_dat_game_child_source_positions AS
SELECT grouping.snapshot_key, owner.set_id, 0 AS family_kind, 0 AS family_order, child.source_order
FROM catalog_set_groups AS grouping
CROSS JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
CROSS JOIN no_intro_dat_game_field_positions AS child ON child.set_id = owner.set_id
WHERE child.field_kind = 4
UNION ALL
SELECT grouping.snapshot_key, owner.set_id, 1, child.category_order, child.source_order
FROM catalog_set_groups AS grouping
CROSS JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
CROSS JOIN no_intro_dat_categories AS child ON child.set_id = owner.set_id
UNION ALL
SELECT grouping.snapshot_key, owner.set_id, 2, child.identifier_order, child.source_order
FROM catalog_set_groups AS grouping
CROSS JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
CROSS JOIN no_intro_dat_identifiers AS child ON child.set_id = owner.set_id
UNION ALL
SELECT grouping.snapshot_key, owner.set_id, 3, occurrence.occurrence_order, child.source_order
FROM catalog_set_groups AS grouping
CROSS JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = owner.set_id
CROSS JOIN no_intro_dat_rom_claims AS child ON child.occurrence_id = occurrence.occurrence_id
UNION ALL
SELECT grouping.snapshot_key, owner.set_id, 4, child.release_order, child.source_order
FROM catalog_set_groups AS grouping
CROSS JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
CROSS JOIN no_intro_dat_releases AS child ON child.set_id = owner.set_id;

CREATE TRIGGER no_intro_dat_source_positions_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM no_intro_dat_root_child_source_positions AS position
    WHERE position.snapshot_key = NEW.snapshot_key
    GROUP BY position.source_order HAVING COUNT(*) > 1
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_headers AS header
    JOIN catalog_set_groups AS grouping ON grouping.snapshot_key = header.snapshot_key
    JOIN catalog_sets AS owner ON owner.set_group_id = grouping.set_group_id
    JOIN no_intro_dat_games AS game ON game.set_id = owner.set_id
    WHERE header.snapshot_key = NEW.snapshot_key
      AND header.source_order >= game.source_order
) OR EXISTS (
    SELECT 1 FROM (
        SELECT game.source_order,
               lag(game.source_order) OVER (ORDER BY owner.list_order) AS previous_source_order
        FROM no_intro_dat_games AS game
        JOIN catalog_sets AS owner ON owner.set_id = game.set_id
        JOIN catalog_set_groups AS grouping ON grouping.set_group_id = owner.set_group_id
        WHERE grouping.snapshot_key = NEW.snapshot_key
    ) AS ordered_game
    WHERE ordered_game.source_order <= ordered_game.previous_source_order
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_header_child_source_positions AS position
    WHERE position.snapshot_key = NEW.snapshot_key
    GROUP BY position.source_order HAVING COUNT(*) > 1
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_game_child_source_positions AS position
    WHERE position.snapshot_key = NEW.snapshot_key
    GROUP BY position.set_id, position.source_order HAVING COUNT(*) > 1
) OR EXISTS (
    SELECT 1 FROM (
        SELECT position.source_order,
               lag(position.source_order) OVER (
                   PARTITION BY position.set_id, position.family_kind ORDER BY position.family_order
               ) AS previous_source_order
        FROM no_intro_dat_game_child_source_positions AS position
        WHERE position.snapshot_key = NEW.snapshot_key
    ) AS ordered_child
    WHERE ordered_child.source_order <= ordered_child.previous_source_order
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_releases AS release
    JOIN catalog_sets AS game ON game.set_id = release.set_id
    JOIN catalog_set_groups AS grouping ON grouping.set_group_id = game.set_group_id
    WHERE grouping.snapshot_key = NEW.snapshot_key
      AND release.name_source_order = release.region_source_order
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT source positions are inconsistent'); END;
