"""Read-only native file byte and hash qualification projections."""


def sql() -> str:
    """Return point-queryable qualification views for the six supported roles.

    A contract row is affirmative provenance supplied by the producer. In
    particular, Logiqx is never assigned a contract from its tag or filename.
    Hash declarations are consumed through their canonical native position;
    no source values or ancestry are copied into these views. A single media
    key drives the closed native-role CASE: a role UNION can become a global
    coroutine when reached through a UUID-filtered evidence view.
    """
    return """
CREATE VIEW candidate_native_file_roles AS
SELECT media.media_entry_id, element.edition_id, rules.format_family,
       CASE element.element_kind
           WHEN 'mame_rom' THEN 'mame_0289_machine_rom'
           WHEN 'software_rom_entry' THEN 'mame_0289_software_file'
           WHEN 'logiqx_rom' THEN 'logiqx_complete_declared_file'
           WHEN 'clrmamepro_rom' THEN 'clrmamepro_declared_asset'
           WHEN 'no_intro_dat_rom' THEN 'no_intro_dat_unfiltered_file'
           WHEN 'no_intro_pc_rom' THEN 'no_intro_pc_fixture_asset'
       END AS contract_kind,
       CASE element.element_kind WHEN 'software_rom_entry' THEN 'file_length'
            ELSE 'size' END AS source_size_field
FROM catalog_media_entries AS media
CROSS JOIN catalog_source_elements AS element ON element.source_element_id=media.media_entry_id
CROSS JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id
CROSS JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
WHERE CASE element.element_kind
WHEN 'mame_rom' THEN rules.format_family='mame' AND EXISTS (
    SELECT 1 FROM mame_roms AS rom
    JOIN mame_machines AS machine ON machine.set_id=rom.machine_id
    JOIN catalog_sets AS sets ON sets.set_id=machine.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND rom.status<>'nodump' AND length(rom.name)>0
      AND NOT EXISTS (
          SELECT 1 FROM mame_rom_compatibility AS compatibility
          WHERE compatibility.media_entry_id=rom.media_entry_id
            AND (compatibility.loadflag IS NOT NULL OR compatibility.value IS NOT NULL
              OR compatibility.inverted IS NOT NULL OR compatibility.ovha IS NOT NULL
              OR compatibility.nothread IS NOT NULL OR compatibility.soundonly IS NOT NULL
              OR compatibility.dispose IS NOT NULL)
      )
)
WHEN 'software_rom_entry' THEN rules.format_family='software' AND EXISTS (
    SELECT 1 FROM software_rom_load_entries AS rom
    JOIN candidate_software_rom_operations AS operation
      ON operation.media_entry_id=rom.media_entry_id AND operation.operation='load'
    JOIN candidate_software_file_lengths AS length
      ON length.media_entry_id=rom.media_entry_id AND length.byte_length>0
    JOIN software_required_files AS required ON required.first_load_entry_id=rom.media_entry_id
    JOIN software_areas AS area ON area.area_id=rom.data_area_id AND area.kind='data'
    JOIN software_parts AS part ON part.part_id=area.part_id
    JOIN software_titles AS title ON title.set_id=part.set_id
    JOIN catalog_sets AS sets ON sets.set_id=title.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND rom.status<>'nodump' AND rom.name IS NOT NULL AND length(rom.name)>0
)
WHEN 'logiqx_rom' THEN rules.format_family='logiqx' AND EXISTS (
    SELECT 1 FROM logiqx_roms AS rom
    JOIN logiqx_games AS game ON game.set_id=rom.set_id
    JOIN catalog_sets AS sets ON sets.set_id=game.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND rom.status<>'nodump' AND length(rom.name)>0
)
WHEN 'clrmamepro_rom' THEN rules.format_family='clrmamepro' AND EXISTS (
    SELECT 1 FROM clrmamepro_roms AS rom
    JOIN clrmamepro_sets AS game ON game.set_id=rom.set_id
    JOIN catalog_sets AS sets ON sets.set_id=game.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    JOIN clrmamepro_rom_details AS details ON details.media_entry_id=rom.media_entry_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND length(rom.name)>0
)
WHEN 'no_intro_dat_rom' THEN rules.format_family='no_intro_dat' AND EXISTS (
    SELECT 1 FROM no_intro_dat_rom_claims AS rom
    JOIN no_intro_dat_games AS game ON game.set_id=rom.set_id
    JOIN catalog_sets AS sets ON sets.set_id=game.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND length(rom.name)>0 AND coalesce(rom.status_text,'')<>'nodump'
      AND rom.header_text IS NULL
      AND NOT EXISTS (
          SELECT 1 FROM no_intro_dat_headers AS header
          JOIN no_intro_dat_clrmamepro_options AS filter USING(header_id)
          WHERE header.set_group_id=groups.set_group_id AND filter.header IS NOT NULL
      )
      AND NOT EXISTS (
          SELECT 1 FROM no_intro_dat_rom_field_positions AS position
          WHERE position.media_entry_id=rom.media_entry_id AND position.field_kind='header'
      )
)
WHEN 'no_intro_pc_rom' THEN rules.format_family='no_intro_pc_fixture' AND EXISTS (
    SELECT 1 FROM no_intro_pc_file_claims AS rom
    JOIN no_intro_pc_games AS game ON game.set_id=rom.set_id
    JOIN catalog_sets AS sets ON sets.set_id=game.set_id
    JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id
    WHERE rom.media_entry_id=media.media_entry_id AND groups.edition_id=element.edition_id
      AND length(rom.name)>0
)
ELSE 0 END;

CREATE VIEW candidate_native_file_byte_coverage AS
SELECT role.media_entry_id, role.edition_id, role.format_family,
       role.contract_kind, role.source_size_field, size.size_state,
       size.byte_length
FROM candidate_native_file_roles AS role
JOIN catalog_editions AS edition ON edition.edition_id = role.edition_id
JOIN catalog_file_byte_contracts AS contract
  ON contract.reading_rules_id = edition.reading_rules_id
 AND contract.contract_kind = role.contract_kind
JOIN candidate_native_file_sizes AS size
  ON size.media_entry_id = role.media_entry_id
 AND size.source_size_field = role.source_size_field;

CREATE VIEW candidate_native_file_qualification AS
SELECT coverage.media_entry_id
FROM candidate_native_file_byte_coverage AS coverage
WHERE ((coverage.size_state = 'omitted' AND coverage.byte_length IS NULL)
    OR (coverage.size_state = 'value'
        AND typeof(coverage.byte_length) = 'integer' AND coverage.byte_length >= 0
        AND (coverage.contract_kind <> 'mame_0289_software_file' OR coverage.byte_length > 0)))
  AND NOT EXISTS (
      SELECT 1 FROM catalog_entry_hashes AS bad
      WHERE bad.media_entry_id = coverage.media_entry_id
        AND (bad.presence <> 'value'
          OR bad.source_hash_field NOT IN ('crc', 'crc32', 'md5', 'sha1', 'sha256')
          OR bad.hash_scope NOT IN ('whole_file', 'whole_asset')
          OR NOT EXISTS (
              SELECT 1 FROM candidate_canonical_hash_positions AS position
              WHERE position.reported_hash_id = bad.reported_hash_id
                AND position.media_entry_id = coverage.media_entry_id
                AND position.source_hash_field = bad.source_hash_field
                AND position.field_occurrence = bad.field_occurrence
          )
          OR NOT EXISTS (
              SELECT 1 FROM hash_values AS value
              WHERE value.hash_id = bad.hash_id
                AND value.algorithm = CASE bad.source_hash_field
                    WHEN 'crc' THEN 'crc32' WHEN 'crc32' THEN 'crc32'
                    WHEN 'md5' THEN 'md5' WHEN 'sha1' THEN 'sha1'
                    WHEN 'sha256' THEN 'sha256' END
                AND length(value.bytes) = CASE value.algorithm
                    WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16
                    WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END
          ))
  )
  AND NOT EXISTS (
      SELECT 1
      FROM catalog_entry_hashes AS left_hash
      JOIN hash_values AS left_value ON left_value.hash_id = left_hash.hash_id
      JOIN catalog_entry_hashes AS right_hash
        ON right_hash.media_entry_id = left_hash.media_entry_id
       AND right_hash.reported_hash_id > left_hash.reported_hash_id
      JOIN hash_values AS right_value ON right_value.hash_id = right_hash.hash_id
       AND right_value.algorithm = left_value.algorithm
      WHERE left_hash.media_entry_id = coverage.media_entry_id
        AND left_hash.presence = 'value' AND right_hash.presence = 'value'
        AND left_hash.hash_id <> right_hash.hash_id
  );

CREATE VIEW candidate_qualified_file_hashes AS
SELECT declaration.reported_hash_id, declaration.media_entry_id,
       declaration.hash_id, declaration.hash_scope,
       declaration.source_hash_field
FROM catalog_entry_hashes AS declaration
CROSS JOIN hash_values AS value ON value.hash_id = declaration.hash_id
WHERE declaration.presence = 'value'
  AND declaration.source_hash_field IN ('crc', 'crc32', 'md5', 'sha1', 'sha256')
  AND declaration.hash_scope IN ('whole_file', 'whole_asset')
  AND value.algorithm = CASE declaration.source_hash_field
      WHEN 'crc' THEN 'crc32' WHEN 'crc32' THEN 'crc32'
      WHEN 'md5' THEN 'md5' WHEN 'sha1' THEN 'sha1'
      WHEN 'sha256' THEN 'sha256' END
  AND length(value.bytes) = CASE value.algorithm
      WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16
      WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END
  AND EXISTS (
      SELECT 1 FROM candidate_native_file_byte_coverage AS coverage
      WHERE coverage.media_entry_id = declaration.media_entry_id
  )
  AND EXISTS (
      SELECT 1 FROM candidate_canonical_hash_positions AS position
      WHERE position.reported_hash_id = declaration.reported_hash_id
        AND position.media_entry_id = declaration.media_entry_id
        AND position.source_hash_field = declaration.source_hash_field
        AND position.field_occurrence = declaration.field_occurrence
  );

CREATE VIEW candidate_file_qualification_problems AS
SELECT 'linked_media_not_eligible_native_file' AS problem,
       media.media_entry_id AS owner_id, element.edition_id
FROM catalog_media_entries AS media
JOIN catalog_source_elements AS element
  ON element.source_element_id = media.media_entry_id
WHERE media.file_uuid IS NOT NULL
  AND NOT EXISTS (
      SELECT 1 FROM candidate_native_file_qualification AS qualification
      WHERE qualification.media_entry_id = media.media_entry_id
  )
UNION ALL
SELECT 'linked_media_without_qualified_strong_hash',
       media.media_entry_id, element.edition_id
FROM catalog_media_entries AS media
JOIN catalog_source_elements AS element
  ON element.source_element_id = media.media_entry_id
WHERE media.file_uuid IS NOT NULL
  AND EXISTS (
      SELECT 1 FROM candidate_native_file_qualification AS qualification
      WHERE qualification.media_entry_id = media.media_entry_id
  )
  AND NOT EXISTS (
      SELECT 1 FROM candidate_qualified_file_hashes AS hash
      JOIN hash_values AS value ON value.hash_id = hash.hash_id
      WHERE hash.media_entry_id = media.media_entry_id
        AND value.algorithm IN ('sha1', 'sha256')
  );
""".strip()
