CREATE TRIGGER software_documents_native_owner_insert
BEFORE INSERT ON software_documents
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'mame-softwarelist-xml'
) OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'software document requires a new unpublished software-list snapshot'); END;
CREATE TRIGGER software_wrapper_headers_native_owner_insert
BEFORE INSERT ON software_wrapper_headers
WHEN NOT EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key = NEW.snapshot_key AND envelope_kind = 'plural_lists')
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM software_wrapper_headers WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'software wrapper header requires an unpublished software document'); END;

CREATE TRIGGER software_parts_native_owner_insert BEFORE INSERT ON software_parts
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key) JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.record_id AND source_element_kind = 'software_item'
      AND kind = 'software_list' AND format = 'mame-softwarelist-xml'
) OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
             JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_parts WHERE part_id = NEW.part_id
            OR (record_id = NEW.record_id AND part_order = NEW.part_order)
            OR (record_id = NEW.record_id AND source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software part requires a new unpublished software item'); END;
CREATE TRIGGER software_areas_native_owner_insert BEFORE INSERT ON software_areas
WHEN NOT EXISTS (SELECT 1 FROM software_parts WHERE part_id = NEW.part_id AND record_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_areas WHERE area_id = NEW.area_id
            OR (part_id = NEW.part_id AND area_order = NEW.area_order))
BEGIN SELECT RAISE(ABORT, 'software area requires a new unpublished software part'); END;
CREATE TRIGGER software_data_areas_native_owner_insert BEFORE INSERT ON software_data_areas
WHEN NOT EXISTS (SELECT 1 FROM software_areas WHERE area_id = NEW.area_id AND area_kind = 'data')
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = (
            SELECT snapshot_key FROM catalog_set_groups WHERE set_group_id = (
                SELECT set_group_id FROM catalog_sets WHERE set_id = (
                    SELECT record_id FROM software_areas WHERE area_id = NEW.area_id))))
 OR EXISTS (SELECT 1 FROM software_data_areas WHERE area_id = NEW.area_id)
 OR EXISTS (SELECT 1 FROM software_disk_areas WHERE area_id = NEW.area_id)
 OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_data_areas AS detail USING (area_id)
            WHERE area.part_id = (SELECT part_id FROM software_areas WHERE area_id = NEW.area_id)
              AND detail.source_order = NEW.source_order)
 OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_disk_areas AS detail USING (area_id)
            WHERE area.part_id = (SELECT part_id FROM software_areas WHERE area_id = NEW.area_id)
              AND detail.source_order = NEW.source_order)
BEGIN SELECT RAISE(ABORT, 'data area details require a new unpublished data area and unique source order'); END;
CREATE TRIGGER software_disk_areas_native_owner_insert BEFORE INSERT ON software_disk_areas
WHEN NOT EXISTS (SELECT 1 FROM software_areas WHERE area_id = NEW.area_id AND area_kind = 'disk')
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = (
            SELECT snapshot_key FROM catalog_set_groups WHERE set_group_id = (
                SELECT set_group_id FROM catalog_sets WHERE set_id = (
                    SELECT record_id FROM software_areas WHERE area_id = NEW.area_id))))
 OR EXISTS (SELECT 1 FROM software_disk_areas WHERE area_id = NEW.area_id)
 OR EXISTS (SELECT 1 FROM software_data_areas WHERE area_id = NEW.area_id)
 OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_data_areas AS detail USING (area_id)
            WHERE area.part_id = (SELECT part_id FROM software_areas WHERE area_id = NEW.area_id)
              AND detail.source_order = NEW.source_order)
 OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_disk_areas AS detail USING (area_id)
            WHERE area.part_id = (SELECT part_id FROM software_areas WHERE area_id = NEW.area_id)
              AND detail.source_order = NEW.source_order)
