-- No-Intro candidate DDL. Candidate only; not approved for implementation.
-- Requires shared.sql to be applied first. Integer IDs are internal source-element
-- identities; source text remains on one typed owner and external source bytes stay external.

-- Flat No-Intro DAT v3/v4 ----------------------------------------------------

CREATE TABLE no_intro_dat_documents (
    edition_id INTEGER PRIMARY KEY REFERENCES catalog_editions(edition_id),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    location_view TEXT NOT NULL CHECK (location_view = 'transport_decoded_xml_text'),
    start_line INTEGER NOT NULL CHECK (start_line > 0),
    start_column INTEGER NOT NULL CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line > 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column > 0),
    column_convention TEXT NOT NULL CHECK (column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK (start_line IS NOT NULL),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK (location_view IS NOT NULL AND column_convention IS NOT NULL AND
           column_convention='one_based_unicode_scalar')
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_headers (
    header_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL UNIQUE REFERENCES catalog_set_groups(set_group_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_dat_headers_group_order ON no_intro_dat_headers(set_group_id, source_order, header_id);

CREATE TABLE no_intro_dat_header_text_children (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL REFERENCES no_intro_dat_headers(header_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'id','name','description','version','date','author','homepage','url',
        'trademarks','piracy','subset','comment'
    )),
    value_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE(header_id, field_kind),
    UNIQUE(header_id, source_order)
) STRICT;
CREATE INDEX no_intro_dat_header_text_kind ON no_intro_dat_header_text_children(header_id, field_kind, source_order);

CREATE TABLE no_intro_dat_clrmamepro_options (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_dat_headers(header_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    forcenodump TEXT,
    header TEXT
) STRICT;
CREATE INDEX no_intro_dat_clrmamepro_header_order ON no_intro_dat_clrmamepro_options(header_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_romcenter_options (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_dat_headers(header_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    plugin TEXT
) STRICT;
CREATE INDEX no_intro_dat_romcenter_header_order ON no_intro_dat_romcenter_options(header_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_games (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_sets(set_id),
    publisher_id_text TEXT
) STRICT;

CREATE TABLE no_intro_dat_set_links (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof','cloneofid')),
    target_literal TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(set_id, link_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_game_descriptions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    description_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE(set_id)
) STRICT;
CREATE INDEX no_intro_dat_game_descriptions_parent_order ON no_intro_dat_game_descriptions(set_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_categories (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    category TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_dat_categories_parent_order ON no_intro_dat_categories(set_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_identifiers (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    identifier TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_dat_identifiers_parent_order ON no_intro_dat_identifiers(set_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_releases (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_dat_releases_parent_order ON no_intro_dat_releases(set_id, source_order, source_element_id);

CREATE TABLE no_intro_dat_rom_claims (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    name TEXT NOT NULL,
    size_text TEXT,
    status_text TEXT,
    serial_text TEXT,
    header_text TEXT,
    date_text TEXT,
    mia_text TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_dat_rom_claims_game_order ON no_intro_dat_rom_claims(set_id, source_order, media_entry_id);

-- Every DAT position table is position-only. field_occurrence is zero for all
-- currently accepted singleton attributes; raw attribute ordinals may have gaps.
CREATE TABLE no_intro_dat_game_field_positions (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('name','id','cloneof','cloneofid')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(set_id, field_kind, field_occurrence),
    UNIQUE(set_id, source_order),
    CHECK ((field_kind IN ('cloneof','cloneofid')) = (relationship_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_dat_game_positions_order ON no_intro_dat_game_field_positions(set_id, source_order);

CREATE TABLE no_intro_dat_release_field_positions (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_releases(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('name','region')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind, field_occurrence),
    UNIQUE(source_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_clrmamepro_field_positions (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_clrmamepro_options(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('forcenodump','header')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind, field_occurrence),
    UNIQUE(source_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_romcenter_field_positions (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_romcenter_options(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind = 'plugin'),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind, field_occurrence),
    UNIQUE(source_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_rom_field_positions (
    media_entry_id INTEGER NOT NULL REFERENCES no_intro_dat_rom_claims(media_entry_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'name','size','crc','md5','sha1','sha256','status','serial','header','date','mia'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY(media_entry_id, field_kind, field_occurrence),
    UNIQUE(media_entry_id, source_order),
    CHECK ((field_kind IN ('crc','md5','sha1','sha256')) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_dat_rom_positions_order ON no_intro_dat_rom_field_positions(media_entry_id, source_order);

-- The xsi owner name identifies the real declaring element. The four closed
-- field codes are stored separately from ordinary source attribute positions.
CREATE TABLE no_intro_dat_document_xsi_attributes (
    edition_id INTEGER NOT NULL REFERENCES no_intro_dat_documents(edition_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL,
    source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(edition_id, field_kind),
    UNIQUE(edition_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_header_xsi_attributes (
    header_id INTEGER NOT NULL REFERENCES no_intro_dat_headers(header_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL,
    source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(header_id, field_kind), UNIQUE(header_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_header_text_child_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_header_text_children(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','type','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    resolved_type_kind TEXT CHECK (resolved_type_kind IS NULL OR resolved_type_kind IN (
        'int','short','byte','string','normalizedString','token','language','Name','NCName','NMTOKEN','ID','IDREF','ENTITY'
    )),
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal),
    CHECK ((field_kind = 'type') = (resolved_type_kind IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_clrmamepro_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_clrmamepro_options(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_romcenter_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_romcenter_options(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_game_xsi_attributes (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(set_id, field_kind), UNIQUE(set_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_game_description_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_game_descriptions(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','type','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    resolved_type_kind TEXT CHECK (resolved_type_kind IS NULL OR resolved_type_kind IN (
        'string','normalizedString','token','language','Name','NCName','NMTOKEN','ID','IDREF','ENTITY'
    )),
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal),
    CHECK ((field_kind = 'type') = (resolved_type_kind IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_category_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_categories(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','type','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    resolved_type_kind TEXT CHECK (resolved_type_kind IS NULL OR resolved_type_kind IN (
        'string','normalizedString','token','language','Name','NCName','NMTOKEN','ID','IDREF','ENTITY'
    )),
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal),
    CHECK ((field_kind = 'type') = (resolved_type_kind IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_identifier_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_identifiers(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','type','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    resolved_type_kind TEXT CHECK (resolved_type_kind IS NULL OR resolved_type_kind IN (
        'string','normalizedString','token','language','Name','NCName','NMTOKEN','ID','IDREF','ENTITY'
    )),
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal),
    CHECK ((field_kind = 'type') = (resolved_type_kind IS NOT NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_release_xsi_attributes (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_dat_releases(source_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(source_element_id, field_kind), UNIQUE(source_element_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dat_rom_xsi_attributes (
    media_entry_id INTEGER NOT NULL REFERENCES no_intro_dat_rom_claims(media_entry_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('schemaLocation','noNamespaceSchemaLocation','nil')),
    value_text TEXT NOT NULL, source_qname TEXT NOT NULL,
    attribute_ordinal INTEGER NOT NULL CHECK (attribute_ordinal >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(media_entry_id, field_kind), UNIQUE(media_entry_id, attribute_ordinal)
) STRICT, WITHOUT ROWID;

-- Synthetic fixture-only P/C projection ------------------------------------

CREATE TABLE no_intro_pc_documents (
    edition_id INTEGER PRIMARY KEY REFERENCES catalog_editions(edition_id),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    location_view TEXT NOT NULL CHECK (location_view='transport_decoded_xml_text'),
    start_line INTEGER NOT NULL CHECK (start_line > 0),
    start_column INTEGER NOT NULL CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line > 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column > 0),
    column_convention TEXT NOT NULL CHECK (column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK (start_line IS NOT NULL),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK (location_view IS NOT NULL AND column_convention IS NOT NULL AND
           column_convention='one_based_unicode_scalar')
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_pc_headers (
    header_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL UNIQUE REFERENCES catalog_set_groups(set_group_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_headers_group_order ON no_intro_pc_headers(set_group_id, source_order, header_id);

CREATE TABLE no_intro_pc_header_names (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL REFERENCES no_intro_pc_headers(header_id),
    name_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_header_names_parent_order ON no_intro_pc_header_names(header_id, source_order, source_element_id);

CREATE TABLE no_intro_pc_header_descriptions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL REFERENCES no_intro_pc_headers(header_id),
    description_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_header_descriptions_parent_order ON no_intro_pc_header_descriptions(header_id, source_order, source_element_id);

CREATE TABLE no_intro_pc_header_versions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_pc_headers(header_id),
    version_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_header_versions_parent_order ON no_intro_pc_header_versions(header_id, source_order, source_element_id);

CREATE TABLE no_intro_pc_games (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_sets(set_id),
    archive_id TEXT CHECK (archive_id IS NULL OR (length(archive_id)>0 AND instr(archive_id,char(0))=0 AND archive_id NOT GLOB '*[^0-9]*')),
    name_alt TEXT,
    region TEXT,
    version TEXT,
    bios_text TEXT
) STRICT;

CREATE TABLE no_intro_pc_game_descriptions (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_pc_games(set_id),
    description_text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_game_descriptions_parent_order ON no_intro_pc_game_descriptions(set_id, source_order, source_element_id);

CREATE TABLE no_intro_pc_languages (
    set_id INTEGER NOT NULL REFERENCES no_intro_pc_games(set_id),
    language_order INTEGER NOT NULL CHECK (language_order >= 0),
    language TEXT NOT NULL CHECK (length(language) > 0),
    PRIMARY KEY(set_id, language_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_pc_clone_markers (
    set_id INTEGER PRIMARY KEY REFERENCES no_intro_pc_games(set_id),
    marker TEXT NOT NULL CHECK (marker = 'P')
) STRICT;
CREATE TABLE no_intro_pc_clone_links (
    set_id INTEGER PRIMARY KEY REFERENCES no_intro_pc_games(set_id),
    target_archive_id TEXT NOT NULL CHECK (length(target_archive_id) > 0 AND instr(target_archive_id,char(0))=0 AND target_archive_id NOT GLOB '*[^0-9]*'),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;
CREATE TABLE no_intro_pc_merge_links (
    set_id INTEGER PRIMARY KEY REFERENCES no_intro_pc_games(set_id),
    target_archive_id TEXT NOT NULL CHECK (length(target_archive_id) > 0 AND instr(target_archive_id,char(0))=0 AND target_archive_id NOT GLOB '*[^0-9]*'),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

CREATE TABLE no_intro_pc_file_claims (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_pc_games(set_id),
    name TEXT NOT NULL CHECK (length(name) > 0),
    -- Rust parse::<u64>: optional single '+', nonempty ASCII digits, no
    -- whitespace/minus, and the complete unsigned range. Preserve spelling.
    size_text TEXT CHECK (size_text IS NULL OR (instr(size_text,char(0))=0 AND
        length(CASE WHEN substr(size_text,1,1)='+' THEN substr(size_text,2) ELSE size_text END)>0 AND
        (CASE WHEN substr(size_text,1,1)='+' THEN substr(size_text,2) ELSE size_text END) NOT GLOB '*[^0-9]*' AND
        (length(ltrim(size_text,'+0')) < 20 OR
         (length(ltrim(size_text,'+0')) = 20 AND ltrim(size_text,'+0') <= '18446744073709551615')))),
    size_i64 INTEGER GENERATED ALWAYS AS (
        CASE WHEN size_text IS NOT NULL
          AND (length(ltrim(size_text,'+0'))<19 OR
            (length(ltrim(size_text,'+0'))=19 AND ltrim(size_text,'+0')<='9223372036854775807'))
          THEN CAST(size_text AS INTEGER) END
    ) VIRTUAL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0)
) STRICT;
CREATE INDEX no_intro_pc_file_claims_game_order ON no_intro_pc_file_claims(set_id, source_order, media_entry_id);

CREATE TABLE no_intro_pc_game_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_pc_games(set_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('name','id','namealt','region','languages','version','bios','clone','mergeof')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(source_element_id, field_kind, field_occurrence), UNIQUE(source_element_id, source_order),
    CHECK ((field_kind='mergeof' AND relationship_id IS NOT NULL)
        OR field_kind='clone'
        OR (field_kind NOT IN ('clone','mergeof') AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_pc_game_positions_order ON no_intro_pc_game_attribute_positions(source_element_id, source_order);

CREATE TABLE no_intro_pc_rom_attribute_positions (
    source_element_id INTEGER NOT NULL REFERENCES no_intro_pc_file_claims(media_entry_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('name','size','crc','md5','sha1')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY(source_element_id, field_kind, field_occurrence), UNIQUE(source_element_id, source_order),
    CHECK ((field_kind IN ('crc','md5','sha1')) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_pc_rom_positions_order ON no_intro_pc_rom_attribute_positions(source_element_id, source_order);

-- Observed No-Intro database-export XML ------------------------------------

CREATE TABLE no_intro_export_documents (
    edition_id INTEGER PRIMARY KEY REFERENCES catalog_editions(edition_id),
    root_set_group_id INTEGER NOT NULL UNIQUE REFERENCES catalog_set_groups(set_group_id),
    envelope_mode TEXT NOT NULL CHECK (envelope_mode IN ('single_datafile','sibling_header_datafile')),
    header_present INTEGER NOT NULL CHECK (header_present IN (0,1)),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text','transport_decoded_xml_text')),
    start_line INTEGER CHECK (start_line IS NULL OR start_line > 0),
    start_column INTEGER CHECK (start_column IS NULL OR start_column > 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line > 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column > 0),
    column_convention TEXT CHECK (column_convention IS NULL OR column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (start_line IS NULL AND end_line IS NULL)),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK ((location_view IS NULL AND column_convention IS NULL) OR
           (location_view IS NOT NULL AND column_convention IS NOT NULL AND
            column_convention='one_based_unicode_scalar'))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_export_datafiles (
    edition_id INTEGER PRIMARY KEY REFERENCES no_intro_export_documents(edition_id),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    location_view TEXT NOT NULL CHECK (location_view='transport_decoded_xml_text'),
    start_line INTEGER NOT NULL CHECK (start_line > 0),
    start_column INTEGER NOT NULL CHECK (start_column > 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line > 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column > 0),
    column_convention TEXT NOT NULL CHECK (column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK (start_line IS NOT NULL),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK (location_view IS NOT NULL AND column_convention IS NOT NULL AND
           column_convention='one_based_unicode_scalar')
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_export_headers (
    header_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_export_documents(edition_id),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    location_view TEXT CHECK (location_view IN ('retained_original_text','transport_decoded_xml_text')),
    start_line INTEGER CHECK (start_line IS NULL OR start_line > 0),
    start_column INTEGER CHECK (start_column IS NULL OR start_column > 0),
    end_line INTEGER CHECK (end_line IS NULL OR end_line > 0),
    end_column INTEGER CHECK (end_column IS NULL OR end_column > 0),
    column_convention TEXT CHECK (column_convention IS NULL OR column_convention='one_based_unicode_scalar'),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK ((start_line IS NULL) = (start_column IS NULL)),
    CHECK ((end_line IS NULL) = (end_column IS NULL)),
    CHECK ((location_view IS NULL) = (start_line IS NULL AND end_line IS NULL)),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK ((location_view IS NULL AND column_convention IS NULL) OR
           (location_view IS NOT NULL AND column_convention IS NOT NULL AND
            column_convention='one_based_unicode_scalar')),
    CHECK (extent_view IS NOT NULL OR location_view IS NOT NULL),
    UNIQUE(header_id,edition_id)
) STRICT;

CREATE TABLE no_intro_export_header_placements (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_export_headers(header_id),
    datafile_edition_id INTEGER NOT NULL REFERENCES no_intro_export_datafiles(edition_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0)
) STRICT;
CREATE INDEX no_intro_export_header_placements_root_order ON no_intro_export_header_placements(datafile_edition_id, source_order, source_element_id);

CREATE TABLE no_intro_export_header_fields (
    source_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    header_id INTEGER NOT NULL REFERENCES no_intro_export_headers(header_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('author','piracy','trademarks','url','version')),
    text TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0),
    UNIQUE(header_id, source_order)
) STRICT;
CREATE INDEX no_intro_export_header_fields_kind ON no_intro_export_header_fields(header_id, field_kind, source_order);

CREATE TABLE no_intro_export_games (
    set_id INTEGER PRIMARY KEY REFERENCES catalog_sets(set_id),
    source_end_line INTEGER CHECK (source_end_line IS NULL OR source_end_line > 0),
    source_end_column INTEGER CHECK (source_end_column IS NULL OR source_end_column > 0),
    extent_view TEXT CHECK (extent_view IN ('retained_original_bytes','transport_decoded_xml_bytes')),
    extent_start INTEGER CHECK (extent_start IS NULL OR extent_start >= 0),
    extent_end INTEGER CHECK (extent_end IS NULL OR extent_end >= 0),
    CHECK ((extent_view IS NULL) = (extent_start IS NULL AND extent_end IS NULL)),
    CHECK ((extent_start IS NULL) = (extent_end IS NULL)),
    CHECK (extent_start IS NULL OR extent_end >= extent_start),
    CHECK ((source_end_line IS NULL) = (source_end_column IS NULL))
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_export_game_field_positions (
    set_id INTEGER NOT NULL REFERENCES no_intro_export_games(set_id),
    field_kind TEXT NOT NULL CHECK (field_kind = 'name'),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(set_id, field_kind, field_occurrence), UNIQUE(set_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_archive_descriptions (
    archive_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_export_games(set_id),
    additional TEXT, adult TEXT, aftermarket TEXT, alt TEXT, bios TEXT, categories TEXT,
    complete TEXT, dat TEXT, datter_note TEXT, description TEXT, devstatus TEXT,
    gameid1 TEXT, gameid2 TEXT, langchecked TEXT, languages TEXT, licensed TEXT, listed TEXT,
    mergename TEXT, name TEXT, name_alt TEXT, number TEXT, physical TEXT, region TEXT,
    regparent TEXT, showlang TEXT, special1 TEXT, special2 TEXT, sticky_note TEXT,
    version1 TEXT, version2 TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_archive_descriptions_game_order ON no_intro_archive_descriptions(set_id, source_order, archive_id);

CREATE TABLE no_intro_archive_clone_markers (
    archive_id INTEGER PRIMARY KEY REFERENCES no_intro_archive_descriptions(archive_id),
    marker TEXT NOT NULL CHECK (marker = 'P')
) STRICT;
CREATE TABLE no_intro_archive_clone_links (
    archive_id INTEGER PRIMARY KEY REFERENCES no_intro_archive_descriptions(archive_id),
    declared_target_number TEXT NOT NULL CHECK (declared_target_number <> 'P'),
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;
CREATE TABLE no_intro_archive_merge_links (
    archive_id INTEGER PRIMARY KEY REFERENCES no_intro_archive_descriptions(archive_id),
    declared_mergeof TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE REFERENCES reported_catalog_relationships(relationship_id)
) STRICT;

-- Read projections over the two disjoint typed clone variants. No second
-- literal is stored; the field ledger names these concrete value views.
CREATE VIEW no_intro_archive_declared_clone_text AS
SELECT archive_id,marker AS declared_text FROM no_intro_archive_clone_markers
UNION ALL
SELECT archive_id,declared_target_number FROM no_intro_archive_clone_links;

CREATE VIEW no_intro_pc_declared_clone_text AS
SELECT set_id,marker AS declared_text FROM no_intro_pc_clone_markers
UNION ALL
SELECT set_id,target_archive_id FROM no_intro_pc_clone_links;

CREATE TABLE no_intro_dump_sources (
    dump_source_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_export_games(set_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_dump_sources_game_order ON no_intro_dump_sources(set_id, source_order, dump_source_id);

CREATE TABLE no_intro_dump_details (
    details_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    dump_source_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_dump_sources(dump_source_id),
    comment1 TEXT, comment2 TEXT, d_date TEXT, d_date_info TEXT, dumper TEXT, id TEXT,
    link1 TEXT, link2 TEXT, link3 TEXT, media_title TEXT, nodump TEXT, origin TEXT,
    originalformat TEXT, project TEXT, r_date TEXT, r_date_info TEXT, region TEXT, rominfo TEXT, section TEXT, tool TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0),
    opening_end_line INTEGER NOT NULL CHECK (opening_end_line > 0),
    opening_end_column INTEGER NOT NULL CHECK (opening_end_column > 0),
    CHECK ((source_line, source_column) < (opening_end_line, opening_end_column)),
    CHECK ((opening_end_line, opening_end_column) <= (source_end_line, source_end_column))
) STRICT;
CREATE INDEX no_intro_dump_details_parent_order ON no_intro_dump_details(dump_source_id, source_order, details_element_id);

CREATE TABLE no_intro_dump_serials (
    serials_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    dump_source_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_dump_sources(dump_source_id),
    box_barcode TEXT, box_serial TEXT, chip_serial TEXT, digital_serial1 TEXT, digital_serial2 TEXT,
    lockout_serial TEXT, media_serial1 TEXT, media_serial2 TEXT, media_serial3 TEXT, mediastamp TEXT,
    pcb_serial TEXT, romchip_serial1 TEXT, romchip_serial2 TEXT, savechip_serial TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_dump_serials_parent_order ON no_intro_dump_serials(dump_source_id, source_order, serials_element_id);

CREATE TABLE no_intro_dump_files (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    dump_source_id INTEGER NOT NULL REFERENCES no_intro_dump_sources(dump_source_id),
    bad TEXT, date TEXT, extension TEXT, filter TEXT, forcename TEXT, forcescenename TEXT, format TEXT,
    header TEXT, id TEXT, item TEXT, mia TEXT, note TEXT, origin_size TEXT, serial TEXT,
    size_text TEXT,
    update_type TEXT, "unique" TEXT, version TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_dump_files_parent_order ON no_intro_dump_files(dump_source_id, source_order, media_entry_id);

CREATE TABLE no_intro_releases (
    release_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    set_id INTEGER NOT NULL REFERENCES no_intro_export_games(set_id),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_releases_game_order ON no_intro_releases(set_id, source_order, release_id);

CREATE TABLE no_intro_release_details (
    details_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    release_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_releases(release_id),
    archivename TEXT, category TEXT, comment TEXT, date TEXT, dirname TEXT, "group" TEXT, id TEXT,
    nfo_size TEXT, nfoname TEXT, nfosize TEXT, origin TEXT, originalformat TEXT, region TEXT, rominfo TEXT, tool TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0),
    opening_end_line INTEGER NOT NULL CHECK (opening_end_line > 0),
    opening_end_column INTEGER NOT NULL CHECK (opening_end_column > 0),
    CHECK ((source_line, source_column) < (opening_end_line, opening_end_column)),
    CHECK ((opening_end_line, opening_end_column) <= (source_end_line, source_end_column))
) STRICT;
CREATE INDEX no_intro_release_details_parent_order ON no_intro_release_details(release_id, source_order, details_element_id);

CREATE TABLE no_intro_release_serials (
    serials_element_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
    release_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_releases(release_id),
    box_barcode TEXT, box_serial TEXT, media_serial1 TEXT, mediastamp TEXT, pcb_serial TEXT, romchip_serial1 TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_release_serials_parent_order ON no_intro_release_serials(release_id, source_order, serials_element_id);

CREATE TABLE no_intro_release_files (
    media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_media_entries(media_entry_id),
    release_id INTEGER NOT NULL REFERENCES no_intro_releases(release_id),
    bad TEXT, extension TEXT, forcename TEXT, forcescenename TEXT, format TEXT, header TEXT,
    id TEXT, item TEXT, note TEXT, serial TEXT, size_text TEXT,
    update_type TEXT, version TEXT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    source_end_line INTEGER NOT NULL CHECK (source_end_line > 0), source_end_column INTEGER NOT NULL CHECK (source_end_column > 0)
) STRICT;
CREATE INDEX no_intro_release_files_parent_order ON no_intro_release_files(release_id, source_order, media_entry_id);

-- Source-field hash declarations are the sole digest value owner. Scope is
-- unknown for current source/release hashes and source_origin for origin_sha256.
CREATE TABLE no_intro_release_nfo_hashes (
    reported_nfo_hash_id INTEGER PRIMARY KEY,
    release_details_element_id INTEGER NOT NULL REFERENCES no_intro_release_details(details_element_id),
    source_hash_field TEXT NOT NULL CHECK (source_hash_field IN ('nfo_crc32','nfocrc')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence = 0),
    reported_text TEXT,
    presence TEXT NOT NULL CHECK (presence IN ('empty','value','invalid')),
    hash_id INTEGER REFERENCES hash_values(hash_id),
    UNIQUE(release_details_element_id, source_hash_field, field_occurrence),
    CHECK ((presence='value' AND hash_id IS NOT NULL AND reported_text IS NULL) OR
           (presence='empty' AND hash_id IS NULL AND reported_text IS NULL) OR
           (presence='invalid' AND hash_id IS NULL AND reported_text IS NOT NULL AND reported_text <> ''))
) STRICT;
CREATE INDEX no_intro_release_nfo_hash_parent ON no_intro_release_nfo_hashes(release_details_element_id, source_hash_field, field_occurrence);

CREATE TABLE invalid_no_intro_release_nfo_hashes (
    reported_nfo_hash_id INTEGER PRIMARY KEY REFERENCES no_intro_release_nfo_hashes(reported_nfo_hash_id),
    diagnostic_code TEXT NOT NULL
) STRICT;

CREATE TABLE no_intro_archive_field_positions (
    archive_id INTEGER NOT NULL REFERENCES no_intro_archive_descriptions(archive_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'additional','adult','aftermarket','alt','bios','categories','clone','complete','dat','datter_note',
        'description','devstatus','gameid1','gameid2','langchecked','languages','licensed','listed','mergename',
        'mergeof','name','name_alt','number','physical','region','regparent','showlang','special1','special2',
        'sticky_note','version1','version2'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    relationship_id INTEGER UNIQUE REFERENCES reported_catalog_relationships(relationship_id),
    PRIMARY KEY(archive_id, field_kind, field_occurrence), UNIQUE(archive_id, source_order),
    CHECK ((field_kind='mergeof' AND relationship_id IS NOT NULL)
        OR field_kind='clone'
        OR (field_kind NOT IN ('clone','mergeof') AND relationship_id IS NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_archive_positions_order ON no_intro_archive_field_positions(archive_id, source_order);

CREATE TABLE no_intro_dump_details_field_positions (
    details_element_id INTEGER NOT NULL REFERENCES no_intro_dump_details(details_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'comment1','comment2','d_date','d_date_info','dumper','id','link1','link2','link3','media_title',
        'nodump','origin','originalformat','project','r_date','r_date_info','region','rominfo','section','tool'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(details_element_id, field_kind, field_occurrence), UNIQUE(details_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dump_serials_field_positions (
    serials_element_id INTEGER NOT NULL REFERENCES no_intro_dump_serials(serials_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'box_barcode','box_serial','chip_serial','digital_serial1','digital_serial2','lockout_serial','media_serial1',
        'media_serial2','media_serial3','mediastamp','pcb_serial','romchip_serial1','romchip_serial2','savechip_serial'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(serials_element_id, field_kind, field_occurrence), UNIQUE(serials_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_release_details_field_positions (
    details_element_id INTEGER NOT NULL REFERENCES no_intro_release_details(details_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'archivename','category','comment','date','dirname','group','id','nfo_crc32','nfo_size','nfocrc',
        'nfoname','nfosize','origin','originalformat','region','rominfo','tool'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_nfo_hash_id INTEGER UNIQUE REFERENCES no_intro_release_nfo_hashes(reported_nfo_hash_id),
    PRIMARY KEY(details_element_id, field_kind, field_occurrence), UNIQUE(details_element_id, source_order),
    CHECK ((field_kind IN ('nfo_crc32','nfocrc')) = (reported_nfo_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_release_details_positions_order ON no_intro_release_details_field_positions(details_element_id, source_order);

CREATE TABLE no_intro_release_serials_field_positions (
    serials_element_id INTEGER NOT NULL REFERENCES no_intro_release_serials(serials_element_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN ('box_barcode','box_serial','media_serial1','mediastamp','pcb_serial','romchip_serial1')),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY(serials_element_id, field_kind, field_occurrence), UNIQUE(serials_element_id, source_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dump_file_field_positions (
    media_entry_id INTEGER NOT NULL REFERENCES no_intro_dump_files(media_entry_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'bad','crc32','date','extension','filter','forcename','forcescenename','format','header','id','item','md5',
        'mia','note','origin_sha256','origin_size','serial','sha1','sha256','size','unique','update_type','version'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY(media_entry_id, field_kind, field_occurrence), UNIQUE(media_entry_id, source_order),
    CHECK ((field_kind IN ('crc32','md5','sha1','sha256','origin_sha256')) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_dump_file_positions_order ON no_intro_dump_file_field_positions(media_entry_id, source_order);

CREATE TABLE no_intro_release_file_field_positions (
    media_entry_id INTEGER NOT NULL REFERENCES no_intro_release_files(media_entry_id),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'bad','crc32','extension','forcename','forcescenename','format','header','id','item','md5','note','serial',
        'sha1','sha256','size','update_type','version'
    )),
    field_occurrence INTEGER NOT NULL DEFAULT 0 CHECK (field_occurrence=0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0), source_line INTEGER NOT NULL CHECK (source_line > 0), source_column INTEGER NOT NULL CHECK (source_column > 0),
    reported_hash_id INTEGER UNIQUE REFERENCES catalog_entry_hashes(reported_hash_id),
    PRIMARY KEY(media_entry_id, field_kind, field_occurrence), UNIQUE(media_entry_id, source_order),
    CHECK ((field_kind IN ('crc32','md5','sha1','sha256')) = (reported_hash_id IS NOT NULL))
) STRICT, WITHOUT ROWID;
CREATE INDEX no_intro_release_file_positions_order ON no_intro_release_file_field_positions(media_entry_id, source_order);

CREATE TABLE no_intro_export_parse_witnesses (
    edition_id INTEGER PRIMARY KEY REFERENCES no_intro_export_documents(edition_id),
    game_count INTEGER NOT NULL CHECK (game_count >= 0),
    archive_count INTEGER NOT NULL CHECK (archive_count >= 0),
    dump_source_count INTEGER NOT NULL CHECK (dump_source_count >= 0),
    dump_details_count INTEGER NOT NULL CHECK (dump_details_count >= 0),
    dump_serials_count INTEGER NOT NULL CHECK (dump_serials_count >= 0),
    dump_file_count INTEGER NOT NULL CHECK (dump_file_count >= 0),
    release_count INTEGER NOT NULL CHECK (release_count >= 0),
    release_details_count INTEGER NOT NULL CHECK (release_details_count >= 0),
    release_serials_count INTEGER NOT NULL CHECK (release_serials_count >= 0),
    release_file_count INTEGER NOT NULL CHECK (release_file_count >= 0),
    header_field_count INTEGER NOT NULL CHECK (header_field_count >= 0),
    archive_position_count INTEGER NOT NULL CHECK (archive_position_count >= 0),
    dump_details_position_count INTEGER NOT NULL CHECK (dump_details_position_count >= 0),
    dump_serials_position_count INTEGER NOT NULL CHECK (dump_serials_position_count >= 0),
    dump_file_position_count INTEGER NOT NULL CHECK (dump_file_position_count >= 0),
    release_details_position_count INTEGER NOT NULL CHECK (release_details_position_count >= 0),
    release_serials_position_count INTEGER NOT NULL CHECK (release_serials_position_count >= 0),
    release_file_position_count INTEGER NOT NULL CHECK (release_file_position_count >= 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_export_game_witnesses (
    set_id INTEGER PRIMARY KEY REFERENCES no_intro_export_games(set_id),
    archive_count INTEGER NOT NULL CHECK (archive_count >= 0),
    dump_source_count INTEGER NOT NULL CHECK (dump_source_count >= 0),
    release_count INTEGER NOT NULL CHECK (release_count >= 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_dump_source_witnesses (
    dump_source_id INTEGER PRIMARY KEY REFERENCES no_intro_dump_sources(dump_source_id),
    details_count INTEGER NOT NULL CHECK (details_count IN (0,1)),
    serials_count INTEGER NOT NULL CHECK (serials_count IN (0,1)),
    file_count INTEGER NOT NULL CHECK (file_count >= 0)
) STRICT, WITHOUT ROWID;

CREATE TABLE no_intro_release_witnesses (
    release_id INTEGER PRIMARY KEY REFERENCES no_intro_releases(release_id),
    details_count INTEGER NOT NULL CHECK (details_count IN (0,1)),
    serials_count INTEGER NOT NULL CHECK (serials_count IN (0,1)),
    file_count INTEGER NOT NULL CHECK (file_count >= 0)
) STRICT, WITHOUT ROWID;

-- Format-local publication evidence. The assembler adds edition/native-owner
-- ancestry checks and folds this view into candidate_integrity_problems.
CREATE VIEW candidate_no_intro_integrity_problems AS
SELECT 'export_datafile_owner_count' AS problem,document.edition_id AS owner_id,document.edition_id AS edition_id
FROM no_intro_export_documents AS document
LEFT JOIN no_intro_export_datafiles AS datafile USING(edition_id)
GROUP BY document.edition_id HAVING count(datafile.edition_id)<>1
UNION ALL
SELECT 'export_header_mode',document.edition_id,document.edition_id
FROM no_intro_export_documents AS document
LEFT JOIN no_intro_export_headers AS header USING(edition_id)
LEFT JOIN no_intro_export_header_placements AS placement ON placement.datafile_edition_id=document.edition_id
GROUP BY document.edition_id,document.envelope_mode
HAVING (document.envelope_mode='single_datafile' AND
        (count(DISTINCT header.header_id)<>document.header_present OR
         count(placement.source_element_id)<>count(DISTINCT header.header_id)))
    OR (document.envelope_mode='sibling_header_datafile' AND
        (document.header_present<>1 OR count(DISTINCT header.header_id)<>document.header_present OR
         count(placement.source_element_id)<>0))
UNION ALL
SELECT 'export_header_placement_ancestry',placement.source_element_id,document.edition_id
FROM no_intro_export_header_placements AS placement
JOIN no_intro_export_documents AS document ON document.edition_id=placement.datafile_edition_id
LEFT JOIN no_intro_export_headers AS header ON header.header_id=placement.header_id
LEFT JOIN catalog_source_elements AS element ON element.source_element_id=placement.source_element_id
WHERE document.envelope_mode<>'single_datafile'
   OR header.edition_id IS NOT document.edition_id
   OR element.edition_id IS NOT document.edition_id
   OR element.element_kind<>'no_intro_export_nested_header'
UNION ALL
SELECT 'export_nested_header_precedes_games',placement.source_element_id,document.edition_id
FROM no_intro_export_header_placements AS placement
JOIN no_intro_export_documents AS document ON document.edition_id=placement.datafile_edition_id
JOIN catalog_sets AS game ON game.set_group_id=document.root_set_group_id
WHERE placement.source_order>=game.source_order
UNION ALL
SELECT 'export_root_group_edition',document.edition_id,document.edition_id
FROM no_intro_export_documents AS document
LEFT JOIN catalog_set_groups AS group_row ON group_row.set_group_id=document.root_set_group_id
WHERE group_row.edition_id IS NOT document.edition_id OR group_row.group_kind<>'root'
UNION ALL
SELECT 'flat_dat_reading_rules',document.edition_id,document.edition_id
FROM no_intro_dat_documents AS document
JOIN catalog_editions AS edition USING(edition_id)
LEFT JOIN catalog_reading_rules AS rules USING(reading_rules_id)
WHERE rules.format_family IS NOT 'no_intro_dat'
   OR rules.dialect NOT IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible',
                            'no-intro-dat-v4-strict','no-intro-dat-v4-compatible')
UNION ALL
SELECT 'flat_dat_header_missing_or_incomplete',document.edition_id,document.edition_id
FROM no_intro_dat_documents AS document
JOIN catalog_editions AS edition USING(edition_id)
JOIN catalog_reading_rules AS rules USING(reading_rules_id)
LEFT JOIN catalog_set_groups AS root_group ON root_group.edition_id=document.edition_id AND root_group.group_kind='root'
LEFT JOIN no_intro_dat_headers AS header ON header.set_group_id=root_group.set_group_id
WHERE header.header_id IS NULL
   OR NOT EXISTS (SELECT 1 FROM no_intro_dat_header_text_children AS field WHERE field.header_id=header.header_id AND field.field_kind='id')
   OR NOT EXISTS (SELECT 1 FROM no_intro_dat_header_text_children AS field WHERE field.header_id=header.header_id AND field.field_kind='name')
   OR NOT EXISTS (SELECT 1 FROM no_intro_dat_header_text_children AS field WHERE field.header_id=header.header_id AND field.field_kind='description')
   OR NOT EXISTS (SELECT 1 FROM no_intro_dat_header_text_children AS field WHERE field.header_id=header.header_id AND field.field_kind='version')
   OR (rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
       AND NOT EXISTS (SELECT 1 FROM no_intro_dat_header_text_children AS field WHERE field.header_id=header.header_id AND field.field_kind='author'))
UNION ALL
SELECT 'flat_dat_incompatible_field_presence',document.edition_id,document.edition_id
FROM no_intro_dat_documents AS document
JOIN catalog_editions AS edition USING(edition_id)
JOIN catalog_reading_rules AS rules USING(reading_rules_id)
WHERE (rules.dialect='no-intro-dat-v3-strict' AND EXISTS (
          SELECT 1 FROM no_intro_dat_headers AS header JOIN no_intro_dat_header_text_children AS field USING(header_id)
          WHERE header.set_group_id IN (SELECT set_group_id FROM catalog_set_groups WHERE edition_id=document.edition_id)
            AND field.field_kind IN ('trademarks','piracy')))
   OR (rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict') AND EXISTS (
          SELECT 1 FROM no_intro_dat_headers AS header JOIN no_intro_dat_header_text_children AS field USING(header_id)
          WHERE header.set_group_id IN (SELECT set_group_id FROM catalog_set_groups WHERE edition_id=document.edition_id)
            AND field.field_kind='comment'))
   OR (rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict') AND EXISTS (
          SELECT 1 FROM catalog_sets AS set_row JOIN no_intro_dat_identifiers AS identifier USING(set_id)
          WHERE set_row.set_group_id IN (SELECT set_group_id FROM catalog_set_groups WHERE edition_id=document.edition_id)))
UNION ALL
SELECT 'flat_dat_game_children_missing_or_mismatched',game.set_id,element.edition_id
FROM no_intro_dat_games AS game
JOIN catalog_source_elements AS element ON element.source_element_id=game.set_id
JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
JOIN catalog_set_groups AS group_row ON group_row.set_group_id=set_row.set_group_id
JOIN no_intro_dat_documents AS document ON document.edition_id=group_row.edition_id
JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=(SELECT reading_rules_id FROM catalog_editions WHERE edition_id=document.edition_id)
WHERE (SELECT count(*) FROM no_intro_dat_game_descriptions AS description WHERE description.set_id=game.set_id)<>1
   OR (SELECT count(*) FROM no_intro_dat_rom_claims AS rom WHERE rom.set_id=game.set_id)<1
   OR (rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
       AND (SELECT count(*) FROM no_intro_dat_rom_claims AS rom WHERE rom.set_id=game.set_id)<>1)
UNION ALL
SELECT 'synthetic_pc_reading_rules',document.edition_id,document.edition_id
FROM no_intro_pc_documents AS document
JOIN catalog_editions AS edition USING(edition_id)
LEFT JOIN catalog_reading_rules AS rules USING(reading_rules_id)
WHERE rules.format_family IS NOT 'no_intro_pc_fixture'
UNION ALL
SELECT 'synthetic_pc_game_name',game.set_id,element.edition_id
FROM no_intro_pc_games AS game
JOIN catalog_source_elements AS element ON element.source_element_id=game.set_id
JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
WHERE length(set_row.set_name)=0
UNION ALL
SELECT 'dat_game_relationship_position_mismatch',position.set_id,element.edition_id
FROM no_intro_dat_game_field_positions AS position
JOIN catalog_source_elements AS element ON element.source_element_id=position.set_id
LEFT JOIN no_intro_dat_set_links AS declaration
  ON declaration.set_id=position.set_id AND declaration.link_kind=position.field_kind
WHERE (position.field_kind IN ('cloneof','cloneofid')
       AND declaration.relationship_id IS NOT position.relationship_id)
   OR (position.field_kind NOT IN ('cloneof','cloneofid') AND position.relationship_id IS NOT NULL)
UNION ALL
SELECT 'archive_relationship_position_mismatch',position.archive_id,element.edition_id
FROM no_intro_archive_field_positions AS position
JOIN no_intro_archive_descriptions AS archive USING(archive_id)
JOIN catalog_source_elements AS element ON element.source_element_id=archive.archive_id
LEFT JOIN no_intro_archive_clone_markers AS marker USING(archive_id)
LEFT JOIN no_intro_archive_clone_links AS clone_link USING(archive_id)
LEFT JOIN no_intro_archive_merge_links AS merge_link USING(archive_id)
WHERE (position.field_kind='clone' AND
       ((marker.archive_id IS NOT NULL AND
         (position.relationship_id IS NOT NULL OR clone_link.archive_id IS NOT NULL))
        OR (marker.archive_id IS NULL AND
            (clone_link.archive_id IS NULL OR position.relationship_id IS NOT clone_link.relationship_id))))
   OR (position.field_kind='mergeof' AND position.relationship_id IS NOT merge_link.relationship_id)
   OR (position.field_kind NOT IN ('clone','mergeof') AND position.relationship_id IS NOT NULL)
UNION ALL
SELECT 'pc_relationship_position_mismatch',position.source_element_id,element.edition_id
FROM no_intro_pc_game_attribute_positions AS position
JOIN catalog_source_elements AS element ON element.source_element_id=position.source_element_id
LEFT JOIN no_intro_pc_clone_markers AS marker ON marker.set_id=position.source_element_id
LEFT JOIN no_intro_pc_clone_links AS clone_link ON clone_link.set_id=position.source_element_id
LEFT JOIN no_intro_pc_merge_links AS merge_link ON merge_link.set_id=position.source_element_id
WHERE (position.field_kind='clone' AND
       ((marker.set_id IS NOT NULL AND
         (position.relationship_id IS NOT NULL OR clone_link.set_id IS NOT NULL))
        OR (marker.set_id IS NULL AND
            (clone_link.set_id IS NULL OR position.relationship_id IS NOT clone_link.relationship_id))))
   OR (position.field_kind='mergeof' AND position.relationship_id IS NOT merge_link.relationship_id)
   OR (position.field_kind NOT IN ('clone','mergeof') AND position.relationship_id IS NOT NULL)
UNION ALL
SELECT 'export_document_datafile_extent_containment',document.edition_id,document.edition_id
FROM no_intro_export_documents AS document
JOIN no_intro_export_datafiles AS datafile USING(edition_id)
WHERE document.extent_view=datafile.extent_view
  AND document.extent_start IS NOT NULL AND document.extent_end IS NOT NULL
  AND datafile.extent_start IS NOT NULL AND datafile.extent_end IS NOT NULL
  AND (document.extent_start>datafile.extent_start OR datafile.extent_end>document.extent_end)
UNION ALL
SELECT 'export_document_datafile_location_containment',document.edition_id,document.edition_id
FROM no_intro_export_documents AS document
JOIN no_intro_export_datafiles AS datafile USING(edition_id)
WHERE document.location_view=datafile.location_view
  AND document.start_line IS NOT NULL AND document.end_line IS NOT NULL
  AND datafile.start_line IS NOT NULL AND datafile.end_line IS NOT NULL
  AND ((datafile.start_line,datafile.start_column)<(document.start_line,document.start_column)
    OR (datafile.end_line,datafile.end_column)>(document.end_line,document.end_column))
UNION ALL
SELECT 'export_header_datafile_extent_containment',header.header_id,header.edition_id
FROM no_intro_export_headers AS header
JOIN no_intro_export_header_placements AS placement USING(header_id)
JOIN no_intro_export_datafiles AS datafile ON datafile.edition_id=placement.datafile_edition_id
WHERE header.extent_view=datafile.extent_view
  AND header.extent_start IS NOT NULL AND header.extent_end IS NOT NULL
  AND datafile.extent_start IS NOT NULL AND datafile.extent_end IS NOT NULL
  AND (header.extent_start<datafile.extent_start OR header.extent_end>datafile.extent_end)
UNION ALL
SELECT 'export_header_datafile_location_containment',header.header_id,header.edition_id
FROM no_intro_export_headers AS header
JOIN no_intro_export_header_placements AS placement USING(header_id)
JOIN no_intro_export_datafiles AS datafile ON datafile.edition_id=placement.datafile_edition_id
WHERE header.location_view=datafile.location_view
  AND header.start_line IS NOT NULL AND header.end_line IS NOT NULL
  AND datafile.start_line IS NOT NULL AND datafile.end_line IS NOT NULL
  AND ((header.start_line,header.start_column)<(datafile.start_line,datafile.start_column)
    OR (header.end_line,header.end_column)>(datafile.end_line,datafile.end_column))
UNION ALL
SELECT 'export_count_witness_missing_or_mismatch',document.edition_id,document.edition_id
FROM no_intro_export_documents AS document
LEFT JOIN no_intro_export_parse_witnesses AS witness USING(edition_id)
WHERE witness.edition_id IS NULL
   OR witness.game_count<>(SELECT count(*) FROM no_intro_export_games AS row WHERE row.set_id IN
       (SELECT set_id FROM catalog_sets WHERE set_group_id=document.root_set_group_id))
   OR witness.archive_count<>(SELECT count(*) FROM no_intro_archive_descriptions AS row JOIN no_intro_export_games AS game USING(set_id)
       JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_source_count<>(SELECT count(*) FROM no_intro_dump_sources AS row JOIN catalog_sets AS set_row ON set_row.set_id=row.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_details_count<>(SELECT count(*) FROM no_intro_dump_details AS row JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_serials_count<>(SELECT count(*) FROM no_intro_dump_serials AS row JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_file_count<>(SELECT count(*) FROM no_intro_dump_files AS row JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_count<>(SELECT count(*) FROM no_intro_releases AS row JOIN catalog_sets AS set_row ON set_row.set_id=row.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_details_count<>(SELECT count(*) FROM no_intro_release_details AS row JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_serials_count<>(SELECT count(*) FROM no_intro_release_serials AS row JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_file_count<>(SELECT count(*) FROM no_intro_release_files AS row JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.header_field_count<>(SELECT count(*) FROM no_intro_export_header_fields AS field JOIN no_intro_export_headers AS header USING(header_id) WHERE header.edition_id=document.edition_id)
   OR witness.archive_position_count<>(SELECT count(*) FROM no_intro_archive_field_positions AS position JOIN no_intro_archive_descriptions AS archive USING(archive_id) JOIN catalog_sets AS set_row ON set_row.set_id=archive.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_details_position_count<>(SELECT count(*) FROM no_intro_dump_details_field_positions AS position JOIN no_intro_dump_details AS details USING(details_element_id) JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_serials_position_count<>(SELECT count(*) FROM no_intro_dump_serials_field_positions AS position JOIN no_intro_dump_serials AS serials USING(serials_element_id) JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.dump_file_position_count<>(SELECT count(*) FROM no_intro_dump_file_field_positions AS position JOIN no_intro_dump_files AS file USING(media_entry_id) JOIN no_intro_dump_sources AS parent USING(dump_source_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_details_position_count<>(SELECT count(*) FROM no_intro_release_details_field_positions AS position JOIN no_intro_release_details AS details USING(details_element_id) JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_serials_position_count<>(SELECT count(*) FROM no_intro_release_serials_field_positions AS position JOIN no_intro_release_serials AS serials USING(serials_element_id) JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
   OR witness.release_file_position_count<>(SELECT count(*) FROM no_intro_release_file_field_positions AS position JOIN no_intro_release_files AS file USING(media_entry_id) JOIN no_intro_releases AS parent USING(release_id) JOIN catalog_sets AS set_row ON set_row.set_id=parent.set_id WHERE set_row.set_group_id=document.root_set_group_id)
UNION ALL
SELECT 'export_game_witness_missing_or_mismatch',game.set_id,document.edition_id
FROM no_intro_export_games AS game
JOIN catalog_sets AS set_row USING(set_id)
JOIN no_intro_export_documents AS document ON document.root_set_group_id=set_row.set_group_id
LEFT JOIN no_intro_export_game_witnesses AS witness USING(set_id)
WHERE witness.set_id IS NULL
   OR witness.archive_count<>(SELECT count(*) FROM no_intro_archive_descriptions WHERE set_id=game.set_id)
   OR witness.dump_source_count<>(SELECT count(*) FROM no_intro_dump_sources WHERE set_id=game.set_id)
   OR witness.release_count<>(SELECT count(*) FROM no_intro_releases WHERE set_id=game.set_id)
UNION ALL
SELECT 'export_dump_source_witness_missing_or_mismatch',source.dump_source_id,document.edition_id
FROM no_intro_dump_sources AS source
JOIN catalog_sets AS set_row ON set_row.set_id=source.set_id
JOIN no_intro_export_documents AS document ON document.root_set_group_id=set_row.set_group_id
LEFT JOIN no_intro_dump_source_witnesses AS witness USING(dump_source_id)
WHERE witness.dump_source_id IS NULL
   OR witness.details_count<>(SELECT count(*) FROM no_intro_dump_details WHERE dump_source_id=source.dump_source_id)
   OR witness.serials_count<>(SELECT count(*) FROM no_intro_dump_serials WHERE dump_source_id=source.dump_source_id)
   OR witness.file_count<>(SELECT count(*) FROM no_intro_dump_files WHERE dump_source_id=source.dump_source_id)
UNION ALL
SELECT 'export_release_witness_missing_or_mismatch',release.release_id,document.edition_id
FROM no_intro_releases AS release
JOIN catalog_sets AS set_row ON set_row.set_id=release.set_id
JOIN no_intro_export_documents AS document ON document.root_set_group_id=set_row.set_group_id
LEFT JOIN no_intro_release_witnesses AS witness USING(release_id)
WHERE witness.release_id IS NULL
   OR witness.details_count<>(SELECT count(*) FROM no_intro_release_details WHERE release_id=release.release_id)
   OR witness.serials_count<>(SELECT count(*) FROM no_intro_release_serials WHERE release_id=release.release_id)
   OR witness.file_count<>(SELECT count(*) FROM no_intro_release_files WHERE release_id=release.release_id)
UNION ALL
SELECT 'export_nfo_invalid_subtype',hash.reported_nfo_hash_id,document.edition_id
FROM no_intro_release_nfo_hashes AS hash
JOIN no_intro_release_details AS details ON details.details_element_id=hash.release_details_element_id
JOIN no_intro_releases AS release ON release.release_id=details.release_id
JOIN catalog_sets AS set_row ON set_row.set_id=release.set_id
JOIN no_intro_export_documents AS document ON document.root_set_group_id=set_row.set_group_id
WHERE (hash.presence='invalid')<>(EXISTS(SELECT 1 FROM invalid_no_intro_release_nfo_hashes AS invalid WHERE invalid.reported_nfo_hash_id=hash.reported_nfo_hash_id));
