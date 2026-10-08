-- Candidate-only MAME software-list native relations.
-- Shared catalog_editions, catalog_source_elements, catalog_set_groups,
-- catalog_sets, catalog_media_entries, catalog_entry_hashes,
-- reported_catalog_relationships and hash_values are supplied by shared.sql
-- plus the relationships.sql fragment.

CREATE TABLE software_documents (
    edition_id INTEGER PRIMARY KEY REFERENCES catalog_editions(edition_id),
    envelope_kind TEXT NOT NULL CHECK (envelope_kind IN ('single_list', 'plural_lists')),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start >= 0),
    extent_end INTEGER CHECK (extent_end >= 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text', 'transport_decoded_xml_text')),
    start_line INTEGER CHECK (start_line > 0),
    start_column INTEGER CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line > 0),
    end_column INTEGER CHECK (end_column > 0),
    column_convention TEXT CHECK (column_convention IN ('one_based_unicode_scalar')),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK (extent_start IS NULL OR (extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((location_view IS NULL) = (start_line IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((start_column IS NULL) = (end_line IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (column_convention IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0 AND end_line > 0 AND end_column > 0
        AND (end_line > start_line OR (end_line = start_line AND end_column > start_column)))),
    CHECK (extent_view IS NOT NULL OR location_view IS NOT NULL)
) STRICT;

-- A plural physical root is real; the bare <softwarelist> root has no wrapper.
CREATE TABLE software_wrapper_headers (
    wrapper_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL UNIQUE REFERENCES software_documents(edition_id),
    build TEXT,
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start >= 0),
    extent_end INTEGER CHECK (extent_end >= 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text', 'transport_decoded_xml_text')),
    start_line INTEGER CHECK (start_line > 0),
    start_column INTEGER CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line > 0),
    end_column INTEGER CHECK (end_column > 0),
    column_convention TEXT CHECK (column_convention IN ('one_based_unicode_scalar')),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK (extent_start IS NULL OR (extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((location_view IS NULL) = (start_line IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((start_column IS NULL) = (end_line IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (column_convention IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0 AND end_line > 0 AND end_column > 0
        AND (end_line > start_line OR (end_line = start_line AND end_column > start_column)))),
    CHECK (extent_view IS NOT NULL OR location_view IS NOT NULL),
    UNIQUE (wrapper_id, edition_id)
) STRICT;

-- Canonical list facts are also the set-group facts in bare and wrapped modes.
CREATE TABLE software_lists (
    set_group_id INTEGER PRIMARY KEY REFERENCES catalog_set_groups(set_group_id),
    name TEXT NOT NULL,
    description TEXT,
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start >= 0),
    extent_end INTEGER CHECK (extent_end >= 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text', 'transport_decoded_xml_text')),
    start_line INTEGER CHECK (start_line > 0),
    start_column INTEGER CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line > 0),
    end_column INTEGER CHECK (end_column > 0),
    column_convention TEXT CHECK (column_convention IN ('one_based_unicode_scalar')),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK (extent_start IS NULL OR (extent_start >= 0 AND extent_end > extent_start)),
    CHECK ((location_view IS NULL) = (start_line IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((start_column IS NULL) = (end_line IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (column_convention IS NULL)),
    CHECK (start_line IS NULL OR (start_line > 0 AND start_column > 0 AND end_line > 0 AND end_column > 0
        AND (end_line > start_line OR (end_line = start_line AND end_column > start_column)))),
    CHECK (extent_view IS NOT NULL OR location_view IS NOT NULL)
) STRICT;

-- Exists only for an actual <softwarelist> child of <softwarelists>.
CREATE TABLE software_list_wrapper_entries (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL UNIQUE REFERENCES software_lists(set_group_id),
    wrapper_id INTEGER NOT NULL REFERENCES software_wrapper_headers(wrapper_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    UNIQUE (wrapper_id, source_order)
) STRICT;

CREATE TABLE software_titles (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_sets(set_id),
    supported TEXT NOT NULL CHECK (supported IN ('yes', 'partial', 'no')),
    supported_specified INTEGER NOT NULL CHECK (supported_specified IN (0, 1)),
    CHECK (supported_specified = 1 OR supported = 'yes')
) STRICT;

CREATE TABLE software_list_notes (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES software_lists(set_group_id),
    text_value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id),
    UNIQUE (set_group_id, source_order)
) STRICT;

CREATE TABLE software_title_text_elements (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES software_titles(set_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 3),
    text_value TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, field_kind),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE software_title_info (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES software_titles(set_id),
    name TEXT NOT NULL,
    value TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE software_shared_features (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES software_titles(set_id),
    name TEXT NOT NULL,
    value TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE software_parts (
    part_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES software_titles(set_id),
    name TEXT NOT NULL,
    interface TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_id, source_order)
) STRICT;

CREATE TABLE software_clone_links (
    set_id INTEGER PRIMARY KEY REFERENCES software_titles(set_id),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

CREATE TABLE software_part_features (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    part_id INTEGER NOT NULL REFERENCES software_parts(part_id),
    name TEXT NOT NULL,
    value TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (part_id, source_order)
) STRICT;

CREATE TABLE software_part_switches (
    switch_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    part_id INTEGER NOT NULL REFERENCES software_parts(part_id),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (part_id, source_order)
) STRICT;

CREATE TABLE software_part_switch_values (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    switch_id INTEGER NOT NULL REFERENCES software_part_switches(switch_id),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    default_specified INTEGER NOT NULL CHECK (default_specified IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    CHECK (default_specified = 1 OR is_default = 0),
    UNIQUE (switch_id, source_order)
) STRICT;

CREATE TABLE software_areas (
    area_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    part_id INTEGER NOT NULL REFERENCES software_parts(part_id),
    kind TEXT NOT NULL CHECK (kind IN ('data', 'disk')),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (part_id, source_order)
) STRICT;

CREATE TABLE software_data_areas (
    area_id INTEGER PRIMARY KEY REFERENCES software_areas(area_id),
    name TEXT NOT NULL,
    declared_size_text TEXT NOT NULL,
    width INTEGER NOT NULL CHECK (width IN (8, 16, 32, 64)),
    width_specified INTEGER NOT NULL CHECK (width_specified IN (0, 1)),
    endianness TEXT NOT NULL CHECK (endianness IN ('little', 'big')),
    endianness_specified INTEGER NOT NULL CHECK (endianness_specified IN (0, 1)),
    CHECK (width_specified = 1 OR width = 8),
    CHECK (endianness_specified = 1 OR endianness = 'little')
) STRICT;

CREATE TABLE software_disk_areas (
    area_id INTEGER PRIMARY KEY REFERENCES software_areas(area_id),
    name TEXT NOT NULL
) STRICT;

CREATE TABLE software_rom_load_entries (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    data_area_id INTEGER NOT NULL REFERENCES software_data_areas(area_id),
    name TEXT,
    size_text TEXT,
    offset_text TEXT,
    value TEXT,
    loadflag TEXT CHECK (loadflag IS NULL OR loadflag IN (
        'reload', 'reload_plain', 'continue', 'ignore', 'fill',
        'load16_byte', 'load16_word', 'load16_word_swap',
        'load32_byte', 'load32_word', 'load32_word_swap', 'load32_dword',
        'load64_word', 'load64_word_swap'
    )),
    status TEXT NOT NULL CHECK (status IN ('good', 'baddump', 'nodump')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    CHECK (status_specified = 1 OR status = 'good'),
    UNIQUE (data_area_id, source_order)
) STRICT;

CREATE TABLE software_disks (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    disk_area_id INTEGER NOT NULL REFERENCES software_disk_areas(area_id),
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('good', 'baddump', 'nodump')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    writeable INTEGER NOT NULL CHECK (writeable IN (0, 1)),
    writeable_specified INTEGER NOT NULL CHECK (writeable_specified IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    CHECK (status_specified = 1 OR status = 'good'),
    CHECK (writeable_specified = 1 OR writeable = 0),
    UNIQUE (disk_area_id, source_order)
) STRICT;

CREATE TABLE software_required_files (
    required_file_id INTEGER PRIMARY KEY,
    first_load_entry_id INTEGER NOT NULL UNIQUE REFERENCES software_rom_load_entries(media_entry_id)
) STRICT;

CREATE TABLE software_file_load_steps (
    load_entry_id INTEGER PRIMARY KEY REFERENCES software_rom_load_entries(media_entry_id),
    required_file_id INTEGER REFERENCES software_required_files(required_file_id)
) STRICT;

-- Thirteen position-only QName ledgers. field_occurrence is zero for every
-- accepted singleton attribute; source_order is local to that XML element.
CREATE TABLE software_wrapper_attribute_positions (
    wrapper_id INTEGER NOT NULL REFERENCES software_wrapper_headers(wrapper_id),
    field_kind INTEGER NOT NULL CHECK (field_kind = 0),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (wrapper_id, field_kind, field_occurrence),
    UNIQUE (wrapper_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_list_attribute_positions (
    set_group_id INTEGER NOT NULL REFERENCES software_lists(set_group_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 1),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (set_group_id, field_kind, field_occurrence),
    UNIQUE (set_group_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_title_attribute_positions (
    set_id INTEGER NOT NULL REFERENCES software_titles(set_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (set_id, field_kind, field_occurrence),
    UNIQUE (set_id, source_order),
    CHECK (reported_hash_id IS NULL),
    CHECK ((field_kind = 1) = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE software_title_info_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES software_title_info(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 1),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence),
    UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_shared_feature_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES software_shared_features(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 1),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence),
    UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_part_attribute_positions (
    part_id INTEGER NOT NULL REFERENCES software_parts(part_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 1),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (part_id, field_kind, field_occurrence),
    UNIQUE (part_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_part_feature_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES software_part_features(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 1),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence),
    UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_part_switch_attribute_positions (
    switch_id INTEGER NOT NULL REFERENCES software_part_switches(switch_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (switch_id, field_kind, field_occurrence),
    UNIQUE (switch_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_part_switch_value_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES software_part_switch_values(source_element_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (source_element_id, field_kind, field_occurrence),
    UNIQUE (source_element_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_data_area_attribute_positions (
    area_id INTEGER NOT NULL REFERENCES software_data_areas(area_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 3),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (area_id, field_kind, field_occurrence),
    UNIQUE (area_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_disk_area_attribute_positions (
    area_id INTEGER NOT NULL REFERENCES software_disk_areas(area_id),
    field_kind INTEGER NOT NULL CHECK (field_kind = 0),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (area_id, field_kind, field_occurrence),
    UNIQUE (area_id, source_order),
    CHECK (reported_hash_id IS NULL)
) STRICT, WITHOUT ROWID;

CREATE TABLE software_rom_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES software_rom_load_entries(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 7),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence),
    UNIQUE (media_entry_id, source_order),
    CHECK ((field_kind IN (2, 3)) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE software_disk_attribute_positions (
    media_entry_id INTEGER NOT NULL REFERENCES software_disks(media_entry_id),
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 3),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY (media_entry_id, field_kind, field_occurrence),
    UNIQUE (media_entry_id, source_order),
    CHECK ((field_kind = 1) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;

-- Set-based software closure audit. The assembler can union this into its
-- candidate_integrity_problems view without per-row history scans.
CREATE VIEW candidate_software_integrity_problems AS
WITH software_editions AS (
    SELECT edition.edition_id
    FROM catalog_editions AS edition
    JOIN catalog_reading_rules AS rules USING (reading_rules_id)
    WHERE rules.format_family = 'software'
), roots AS (
    SELECT 'document' AS owner_kind, document.edition_id AS owner_id,
           document.edition_id,
           document.extent_view, document.extent_start, document.extent_end,
           document.location_view, document.start_line, document.start_column,
           document.end_line, document.end_column, document.column_convention
    FROM software_documents AS document
    UNION ALL
    SELECT 'wrapper', wrapper.wrapper_id, wrapper.edition_id,
           wrapper.extent_view, wrapper.extent_start, wrapper.extent_end,
           wrapper.location_view, wrapper.start_line, wrapper.start_column,
           wrapper.end_line, wrapper.end_column, wrapper.column_convention
    FROM software_wrapper_headers AS wrapper
    UNION ALL
    SELECT 'list', list.set_group_id, groups.edition_id,
           list.extent_view, list.extent_start, list.extent_end,
           list.location_view, list.start_line, list.start_column,
           list.end_line, list.end_column, list.column_convention
    FROM software_lists AS list
    JOIN catalog_set_groups AS groups USING (set_group_id)
), root_edges AS (
    SELECT 'document_list' AS edge_kind, document.edition_id,
           'document' AS parent_kind, document.edition_id AS parent_id,
           'list' AS child_kind, list.set_group_id AS child_id
    FROM software_documents AS document
    JOIN software_lists AS list
    JOIN catalog_set_groups AS groups ON groups.set_group_id = list.set_group_id
    WHERE document.envelope_kind = 'single_list'
      AND groups.edition_id = document.edition_id
    UNION ALL
    SELECT 'document_wrapper', document.edition_id,
           'document', document.edition_id, 'wrapper', wrapper.wrapper_id
    FROM software_documents AS document
    JOIN software_wrapper_headers AS wrapper ON wrapper.edition_id = document.edition_id
    WHERE document.envelope_kind = 'plural_lists'
    UNION ALL
    SELECT 'wrapper_list', wrapper.edition_id,
           'wrapper', wrapper.wrapper_id, 'list', placement.set_group_id
    FROM software_list_wrapper_entries AS placement
    JOIN software_wrapper_headers AS wrapper USING (wrapper_id)
    JOIN software_lists AS list USING (set_group_id)
), rom_operations AS (
    SELECT rom.media_entry_id, rom.data_area_id, rom.source_order, rom.loadflag,
           CASE WHEN rom.loadflag IS NULL OR rom.loadflag IN (
               'load16_byte', 'load16_word', 'load16_word_swap',
               'load32_byte', 'load32_word', 'load32_word_swap',
               'load32_dword', 'load64_word', 'load64_word_swap'
           ) THEN 'load' ELSE rom.loadflag END AS operation
    FROM software_rom_load_entries AS rom
), hash_slots AS (
    SELECT rom.media_entry_id, 2 AS field_kind, 'crc' AS source_hash_field,
           hash.field_occurrence, hash.reported_hash_id
    FROM software_rom_load_entries AS rom
    JOIN catalog_entry_hashes AS hash USING (media_entry_id)
    WHERE hash.source_hash_field = 'crc'
    UNION ALL
    SELECT rom.media_entry_id, 3, 'sha1', hash.field_occurrence, hash.reported_hash_id
    FROM software_rom_load_entries AS rom
    JOIN catalog_entry_hashes AS hash USING (media_entry_id)
    WHERE hash.source_hash_field = 'sha1'
    UNION ALL
    SELECT disk.media_entry_id, 1, 'sha1', hash.field_occurrence, hash.reported_hash_id
    FROM software_disks AS disk
    JOIN catalog_entry_hashes AS hash USING (media_entry_id)
    WHERE hash.source_hash_field = 'sha1'
)
SELECT 'missing_software_document' AS problem, software_editions.edition_id AS owner_id,
       software_editions.edition_id
FROM software_editions
LEFT JOIN software_documents USING (edition_id)
WHERE software_documents.edition_id IS NULL
UNION ALL
SELECT 'software_document_wrong_format', document.edition_id, document.edition_id
FROM software_documents AS document
JOIN catalog_editions AS edition USING (edition_id)
LEFT JOIN catalog_reading_rules AS rules USING (reading_rules_id)
WHERE rules.format_family IS NOT 'software'
UNION ALL
SELECT 'software_group_kind_or_missing_list', groups.set_group_id, groups.edition_id
FROM catalog_set_groups AS groups
JOIN software_documents AS document ON document.edition_id = groups.edition_id
LEFT JOIN software_lists AS list USING (set_group_id)
WHERE groups.group_kind <> 'software_list' OR list.set_group_id IS NULL
UNION ALL
SELECT 'software_list_wrong_group_or_edition', list.set_group_id, groups.edition_id
FROM software_lists AS list
LEFT JOIN catalog_set_groups AS groups USING (set_group_id)
LEFT JOIN software_documents AS document ON document.edition_id = groups.edition_id
WHERE groups.group_kind IS NOT 'software_list' OR document.edition_id IS NULL
UNION ALL
SELECT 'software_list_without_title', list.set_group_id, groups.edition_id
FROM software_lists AS list
JOIN catalog_set_groups AS groups USING (set_group_id)
WHERE NOT EXISTS (SELECT 1 FROM catalog_sets AS set_entry
                  JOIN software_titles AS title ON title.set_id = set_entry.set_id
                  WHERE set_entry.set_group_id = list.set_group_id)
UNION ALL
SELECT 'software_title_required_text_missing', title.set_id, groups.edition_id
FROM software_titles AS title
JOIN catalog_sets AS set_entry ON set_entry.set_id = title.set_id
JOIN catalog_set_groups AS groups USING (set_group_id)
WHERE NOT EXISTS (SELECT 1 FROM software_title_text_elements AS text
                  WHERE text.set_id = title.set_id AND text.field_kind = 0)
   OR NOT EXISTS (SELECT 1 FROM software_title_text_elements AS text
                  WHERE text.set_id = title.set_id AND text.field_kind = 1)
   OR NOT EXISTS (SELECT 1 FROM software_title_text_elements AS text
                  WHERE text.set_id = title.set_id AND text.field_kind = 2)
UNION ALL
SELECT 'software_envelope_mode_closure', document.edition_id, document.edition_id
FROM software_documents AS document
WHERE (document.envelope_kind = 'single_list' AND (
           (SELECT count(*) FROM catalog_set_groups AS groups
            WHERE groups.edition_id = document.edition_id AND groups.group_kind = 'software_list') <> 1
        OR EXISTS (SELECT 1 FROM software_wrapper_headers AS wrapper
                   WHERE wrapper.edition_id = document.edition_id)
        OR EXISTS (SELECT 1 FROM software_list_wrapper_entries AS placement
                   JOIN catalog_set_groups AS groups USING (set_group_id)
                   WHERE groups.edition_id = document.edition_id)))
   OR (document.envelope_kind = 'plural_lists' AND (
           NOT EXISTS (SELECT 1 FROM software_wrapper_headers AS wrapper
                       WHERE wrapper.edition_id = document.edition_id)
        OR EXISTS (SELECT 1 FROM catalog_set_groups AS groups
                   JOIN software_lists AS list USING (set_group_id)
                   LEFT JOIN software_list_wrapper_entries AS placement USING (set_group_id)
                   WHERE groups.edition_id = document.edition_id
                     AND groups.group_kind = 'software_list'
                     AND placement.source_element_id IS NULL)))
UNION ALL
SELECT 'software_wrapper_mode_or_edition', wrapper.wrapper_id, wrapper.edition_id
FROM software_wrapper_headers AS wrapper
LEFT JOIN software_documents AS document USING (edition_id)
WHERE document.envelope_kind IS NOT 'plural_lists'
UNION ALL
SELECT 'software_list_placement_closure', placement.source_element_id, groups.edition_id
FROM software_list_wrapper_entries AS placement
LEFT JOIN software_wrapper_headers AS wrapper USING (wrapper_id)
LEFT JOIN software_lists AS list USING (set_group_id)
LEFT JOIN catalog_set_groups AS groups USING (set_group_id)
LEFT JOIN software_documents AS document ON document.edition_id = groups.edition_id
LEFT JOIN catalog_source_elements AS element
       ON element.source_element_id = placement.source_element_id
WHERE wrapper.wrapper_id IS NULL OR list.set_group_id IS NULL
   OR groups.group_kind IS NOT 'software_list'
   OR document.envelope_kind IS NOT 'plural_lists'
   OR wrapper.edition_id IS NOT groups.edition_id
   OR element.element_kind IS NOT 'software_list_wrapper_entry'
   OR element.edition_id IS NOT groups.edition_id
UNION ALL
SELECT 'software_root_unmapped_extent:' || edge.edge_kind, edge.child_id, edge.edition_id
FROM root_edges AS edge
JOIN roots AS parent ON parent.owner_kind = edge.parent_kind AND parent.owner_id = edge.parent_id
JOIN roots AS child ON child.owner_kind = edge.child_kind AND child.owner_id = edge.child_id
WHERE NOT (
    (parent.extent_view IS NOT NULL AND parent.extent_view = child.extent_view)
    OR (parent.location_view IS NOT NULL AND parent.location_view = child.location_view
        AND parent.column_convention = child.column_convention))
UNION ALL
SELECT 'software_root_byte_extent_escape:' || edge.edge_kind, edge.child_id, edge.edition_id
FROM root_edges AS edge
JOIN roots AS parent ON parent.owner_kind = edge.parent_kind AND parent.owner_id = edge.parent_id
JOIN roots AS child ON child.owner_kind = edge.child_kind AND child.owner_id = edge.child_id
WHERE parent.extent_view = child.extent_view AND parent.extent_view IS NOT NULL
  AND NOT (parent.extent_start <= child.extent_start AND child.extent_end <= parent.extent_end)
UNION ALL
SELECT 'software_root_coordinate_extent_escape:' || edge.edge_kind, edge.child_id, edge.edition_id
FROM root_edges AS edge
JOIN roots AS parent ON parent.owner_kind = edge.parent_kind AND parent.owner_id = edge.parent_id
JOIN roots AS child ON child.owner_kind = edge.child_kind AND child.owner_id = edge.child_id
WHERE parent.location_view = child.location_view
  AND parent.column_convention = child.column_convention
  AND parent.location_view IS NOT NULL
  AND NOT ((parent.start_line, parent.start_column) <= (child.start_line, child.start_column)
       AND (child.end_line, child.end_column) <= (parent.end_line, parent.end_column))
UNION ALL
SELECT 'software_area_subtype_closure', area.area_id, element.edition_id
FROM software_areas AS area
LEFT JOIN software_data_areas AS data USING (area_id)
LEFT JOIN software_disk_areas AS disk USING (area_id)
LEFT JOIN catalog_source_elements AS element ON element.source_element_id = area.area_id
WHERE (area.kind = 'data' AND (data.area_id IS NULL OR disk.area_id IS NOT NULL))
   OR (area.kind = 'disk' AND (disk.area_id IS NULL OR data.area_id IS NOT NULL))
UNION ALL
SELECT 'software_rom_missing_load_step', rom.media_entry_id, element.edition_id
FROM software_rom_load_entries AS rom
LEFT JOIN software_file_load_steps AS step ON step.load_entry_id = rom.media_entry_id
LEFT JOIN catalog_source_elements AS element ON element.source_element_id = rom.media_entry_id
WHERE step.load_entry_id IS NULL
UNION ALL
SELECT 'software_rom_requirement_link', operation.media_entry_id, element.edition_id
FROM rom_operations AS operation
LEFT JOIN software_file_load_steps AS step ON step.load_entry_id = operation.media_entry_id
LEFT JOIN software_required_files AS own_requirement
       ON own_requirement.first_load_entry_id = operation.media_entry_id
LEFT JOIN catalog_source_elements AS element ON element.source_element_id = operation.media_entry_id
WHERE (operation.operation = 'load' AND (
           own_requirement.required_file_id IS NULL
        OR step.required_file_id IS NOT own_requirement.required_file_id))
   OR (operation.operation = 'fill' AND (
           own_requirement.required_file_id IS NOT NULL OR step.required_file_id IS NOT NULL))
   OR (operation.operation IN ('continue', 'reload', 'reload_plain', 'ignore') AND (
           own_requirement.required_file_id IS NOT NULL OR step.required_file_id IS NOT (
               SELECT prior_step.required_file_id
               FROM rom_operations AS prior
               JOIN software_file_load_steps AS prior_step
                 ON prior_step.load_entry_id = prior.media_entry_id
               JOIN software_required_files AS prior_requirement
                 ON prior_requirement.required_file_id = prior_step.required_file_id
                AND prior_requirement.first_load_entry_id = prior.media_entry_id
               WHERE prior.data_area_id = operation.data_area_id
                 AND prior.operation = 'load'
                 AND prior.source_order < operation.source_order
                 AND prior.source_order > coalesce((
                     SELECT max(fill.source_order)
                     FROM rom_operations AS fill
                     WHERE fill.data_area_id = operation.data_area_id
                       AND fill.operation = 'fill'
                       AND fill.source_order < operation.source_order
                 ), -1)
               ORDER BY prior.source_order DESC LIMIT 1)))
UNION ALL
SELECT 'software_required_file_not_first_load', requirement.required_file_id, element.edition_id
FROM software_required_files AS requirement
LEFT JOIN rom_operations AS first_load ON first_load.media_entry_id = requirement.first_load_entry_id
LEFT JOIN software_file_load_steps AS step ON step.load_entry_id = requirement.first_load_entry_id
LEFT JOIN catalog_source_elements AS element
       ON element.source_element_id = requirement.first_load_entry_id
WHERE first_load.media_entry_id IS NULL OR first_load.operation <> 'load'
   OR step.required_file_id IS NOT requirement.required_file_id
UNION ALL
SELECT 'software_clone_position_link_mismatch', link.set_id, element.edition_id
FROM software_clone_links AS link
LEFT JOIN software_title_attribute_positions AS position
       ON position.set_id = link.set_id AND position.field_kind = 1
      AND position.field_occurrence = 0
LEFT JOIN reported_catalog_relationships AS reported
       ON reported.relationship_id = link.relationship_id
LEFT JOIN catalog_source_elements AS element ON element.source_element_id = link.set_id
WHERE position.relationship_id IS NOT link.relationship_id
   OR reported.reported_kind IS NOT 'software_cloneof'
UNION ALL
SELECT 'software_clone_position_without_link', position.set_id, element.edition_id
FROM software_title_attribute_positions AS position
LEFT JOIN software_clone_links AS link USING (set_id)
LEFT JOIN catalog_source_elements AS element ON element.source_element_id = position.set_id
WHERE position.field_kind = 1 AND link.set_id IS NULL
UNION ALL
SELECT 'software_hash_without_matching_position', hash.reported_hash_id, element.edition_id
FROM catalog_entry_hashes AS hash
JOIN catalog_source_elements AS element ON element.source_element_id = hash.media_entry_id
LEFT JOIN software_rom_attribute_positions AS rom_position
       ON rom_position.media_entry_id = hash.media_entry_id
      AND rom_position.field_kind = CASE hash.source_hash_field WHEN 'crc' THEN 2 ELSE 3 END
      AND rom_position.field_occurrence = hash.field_occurrence
      AND rom_position.reported_hash_id = hash.reported_hash_id
LEFT JOIN software_disk_attribute_positions AS disk_position
       ON disk_position.media_entry_id = hash.media_entry_id
      AND disk_position.field_kind = 1
      AND disk_position.field_occurrence = hash.field_occurrence
      AND disk_position.reported_hash_id = hash.reported_hash_id
WHERE (element.element_kind = 'software_rom_entry'
       AND hash.source_hash_field IN ('crc', 'sha1')
       AND rom_position.reported_hash_id IS NULL)
   OR (element.element_kind = 'software_disk_entry'
       AND hash.source_hash_field = 'sha1'
       AND disk_position.reported_hash_id IS NULL)
   OR (element.element_kind IN ('software_rom_entry', 'software_disk_entry')
       AND hash.source_hash_field NOT IN ('crc', 'sha1'))
UNION ALL
SELECT 'software_hash_position_without_matching_declaration', position.reported_hash_id,
       element.edition_id
FROM software_rom_attribute_positions AS position
JOIN catalog_source_elements AS element ON element.source_element_id = position.media_entry_id
LEFT JOIN catalog_entry_hashes AS hash
       ON hash.reported_hash_id = position.reported_hash_id
      AND hash.media_entry_id = position.media_entry_id
      AND hash.source_hash_field = CASE position.field_kind WHEN 2 THEN 'crc' WHEN 3 THEN 'sha1' END
      AND hash.field_occurrence = position.field_occurrence
WHERE position.field_kind IN (2, 3) AND hash.reported_hash_id IS NULL
UNION ALL
SELECT 'software_disk_hash_position_without_matching_declaration', position.reported_hash_id,
       element.edition_id
FROM software_disk_attribute_positions AS position
JOIN catalog_source_elements AS element ON element.source_element_id = position.media_entry_id
LEFT JOIN catalog_entry_hashes AS hash
       ON hash.reported_hash_id = position.reported_hash_id
      AND hash.media_entry_id = position.media_entry_id
      AND hash.source_hash_field = 'sha1'
      AND hash.field_occurrence = position.field_occurrence
WHERE position.field_kind = 1 AND hash.reported_hash_id IS NULL;
