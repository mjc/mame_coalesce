-- Complete first verification runs, not segment lengths or maximum reload runs.
-- Correlation preserves primary-key/area-order seeks for selected occurrences.
-- Invalid successors cannot turn a valid prefix into complete-file evidence.
CREATE VIEW software_file_size_assertions AS
SELECT declaration.occurrence_id, 'software_rom_file_size' AS size_field,
    (WITH RECURSIVE run(component_order, size) AS (
        SELECT base.component_order, base.size
        WHERE typeof(base.component_order) = 'integer'
          AND typeof(base.size) = 'integer' AND base.size > 0
        UNION ALL
        SELECT next.component_order, run.size + next.size
        FROM run
        CROSS JOIN software_rom_entries AS next
          ON next.area_id = base.area_id
         AND run.component_order < 9223372036854775807
         AND next.component_order = run.component_order + 1
        CROSS JOIN asset_occurrences AS occurrence
          ON occurrence.occurrence_id = next.occurrence_id
         AND occurrence.record_id = next.record_id
         AND occurrence.claim_kind = 'software_rom_operation'
        CROSS JOIN software_file_uses AS file_use
          ON file_use.occurrence_id = next.occurrence_id
         AND file_use.record_id = base.record_id
         AND file_use.declaration_occurrence_id = base.occurrence_id
         AND file_use.operation = next.load_instruction
        WHERE next.record_id = base.record_id
          AND typeof(next.component_order) = 'integer'
          AND next.load_instruction IN ('continue', 'ignore')
          AND typeof(next.size) = 'integer'
          AND next.size >= CASE next.load_instruction WHEN 'continue' THEN 1 ELSE 0 END
          AND next.size <= 9223372036854775807 - run.size
    ) SELECT run.size FROM run
      WHERE NOT EXISTS (
          SELECT 1 FROM software_rom_entries AS next
          WHERE next.area_id = base.area_id
            AND run.component_order < 9223372036854775807
            AND next.component_order = run.component_order + 1
            AND next.load_instruction IN ('continue', 'ignore')
      ) AND (
          EXISTS (SELECT 1 FROM software_rom_entries AS next
                  WHERE next.area_id = base.area_id
                    AND run.component_order < 9223372036854775807
                    AND next.component_order = run.component_order + 1)
          OR NOT EXISTS (SELECT 1 FROM software_rom_entries AS later
                         WHERE later.area_id = base.area_id
                           AND later.component_order > run.component_order)
      )
    ) AS size
FROM software_file_declarations AS declaration
JOIN software_rom_entries AS base
  ON base.occurrence_id = declaration.occurrence_id AND base.record_id = declaration.record_id
JOIN asset_occurrences AS occurrence
  ON occurrence.occurrence_id = base.occurrence_id AND occurrence.record_id = base.record_id
JOIN software_file_uses AS file_use
  ON file_use.occurrence_id = base.occurrence_id AND file_use.record_id = base.record_id
 AND file_use.declaration_occurrence_id = base.occurrence_id AND file_use.operation = 'load'
JOIN software_areas AS area ON area.area_id = base.area_id AND area.record_id = base.record_id
JOIN software_data_areas AS detail ON detail.area_id = area.area_id
JOIN software_parts AS part ON part.part_id = area.part_id AND part.record_id = base.record_id
JOIN software_items AS item ON item.record_id = part.record_id
JOIN catalog_sets AS sets ON sets.set_id = item.record_id
JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = groups.snapshot_key
JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key = snapshot.interpretation_key
WHERE occurrence.claim_kind = 'software_rom_entry'
  AND area.area_kind = 'data'
  AND NOT EXISTS (SELECT 1 FROM software_disk_areas WHERE area_id = area.area_id)
  AND groups.kind = 'software_list' AND sets.source_element_kind = 'software_item'
  AND interpretation.format = 'mame-softwarelist-xml'
  AND base.name IS NOT NULL AND base.name <> '' AND base.dump_status <> 'nodump'
  AND base.evidence_scope IN ('whole_asset', 'whole_file')
  AND (base.load_instruction IS NULL OR base.load_instruction NOT IN ('continue', 'ignore', 'reload', 'reload_plain', 'fill'));

CREATE TRIGGER software_linked_file_size_publication BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups AS groups
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
    CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id
    JOIN canonical_occurrence_content AS canonical USING (occurrence_id)
    JOIN source_file_size_assertions AS incoming USING (occurrence_id)
    WHERE groups.snapshot_key = NEW.snapshot_key
      AND occurrence.claim_kind = 'software_rom_entry'
      AND EXISTS (
          WITH RECURSIVE component(content_uuid) AS (
              SELECT canonical.content_uuid
              UNION
              SELECT redirect.old_content_uuid FROM component
              JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = component.content_uuid
              JOIN file_match_decision_publications USING (decision_id)
          )
          SELECT 1 FROM component
          CROSS JOIN asset_occurrences AS known ON known.content_uuid = component.content_uuid
          CROSS JOIN source_file_size_assertions AS size ON size.occurrence_id = known.occurrence_id
          WHERE size.size <> incoming.size
      )
)
BEGIN SELECT RAISE(ABORT, 'software UUID has contradictory source whole-file lengths'); END;
