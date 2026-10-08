-- Run from the repository root with:
-- sqlite3 :memory: < docs/schema-candidate/software_witnesses.sql
-- Minimal common-key fixture; the production candidate bases live in shared.sql.
.bail on
PRAGMA foreign_keys = ON;

CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY,
    format_family TEXT NOT NULL
) STRICT;
CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY,
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id)
) STRICT;
CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    element_kind TEXT NOT NULL,
    UNIQUE (source_element_id, edition_id)
) STRICT;
CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    group_kind TEXT NOT NULL,
    UNIQUE (set_group_id, edition_id)
) STRICT;
CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    set_name TEXT NOT NULL,
    source_order INTEGER NOT NULL,
    UNIQUE (set_group_id, source_order)
) STRICT;
CREATE TABLE catalog_media_entries (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id)
) STRICT;
CREATE TABLE hash_values (
    hash_id INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL,
    bytes BLOB NOT NULL
) STRICT;
CREATE TABLE catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY,
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    source_hash_field TEXT NOT NULL,
    field_occurrence INTEGER NOT NULL,
    presence TEXT NOT NULL,
    hash_scope TEXT NOT NULL,
    hash_id INTEGER REFERENCES hash_values(hash_id),
    reported_text TEXT,
    UNIQUE (media_entry_id, source_hash_field, field_occurrence)
) STRICT;

.read docs/schema-candidate/relationships.sql

.read docs/schema-candidate/software.sql

