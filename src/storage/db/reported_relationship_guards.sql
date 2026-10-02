-- Qualified owners support publication/readiness/explanations; raw IDs below seal collisions.
-- Typed NULL occurrence IDs keep compound-view affinity consistent so requested-ID
-- predicates push into every native branch instead of materializing the catalog.
CREATE VIEW reported_catalog_relationship_owners AS
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER) AS occurrence_id,groups.snapshot_key
FROM mame_machine_links AS link CROSS JOIN mame_machines AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id) CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='mame_machine' AND interpretation.format='mame-listxml'
UNION ALL
SELECT reference.relationship_id,reference.source_reference_kind,reference.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM mame_device_references AS reference CROSS JOIN mame_machines AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id) CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='mame_machine' AND interpretation.format='mame-listxml'
UNION ALL
SELECT merge_owners.relationship_id,merge_owners.source_reference_kind,merge_owners.set_id,merge_owners.occurrence_id,merge_owners.snapshot_key
FROM mame_merge_relationship_owners AS merge_owners CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE interpretation.format='mame-listxml'
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM logiqx_set_links AS link CROSS JOIN logiqx_games AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id) CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='logiqx_game' AND interpretation.format='logiqx'
UNION ALL
SELECT reference.relationship_id,reference.source_reference_kind,reference.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM logiqx_device_references AS reference CROSS JOIN logiqx_games AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id) CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='logiqx_game' AND interpretation.format='logiqx'
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,occurrence.record_id,
       declaration.occurrence_id,groups.snapshot_key
FROM logiqx_file_merges AS declaration CROSS JOIN asset_occurrences AS occurrence USING(occurrence_id)
CROSS JOIN logiqx_games AS native ON native.set_id=occurrence.record_id
CROSS JOIN logiqx_rom_claims AS payload ON payload.occurrence_id=declaration.occurrence_id
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.set_id CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE declaration.claim_kind='logiqx_rom' AND occurrence.claim_kind='logiqx_rom'
  AND sets.source_element_kind='logiqx_game' AND interpretation.format='logiqx'
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,occurrence.record_id,
       declaration.occurrence_id,groups.snapshot_key
FROM logiqx_file_merges AS declaration CROSS JOIN asset_occurrences AS occurrence USING(occurrence_id)
CROSS JOIN logiqx_games AS native ON native.set_id=occurrence.record_id
CROSS JOIN logiqx_disk_claims AS payload ON payload.occurrence_id=declaration.occurrence_id
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.set_id CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE declaration.claim_kind='logiqx_disk' AND occurrence.claim_kind='logiqx_disk'
  AND sets.source_element_kind='logiqx_game' AND interpretation.format='logiqx'
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM clrmamepro_set_links AS link INDEXED BY clrmamepro_set_links_relationship_id_index
CROSS JOIN cmp_set_facts AS native ON native.record_id=link.set_id
CROSS JOIN cmp_set_field_positions AS position ON position.record_id=link.set_id
 AND position.field_kind=CASE link.link_kind WHEN 'cloneof' THEN 1 WHEN 'sampleof' THEN 6 END
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.record_id CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='cmp_set' AND interpretation.format='clrmamepro-dat'
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,occurrence.record_id,
       declaration.occurrence_id,groups.snapshot_key
FROM clrmamepro_rom_merges AS declaration
CROSS JOIN asset_occurrences AS occurrence USING(occurrence_id)
CROSS JOIN cmp_rom_claims AS payload USING(occurrence_id)
CROSS JOIN cmp_rom_field_positions AS position ON position.occurrence_id=declaration.occurrence_id
 AND position.field_kind=6
CROSS JOIN cmp_set_facts AS native ON native.record_id=occurrence.record_id
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.record_id CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE occurrence.claim_kind='cmp_rom' AND sets.source_element_kind='cmp_set'
  AND interpretation.format='clrmamepro-dat'
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM software_clone_links AS link
CROSS JOIN software_items AS native ON native.record_id=link.set_id
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.record_id
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN software_lists AS list ON list.namespace_id=groups.set_group_id
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='software_item' AND groups.kind='software_list'
  AND interpretation.format='mame-softwarelist-xml'
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM no_intro_dat_set_links AS link
CROSS JOIN no_intro_dat_games AS native USING(set_id)
CROSS JOIN no_intro_dat_game_field_positions AS position
  ON position.set_id=link.set_id
 AND position.field_kind=CASE link.link_kind WHEN 'cloneof' THEN 2 ELSE 3 END
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.set_id
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE sets.source_element_kind='no_intro_dat_game' AND groups.kind='root'
  AND interpretation.format IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible',
                                 'no-intro-dat-v4-strict','no-intro-dat-v4-compatible')
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,archive.set_id,
       CAST(NULL AS INTEGER),groups.snapshot_key