BEGIN SELECT RAISE(ABORT, 'disk area details require a new unpublished disk area and unique source order'); END;
CREATE TRIGGER software_rom_entries_native_owner_insert BEFORE INSERT ON software_rom_entries
WHEN NOT EXISTS (SELECT 1 FROM software_areas WHERE area_id = NEW.area_id AND record_id = NEW.record_id
                AND area_kind = 'data')
 OR NOT EXISTS (SELECT 1 FROM software_data_areas WHERE area_id = NEW.area_id)
 OR NOT EXISTS (SELECT 1 FROM asset_occurrences WHERE occurrence_id = NEW.occurrence_id
                AND record_id = NEW.record_id AND claim_kind IN ('software_rom_entry', 'software_rom_operation'))
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_rom_entries WHERE occurrence_id = NEW.occurrence_id
            OR (area_id = NEW.area_id AND component_order = NEW.component_order)
            OR (area_id = NEW.area_id AND source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software ROM requires a new unpublished software area and occurrence'); END;
CREATE TRIGGER software_disk_entries_native_owner_insert BEFORE INSERT ON software_disk_entries
WHEN NOT EXISTS (SELECT 1 FROM software_areas WHERE area_id = NEW.area_id AND record_id = NEW.record_id
                AND area_kind = 'disk')
 OR NOT EXISTS (SELECT 1 FROM software_disk_areas WHERE area_id = NEW.area_id)
 OR NOT EXISTS (SELECT 1 FROM asset_occurrences WHERE occurrence_id = NEW.occurrence_id
                AND record_id = NEW.record_id AND claim_kind = 'software_disk_entry')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_disk_entries WHERE occurrence_id = NEW.occurrence_id
            OR (area_id = NEW.area_id AND component_order = NEW.component_order)
            OR (area_id = NEW.area_id AND source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software disk requires a new unpublished disk area and occurrence'); END;
CREATE TRIGGER software_file_declarations_native_owner_insert BEFORE INSERT ON software_file_declarations
WHEN NOT EXISTS (SELECT 1 FROM software_rom_entries AS rom
                 JOIN asset_occurrences AS occurrence USING (occurrence_id, record_id)
                 WHERE rom.occurrence_id = NEW.occurrence_id AND rom.record_id = NEW.record_id
                   AND occurrence.claim_kind = 'software_rom_entry')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_file_declarations WHERE occurrence_id = NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'software file declaration requires a source ROM declaration'); END;
CREATE TRIGGER software_file_uses_native_owner_insert BEFORE INSERT ON software_file_uses
WHEN NOT (
    (NEW.operation <> 'disk' AND EXISTS (
        SELECT 1 FROM software_rom_entries AS rom
        JOIN asset_occurrences AS occurrence USING (occurrence_id, record_id)
        WHERE rom.occurrence_id = NEW.occurrence_id AND rom.record_id = NEW.record_id
          AND occurrence.claim_kind IN ('software_rom_entry', 'software_rom_operation')
          AND (NEW.declaration_occurrence_id IS NULL OR EXISTS (
              SELECT 1 FROM software_rom_entries AS declaration_rom
              JOIN software_file_declarations AS declaration USING (occurrence_id, record_id)
              WHERE declaration.occurrence_id = NEW.declaration_occurrence_id
                AND declaration.record_id = NEW.record_id
                AND declaration_rom.area_id = rom.area_id
          ))
    )) OR (NEW.operation = 'disk' AND NEW.declaration_occurrence_id IS NULL AND EXISTS (
        SELECT 1 FROM software_disk_entries AS disk
        JOIN asset_occurrences AS occurrence USING (occurrence_id, record_id)
        WHERE disk.occurrence_id = NEW.occurrence_id AND disk.record_id = NEW.record_id
          AND occurrence.claim_kind = 'software_disk_entry'
    ))
)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_file_uses WHERE occurrence_id = NEW.occurrence_id
            OR (occurrence_id = NEW.occurrence_id AND record_id = NEW.record_id))
BEGIN SELECT RAISE(ABORT, 'software file use requires a source ROM'); END;