INSERT INTO catalog_reading_rules VALUES (1, 'software');
INSERT INTO catalog_editions VALUES (1, 1);
INSERT INTO software_documents
    (edition_id, envelope_kind, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (1, 'single_list', 'retained_original_text', 1, 1, 10, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_set_groups VALUES (10, 1, 'software_list');
INSERT INTO software_lists
    (set_group_id, name, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (10, 'inventory', 'retained_original_text', 2, 1, 9, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_source_elements VALUES (20, 1, 'software_item');
INSERT INTO catalog_source_elements VALUES (24, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (25, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (26, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (27, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (28, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (29, 1, 'software_title_text_element');
INSERT INTO catalog_sets VALUES (20, 10, 'game', 0);
INSERT INTO software_titles VALUES (20, 'partial', 1);
INSERT INTO software_title_text_elements VALUES (24, 20, 0, 'Game', 0, 2, 12);
INSERT INTO software_title_text_elements VALUES (25, 20, 1, '2000', 1, 2, 30);
INSERT INTO software_title_text_elements VALUES (26, 20, 2, 'Pub', 2, 2, 42);
INSERT INTO catalog_source_elements VALUES (21, 1, 'software_item');
INSERT INTO catalog_sets VALUES (21, 10, 'clone', 1);
INSERT INTO software_titles VALUES (21, 'yes', 0);
INSERT INTO software_title_text_elements VALUES (27, 21, 0, 'Clone', 0, 2, 12);
INSERT INTO software_title_text_elements VALUES (28, 21, 1, '2002', 1, 2, 30);
INSERT INTO software_title_text_elements VALUES (29, 21, 2, 'Pub', 2, 2, 42);
INSERT INTO catalog_relationships (relationship_id, assertion_key, origin, edition_id)
VALUES (80, 'software-clone-80', 'source', 1);
INSERT INTO reported_catalog_relationships VALUES (80, 'software_cloneof');
INSERT INTO software_clone_links VALUES (21, '', 80);
INSERT INTO software_title_attribute_positions
    (set_id, field_kind, source_order, source_line, source_column, relationship_id)
VALUES (21, 1, 0, 2, 30, 80);
INSERT INTO catalog_source_elements VALUES (30, 1, 'software_part');
INSERT INTO software_parts VALUES (30, 20, 'cart', 'cart', 3, 3, 3);
INSERT INTO catalog_source_elements VALUES (40, 1, 'software_area');
INSERT INTO software_areas VALUES (40, 30, 'data', 0, 4, 3);
INSERT INTO software_data_areas VALUES (40, 'rom', '010', 8, 0, 'little', 0);
INSERT INTO catalog_source_elements VALUES (50, 1, 'software_rom_entry');
INSERT INTO catalog_media_entries VALUES (50);
INSERT INTO catalog_source_elements VALUES (51, 1, 'software_rom_entry');
INSERT INTO catalog_media_entries VALUES (51);
INSERT INTO catalog_source_elements VALUES (52, 1, 'software_rom_entry');
INSERT INTO catalog_media_entries VALUES (52);
INSERT INTO software_rom_load_entries
    (media_entry_id, data_area_id, name, size_text, status, status_specified, source_order, source_line, source_column)
VALUES (50, 40, '', 'invalid-size', 'good', 0, 0, 5, 5);
INSERT INTO software_rom_load_entries
    (media_entry_id, data_area_id, name, loadflag, status, status_specified, source_order, source_line, source_column)
VALUES (51, 40, 'other-layout.bin', 'load64_word_swap', 'good', 0, 1, 6, 5);
INSERT INTO software_required_files VALUES (60, 50);
INSERT INTO software_file_load_steps VALUES (50, 60);
INSERT INTO software_required_files VALUES (61, 51);
INSERT INTO software_file_load_steps VALUES (51, 61);

-- A plural wrapper has a real root owner; each nested list is a source-element
-- placement whose canonical list extent is contained by the wrapper root.
INSERT INTO catalog_reading_rules VALUES (2, 'software');
INSERT INTO catalog_editions VALUES (2, 2);
INSERT INTO software_documents
    (edition_id, envelope_kind, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (2, 'plural_lists', 'retained_original_text', 1, 1, 20, 1, 'one_based_unicode_scalar');
INSERT INTO software_wrapper_headers
    (wrapper_id, edition_id, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (200, 2, 'retained_original_text', 2, 1, 18, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_set_groups VALUES (20, 2, 'software_list');
INSERT INTO software_lists
    (set_group_id, name, location_view, start_line, start_column, end_line, end_column, column_convention)
VALUES (20, 'nested', 'retained_original_text', 3, 1, 12, 1, 'one_based_unicode_scalar');
INSERT INTO catalog_source_elements VALUES (220, 2, 'software_list_wrapper_entry');
INSERT INTO software_list_wrapper_entries VALUES (220, 20, 200, 0);
INSERT INTO catalog_source_elements VALUES (230, 2, 'software_item');
INSERT INTO catalog_source_elements VALUES (231, 2, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (232, 2, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (233, 2, 'software_title_text_element');
INSERT INTO catalog_sets VALUES (230, 20, 'nested-game', 0);
INSERT INTO software_titles VALUES (230, 'yes', 0);
INSERT INTO software_title_text_elements VALUES (231, 230, 0, 'Nested game', 0, 4, 12);
INSERT INTO software_title_text_elements VALUES (232, 230, 1, '2001', 1, 4, 31);
INSERT INTO software_title_text_elements VALUES (233, 230, 2, 'Pub', 2, 4, 39);

-- Default and explicit state, title text singleton, and the raw invalid numeric
-- lexeme survive as specified. These assertions fail the script if not true.
CREATE TEMP TABLE witness_assert (ok INTEGER NOT NULL CHECK (ok = 1)) STRICT;
INSERT INTO witness_assert
SELECT count(*) = 3
   AND sum(name = 'problem') = 1
   AND sum(name = 'owner_id') = 1
   AND sum(name = 'edition_id') = 1
FROM pragma_table_info('candidate_software_integrity_problems');
INSERT INTO witness_assert
SELECT supported = 'partial' AND supported_specified = 1
FROM software_titles WHERE set_id = 20;
INSERT INTO witness_assert
SELECT size_text = 'invalid-size' AND status = 'good' AND status_specified = 0
FROM software_rom_load_entries WHERE media_entry_id = 50;
INSERT INTO witness_assert
SELECT loadflag = 'load64_word_swap'
FROM software_rom_load_entries WHERE media_entry_id = 51;
INSERT INTO witness_assert
SELECT step.load_entry_id = 50 AND step.required_file_id = 60
FROM software_file_load_steps AS step WHERE step.load_entry_id = 50;
INSERT INTO witness_assert
SELECT link.target_name = '' AND position.relationship_id = link.relationship_id
FROM software_clone_links AS link
JOIN software_title_attribute_positions AS position USING (set_id)
WHERE link.set_id = 21 AND position.field_kind = 1;
INSERT INTO witness_assert
SELECT NOT EXISTS (SELECT 1 FROM candidate_software_integrity_problems);

-- A nested canonical list must stay within its actual plural wrapper extent.
UPDATE software_lists SET end_line = 19 WHERE set_group_id = 20;
INSERT INTO witness_assert
SELECT EXISTS (SELECT 1 FROM candidate_software_integrity_problems
               WHERE problem = 'software_root_coordinate_extent_escape:wrapper_list'
                 AND owner_id = 20);
UPDATE software_lists SET end_line = 12 WHERE set_group_id = 20;
INSERT INTO witness_assert
SELECT NOT EXISTS (SELECT 1 FROM candidate_software_integrity_problems);

-- Isolate the supported enum from identity, placement and default-state rules.
-- The same fresh owner succeeds with an explicit valid value and has complete
-- required text children; rolling back that control frees its native PK again.
SAVEPOINT supported_enum_witness;
INSERT INTO catalog_source_elements VALUES (22, 1, 'software_item');
INSERT INTO catalog_sets VALUES (22, 10, 'enum-control', 2);
INSERT INTO catalog_source_elements VALUES (122, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (123, 1, 'software_title_text_element');
INSERT INTO catalog_source_elements VALUES (124, 1, 'software_title_text_element');
SAVEPOINT supported_enum_control;
INSERT INTO software_titles VALUES (22, 'partial', 1);
INSERT INTO software_title_text_elements VALUES (122, 22, 0, 'Enum control', 0, 3, 12);
INSERT INTO software_title_text_elements VALUES (123, 22, 1, '2003', 1, 3, 31);
INSERT INTO software_title_text_elements VALUES (124, 22, 2, 'Pub', 2, 3, 42);
INSERT INTO witness_assert
SELECT count(*) = 1 AND min(supported = 'partial' AND supported_specified = 1)
FROM software_titles WHERE set_id = 22;
INSERT INTO witness_assert
SELECT NOT EXISTS (SELECT 1 FROM candidate_software_integrity_problems);
INSERT INTO witness_assert SELECT count(*) = 0 FROM pragma_foreign_key_check;
ROLLBACK TO supported_enum_control;
RELEASE supported_enum_control;
-- Only the supported token differs from the successful native-owner insert.
-- specified=1 also satisfies the separate omitted-default CHECK for 'maybe'.
INSERT OR IGNORE INTO software_titles VALUES (22, 'maybe', 1);
INSERT INTO witness_assert SELECT changes() = 0;
INSERT INTO witness_assert SELECT count(*) = 0 FROM software_titles WHERE set_id = 22;
ROLLBACK TO supported_enum_witness;
RELEASE supported_enum_witness;

INSERT OR IGNORE INTO software_rom_load_entries
    (media_entry_id, data_area_id, name, loadflag, status, status_specified, source_order, source_line, source_column)
VALUES (52, 40, 'wrong-layout.bin', 'load', 'good', 0, 2, 7, 5);
INSERT INTO witness_assert SELECT changes() = 0;

-- Position ledgers own only QName coordinates; singleton occurrence and order
-- constraints reject duplicate codes/source-order slots.
INSERT INTO software_title_attribute_positions
    (set_id, field_kind, source_order, source_line, source_column)
VALUES (20, 0, 0, 2, 11);
INSERT OR IGNORE INTO software_title_attribute_positions
    (set_id, field_kind, source_order, source_line, source_column)
VALUES (20, 0, 1, 2, 11);
INSERT INTO witness_assert SELECT changes() = 0;

-- A hash position cannot point at no declaration; the common declaration row
-- itself distinguishes supplied-empty from absent (absence has no row).
INSERT INTO catalog_entry_hashes VALUES (70, 50, 'crc', 0, 'empty', 'whole_file', NULL, NULL);
INSERT INTO software_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column, reported_hash_id)
VALUES (50, 2, 0, 5, 17, 70);
INSERT OR IGNORE INTO software_rom_attribute_positions
    (media_entry_id, field_kind, source_order, source_line, source_column)
VALUES (50, 3, 1, 5, 17);
INSERT INTO witness_assert SELECT changes() = 0;

INSERT INTO witness_assert SELECT count(*) = 0 FROM pragma_foreign_key_check;

-- Print the populated area-local ordered query plan for inspection.
.eqp on
SELECT media_entry_id FROM software_rom_load_entries
WHERE data_area_id = 40 ORDER BY source_order;
.eqp off

SELECT 'software witnesses passed' AS result;