FROM no_intro_archive_clone_links AS link
CROSS JOIN no_intro_archive_descriptions AS archive USING(archive_id)
CROSS JOIN no_intro_database_games AS native ON native.set_id=archive.set_id
CROSS JOIN no_intro_archive_field_positions AS position
  ON position.archive_id=link.archive_id AND position.field_kind=30
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.set_id
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN no_intro_exports AS export ON export.snapshot_key=groups.snapshot_key
CROSS JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
CROSS JOIN parser_interpretations AS interpretation
  ON interpretation.interpretation_key=snapshot.interpretation_key
WHERE sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
  AND interpretation.format IN ('no-intro-database-xml-compatible',
                                 'no-intro-database-xml-nul-compatible')
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,archive.set_id,
       CAST(NULL AS INTEGER),groups.snapshot_key
FROM no_intro_archive_merge_links AS link
CROSS JOIN no_intro_archive_descriptions AS archive USING(archive_id)
CROSS JOIN no_intro_database_games AS native ON native.set_id=archive.set_id
CROSS JOIN no_intro_archive_field_positions AS position
  ON position.archive_id=link.archive_id AND position.field_kind=31
CROSS JOIN catalog_sets AS sets ON sets.set_id=native.set_id
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN no_intro_exports AS export ON export.snapshot_key=groups.snapshot_key
CROSS JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
CROSS JOIN parser_interpretations AS interpretation
  ON interpretation.interpretation_key=snapshot.interpretation_key
WHERE sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
  AND interpretation.format IN ('no-intro-database-xml-compatible',
                                 'no-intro-database-xml-nul-compatible')
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM no_intro_pc_clone_links AS link
CROSS JOIN no_intro_pc_games AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id)
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
JOIN catalog_relationships AS identity
  ON identity.relationship_id=link.relationship_id AND identity.origin='source'
 AND identity.snapshot_key=groups.snapshot_key
WHERE sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
  AND interpretation.format='no-intro-pc-xml'
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,link.set_id,CAST(NULL AS INTEGER),groups.snapshot_key
FROM no_intro_pc_merge_links AS link
CROSS JOIN no_intro_pc_games AS native USING(set_id)
CROSS JOIN catalog_sets AS sets USING(set_id)
CROSS JOIN catalog_set_groups AS groups USING(set_group_id)
CROSS JOIN catalog_snapshots AS snapshot USING(snapshot_key)
CROSS JOIN parser_interpretations AS interpretation USING(interpretation_key)
JOIN catalog_relationships AS identity
  ON identity.relationship_id=link.relationship_id AND identity.origin='source'
 AND identity.snapshot_key=groups.snapshot_key
WHERE sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
  AND interpretation.format='no-intro-pc-xml';

CREATE VIEW reported_catalog_relationship_owner_ids AS
SELECT relationship_id FROM mame_machine_links
UNION ALL SELECT relationship_id FROM mame_device_references
UNION ALL SELECT relationship_id FROM mame_rom_merges
UNION ALL SELECT relationship_id FROM mame_disk_merges
UNION ALL SELECT relationship_id FROM logiqx_set_links
UNION ALL SELECT relationship_id FROM logiqx_device_references
UNION ALL SELECT relationship_id FROM logiqx_file_merges
UNION ALL SELECT relationship_id FROM clrmamepro_set_links
UNION ALL SELECT relationship_id FROM clrmamepro_rom_merges
UNION ALL SELECT relationship_id FROM software_clone_links
UNION ALL SELECT relationship_id FROM no_intro_dat_set_links
UNION ALL SELECT relationship_id FROM no_intro_archive_clone_links
UNION ALL SELECT relationship_id FROM no_intro_archive_merge_links
UNION ALL SELECT relationship_id FROM no_intro_pc_clone_links
UNION ALL SELECT relationship_id FROM no_intro_pc_merge_links;

CREATE VIEW reported_catalog_relationship_raw_owners AS
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN mame_machine_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT reference.relationship_id,reference.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN mame_device_references AS reference
WHERE sets.set_group_id=groups.set_group_id AND reference.set_id=sets.set_id
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN mame_rom_merges AS declaration
WHERE sets.set_group_id=groups.set_group_id AND occurrence.record_id=sets.set_id
  AND declaration.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN mame_disk_merges AS declaration
