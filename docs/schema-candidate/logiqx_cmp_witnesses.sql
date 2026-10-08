-- Run from the repository root with:
-- sqlite3 :memory: < docs/schema-candidate/logiqx_cmp_witnesses.sql
-- Minimal common-key fixtures keep this witness bounded and self-contained.
.bail on
PRAGMA foreign_keys = ON;

CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY,
    format_family TEXT NOT NULL,
    rules_version TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY,
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id)
) STRICT;
CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    group_kind TEXT NOT NULL,
    UNIQUE (set_group_id, edition_id)
) STRICT;
CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    element_kind TEXT NOT NULL,
    UNIQUE (source_element_id, edition_id)
) STRICT;
CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    set_name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id, source_order)
) STRICT;
CREATE TABLE catalog_media_entries (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    file_uuid BLOB
) STRICT;
CREATE TABLE hash_values (
    hash_id INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    bytes BLOB NOT NULL,
    CHECK (length(bytes) = CASE algorithm WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16 WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END),
    UNIQUE (algorithm, bytes)
) STRICT;
CREATE TABLE catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY,
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    source_hash_field TEXT NOT NULL CHECK (source_hash_field IN ('crc', 'crc32', 'md5', 'sha1', 'sha256', 'origin_sha256')),
    field_occurrence INTEGER NOT NULL CHECK (field_occurrence >= 0),
    presence TEXT NOT NULL CHECK (presence IN ('empty', 'invalid', 'value')),
    hash_scope TEXT NOT NULL CHECK (hash_scope IN ('whole_file', 'whole_asset', 'unknown', 'chd_header_sha1', 'source_origin', 'nfo_companion')),
    hash_id INTEGER REFERENCES hash_values(hash_id),
    reported_text TEXT,
    CHECK ((presence = 'empty' AND hash_id IS NULL AND reported_text IS NULL) OR
           (presence = 'invalid' AND hash_id IS NULL AND reported_text IS NOT NULL AND reported_text <> '') OR
           (presence = 'value' AND hash_id IS NOT NULL)),
    UNIQUE (media_entry_id, source_hash_field, field_occurrence)
) STRICT;
CREATE TABLE reported_catalog_relationships (relationship_id INTEGER PRIMARY KEY) STRICT;

.read docs/schema-candidate/logiqx_cmp.sql

INSERT INTO catalog_reading_rules VALUES
    (1, 'logiqx', 'logiqx-declared-text-compat-v2'),
    (2, 'clrmamepro', 'clrmamepro-declared-text-compat-v1');
