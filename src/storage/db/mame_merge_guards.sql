-- Only declarations live here; referenced names do not create resolved targets.
CREATE VIEW mame_merge_relationship_owners AS
SELECT declaration.relationship_id, declaration.source_reference_kind,
       declaration.occurrence_id, occurrence.record_id AS set_id, owner_group.snapshot_key
FROM mame_rom_merges AS declaration
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN mame_rom_claims AS payload
CROSS JOIN mame_machines AS machine CROSS JOIN catalog_sets AS owner
CROSS JOIN catalog_set_groups AS owner_group
WHERE occurrence.occurrence_id=declaration.occurrence_id AND occurrence.claim_kind='mame_rom'
  AND payload.occurrence_id=occurrence.occurrence_id AND payload.claim_kind='mame_rom'
  AND machine.set_id=occurrence.record_id AND owner.set_id=machine.set_id
  AND owner.source_element_kind='mame_machine' AND owner_group.set_group_id=owner.set_group_id
UNION ALL
SELECT declaration.relationship_id, declaration.source_reference_kind,
       declaration.occurrence_id, occurrence.record_id, owner_group.snapshot_key
FROM mame_disk_merges AS declaration
CROSS JOIN asset_occurrences AS occurrence CROSS JOIN mame_disk_claims AS payload
CROSS JOIN mame_machines AS machine CROSS JOIN catalog_sets AS owner
CROSS JOIN catalog_set_groups AS owner_group
WHERE occurrence.occurrence_id=declaration.occurrence_id AND occurrence.claim_kind='mame_disk'
  AND payload.occurrence_id=occurrence.occurrence_id AND payload.claim_kind='mame_disk'
  AND machine.set_id=occurrence.record_id AND owner.set_id=machine.set_id
  AND owner.source_element_kind='mame_machine' AND owner_group.set_group_id=owner.set_group_id;

CREATE TRIGGER mame_rom_merges_insert_guard BEFORE INSERT ON mame_rom_merges
WHEN EXISTS (SELECT 1 FROM mame_rom_merges WHERE occurrence_id=NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM mame_relationship_declaration_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM mame_rom_claims AS payload
    JOIN asset_occurrences AS occurrence USING(occurrence_id)
    JOIN mame_machines AS machine ON machine.set_id=occurrence.record_id
    JOIN catalog_sets AS owner ON owner.set_id=machine.set_id
    JOIN catalog_set_groups AS owner_group USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE payload.occurrence_id=NEW.occurrence_id AND payload.claim_kind='mame_rom'
      AND occurrence.claim_kind='mame_rom' AND owner.source_element_kind='mame_machine'
      AND identity.origin='source' AND identity.snapshot_key=owner_group.snapshot_key
      AND reported.source_reference_kind='mame_rom_merge'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=owner_group.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'MAME ROM merge requires an unused source identity and unpublished native owner in the same snapshot'); END;

CREATE TRIGGER mame_disk_merges_insert_guard BEFORE INSERT ON mame_disk_merges
WHEN EXISTS (SELECT 1 FROM mame_disk_merges WHERE occurrence_id=NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM mame_relationship_declaration_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM mame_disk_claims AS payload
    JOIN asset_occurrences AS occurrence USING(occurrence_id)
    JOIN mame_machines AS machine ON machine.set_id=occurrence.record_id
    JOIN catalog_sets AS owner ON owner.set_id=machine.set_id
    JOIN catalog_set_groups AS owner_group USING(set_group_id)
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE payload.occurrence_id=NEW.occurrence_id AND payload.claim_kind='mame_disk'
      AND occurrence.claim_kind='mame_disk' AND owner.source_element_kind='mame_machine'
      AND identity.origin='source' AND identity.snapshot_key=owner_group.snapshot_key
      AND reported.source_reference_kind='mame_disk_merge'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=owner_group.snapshot_key)
 )
BEGIN SELECT RAISE(ABORT,'MAME disk merge requires an unused source identity and unpublished native owner in the same snapshot'); END;

CREATE TRIGGER mame_rom_merges_immutable_update BEFORE UPDATE ON mame_rom_merges
BEGIN SELECT RAISE(ABORT,'MAME ROM merges are immutable'); END;
CREATE TRIGGER mame_rom_merges_immutable_delete BEFORE DELETE ON mame_rom_merges
BEGIN SELECT RAISE(ABORT,'MAME ROM merges are immutable'); END;
CREATE TRIGGER mame_disk_merges_immutable_update BEFORE UPDATE ON mame_disk_merges
BEGIN SELECT RAISE(ABORT,'MAME disk merges are immutable'); END;
CREATE TRIGGER mame_disk_merges_immutable_delete BEFORE DELETE ON mame_disk_merges
BEGIN SELECT RAISE(ABORT,'MAME disk merges are immutable'); END;

CREATE TRIGGER mame_merge_owners_publication_guard BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups AS owner_group
    CROSS JOIN catalog_sets AS owner CROSS JOIN asset_occurrences AS occurrence
    CROSS JOIN mame_rom_merges AS declaration
    WHERE owner_group.snapshot_key=NEW.snapshot_key AND owner.set_group_id=owner_group.set_group_id
      AND occurrence.record_id=owner.set_id AND declaration.occurrence_id=occurrence.occurrence_id
      AND NOT EXISTS (
          SELECT 1 FROM catalog_relationships AS identity
          JOIN reported_catalog_relationships AS reported USING(relationship_id)
          JOIN mame_merge_relationship_owners AS native USING(relationship_id)
          WHERE identity.relationship_id=declaration.relationship_id AND identity.origin='source'
            AND identity.snapshot_key=NEW.snapshot_key AND native.snapshot_key=NEW.snapshot_key
            AND native.occurrence_id=declaration.occurrence_id
            AND reported.source_reference_kind='mame_rom_merge'
            AND native.source_reference_kind=reported.source_reference_kind
      )
 ) OR EXISTS (
    SELECT 1 FROM catalog_set_groups AS owner_group
    CROSS JOIN catalog_sets AS owner CROSS JOIN asset_occurrences AS occurrence
    CROSS JOIN mame_disk_merges AS declaration
    WHERE owner_group.snapshot_key=NEW.snapshot_key AND owner.set_group_id=owner_group.set_group_id
      AND occurrence.record_id=owner.set_id AND declaration.occurrence_id=occurrence.occurrence_id
      AND NOT EXISTS (
          SELECT 1 FROM catalog_relationships AS identity
          JOIN reported_catalog_relationships AS reported USING(relationship_id)
          JOIN mame_merge_relationship_owners AS native USING(relationship_id)
          WHERE identity.relationship_id=declaration.relationship_id AND identity.origin='source'
            AND identity.snapshot_key=NEW.snapshot_key AND native.snapshot_key=NEW.snapshot_key
            AND native.occurrence_id=declaration.occurrence_id
            AND reported.source_reference_kind='mame_disk_merge'
            AND native.source_reference_kind=reported.source_reference_kind
      )
 )
BEGIN SELECT RAISE(ABORT,'MAME merge identities and native owners must close within the published snapshot'); END;
