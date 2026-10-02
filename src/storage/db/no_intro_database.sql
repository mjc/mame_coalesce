-- Native owners for the streaming No-Intro DATABASE-EXPORT reader. Values
-- remain declared text; only the numeric size projection and digest bytes are
-- interpreted here.
CREATE TABLE no_intro_exports (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    envelope_kind TEXT NOT NULL CHECK (envelope_kind IN ('single_datafile', 'sibling_header_datafile')),
    header_present INTEGER NOT NULL CHECK (typeof(header_present) = 'integer' AND header_present IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0)
) WITHOUT ROWID;

-- The optional tag is a separate XML element from the datafile element.
-- Retain its own location even when the tag contains no child fields.
CREATE TABLE no_intro_export_headers (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES no_intro_exports(snapshot_key) ON DELETE RESTRICT,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0)
) WITHOUT ROWID;

CREATE TABLE no_intro_database_games (
    set_id INTEGER PRIMARY KEY NOT NULL,
    source_element_kind TEXT NOT NULL DEFAULT 'no_intro_database_game'
        CHECK (source_element_kind = 'no_intro_database_game'),
    name_source_order INTEGER NOT NULL CHECK (typeof(name_source_order) = 'integer' AND name_source_order >= 0),
    name_source_line INTEGER NOT NULL CHECK (typeof(name_source_line) = 'integer' AND name_source_line > 0),
    name_source_column INTEGER NOT NULL CHECK (typeof(name_source_column) = 'integer' AND name_source_column > 0),
    FOREIGN KEY (set_id, source_element_kind)
        REFERENCES catalog_sets(set_id, source_element_kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_header_fields (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_export_headers(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 4),
    value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_archive_descriptions (
    archive_id INTEGER PRIMARY KEY NOT NULL CHECK (archive_id > 0),
    set_id INTEGER NOT NULL REFERENCES no_intro_database_games(set_id) ON DELETE RESTRICT,
    -- Raw child ordinal shared with dump-source and release siblings.
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    additional TEXT, adult TEXT, aftermarket TEXT, alt TEXT, bios TEXT, categories TEXT,
    complete TEXT, dat TEXT, datter_note TEXT, description TEXT, devstatus TEXT,
    gameid1 TEXT, gameid2 TEXT, langchecked TEXT, languages TEXT, licensed TEXT,
    listed TEXT, mergename TEXT, name TEXT, name_alt TEXT, number TEXT,
    physical TEXT, region TEXT, regparent TEXT, showlang TEXT, special1 TEXT,
    special2 TEXT, sticky_note TEXT, version1 TEXT, version2 TEXT,
    UNIQUE (set_id, source_order)
);

CREATE TABLE no_intro_archive_clone_markers (
    archive_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    marker TEXT NOT NULL CHECK (marker = 'P')
) WITHOUT ROWID;

CREATE TABLE no_intro_archive_clone_links (
    archive_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    declared_target_number TEXT NOT NULL CHECK (declared_target_number IS NOT NULL AND declared_target_number <> 'P'),
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT GENERATED ALWAYS AS ('no_intro_database_archive_clone') VIRTUAL,
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_archive_merge_links (
    archive_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    declared_mergeof TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT GENERATED ALWAYS AS ('no_intro_database_archive_mergeof') VIRTUAL,
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_sources (
    dump_source_id INTEGER PRIMARY KEY NOT NULL CHECK (dump_source_id > 0),
    set_id INTEGER NOT NULL REFERENCES no_intro_database_games(set_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    UNIQUE (dump_source_id, set_id),
    UNIQUE (set_id, source_order)
);

CREATE TABLE no_intro_dump_details (
    dump_source_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES no_intro_dump_sources(dump_source_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    opening_end_line INTEGER NOT NULL CHECK (typeof(opening_end_line)='integer' AND opening_end_line>0),
    opening_end_column INTEGER NOT NULL CHECK (typeof(opening_end_column)='integer' AND opening_end_column>0),
    comment1 TEXT, comment2 TEXT, d_date TEXT, d_date_info TEXT, dumper TEXT,
    id TEXT, link1 TEXT, link2 TEXT, link3 TEXT, media_title TEXT, nodump TEXT,
    origin TEXT, originalformat TEXT, project TEXT, r_date TEXT, r_date_info TEXT,
    region TEXT, rominfo TEXT, section TEXT, tool TEXT,
    CHECK (opening_end_line>source_line OR (opening_end_line=source_line AND opening_end_column>source_column))
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_serials (
    dump_source_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES no_intro_dump_sources(dump_source_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    box_barcode TEXT, box_serial TEXT, chip_serial TEXT, digital_serial1 TEXT,
    digital_serial2 TEXT, lockout_serial TEXT, media_serial1 TEXT, media_serial2 TEXT,
    media_serial3 TEXT, mediastamp TEXT, pcb_serial TEXT, romchip_serial1 TEXT,
    romchip_serial2 TEXT, savechip_serial TEXT
) WITHOUT ROWID;

CREATE TABLE no_intro_releases (
    release_id INTEGER PRIMARY KEY NOT NULL CHECK (release_id > 0),
    set_id INTEGER NOT NULL REFERENCES no_intro_database_games(set_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    UNIQUE (release_id, set_id),
    UNIQUE (set_id, source_order)
);

CREATE TABLE no_intro_release_details (
    release_id INTEGER PRIMARY KEY NOT NULL REFERENCES no_intro_releases(release_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    opening_end_line INTEGER NOT NULL CHECK (typeof(opening_end_line)='integer' AND opening_end_line>0),
    opening_end_column INTEGER NOT NULL CHECK (typeof(opening_end_column)='integer' AND opening_end_column>0),
    archivename TEXT, category TEXT, comment TEXT, date TEXT, dirname TEXT, "group" TEXT,
    id TEXT, nfo_size TEXT, nfoname TEXT, nfosize TEXT, origin TEXT,
    originalformat TEXT, region TEXT, rominfo TEXT, tool TEXT,
    CHECK (opening_end_line>source_line OR (opening_end_line=source_line AND opening_end_column>source_column))
) WITHOUT ROWID;

CREATE TABLE no_intro_release_serials (
    release_id INTEGER PRIMARY KEY NOT NULL REFERENCES no_intro_releases(release_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    box_barcode TEXT, box_serial TEXT, media_serial1 TEXT, mediastamp TEXT,
    pcb_serial TEXT, romchip_serial1 TEXT
) WITHOUT ROWID;

-- A source/release file is the actual asset occurrence. The repeated set_id
-- is a composite-FK witness, preventing a file from crossing game ownership.
CREATE TABLE no_intro_dump_files (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'no_intro_database_source_file'
        CHECK (claim_kind = 'no_intro_database_source_file'),
    evidence_scope TEXT NOT NULL DEFAULT 'unknown' CHECK (evidence_scope IN ('whole_file', 'unknown')),
    evidence_provenance TEXT NOT NULL DEFAULT 'source_declared'
        CHECK (evidence_provenance = 'source_declared'),
    dump_source_id INTEGER NOT NULL,
    set_id INTEGER NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    bad TEXT, date TEXT, extension TEXT, filter TEXT, forcename TEXT,
    forcescenename TEXT, format TEXT, header TEXT, id TEXT, item TEXT,
    mia TEXT, note TEXT, origin_size TEXT, serial TEXT, source_size TEXT,
    size INTEGER GENERATED ALWAYS AS (
      CASE
        WHEN source_size IS NULL OR source_size = '' OR instr(source_size, char(0)) > 0
          OR source_size GLOB '*[^0-9]*' THEN NULL
        WHEN length(ltrim(source_size, '0')) > 19 THEN NULL
        WHEN length(ltrim(source_size, '0')) = 19
          AND ltrim(source_size, '0') > '9223372036854775807' THEN NULL
        ELSE CAST(source_size AS INTEGER)
      END
    ) VIRTUAL,
    "unique" TEXT, update_type TEXT, version TEXT,
    UNIQUE (occurrence_id, set_id),
    UNIQUE (occurrence_id, claim_kind),
    UNIQUE (dump_source_id, source_order),
    FOREIGN KEY (dump_source_id, set_id)
        REFERENCES no_intro_dump_sources(dump_source_id, set_id) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, set_id)
        REFERENCES asset_occurrences(occurrence_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, claim_kind)
        REFERENCES asset_occurrences(occurrence_id, claim_kind) ON DELETE RESTRICT
);

CREATE TABLE no_intro_release_files (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'no_intro_database_release_file'
        CHECK (claim_kind = 'no_intro_database_release_file'),
    evidence_scope TEXT NOT NULL DEFAULT 'unknown' CHECK (evidence_scope IN ('whole_file', 'unknown')),
    evidence_provenance TEXT NOT NULL DEFAULT 'source_declared'
        CHECK (evidence_provenance = 'source_declared'),
    release_id INTEGER NOT NULL,
    set_id INTEGER NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    bad TEXT, extension TEXT, forcename TEXT, forcescenename TEXT, format TEXT,
    header TEXT, id TEXT, item TEXT, note TEXT, serial TEXT, source_size TEXT,
    size INTEGER GENERATED ALWAYS AS (
      CASE
        WHEN source_size IS NULL OR source_size = '' OR instr(source_size, char(0)) > 0
          OR source_size GLOB '*[^0-9]*' THEN NULL
        WHEN length(ltrim(source_size, '0')) > 19 THEN NULL
        WHEN length(ltrim(source_size, '0')) = 19
          AND ltrim(source_size, '0') > '9223372036854775807' THEN NULL
        ELSE CAST(source_size AS INTEGER)
      END
    ) VIRTUAL,
    update_type TEXT, version TEXT,
    UNIQUE (occurrence_id, set_id),
    UNIQUE (occurrence_id, claim_kind),
    UNIQUE (release_id, source_order),
    FOREIGN KEY (release_id, set_id)
        REFERENCES no_intro_releases(release_id, set_id) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, set_id)
        REFERENCES asset_occurrences(occurrence_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, claim_kind)
        REFERENCES asset_occurrences(occurrence_id, claim_kind) ON DELETE RESTRICT
);

-- Closed archive attribute codes are 0..29 in the approved ledger order;
-- clone is 30 and mergeof is 31. Digest rows reference these positions via
-- generated mappings, so digest source order/location exists once per field.
CREATE TABLE no_intro_archive_field_positions (
    archive_id INTEGER NOT NULL REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 31),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (archive_id, field_kind), UNIQUE (archive_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_details_field_positions (
    dump_source_id INTEGER NOT NULL REFERENCES no_intro_dump_details(dump_source_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 19),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (dump_source_id, field_kind), UNIQUE (dump_source_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_serials_field_positions (
    dump_source_id INTEGER NOT NULL REFERENCES no_intro_dump_serials(dump_source_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 13),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (dump_source_id, field_kind), UNIQUE (dump_source_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_file_field_positions (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_dump_files(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 22),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (occurrence_id, field_kind), UNIQUE (occurrence_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_release_details_field_positions (
    release_id INTEGER NOT NULL REFERENCES no_intro_release_details(release_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 16),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (release_id, field_kind), UNIQUE (release_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_release_serials_field_positions (
    release_id INTEGER NOT NULL REFERENCES no_intro_release_serials(release_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 5),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (release_id, field_kind), UNIQUE (release_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_release_file_field_positions (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_release_files(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 16),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (occurrence_id, field_kind), UNIQUE (occurrence_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_file_digests (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_dump_files(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 4),
    position_field_kind INTEGER GENERATED ALWAYS AS
        (CASE field_kind WHEN 0 THEN 1 WHEN 1 THEN 11 WHEN 2 THEN 17 WHEN 3 THEN 18 WHEN 4 THEN 14 END) VIRTUAL,
    digest_id INTEGER REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    invalid_literal TEXT,
    PRIMARY KEY (occurrence_id, field_kind),
    FOREIGN KEY (occurrence_id, position_field_kind)
        REFERENCES no_intro_dump_file_field_positions(occurrence_id, field_kind) ON DELETE RESTRICT,
    CHECK ((digest_id IS NOT NULL AND invalid_literal IS NULL) OR
           (digest_id IS NULL AND invalid_literal IS NOT NULL AND
            NOT (length(invalid_literal) = CASE field_kind WHEN 0 THEN 8 WHEN 1 THEN 32 WHEN 2 THEN 40 ELSE 64 END
                 AND invalid_literal NOT GLOB '*[^0-9A-Fa-f]*')))
) WITHOUT ROWID;

CREATE TABLE no_intro_release_nfo_hashes (
    release_id INTEGER NOT NULL REFERENCES no_intro_release_details(release_id) ON DELETE RESTRICT,
    source_hash_field TEXT NOT NULL CHECK (source_hash_field IN ('nfocrc', 'nfo_crc32')),
    position_field_kind INTEGER GENERATED ALWAYS AS
        (CASE source_hash_field WHEN 'nfo_crc32' THEN 7 WHEN 'nfocrc' THEN 9 END) VIRTUAL,
    hash_id INTEGER REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    presence TEXT NOT NULL CHECK (presence = 'present'),
    scope TEXT NOT NULL CHECK (scope = 'nfo_companion'),
    invalid_literal TEXT,
    PRIMARY KEY (release_id, source_hash_field),
    FOREIGN KEY (release_id, position_field_kind)
        REFERENCES no_intro_release_details_field_positions(release_id, field_kind) ON DELETE RESTRICT,
    CHECK ((hash_id IS NOT NULL AND invalid_literal IS NULL) OR
           (hash_id IS NULL AND invalid_literal IS NOT NULL AND
            NOT (length(invalid_literal) = 8 AND invalid_literal NOT GLOB '*[^0-9A-Fa-f]*')))
) WITHOUT ROWID;

CREATE TABLE no_intro_release_file_digests (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_release_files(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 3),
    position_field_kind INTEGER GENERATED ALWAYS AS
        (CASE field_kind WHEN 0 THEN 1 WHEN 1 THEN 9 WHEN 2 THEN 12 WHEN 3 THEN 13 END) VIRTUAL,
    digest_id INTEGER REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    invalid_literal TEXT,
    PRIMARY KEY (occurrence_id, field_kind),
    FOREIGN KEY (occurrence_id, position_field_kind)
        REFERENCES no_intro_release_file_field_positions(occurrence_id, field_kind) ON DELETE RESTRICT,
    CHECK ((digest_id IS NOT NULL AND invalid_literal IS NULL) OR
           (digest_id IS NULL AND invalid_literal IS NOT NULL AND
            NOT (length(invalid_literal) = CASE field_kind WHEN 0 THEN 8 WHEN 1 THEN 32 WHEN 2 THEN 40 ELSE 64 END
                 AND invalid_literal NOT GLOB '*[^0-9A-Fa-f]*')))
) WITHOUT ROWID;

CREATE TABLE no_intro_database_parse_counts (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES no_intro_exports(snapshot_key) ON DELETE RESTRICT,
    game_count INTEGER NOT NULL CHECK (typeof(game_count) = 'integer' AND game_count >= 0),
    archive_count INTEGER NOT NULL CHECK (typeof(archive_count) = 'integer' AND archive_count >= 0),
    dump_source_count INTEGER NOT NULL CHECK (typeof(dump_source_count) = 'integer' AND dump_source_count >= 0),
    dump_details_count INTEGER NOT NULL CHECK (typeof(dump_details_count) = 'integer' AND dump_details_count >= 0),
    dump_serials_count INTEGER NOT NULL CHECK (typeof(dump_serials_count) = 'integer' AND dump_serials_count >= 0),
    dump_file_count INTEGER NOT NULL CHECK (typeof(dump_file_count) = 'integer' AND dump_file_count >= 0),
    release_count INTEGER NOT NULL CHECK (typeof(release_count) = 'integer' AND release_count >= 0),
    release_details_count INTEGER NOT NULL CHECK (typeof(release_details_count) = 'integer' AND release_details_count >= 0),
    release_serials_count INTEGER NOT NULL CHECK (typeof(release_serials_count) = 'integer' AND release_serials_count >= 0),
    release_file_count INTEGER NOT NULL CHECK (typeof(release_file_count) = 'integer' AND release_file_count >= 0),
    header_field_count INTEGER NOT NULL CHECK (typeof(header_field_count) = 'integer' AND header_field_count >= 0),
    archive_field_count INTEGER NOT NULL CHECK (typeof(archive_field_count) = 'integer' AND archive_field_count >= 0),
    dump_details_field_count INTEGER NOT NULL CHECK (typeof(dump_details_field_count) = 'integer' AND dump_details_field_count >= 0),
    dump_serials_field_count INTEGER NOT NULL CHECK (typeof(dump_serials_field_count) = 'integer' AND dump_serials_field_count >= 0),
    dump_file_field_count INTEGER NOT NULL CHECK (typeof(dump_file_field_count) = 'integer' AND dump_file_field_count >= 0),
    release_details_field_count INTEGER NOT NULL CHECK (typeof(release_details_field_count) = 'integer' AND release_details_field_count >= 0),
    release_serials_field_count INTEGER NOT NULL CHECK (typeof(release_serials_field_count) = 'integer' AND release_serials_field_count >= 0),
    release_file_field_count INTEGER NOT NULL CHECK (typeof(release_file_field_count) = 'integer' AND release_file_field_count >= 0)
) WITHOUT ROWID;

CREATE TABLE no_intro_dump_details_diagnostics (
    diagnostic_key TEXT NOT NULL,
    run_key TEXT NOT NULL,
    dump_source_id INTEGER NOT NULL REFERENCES no_intro_dump_details(dump_source_id) ON DELETE RESTRICT,
    PRIMARY KEY(diagnostic_key),
    FOREIGN KEY(diagnostic_key,run_key) REFERENCES import_diagnostics(diagnostic_key,run_key) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_release_details_diagnostics (
    diagnostic_key TEXT NOT NULL,
    run_key TEXT NOT NULL,
    release_id INTEGER NOT NULL REFERENCES no_intro_release_details(release_id) ON DELETE RESTRICT,
    PRIMARY KEY(diagnostic_key),
    FOREIGN KEY(diagnostic_key,run_key) REFERENCES import_diagnostics(diagnostic_key,run_key) ON DELETE RESTRICT
) WITHOUT ROWID;

-- The source version is owned by each format's native header. Keep this as a
-- derived compatibility view rather than persisting a second projection.
CREATE VIEW catalog_snapshot_versions AS
SELECT snapshot.snapshot_key,
       CASE
         WHEN parser.format = 'no-intro-pc-xml' THEN (
             SELECT header.version_text FROM no_intro_pc_headers AS header
             WHERE header.snapshot_key = snapshot.snapshot_key
         )
         WHEN parser.format = 'mame-listxml' THEN (
             SELECT header.build FROM mame_document_facts AS header
             WHERE header.snapshot_key = snapshot.snapshot_key
         )
         WHEN parser.format = 'mame-softwarelist-xml' THEN (
             SELECT wrapper.build FROM software_wrapper_headers AS wrapper
             WHERE wrapper.snapshot_key = snapshot.snapshot_key
         )
         WHEN parser.format IN (
             'no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
             'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible'
         ) THEN dat_header.version_text
         WHEN parser.format IN (
             'no-intro-database-xml-compatible',
             'no-intro-database-xml-nul-compatible'
         ) THEN (
             SELECT CASE WHEN COUNT(*) = 1 THEN MIN(header_field.value) END
             FROM no_intro_header_fields AS header_field
             WHERE header_field.snapshot_key = snapshot.snapshot_key
               AND header_field.field_kind = 4
         )
         ELSE snapshot.declared_version
       END AS declared_version
FROM catalog_snapshots AS snapshot
LEFT JOIN parser_interpretations AS parser
  ON parser.interpretation_key = snapshot.interpretation_key
LEFT JOIN no_intro_dat_headers AS dat_header
  ON dat_header.snapshot_key = snapshot.snapshot_key;

CREATE VIEW no_intro_database_source_relationships AS
SELECT registry.assertion_key,
       CASE reported.source_reference_kind
         WHEN 'no_intro_database_archive_clone' THEN 'source_parent_clone'
         ELSE 'alternate_representation_of'
       END AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       CASE reported.source_reference_kind
         WHEN 'no_intro_database_archive_clone' THEN 'archive_clone'
         ELSE 'archive_mergeof'
       END AS source_field,
       position.source_line, position.source_column,
       NULL AS generic_subject_snapshot_key, 'no_intro_archive' AS subject_kind,
       NULL AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, NULL AS source_subject_a, NULL AS source_subject_b,
       archive.archive_id AS source_subject_c, registry.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key, 'no_intro_archive_reference' AS target_kind,
       NULL AS target_set_id, NULL AS generic_target_a, NULL AS generic_target_b,
       NULL AS generic_target_c,
       CASE reported.source_reference_kind
         WHEN 'no_intro_database_archive_clone' THEN clone.declared_target_number
         ELSE merge.declared_mergeof
       END AS source_target_a,
       NULL AS source_target_b, NULL AS source_target_c,
       registry.snapshot_key AS target_snapshot_key, NULL AS rule_version
FROM catalog_relationships AS registry
JOIN reported_catalog_relationships AS reported USING(relationship_id)
LEFT JOIN no_intro_archive_clone_links AS clone USING(relationship_id)
LEFT JOIN no_intro_archive_merge_links AS merge USING(relationship_id)
JOIN no_intro_archive_descriptions AS archive
  ON archive.archive_id=COALESCE(clone.archive_id,merge.archive_id)
JOIN no_intro_archive_field_positions AS position
  ON position.archive_id=archive.archive_id
 AND position.field_kind=CASE reported.source_reference_kind
     WHEN 'no_intro_database_archive_clone' THEN 30 ELSE 31 END
JOIN no_intro_database_games AS native ON native.set_id=archive.set_id
JOIN catalog_sets AS sets ON sets.set_id=native.set_id
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
JOIN parser_interpretations AS interpretation
  ON interpretation.interpretation_key=snapshot.interpretation_key
WHERE registry.origin='source' AND registry.snapshot_key=groups.snapshot_key
  AND reported.source_reference_kind IN ('no_intro_database_archive_clone',
                                         'no_intro_database_archive_mergeof')
  AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
  AND interpretation.format IN ('no-intro-database-xml-compatible',
                                 'no-intro-database-xml-nul-compatible');

CREATE VIEW no_intro_pc_source_relationships AS
SELECT registry.assertion_key,
       CASE reported.source_reference_kind
         WHEN 'no_intro_pc_clone' THEN 'source_parent_clone'
         ELSE 'alternate_representation_of'
       END AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       CASE reported.source_reference_kind WHEN 'no_intro_pc_clone' THEN 'clone' ELSE 'mergeof' END AS source_field,
       sets.source_line, sets.source_column,
       NULL AS generic_subject_snapshot_key, 'catalog_set' AS subject_kind,
       sets.set_id AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, sets.set_name AS source_subject_a, NULL AS source_subject_b,
       NULL AS source_subject_c, registry.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key, 'no_intro_archive_reference' AS target_kind,
       NULL AS target_set_id, NULL AS generic_target_a, NULL AS generic_target_b,
       NULL AS generic_target_c, link.target_archive_id AS source_target_a,
       NULL AS source_target_b, NULL AS source_target_c, registry.snapshot_key AS target_snapshot_key,
       NULL AS rule_version
FROM catalog_relationships AS registry
JOIN reported_catalog_relationships AS reported USING(relationship_id)
JOIN no_intro_pc_clone_links AS link USING(relationship_id)
JOIN no_intro_pc_games AS native USING(set_id)
JOIN catalog_sets AS sets USING(set_id)
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN catalog_snapshots AS snapshot USING(snapshot_key)
JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE registry.origin='source' AND registry.snapshot_key=groups.snapshot_key
  AND reported.source_reference_kind='no_intro_pc_clone'
  AND sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
  AND interpretation.format='no-intro-pc-xml'
UNION ALL
SELECT registry.assertion_key,'alternate_representation_of','source_assertion',registry.snapshot_key,
       'mergeof',sets.source_line,sets.source_column,
       NULL,'catalog_set',sets.set_id,NULL,NULL,NULL,sets.set_name,NULL,NULL,registry.snapshot_key,
       NULL,'no_intro_archive_reference',NULL,NULL,NULL,NULL,link.target_archive_id,NULL,NULL,
       registry.snapshot_key,NULL
FROM catalog_relationships AS registry
JOIN reported_catalog_relationships AS reported USING(relationship_id)
JOIN no_intro_pc_merge_links AS link USING(relationship_id)
JOIN no_intro_pc_games AS native USING(set_id)
JOIN catalog_sets AS sets USING(set_id)
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN catalog_snapshots AS snapshot USING(snapshot_key)
JOIN parser_interpretations AS interpretation USING(interpretation_key)
WHERE registry.origin='source' AND registry.snapshot_key=groups.snapshot_key
  AND reported.source_reference_kind='no_intro_pc_mergeof'
  AND sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
  AND interpretation.format='no-intro-pc-xml';