INSERT INTO catalog_editions VALUES (1, 1), (2, 2);
INSERT INTO catalog_set_groups VALUES (10, 1, 'root'), (20, 2, 'root');
INSERT INTO logiqx_documents
    (edition_id, set_group_id, build, debug, debug_was_present, extent_view, extent_start, extent_end,
     location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (1, 10, 'build-x', 'no', 0, 'retained_original_bytes', 0, 100,
        'retained_original_text', 1, 1, 10, 1, 'one_based_unicode_scalar');
INSERT INTO logiqx_document_attribute_positions
    (edition_id, field_kind, source_order, source_line, source_column)
VALUES (1, 0, 0, 1, 2);

INSERT INTO catalog_source_elements VALUES
    (101, 1, 'logiqx_root_file_name'), (102, 1, 'logiqx_root_sha1'),
    (103, 1, 'logiqx_header'), (104, 1, 'logiqx_clrmamepro_options'),
    (105, 1, 'logiqx_game'), (106, 1, 'logiqx_header_text'),
    (107, 1, 'logiqx_game_text'), (108, 1, 'logiqx_game_comment'),
    (109, 1, 'logiqx_rom'), (110, 1, 'logiqx_device_reference'),
    (111, 1, 'logiqx_disk');
INSERT INTO reported_catalog_relationships VALUES
    (900), (901), (902), (903), (910), (911), (912), (913), (914), (915), (916);
INSERT INTO hash_values VALUES (1, 'sha1', zeroblob(20)), (2, 'crc32', x'01020304');
INSERT INTO logiqx_root_file_names VALUES (101, 10, '', 0, 1, 20);
INSERT INTO logiqx_root_sha1_elements VALUES (102, 10, 1, 'unknown', 1, 1, 40);
INSERT INTO logiqx_headers VALUES (103, 10, 2, 1, 60);
INSERT INTO logiqx_clrmamepro_options
    (source_element_id, header_id, source_order, source_line, source_column,
     forcemerging_was_present, forcenodump_was_present, forcepacking_was_present)
VALUES (104, 103, 0, 1, 66, 0, 0, 0);
INSERT INTO logiqx_header_text_elements VALUES (106, 103, 0, '', 1, 1, 70);
INSERT INTO catalog_sets VALUES (105, 10, 'game-a', 3, 2, 1);
INSERT INTO logiqx_games VALUES (105, NULL, 'no', 0, NULL, NULL);
INSERT INTO logiqx_set_links VALUES
    (105, 'cloneof', 'parent-a', 910), (105, 'romof', 'parent-b', 911),
    (105, 'sampleof', 'parent-c', 912);
INSERT INTO logiqx_game_text_elements VALUES (107, 105, 0, '', 0, 2, 1);
INSERT INTO logiqx_game_attribute_positions
    (set_id, field_kind, source_order, source_line, source_column)
VALUES (105, 0, 0, 2, 5);
INSERT INTO logiqx_game_attribute_positions
    (set_id, field_kind, source_order, source_line, source_column, relationship_id)
VALUES (105, 3, 1, 2, 10, 910), (105, 4, 2, 2, 20, 911), (105, 5, 3, 2, 30, 912);
INSERT INTO logiqx_game_comments VALUES (108, 105, '; repeated text', 1, 3, 1);
INSERT INTO catalog_media_entries VALUES (109, NULL);
INSERT INTO logiqx_roms
    (media_entry_id, set_id, name, size_text, status, status_specified, date, source_order, source_line, source_column)
VALUES (109, 105, '', 'not-decimal', 'good', 0, NULL, 2, 4, 1);
INSERT INTO catalog_entry_hashes VALUES (700, 109, 'crc', 0, 'value', 'unknown', 2, NULL);
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column)
VALUES (109, 0, 0, 4, 10);
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column)
VALUES (109, 1, 1, 4, 15);
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, reported_hash_id)
VALUES (109, 2, 2, 4, 20, 700);
INSERT INTO logiqx_rom_merges VALUES (109, 'parent-rom.bin', 914);
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, relationship_id)
VALUES (109, 5, 3, 4, 30, 914);
INSERT INTO logiqx_device_references VALUES (110, 105, 'device-a', 913, 3, 5, 1);
INSERT INTO logiqx_device_reference_attribute_positions
    (source_element_id, field_kind, source_order, source_line, source_column, relationship_id)
VALUES (110, 0, 0, 5, 10, 913);
INSERT INTO catalog_media_entries VALUES (111, NULL);
INSERT INTO logiqx_disks VALUES (111, 105, 'disk.chd', 'good', 0, 4, 6, 1);
INSERT INTO logiqx_disk_merges VALUES (111, 'parent-disk.chd', 915);
INSERT INTO logiqx_disk_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, relationship_id)
VALUES (111, 0, 0, 6, 10, NULL), (111, 3, 1, 6, 20, 915);

