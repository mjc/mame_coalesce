"""Candidate SQL derivation for a provisional MAME software-list run length.

This candidate fragment is assembled after the native software tables and
software-number projections. The view stores no derived values and performs a
chain-local recursive seek for each requested load row. A non-NULL result is
not parser-EOF evidence, complete edition closure, file qualification, or
publication eligibility; those remain separate gates.
"""

def sql() -> str:
    """Return DDL for ``candidate_software_file_lengths``.

    The query-only view has exactly ``(media_entry_id, byte_length)``. It emits
    one row per ordinary ROM load; unknown or malformed sizes/owner links yield
    NULL. A base load and each continue require a positive checked integer;
    ignore accepts zero and contributes zero bytes. Reload, reload_plain, the
    next load, fill, or the end of the data area closes the first run. This
    local derivation is not a qualification or whole-edition closure claim.
    """
    root_size = "root_numbers.byte_length"
    next_size = "next_numbers.byte_length"
    next_length_valid = f"""(
        CASE next_operation.operation WHEN 'continue'
             THEN {next_size} > 0
             WHEN 'ignore' THEN {next_size} >= 0
             ELSE 0 END
    )"""
    next_step_valid = """(
        next_media.media_entry_id IS NOT NULL
        AND next_element.element_kind = 'software_rom_entry'
        AND next_step.load_entry_id IS NOT NULL
    )"""
    same_file_step = f"""(
        {next_step_valid}
        AND next_step.required_file_id = walk.required_file_id
        AND next_requirement.required_file_id IS NULL
        AND linked_requirement.first_load_entry_id = walk.root_media_entry_id
    )"""

    return f"""
CREATE INDEX IF NOT EXISTS candidate_software_file_steps_by_required_file
    ON software_file_load_steps(required_file_id, load_entry_id);

CREATE VIEW candidate_software_file_lengths AS
SELECT root.media_entry_id,
       (
           WITH RECURSIVE walk(
               root_media_entry_id, data_area_id, edition_id, required_file_id,
               source_order, byte_length, state
           ) AS (
               SELECT root.media_entry_id, root.data_area_id, groups.edition_id,
                      requirement.required_file_id, root.source_order,
                      {root_size}, 'walking'
               FROM catalog_media_entries AS media
               JOIN catalog_source_elements AS element
                 ON element.source_element_id = media.media_entry_id
                AND element.element_kind = 'software_rom_entry'
               JOIN software_areas AS area
                 ON area.area_id = root.data_area_id AND area.kind = 'data'
               JOIN software_data_areas AS data ON data.area_id = area.area_id
               JOIN software_parts AS part ON part.part_id = area.part_id
               JOIN software_titles AS title ON title.set_id = part.set_id
               JOIN catalog_sets AS sets ON sets.set_id = title.set_id
               JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id
               JOIN software_lists AS lists ON lists.set_group_id = groups.set_group_id
               JOIN software_required_files AS requirement
                 ON requirement.first_load_entry_id = root.media_entry_id
               JOIN software_file_load_steps AS root_step
                 ON root_step.load_entry_id = root.media_entry_id
                AND root_step.required_file_id = requirement.required_file_id
               JOIN candidate_software_rom_numbers AS root_numbers
                 ON root_numbers.media_entry_id = root.media_entry_id
               WHERE media.media_entry_id = root.media_entry_id
                 AND groups.group_kind = 'software_list'
                 AND element.edition_id = groups.edition_id
                 AND {root_size} > 0
                 AND NOT EXISTS (
                     SELECT 1
                     FROM software_file_load_steps AS elsewhere_step
                     JOIN software_rom_load_entries AS elsewhere
                       ON elsewhere.media_entry_id = elsewhere_step.load_entry_id
                     WHERE elsewhere_step.required_file_id = requirement.required_file_id
                       AND elsewhere.data_area_id <> root.data_area_id
                 )

               UNION ALL

               SELECT walk.root_media_entry_id, walk.data_area_id, walk.edition_id,
                      walk.required_file_id, next_rom.source_order,
                      CASE
                          WHEN next_rom.source_order = walk.source_order + 1
                           AND next_operation.operation IN ('continue', 'ignore')
                           AND {same_file_step}
                           AND {next_length_valid}
                           AND {next_size} <= 9223372036854775807 - walk.byte_length
                          THEN walk.byte_length + {next_size}
                          ELSE walk.byte_length
                      END,
                      CASE
                          WHEN next_rom.source_order <> walk.source_order + 1
                          THEN 'invalid'
                          WHEN next_operation.operation IN ('continue', 'ignore') THEN
                              CASE WHEN {same_file_step}
                                         AND {next_length_valid}
                                         AND {next_size} <= 9223372036854775807 - walk.byte_length
                                   THEN 'walking' ELSE 'invalid' END
                          WHEN next_operation.operation IN ('reload', 'reload_plain') THEN
                              CASE WHEN {next_step_valid}
                                         AND next_step.required_file_id = walk.required_file_id
                                         AND next_requirement.required_file_id IS NULL
                                         AND linked_requirement.first_load_entry_id = walk.root_media_entry_id
                                   THEN 'complete' ELSE 'invalid' END
                          WHEN next_operation.operation = 'load' THEN
                              CASE WHEN {next_step_valid}
                                         AND next_step.required_file_id = next_requirement.required_file_id
                                         AND next_requirement.first_load_entry_id = next_rom.media_entry_id
                                   THEN 'complete' ELSE 'invalid' END
                          WHEN next_operation.operation = 'fill' THEN
                              CASE WHEN {next_step_valid}
                                         AND next_step.required_file_id IS NULL
                                         AND next_requirement.required_file_id IS NULL
                                   THEN 'complete' ELSE 'invalid' END
                          ELSE 'invalid'
                      END
               FROM walk
               JOIN software_rom_load_entries AS next_rom
                 ON next_rom.data_area_id = walk.data_area_id
               AND next_rom.source_order = (
                    SELECT min(later.source_order)
                    FROM software_rom_load_entries AS later
                    WHERE later.data_area_id = walk.data_area_id
                      AND later.source_order > walk.source_order
                )
               JOIN candidate_software_rom_operations AS next_operation
                 ON next_operation.media_entry_id = next_rom.media_entry_id
               JOIN candidate_software_rom_numbers AS next_numbers
                 ON next_numbers.media_entry_id = next_rom.media_entry_id
               LEFT JOIN catalog_media_entries AS next_media
                 ON next_media.media_entry_id = next_rom.media_entry_id
               LEFT JOIN catalog_source_elements AS next_element
                 ON next_element.source_element_id = next_media.media_entry_id
               LEFT JOIN software_file_load_steps AS next_step
                 ON next_step.load_entry_id = next_rom.media_entry_id
               LEFT JOIN software_required_files AS next_requirement
                 ON next_requirement.first_load_entry_id = next_rom.media_entry_id
               LEFT JOIN software_required_files AS linked_requirement
                 ON linked_requirement.required_file_id = next_step.required_file_id
               WHERE walk.state = 'walking'
                 AND (next_element.edition_id IS NULL OR next_element.edition_id = walk.edition_id)
           )
           SELECT CASE
                      WHEN walk.state = 'complete' THEN walk.byte_length
                      WHEN walk.state = 'walking'
                       AND NOT EXISTS (
                           SELECT 1 FROM software_rom_load_entries AS later
                           WHERE later.data_area_id = walk.data_area_id
                             AND later.source_order > walk.source_order
                       ) THEN walk.byte_length
                      ELSE NULL
                  END
           FROM walk
           ORDER BY walk.source_order DESC
           LIMIT 1
       ) AS byte_length
FROM software_rom_load_entries AS root
JOIN candidate_software_rom_operations AS root_operation
  ON root_operation.media_entry_id = root.media_entry_id
WHERE root_operation.operation = 'load';
""".strip()