CREATE TRIGGER software_item_info_native_owner_insert BEFORE INSERT ON software_item_info
WHEN NOT EXISTS (SELECT 1 FROM software_items WHERE record_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_item_info WHERE record_id = NEW.record_id
            AND (value_order = NEW.value_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software item info requires a new unpublished software item'); END;
CREATE TRIGGER software_item_shared_features_native_owner_insert BEFORE INSERT ON software_item_shared_features
WHEN NOT EXISTS (SELECT 1 FROM software_items WHERE record_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_item_shared_features WHERE record_id = NEW.record_id
            AND (value_order = NEW.value_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software shared feature requires a new unpublished software item'); END;
CREATE TRIGGER software_item_text_positions_native_owner_insert BEFORE INSERT ON software_item_text_positions
WHEN NOT EXISTS (SELECT 1 FROM software_items WHERE record_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
 OR EXISTS (SELECT 1 FROM software_item_text_positions WHERE record_id = NEW.record_id
            AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software item text position requires an unpublished item'); END;
CREATE TRIGGER software_list_text_positions_native_owner_insert BEFORE INSERT ON software_list_text_positions
WHEN NOT EXISTS (SELECT 1 FROM software_lists WHERE namespace_id = NEW.namespace_id)
 OR EXISTS (SELECT 1 FROM catalog_set_groups JOIN snapshot_publications USING (snapshot_key)
            WHERE set_group_id = NEW.namespace_id)
 OR EXISTS (SELECT 1 FROM software_list_text_positions WHERE namespace_id = NEW.namespace_id
            AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software list text position requires an unpublished software list'); END;
CREATE TRIGGER software_part_features_native_owner_insert BEFORE INSERT ON software_part_features
WHEN NOT EXISTS (SELECT 1 FROM software_parts WHERE part_id = NEW.part_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key)
            JOIN software_parts AS parts ON parts.record_id = catalog_sets.set_id
            WHERE parts.part_id = NEW.part_id)
 OR EXISTS (SELECT 1 FROM software_part_features WHERE part_id = NEW.part_id
            AND (value_order = NEW.value_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software part feature requires an unpublished part'); END;
CREATE TRIGGER software_part_dipswitches_native_owner_insert BEFORE INSERT ON software_part_dipswitches
WHEN NOT EXISTS (SELECT 1 FROM software_parts WHERE part_id = NEW.part_id)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key)
            JOIN software_parts AS parts ON parts.record_id = catalog_sets.set_id
            WHERE parts.part_id = NEW.part_id)
 OR EXISTS (SELECT 1 FROM software_part_dipswitches WHERE part_id = NEW.part_id
            AND (dipswitch_order = NEW.dipswitch_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software DIP switch requires an unpublished part'); END;
CREATE TRIGGER software_part_dip_values_native_owner_insert BEFORE INSERT ON software_part_dip_values
WHEN NOT EXISTS (SELECT 1 FROM software_part_dipswitches WHERE part_id = NEW.part_id
                 AND dipswitch_order = NEW.dipswitch_order)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key)
            JOIN software_parts AS parts ON parts.record_id = catalog_sets.set_id
            WHERE parts.part_id = NEW.part_id)
 OR EXISTS (SELECT 1 FROM software_part_dip_values WHERE part_id = NEW.part_id
            AND dipswitch_order = NEW.dipswitch_order
            AND (value_order = NEW.value_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'software DIP value requires an unpublished DIP switch'); END;

CREATE TRIGGER software_native_publication_guard BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'mame-softwarelist-xml'
) AND (
    NOT EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key = NEW.snapshot_key)
    OR EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key = NEW.snapshot_key
               AND envelope_kind = 'plural_lists'
               AND NOT EXISTS (SELECT 1 FROM software_wrapper_headers WHERE snapshot_key = NEW.snapshot_key))
    OR EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key = NEW.snapshot_key
               AND envelope_kind = 'single_list'
               AND EXISTS (SELECT 1 FROM software_wrapper_headers WHERE snapshot_key = NEW.snapshot_key))
    OR EXISTS (
        SELECT 1 FROM software_documents AS document
        WHERE document.snapshot_key = NEW.snapshot_key AND document.envelope_kind = 'single_list'
          AND (SELECT COUNT(*) FROM catalog_set_groups
               WHERE snapshot_key = NEW.snapshot_key AND kind = 'software_list') <> 1
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        LEFT JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (lists.namespace_id IS NULL OR NOT EXISTS (
              SELECT 1 FROM catalog_sets AS sets WHERE sets.set_group_id = groups.set_group_id
                AND sets.source_element_kind = 'software_item'
          ))
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
        LEFT JOIN software_items AS items ON items.record_id = sets.set_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND sets.source_element_kind = 'software_item' AND items.record_id IS NULL
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
        JOIN software_items AS items ON items.record_id = sets.set_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND sets.source_element_kind = 'software_item'
          AND (
              NOT EXISTS (SELECT 1 FROM software_item_text_positions AS position
                          WHERE position.record_id = items.record_id AND position.field_kind = 0)
              OR NOT EXISTS (SELECT 1 FROM software_item_text_positions AS position
                             WHERE position.record_id = items.record_id AND position.field_kind = 1)
              OR NOT EXISTS (SELECT 1 FROM software_item_text_positions AS position
                             WHERE position.record_id = items.record_id AND position.field_kind = 2)
              OR ((items.notes IS NOT NULL) != EXISTS (
                  SELECT 1 FROM software_item_text_positions AS position
                  WHERE position.record_id = items.record_id AND position.field_kind = 3
              ))
          )
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND ((lists.notes IS NOT NULL) != EXISTS (
              SELECT 1 FROM software_list_text_positions AS position
              WHERE position.namespace_id = lists.namespace_id AND position.field_kind = 3
          ))
    )
    OR EXISTS (
        SELECT 1 FROM software_parts AS parts
        JOIN catalog_sets AS sets ON sets.set_id = parts.record_id
        JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (sets.source_element_kind <> 'software_item'
               OR NOT EXISTS (SELECT 1 FROM software_items AS items WHERE items.record_id = sets.set_id))
    )
    OR EXISTS (
        SELECT 1 FROM software_areas AS areas
        JOIN software_parts AS parts ON parts.part_id = areas.part_id
        JOIN catalog_sets AS sets ON sets.set_id = parts.record_id
        JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
        LEFT JOIN software_data_areas AS data_area ON data_area.area_id = areas.area_id
        LEFT JOIN software_disk_areas AS disk_area ON disk_area.area_id = areas.area_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (areas.record_id <> sets.set_id
               OR (areas.area_kind = 'data' AND (data_area.area_id IS NULL OR disk_area.area_id IS NOT NULL))
               OR (areas.area_kind = 'disk' AND (disk_area.area_id IS NULL OR data_area.area_id IS NOT NULL)))
    )
    OR EXISTS (
        SELECT 1 FROM software_rom_entries AS rom
        JOIN catalog_sets AS sets ON sets.set_id = rom.record_id
        JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
        LEFT JOIN software_areas AS areas ON areas.area_id = rom.area_id
        LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id = rom.occurrence_id
        LEFT JOIN software_file_uses AS file_use ON file_use.occurrence_id = rom.occurrence_id
             AND file_use.record_id = rom.record_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (sets.source_element_kind <> 'software_item'
               OR areas.area_id IS NULL OR areas.area_kind <> 'data' OR areas.record_id <> rom.record_id
               OR occurrence.occurrence_id IS NULL OR occurrence.record_id <> rom.record_id
               OR occurrence.claim_kind NOT IN ('software_rom_entry', 'software_rom_operation')
               OR file_use.occurrence_id IS NULL OR file_use.operation = 'disk'
               OR file_use.operation <> CASE
                   WHEN rom.load_instruction IN ('continue', 'reload', 'reload_plain', 'ignore', 'fill')
                   THEN rom.load_instruction ELSE 'load' END
               OR occurrence.claim_kind <> CASE
                   WHEN rom.load_instruction IN ('continue', 'reload', 'reload_plain', 'ignore', 'fill')
                   THEN 'software_rom_operation' ELSE 'software_rom_entry' END
               OR (occurrence.claim_kind = 'software_rom_operation' AND (
                   file_use.declaration_occurrence_id = rom.occurrence_id
                   OR (rom.load_instruction = 'fill' AND file_use.declaration_occurrence_id IS NOT NULL)
               ))
               OR (occurrence.claim_kind = 'software_rom_entry' AND (
                   NOT EXISTS (SELECT 1 FROM software_file_declarations AS declaration
                               WHERE declaration.occurrence_id = rom.occurrence_id
                                 AND declaration.record_id = rom.record_id)
                   OR file_use.declaration_occurrence_id IS NULL
                   OR file_use.declaration_occurrence_id <> rom.occurrence_id
               ))
               OR (file_use.declaration_occurrence_id IS NOT NULL AND NOT EXISTS (
                   SELECT 1 FROM software_file_declarations AS declaration
                   JOIN software_rom_entries AS declaration_rom
                     ON declaration_rom.occurrence_id = declaration.occurrence_id
                    AND declaration_rom.record_id = declaration.record_id
                   WHERE declaration.occurrence_id = file_use.declaration_occurrence_id
                     AND declaration.record_id = rom.record_id
                     AND declaration_rom.area_id = rom.area_id
               )))
    )
    OR EXISTS (
        SELECT 1 FROM software_disk_entries AS disk
        JOIN catalog_sets AS sets ON sets.set_id = disk.record_id
        JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
        LEFT JOIN software_areas AS areas ON areas.area_id = disk.area_id
        LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id = disk.occurrence_id
        LEFT JOIN software_file_uses AS file_use ON file_use.occurrence_id = disk.occurrence_id
             AND file_use.record_id = disk.record_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (sets.source_element_kind <> 'software_item'
               OR areas.area_id IS NULL OR areas.area_kind <> 'disk' OR areas.record_id <> disk.record_id
               OR occurrence.occurrence_id IS NULL OR occurrence.record_id <> disk.record_id
               OR occurrence.claim_kind <> 'software_disk_entry'
               OR file_use.occurrence_id IS NULL OR file_use.operation <> 'disk'
               OR file_use.declaration_occurrence_id IS NOT NULL)
    )
    OR EXISTS (
        SELECT 1 FROM software_file_declarations AS declaration
        JOIN catalog_sets AS sets ON sets.set_id = declaration.record_id
        JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
        LEFT JOIN software_rom_entries AS rom ON rom.occurrence_id = declaration.occurrence_id
             AND rom.record_id = declaration.record_id
        LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id = declaration.occurrence_id
             AND occurrence.record_id = declaration.record_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND (sets.source_element_kind <> 'software_item' OR rom.occurrence_id IS NULL
               OR occurrence.claim_kind <> 'software_rom_entry')
    )
    OR EXISTS (SELECT 1 FROM software_items
               WHERE record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (supported IS NULL OR (supported_specified = 0 AND supported <> 'yes')))
    OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_data_areas AS detail USING (area_id)
               WHERE area.record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (width IS NULL OR (width_specified = 0 AND width <> 8)))
    OR EXISTS (SELECT 1 FROM software_areas AS area JOIN software_data_areas AS detail USING (area_id)
               WHERE area.record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (endianness IS NULL OR (endianness_specified = 0 AND endianness <> 'little')))
    OR EXISTS (SELECT 1 FROM software_rom_entries
               WHERE record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (dump_status IS NULL OR (status_specified = 0 AND dump_status <> 'good')))
    OR EXISTS (SELECT 1 FROM software_disk_entries
               WHERE record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (dump_status IS NULL OR (status_specified = 0 AND dump_status <> 'good')))
    OR EXISTS (SELECT 1 FROM software_disk_entries
               WHERE record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
                                   WHERE snapshot_key = NEW.snapshot_key)
                 AND (writeable IS NULL OR (writeable_specified = 0 AND writeable <> 0)))
    OR EXISTS (SELECT 1 FROM software_part_dip_values
               WHERE part_id IN (
                   SELECT parts.part_id FROM software_parts AS parts
                   JOIN catalog_sets AS sets ON sets.set_id = parts.record_id
                   JOIN catalog_set_groups AS groups USING (set_group_id)
                   WHERE groups.snapshot_key = NEW.snapshot_key
               )
                 AND (is_default IS NULL OR (default_specified = 0 AND is_default <> 0)))
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
        CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id
        LEFT JOIN software_rom_entries AS rom USING (occurrence_id, record_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND occurrence.content_uuid IS NOT NULL AND (
            occurrence.claim_kind <> 'software_rom_entry'
            OR rom.name IS NULL OR rom.name = '' OR rom.dump_status = 'nodump'
            OR rom.evidence_scope NOT IN ('whole_file', 'whole_asset')
            OR NOT COALESCE(length(rom.sha1_text) = 40
                            AND rom.sha1_text NOT GLOB '*[^0-9a-fA-F]*', 0)
            OR (rom.crc_text IS NOT NULL AND NOT (
                length(rom.crc_text) = 8 AND rom.crc_text NOT GLOB '*[^0-9a-fA-F]*'
            ))
            OR NOT EXISTS (
                SELECT 1 FROM software_file_declarations AS declaration
                WHERE declaration.occurrence_id = rom.occurrence_id
                  AND declaration.record_id = rom.record_id
            )
        )
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
        CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id
        JOIN software_rom_entries AS rom USING (occurrence_id, record_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND CASE WHEN occurrence.claim_kind = 'software_rom_entry'
                        AND rom.name IS NOT NULL AND rom.name <> '' AND rom.dump_status <> 'nodump'
                   THEN rom.evidence_scope NOT IN ('whole_file', 'whole_asset')
                   ELSE rom.evidence_scope <> 'unknown' END
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups
        JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
        GROUP BY lists.source_order HAVING COUNT(*) > 1
    )
    OR EXISTS (
        SELECT 1 FROM (
            SELECT sets.set_group_id AS owner_id, items.source_order
            FROM catalog_sets AS sets
            JOIN catalog_set_groups AS groups USING (set_group_id)
            JOIN software_items AS items ON items.record_id = sets.set_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
            UNION ALL
            SELECT positions.namespace_id, positions.source_order
            FROM software_list_text_positions AS positions
            JOIN catalog_set_groups AS groups ON groups.set_group_id = positions.namespace_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
        ) AS children
        GROUP BY children.owner_id, children.source_order HAVING COUNT(*) > 1
    )
    OR EXISTS (
        SELECT 1 FROM (
            SELECT sets.set_id AS owner_id, parts.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
              AND sets.source_element_kind = 'software_item'
            UNION ALL
            SELECT sets.set_id, info.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_item_info AS info ON info.record_id = sets.set_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
              AND sets.source_element_kind = 'software_item'
            UNION ALL
            SELECT sets.set_id, features.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_item_shared_features AS features ON features.record_id = sets.set_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
              AND sets.source_element_kind = 'software_item'
            UNION ALL
            SELECT sets.set_id, positions.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_item_text_positions AS positions ON positions.record_id = sets.set_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
              AND sets.source_element_kind = 'software_item'
        ) AS children
        GROUP BY children.owner_id, children.source_order HAVING COUNT(*) > 1
    )
    OR EXISTS (
        SELECT 1 FROM (
            SELECT parts.part_id AS owner_id, features.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_part_features AS features ON features.part_id = parts.part_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
            UNION ALL
            SELECT parts.part_id, switches.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_part_dipswitches AS switches ON switches.part_id = parts.part_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
            UNION ALL
            SELECT parts.part_id, data_area.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_areas AS areas ON areas.part_id = parts.part_id
            JOIN software_data_areas AS data_area ON data_area.area_id = areas.area_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
            UNION ALL
            SELECT parts.part_id, disk_area.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_areas AS areas ON areas.part_id = parts.part_id
            JOIN software_disk_areas AS disk_area ON disk_area.area_id = areas.area_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
        ) AS children
        GROUP BY children.owner_id, children.source_order HAVING COUNT(*) > 1
    )
    OR EXISTS (
        SELECT 1 FROM (
            SELECT areas.area_id AS owner_id, rom.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_areas AS areas ON areas.part_id = parts.part_id
            JOIN software_rom_entries AS rom ON rom.area_id = areas.area_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
            UNION ALL
            SELECT areas.area_id, disk.source_order
            FROM catalog_set_groups AS groups
            JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
            JOIN software_parts AS parts ON parts.record_id = sets.set_id
            JOIN software_areas AS areas ON areas.part_id = parts.part_id
            JOIN software_disk_entries AS disk ON disk.area_id = areas.area_id
            WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
        ) AS children
        GROUP BY children.owner_id, children.source_order HAVING COUNT(*) > 1
    )
    OR EXISTS (
        SELECT 1 FROM software_rom_entries AS rom
        JOIN catalog_sets AS sets ON sets.set_id = rom.record_id
        JOIN catalog_set_groups AS groups USING (set_group_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND COALESCE(length(rom.crc_text) = 8 AND rom.crc_text NOT GLOB '*[^0-9a-fA-F]*', 0)
          AND NOT EXISTS (
              SELECT 1 FROM occurrence_digest_assertions AS assertion
              JOIN digest_values AS digest USING (digest_id)
              WHERE assertion.occurrence_id = rom.occurrence_id
                AND assertion.scope = rom.evidence_scope
                AND assertion.provenance = 'source_declared'
                AND digest.algorithm = 'crc32'
                AND lower(hex(digest.digest)) = lower(rom.crc_text)
          )
    )
    OR EXISTS (
        SELECT 1 FROM software_rom_entries AS rom
        JOIN catalog_sets AS sets ON sets.set_id = rom.record_id
        JOIN catalog_set_groups AS groups USING (set_group_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND COALESCE(length(rom.sha1_text) = 40 AND rom.sha1_text NOT GLOB '*[^0-9a-fA-F]*', 0)
          AND NOT EXISTS (
              SELECT 1 FROM occurrence_digest_assertions AS assertion
              JOIN digest_values AS digest USING (digest_id)
              WHERE assertion.occurrence_id = rom.occurrence_id
                AND assertion.scope = rom.evidence_scope
                AND assertion.provenance = 'source_declared'
                AND digest.algorithm = 'sha1'
                AND lower(hex(digest.digest)) = lower(rom.sha1_text)
          )
    )
    OR EXISTS (
        SELECT 1 FROM software_disk_entries AS disk
        JOIN catalog_sets AS sets ON sets.set_id = disk.record_id
        JOIN catalog_set_groups AS groups USING (set_group_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND COALESCE(length(disk.sha1_text) = 40 AND disk.sha1_text NOT GLOB '*[^0-9a-fA-F]*', 0)
          AND NOT EXISTS (
              SELECT 1 FROM occurrence_digest_assertions AS assertion
              JOIN digest_values AS digest USING (digest_id)
              WHERE assertion.occurrence_id = disk.occurrence_id
                AND assertion.scope = disk.evidence_scope
                AND assertion.provenance = 'source_declared'
                AND digest.algorithm = 'sha1'
                AND lower(hex(digest.digest)) = lower(disk.sha1_text)
          )
    )
    OR EXISTS (
        SELECT 1 FROM occurrence_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        JOIN asset_occurrences AS occurrence USING (occurrence_id)
        JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
        JOIN catalog_set_groups AS groups USING (set_group_id)
        WHERE groups.snapshot_key = NEW.snapshot_key AND groups.kind = 'software_list'
          AND assertion.provenance = 'source_declared'
          AND NOT (
              EXISTS (
                  SELECT 1 FROM software_rom_entries AS rom
                  WHERE rom.occurrence_id = occurrence.occurrence_id
                    AND rom.record_id = occurrence.record_id
                    AND assertion.scope = rom.evidence_scope
                    AND ((digest.algorithm = 'crc32'
                          AND COALESCE(length(rom.crc_text) = 8
                                       AND rom.crc_text NOT GLOB '*[^0-9a-fA-F]*', 0)
                          AND lower(hex(digest.digest)) = lower(rom.crc_text))
                      OR (digest.algorithm = 'sha1'
                          AND COALESCE(length(rom.sha1_text) = 40
                                       AND rom.sha1_text NOT GLOB '*[^0-9a-fA-F]*', 0)
                          AND lower(hex(digest.digest)) = lower(rom.sha1_text)))
              )
              OR EXISTS (
                  SELECT 1 FROM software_disk_entries AS disk
                  WHERE disk.occurrence_id = occurrence.occurrence_id
                    AND disk.record_id = occurrence.record_id
                    AND assertion.scope = disk.evidence_scope
                    AND digest.algorithm = 'sha1'
                    AND COALESCE(length(disk.sha1_text) = 40
                                 AND disk.sha1_text NOT GLOB '*[^0-9a-fA-F]*', 0)
                    AND lower(hex(digest.digest)) = lower(disk.sha1_text)
              )
          )
    )
)
BEGIN SELECT RAISE(ABORT, 'software list snapshot is incomplete or source hashes disagree with native assertions'); END;

CREATE TRIGGER software_documents_immutable_update BEFORE UPDATE ON software_documents
BEGIN SELECT RAISE(ABORT, 'native software documents are immutable'); END;
CREATE TRIGGER software_documents_immutable_delete BEFORE DELETE ON software_documents
BEGIN SELECT RAISE(ABORT, 'native software documents are immutable'); END;
CREATE TRIGGER software_wrapper_headers_immutable_update BEFORE UPDATE ON software_wrapper_headers
BEGIN SELECT RAISE(ABORT, 'native software headers are immutable'); END;
CREATE TRIGGER software_wrapper_headers_immutable_delete BEFORE DELETE ON software_wrapper_headers
BEGIN SELECT RAISE(ABORT, 'native software headers are immutable'); END;
CREATE TRIGGER software_parts_immutable_update BEFORE UPDATE ON software_parts
BEGIN SELECT RAISE(ABORT, 'native software parts are immutable'); END;
CREATE TRIGGER software_parts_immutable_delete BEFORE DELETE ON software_parts
BEGIN SELECT RAISE(ABORT, 'native software parts are immutable'); END;
CREATE TRIGGER software_areas_immutable_update BEFORE UPDATE ON software_areas
BEGIN SELECT RAISE(ABORT, 'native software areas are immutable'); END;
CREATE TRIGGER software_areas_immutable_delete BEFORE DELETE ON software_areas
BEGIN SELECT RAISE(ABORT, 'native software areas are immutable'); END;
CREATE TRIGGER software_data_areas_immutable_update BEFORE UPDATE ON software_data_areas
BEGIN SELECT RAISE(ABORT, 'native software data areas are immutable'); END;
CREATE TRIGGER software_data_areas_immutable_delete BEFORE DELETE ON software_data_areas
BEGIN SELECT RAISE(ABORT, 'native software data areas are immutable'); END;
CREATE TRIGGER software_disk_areas_immutable_update BEFORE UPDATE ON software_disk_areas
BEGIN SELECT RAISE(ABORT, 'native software disk areas are immutable'); END;
CREATE TRIGGER software_disk_areas_immutable_delete BEFORE DELETE ON software_disk_areas
BEGIN SELECT RAISE(ABORT, 'native software disk areas are immutable'); END;
CREATE TRIGGER software_disk_entries_immutable_update BEFORE UPDATE ON software_disk_entries
BEGIN SELECT RAISE(ABORT, 'native software disk entries are immutable'); END;
CREATE TRIGGER software_disk_entries_immutable_delete BEFORE DELETE ON software_disk_entries
BEGIN SELECT RAISE(ABORT, 'native software disk entries are immutable'); END;
CREATE TRIGGER software_file_declarations_immutable_update BEFORE UPDATE ON software_file_declarations
BEGIN SELECT RAISE(ABORT, 'native software file declarations are immutable'); END;
CREATE TRIGGER software_file_declarations_immutable_delete BEFORE DELETE ON software_file_declarations
BEGIN SELECT RAISE(ABORT, 'native software file declarations are immutable'); END;
CREATE TRIGGER software_file_uses_immutable_update BEFORE UPDATE ON software_file_uses
BEGIN SELECT RAISE(ABORT, 'native software file uses are immutable'); END;
CREATE TRIGGER software_file_uses_immutable_delete BEFORE DELETE ON software_file_uses
BEGIN SELECT RAISE(ABORT, 'native software file uses are immutable'); END;
CREATE TRIGGER software_item_info_immutable_update BEFORE UPDATE ON software_item_info
BEGIN SELECT RAISE(ABORT, 'native software item info are immutable'); END;
CREATE TRIGGER software_item_info_immutable_delete BEFORE DELETE ON software_item_info
BEGIN SELECT RAISE(ABORT, 'native software item info are immutable'); END;
CREATE TRIGGER software_item_shared_features_immutable_update BEFORE UPDATE ON software_item_shared_features
BEGIN SELECT RAISE(ABORT, 'native software item shared features are immutable'); END;
CREATE TRIGGER software_item_shared_features_immutable_delete BEFORE DELETE ON software_item_shared_features
BEGIN SELECT RAISE(ABORT, 'native software item shared features are immutable'); END;
CREATE TRIGGER software_item_text_positions_immutable_update BEFORE UPDATE ON software_item_text_positions
BEGIN SELECT RAISE(ABORT, 'native software item text positions are immutable'); END;
CREATE TRIGGER software_item_text_positions_immutable_delete BEFORE DELETE ON software_item_text_positions
BEGIN SELECT RAISE(ABORT, 'native software item text positions are immutable'); END;
CREATE TRIGGER software_list_text_positions_immutable_update BEFORE UPDATE ON software_list_text_positions
BEGIN SELECT RAISE(ABORT, 'native software list text positions are immutable'); END;
CREATE TRIGGER software_list_text_positions_immutable_delete BEFORE DELETE ON software_list_text_positions
BEGIN SELECT RAISE(ABORT, 'native software list text positions are immutable'); END;
CREATE TRIGGER software_part_features_immutable_update BEFORE UPDATE ON software_part_features
BEGIN SELECT RAISE(ABORT, 'native software part features are immutable'); END;
CREATE TRIGGER software_part_features_immutable_delete BEFORE DELETE ON software_part_features
BEGIN SELECT RAISE(ABORT, 'native software part features are immutable'); END;
CREATE TRIGGER software_part_dipswitches_immutable_update BEFORE UPDATE ON software_part_dipswitches
BEGIN SELECT RAISE(ABORT, 'native software part dipswitches are immutable'); END;
CREATE TRIGGER software_part_dipswitches_immutable_delete BEFORE DELETE ON software_part_dipswitches
BEGIN SELECT RAISE(ABORT, 'native software part dipswitches are immutable'); END;
CREATE TRIGGER software_part_dip_values_immutable_update BEFORE UPDATE ON software_part_dip_values
BEGIN SELECT RAISE(ABORT, 'native software part dip values are immutable'); END;
CREATE TRIGGER software_part_dip_values_immutable_delete BEFORE DELETE ON software_part_dip_values
BEGIN SELECT RAISE(ABORT, 'native software part dip values are immutable'); END;
CREATE TRIGGER software_rom_entries_immutable_update BEFORE UPDATE ON software_rom_entries
BEGIN SELECT RAISE(ABORT, 'native software rom entries are immutable'); END;
CREATE TRIGGER software_rom_entries_immutable_delete BEFORE DELETE ON software_rom_entries
BEGIN SELECT RAISE(ABORT, 'native software rom entries are immutable'); END;
CREATE TRIGGER software_items_reject_replace BEFORE INSERT ON software_items
WHEN EXISTS (SELECT 1 FROM software_items WHERE record_id = NEW.record_id)
BEGIN SELECT RAISE(ABORT, 'native software items are immutable'); END;
CREATE TRIGGER software_lists_reject_replace BEFORE INSERT ON software_lists
WHEN EXISTS (SELECT 1 FROM software_lists WHERE namespace_id = NEW.namespace_id)
BEGIN SELECT RAISE(ABORT, 'native software lists are immutable'); END;