WHERE sets.set_group_id=groups.set_group_id AND occurrence.record_id=sets.set_id
  AND declaration.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN logiqx_set_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT reference.relationship_id,reference.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN logiqx_device_references AS reference
WHERE sets.set_group_id=groups.set_group_id AND reference.set_id=sets.set_id
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN logiqx_file_merges AS declaration
WHERE sets.set_group_id=groups.set_group_id AND occurrence.record_id=sets.set_id
  AND declaration.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN clrmamepro_set_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT declaration.relationship_id,declaration.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN clrmamepro_rom_merges AS declaration
WHERE sets.set_group_id=groups.set_group_id AND occurrence.record_id=sets.set_id
  AND declaration.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN software_clone_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN no_intro_dat_set_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN no_intro_archive_descriptions AS archive
CROSS JOIN no_intro_archive_clone_links AS link USING(archive_id)
WHERE sets.set_group_id=groups.set_group_id AND archive.set_id=sets.set_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN no_intro_archive_descriptions AS archive
CROSS JOIN no_intro_archive_merge_links AS link USING(archive_id)
WHERE sets.set_group_id=groups.set_group_id AND archive.set_id=sets.set_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN no_intro_pc_clone_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id
UNION ALL
SELECT link.relationship_id,link.source_reference_kind,groups.snapshot_key
FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
CROSS JOIN no_intro_pc_merge_links AS link
WHERE sets.set_group_id=groups.set_group_id AND link.set_id=sets.set_id;

-- Source identities and their native declarations close in both directions at publication.
CREATE TRIGGER reported_relationships_require_native_owner_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    LEFT JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE identity.origin='source' AND identity.snapshot_key=NEW.snapshot_key
      AND (reported.relationship_id IS NULL
           OR (SELECT COUNT(*) FROM reported_catalog_relationship_owner_ids AS physical
               WHERE physical.relationship_id=identity.relationship_id)<>1
           OR (SELECT COUNT(*) FROM reported_catalog_relationship_owners AS owner
           WHERE owner.relationship_id=identity.relationship_id
             AND owner.source_reference_kind=reported.source_reference_kind
             AND owner.snapshot_key=NEW.snapshot_key)<>1)
) OR EXISTS (
    SELECT 1 FROM reported_catalog_relationship_raw_owners AS owner
    LEFT JOIN catalog_relationships AS identity
      ON identity.relationship_id=owner.relationship_id AND identity.origin='source'
    LEFT JOIN reported_catalog_relationships AS reported
      ON reported.relationship_id=owner.relationship_id
     AND reported.source_reference_kind=owner.source_reference_kind
    WHERE owner.snapshot_key=NEW.snapshot_key
      AND (identity.relationship_id IS NULL OR identity.snapshot_key IS NOT owner.snapshot_key
           OR reported.relationship_id IS NULL
           OR (SELECT COUNT(*) FROM reported_catalog_relationship_owners AS qualified
               WHERE qualified.relationship_id=owner.relationship_id
                 AND qualified.source_reference_kind=owner.source_reference_kind
                 AND qualified.snapshot_key=NEW.snapshot_key)<>1
           OR (SELECT COUNT(*) FROM reported_catalog_relationship_owner_ids AS duplicate
               WHERE duplicate.relationship_id=owner.relationship_id)<>1)
)
BEGIN SELECT RAISE(ABORT,'reported source relationships require complete native identity ownership'); END;

CREATE TRIGGER logiqx_set_links_insert_guard
BEFORE INSERT ON logiqx_set_links
WHEN EXISTS (SELECT 1 FROM logiqx_set_links WHERE (set_id,link_kind)=(NEW.set_id,NEW.link_kind)
             OR relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM logiqx_games AS native
    JOIN catalog_sets AS sets ON sets.set_id=native.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE native.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='logiqx_'||NEW.link_kind
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'Logiqx link requires an unused source identity and unpublished native owner'); END;
CREATE TRIGGER logiqx_set_links_immutable_update BEFORE UPDATE ON logiqx_set_links
BEGIN SELECT RAISE(ABORT,'Logiqx links are immutable'); END;
CREATE TRIGGER logiqx_set_links_immutable_delete BEFORE DELETE ON logiqx_set_links
BEGIN SELECT RAISE(ABORT,'Logiqx links are immutable'); END;