INSERT INTO clrmamepro_documents
    (edition_id, set_group_id, header_present, comment_count, extent_view, extent_start, extent_end,
     location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (2, 20, 1, 1, 'retained_original_bytes', 0, 80,
        'decoded_dat_text', 1, 1, 8, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_source_elements VALUES
    (201, 2, 'clrmamepro_header'), (202, 2, 'clrmamepro_set'),
    (203, 2, 'clrmamepro_rom'), (204, 2, 'clrmamepro_sample'),
    (205, 2, 'clrmamepro_comment');
INSERT INTO clrmamepro_headers VALUES
    (201, 2, 'clrmamepro', 0, 1, 1, '', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
UPDATE clrmamepro_header_options SET header_definition = 'clrmamepro' WHERE header_id = 201;
INSERT INTO clrmamepro_header_field_positions
    (header_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (201, 0, 0, 'name', 1, 3, 1, 8, 1);
INSERT INTO clrmamepro_header_field_positions
    (header_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (201, 10, 1, 'header', 1, 14, 1, 22, 1);

INSERT INTO catalog_sets VALUES (202, 20, '', 1, 2, 1);
INSERT INTO clrmamepro_sets VALUES (202, 'game', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
INSERT INTO clrmamepro_set_links VALUES (202, 'cloneof', 'parent-game', 900), (202, 'sampleof', 'parent-samples', 901);
INSERT INTO clrmamepro_set_field_positions
    (set_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (202, 0, 0, 'name', 2, 3, 2, 8, 1);
INSERT INTO clrmamepro_set_field_positions
    (set_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted, relationship_id)
VALUES (202, 1, 1, 'cloneof', 2, 12, 2, 20, 1, 900),
       (202, 6, 2, 'sampleof', 2, 25, 2, 34, 1, 901);

INSERT INTO catalog_media_entries VALUES (203, NULL), (204, NULL);
INSERT INTO clrmamepro_roms
    (media_entry_id, set_id, name, size_text, source_order, source_line, source_column)
VALUES (203, 202, 'rom.bin', '0008', 3, 3, 3);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (203, 0, 0, 'name', 3, 10, 3, 15, 1);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (203, 1, 1, 'size', 3, 17, 3, 18, 0);
INSERT INTO catalog_entry_hashes VALUES (701, 203, 'crc', 0, 'value', 'whole_asset', 2, '01020304');
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted, reported_hash_id)
VALUES (203, 2, 2, 'crc', 3, 20, 3, 25, 1, 701);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_is_quoted)
VALUES (203, 10, 3, 'nodump', 3, 40, 0);
INSERT INTO clrmamepro_rom_merges VALUES (203, 'parent-rom.bin', 902);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted, relationship_id)
VALUES (203, 6, 4, 'merge', 3, 50, 3, 58, 1, 902);
INSERT INTO clrmamepro_samples
    (media_entry_id, set_id, sample_name, source_field, source_order,
     keyword_line, keyword_column, value_line, value_column, value_is_quoted)
VALUES (204, 202, '', 'sample', 4, 4, 3, 4, 10, 1);
INSERT INTO clrmamepro_comments VALUES (205, 2, '; note', 5, 1);

CREATE TEMP TABLE witness_assert (ok INTEGER NOT NULL CHECK (ok = 1)) STRICT;
INSERT INTO witness_assert
SELECT file_name = '' AND (SELECT hash_scope FROM logiqx_root_sha1_elements) = 'unknown'
FROM logiqx_root_file_names;
INSERT INTO witness_assert
SELECT size_text = 'not-decimal' AND size_value IS NULL AND status = 'good' AND status_specified = 0
FROM logiqx_roms;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(value_is_quoted) = 1
FROM clrmamepro_rom_details AS details
JOIN clrmamepro_roms AS rom USING (media_entry_id)
JOIN clrmamepro_rom_field_positions AS position USING (media_entry_id)
WHERE rom.media_entry_id = 203 AND position.field_kind = 2;
INSERT INTO witness_assert
SELECT count(*) = 2 AND sum(role = 'Keyword') = 1 AND sum(role = 'Value') = 1
FROM clrmamepro_position_tokens WHERE owner_kind = 'rom' AND owner_id = 203 AND field_kind = 2;
INSERT INTO witness_assert
SELECT count(*) = 2 AND count(relationship_id) = 1 AND max(CASE WHEN role = 'Value' THEN relationship_id END) = 902
FROM clrmamepro_position_tokens WHERE owner_kind = 'rom' AND owner_id = 203 AND field_kind = 6;
INSERT INTO witness_assert
SELECT sum(role = 'Keyword') = 1 AND sum(role = 'Value') = 1
FROM clrmamepro_position_tokens WHERE owner_kind = 'sample' AND owner_id = 204;
INSERT INTO witness_assert
SELECT comment_count = (SELECT count(*) FROM clrmamepro_comments WHERE edition_id = 2)
FROM clrmamepro_documents WHERE edition_id = 2;

-- Local SQL constraints reject non-closed values, duplicate field identities,
-- invalid CMP sizes, hash-position mismatches and repeated comment starts.
UPDATE OR IGNORE logiqx_clrmamepro_options SET forcepacking = 'rar', forcepacking_was_present = 1 WHERE source_element_id = 104;
INSERT INTO witness_assert SELECT changes() = 0;
INSERT OR IGNORE INTO clrmamepro_header_field_positions
    (header_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted)
VALUES (201, 10, 2, 'header', 1, 30, 1, 38, 0);
INSERT INTO witness_assert SELECT changes() = 0;
INSERT OR IGNORE INTO catalog_source_elements VALUES (206, 2, 'clrmamepro_rom');
INSERT INTO catalog_media_entries VALUES (206, NULL);
INSERT OR IGNORE INTO clrmamepro_roms
    (media_entry_id, set_id, name, size_text, source_order, source_line, source_column)
VALUES (206, 202, 'bad.bin', '8x', 5, 6, 1);
INSERT INTO witness_assert SELECT changes() = 0;
INSERT OR IGNORE INTO catalog_source_elements VALUES (207, 2, 'clrmamepro_comment');
INSERT OR IGNORE INTO clrmamepro_comments VALUES (207, 2, '; duplicate coordinate', 5, 1);
INSERT INTO witness_assert SELECT changes() = 0;

-- Banach 01a119fa-da75-7e93-8230-478660ef464c: each mutation targets an
-- existing otherwise-valid row, so no duplicate key masks its CHECK failure.
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_line = NULL WHERE media_entry_id = 203 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_column = NULL WHERE media_entry_id = 203 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_line = 0 WHERE media_entry_id = 203 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_line = NULL WHERE media_entry_id = 203 AND field_kind = 2;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_column = NULL WHERE media_entry_id = 203 AND field_kind = 2;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_column = 0 WHERE media_entry_id = 203 AND field_kind = 2;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET value_line = 3 WHERE media_entry_id = 203 AND field_kind = 10;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_header_field_positions SET value_line = NULL WHERE header_id = 201 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_set_field_positions SET value_column = NULL WHERE set_id = 202 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_documents SET column_convention = NULL WHERE edition_id = 1;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_documents SET column_convention = NULL WHERE edition_id = 2;
INSERT INTO witness_assert SELECT changes() = 0;

-- Required relationship codes reject NULL; ordinary scalar/hash/flag codes
-- reject even a real, unclaimed relationship ID. All six position families
-- are exercised, including all three game links and both CMP set links.
UPDATE OR IGNORE logiqx_game_attribute_positions SET relationship_id = NULL WHERE set_id = 105 AND field_kind IN (3,4,5);
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_device_reference_attribute_positions SET relationship_id = NULL WHERE source_element_id = 110;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_rom_attribute_positions SET relationship_id = NULL WHERE media_entry_id = 109 AND field_kind = 5;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_disk_attribute_positions SET relationship_id = NULL WHERE media_entry_id = 111 AND field_kind = 3;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_set_field_positions SET relationship_id = NULL WHERE set_id = 202 AND field_kind IN (1,6);
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET relationship_id = NULL WHERE media_entry_id = 203 AND field_kind = 6;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_game_attribute_positions SET relationship_id = 916 WHERE set_id = 105 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_rom_attribute_positions SET relationship_id = 916 WHERE media_entry_id = 109 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE logiqx_disk_attribute_positions SET relationship_id = 916 WHERE media_entry_id = 111 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_set_field_positions SET relationship_id = 903 WHERE set_id = 202 AND field_kind = 0;
INSERT INTO witness_assert SELECT changes() = 0;
UPDATE OR IGNORE clrmamepro_rom_field_positions SET relationship_id = 903 WHERE media_entry_id = 203 AND field_kind IN (0,2,10);
INSERT INTO witness_assert SELECT changes() = 0;
-- This case specifically tests relationship-ID uniqueness, with a valid code.
UPDATE OR IGNORE clrmamepro_set_field_positions SET relationship_id = 901 WHERE set_id = 202 AND field_kind = 1;
INSERT INTO witness_assert SELECT changes() = 0;

-- Fresh valid declaration IDs and unoccupied hash codes/orders reach the
-- mapping diagnostic; UNIQUE/FK/CHECK rejection would stop this witness.
INSERT INTO catalog_entry_hashes VALUES (702, 203, 'sha1', 0, 'value', 'whole_asset', 1, NULL);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted, reported_hash_id)
VALUES (203, 3, 5, 'crc32', 3, 65, 3, 72, 0, 702);
INSERT INTO witness_assert SELECT changes() = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(edition_id) = 2 AND min(owner_id) = 702
FROM candidate_logiqx_cmp_integrity_problems WHERE problem = 'cmp_hash_mapping';
DELETE FROM clrmamepro_rom_field_positions WHERE media_entry_id = 203 AND field_kind = 3;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id = 702;
INSERT INTO catalog_entry_hashes VALUES (703, 109, 'sha1', 0, 'value', 'unknown', 1, NULL);
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, reported_hash_id)
VALUES (109, 4, 4, 4, 40, 703);
INSERT INTO witness_assert SELECT changes() = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(edition_id) = 1 AND min(owner_id) = 703
FROM candidate_logiqx_cmp_integrity_problems WHERE problem = 'logiqx_hash_mapping';
DELETE FROM logiqx_rom_attribute_positions WHERE media_entry_id = 109 AND field_kind = 4;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id = 703;

-- A correct code with the wrong declaration owner must report the native
-- position owner's edition (2), not the declaration owner's edition (1).
INSERT INTO catalog_entry_hashes VALUES (704, 109, 'sha1', 0, 'value', 'unknown', 1, NULL);
INSERT INTO clrmamepro_rom_field_positions
    (media_entry_id, field_kind, source_order, keyword, keyword_line, keyword_column,
     value_line, value_column, value_is_quoted, reported_hash_id)
VALUES (203, 5, 5, 'sha1', 3, 65, 3, 72, 0, 704);
INSERT INTO witness_assert SELECT changes() = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(edition_id) = 2 AND min(owner_id) = 704
FROM candidate_logiqx_cmp_integrity_problems WHERE problem = 'cmp_hash_mapping';
DELETE FROM clrmamepro_rom_field_positions WHERE media_entry_id = 203 AND field_kind = 5;
DELETE FROM catalog_entry_hashes WHERE reported_hash_id = 704;

-- A real but different relationship ID passes local FK/code checks and must
-- be diagnosed against the actual native link/merge owner.
UPDATE clrmamepro_set_field_positions SET relationship_id = 903 WHERE set_id = 202 AND field_kind = 1;
INSERT INTO witness_assert SELECT changes() = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(edition_id) = 2
FROM candidate_logiqx_cmp_integrity_problems WHERE problem = 'relationship_position_mismatch' AND detail = 'clrmamepro_set:1';
UPDATE clrmamepro_set_field_positions SET relationship_id = 900 WHERE set_id = 202 AND field_kind = 1;
UPDATE logiqx_game_attribute_positions SET relationship_id = 916 WHERE set_id = 105 AND field_kind = 3;
INSERT INTO witness_assert SELECT changes() = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(edition_id) = 1
FROM candidate_logiqx_cmp_integrity_problems WHERE problem = 'relationship_position_mismatch' AND detail = 'logiqx_game:3';
UPDATE logiqx_game_attribute_positions SET relationship_id = 910 WHERE set_id = 105 AND field_kind = 3;

-- The family view catches cross-table rules that cannot be expressed by a
-- single table's CHECK/UNIQUE constraint, then returns to a clean witness.
UPDATE logiqx_game_comments SET source_order = 2 WHERE source_element_id = 108;
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'logiqx_game_mixed_order_collision';
UPDATE logiqx_game_comments SET source_order = 1 WHERE source_element_id = 108;
UPDATE clrmamepro_comments SET source_line = 3, source_column = 20 WHERE comment_id = 205;
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'cmp_lexical_coordinate_collision';
UPDATE clrmamepro_comments SET source_line = 5, source_column = 1 WHERE comment_id = 205;
UPDATE clrmamepro_samples SET keyword_line = 3, keyword_column = 1 WHERE media_entry_id = 204;
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'cmp_set_item_order_inversion';
UPDATE clrmamepro_samples SET keyword_line = 4, keyword_column = 3 WHERE media_entry_id = 204;
UPDATE clrmamepro_rom_field_positions SET keyword_column = 5 WHERE media_entry_id = 203 AND field_kind = 1;
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'cmp_nested_field_order_inversion';
UPDATE clrmamepro_rom_field_positions SET keyword_column = 17 WHERE media_entry_id = 203 AND field_kind = 1;
DELETE FROM logiqx_rom_attribute_positions WHERE media_entry_id = 109 AND field_kind = 2;
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'missing_position' AND detail = 'logiqx_rom:2';
INSERT INTO logiqx_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, reported_hash_id)
VALUES (109, 2, 2, 4, 20, 700);

INSERT INTO catalog_reading_rules VALUES (3, 'logiqx', 'logiqx-dtd-1.5-v1');
INSERT INTO catalog_editions VALUES (3, 3);
INSERT INTO catalog_set_groups VALUES (30, 3, 'root');
INSERT INTO logiqx_documents
    (edition_id, set_group_id, debug, debug_was_present, extent_view, extent_start, extent_end)
VALUES (3, 30, 'no', 0, 'retained_original_bytes', 0, 10);
INSERT INTO witness_assert
SELECT count(*) = 1 FROM candidate_logiqx_cmp_integrity_problems
WHERE problem = 'strict_logiqx_game_count' AND edition_id = 3;
DELETE FROM logiqx_documents WHERE edition_id = 3;

INSERT INTO witness_assert SELECT count(*) = 0 FROM pragma_foreign_key_check;
INSERT INTO witness_assert SELECT count(*) = 0 FROM candidate_logiqx_cmp_integrity_problems;

.eqp on
SELECT source_element_id FROM logiqx_game_comments
WHERE set_id = 105 ORDER BY source_order, source_element_id;
SELECT media_entry_id FROM clrmamepro_roms
WHERE set_id = 202 ORDER BY source_order, media_entry_id;
SELECT comment_id FROM clrmamepro_comments
WHERE edition_id = 2 ORDER BY source_line, source_column;
.eqp off

SELECT 'Logiqx/CMP witnesses passed' AS result;
