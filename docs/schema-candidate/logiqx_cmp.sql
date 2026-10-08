-- DESIGN ONLY. Requires shared.sql first; this is not application SCHEMA.
-- IDs and native values are stored once. Cross-family edition, registry-kind,
-- mixed-order, publication, extent-bound and count-seal checks are assembled
-- by the parent harness from the owner manifest.

-- Physical Logiqx datafile root. The virtual root set group is shared state.
CREATE TABLE logiqx_documents (
    edition_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_editions(edition_id),
    set_group_id INTEGER NOT NULL UNIQUE REFERENCES catalog_set_groups(set_group_id),
    build TEXT,
    debug TEXT NOT NULL DEFAULT 'no' CHECK (debug IN ('yes', 'no')),
    debug_was_present INTEGER NOT NULL CHECK (debug_was_present IN (0, 1)),
    extent_view TEXT CHECK (extent_view IS NULL OR extent_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    extent_start INTEGER,
    extent_end INTEGER,
    location_view TEXT CHECK (location_view IS NULL OR location_view IN ('retained_original_text', 'transport_decoded_xml_text')),
    start_line INTEGER,
    start_column INTEGER,
    end_line INTEGER,
    end_column INTEGER,
    column_convention TEXT CHECK (column_convention IS NULL OR column_convention = 'one_based_unicode_scalar'),
    CHECK ((debug_was_present = 1) OR debug = 'no'),
    CHECK ((extent_view IS NULL AND extent_start IS NULL AND extent_end IS NULL) OR
           (extent_view IS NOT NULL AND extent_start IS NOT NULL AND extent_end IS NOT NULL AND extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0)),
    CHECK (end_line IS NULL OR (end_line > 0 AND end_column > 0)),
    CHECK (start_line IS NULL OR end_line IS NULL OR end_line > start_line OR (end_line = start_line AND end_column >= start_column)),
    CHECK ((location_view IS NULL AND start_line IS NULL AND end_line IS NULL AND column_convention IS NULL) OR
           (location_view IS NOT NULL AND (start_line IS NOT NULL OR end_line IS NOT NULL) AND column_convention IS NOT NULL AND column_convention = 'one_based_unicode_scalar'))
) STRICT;

CREATE TABLE logiqx_root_file_names (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    file_name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id), UNIQUE (set_group_id, source_order)
) STRICT;

CREATE TABLE logiqx_root_sha1_elements (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    hash_id INTEGER NOT NULL REFERENCES hash_values(hash_id),
    hash_scope TEXT NOT NULL CHECK (hash_scope = 'unknown'),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id), UNIQUE (set_group_id, source_order)
) STRICT;

CREATE TRIGGER logiqx_root_sha1_algorithm_insert
BEFORE INSERT ON logiqx_root_sha1_elements
WHEN NOT EXISTS (
    SELECT 1 FROM hash_values
    WHERE hash_id = NEW.hash_id AND algorithm = 'sha1' AND length(bytes) = 20
)
BEGIN
    SELECT RAISE(ABORT, 'Logiqx root sha1 must reference a 20-byte SHA-1 value');
END;

CREATE TRIGGER logiqx_root_sha1_algorithm_update
BEFORE UPDATE OF hash_id ON logiqx_root_sha1_elements
WHEN NOT EXISTS (
    SELECT 1 FROM hash_values
    WHERE hash_id = NEW.hash_id AND algorithm = 'sha1' AND length(bytes) = 20
)
BEGIN
    SELECT RAISE(ABORT, 'Logiqx root sha1 must reference a 20-byte SHA-1 value');
END;

CREATE TABLE logiqx_headers (
    header_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id), UNIQUE (set_group_id, source_order)
) STRICT;

