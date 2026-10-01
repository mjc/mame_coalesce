CREATE TABLE cmp_rom_field_positions (
    occurrence_id INTEGER NOT NULL REFERENCES cmp_rom_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 11),
    source_field TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    is_quoted INTEGER NOT NULL CHECK (is_quoted IN (0,1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (occurrence_id,field_kind),
    UNIQUE (occurrence_id,source_order),
    CHECK (lower(source_field) = CASE field_kind
      WHEN 0 THEN 'name' WHEN 1 THEN 'size' WHEN 2 THEN 'crc' WHEN 3 THEN 'crc32'
      WHEN 4 THEN 'md5' WHEN 5 THEN 'sha1' WHEN 6 THEN 'merge' WHEN 7 THEN 'date'
      WHEN 8 THEN 'serial' WHEN 9 THEN 'status' WHEN 10 THEN 'nodump' WHEN 11 THEN 'baddump' END),
    CHECK (field_kind < 10 OR is_quoted = 0)
) WITHOUT ROWID;

CREATE TRIGGER cmp_rom_claims_native_owner_insert
BEFORE INSERT ON cmp_rom_claims
WHEN NOT EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE occurrence_id = NEW.occurrence_id AND claim_kind = 'cmp_rom'
      AND source_element_kind = 'cmp_set' AND format = 'clrmamepro-dat'
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'CMP ROM claims require an unpublished native owner'); END;

CREATE VIEW cmp_rom_declared_field_presence AS
SELECT occurrence_id, 0 AS field_kind FROM cmp_rom_claims
UNION ALL SELECT occurrence_id,1 FROM cmp_rom_claims WHERE size_text IS NOT NULL
UNION ALL SELECT occurrence_id,2 FROM cmp_rom_claims WHERE crc_text IS NOT NULL
UNION ALL SELECT occurrence_id,3 FROM cmp_rom_claims WHERE crc32_text IS NOT NULL
UNION ALL SELECT occurrence_id,4 FROM cmp_rom_claims WHERE md5_text IS NOT NULL
UNION ALL SELECT occurrence_id,5 FROM cmp_rom_claims WHERE sha1_text IS NOT NULL
UNION ALL SELECT occurrence_id,6 FROM cmp_rom_claims WHERE merge_name IS NOT NULL
UNION ALL SELECT occurrence_id,7 FROM cmp_rom_claims WHERE date IS NOT NULL
UNION ALL SELECT occurrence_id,8 FROM cmp_rom_claims WHERE serial IS NOT NULL
UNION ALL SELECT occurrence_id,9 FROM cmp_rom_claims WHERE status_text IS NOT NULL
UNION ALL SELECT occurrence_id,10 FROM cmp_rom_claims WHERE nodump_present = 1
UNION ALL SELECT occurrence_id,11 FROM cmp_rom_claims WHERE baddump_present = 1;

CREATE TRIGGER cmp_rom_field_positions_native_owner_insert
BEFORE INSERT ON cmp_rom_field_positions
WHEN NOT EXISTS (
    SELECT 1 FROM cmp_rom_declared_field_presence AS field
    JOIN asset_occurrences USING (occurrence_id)
    JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE field.occurrence_id = NEW.occurrence_id AND field.field_kind = NEW.field_kind
      AND source_element_kind = 'cmp_set' AND format = 'clrmamepro-dat'
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'CMP positions require a present unpublished native ROM field'); END;
CREATE TRIGGER cmp_rom_field_positions_immutable_update BEFORE UPDATE ON cmp_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM positions are immutable'); END;
CREATE TRIGGER cmp_rom_field_positions_immutable_delete BEFORE DELETE ON cmp_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM positions are immutable'); END;

CREATE TRIGGER cmp_rom_fields_require_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    LEFT JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND asset_occurrences.claim_kind = 'cmp_rom' AND rom.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    JOIN cmp_rom_declared_field_presence AS field USING (occurrence_id)
    LEFT JOIN cmp_rom_field_positions AS position USING (occurrence_id,field_kind)
    WHERE snapshot_key = NEW.snapshot_key AND position.occurrence_id IS NULL
)
BEGIN SELECT RAISE(ABORT, 'CMP ROM fields require complete position ownership'); END;

CREATE VIEW cmp_rom_declaration_conflicts AS
SELECT occurrence_id,'crc_aliases' AS conflict_kind FROM cmp_rom_claims
WHERE crc_text IS NOT NULL AND crc32_text IS NOT NULL AND upper(crc_text) <> upper(crc32_text)
UNION ALL
SELECT occurrence_id,'dump_markers' FROM cmp_rom_claims
WHERE nodump_present + baddump_present > 1
   OR (status_text IS NOT NULL AND nodump_present + baddump_present > 0);

CREATE TRIGGER cmp_uuid_requires_consistent_source_fields
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND content_uuid IS NOT NULL
      AND (rom.sha1_text IS NULL
        OR EXISTS (SELECT 1 FROM cmp_rom_declaration_conflicts AS conflict
                   WHERE conflict.occurrence_id = rom.occurrence_id)
        OR NOT EXISTS (
            SELECT 1 FROM usable_occurrence_digest_assertions AS assertion
            WHERE assertion.occurrence_id = rom.occurrence_id
              AND assertion.provenance = 'source_declared'
              AND assertion.scope = rom.evidence_scope AND assertion.scope IN ('whole_file','whole_asset')
              AND assertion.algorithm = 'sha1' AND hex(assertion.digest) = upper(rom.sha1_text)
        ))
)
BEGIN SELECT RAISE(ABORT, 'CMP UUID requires consistent whole-file source evidence'); END;

CREATE VIEW cmp_rom_declared_digests AS
SELECT occurrence_id,evidence_scope AS scope,'crc32' AS algorithm,upper(crc_text) AS digest_hex
FROM cmp_rom_claims WHERE crc_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'crc32',upper(crc32_text) FROM cmp_rom_claims WHERE crc32_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'md5',upper(md5_text) FROM cmp_rom_claims WHERE md5_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'sha1',upper(sha1_text) FROM cmp_rom_claims WHERE sha1_text IS NOT NULL;

CREATE TRIGGER cmp_rom_digests_match_source_declarations
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND (
      EXISTS (
        SELECT 1 FROM cmp_rom_declared_digests AS field WHERE field.occurrence_id = rom.occurrence_id
          AND NOT EXISTS (
            SELECT 1 FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING (digest_id)
            WHERE assertion.occurrence_id = field.occurrence_id AND assertion.scope = field.scope
              AND assertion.provenance = 'source_declared' AND digest.algorithm = field.algorithm
              AND hex(digest.digest) = field.digest_hex
          )
      ) OR EXISTS (
        SELECT 1 FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.occurrence_id = rom.occurrence_id AND assertion.provenance = 'source_declared'
          AND NOT EXISTS (
            SELECT 1 FROM cmp_rom_declared_digests AS field
            WHERE field.occurrence_id = rom.occurrence_id AND field.scope = assertion.scope
              AND field.algorithm = digest.algorithm AND field.digest_hex = hex(digest.digest)
          )
      )
    )
)
BEGIN SELECT RAISE(ABORT, 'CMP source assertions must match declared ROM fields'); END;