CREATE TRIGGER logiqx_device_references_insert_guard
BEFORE INSERT ON logiqx_device_references
WHEN EXISTS (SELECT 1 FROM logiqx_device_references
             WHERE (set_id,reference_order)=(NEW.set_id,NEW.reference_order)
                OR relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM logiqx_games AS native
    JOIN catalog_sets AS sets ON sets.set_id=native.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE native.set_id=NEW.set_id AND sets.source_element_kind='logiqx_game'
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='logiqx_device_ref'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'Logiqx device reference requires an unused source identity and unpublished native owner'); END;
CREATE TRIGGER logiqx_device_references_immutable_update BEFORE UPDATE ON logiqx_device_references
BEGIN SELECT RAISE(ABORT,'Logiqx device references are immutable'); END;
CREATE TRIGGER logiqx_device_references_immutable_delete BEFORE DELETE ON logiqx_device_references
BEGIN SELECT RAISE(ABORT,'Logiqx device references are immutable'); END;

CREATE TRIGGER clrmamepro_set_links_insert_guard
BEFORE INSERT ON clrmamepro_set_links
WHEN EXISTS (SELECT 1 FROM clrmamepro_set_links
             WHERE (set_id,link_kind)=(NEW.set_id,NEW.link_kind)
                OR relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM cmp_set_facts AS native
    JOIN catalog_sets AS sets ON sets.set_id=native.record_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS interpretation USING(interpretation_key)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE native.record_id=NEW.set_id AND sets.source_element_kind='cmp_set'
      AND interpretation.format='clrmamepro-dat'
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='clrmamepro_'||NEW.link_kind
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER clrmamepro_set_links_immutable_update BEFORE UPDATE ON clrmamepro_set_links
BEGIN SELECT RAISE(ABORT,'CMP native set links are immutable'); END;
CREATE TRIGGER clrmamepro_set_links_immutable_delete BEFORE DELETE ON clrmamepro_set_links
BEGIN SELECT RAISE(ABORT,'CMP native set links are immutable'); END;

CREATE TRIGGER logiqx_file_merges_insert_guard
BEFORE INSERT ON logiqx_file_merges
WHEN EXISTS (SELECT 1 FROM logiqx_file_merges
             WHERE occurrence_id=NEW.occurrence_id OR relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence
    JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
    JOIN logiqx_games AS native USING(set_id)
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE occurrence.occurrence_id=NEW.occurrence_id
      AND occurrence.claim_kind=NEW.claim_kind
      AND ((NEW.claim_kind='logiqx_rom' AND EXISTS
              (SELECT 1 FROM logiqx_rom_claims WHERE occurrence_id=NEW.occurrence_id))
        OR (NEW.claim_kind='logiqx_disk' AND EXISTS
              (SELECT 1 FROM logiqx_disk_claims WHERE occurrence_id=NEW.occurrence_id)))
      AND sets.source_element_kind='logiqx_game'
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind=NEW.source_reference_kind
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'Logiqx merge requires an unused source identity and matching unpublished media owner'); END;
CREATE TRIGGER logiqx_file_merges_immutable_update BEFORE UPDATE ON logiqx_file_merges
BEGIN SELECT RAISE(ABORT,'Logiqx merges are immutable'); END;
CREATE TRIGGER logiqx_file_merges_immutable_delete BEFORE DELETE ON logiqx_file_merges
BEGIN SELECT RAISE(ABORT,'Logiqx merges are immutable'); END;

CREATE TRIGGER clrmamepro_rom_merges_insert_guard
BEFORE INSERT ON clrmamepro_rom_merges
WHEN EXISTS (SELECT 1 FROM clrmamepro_rom_merges
             WHERE occurrence_id=NEW.occurrence_id OR relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM cmp_rom_claims AS native
    JOIN asset_occurrences AS occurrence USING(occurrence_id)
    JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE native.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='cmp_rom'
      AND sets.source_element_kind='cmp_set'
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='clrmamepro_rom_merge'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'CMP ROM merge requires an unused source identity and matching unpublished media owner'); END;
CREATE TRIGGER clrmamepro_rom_merges_immutable_update BEFORE UPDATE ON clrmamepro_rom_merges
BEGIN SELECT RAISE(ABORT,'CMP ROM merges are immutable'); END;
CREATE TRIGGER clrmamepro_rom_merges_immutable_delete BEFORE DELETE ON clrmamepro_rom_merges
BEGIN SELECT RAISE(ABORT,'CMP ROM merges are immutable'); END;