CREATE TABLE logiqx_clrmamepro_options (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES logiqx_headers(header_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    header TEXT,
    forcemerging TEXT NOT NULL DEFAULT 'split' CHECK (forcemerging IN ('split', 'none', 'full')),
    forcemerging_was_present INTEGER NOT NULL CHECK (forcemerging_was_present IN (0, 1)),
    forcenodump TEXT NOT NULL DEFAULT 'obsolete' CHECK (forcenodump IN ('obsolete', 'required', 'ignore')),
    forcenodump_was_present INTEGER NOT NULL CHECK (forcenodump_was_present IN (0, 1)),
    forcepacking TEXT NOT NULL DEFAULT 'zip' CHECK (forcepacking IN ('zip', 'unzip')),
    forcepacking_was_present INTEGER NOT NULL CHECK (forcepacking_was_present IN (0, 1)),
    CHECK ((forcemerging_was_present = 1) OR forcemerging = 'split'),
    CHECK ((forcenodump_was_present = 1) OR forcenodump = 'obsolete'),
    CHECK ((forcepacking_was_present = 1) OR forcepacking = 'zip')
) STRICT;

CREATE TABLE logiqx_romcenter_options (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES logiqx_headers(header_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    plugin TEXT,
    rommode TEXT NOT NULL DEFAULT 'split' CHECK (rommode IN ('split', 'merged', 'unmerged')),
    rommode_was_present INTEGER NOT NULL CHECK (rommode_was_present IN (0, 1)),
    biosmode TEXT NOT NULL DEFAULT 'split' CHECK (biosmode IN ('split', 'merged', 'unmerged')),
    biosmode_was_present INTEGER NOT NULL CHECK (biosmode_was_present IN (0, 1)),
    samplemode TEXT NOT NULL DEFAULT 'merged' CHECK (samplemode IN ('merged', 'unmerged')),
    samplemode_was_present INTEGER NOT NULL CHECK (samplemode_was_present IN (0, 1)),
    lockrommode TEXT NOT NULL DEFAULT 'no' CHECK (lockrommode IN ('no', 'yes')),
    lockrommode_was_present INTEGER NOT NULL CHECK (lockrommode_was_present IN (0, 1)),
    lockbiosmode TEXT NOT NULL DEFAULT 'no' CHECK (lockbiosmode IN ('no', 'yes')),
    lockbiosmode_was_present INTEGER NOT NULL CHECK (lockbiosmode_was_present IN (0, 1)),
    locksamplemode TEXT NOT NULL DEFAULT 'no' CHECK (locksamplemode IN ('no', 'yes')),
    locksamplemode_was_present INTEGER NOT NULL CHECK (locksamplemode_was_present IN (0, 1)),
    CHECK ((rommode_was_present = 1) OR rommode = 'split'),
    CHECK ((biosmode_was_present = 1) OR biosmode = 'split'),
    CHECK ((samplemode_was_present = 1) OR samplemode = 'merged'),
    CHECK ((lockrommode_was_present = 1) OR lockrommode = 'no'),
    CHECK ((lockbiosmode_was_present = 1) OR lockbiosmode = 'no'),
    CHECK ((locksamplemode_was_present = 1) OR locksamplemode = 'no')
) STRICT;

CREATE TABLE logiqx_header_text_elements (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL REFERENCES logiqx_headers(header_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 9),
    text_value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (header_id, field_kind), UNIQUE (header_id, source_order)
) STRICT;

CREATE TABLE logiqx_games (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_sets(set_id),
    sourcefile TEXT,
    isbios TEXT NOT NULL DEFAULT 'no' CHECK (isbios IN ('yes', 'no')),
    isbios_was_present INTEGER NOT NULL CHECK (isbios_was_present IN (0, 1)),
    board TEXT,
    rebuildto TEXT,
    CHECK ((isbios_was_present = 1) OR isbios = 'no')
) STRICT;

CREATE TABLE logiqx_game_text_elements (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    text_value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, field_kind), UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE logiqx_game_comments (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    comment_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE logiqx_set_links (
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof', 'romof', 'sampleof')),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (set_id, link_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_releases (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    language TEXT,
    date TEXT,
    "default" TEXT NOT NULL DEFAULT 'no' CHECK ("default" IN ('yes', 'no')),
    default_was_present INTEGER NOT NULL CHECK (default_was_present IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order), CHECK ((default_was_present = 1) OR "default" = 'no')
) STRICT;

CREATE TABLE logiqx_bios_sets (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    is_default TEXT NOT NULL DEFAULT 'no' CHECK (is_default IN ('yes', 'no')),
    default_was_present INTEGER NOT NULL CHECK (default_was_present IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order), CHECK ((default_was_present = 1) OR is_default = 'no')
) STRICT;

CREATE TABLE logiqx_archive_references (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    archive_name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE logiqx_device_references (
    source_element_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE logiqx_roms (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    name TEXT NOT NULL,
    size_text TEXT,
    status TEXT NOT NULL DEFAULT 'good' CHECK (status IN ('good', 'baddump', 'nodump', 'verified')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    date TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    size_value INTEGER GENERATED ALWAYS AS (
        CASE WHEN size_text IS NOT NULL AND size_text <> '' AND instr(size_text, char(0)) = 0
          AND size_text NOT GLOB '*[^0-9]*'
          AND (length(ltrim(size_text, '0')) < 19 OR
               (length(ltrim(size_text, '0')) = 19 AND ltrim(size_text, '0') <= '9223372036854775807'))
          THEN CAST(size_text AS INTEGER) END
    ) VIRTUAL,
    UNIQUE (set_id, source_order), CHECK ((status_specified = 1) OR status = 'good')
) STRICT;

CREATE TABLE logiqx_rom_compatibility (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES logiqx_roms(media_entry_id),
    serial TEXT NOT NULL
) STRICT;

CREATE TABLE logiqx_rom_merges (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES logiqx_roms(media_entry_id),
    merge_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

CREATE TABLE logiqx_disks (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'good' CHECK (status IN ('good', 'baddump', 'nodump', 'verified')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order), CHECK ((status_specified = 1) OR status = 'good')
) STRICT;

CREATE TABLE logiqx_disk_merges (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES logiqx_disks(media_entry_id),
    merge_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

CREATE TABLE logiqx_samples (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

-- Attribute ledgers. field_kind values are the exact local dictionaries in
-- MAMEC-DOC-11. Hash declaration values/positions are linked, never copied.
CREATE TABLE logiqx_document_attribute_positions (
    edition_id INTEGER NOT NULL REFERENCES logiqx_documents(edition_id),
    field_kind INTEGER NOT NULL CHECK (field_kind IN (0, 1)),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (edition_id, field_kind, field_occurrence), UNIQUE (edition_id, source_order),
    CHECK ((field_kind = 0 AND reported_hash_id IS NULL) OR (field_kind = 1 AND reported_hash_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_clrmamepro_option_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_clrmamepro_options(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 3),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_romcenter_option_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_romcenter_options(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 6),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_game_attribute_positions (
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 7),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (set_id, field_kind, field_occurrence), UNIQUE (set_id, source_order),
    CHECK (reported_hash_id IS NULL),
    CHECK ((field_kind IN (3, 4, 5) AND relationship_id IS NOT NULL) OR
           (field_kind NOT IN (3, 4, 5) AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_release_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_releases(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 4),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_bios_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_bios_sets(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_archive_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_archive_references(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind = 0),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_device_reference_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES logiqx_device_references(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind = 0),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence), UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_rom_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES logiqx_roms(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 8),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence), UNIQUE (media_entry_id, source_order),
    CHECK ((field_kind IN (2, 3, 4) AND reported_hash_id IS NOT NULL) OR
           (field_kind NOT IN (2, 3, 4) AND reported_hash_id IS NULL)),
    CHECK ((field_kind = 5 AND relationship_id IS NOT NULL) OR
           (field_kind <> 5 AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_disk_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES logiqx_disks(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 4),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence), UNIQUE (media_entry_id, source_order),
    CHECK ((field_kind IN (1, 2) AND reported_hash_id IS NOT NULL) OR
           (field_kind NOT IN (1, 2) AND reported_hash_id IS NULL)),
    CHECK ((field_kind = 3 AND relationship_id IS NOT NULL) OR
           (field_kind <> 3 AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE logiqx_sample_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES logiqx_samples(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind = 0),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence), UNIQUE (media_entry_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

-- ClrMamePro text document and physical decoded-DAT extent. Header presence
-- and comment count are seal facts handled by the parent finalizer.
CREATE TABLE clrmamepro_documents (
    edition_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_editions(edition_id),
    set_group_id INTEGER NOT NULL UNIQUE REFERENCES catalog_set_groups(set_group_id),
    header_present INTEGER NOT NULL CHECK (header_present IN (0, 1)),
    comment_count INTEGER NOT NULL CHECK (comment_count >= 0),
    extent_view TEXT CHECK (extent_view IS NULL OR extent_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    extent_start INTEGER,
    extent_end INTEGER,
    location_view TEXT CHECK (location_view IS NULL OR location_view IN ('retained_original_text', 'decoded_dat_text')),
    start_line INTEGER,
    start_column INTEGER,
    end_line INTEGER,
    end_column INTEGER,
    column_convention TEXT CHECK (column_convention IS NULL OR column_convention = 'one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL AND extent_start IS NULL AND extent_end IS NULL) OR
           (extent_view IS NOT NULL AND extent_start IS NOT NULL AND extent_end IS NOT NULL AND extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0)),
    CHECK (end_line IS NULL OR (end_line > 0 AND end_column > 0)),
    CHECK (start_line IS NULL OR end_line IS NULL OR end_line > start_line OR (end_line = start_line AND end_column >= start_column)),
    CHECK ((location_view IS NULL AND start_line IS NULL AND end_line IS NULL AND column_convention IS NULL) OR
           (location_view IS NOT NULL AND (start_line IS NOT NULL OR end_line IS NOT NULL) AND column_convention IS NOT NULL AND column_convention = 'one_based_unicode_scalar'))
) STRICT;

CREATE TABLE clrmamepro_headers (
    header_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    edition_id INTEGER NOT NULL UNIQUE REFERENCES clrmamepro_documents(edition_id),
    source_block TEXT NOT NULL CHECK (lower(source_block) = 'clrmamepro'),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    name TEXT, description TEXT, version TEXT, date TEXT, author TEXT,
    email TEXT, homepage TEXT, url TEXT, comment TEXT, category TEXT
) STRICT;

CREATE TABLE clrmamepro_header_options (
    header_id INTEGER PRIMARY KEY NOT NULL REFERENCES clrmamepro_headers(header_id),
    header_definition TEXT,
    forcemerging TEXT,
    forcezipping TEXT,
    forcepacking TEXT,
    forcenodump TEXT,
    forcemerging_effective TEXT GENERATED ALWAYS AS (
        CASE WHEN forcemerging IN ('none', 'split', 'full') THEN forcemerging END
    ) VIRTUAL,
    forcezipping_effective TEXT GENERATED ALWAYS AS (
        CASE WHEN forcezipping IN ('zip', 'unzip') THEN forcezipping END
    ) VIRTUAL,
    forcenodump_effective TEXT GENERATED ALWAYS AS (
        CASE WHEN forcenodump IS NULL THEN 'obsolete'
             WHEN forcenodump IN ('obsolete', 'required', 'ignore') THEN forcenodump END
    ) VIRTUAL
) STRICT;

-- One empty-capable options facet exists for every present header. It carries
-- no independent identity, order or location.
CREATE TRIGGER clrmamepro_header_options_after_header
AFTER INSERT ON clrmamepro_headers
BEGIN
    INSERT INTO clrmamepro_header_options(header_id) VALUES (NEW.header_id);
END;

CREATE TABLE clrmamepro_sets (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_sets(set_id),
    source_block TEXT NOT NULL CHECK (lower(source_block) IN ('game', 'set')),
    description TEXT,
    year TEXT,
    manufacturer TEXT,
    rebuildto TEXT,
    region TEXT,
    releaseyear TEXT,
    releasemonth TEXT,
    releaseday TEXT,
    serial TEXT
) STRICT;

CREATE TABLE clrmamepro_set_links (
    set_id INTEGER NOT NULL REFERENCES clrmamepro_sets(set_id),
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof', 'sampleof')),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (set_id, link_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE clrmamepro_roms (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES clrmamepro_sets(set_id),
    name TEXT NOT NULL,
    size_text TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    size_value INTEGER GENERATED ALWAYS AS (
        CASE WHEN size_text IS NOT NULL AND size_text <> '' AND instr(size_text, char(0)) = 0
          AND size_text NOT GLOB '*[^0-9]*'
          AND (length(ltrim(size_text, '0')) < 19 OR
               (length(ltrim(size_text, '0')) = 19 AND ltrim(size_text, '0') <= '9223372036854775807'))
          THEN CAST(size_text AS INTEGER) END
    ) VIRTUAL,
    UNIQUE (set_id, source_order),
    CHECK (size_text IS NULL OR (size_text <> '' AND size_text NOT GLOB '*[^0-9]*' AND instr(size_text, char(0)) = 0 AND
          (length(ltrim(size_text, '0')) < 19 OR
           (length(ltrim(size_text, '0')) = 19 AND ltrim(size_text, '0') <= '9223372036854775807'))))
) STRICT;

CREATE TABLE clrmamepro_rom_details (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES clrmamepro_roms(media_entry_id),
    date TEXT,
    serial TEXT,
    status_text TEXT
) STRICT;

-- Every ROM has exactly one detail facet; status/flag interpretation is
-- derived later from this text and the canonical flag position declarations.
CREATE TRIGGER clrmamepro_rom_details_after_rom
AFTER INSERT ON clrmamepro_roms
BEGIN
    INSERT INTO clrmamepro_rom_details(media_entry_id) VALUES (NEW.media_entry_id);
END;

CREATE TABLE clrmamepro_rom_merges (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES clrmamepro_roms(media_entry_id),
    merge_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

CREATE TABLE clrmamepro_samples (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES clrmamepro_sets(set_id),
    sample_name TEXT NOT NULL,
    source_field TEXT NOT NULL CHECK (lower(source_field) = 'sample'),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    keyword_line INTEGER NOT NULL CHECK (keyword_line > 0),
    keyword_column INTEGER NOT NULL CHECK (keyword_column > 0),
    value_line INTEGER NOT NULL CHECK (value_line > 0),
    value_column INTEGER NOT NULL CHECK (value_column > 0),
    value_is_quoted INTEGER NOT NULL CHECK (value_is_quoted IN (0, 1)),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE clrmamepro_comments (
    comment_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    edition_id INTEGER NOT NULL REFERENCES clrmamepro_documents(edition_id),
    comment_text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (edition_id, source_line, source_column)
) STRICT;

-- CMP code ledgers. `source_order` is one form/item order per declaration;
-- Keyword and Value are read-only token-anchor roles over the coordinates.
CREATE TABLE clrmamepro_header_field_positions (
    header_id INTEGER NOT NULL REFERENCES clrmamepro_headers(header_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 14),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    keyword TEXT NOT NULL CHECK (lower(keyword) = CASE field_kind
        WHEN 0 THEN 'name' WHEN 1 THEN 'description' WHEN 2 THEN 'version'
        WHEN 3 THEN 'date' WHEN 4 THEN 'author' WHEN 5 THEN 'email'
        WHEN 6 THEN 'homepage' WHEN 7 THEN 'url' WHEN 8 THEN 'comment'
        WHEN 9 THEN 'category' WHEN 10 THEN 'header' WHEN 11 THEN 'forcemerging'
        WHEN 12 THEN 'forcezipping' WHEN 13 THEN 'forcepacking' WHEN 14 THEN 'forcenodump' END),
    keyword_line INTEGER NOT NULL CHECK (keyword_line > 0),
    keyword_column INTEGER NOT NULL CHECK (keyword_column > 0),
    value_line INTEGER NOT NULL CHECK (value_line > 0),
    value_column INTEGER NOT NULL CHECK (value_column > 0),
    value_is_quoted INTEGER NOT NULL CHECK (value_is_quoted IN (0, 1)),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (header_id, field_kind, field_occurrence), UNIQUE (header_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE clrmamepro_set_field_positions (
    set_id INTEGER NOT NULL REFERENCES clrmamepro_sets(set_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 11),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    keyword TEXT NOT NULL CHECK (lower(keyword) = CASE field_kind
        WHEN 0 THEN 'name' WHEN 1 THEN 'cloneof' WHEN 2 THEN 'description'
        WHEN 3 THEN 'year' WHEN 4 THEN 'manufacturer' WHEN 5 THEN 'rebuildto'
        WHEN 6 THEN 'sampleof' WHEN 7 THEN 'region' WHEN 8 THEN 'releaseyear'
        WHEN 9 THEN 'releasemonth' WHEN 10 THEN 'releaseday' WHEN 11 THEN 'serial' END),
    keyword_line INTEGER NOT NULL CHECK (keyword_line > 0),
    keyword_column INTEGER NOT NULL CHECK (keyword_column > 0),
    value_line INTEGER NOT NULL CHECK (value_line > 0),
    value_column INTEGER NOT NULL CHECK (value_column > 0),
    value_is_quoted INTEGER NOT NULL CHECK (value_is_quoted IN (0, 1)),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (set_id, field_kind, field_occurrence), UNIQUE (set_id, source_order),
    CHECK (reported_hash_id IS NULL),
    CHECK ((field_kind IN (1, 6) AND relationship_id IS NOT NULL) OR
           (field_kind NOT IN (1, 6) AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE clrmamepro_rom_field_positions (
    media_entry_id INTEGER NOT NULL REFERENCES clrmamepro_roms(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 11),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    keyword TEXT NOT NULL CHECK (lower(keyword) = CASE field_kind
        WHEN 0 THEN 'name' WHEN 1 THEN 'size' WHEN 2 THEN 'crc' WHEN 3 THEN 'crc32'
        WHEN 4 THEN 'md5' WHEN 5 THEN 'sha1' WHEN 6 THEN 'merge' WHEN 7 THEN 'date'
        WHEN 8 THEN 'serial' WHEN 9 THEN 'status' WHEN 10 THEN 'nodump' WHEN 11 THEN 'baddump' END),
    keyword_line INTEGER NOT NULL CHECK (keyword_line > 0),
    keyword_column INTEGER NOT NULL CHECK (keyword_column > 0),
    value_line INTEGER,
    value_column INTEGER,
    value_is_quoted INTEGER NOT NULL CHECK (value_is_quoted IN (0, 1)),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence), UNIQUE (media_entry_id, source_order),
    CHECK ((field_kind IN (10, 11) AND value_line IS NULL AND value_column IS NULL AND value_is_quoted = 0) OR
           (field_kind NOT IN (10, 11) AND value_line IS NOT NULL AND value_column IS NOT NULL AND value_line > 0 AND value_column > 0)),
    CHECK ((field_kind IN (2, 3, 4, 5) AND reported_hash_id IS NOT NULL) OR
           (field_kind NOT IN (2, 3, 4, 5) AND reported_hash_id IS NULL)),
    CHECK ((field_kind = 6 AND relationship_id IS NOT NULL) OR
           (field_kind <> 6 AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;

CREATE VIEW clrmamepro_position_tokens AS
SELECT 'header' AS owner_kind, header_id AS owner_id, field_kind, field_occurrence,
       'Keyword' AS role, keyword AS token_text, keyword_line AS source_line, keyword_column AS source_column,
       NULL AS reported_hash_id, NULL AS relationship_id
FROM clrmamepro_header_field_positions
UNION ALL
SELECT 'header', header_id, field_kind, field_occurrence,
       'Value', NULL, value_line, value_column, NULL, NULL FROM clrmamepro_header_field_positions
UNION ALL
SELECT 'set', set_id, field_kind, field_occurrence,
       'Keyword', keyword, keyword_line, keyword_column, NULL, NULL FROM clrmamepro_set_field_positions
UNION ALL
SELECT 'set', set_id, field_kind, field_occurrence,
       'Value', NULL, value_line, value_column, NULL, relationship_id FROM clrmamepro_set_field_positions
UNION ALL
SELECT 'rom', media_entry_id, field_kind, field_occurrence,
       'Keyword', keyword, keyword_line, keyword_column, NULL, NULL FROM clrmamepro_rom_field_positions
UNION ALL
SELECT 'rom', media_entry_id, field_kind, field_occurrence,
       'Value', NULL, value_line, value_column, reported_hash_id, relationship_id FROM clrmamepro_rom_field_positions
WHERE value_line IS NOT NULL
UNION ALL
SELECT 'sample', media_entry_id, 0, 0, 'Keyword', source_field, keyword_line, keyword_column, NULL, NULL
FROM clrmamepro_samples
UNION ALL
SELECT 'sample', media_entry_id, 0, 0, 'Value', NULL, value_line, value_column, NULL, NULL
FROM clrmamepro_samples;

-- Bounded, read-only local integrity report. The parent assembler adds
-- cross-fragment kind/edition/registry, source-bound, publication and count
-- seal checks; this view checks this family’s own exact field and order facts.
CREATE VIEW candidate_logiqx_cmp_integrity_problems AS
WITH
expected_positions(owner_kind, owner_id, field_kind) AS (
    SELECT 'logiqx_document', edition_id, 0 FROM logiqx_documents WHERE build IS NOT NULL
    UNION ALL SELECT 'logiqx_document', edition_id, 1 FROM logiqx_documents WHERE debug_was_present = 1
    UNION ALL SELECT 'logiqx_clrmamepro_options', source_element_id, 0 FROM logiqx_clrmamepro_options WHERE header IS NOT NULL
    UNION ALL SELECT 'logiqx_clrmamepro_options', source_element_id, 1 FROM logiqx_clrmamepro_options WHERE forcemerging_was_present = 1
    UNION ALL SELECT 'logiqx_clrmamepro_options', source_element_id, 2 FROM logiqx_clrmamepro_options WHERE forcenodump_was_present = 1
    UNION ALL SELECT 'logiqx_clrmamepro_options', source_element_id, 3 FROM logiqx_clrmamepro_options WHERE forcepacking_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 0 FROM logiqx_romcenter_options WHERE plugin IS NOT NULL
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 1 FROM logiqx_romcenter_options WHERE rommode_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 2 FROM logiqx_romcenter_options WHERE biosmode_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 3 FROM logiqx_romcenter_options WHERE samplemode_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 4 FROM logiqx_romcenter_options WHERE lockrommode_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 5 FROM logiqx_romcenter_options WHERE lockbiosmode_was_present = 1
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, 6 FROM logiqx_romcenter_options WHERE locksamplemode_was_present = 1
    UNION ALL SELECT 'logiqx_game', set_id, 0 FROM logiqx_games
    UNION ALL SELECT 'logiqx_game', set_id, 1 FROM logiqx_games WHERE sourcefile IS NOT NULL
    UNION ALL SELECT 'logiqx_game', set_id, 2 FROM logiqx_games WHERE isbios_was_present = 1
    UNION ALL SELECT 'logiqx_game', set_id, 3 FROM logiqx_set_links WHERE link_kind = 'cloneof'
    UNION ALL SELECT 'logiqx_game', set_id, 4 FROM logiqx_set_links WHERE link_kind = 'romof'
    UNION ALL SELECT 'logiqx_game', set_id, 5 FROM logiqx_set_links WHERE link_kind = 'sampleof'
    UNION ALL SELECT 'logiqx_game', set_id, 6 FROM logiqx_games WHERE board IS NOT NULL
    UNION ALL SELECT 'logiqx_game', set_id, 7 FROM logiqx_games WHERE rebuildto IS NOT NULL
    UNION ALL SELECT 'logiqx_release', source_element_id, 4 FROM logiqx_releases WHERE default_was_present = 1
    UNION ALL SELECT 'logiqx_bios', source_element_id, 2 FROM logiqx_bios_sets WHERE default_was_present = 1
    UNION ALL SELECT 'logiqx_archive', source_element_id, 0 FROM logiqx_archive_references
    UNION ALL SELECT 'logiqx_device', source_element_id, 0 FROM logiqx_device_references
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 0 FROM logiqx_roms
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 1 FROM logiqx_roms WHERE size_text IS NOT NULL
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 5 FROM logiqx_rom_merges
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 6 FROM logiqx_roms WHERE status_specified = 1
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 7 FROM logiqx_roms WHERE date IS NOT NULL
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 8 FROM logiqx_rom_compatibility
    UNION ALL SELECT 'logiqx_disk', media_entry_id, 0 FROM logiqx_disks
    UNION ALL SELECT 'logiqx_disk', media_entry_id, 3 FROM logiqx_disk_merges
    UNION ALL SELECT 'logiqx_disk', media_entry_id, 4 FROM logiqx_disks WHERE status_specified = 1
    UNION ALL SELECT 'logiqx_sample', media_entry_id, 0 FROM logiqx_samples
    UNION ALL SELECT 'logiqx_header_text', source_element_id, field_kind FROM logiqx_header_text_elements
    UNION ALL SELECT 'logiqx_game_text', source_element_id, field_kind FROM logiqx_game_text_elements
    UNION ALL SELECT 'logiqx_release', source_element_id, 0 FROM logiqx_releases
    UNION ALL SELECT 'logiqx_release', source_element_id, 1 FROM logiqx_releases
    UNION ALL SELECT 'logiqx_release', source_element_id, 2 FROM logiqx_releases WHERE language IS NOT NULL
    UNION ALL SELECT 'logiqx_release', source_element_id, 3 FROM logiqx_releases WHERE date IS NOT NULL
    UNION ALL SELECT 'logiqx_bios', source_element_id, 0 FROM logiqx_bios_sets
    UNION ALL SELECT 'logiqx_bios', source_element_id, 1 FROM logiqx_bios_sets
    UNION ALL SELECT 'logiqx_rom', media_entry_id, field_kind FROM logiqx_entry_hash_expected
    UNION ALL SELECT 'logiqx_disk', media_entry_id, field_kind FROM logiqx_disk_hash_expected
    UNION ALL SELECT 'clrmamepro_header', header_id, field_kind FROM cmp_header_position_expected
    UNION ALL SELECT 'clrmamepro_set', set_id, field_kind FROM cmp_set_position_expected
    UNION ALL SELECT 'clrmamepro_rom', media_entry_id, field_kind FROM cmp_rom_position_expected
),
logiqx_entry_hash_expected(media_entry_id, field_kind) AS (
    SELECT h.media_entry_id, CASE h.source_hash_field WHEN 'crc' THEN 2 WHEN 'sha1' THEN 3 WHEN 'md5' THEN 4 END
    FROM catalog_entry_hashes h JOIN logiqx_roms r USING (media_entry_id)
    WHERE h.source_hash_field IN ('crc','sha1','md5')
),
logiqx_disk_hash_expected(media_entry_id, field_kind) AS (
    SELECT h.media_entry_id, CASE h.source_hash_field WHEN 'sha1' THEN 1 WHEN 'md5' THEN 2 END
    FROM catalog_entry_hashes h JOIN logiqx_disks d USING (media_entry_id)
    WHERE h.source_hash_field IN ('sha1','md5')
),
cmp_header_position_expected(header_id, field_kind) AS (
    SELECT header_id, 0 FROM clrmamepro_headers WHERE name IS NOT NULL
    UNION ALL SELECT header_id, 1 FROM clrmamepro_headers WHERE description IS NOT NULL
    UNION ALL SELECT header_id, 2 FROM clrmamepro_headers WHERE version IS NOT NULL
    UNION ALL SELECT header_id, 3 FROM clrmamepro_headers WHERE date IS NOT NULL
    UNION ALL SELECT header_id, 4 FROM clrmamepro_headers WHERE author IS NOT NULL
    UNION ALL SELECT header_id, 5 FROM clrmamepro_headers WHERE email IS NOT NULL
    UNION ALL SELECT header_id, 6 FROM clrmamepro_headers WHERE homepage IS NOT NULL
    UNION ALL SELECT header_id, 7 FROM clrmamepro_headers WHERE url IS NOT NULL
    UNION ALL SELECT header_id, 8 FROM clrmamepro_headers WHERE comment IS NOT NULL
    UNION ALL SELECT header_id, 9 FROM clrmamepro_headers WHERE category IS NOT NULL
    UNION ALL SELECT h.header_id, 10 FROM clrmamepro_header_options o JOIN clrmamepro_headers h USING (header_id) WHERE o.header_definition IS NOT NULL
    UNION ALL SELECT h.header_id, 11 FROM clrmamepro_header_options o JOIN clrmamepro_headers h USING (header_id) WHERE o.forcemerging IS NOT NULL
    UNION ALL SELECT h.header_id, 12 FROM clrmamepro_header_options o JOIN clrmamepro_headers h USING (header_id) WHERE o.forcezipping IS NOT NULL
    UNION ALL SELECT h.header_id, 13 FROM clrmamepro_header_options o JOIN clrmamepro_headers h USING (header_id) WHERE o.forcepacking IS NOT NULL
    UNION ALL SELECT h.header_id, 14 FROM clrmamepro_header_options o JOIN clrmamepro_headers h USING (header_id) WHERE o.forcenodump IS NOT NULL
),
cmp_set_position_expected(set_id, field_kind) AS (
    SELECT set_id, 0 FROM clrmamepro_sets
    UNION ALL SELECT set_id, 1 FROM clrmamepro_set_links WHERE link_kind = 'cloneof'
    UNION ALL SELECT set_id, 2 FROM clrmamepro_sets WHERE description IS NOT NULL
    UNION ALL SELECT set_id, 3 FROM clrmamepro_sets WHERE year IS NOT NULL
    UNION ALL SELECT set_id, 4 FROM clrmamepro_sets WHERE manufacturer IS NOT NULL
    UNION ALL SELECT set_id, 5 FROM clrmamepro_sets WHERE rebuildto IS NOT NULL
    UNION ALL SELECT set_id, 6 FROM clrmamepro_set_links WHERE link_kind = 'sampleof'
    UNION ALL SELECT set_id, 7 FROM clrmamepro_sets WHERE region IS NOT NULL
    UNION ALL SELECT set_id, 8 FROM clrmamepro_sets WHERE releaseyear IS NOT NULL
    UNION ALL SELECT set_id, 9 FROM clrmamepro_sets WHERE releasemonth IS NOT NULL
    UNION ALL SELECT set_id, 10 FROM clrmamepro_sets WHERE releaseday IS NOT NULL
    UNION ALL SELECT set_id, 11 FROM clrmamepro_sets WHERE serial IS NOT NULL
),
cmp_rom_position_expected(media_entry_id, field_kind) AS (
    SELECT media_entry_id, 0 FROM clrmamepro_roms
    UNION ALL SELECT media_entry_id, 1 FROM clrmamepro_roms WHERE size_text IS NOT NULL
    UNION ALL SELECT media_entry_id, CASE source_hash_field WHEN 'crc' THEN 2 WHEN 'crc32' THEN 3 WHEN 'md5' THEN 4 WHEN 'sha1' THEN 5 END
      FROM catalog_entry_hashes h JOIN clrmamepro_roms r USING (media_entry_id)
      WHERE source_hash_field IN ('crc','crc32','md5','sha1')
    UNION ALL SELECT media_entry_id, 6 FROM clrmamepro_rom_merges
    UNION ALL SELECT media_entry_id, 7 FROM clrmamepro_rom_details WHERE date IS NOT NULL
    UNION ALL SELECT media_entry_id, 8 FROM clrmamepro_rom_details WHERE serial IS NOT NULL
    UNION ALL SELECT media_entry_id, 9 FROM clrmamepro_rom_details WHERE status_text IS NOT NULL
    UNION ALL SELECT media_entry_id, field_kind FROM clrmamepro_rom_field_positions WHERE field_kind IN (10,11)
),
actual_positions(owner_kind, owner_id, field_kind) AS (
    SELECT 'logiqx_document', edition_id, field_kind FROM logiqx_document_attribute_positions
    UNION ALL SELECT 'logiqx_clrmamepro_options', source_element_id, field_kind FROM logiqx_clrmamepro_option_positions
    UNION ALL SELECT 'logiqx_romcenter_options', source_element_id, field_kind FROM logiqx_romcenter_option_positions
    UNION ALL SELECT 'logiqx_game', set_id, field_kind FROM logiqx_game_attribute_positions
    UNION ALL SELECT 'logiqx_release', source_element_id, field_kind FROM logiqx_release_attribute_positions
    UNION ALL SELECT 'logiqx_bios', source_element_id, field_kind FROM logiqx_bios_attribute_positions
    UNION ALL SELECT 'logiqx_archive', source_element_id, field_kind FROM logiqx_archive_attribute_positions
    UNION ALL SELECT 'logiqx_device', source_element_id, field_kind FROM logiqx_device_reference_attribute_positions
    UNION ALL SELECT 'logiqx_rom', media_entry_id, field_kind FROM logiqx_rom_attribute_positions
    UNION ALL SELECT 'logiqx_disk', media_entry_id, field_kind FROM logiqx_disk_attribute_positions
    UNION ALL SELECT 'logiqx_sample', media_entry_id, field_kind FROM logiqx_sample_attribute_positions
    UNION ALL SELECT 'logiqx_header_text', source_element_id, field_kind FROM logiqx_header_text_elements
    UNION ALL SELECT 'logiqx_game_text', source_element_id, field_kind FROM logiqx_game_text_elements
    UNION ALL SELECT 'clrmamepro_header', header_id, field_kind FROM clrmamepro_header_field_positions
    UNION ALL SELECT 'clrmamepro_set', set_id, field_kind FROM clrmamepro_set_field_positions
    UNION ALL SELECT 'clrmamepro_rom', media_entry_id, field_kind FROM clrmamepro_rom_field_positions
),
expected_relationship_positions(owner_kind, owner_id, field_kind, relationship_id) AS (
    SELECT 'logiqx_game', set_id,
           CASE link_kind WHEN 'cloneof' THEN 3 WHEN 'romof' THEN 4 WHEN 'sampleof' THEN 5 END,
           relationship_id FROM logiqx_set_links
    UNION ALL SELECT 'logiqx_device', source_element_id, 0, relationship_id FROM logiqx_device_references
    UNION ALL SELECT 'logiqx_rom', media_entry_id, 5, relationship_id FROM logiqx_rom_merges
    UNION ALL SELECT 'logiqx_disk', media_entry_id, 3, relationship_id FROM logiqx_disk_merges
    UNION ALL SELECT 'clrmamepro_set', set_id, CASE link_kind WHEN 'cloneof' THEN 1 WHEN 'sampleof' THEN 6 END,
           relationship_id FROM clrmamepro_set_links
    UNION ALL SELECT 'clrmamepro_rom', media_entry_id, 6, relationship_id FROM clrmamepro_rom_merges
),
actual_relationship_positions(owner_kind, owner_id, field_kind, relationship_id) AS (
    SELECT 'logiqx_game', set_id, field_kind, relationship_id FROM logiqx_game_attribute_positions WHERE relationship_id IS NOT NULL
    UNION ALL SELECT 'logiqx_device', source_element_id, field_kind, relationship_id FROM logiqx_device_reference_attribute_positions
    UNION ALL SELECT 'logiqx_rom', media_entry_id, field_kind, relationship_id FROM logiqx_rom_attribute_positions WHERE relationship_id IS NOT NULL
    UNION ALL SELECT 'logiqx_disk', media_entry_id, field_kind, relationship_id FROM logiqx_disk_attribute_positions WHERE relationship_id IS NOT NULL
    UNION ALL SELECT 'clrmamepro_set', set_id, field_kind, relationship_id FROM clrmamepro_set_field_positions WHERE relationship_id IS NOT NULL
    UNION ALL SELECT 'clrmamepro_rom', media_entry_id, field_kind, relationship_id FROM clrmamepro_rom_field_positions WHERE relationship_id IS NOT NULL
),
native_hash_positions(family, media_entry_id, reported_hash_id, field_occurrence, expected_source_hash_field) AS (
    SELECT 'logiqx', r.media_entry_id, p.reported_hash_id, p.field_occurrence,
           CASE p.field_kind WHEN 2 THEN 'crc' WHEN 3 THEN 'sha1' WHEN 4 THEN 'md5' END
      FROM logiqx_rom_attribute_positions p JOIN logiqx_roms r USING (media_entry_id)
      WHERE p.reported_hash_id IS NOT NULL
    UNION ALL SELECT 'logiqx', d.media_entry_id, p.reported_hash_id, p.field_occurrence,
           CASE p.field_kind WHEN 1 THEN 'sha1' WHEN 2 THEN 'md5' END
      FROM logiqx_disk_attribute_positions p JOIN logiqx_disks d USING (media_entry_id)
      WHERE p.reported_hash_id IS NOT NULL
    UNION ALL SELECT 'clrmamepro', r.media_entry_id, p.reported_hash_id, p.field_occurrence,
           CASE p.field_kind WHEN 2 THEN 'crc' WHEN 3 THEN 'crc32' WHEN 4 THEN 'md5' WHEN 5 THEN 'sha1' END
      FROM clrmamepro_rom_field_positions p JOIN clrmamepro_roms r USING (media_entry_id)
      WHERE p.reported_hash_id IS NOT NULL
),
placements(domain, parent_key, owner_id, source_order) AS (
    SELECT 'logiqx-root', set_group_id, source_element_id, source_order FROM logiqx_root_file_names
    UNION ALL SELECT 'logiqx-root', set_group_id, source_element_id, source_order FROM logiqx_root_sha1_elements
    UNION ALL SELECT 'logiqx-root', set_group_id, header_id, source_order FROM logiqx_headers
    UNION ALL SELECT 'logiqx-root', set_group_id, set_id, source_order FROM catalog_sets
      WHERE set_group_id IN (SELECT set_group_id FROM logiqx_documents)
    UNION ALL SELECT 'logiqx-header', header_id, source_element_id, source_order FROM logiqx_clrmamepro_options
    UNION ALL SELECT 'logiqx-header', header_id, source_element_id, source_order FROM logiqx_romcenter_options
    UNION ALL SELECT 'logiqx-header', header_id, source_element_id, source_order FROM logiqx_header_text_elements
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_game_text_elements
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_game_comments
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_releases
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_bios_sets
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_archive_references
    UNION ALL SELECT 'logiqx-game', set_id, source_element_id, source_order FROM logiqx_device_references
    UNION ALL SELECT 'logiqx-game', set_id, media_entry_id, source_order FROM logiqx_roms
    UNION ALL SELECT 'logiqx-game', set_id, media_entry_id, source_order FROM logiqx_disks
    UNION ALL SELECT 'logiqx-game', set_id, media_entry_id, source_order FROM logiqx_samples
    UNION ALL SELECT 'cmp-document', edition_id, header_id, source_order FROM clrmamepro_headers
    UNION ALL SELECT 'cmp-document', g.edition_id, s.set_id, s.source_order FROM catalog_sets s JOIN catalog_set_groups g USING (set_group_id)
      WHERE g.set_group_id IN (SELECT set_group_id FROM clrmamepro_documents)
    UNION ALL SELECT 'cmp-set', set_id, field_kind, source_order FROM clrmamepro_set_field_positions
    UNION ALL SELECT 'cmp-set', set_id, media_entry_id, source_order FROM clrmamepro_roms
    UNION ALL SELECT 'cmp-set', set_id, media_entry_id, source_order FROM clrmamepro_samples
),
cmp_lexical_anchors(edition_id, source_line, source_column, anchor_kind, owner_kind, owner_id, field_kind, field_occurrence) AS (
    SELECT h.edition_id, p.keyword_line, p.keyword_column, 'keyword', 'header', p.header_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_header_field_positions p JOIN clrmamepro_headers h USING (header_id)
    UNION ALL SELECT h.edition_id, p.value_line, p.value_column, 'value', 'header', p.header_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_header_field_positions p JOIN clrmamepro_headers h USING (header_id)
    UNION ALL SELECT g.edition_id, p.keyword_line, p.keyword_column, 'keyword', 'set', p.set_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_set_field_positions p JOIN clrmamepro_sets s USING (set_id)
      JOIN catalog_sets c USING (set_id) JOIN catalog_set_groups g USING (set_group_id)
    UNION ALL SELECT g.edition_id, p.value_line, p.value_column, 'value', 'set', p.set_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_set_field_positions p JOIN clrmamepro_sets s USING (set_id)
      JOIN catalog_sets c USING (set_id) JOIN catalog_set_groups g USING (set_group_id)
    UNION ALL SELECT h.edition_id, p.keyword_line, p.keyword_column, 'keyword', 'rom', p.media_entry_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_rom_field_positions p JOIN clrmamepro_roms r USING (media_entry_id)
      JOIN clrmamepro_sets s USING (set_id) JOIN catalog_sets c USING (set_id) JOIN catalog_set_groups h USING (set_group_id)
    UNION ALL SELECT h.edition_id, p.value_line, p.value_column, 'value', 'rom', p.media_entry_id, p.field_kind, p.field_occurrence
      FROM clrmamepro_rom_field_positions p JOIN clrmamepro_roms r USING (media_entry_id)
      JOIN clrmamepro_sets s USING (set_id) JOIN catalog_sets c USING (set_id) JOIN catalog_set_groups h USING (set_group_id)
      WHERE p.value_line IS NOT NULL
    UNION ALL SELECT h.edition_id, s.keyword_line, s.keyword_column, 'keyword', 'sample', s.media_entry_id, 0, 0
      FROM clrmamepro_samples s JOIN clrmamepro_sets n ON n.set_id = s.set_id
      JOIN catalog_sets c ON c.set_id = n.set_id JOIN catalog_set_groups h USING (set_group_id)
    UNION ALL SELECT h.edition_id, s.value_line, s.value_column, 'value', 'sample', s.media_entry_id, 0, 0
      FROM clrmamepro_samples s JOIN clrmamepro_sets n ON n.set_id = s.set_id
      JOIN catalog_sets c ON c.set_id = n.set_id JOIN catalog_set_groups h USING (set_group_id)
    UNION ALL SELECT c.edition_id, c.source_line, c.source_column, 'comment', 'comment', c.comment_id, 0, 0 FROM clrmamepro_comments c
),
cmp_set_item_anchors(set_id, source_order, source_line, source_column, item_kind, item_id) AS (
    SELECT set_id, source_order, keyword_line, keyword_column, 'scalar', field_kind
      FROM clrmamepro_set_field_positions
    UNION ALL SELECT set_id, source_order, source_line, source_column, 'rom', media_entry_id
      FROM clrmamepro_roms
    UNION ALL SELECT set_id, source_order, keyword_line, keyword_column, 'sample', media_entry_id
      FROM clrmamepro_samples
),
cmp_document_item_anchors(edition_id, source_order, source_line, source_column, item_kind, item_id) AS (
    SELECT edition_id, source_order, source_line, source_column, 'header', header_id
      FROM clrmamepro_headers
    UNION ALL SELECT g.edition_id, s.source_order, s.source_line, s.source_column, 'set', s.set_id
      FROM catalog_sets s JOIN catalog_set_groups g USING (set_group_id)
      WHERE g.set_group_id IN (SELECT set_group_id FROM clrmamepro_documents)
),
cmp_nested_field_anchors(owner_kind, owner_id, source_order, source_line, source_column, field_kind) AS (
    SELECT 'header', header_id, source_order, keyword_line, keyword_column, field_kind
      FROM clrmamepro_header_field_positions
    UNION ALL SELECT 'rom', media_entry_id, source_order, keyword_line, keyword_column, field_kind
      FROM clrmamepro_rom_field_positions
),
problems(problem, edition_id, owner_id, detail) AS (
    SELECT 'missing_position', NULL, e.owner_id, e.owner_kind || ':' || e.field_kind
      FROM expected_positions e LEFT JOIN actual_positions a USING (owner_kind, owner_id, field_kind)
      WHERE a.owner_id IS NULL
    UNION ALL SELECT 'unexpected_position', NULL, a.owner_id, a.owner_kind || ':' || a.field_kind
      FROM actual_positions a LEFT JOIN expected_positions e USING (owner_kind, owner_id, field_kind)
      WHERE e.owner_id IS NULL
    UNION ALL SELECT 'relationship_position_mismatch', s.edition_id, e.owner_id, e.owner_kind || ':' || e.field_kind
      FROM expected_relationship_positions e
      LEFT JOIN actual_relationship_positions a USING (owner_kind, owner_id, field_kind)
      LEFT JOIN catalog_source_elements s ON s.source_element_id=e.owner_id
      WHERE e.relationship_id IS NOT a.relationship_id
    UNION ALL SELECT 'unexpected_relationship_position', s.edition_id, a.owner_id, a.owner_kind || ':' || a.field_kind
      FROM actual_relationship_positions a
      LEFT JOIN expected_relationship_positions e USING (owner_kind, owner_id, field_kind)
      LEFT JOIN catalog_source_elements s ON s.source_element_id=a.owner_id
      WHERE e.owner_id IS NULL
    UNION ALL SELECT 'bad_logiqx_rules', d.edition_id, d.edition_id, r.rules_version
      FROM logiqx_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id)
      WHERE r.format_family <> 'logiqx' OR r.rules_version NOT IN ('logiqx-declared-text-compat-v2','logiqx-dtd-1.5-v1')
    UNION ALL SELECT 'bad_cmp_rules', d.edition_id, d.edition_id, r.rules_version
      FROM clrmamepro_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id)
      WHERE r.format_family <> 'clrmamepro' OR r.rules_version <> 'clrmamepro-declared-text-compat-v1'
    UNION ALL SELECT 'strict_logiqx_game_count', d.edition_id, d.edition_id, 'strict requires one or more games'
      FROM logiqx_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id)
      WHERE r.rules_version = 'logiqx-dtd-1.5-v1'
        AND NOT EXISTS (SELECT 1 FROM catalog_sets s JOIN logiqx_games g USING (set_id) WHERE s.set_group_id = d.set_group_id)
    UNION ALL SELECT 'strict_logiqx_extension', d.edition_id, d.edition_id, 'compatible-only root/device/serial owner in strict edition'
      FROM logiqx_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id)
      WHERE r.rules_version = 'logiqx-dtd-1.5-v1' AND (
        EXISTS (SELECT 1 FROM logiqx_root_file_names x WHERE x.set_group_id = d.set_group_id) OR
        EXISTS (SELECT 1 FROM logiqx_root_sha1_elements x WHERE x.set_group_id = d.set_group_id) OR
        EXISTS (SELECT 1 FROM logiqx_device_references x JOIN logiqx_games g USING (set_id) JOIN catalog_sets s USING (set_id) WHERE s.set_group_id = d.set_group_id) OR
        EXISTS (SELECT 1 FROM logiqx_rom_compatibility x JOIN logiqx_roms m USING (media_entry_id) JOIN catalog_sets s ON s.set_id=m.set_id WHERE s.set_group_id=d.set_group_id))
    UNION ALL SELECT 'missing_strict_header_text', d.edition_id, h.header_id, 'required name/description/version/author'
      FROM logiqx_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id) JOIN logiqx_headers h USING (set_group_id)
      WHERE r.rules_version = 'logiqx-dtd-1.5-v1'
        AND EXISTS (SELECT 1 FROM (SELECT 0 AS k UNION ALL SELECT 1 UNION ALL SELECT 3 UNION ALL SELECT 5) req
                    WHERE NOT EXISTS (SELECT 1 FROM logiqx_header_text_elements t WHERE t.header_id=h.header_id AND t.field_kind=req.k))
    UNION ALL SELECT 'missing_required_logiqx_header_name', d.edition_id, h.header_id, 'compatible header needs name'
      FROM logiqx_documents d JOIN catalog_editions e USING (edition_id)
      JOIN catalog_reading_rules r USING (reading_rules_id) JOIN logiqx_headers h USING (set_group_id)
      WHERE r.rules_version = 'logiqx-declared-text-compat-v2'
        AND NOT EXISTS (SELECT 1 FROM logiqx_header_text_elements t WHERE t.header_id=h.header_id AND t.field_kind=0)
    UNION ALL SELECT 'missing_strict_game_description', NULL, g.set_id, 'strict game needs description'
      FROM logiqx_games g WHERE NOT EXISTS (SELECT 1 FROM logiqx_game_text_elements t WHERE t.set_id=g.set_id AND t.field_kind=0)
       AND EXISTS (SELECT 1 FROM catalog_sets s JOIN logiqx_documents d USING (set_group_id)
                   JOIN catalog_editions e USING (edition_id) JOIN catalog_reading_rules r USING (reading_rules_id)
                   WHERE s.set_id=g.set_id AND r.rules_version='logiqx-dtd-1.5-v1')
    UNION ALL SELECT 'missing_strict_rom_size', NULL, r.media_entry_id, 'strict ROM needs size attribute'
      FROM logiqx_roms r JOIN catalog_sets s USING (set_id) JOIN logiqx_documents d USING (set_group_id)
      JOIN catalog_editions e USING (edition_id) JOIN catalog_reading_rules rr USING (reading_rules_id)
      WHERE rr.rules_version='logiqx-dtd-1.5-v1' AND r.size_text IS NULL
    UNION ALL SELECT 'root_group_edition_or_kind', d.edition_id, d.set_group_id, 'root group must belong to edition and have root kind'
      FROM logiqx_documents d LEFT JOIN catalog_set_groups g ON g.set_group_id=d.set_group_id AND g.edition_id=d.edition_id AND g.group_kind='root'
      WHERE g.set_group_id IS NULL
    UNION ALL SELECT 'root_group_edition_or_kind', d.edition_id, d.set_group_id, 'CMP root group must belong to edition and have root kind'
      FROM clrmamepro_documents d LEFT JOIN catalog_set_groups g ON g.set_group_id=d.set_group_id AND g.edition_id=d.edition_id AND g.group_kind='root'
      WHERE g.set_group_id IS NULL
    UNION ALL SELECT 'missing_physical_root_diagnostic', d.edition_id, d.edition_id, 'root requires at least one recorded extent or location system'
      FROM logiqx_documents d WHERE d.extent_view IS NULL AND d.location_view IS NULL
    UNION ALL SELECT 'missing_physical_root_diagnostic', d.edition_id, d.edition_id, 'CMP document requires at least one recorded extent or location system'
      FROM clrmamepro_documents d WHERE d.extent_view IS NULL AND d.location_view IS NULL
    UNION ALL SELECT 'cmp_header_presence_seal', d.edition_id, d.edition_id, 'header_present differs from typed header cardinality'
      FROM clrmamepro_documents d WHERE d.header_present <> EXISTS(SELECT 1 FROM clrmamepro_headers h WHERE h.edition_id=d.edition_id)
    UNION ALL SELECT 'cmp_comment_count_seal', d.edition_id, d.edition_id, 'comment_count differs from retained comment cardinality'
      FROM clrmamepro_documents d WHERE d.comment_count <> (SELECT count(*) FROM clrmamepro_comments c WHERE c.edition_id=d.edition_id)
    UNION ALL SELECT 'missing_cmp_header_options', h.edition_id, h.header_id, 'present header lacks its one keyed options facet'
      FROM clrmamepro_headers h WHERE NOT EXISTS (SELECT 1 FROM clrmamepro_header_options o WHERE o.header_id=h.header_id)
    UNION ALL SELECT 'missing_cmp_rom_details', NULL, r.media_entry_id, 'present ROM lacks its one keyed detail facet'
      FROM clrmamepro_roms r WHERE NOT EXISTS (SELECT 1 FROM clrmamepro_rom_details d WHERE d.media_entry_id=r.media_entry_id)
    UNION ALL SELECT 'logiqx_root_mixed_order_collision', NULL, p.owner_id, 'duplicate source_order in root domain'
      FROM placements p WHERE p.domain='logiqx-root' GROUP BY p.parent_key, p.source_order HAVING count(*) > 1
    UNION ALL SELECT 'logiqx_header_mixed_order_collision', NULL, p.owner_id, 'duplicate source_order in header domain'
      FROM placements p WHERE p.domain='logiqx-header' GROUP BY p.parent_key, p.source_order HAVING count(*) > 1
    UNION ALL SELECT 'logiqx_game_mixed_order_collision', NULL, p.owner_id, 'duplicate source_order in game domain'
      FROM placements p WHERE p.domain='logiqx-game' GROUP BY p.parent_key, p.source_order HAVING count(*) > 1
    UNION ALL SELECT 'cmp_document_mixed_order_collision', NULL, p.owner_id, 'duplicate source_order in document-form domain'
      FROM placements p WHERE p.domain='cmp-document' GROUP BY p.parent_key, p.source_order HAVING count(*) > 1
    UNION ALL SELECT 'cmp_set_item_mixed_order_collision', NULL, p.owner_id, 'duplicate source_order across fields/ROMs/samples'
      FROM placements p WHERE p.domain='cmp-set' GROUP BY p.parent_key, p.source_order HAVING count(*) > 1
    UNION ALL SELECT 'cmp_lexical_coordinate_collision', a.edition_id, a.owner_id, 'two token/comment anchors start at the same coordinate'
      FROM cmp_lexical_anchors a GROUP BY a.edition_id,a.source_line,a.source_column HAVING count(*) > 1
    UNION ALL SELECT 'cmp_set_item_order_inversion', NULL, a.set_id, a.item_kind || ':' || a.item_id || ' precedes a later source_order item lexically'
      FROM cmp_set_item_anchors a JOIN cmp_set_item_anchors b ON b.set_id=a.set_id AND a.source_order < b.source_order
      WHERE a.source_line > b.source_line OR (a.source_line=b.source_line AND a.source_column >= b.source_column)
    UNION ALL SELECT 'cmp_document_form_order_inversion', a.edition_id, a.item_id, a.item_kind || ' precedes a later form lexically'
      FROM cmp_document_item_anchors a JOIN cmp_document_item_anchors b ON b.edition_id=a.edition_id AND a.source_order < b.source_order
      WHERE a.source_line > b.source_line OR (a.source_line=b.source_line AND a.source_column >= b.source_column)
    UNION ALL SELECT 'cmp_nested_field_order_inversion', NULL, a.owner_id, a.owner_kind || ' field ' || a.field_kind || ' precedes a later field lexically'
      FROM cmp_nested_field_anchors a JOIN cmp_nested_field_anchors b
        ON b.owner_kind=a.owner_kind AND b.owner_id=a.owner_id AND a.source_order < b.source_order
      WHERE a.source_line > b.source_line OR (a.source_line=b.source_line AND a.source_column >= b.source_column)
    UNION ALL SELECT 'cmp_keyword_value_order', a.edition_id, a.owner_id, 'keyword coordinate must precede value coordinate'
      FROM cmp_lexical_anchors a JOIN cmp_lexical_anchors b ON b.edition_id=a.edition_id AND b.owner_id=a.owner_id
        AND b.anchor_kind='value' AND a.anchor_kind='keyword'
      WHERE a.owner_kind=b.owner_kind AND a.field_kind=b.field_kind AND a.field_occurrence=b.field_occurrence
        AND (a.source_line > b.source_line OR (a.source_line=b.source_line AND a.source_column >= b.source_column))
    UNION ALL SELECT CASE p.family WHEN 'logiqx' THEN 'logiqx_hash_mapping' ELSE 'cmp_hash_mapping' END,
           s.edition_id, p.reported_hash_id, 'hash declaration field/owner/occurrence differs from its canonical position'
      FROM native_hash_positions p JOIN catalog_entry_hashes h USING (reported_hash_id)
      JOIN catalog_source_elements s ON s.source_element_id=p.media_entry_id
      WHERE h.source_hash_field IS NOT p.expected_source_hash_field OR h.media_entry_id <> p.media_entry_id
         OR h.field_occurrence <> p.field_occurrence
    UNION ALL SELECT 'cmp_invalid_hash_state', s.edition_id, h.reported_hash_id,
           'CMP compatible-v1 accepts supplied hashes only in valid value state'
      FROM catalog_entry_hashes h JOIN clrmamepro_roms r USING (media_entry_id)
      JOIN catalog_source_elements s ON s.source_element_id=r.media_entry_id
      WHERE h.source_hash_field IN ('crc','crc32','md5','sha1') AND h.presence <> 'value'
)
SELECT problem, edition_id, owner_id, detail FROM problems;
