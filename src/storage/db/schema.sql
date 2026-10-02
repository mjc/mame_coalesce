-- Direct-create schema. Never replay or upgrade historical databases.
-- This initial baseline is being replaced with the reviewed format-specific model.
CREATE TABLE database_schema (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    schema_digest BLOB NOT NULL CHECK (typeof(schema_digest) = 'blob' AND length(schema_digest) = 32)
);

CREATE TABLE acquisition_attempt_transport_headers (
    attempt_key TEXT NOT NULL REFERENCES acquisition_attempts (attempt_key) ON DELETE RESTRICT,
    header_order INTEGER NOT NULL CHECK (header_order >= 0),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (attempt_key, header_order)
) WITHOUT ROWID;
CREATE TABLE acquisition_attempts (
    attempt_key             TEXT PRIMARY KEY NOT NULL,
    source_key              TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    source_uri              TEXT,
    method                  TEXT,
    attempted_at            DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expected_sha256         BLOB CHECK (expected_sha256 IS NULL OR length(expected_sha256) = 32),
    outcome                 TEXT NOT NULL CHECK (outcome IN ('retained', 'failed')),
    verification_status     TEXT NOT NULL
                                CHECK (verification_status IN ('verified', 'unverified', 'rejected')),
    document_key            TEXT REFERENCES documents (document_key) ON DELETE RESTRICT,
    acquisition_key         TEXT,
    diagnostic              TEXT,
    FOREIGN KEY (acquisition_key, document_key, source_key)
        REFERENCES acquisitions (acquisition_key, document_key, source_key) ON DELETE RESTRICT,
    CHECK (
        (outcome = 'retained' AND verification_status IN ('verified', 'unverified')
            AND document_key IS NOT NULL AND acquisition_key IS NOT NULL AND diagnostic IS NULL)
        OR
        (outcome = 'failed' AND verification_status = 'rejected'
            AND document_key IS NULL AND acquisition_key IS NULL AND diagnostic IS NOT NULL)
    )
);
CREATE TABLE acquisition_transport_headers (
    acquisition_key TEXT NOT NULL REFERENCES acquisitions (acquisition_key) ON DELETE RESTRICT,
    header_order INTEGER NOT NULL CHECK (header_order >= 0),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (acquisition_key, header_order)
) WITHOUT ROWID;
CREATE TABLE acquisitions (
    acquisition_key TEXT PRIMARY KEY NOT NULL,
    source_key      TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    document_key    TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    source_uri      TEXT,
    method          TEXT,
    acquired_at     DATETIME, expected_sha256 BLOB
    CHECK (expected_sha256 IS NULL OR length(expected_sha256) = 32), verification_status TEXT NOT NULL DEFAULT 'unverified'
        CHECK (verification_status IN ('verified', 'unverified', 'failed')),
    UNIQUE (acquisition_key, document_key)
);
CREATE TABLE asset_occurrences (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    record_id INTEGER NOT NULL REFERENCES catalog_sets (set_id) ON DELETE RESTRICT,
    occurrence_order INTEGER NOT NULL CHECK (typeof(occurrence_order) = 'integer' AND occurrence_order >= 0),
    claim_kind TEXT NOT NULL CHECK (claim_kind IN (
        'mame_rom', 'mame_disk', 'mame_sample', 'logiqx_rom', 'logiqx_disk',
        'logiqx_sample', 'cmp_rom', 'cmp_sample', 'no_intro_pc_file',
        'no_intro_dat_rom', 'no_intro_database_source_file', 'no_intro_database_release_file', 'software_rom_entry',
        'software_rom_operation', 'software_disk_entry'
    )),
    content_uuid BLOB REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    CHECK (content_uuid IS NULL OR length(content_uuid) = 16),
    CHECK (claim_kind NOT IN ('software_rom_operation', 'mame_sample', 'logiqx_sample', 'cmp_sample')
        OR content_uuid IS NULL),
    UNIQUE (record_id, occurrence_order),
    UNIQUE (occurrence_id, record_id),
    UNIQUE (occurrence_id, claim_kind)
);
CREATE TABLE catalog_snapshots (
    snapshot_key        TEXT PRIMARY KEY NOT NULL,
    catalog_key         TEXT NOT NULL REFERENCES catalogs (catalog_key) ON DELETE RESTRICT,
    document_key        TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    interpretation_key  TEXT NOT NULL REFERENCES parser_interpretations (interpretation_key) ON DELETE RESTRICT,
    acquisition_key     TEXT REFERENCES acquisitions (acquisition_key) ON DELETE RESTRICT,
    declared_version    TEXT,
    coverage_id INTEGER NOT NULL REFERENCES catalog_coverage(coverage_id) ON DELETE RESTRICT,
    parent_snapshot_key TEXT,
    CHECK (parent_snapshot_key IS NULL OR parent_snapshot_key <> snapshot_key),
    UNIQUE (snapshot_key, catalog_key),
    UNIQUE (snapshot_key, catalog_key, document_key, interpretation_key),
    FOREIGN KEY (parent_snapshot_key, catalog_key)
        REFERENCES catalog_snapshots (snapshot_key, catalog_key)
        ON DELETE RESTRICT,
    FOREIGN KEY (acquisition_key, document_key)
        REFERENCES acquisitions (acquisition_key, document_key)
        ON DELETE RESTRICT
);
CREATE TABLE catalogs (
    catalog_key TEXT PRIMARY KEY NOT NULL,
    source_key  TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    display_name TEXT NOT NULL
);
CREATE TABLE cmp_header_directives (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES cmp_header_facts(snapshot_key) ON DELETE RESTRICT,
    header_definition TEXT, forcemerging TEXT, forcezipping TEXT, forcepacking TEXT, forcenodump TEXT,
    forcemerging_effective TEXT GENERATED ALWAYS AS (
      CASE WHEN forcemerging IN ('none','split','full') THEN forcemerging END) VIRTUAL,
    forcezipping_effective TEXT GENERATED ALWAYS AS (
      CASE WHEN forcezipping IN ('zip','unzip') THEN forcezipping END) VIRTUAL,
    forcenodump_effective TEXT GENERATED ALWAYS AS (
      CASE WHEN forcenodump IS NULL THEN 'obsolete'
           WHEN forcenodump IN ('obsolete','required','ignore') THEN forcenodump END) VIRTUAL
) WITHOUT ROWID;
CREATE TABLE cmp_header_facts (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    source_block TEXT NOT NULL CHECK (lower(source_block) = 'clrmamepro'),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    name TEXT, description TEXT, version TEXT, date TEXT, author TEXT,
    email TEXT, homepage TEXT, url TEXT, comment TEXT, category TEXT
) WITHOUT ROWID;
CREATE TABLE cmp_rom_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'cmp_rom' CHECK (claim_kind = 'cmp_rom'),
    name TEXT NOT NULL,
    size_text TEXT,
    size INTEGER GENERATED ALWAYS AS (CAST(size_text AS INTEGER)) VIRTUAL,
    crc_text TEXT,
    crc32_text TEXT,
    md5_text TEXT,
    sha1_text TEXT,
    evidence_scope TEXT NOT NULL CHECK (evidence_scope IN ('whole_file','whole_asset')),
    evidence_provenance TEXT NOT NULL CHECK (evidence_provenance = 'source_declared'),
    merge_name TEXT,
    date TEXT,
    serial TEXT,
    status_text TEXT,
    nodump_present INTEGER NOT NULL CHECK (nodump_present IN (0,1)),
    baddump_present INTEGER NOT NULL CHECK (baddump_present IN (0,1)),
    dump_status TEXT GENERATED ALWAYS AS (
        CASE WHEN nodump_present + baddump_present > 1
               OR (status_text IS NOT NULL AND nodump_present + baddump_present > 0) THEN NULL
             WHEN status_text IS NOT NULL THEN status_text
             WHEN nodump_present = 1 THEN 'nodump'
             WHEN baddump_present = 1 THEN 'baddump' END
    ) VIRTUAL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT,
    CHECK (size_text IS NULL OR (length(size_text) > 0 AND instr(size_text,char(0)) = 0
      AND size_text NOT GLOB '*[^0-9]*' AND (length(ltrim(size_text,'0')) < 19
      OR (length(ltrim(size_text,'0')) = 19 AND ltrim(size_text,'0') <= '9223372036854775807')))),
    CHECK (crc_text IS NULL OR (length(crc_text) = 8 AND instr(crc_text,char(0)) = 0 AND crc_text NOT GLOB '*[^0-9a-fA-F]*')),
    CHECK (crc32_text IS NULL OR (length(crc32_text) = 8 AND instr(crc32_text,char(0)) = 0 AND crc32_text NOT GLOB '*[^0-9a-fA-F]*')),
    CHECK (md5_text IS NULL OR (length(md5_text) = 32 AND instr(md5_text,char(0)) = 0 AND md5_text NOT GLOB '*[^0-9a-fA-F]*')),
    CHECK (sha1_text IS NULL OR (length(sha1_text) = 40 AND instr(sha1_text,char(0)) = 0 AND sha1_text NOT GLOB '*[^0-9a-fA-F]*'))
);
CREATE TABLE cmp_sample_parent_links (
    record_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_sets (set_id) ON DELETE RESTRICT,
    target_name TEXT NOT NULL
) WITHOUT ROWID;
CREATE TABLE cmp_samples (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'cmp_sample' CHECK (claim_kind = 'cmp_sample'),
    sample_name TEXT NOT NULL,
    source_field TEXT NOT NULL CHECK (lower(source_field) = 'sample'),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    is_quoted INTEGER NOT NULL CHECK (typeof(is_quoted) = 'integer' AND is_quoted IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    FOREIGN KEY (occurrence_id, claim_kind) REFERENCES asset_occurrences(occurrence_id, claim_kind) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE cmp_set_facts (
    record_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    source_block TEXT NOT NULL CHECK (lower(source_block) IN ('game','set')),
    document_order INTEGER NOT NULL CHECK (typeof(document_order) = 'integer' AND document_order >= 0),
    description TEXT, year TEXT, manufacturer TEXT, rebuildto TEXT,
    region TEXT, release_year_text TEXT, release_month_text TEXT, release_day_text TEXT, serial TEXT
) WITHOUT ROWID;
CREATE TABLE digest_values (
    digest_id INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    digest BLOB NOT NULL,
    UNIQUE (algorithm, digest),
    CHECK (
        (algorithm = 'crc32' AND length(digest) = 4) OR
        (algorithm = 'md5' AND length(digest) = 16) OR
        (algorithm = 'sha1' AND length(digest) = 20) OR
        (algorithm = 'sha256' AND length(digest) = 32)
    )
);
CREATE TABLE documents (
    document_key TEXT PRIMARY KEY NOT NULL,
    sha1         BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    byte_length  INTEGER CHECK (byte_length IS NULL OR byte_length >= 0)
, sha256 BLOB CHECK (sha256 IS NULL OR length(sha256) = 32), format_hint TEXT, retention_status TEXT NOT NULL DEFAULT 'unavailable'
        CHECK (retention_status IN ('unavailable', 'retained')), object_key TEXT);
CREATE TABLE import_diagnostics (
    diagnostic_key TEXT PRIMARY KEY NOT NULL,
    run_key TEXT NOT NULL REFERENCES import_runs (run_key) ON DELETE RESTRICT,
    document_key TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    severity TEXT NOT NULL DEFAULT 'error' CHECK (severity IN ('warning', 'error')),
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    record_kind TEXT,
    record_name TEXT,
    field_name TEXT,
    offending_text TEXT,
    source_line INTEGER,
    source_column INTEGER,
    source_excerpt BLOB,
    excerpt_view TEXT CHECK (excerpt_view IN ('retained_original_bytes', 'transport_decoded_xml_bytes')),
    excerpt_start_byte INTEGER,
    problem_start_byte INTEGER,
    problem_end_byte INTEGER,
    source_problem_start_byte INTEGER,
    source_problem_end_byte INTEGER,
    original_problem_start_byte INTEGER,
    original_problem_end_byte INTEGER,
    coordinate_view TEXT CHECK (coordinate_view IN ('transport_decoded_xml_text', 'decoded_dat_text')),
    column_convention TEXT CHECK (column_convention = 'unicode_scalar_1based'),
    UNIQUE (diagnostic_key, run_key),
    FOREIGN KEY (run_key, document_key) REFERENCES import_runs(run_key, document_key) ON DELETE RESTRICT,
    CHECK ((source_excerpt IS NULL AND excerpt_view IS NULL AND excerpt_start_byte IS NULL)
        OR (typeof(source_excerpt) = 'blob' AND excerpt_view IS NOT NULL
            AND (excerpt_start_byte IS NULL OR (typeof(excerpt_start_byte) = 'integer' AND excerpt_start_byte >= 0)))),
    CHECK ((problem_start_byte IS NULL AND problem_end_byte IS NULL)
        OR (typeof(problem_start_byte) = 'integer' AND typeof(problem_end_byte) = 'integer'
            AND source_excerpt IS NOT NULL AND problem_start_byte >= 0
            AND problem_end_byte >= problem_start_byte AND problem_end_byte <= length(source_excerpt))),
    CHECK ((source_problem_start_byte IS NULL AND source_problem_end_byte IS NULL)
        OR (typeof(source_problem_start_byte) = 'integer' AND typeof(source_problem_end_byte) = 'integer'
            AND excerpt_view IS NOT NULL AND source_problem_start_byte >= 0
            AND source_problem_end_byte >= source_problem_start_byte)),
    CHECK ((original_problem_start_byte IS NULL AND original_problem_end_byte IS NULL)
        OR (typeof(original_problem_start_byte) = 'integer' AND typeof(original_problem_end_byte) = 'integer'
            AND original_problem_start_byte >= 0 AND original_problem_end_byte >= original_problem_start_byte
            AND excerpt_view IS NOT NULL AND excerpt_view = 'retained_original_bytes')),
    CHECK (original_problem_start_byte IS NULL OR source_problem_start_byte IS NULL
        OR (original_problem_start_byte = source_problem_start_byte
            AND original_problem_end_byte = source_problem_end_byte)),
    CHECK (problem_start_byte IS NULL OR excerpt_start_byte IS NULL OR source_problem_start_byte IS NULL
        OR (source_problem_start_byte = excerpt_start_byte + problem_start_byte
            AND source_problem_end_byte = excerpt_start_byte + problem_end_byte)),
    CHECK (problem_start_byte IS NULL OR excerpt_start_byte IS NULL OR original_problem_start_byte IS NULL
        OR (original_problem_start_byte = excerpt_start_byte + problem_start_byte
            AND original_problem_end_byte = excerpt_start_byte + problem_end_byte)),
    CHECK ((coordinate_view IS NULL AND column_convention IS NULL)
        OR (coordinate_view IS NOT NULL AND column_convention IS NOT NULL
            AND typeof(source_line) = 'integer' AND source_line > 0
            AND typeof(source_column) = 'integer' AND source_column > 0))
);
CREATE TABLE import_runs (
    run_key             TEXT PRIMARY KEY NOT NULL,
    catalog_key         TEXT NOT NULL REFERENCES catalogs (catalog_key) ON DELETE RESTRICT,
    document_key        TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    interpretation_key  TEXT NOT NULL REFERENCES parser_interpretations (interpretation_key) ON DELETE RESTRICT,
    acquisition_key     TEXT REFERENCES acquisitions (acquisition_key) ON DELETE RESTRICT,
    snapshot_key        TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    status              TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
    started_at          DATETIME,
    finished_at         DATETIME,
    diagnostic          TEXT,
    UNIQUE (run_key, document_key),
    CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at),
    FOREIGN KEY (snapshot_key, catalog_key, document_key, interpretation_key)
        REFERENCES catalog_snapshots
            (snapshot_key, catalog_key, document_key, interpretation_key)
        ON DELETE RESTRICT,
    FOREIGN KEY (acquisition_key, document_key)
        REFERENCES acquisitions (acquisition_key, document_key)
        ON DELETE RESTRICT
);
CREATE TABLE logiqx_disk_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'logiqx_disk' CHECK (claim_kind = 'logiqx_disk'),
    name TEXT NOT NULL,
    md5_text TEXT,
    sha1_text TEXT,
    evidence_scope TEXT NOT NULL,
    evidence_provenance TEXT NOT NULL,
    merge_name TEXT,
    dump_status TEXT NOT NULL DEFAULT 'good' CHECK (dump_status IN ('good','baddump','nodump','verified')),
    status_was_present INTEGER NOT NULL DEFAULT 0 CHECK (status_was_present IN (0,1)),
    source_order INTEGER NOT NULL DEFAULT 0 CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    CHECK (status_was_present = 1 OR dump_status = 'good'),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TABLE logiqx_document_facts (
    snapshot_key TEXT PRIMARY KEY NOT NULL,
    build TEXT,
    debug TEXT NOT NULL DEFAULT 'no' CHECK (debug IN ('yes','no')),
    debug_was_present INTEGER NOT NULL DEFAULT 0 CHECK (debug_was_present IN (0,1)),
    file_name TEXT,
    sha1 BLOB,
    header_name TEXT,
    header_description TEXT,
    header_version TEXT,
    header_date TEXT,
    header_author TEXT,
    header_email TEXT,
    header_homepage TEXT,
    header_url TEXT,
    header_comment TEXT,
    header_category TEXT,
    FOREIGN KEY (snapshot_key) REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    CHECK (sha1 IS NULL OR length(sha1) = 20),
    CHECK (debug_was_present = 1 OR debug = 'no')
);
CREATE TABLE logiqx_rom_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'logiqx_rom' CHECK (claim_kind = 'logiqx_rom'),
    name TEXT NOT NULL,
    size_text TEXT,
    size INTEGER GENERATED ALWAYS AS (
        CASE WHEN length(size_text) > 0 AND instr(size_text, char(0)) = 0
            AND size_text NOT GLOB '*[^0-9]*'
            AND (length(ltrim(size_text,'0')) < 19
                OR (length(ltrim(size_text,'0')) = 19
                    AND ltrim(size_text,'0') <= '9223372036854775807'))
        THEN CAST(size_text AS INTEGER) END
    ) VIRTUAL,
    crc_text TEXT,
    md5_text TEXT,
    sha1_text TEXT,
    evidence_scope TEXT NOT NULL,
    evidence_provenance TEXT NOT NULL,
    merge_name TEXT,
    dump_status TEXT NOT NULL DEFAULT 'good' CHECK (dump_status IN ('good','baddump','nodump','verified')),
    status_was_present INTEGER NOT NULL DEFAULT 0 CHECK (status_was_present IN (0,1)),
    source_order INTEGER NOT NULL DEFAULT 0 CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    serial TEXT,
    date TEXT,
    CHECK (status_was_present = 1 OR dump_status = 'good'),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TABLE logiqx_sample_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'logiqx_sample' CHECK (claim_kind = 'logiqx_sample'),
    name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TRIGGER logiqx_sample_claims_immutable_update BEFORE UPDATE ON logiqx_sample_claims
BEGIN SELECT RAISE(ABORT,'native sample claims are immutable'); END;
CREATE TRIGGER logiqx_sample_claims_immutable_delete BEFORE DELETE ON logiqx_sample_claims
BEGIN SELECT RAISE(ABORT,'native sample claims are immutable'); END;
CREATE TRIGGER logiqx_rom_claims_require_unpublished_snapshot
BEFORE INSERT ON logiqx_rom_claims
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences
    JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published catalog claims are immutable'); END;

CREATE TRIGGER logiqx_disk_claims_require_unpublished_snapshot
BEFORE INSERT ON logiqx_disk_claims
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences
    JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published catalog claims are immutable'); END;

CREATE TRIGGER logiqx_sample_claims_require_unpublished_snapshot
BEFORE INSERT ON logiqx_sample_claims
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences
    JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published catalog claims are immutable'); END;
CREATE TABLE logiqx_games (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    source_file TEXT,
    is_bios TEXT NOT NULL DEFAULT 'no' CHECK (is_bios IN ('yes','no')),
    is_bios_was_present INTEGER NOT NULL DEFAULT 0 CHECK (is_bios_was_present IN (0,1)),
    board TEXT,
    rebuild_to TEXT,
    description TEXT,
    year TEXT,
    manufacturer TEXT,
    PRIMARY KEY (set_id),
    CHECK (is_bios_was_present = 1 OR is_bios = 'no')
) WITHOUT ROWID;
CREATE TABLE mame_bios_sets (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    bios_order INTEGER NOT NULL CHECK (bios_order >= 0),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    default_specified INTEGER NOT NULL CHECK (default_specified IN (0, 1)),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, bios_order)
) WITHOUT ROWID;
CREATE TABLE machine_switch_conditions (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    condition_order INTEGER NOT NULL CHECK (condition_order >= 0),
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('eq', 'ne', 'gt', 'le', 'lt', 'ge')),
    value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, condition_order),
    FOREIGN KEY (set_id, switch_order)
        REFERENCES machine_switches (set_id, switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE "machine_switch_locations" (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    location_order INTEGER NOT NULL CHECK (location_order >= 0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    name TEXT NOT NULL,
    number TEXT NOT NULL,
    inverted INTEGER NOT NULL CHECK (inverted IN (0, 1)),
    inverted_specified INTEGER NOT NULL CHECK (inverted_specified IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, location_order),
    FOREIGN KEY (set_id, switch_order)
        REFERENCES "machine_switches" (set_id, switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switch_value_conditions (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    value_order INTEGER NOT NULL CHECK (value_order >= 0),
    condition_order INTEGER NOT NULL CHECK (condition_order >= 0),
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('eq', 'ne', 'gt', 'le', 'lt', 'ge')),
    value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, value_order, condition_order),
    FOREIGN KEY (set_id, switch_order, value_order)
        REFERENCES machine_switch_values (set_id, switch_order, value_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE "machine_switch_values" (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    value_order INTEGER NOT NULL CHECK (value_order >= 0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    default_specified INTEGER NOT NULL CHECK (default_specified IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, value_order),
    FOREIGN KEY (set_id, switch_order)
        REFERENCES "machine_switches" (set_id, switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE "machine_switches" (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('dipswitch', 'configuration')),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_disk_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'mame_disk' CHECK (claim_kind = 'mame_disk'),
    name TEXT NOT NULL,
    sha1_text TEXT,
    evidence_scope TEXT NOT NULL,
    evidence_provenance TEXT NOT NULL,
    dump_status TEXT,
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    region TEXT,
    optional INTEGER NOT NULL CHECK (optional IN (0,1)),
    optional_specified INTEGER NOT NULL CHECK (optional_specified IN (0,1)),
    disk_index TEXT,
    writable INTEGER NOT NULL CHECK (writable IN (0,1)),
    writable_specified INTEGER NOT NULL CHECK (writable_specified IN (0,1)),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TABLE mame_document_facts (
    snapshot_key TEXT NOT NULL PRIMARY KEY,
    build TEXT,
    debug INTEGER NOT NULL CHECK (debug IN (0, 1)),
    debug_specified INTEGER NOT NULL CHECK (debug_specified IN (0, 1)),
    config_version TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (snapshot_key)
        REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT
);
CREATE TABLE mame_machine_adjuster_conditions (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    condition_order INTEGER NOT NULL CHECK (condition_order >= 0),
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('eq', 'ne', 'gt', 'le', 'lt', 'ge')),
    value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order, condition_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_adjusters (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_adjusters (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    name TEXT NOT NULL,
    default_value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_analogs (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    analog_order INTEGER NOT NULL CHECK (analog_order >= 0),
    mask TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order, analog_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_ports (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_chips (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    name TEXT NOT NULL,
    tag TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('cpu', 'audio')),
    clock TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_device_extensions (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    extension_order INTEGER NOT NULL CHECK (extension_order >= 0),
    name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order, extension_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_devices (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_device_instances (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    name TEXT NOT NULL,
    brief_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_devices (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_devices (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    kind TEXT NOT NULL,
    tag TEXT,
    fixed_image TEXT,
    mandatory TEXT,
    interface TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_displays (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    tag TEXT,
    kind TEXT NOT NULL CHECK (kind IN ('raster', 'vector', 'lcd', 'svg', 'unknown')),
    rotation TEXT CHECK (rotation IS NULL OR rotation IN ('0', '90', '180', '270')),
    flip_x INTEGER NOT NULL CHECK (flip_x IN (0, 1)),
    flip_x_specified INTEGER NOT NULL CHECK (flip_x_specified IN (0, 1)),
    width TEXT,
    height TEXT,
    refresh TEXT NOT NULL,
    pixel_clock TEXT,
    horizontal_total TEXT,
    horizontal_blank_end TEXT,
    horizontal_blank_start TEXT,
    vertical_total TEXT,
    vertical_blank_end TEXT,
    vertical_blank_start TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_drivers (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    status TEXT NOT NULL CHECK (status IN ('good', 'imperfect', 'preliminary')),
    emulation TEXT NOT NULL CHECK (emulation IN ('good', 'imperfect', 'preliminary')),
    cocktail TEXT CHECK (cocktail IS NULL OR cocktail IN ('good', 'imperfect', 'preliminary')),
    savestate TEXT NOT NULL CHECK (savestate IN ('supported', 'unsupported')),
    requires_artwork INTEGER NOT NULL CHECK (requires_artwork IN (0, 1)),
    requires_artwork_specified INTEGER NOT NULL CHECK (requires_artwork_specified IN (0, 1)),
    unofficial INTEGER NOT NULL CHECK (unofficial IN (0, 1)),
    unofficial_specified INTEGER NOT NULL CHECK (unofficial_specified IN (0, 1)),
    no_sound_hardware INTEGER NOT NULL CHECK (no_sound_hardware IN (0, 1)),
    no_sound_hardware_specified INTEGER NOT NULL CHECK (no_sound_hardware_specified IN (0, 1)),
    incomplete INTEGER NOT NULL CHECK (incomplete IN (0, 1)),
    incomplete_specified INTEGER NOT NULL CHECK (incomplete_specified IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machines (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    source_file TEXT,
    description TEXT NOT NULL,
    description_source_order INTEGER NOT NULL CHECK (typeof(description_source_order)='integer' AND description_source_order>=0),
    description_line INTEGER NOT NULL CHECK (description_line > 0),
    description_column INTEGER NOT NULL CHECK (description_column > 0),
    year TEXT,
    year_source_order INTEGER CHECK (year_source_order IS NULL OR (typeof(year_source_order)='integer' AND year_source_order>=0)),
    year_line INTEGER,
    year_column INTEGER,
    manufacturer TEXT,
    manufacturer_source_order INTEGER CHECK (manufacturer_source_order IS NULL OR (typeof(manufacturer_source_order)='integer' AND manufacturer_source_order>=0)),
    manufacturer_line INTEGER,
    manufacturer_column INTEGER,
    is_device INTEGER NOT NULL CHECK (is_device IN (0, 1)),
    is_device_specified INTEGER NOT NULL CHECK (is_device_specified IN (0, 1)),
    runnable INTEGER NOT NULL CHECK (runnable IN (0, 1)),
    runnable_specified INTEGER NOT NULL CHECK (runnable_specified IN (0, 1)),
    is_bios INTEGER NOT NULL CHECK (is_bios IN (0, 1)),
    is_bios_specified INTEGER NOT NULL CHECK (is_bios_specified IN (0, 1)),
    is_mechanical INTEGER NOT NULL CHECK (is_mechanical IN (0, 1)),
    is_mechanical_specified INTEGER NOT NULL CHECK (is_mechanical_specified IN (0, 1)),
    attributes_line INTEGER NOT NULL CHECK (attributes_line > 0),
    attributes_column INTEGER NOT NULL CHECK (attributes_column > 0),
    CHECK ((year_line IS NULL) = (year_column IS NULL)),
    CHECK ((year IS NULL) = (year_line IS NULL)),
    CHECK ((year IS NULL) = (year_source_order IS NULL)),
    CHECK ((manufacturer_line IS NULL) = (manufacturer_column IS NULL)),
    CHECK ((manufacturer IS NULL) = (manufacturer_line IS NULL)),
    CHECK ((manufacturer IS NULL) = (manufacturer_source_order IS NULL)),
    PRIMARY KEY (set_id)
) WITHOUT ROWID;
CREATE TABLE mame_machine_features (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'protection', 'timing', 'graphics', 'palette', 'sound', 'capture',
        'camera', 'microphone', 'controls', 'keyboard', 'mouse', 'media',
        'disk', 'printer', 'tape', 'punch', 'drum', 'rom', 'comms', 'lan', 'wan'
    )),
    status TEXT CHECK (status IS NULL OR status IN ('unemulated', 'imperfect')),
    overall TEXT CHECK (overall IS NULL OR overall IN ('unemulated', 'imperfect')),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_input_controls (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    control_order INTEGER NOT NULL CHECK (control_order >= 0),
    control_type TEXT NOT NULL,
    player TEXT,
    buttons TEXT,
    minimum TEXT,
    maximum TEXT,
    sensitivity TEXT,
    keydelta TEXT,
    reverse INTEGER NOT NULL CHECK (reverse IN (0, 1)),
    reverse_specified INTEGER NOT NULL CHECK (reverse_specified IN (0, 1)),
    ways TEXT,
    ways2 TEXT,
    ways3 TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order, control_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_inputs (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_inputs (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    service INTEGER NOT NULL CHECK (service IN (0, 1)),
    service_specified INTEGER NOT NULL CHECK (service_specified IN (0, 1)),
    tilt INTEGER NOT NULL CHECK (tilt IN (0, 1)),
    tilt_specified INTEGER NOT NULL CHECK (tilt_specified IN (0, 1)),
    players TEXT NOT NULL,
    coins TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_ports (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    tag TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_ram_options (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    name TEXT NOT NULL,
    default_value TEXT,
    text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_samples (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'mame_sample' CHECK (claim_kind = 'mame_sample'),
    name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (occurrence_id, claim_kind) REFERENCES asset_occurrences (occurrence_id, claim_kind) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_slot_options (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    option_order INTEGER NOT NULL CHECK (option_order >= 0),
    name TEXT NOT NULL,
    devname TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    default_specified INTEGER NOT NULL CHECK (default_specified IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order, option_order),
    FOREIGN KEY (set_id, element_order)
        REFERENCES mame_machine_slots (set_id, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_slots (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_software_lists (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    tag TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('original', 'compatible')),
    filter TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_sounds (
    set_id INTEGER NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    channels TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, element_order),
    FOREIGN KEY (set_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_rom_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'mame_rom' CHECK (claim_kind = 'mame_rom'),
    name TEXT NOT NULL,
    size_text TEXT,
    size INTEGER GENERATED ALWAYS AS (
        CASE WHEN length(size_text)>0 AND instr(size_text,char(0))=0
            AND size_text NOT GLOB '*[^0-9]*'
            AND (length(ltrim(size_text,'0'))<19
                OR (length(ltrim(size_text,'0'))=19
                    AND ltrim(size_text,'0')<='9223372036854775807'))
        THEN CAST(size_text AS INTEGER) END
    ) VIRTUAL,
    crc_text TEXT,
    sha1_text TEXT,
    evidence_scope TEXT NOT NULL,
    evidence_provenance TEXT NOT NULL,
    dump_status TEXT,
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0,1)),
    source_order INTEGER NOT NULL CHECK (typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    region TEXT,
    bios TEXT,
    offset_text TEXT,
    optional INTEGER NOT NULL CHECK (optional IN (0,1)),
    optional_specified INTEGER NOT NULL CHECK (optional_specified IN (0,1)),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TABLE no_intro_pc_games (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    archive_id TEXT,
    description TEXT,
    description_line INTEGER,
    description_column INTEGER,
    name_alt TEXT,
    region TEXT,
    version TEXT,
    bios_text TEXT,
    languages_present INTEGER NOT NULL DEFAULT 0 CHECK (languages_present IN (0,1)),
    PRIMARY KEY (set_id),
    CHECK ((description_line IS NULL) = (description_column IS NULL)),
    CHECK (description_line IS NULL OR description_line > 0),
    CHECK (description_column IS NULL OR description_column > 0)
) WITHOUT ROWID;
CREATE TABLE no_intro_pc_languages (
    set_id INTEGER NOT NULL REFERENCES no_intro_pc_games(set_id) ON DELETE RESTRICT,
    language_order INTEGER NOT NULL CHECK (language_order >= 0),
    language TEXT NOT NULL,
    PRIMARY KEY (set_id, language_order)
) WITHOUT ROWID;
CREATE TABLE no_intro_pc_clone_markers (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES no_intro_pc_games(set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE no_intro_pc_clone_links (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES no_intro_pc_games(set_id) ON DELETE RESTRICT,
    target_archive_id TEXT NOT NULL CHECK (target_archive_id <> '' AND target_archive_id NOT GLOB '*[^0-9]*')
) WITHOUT ROWID;
CREATE TABLE no_intro_pc_merge_links (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES no_intro_pc_games(set_id) ON DELETE RESTRICT,
    target_archive_id TEXT NOT NULL CHECK (target_archive_id <> '' AND target_archive_id NOT GLOB '*[^0-9]*')
) WITHOUT ROWID;
CREATE TRIGGER no_intro_pc_languages_unpublished_owner_insert BEFORE INSERT ON no_intro_pc_languages
WHEN EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
             JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'published P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_languages_immutable_update BEFORE UPDATE ON no_intro_pc_languages
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_languages_immutable_delete BEFORE DELETE ON no_intro_pc_languages
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_markers_unpublished_owner_insert BEFORE INSERT ON no_intro_pc_clone_markers
WHEN EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
             JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'published P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_markers_immutable_update BEFORE UPDATE ON no_intro_pc_clone_markers
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_markers_immutable_delete BEFORE DELETE ON no_intro_pc_clone_markers
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_links_unpublished_owner_insert BEFORE INSERT ON no_intro_pc_clone_links
WHEN EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
             JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'published P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_links_immutable_update BEFORE UPDATE ON no_intro_pc_clone_links
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_clone_links_immutable_delete BEFORE DELETE ON no_intro_pc_clone_links
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_merge_links_unpublished_owner_insert BEFORE INSERT ON no_intro_pc_merge_links
WHEN EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
             JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'published P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_merge_links_immutable_update BEFORE UPDATE ON no_intro_pc_merge_links
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_merge_links_immutable_delete BEFORE DELETE ON no_intro_pc_merge_links
BEGIN SELECT RAISE(ABORT, 'P/C facts are immutable'); END;
CREATE TRIGGER no_intro_pc_language_presence_insert BEFORE INSERT ON no_intro_pc_languages
WHEN NOT EXISTS (SELECT 1 FROM no_intro_pc_games WHERE set_id = NEW.set_id AND languages_present = 1)
BEGIN SELECT RAISE(ABORT, 'P/C language requires a present language declaration'); END;
CREATE TRIGGER no_intro_pc_clone_marker_excludes_link BEFORE INSERT ON no_intro_pc_clone_markers
WHEN EXISTS (SELECT 1 FROM no_intro_pc_clone_links WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'P/C clone is a marker or a link, never both'); END;
CREATE TRIGGER no_intro_pc_clone_link_excludes_marker BEFORE INSERT ON no_intro_pc_clone_links
WHEN EXISTS (SELECT 1 FROM no_intro_pc_clone_markers WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'P/C clone is a marker or a link, never both'); END;
CREATE TABLE no_intro_pc_file_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'no_intro_pc_file' CHECK (claim_kind = 'no_intro_pc_file'),
    name TEXT NOT NULL,
    size INTEGER CHECK (size IS NULL OR size >= 0),
    evidence_scope TEXT NOT NULL,
    evidence_provenance TEXT NOT NULL,
    merge_name TEXT,
    dump_status TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (occurrence_id,claim_kind) REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT
);
CREATE TABLE occurrence_content_conflicts (
    occurrence_id INTEGER NOT NULL REFERENCES asset_occurrences(occurrence_id) ON DELETE RESTRICT,
    candidate_content_uuid BLOB NOT NULL REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (reason IN ('ambiguous_alias', 'contradictory_assertions', 'disputed_alias')),
    PRIMARY KEY (occurrence_id, candidate_content_uuid)
) WITHOUT ROWID;
CREATE TABLE occurrence_digest_assertions (
    occurrence_id INTEGER NOT NULL REFERENCES asset_occurrences(occurrence_id) ON DELETE RESTRICT,
    digest_id INTEGER NOT NULL REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    scope TEXT NOT NULL CHECK (length(scope) > 0),
    provenance TEXT NOT NULL CHECK (provenance IN ('source_declared', 'computed', 'unknown')),
    PRIMARY KEY (occurrence_id, digest_id, scope, provenance)
) WITHOUT ROWID;
CREATE TABLE parser_interpretations (
    interpretation_key TEXT PRIMARY KEY NOT NULL,
    format              TEXT NOT NULL,
    parser_name         TEXT,
    parser_version      TEXT,
    rules_version       TEXT
);
CREATE TABLE publishing_sources (
    source_key  TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    locator     TEXT
);
CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY NOT NULL CHECK (set_group_id > 0),
    snapshot_key TEXT NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('root', 'software_list')),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    CHECK (kind <> 'root' OR list_order = 0),
    UNIQUE (snapshot_key, kind, list_order)
);
CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY NOT NULL CHECK (set_id > 0),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id) ON DELETE RESTRICT,
    source_element_kind TEXT NOT NULL CHECK (source_element_kind IN (
        'mame_machine', 'software_item', 'logiqx_game', 'cmp_set',
        'no_intro_pc_game', 'no_intro_dat_game', 'no_intro_database_game'
    )),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    set_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id, list_order),
    UNIQUE (set_id, source_element_kind)
);
CREATE TABLE relationship_rationales (
    assertion_key TEXT PRIMARY KEY NOT NULL REFERENCES relationship_assertions(assertion_key) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (typeof(reason) = 'text' AND length(trim(reason)) > 0)
) WITHOUT ROWID;
CREATE TABLE relationship_comparisons (
    assertion_key TEXT PRIMARY KEY NOT NULL REFERENCES relationship_assertions(assertion_key) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('compatible','candidate','contradictory','ambiguous','unknown'))
) WITHOUT ROWID;
CREATE TABLE relationship_comparison_fields (
    assertion_key TEXT NOT NULL REFERENCES relationship_comparisons(assertion_key) ON DELETE RESTRICT,
    disposition TEXT NOT NULL CHECK (disposition IN ('agreement','contradiction')),
    position INTEGER NOT NULL CHECK (typeof(position) = 'integer' AND position >= 0),
    field TEXT NOT NULL CHECK (field IN ('sha1','md5','crc','size')),
    PRIMARY KEY (assertion_key, disposition, position),
    UNIQUE (assertion_key, field)
) WITHOUT ROWID;
CREATE TABLE relationship_evidence_publications (
    assertion_key TEXT PRIMARY KEY NOT NULL REFERENCES relationship_assertions(assertion_key) ON DELETE RESTRICT,
    evidence_kind TEXT NOT NULL CHECK (evidence_kind IN ('rationale','catalog_comparison'))
) WITHOUT ROWID;
CREATE TABLE "relationship_assertion_support" (
    assertion_key TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (typeof(position)='integer' AND position >= 0),
    supported_assertion_key TEXT NOT NULL,
    PRIMARY KEY (assertion_key, position)
) WITHOUT ROWID;
CREATE TABLE relationship_assertions (
    assertion_key TEXT PRIMARY KEY NOT NULL,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    origin TEXT NOT NULL CHECK (origin IN (
        'source_assertion', 'derived_candidate', 'user_conclusion'
    )),
    source_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    source_field TEXT,
    source_line BIGINT CHECK (source_line IS NULL OR source_line > 0),
    source_column BIGINT CHECK (source_column IS NULL OR source_column > 0),

    generic_subject_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    subject_kind TEXT NOT NULL,
    subject_set_id INTEGER REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    subject_archive_id INTEGER REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    generic_subject_a TEXT,
    generic_subject_b TEXT,
    generic_subject_c BIGINT,
    source_subject_a TEXT,
    source_subject_b TEXT,
    source_subject_c BIGINT,
    subject_snapshot_key TEXT GENERATED ALWAYS AS (
        CASE WHEN origin = 'source_assertion' THEN source_snapshot_key
             ELSE generic_subject_snapshot_key END
    ) VIRTUAL,
    generic_target_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    target_kind TEXT NOT NULL,
    target_set_id INTEGER REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    generic_target_a TEXT,
    generic_target_b TEXT,
    generic_target_c BIGINT,
    source_target_a TEXT,
    source_target_b TEXT,
    source_target_c BIGINT,
    target_snapshot_key TEXT GENERATED ALWAYS AS (
        CASE WHEN origin = 'source_assertion' THEN source_snapshot_key
             ELSE generic_target_snapshot_key END
    ) VIRTUAL,
    rule_version TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (origin = 'source_assertion' AND source_snapshot_key IS NOT NULL
            AND source_field IS NOT NULL AND generic_subject_snapshot_key IS NULL
            AND generic_target_snapshot_key IS NULL
            AND ((subject_kind = 'no_intro_archive' AND source_subject_a IS NULL)
                 OR (subject_kind <> 'no_intro_archive' AND source_subject_a IS NOT NULL))
            AND ((target_kind = 'no_intro_archive_reference' AND source_target_a IS NULL)
                 OR (target_kind <> 'no_intro_archive_reference' AND source_target_a IS NOT NULL))
            AND generic_subject_a IS NULL
            AND generic_subject_b IS NULL AND generic_subject_c IS NULL
            AND generic_target_a IS NULL AND generic_target_b IS NULL
            AND generic_target_c IS NULL)
        OR (origin != 'source_assertion' AND source_snapshot_key IS NULL
            AND source_field IS NULL AND generic_subject_a IS NOT NULL
            AND generic_target_a IS NOT NULL AND source_subject_a IS NULL
            AND source_subject_b IS NULL AND source_subject_c IS NULL
            AND source_target_a IS NULL AND source_target_b IS NULL
            AND source_target_c IS NULL)
    ),
    CHECK (origin = 'source_assertion' OR
        ((subject_kind = 'catalog_set' AND generic_subject_b IS NULL AND generic_subject_c IS NULL)
         OR (subject_kind IN ('software_item', 'asset_requirement', 'content_object', 'external_record')
             AND generic_subject_b IS NOT NULL
             AND ((subject_kind = 'asset_requirement') = (generic_subject_c IS NOT NULL))))),
    CHECK (origin = 'source_assertion' OR
        ((target_kind = 'catalog_set' AND generic_target_b IS NULL AND generic_target_c IS NULL)
         OR (target_kind IN ('software_item', 'asset_requirement', 'content_object', 'external_record')
             AND generic_target_b IS NOT NULL
             AND ((target_kind = 'asset_requirement') = (generic_target_c IS NOT NULL))))),
    CHECK ((origin = 'derived_candidate') = (rule_version IS NOT NULL)),
    CHECK (origin != 'source_assertion' OR
        ((subject_kind IN ('catalog_set', 'software_item', 'asset_requirement')
         AND target_kind IN ('catalog_set', 'software_item', 'asset_requirement'))
         OR (subject_kind = 'no_intro_archive' AND target_kind = 'no_intro_archive_reference'))),
    CHECK ((subject_kind = 'no_intro_archive') = (subject_archive_id IS NOT NULL)),
    CHECK (origin <> 'source_assertion' OR
        ((source_field IN ('archive_clone','archive_mergeof')) = (subject_kind='no_intro_archive'))),
    CHECK (subject_kind <> 'no_intro_archive' OR
        (origin = 'source_assertion' AND target_kind = 'no_intro_archive_reference'
         AND source_field IN ('archive_clone', 'archive_mergeof')
         AND relation_type = CASE source_field WHEN 'archive_clone' THEN 'source_parent_clone'
                                              ELSE 'alternate_representation_of' END
         AND subject_set_id IS NULL AND target_set_id IS NULL
         AND source_subject_b IS NULL AND source_subject_c IS NULL
         AND source_target_b IS NULL AND source_target_c IS NULL)),
    CHECK (origin != 'source_assertion' OR source_field != 'merge' OR
        (subject_kind = 'asset_requirement' AND target_kind = 'asset_requirement')),
    CHECK (origin != 'source_assertion' OR
        source_field NOT IN ('romof', 'sampleof', 'device_ref', 'parent_name') OR
        (subject_kind = 'catalog_set' AND target_kind = 'catalog_set')),
    CHECK (origin != 'source_assertion' OR source_field != 'cloneof' OR
        ((subject_kind = 'software_item' AND target_kind = 'software_item') OR
         (subject_kind = 'catalog_set' AND target_kind = 'catalog_set'))),
    CHECK (origin != 'source_assertion' OR (
        (subject_kind != 'catalog_set' OR
            (source_subject_b IS NULL AND source_subject_c IS NULL)) AND
        (target_kind != 'catalog_set' OR
            (source_target_b IS NULL AND source_target_c IS NULL)) AND
        (subject_kind != 'software_item' OR
            (source_subject_b IS NOT NULL AND source_subject_c IS NULL)) AND
        (target_kind != 'software_item' OR
            (source_target_b IS NOT NULL AND source_target_c IS NULL)) AND
        (subject_kind != 'asset_requirement' OR
            (source_subject_b IS NOT NULL AND source_subject_c IS NOT NULL)) AND
        (target_kind != 'asset_requirement' OR
            (source_target_b IS NOT NULL AND source_target_c IS NOT NULL))
    ))
);
CREATE TABLE "relationship_reviews" (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE,
    assertion_key TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected', 'withdrawn', 'superseded')),
    note TEXT NOT NULL,
    superseded_by_assertion_key TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'superseded') = (superseded_by_assertion_key IS NOT NULL))
);
CREATE TABLE rom_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    parent_path     TEXT    NOT NULL,
    parent_game_name TEXT,
    path            TEXT    NOT NULL,
    name            TEXT    CONSTRAINT file_name NOT NULL,
    crc             BLOB,
    sha1            BLOB    NOT NULL,
    md5             BLOB,
    xxhash3         BLOB    NOT NULL,
    in_archive      BOOLEAN NOT NULL
, archive_backend TEXT
        CHECK (archive_backend IS NULL OR archive_backend IN ('zip', '7z', 'rar')), archive_member_index BIGINT
        CHECK (archive_member_index IS NULL OR archive_member_index >= 0), scan_root TEXT, scan_run TEXT, observed_size BIGINT
        CHECK (observed_size IS NULL OR observed_size >= 0), source_fingerprint BLOB
        CHECK (source_fingerprint IS NULL OR length(source_fingerprint) = 20), scan_provenance TEXT
        CHECK (scan_provenance IS NULL OR scan_provenance = 'streamed_sha1_xxh3_v1'), bare_file_cache_stamp BLOB
        CHECK (bare_file_cache_stamp IS NULL OR length(bare_file_cache_stamp) = 32), cache_reused BOOLEAN NOT NULL DEFAULT FALSE
        CHECK (cache_reused IN (FALSE, TRUE)), physical_path TEXT);
CREATE TABLE snapshot_publications (
    catalog_key TEXT NOT NULL,
    document_key TEXT NOT NULL,
    interpretation_key TEXT NOT NULL,
    snapshot_key TEXT NOT NULL UNIQUE,
    published_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (catalog_key, document_key, interpretation_key),
    FOREIGN KEY (snapshot_key, catalog_key, document_key, interpretation_key)
        REFERENCES catalog_snapshots
            (snapshot_key, catalog_key, document_key, interpretation_key)
        ON DELETE RESTRICT
);
CREATE TABLE software_areas (
    area_id       INTEGER PRIMARY KEY,
    part_id       INTEGER NOT NULL,
    record_id     INTEGER NOT NULL,
    area_kind     TEXT NOT NULL CHECK (area_kind IN ('data', 'disk')),
    area_order    INTEGER NOT NULL CHECK (area_order >= 0),
    UNIQUE (area_id, record_id),
    UNIQUE (part_id, area_order),
    FOREIGN KEY (part_id, record_id)
        REFERENCES software_parts (part_id, record_id) ON DELETE RESTRICT
);
CREATE TABLE software_data_areas (
    area_id       INTEGER PRIMARY KEY REFERENCES software_areas (area_id) ON DELETE RESTRICT,
    area_name     TEXT NOT NULL,
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    declared_size_text TEXT NOT NULL,
    declared_size INTEGER GENERATED ALWAYS AS (
CASE
        WHEN declared_size_text IS NULL OR declared_size_text = '' THEN NULL
        WHEN substr(declared_size_text, 1, 2) GLOB '0[xX]'
             AND length(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END) > 0
             AND CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END NOT GLOB '*[^0-9a-fA-F]*'
             AND length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) <= 16
             AND (length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) < 16 OR substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), 1, 1) <= '7')
            THEN (
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 0 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 1 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -2, 1))) - 1) * 16 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 2 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -3, 1))) - 1) * 256 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 3 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -4, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 4 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -5, 1))) - 1) * 65536 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 5 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -6, 1))) - 1) * 1048576 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 6 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -7, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 7 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -8, 1))) - 1) * 268435456 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 8 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -9, 1))) - 1) * 4294967296 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 9 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -10, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 10 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -11, 1))) - 1) * 1099511627776 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 11 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -12, 1))) - 1) * 17592186044416 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 12 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -13, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 13 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -14, 1))) - 1) * 4503599627370496 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 14 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -15, 1))) - 1) * 72057594037927936 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0')) > 15 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(declared_size_text, 3, 1) = '+' THEN substr(declared_size_text, 4) ELSE substr(declared_size_text, 3) END, '0'), -16, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(declared_size_text) > 1
             AND substr(declared_size_text, 1, 1) = '0'
             AND length(substr(declared_size_text, 2)) > 0
             AND substr(declared_size_text, 2) NOT GLOB '*[^0-7]*'
             AND length(ltrim(substr(declared_size_text, 2), '0')) <= 21
            THEN (
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 0 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 1 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -2, 1))) - 1) * 8 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 2 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -3, 1))) - 1) * 64 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 3 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -4, 1))) - 1) * 512 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 4 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -5, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 5 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -6, 1))) - 1) * 32768 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 6 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -7, 1))) - 1) * 262144 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 7 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -8, 1))) - 1) * 2097152 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 8 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -9, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 9 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -10, 1))) - 1) * 134217728 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 10 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -11, 1))) - 1) * 1073741824 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 11 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -12, 1))) - 1) * 8589934592 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 12 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -13, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 13 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -14, 1))) - 1) * 549755813888 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 14 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -15, 1))) - 1) * 4398046511104 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 15 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -16, 1))) - 1) * 35184372088832 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 16 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -17, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 17 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -18, 1))) - 1) * 2251799813685248 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 18 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -19, 1))) - 1) * 18014398509481984 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 19 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -20, 1))) - 1) * 144115188075855872 ELSE 0 END +
            CASE WHEN length(ltrim(substr(declared_size_text, 2), '0')) > 20 THEN (instr('01234567', lower(substr(ltrim(substr(declared_size_text, 2), '0'), -21, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(CASE WHEN substr(declared_size_text, 1, 1) = '+' THEN substr(declared_size_text, 2) ELSE declared_size_text END) > 0
             AND CASE WHEN substr(declared_size_text, 1, 1) = '+' THEN substr(declared_size_text, 2) ELSE declared_size_text END NOT GLOB '*[^0-9]*'
             AND (substr(declared_size_text, 1, 1) <> '0' OR declared_size_text = '0')
             AND length(ltrim(CASE WHEN substr(declared_size_text, 1, 1) = '+' THEN substr(declared_size_text, 2) ELSE declared_size_text END, '0')) <= 19
             AND (length(ltrim(CASE WHEN substr(declared_size_text, 1, 1) = '+' THEN substr(declared_size_text, 2) ELSE declared_size_text END, '0')) < 19 OR ltrim(CASE WHEN substr(declared_size_text, 1, 1) = '+' THEN substr(declared_size_text, 2) ELSE declared_size_text END, '0') <= '9223372036854775807')
            THEN CAST(declared_size_text AS INTEGER)
        ELSE NULL
    END
    ) VIRTUAL CHECK (declared_size IS NULL OR (typeof(declared_size) = 'integer' AND declared_size >= 0)),
    width         INTEGER NOT NULL CHECK (width IN (8, 16, 32, 64)),
    width_specified INTEGER NOT NULL CHECK (width_specified IN (0, 1)),
    endianness    TEXT NOT NULL CHECK (endianness IN ('little', 'big')),
    endianness_specified INTEGER NOT NULL CHECK (endianness_specified IN (0, 1)),
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    CHECK (width_specified = 1 OR width = 8),
    CHECK (endianness_specified = 1 OR endianness = 'little')
);
CREATE TABLE software_disk_areas (
    area_id       INTEGER PRIMARY KEY REFERENCES software_areas (area_id) ON DELETE RESTRICT,
    area_name     TEXT NOT NULL,
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
);
CREATE TABLE software_disk_entries (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    record_id     INTEGER NOT NULL,
    area_id       INTEGER NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name          TEXT NOT NULL,
    evidence_scope TEXT NOT NULL,
    sha1_text     TEXT,
    dump_status   TEXT NOT NULL CHECK (dump_status IN ('good', 'baddump', 'nodump')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    writeable     INTEGER NOT NULL CHECK (writeable IN (0, 1)),
    writeable_specified INTEGER NOT NULL CHECK (writeable_specified IN (0, 1)),
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (area_id, component_order),
    UNIQUE (area_id, source_order),
    UNIQUE (occurrence_id, record_id),
    FOREIGN KEY (occurrence_id, record_id)
        REFERENCES asset_occurrences (occurrence_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (area_id, record_id)
        REFERENCES software_areas (area_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (area_id) REFERENCES software_disk_areas (area_id) ON DELETE RESTRICT
);
CREATE TABLE software_file_declarations (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    record_id     INTEGER NOT NULL,
    declared_size INTEGER CHECK (declared_size IS NULL),
    UNIQUE (occurrence_id, record_id),
    FOREIGN KEY (occurrence_id, record_id)
        REFERENCES asset_occurrences (occurrence_id, record_id) ON DELETE RESTRICT
);
CREATE TABLE software_file_uses (
    occurrence_id           INTEGER PRIMARY KEY NOT NULL,
    record_id               INTEGER NOT NULL,
    declaration_occurrence_id INTEGER,
    operation               TEXT NOT NULL CHECK (operation IN (
        'load', 'continue', 'reload', 'reload_plain', 'ignore', 'fill', 'disk'
    )),
    UNIQUE (occurrence_id, record_id),
    FOREIGN KEY (occurrence_id, record_id)
        REFERENCES asset_occurrences (occurrence_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (declaration_occurrence_id, record_id)
        REFERENCES software_file_declarations (occurrence_id, record_id) ON DELETE RESTRICT
);
CREATE VIEW software_item_dependencies AS
SELECT record_id, 'clone_of' AS dependency_kind, clone_of AS target_name
FROM software_items WHERE clone_of IS NOT NULL;
CREATE TABLE software_item_info (
    record_id     INTEGER NOT NULL,
    value_order   INTEGER NOT NULL CHECK (value_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name          TEXT NOT NULL,
    value         TEXT,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (record_id, value_order),
    UNIQUE (record_id, source_order),
    FOREIGN KEY (record_id) REFERENCES software_items (record_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_item_shared_features (
    record_id     INTEGER NOT NULL,
    value_order   INTEGER NOT NULL CHECK (value_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name          TEXT NOT NULL,
    value         TEXT,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (record_id, value_order),
    UNIQUE (record_id, source_order),
    FOREIGN KEY (record_id) REFERENCES software_items (record_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_items (
    record_id   INTEGER PRIMARY KEY NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    clone_of    TEXT,
    supported   TEXT NOT NULL CHECK (supported IN ('yes', 'partial', 'no')),
    supported_specified INTEGER NOT NULL CHECK (supported_specified IN (0, 1)),
    description TEXT NOT NULL,
    year        TEXT NOT NULL,
    publisher   TEXT NOT NULL,
    notes       TEXT,
    FOREIGN KEY (record_id) REFERENCES catalog_sets (set_id) ON DELETE RESTRICT
);
CREATE TABLE software_lists (
    namespace_id  INTEGER PRIMARY KEY NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name TEXT NOT NULL,
    description   TEXT,
    notes         TEXT,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (namespace_id) REFERENCES catalog_set_groups (set_group_id)
        ON DELETE RESTRICT
);
CREATE TABLE software_documents (
    snapshot_key TEXT PRIMARY KEY NOT NULL,
    envelope_kind TEXT NOT NULL CHECK (envelope_kind IN ('single_list', 'plural_lists')),
    FOREIGN KEY (snapshot_key) REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_wrapper_headers (
    snapshot_key TEXT PRIMARY KEY NOT NULL,
    build TEXT,
    FOREIGN KEY (snapshot_key) REFERENCES software_documents (snapshot_key) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_list_text_positions (
    namespace_id INTEGER NOT NULL,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind = 3),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (namespace_id, field_kind),
    UNIQUE (namespace_id, source_order),
    FOREIGN KEY (namespace_id) REFERENCES software_lists (namespace_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_item_text_positions (
    record_id INTEGER NOT NULL,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind IN (0, 1, 2, 3)),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (record_id, field_kind),
    UNIQUE (record_id, source_order),
    FOREIGN KEY (record_id) REFERENCES software_items (record_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_part_dip_values (
    part_id         INTEGER NOT NULL,
    dipswitch_order INTEGER NOT NULL CHECK (dipswitch_order >= 0),
    value_order     INTEGER NOT NULL CHECK (value_order >= 0),
    source_order    INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name            TEXT NOT NULL,
    value           TEXT NOT NULL,
    is_default      INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    default_specified INTEGER NOT NULL CHECK (default_specified IN (0, 1)),
    source_line     INTEGER NOT NULL CHECK (source_line > 0),
    source_column   INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (part_id, dipswitch_order, value_order),
    UNIQUE (part_id, dipswitch_order, source_order),
    FOREIGN KEY (part_id, dipswitch_order)
        REFERENCES software_part_dipswitches (part_id, dipswitch_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_part_dipswitches (
    part_id        INTEGER NOT NULL,
    dipswitch_order INTEGER NOT NULL CHECK (dipswitch_order >= 0),
    source_order    INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name           TEXT NOT NULL,
    tag            TEXT NOT NULL,
    mask           TEXT NOT NULL,
    source_line    INTEGER NOT NULL CHECK (source_line > 0),
    source_column  INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (part_id, dipswitch_order),
    UNIQUE (part_id, source_order),
    FOREIGN KEY (part_id) REFERENCES software_parts (part_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_part_features (
    part_id       INTEGER NOT NULL,
    value_order   INTEGER NOT NULL CHECK (value_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name          TEXT NOT NULL,
    value         TEXT,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (part_id, value_order),
    UNIQUE (part_id, source_order),
    FOREIGN KEY (part_id) REFERENCES software_parts (part_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE software_parts (
    part_id       INTEGER PRIMARY KEY,
    record_id     INTEGER NOT NULL,
    part_name     TEXT NOT NULL,
    part_order    INTEGER NOT NULL CHECK (part_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    interface     TEXT NOT NULL,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (part_id, record_id),
    UNIQUE (record_id, part_order),
    UNIQUE (record_id, source_order),
    FOREIGN KEY (record_id) REFERENCES software_items (record_id) ON DELETE RESTRICT
);
CREATE TABLE software_rom_entries (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    record_id     INTEGER NOT NULL,
    area_id       INTEGER NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    source_order  INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    name          TEXT,
    evidence_scope TEXT NOT NULL,
    size_text     TEXT,
    size INTEGER GENERATED ALWAYS AS (
CASE
        WHEN size_text IS NULL OR size_text = '' THEN NULL
        WHEN substr(size_text, 1, 2) GLOB '0[xX]'
             AND length(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END) > 0
             AND CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END NOT GLOB '*[^0-9a-fA-F]*'
             AND length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) <= 16
             AND (length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) < 16 OR substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), 1, 1) <= '7')
            THEN (
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 0 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 1 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -2, 1))) - 1) * 16 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 2 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -3, 1))) - 1) * 256 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 3 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -4, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 4 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -5, 1))) - 1) * 65536 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 5 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -6, 1))) - 1) * 1048576 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 6 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -7, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 7 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -8, 1))) - 1) * 268435456 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 8 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -9, 1))) - 1) * 4294967296 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 9 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -10, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 10 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -11, 1))) - 1) * 1099511627776 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 11 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -12, 1))) - 1) * 17592186044416 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 12 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -13, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 13 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -14, 1))) - 1) * 4503599627370496 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 14 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -15, 1))) - 1) * 72057594037927936 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0')) > 15 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(size_text, 3, 1) = '+' THEN substr(size_text, 4) ELSE substr(size_text, 3) END, '0'), -16, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(size_text) > 1
             AND substr(size_text, 1, 1) = '0'
             AND length(substr(size_text, 2)) > 0
             AND substr(size_text, 2) NOT GLOB '*[^0-7]*'
             AND length(ltrim(substr(size_text, 2), '0')) <= 21
            THEN (
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 0 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 1 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -2, 1))) - 1) * 8 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 2 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -3, 1))) - 1) * 64 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 3 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -4, 1))) - 1) * 512 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 4 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -5, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 5 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -6, 1))) - 1) * 32768 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 6 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -7, 1))) - 1) * 262144 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 7 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -8, 1))) - 1) * 2097152 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 8 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -9, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 9 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -10, 1))) - 1) * 134217728 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 10 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -11, 1))) - 1) * 1073741824 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 11 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -12, 1))) - 1) * 8589934592 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 12 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -13, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 13 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -14, 1))) - 1) * 549755813888 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 14 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -15, 1))) - 1) * 4398046511104 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 15 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -16, 1))) - 1) * 35184372088832 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 16 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -17, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 17 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -18, 1))) - 1) * 2251799813685248 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 18 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -19, 1))) - 1) * 18014398509481984 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 19 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -20, 1))) - 1) * 144115188075855872 ELSE 0 END +
            CASE WHEN length(ltrim(substr(size_text, 2), '0')) > 20 THEN (instr('01234567', lower(substr(ltrim(substr(size_text, 2), '0'), -21, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(CASE WHEN substr(size_text, 1, 1) = '+' THEN substr(size_text, 2) ELSE size_text END) > 0
             AND CASE WHEN substr(size_text, 1, 1) = '+' THEN substr(size_text, 2) ELSE size_text END NOT GLOB '*[^0-9]*'
             AND (substr(size_text, 1, 1) <> '0' OR size_text = '0')
             AND length(ltrim(CASE WHEN substr(size_text, 1, 1) = '+' THEN substr(size_text, 2) ELSE size_text END, '0')) <= 19
             AND (length(ltrim(CASE WHEN substr(size_text, 1, 1) = '+' THEN substr(size_text, 2) ELSE size_text END, '0')) < 19 OR ltrim(CASE WHEN substr(size_text, 1, 1) = '+' THEN substr(size_text, 2) ELSE size_text END, '0') <= '9223372036854775807')
            THEN CAST(size_text AS INTEGER)
        ELSE NULL
    END
    ) VIRTUAL CHECK (size IS NULL OR (typeof(size) = 'integer' AND size >= 0)),
    offset_text   TEXT,
    offset INTEGER GENERATED ALWAYS AS (
CASE
        WHEN offset_text IS NULL OR offset_text = '' THEN NULL
        WHEN substr(offset_text, 1, 2) GLOB '0[xX]'
             AND length(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END) > 0
             AND CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END NOT GLOB '*[^0-9a-fA-F]*'
             AND length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) <= 16
             AND (length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) < 16 OR substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), 1, 1) <= '7')
            THEN (
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 0 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 1 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -2, 1))) - 1) * 16 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 2 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -3, 1))) - 1) * 256 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 3 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -4, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 4 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -5, 1))) - 1) * 65536 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 5 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -6, 1))) - 1) * 1048576 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 6 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -7, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 7 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -8, 1))) - 1) * 268435456 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 8 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -9, 1))) - 1) * 4294967296 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 9 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -10, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 10 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -11, 1))) - 1) * 1099511627776 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 11 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -12, 1))) - 1) * 17592186044416 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 12 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -13, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 13 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -14, 1))) - 1) * 4503599627370496 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 14 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -15, 1))) - 1) * 72057594037927936 ELSE 0 END +
            CASE WHEN length(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0')) > 15 THEN (instr('0123456789abcdef', lower(substr(ltrim(CASE WHEN substr(offset_text, 3, 1) = '+' THEN substr(offset_text, 4) ELSE substr(offset_text, 3) END, '0'), -16, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(offset_text) > 1
             AND substr(offset_text, 1, 1) = '0'
             AND length(substr(offset_text, 2)) > 0
             AND substr(offset_text, 2) NOT GLOB '*[^0-7]*'
             AND length(ltrim(substr(offset_text, 2), '0')) <= 21
            THEN (
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 0 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -1, 1))) - 1) * 1 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 1 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -2, 1))) - 1) * 8 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 2 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -3, 1))) - 1) * 64 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 3 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -4, 1))) - 1) * 512 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 4 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -5, 1))) - 1) * 4096 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 5 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -6, 1))) - 1) * 32768 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 6 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -7, 1))) - 1) * 262144 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 7 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -8, 1))) - 1) * 2097152 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 8 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -9, 1))) - 1) * 16777216 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 9 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -10, 1))) - 1) * 134217728 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 10 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -11, 1))) - 1) * 1073741824 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 11 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -12, 1))) - 1) * 8589934592 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 12 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -13, 1))) - 1) * 68719476736 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 13 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -14, 1))) - 1) * 549755813888 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 14 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -15, 1))) - 1) * 4398046511104 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 15 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -16, 1))) - 1) * 35184372088832 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 16 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -17, 1))) - 1) * 281474976710656 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 17 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -18, 1))) - 1) * 2251799813685248 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 18 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -19, 1))) - 1) * 18014398509481984 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 19 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -20, 1))) - 1) * 144115188075855872 ELSE 0 END +
            CASE WHEN length(ltrim(substr(offset_text, 2), '0')) > 20 THEN (instr('01234567', lower(substr(ltrim(substr(offset_text, 2), '0'), -21, 1))) - 1) * 1152921504606846976 ELSE 0 END
            )
        WHEN length(CASE WHEN substr(offset_text, 1, 1) = '+' THEN substr(offset_text, 2) ELSE offset_text END) > 0
             AND CASE WHEN substr(offset_text, 1, 1) = '+' THEN substr(offset_text, 2) ELSE offset_text END NOT GLOB '*[^0-9]*'
             AND (substr(offset_text, 1, 1) <> '0' OR offset_text = '0')
             AND length(ltrim(CASE WHEN substr(offset_text, 1, 1) = '+' THEN substr(offset_text, 2) ELSE offset_text END, '0')) <= 19
             AND (length(ltrim(CASE WHEN substr(offset_text, 1, 1) = '+' THEN substr(offset_text, 2) ELSE offset_text END, '0')) < 19 OR ltrim(CASE WHEN substr(offset_text, 1, 1) = '+' THEN substr(offset_text, 2) ELSE offset_text END, '0') <= '9223372036854775807')
            THEN CAST(offset_text AS INTEGER)
        ELSE NULL
    END
    ) VIRTUAL CHECK (offset IS NULL OR (typeof(offset) = 'integer' AND offset >= 0)),
    value         TEXT,
    crc_text      TEXT,
    sha1_text     TEXT,
    dump_status   TEXT NOT NULL CHECK (dump_status IN ('good', 'baddump', 'nodump')),
    status_specified INTEGER NOT NULL CHECK (status_specified IN (0, 1)),
    load_instruction TEXT,
    source_line   INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (area_id, component_order),
    UNIQUE (area_id, source_order),
    UNIQUE (occurrence_id, record_id),
    FOREIGN KEY (occurrence_id, record_id)
        REFERENCES asset_occurrences (occurrence_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (area_id, record_id)
        REFERENCES software_areas (area_id, record_id) ON DELETE RESTRICT,
    FOREIGN KEY (area_id) REFERENCES software_data_areas (area_id) ON DELETE RESTRICT
);
CREATE INDEX acquisition_attempts_acquisition_key_index ON acquisition_attempts (acquisition_key);
CREATE INDEX acquisition_attempts_document_key_index ON acquisition_attempts (document_key);
CREATE INDEX acquisition_attempts_source_key_index ON acquisition_attempts (source_key);
CREATE INDEX acquisitions_document_key_index ON acquisitions (document_key);
CREATE UNIQUE INDEX acquisitions_key_document_source_unique
    ON acquisitions (acquisition_key, document_key, source_key);
CREATE INDEX acquisitions_source_key_index ON acquisitions (source_key);
CREATE INDEX catalogs_source_key_index ON catalogs (source_key);
CREATE INDEX diagnostics_run_index ON import_diagnostics (run_key);
CREATE INDEX digest_value_lookup
    ON digest_values (algorithm, digest);
CREATE INDEX documents_byte_length_index ON documents (byte_length);
CREATE INDEX documents_sha1_index ON documents (sha1);
CREATE UNIQUE INDEX documents_sha256_unique ON documents (sha256)
    WHERE sha256 IS NOT NULL;
CREATE INDEX import_runs_catalog_key_index ON import_runs (catalog_key);
CREATE INDEX import_runs_document_key_index ON import_runs (document_key);
CREATE INDEX import_runs_snapshot_key_index ON import_runs (snapshot_key);
CREATE INDEX mame_bios_sets_name_index ON mame_bios_sets (name);
CREATE INDEX machine_switches_tag_index ON machine_switches (tag, name, set_id);
CREATE INDEX mame_machine_chips_name_index ON mame_machine_chips (name, set_id);
CREATE INDEX mame_machine_devices_tag_index ON mame_machine_devices (tag, set_id);
CREATE INDEX mame_machines_source_file_index ON mame_machines (source_file);
CREATE INDEX mame_samples_name_index ON mame_samples (name, occurrence_id);
CREATE INDEX mame_machine_software_lists_name_index ON mame_machine_software_lists (name, set_id);
CREATE INDEX occurrence_content_lookup ON asset_occurrences(content_uuid, occurrence_id)
    WHERE content_uuid IS NOT NULL;
CREATE INDEX occurrence_digest_lookup ON occurrence_digest_assertions(digest_id, occurrence_id);
CREATE INDEX catalog_set_name_lookup ON catalog_sets(set_group_id, set_name);
CREATE INDEX relationship_assertions_source_snapshot_index
    ON relationship_assertions (source_snapshot_key)
    WHERE origin = 'source_assertion';
CREATE INDEX relationship_assertions_subject_snapshot_kind_index
    ON relationship_assertions (generic_subject_snapshot_key, subject_kind)
    WHERE origin != 'source_assertion';
CREATE INDEX relationship_assertions_target_snapshot_kind_index
    ON relationship_assertions (generic_target_snapshot_key, target_kind)
    WHERE origin != 'source_assertion';
CREATE INDEX relationship_reviews_assertion_index
    ON relationship_reviews (assertion_key, review_id);
CREATE INDEX rom_file_sha1_index ON rom_files (sha1);
CREATE INDEX rom_file_xxhash3_index ON rom_files (xxhash3);
CREATE INDEX rom_files_scan_root_index ON rom_files (scan_root);
CREATE INDEX snapshots_catalog_key_index ON catalog_snapshots (catalog_key);
CREATE INDEX snapshots_document_key_index ON catalog_snapshots (document_key);
CREATE INDEX snapshots_interpretation_key_index ON catalog_snapshots (interpretation_key);
CREATE INDEX snapshots_parent_key_index ON catalog_snapshots (parent_snapshot_key);
CREATE INDEX software_areas_part_order ON software_areas (part_id, area_order);
CREATE INDEX software_disk_entries_area_order ON software_disk_entries (area_id, component_order);
CREATE INDEX software_file_uses_declaration ON software_file_uses (declaration_occurrence_id);
CREATE INDEX software_parts_item_order ON software_parts (record_id, part_order);
CREATE INDEX software_rom_entries_area_order ON software_rom_entries (area_id, component_order);
CREATE TABLE logiqx_set_links (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof','romof','sampleof')),
    target_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, link_kind)
) WITHOUT ROWID;
CREATE TABLE logiqx_device_references (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    reference_order INTEGER NOT NULL CHECK (reference_order >= 0),
    target_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, reference_order)
) WITHOUT ROWID;
CREATE TABLE clrmamepro_set_links (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    link_kind TEXT NOT NULL CHECK (link_kind = 'cloneof'),
    target_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, link_kind)
) WITHOUT ROWID;
-- Native payloads are format-qualified, immutable source facts.
CREATE TRIGGER logiqx_games_native_owner_insert BEFORE INSERT ON logiqx_games
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER logiqx_games_native_immutable_update BEFORE UPDATE ON logiqx_games
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER logiqx_games_native_immutable_delete BEFORE DELETE ON logiqx_games
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER logiqx_set_links_native_owner_insert BEFORE INSERT ON logiqx_set_links
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER logiqx_set_links_native_immutable_update BEFORE UPDATE ON logiqx_set_links
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER logiqx_set_links_native_immutable_delete BEFORE DELETE ON logiqx_set_links
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER logiqx_device_references_native_owner_insert BEFORE INSERT ON logiqx_device_references
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER logiqx_device_references_native_immutable_update BEFORE UPDATE ON logiqx_device_references
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER logiqx_device_references_native_immutable_delete BEFORE DELETE ON logiqx_device_references
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER mame_machines_native_owner_insert BEFORE INSERT ON mame_machines
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'mame_machine')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER mame_machines_native_immutable_update BEFORE UPDATE ON mame_machines
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER mame_machines_native_immutable_delete BEFORE DELETE ON mame_machines
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER mame_bios_sets_native_owner_insert BEFORE INSERT ON mame_bios_sets
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'mame_machine')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER mame_bios_sets_native_immutable_update BEFORE UPDATE ON mame_bios_sets
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER mame_bios_sets_native_immutable_delete BEFORE DELETE ON mame_bios_sets
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER no_intro_pc_games_native_owner_insert BEFORE INSERT ON no_intro_pc_games
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'no_intro_pc_game')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER no_intro_pc_games_native_immutable_update BEFORE UPDATE ON no_intro_pc_games
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER no_intro_pc_games_native_immutable_delete BEFORE DELETE ON no_intro_pc_games
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER cmp_set_facts_native_owner_insert BEFORE INSERT ON cmp_set_facts
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.record_id AND source_element_kind = 'cmp_set')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER cmp_set_facts_native_immutable_update BEFORE UPDATE ON cmp_set_facts
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER cmp_set_facts_native_immutable_delete BEFORE DELETE ON cmp_set_facts
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER clrmamepro_set_links_native_owner_insert BEFORE INSERT ON clrmamepro_set_links
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id AND source_element_kind = 'cmp_set')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER clrmamepro_set_links_native_immutable_update BEFORE UPDATE ON clrmamepro_set_links
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER clrmamepro_set_links_native_immutable_delete BEFORE DELETE ON clrmamepro_set_links
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER software_items_native_owner_insert BEFORE INSERT ON software_items
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.record_id AND source_element_kind = 'software_item')
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
            JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.record_id)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished set of the matching format'); END;
CREATE TRIGGER software_items_native_immutable_update BEFORE UPDATE ON software_items
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER software_items_native_immutable_delete BEFORE DELETE ON software_items
BEGIN SELECT RAISE(ABORT, 'native set details are immutable'); END;
CREATE TRIGGER catalog_groups_require_unpublished_snapshot BEFORE INSERT ON catalog_set_groups
WHEN EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'published catalog groups are immutable'); END;
CREATE TRIGGER catalog_groups_reject_replacement BEFORE INSERT ON catalog_set_groups
WHEN EXISTS (SELECT 1 FROM catalog_set_groups WHERE set_group_id = NEW.set_group_id)
 OR EXISTS (SELECT 1 FROM catalog_set_groups
            WHERE snapshot_key = NEW.snapshot_key AND kind = NEW.kind AND list_order = NEW.list_order)
BEGIN SELECT RAISE(ABORT, 'catalog groups are immutable'); END;
CREATE TRIGGER catalog_groups_require_matching_format BEFORE INSERT ON catalog_set_groups
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND (
        (NEW.kind = 'software_list' AND format = 'mame-softwarelist-xml') OR
        (NEW.kind = 'root' AND format IN ('mame-listxml', 'logiqx', 'clrmamepro-dat', 'no-intro-pc-xml',
            'no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
            'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible',
            'no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible'))
    )
)
BEGIN SELECT RAISE(ABORT, 'catalog group does not match its parser format'); END;
CREATE TRIGGER catalog_sets_require_unpublished_snapshot BEFORE INSERT ON catalog_sets
WHEN EXISTS (SELECT 1 FROM catalog_set_groups JOIN snapshot_publications USING (snapshot_key)
             WHERE set_group_id = NEW.set_group_id)
BEGIN SELECT RAISE(ABORT, 'published catalog sets are immutable'); END;
CREATE TRIGGER catalog_sets_reject_replacement BEFORE INSERT ON catalog_sets
WHEN EXISTS (SELECT 1 FROM catalog_sets WHERE set_id = NEW.set_id)
 OR EXISTS (SELECT 1 FROM catalog_sets
            WHERE set_group_id = NEW.set_group_id AND list_order = NEW.list_order)
BEGIN SELECT RAISE(ABORT, 'catalog sets are immutable'); END;
CREATE TRIGGER software_lists_native_owner_insert BEFORE INSERT ON software_lists
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_set_groups
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_group_id = NEW.namespace_id AND kind = 'software_list' AND format = 'mame-softwarelist-xml'
)
 OR EXISTS (SELECT 1 FROM catalog_set_groups JOIN snapshot_publications USING (snapshot_key)
            WHERE set_group_id = NEW.namespace_id)
BEGIN SELECT RAISE(ABORT, 'software lists require an unpublished software-list group'); END;
CREATE TRIGGER software_lists_native_immutable_update BEFORE UPDATE ON software_lists
BEGIN SELECT RAISE(ABORT, 'native software lists are immutable'); END;
CREATE TRIGGER software_lists_native_immutable_delete BEFORE DELETE ON software_lists
BEGIN SELECT RAISE(ABORT, 'native software lists are immutable'); END;
CREATE VIEW record_namespaces AS
SELECT groups.set_group_id AS namespace_id, groups.snapshot_key, groups.kind,
       groups.list_order AS source_order, lists.name AS source_name
FROM catalog_set_groups AS groups LEFT JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id;
CREATE VIEW records AS
SELECT set_id AS record_id, set_group_id AS namespace_id, source_element_kind AS kind,
       list_order AS source_order, set_name AS source_name, source_line, source_column
FROM catalog_sets;
CREATE VIEW snapshot_sets AS
SELECT sets.set_id, sets.set_group_id, groups.snapshot_key, sets.set_name,
       COALESCE(mame.target_name, logiqx.target_name, cmp.target_name, no_intro.cloneof_text) AS parent_name,
       sets.source_line, sets.source_column
FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING (set_group_id)
LEFT JOIN mame_machine_links AS mame ON mame.set_id = sets.set_id AND mame.link_kind = 'cloneof'
LEFT JOIN logiqx_set_links AS logiqx ON logiqx.set_id = sets.set_id AND logiqx.link_kind = 'cloneof'
LEFT JOIN clrmamepro_set_links AS cmp ON cmp.set_id = sets.set_id AND cmp.link_kind = 'cloneof'
LEFT JOIN no_intro_dat_games AS no_intro ON no_intro.set_id = sets.set_id
WHERE groups.kind = 'root';
CREATE VIEW mame_machine_facts AS
SELECT sets.snapshot_key,sets.set_name,machines.*,
       COALESCE(compatibility.is_consumable,0) AS is_consumable,
       COALESCE(compatibility.is_consumable_specified,0) AS is_consumable_specified
FROM mame_machines AS machines JOIN snapshot_sets AS sets USING(set_id)
LEFT JOIN mame_machine_compatibility AS compatibility USING(set_id);
CREATE VIEW logiqx_set_facts AS
SELECT sets.snapshot_key, sets.set_name, games.*
FROM logiqx_games AS games JOIN snapshot_sets AS sets USING (set_id);
CREATE VIEW no_intro_game_facts AS
SELECT sets.snapshot_key, sets.set_name, games.*
FROM no_intro_pc_games AS games JOIN snapshot_sets AS sets USING (set_id);
CREATE VIEW machine_bios_sets AS
SELECT sets.snapshot_key, sets.set_name, bios.*
FROM mame_bios_sets AS bios JOIN snapshot_sets AS sets USING (set_id);
CREATE VIEW asset_requirement_content_conflicts AS
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,conflict.candidate_content_uuid,conflict.reason
FROM occurrence_content_conflicts AS conflict JOIN asset_occurrences AS occurrence USING (occurrence_id)
JOIN records ON records.record_id = occurrence.record_id WHERE records.kind <> 'software_item';
CREATE VIEW asset_requirement_digest_assertions AS
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,assertion.digest_id,assertion.scope,assertion.provenance
FROM occurrence_digest_assertions AS assertion JOIN asset_occurrences AS occurrence USING (occurrence_id)
JOIN records ON records.record_id = occurrence.record_id WHERE records.kind <> 'software_item';
CREATE VIEW usable_occurrence_digest_assertions AS
SELECT assertion.occurrence_id, assertion.digest_id, assertion.scope, assertion.provenance,
       digest.algorithm, digest.digest
FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING (digest_id)
WHERE NOT EXISTS (
    SELECT 1 FROM occurrence_digest_assertions AS other
    JOIN digest_values AS other_digest ON other_digest.digest_id = other.digest_id
    WHERE other.occurrence_id = assertion.occurrence_id AND other.scope = assertion.scope
      AND other.provenance = assertion.provenance AND other_digest.algorithm = digest.algorithm
      AND other_digest.digest <> digest.digest
);
CREATE VIEW asset_requirement_usable_digests AS
SELECT occurrence.record_id AS set_id, occurrence.occurrence_order AS component_order,
       assertion.digest_id, assertion.scope, assertion.provenance
FROM usable_occurrence_digest_assertions AS assertion
JOIN asset_occurrences AS occurrence USING (occurrence_id)
JOIN records ON records.record_id = occurrence.record_id WHERE records.kind <> 'software_item';
CREATE VIEW asset_requirement_rows AS
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'rom' AS role,
       payload.size,payload.evidence_scope,payload.evidence_provenance,merge_declaration.merge_name,payload.dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,payload.region,payload.bios,payload.offset_text AS offset,payload.optional,compatibility.sound_only,compatibility.dispose,compatibility.load_flag,compatibility.value,compatibility.inverted,compatibility.ovha,compatibility.no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM mame_rom_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
LEFT JOIN mame_rom_merges AS merge_declaration USING(occurrence_id)
LEFT JOIN mame_rom_compatibility AS compatibility USING(occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'disk' AS role,
       NULL AS size,payload.evidence_scope,payload.evidence_provenance,merge_declaration.merge_name,payload.dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,payload.region,NULL AS bios,NULL AS offset,payload.optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,payload.disk_index,payload.writable,compatibility.writeable,
       occurrence.content_uuid
FROM mame_disk_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
LEFT JOIN mame_disk_merges AS merge_declaration USING(occurrence_id)
LEFT JOIN mame_disk_compatibility AS compatibility USING(occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'rom' AS role,
       payload.size,payload.evidence_scope,payload.evidence_provenance,payload.merge_name,payload.dump_status,payload.source_line,payload.source_column,payload.serial,payload.date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM logiqx_rom_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'disk' AS role,
       NULL AS size,payload.evidence_scope,payload.evidence_provenance,payload.merge_name,payload.dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM logiqx_disk_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'other' AS role,
       NULL AS size,'unknown' AS evidence_scope,'source_declared' AS evidence_provenance,NULL AS merge_name,NULL AS dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM logiqx_sample_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'other' AS role,
       NULL AS size,'unknown' AS evidence_scope,'source_declared' AS evidence_provenance,NULL AS merge_name,NULL AS dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM mame_samples AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'rom' AS role,
       payload.size,payload.evidence_scope,payload.evidence_provenance,payload.merge_name,payload.dump_status,payload.source_line,payload.source_column,payload.serial,payload.date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM cmp_rom_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'rom' AS role,
       payload.size,payload.evidence_scope,payload.evidence_provenance,payload.merge_name,payload.dump_status,payload.source_line,payload.source_column,NULL AS serial,NULL AS date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM no_intro_pc_file_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id)
UNION ALL
SELECT occurrence.record_id AS set_id,occurrence.occurrence_order AS component_order,
       payload.name AS asset_name,'rom' AS role,
       payload.size,payload.evidence_scope,payload.evidence_provenance,NULL AS merge_name,payload.status_text AS dump_status,payload.source_line,payload.source_column,payload.serial_text AS serial,payload.date_text AS date,NULL AS region,NULL AS bios,NULL AS offset,NULL AS optional,NULL AS sound_only,NULL AS dispose,NULL AS load_flag,NULL AS value,NULL AS inverted,NULL AS ovha,NULL AS no_thread,NULL AS disk_index,NULL AS writable,NULL AS writeable,
       occurrence.content_uuid
FROM no_intro_dat_rom_claims AS payload JOIN asset_occurrences AS occurrence USING (occurrence_id);
CREATE VIEW asset_requirements AS
SELECT sets.snapshot_key,sets.set_name,rows.component_order,rows.asset_name,rows.role,rows.size,
       (SELECT assertion.digest FROM usable_occurrence_digest_assertions AS assertion
        WHERE assertion.occurrence_id = occurrence.occurrence_id AND assertion.algorithm = 'crc32' AND assertion.scope = rows.evidence_scope AND assertion.provenance = rows.evidence_provenance) AS crc,
       (SELECT assertion.digest FROM usable_occurrence_digest_assertions AS assertion
        WHERE assertion.occurrence_id = occurrence.occurrence_id AND assertion.algorithm = 'md5' AND assertion.scope = rows.evidence_scope AND assertion.provenance = rows.evidence_provenance) AS md5,
       (SELECT assertion.digest FROM usable_occurrence_digest_assertions AS assertion
        WHERE assertion.occurrence_id = occurrence.occurrence_id AND assertion.algorithm = 'sha1' AND assertion.scope = rows.evidence_scope AND assertion.provenance = rows.evidence_provenance) AS sha1,
       rows.evidence_scope,rows.evidence_provenance,rows.merge_name,rows.dump_status,rows.serial,rows.date,rows.source_line,rows.source_column,rows.content_uuid
FROM asset_requirement_rows AS rows JOIN snapshot_sets AS sets USING (set_id)
JOIN asset_occurrences AS occurrence ON occurrence.record_id = rows.set_id AND occurrence.occurrence_order = rows.component_order;
CREATE VIEW mame_asset_facts AS
SELECT sets.set_id,sets.snapshot_key,sets.set_name,rows.component_order,rows.region,rows.bios,rows.offset,rows.optional,rows.sound_only,rows.dispose,rows.load_flag,rows.value,rows.inverted,rows.ovha,rows.no_thread,rows.disk_index,rows.writable,rows.writeable,
       native.source_order,native.status_specified,native.optional_specified,native.writable_specified
FROM asset_requirement_rows AS rows JOIN snapshot_sets AS sets USING (set_id) JOIN records ON records.record_id = rows.set_id
JOIN asset_occurrences AS occurrence ON occurrence.record_id=rows.set_id AND occurrence.occurrence_order=rows.component_order
JOIN (
    SELECT occurrence_id,source_order,status_specified,optional_specified,0 AS writable_specified FROM mame_rom_claims
    UNION ALL SELECT occurrence_id,source_order,status_specified,optional_specified,writable_specified FROM mame_disk_claims
) AS native USING(occurrence_id)
WHERE records.kind = 'mame_machine';
CREATE VIEW mame_machine_conditions AS
SELECT set_id, 'element' AS owner_kind, element_order AS owner_element_order,
       NULL AS owner_switch_order, NULL AS owner_child_order, condition_order,
       tag, mask, relation, value, source_line, source_column
FROM mame_machine_adjuster_conditions
UNION ALL
SELECT set_id, 'switch', NULL, switch_order, NULL, condition_order,
       tag, mask, relation, value, source_line, source_column
FROM machine_switch_conditions
UNION ALL
SELECT set_id, 'switch_value', NULL, switch_order, value_order, condition_order,
       tag, mask, relation, value, source_line, source_column
FROM machine_switch_value_conditions;
CREATE VIEW stored_relationship_assertion_explanations AS
SELECT assertion_key, relation_type, origin, source_snapshot_key, source_field,
       source_line, source_column, generic_subject_snapshot_key, subject_kind, subject_set_id,
       generic_subject_a, generic_subject_b, generic_subject_c, source_subject_a,
       source_subject_b, CASE WHEN subject_kind = 'no_intro_archive' THEN subject_archive_id ELSE source_subject_c END AS source_subject_c, subject_snapshot_key,
       generic_target_snapshot_key, target_kind, target_set_id, generic_target_a, generic_target_b,
       generic_target_c, CASE WHEN target_kind = 'no_intro_archive_reference' THEN
           CASE source_field WHEN 'archive_clone' THEN (SELECT declared_target_number FROM no_intro_archive_clone_links WHERE relationship_id = assertion_key)
                             WHEN 'archive_mergeof' THEN (SELECT declared_mergeof FROM no_intro_archive_merge_links WHERE relationship_id = assertion_key) END
           ELSE source_target_a END AS source_target_a, source_target_b, source_target_c,
       target_snapshot_key, rule_version
FROM relationship_assertions;
CREATE VIEW relationship_assertion_explanations AS
SELECT * FROM stored_relationship_assertion_explanations
UNION ALL
SELECT * FROM mame_source_relationships
UNION ALL
SELECT assertion_key, relation_type, origin, source_snapshot_key, source_field,
       source_line, source_column, generic_subject_snapshot_key, subject_kind, subject_set_id,
       generic_subject_a, generic_subject_b, generic_subject_c, source_subject_a,
       source_subject_b, source_subject_c, subject_snapshot_key,
       generic_target_snapshot_key, target_kind, target_set_id, generic_target_a,
       generic_target_b, generic_target_c, source_target_a, source_target_b,
       source_target_c, target_snapshot_key, rule_version
FROM no_intro_dat_cloneof_assertions;
CREATE VIEW no_intro_dat_cloneof_assertions AS
SELECT 'no-intro-dat-cloneof:' || game.set_id AS assertion_key,
       'source_parent_clone' AS relation_type, 'source_assertion' AS origin,
       groups.snapshot_key AS source_snapshot_key, 'cloneof' AS source_field,
       position.source_line AS source_line, position.source_column AS source_column,
       NULL AS generic_subject_snapshot_key, 'catalog_set' AS subject_kind,
       game.set_id AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, sets.set_name AS source_subject_a,
       NULL AS source_subject_b, NULL AS source_subject_c,
       groups.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key, 'catalog_set' AS target_kind,
       NULL AS target_set_id, NULL AS generic_target_a, NULL AS generic_target_b,
       NULL AS generic_target_c, game.cloneof_text AS source_target_a,
       NULL AS source_target_b, NULL AS source_target_c,
       groups.snapshot_key AS target_snapshot_key, NULL AS rule_version,
       game.set_id AS native_set_id, position.field_kind AS native_position_field_kind
FROM no_intro_dat_games AS game
JOIN catalog_sets AS sets USING (set_id)
JOIN catalog_set_groups AS groups USING (set_group_id)
JOIN no_intro_dat_game_field_positions AS position
  ON position.set_id = game.set_id AND position.field_kind = 2
WHERE game.cloneof_text IS NOT NULL;
CREATE VIEW software_components AS
SELECT occurrences.occurrence_id, namespaces.snapshot_key, namespaces.source_name AS list_name,
       records.source_name AS item_name, parts.part_name, areas.area_order,
       areas.area_kind, COALESCE(data_area.area_name, disk_area.area_name) AS area_name,
       COALESCE(rom.component_order, disk.component_order) AS component_order,
       CASE WHEN occurrences.claim_kind = 'software_disk_entry' THEN 'disk' ELSE 'rom' END AS component_kind,
       COALESCE(rom.name, disk.name) AS component_name,
       rom.size,
       (SELECT digest.digest FROM occurrence_digest_assertions AS assertions
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertions.occurrence_id = occurrences.occurrence_id
          AND digest.algorithm = 'crc32'
          AND assertions.scope = COALESCE(rom.evidence_scope, disk.evidence_scope) LIMIT 1) AS crc,
       (SELECT digest.digest FROM occurrence_digest_assertions AS assertions
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertions.occurrence_id = occurrences.occurrence_id
          AND digest.algorithm = 'sha1'
          AND assertions.scope = COALESCE(rom.evidence_scope, disk.evidence_scope) LIMIT 1) AS sha1,
       rom.offset,
       rom.value,
       COALESCE(rom.dump_status, disk.dump_status) AS dump_status,
       disk.writeable,
       rom.load_instruction,
       COALESCE(rom.source_line, disk.source_line) AS source_line,
       COALESCE(rom.source_column, disk.source_column) AS source_column,
       COALESCE(rom.evidence_scope, disk.evidence_scope) AS evidence_scope,
       occurrences.content_uuid
FROM asset_occurrences AS occurrences
JOIN records ON records.record_id = occurrences.record_id
JOIN record_namespaces AS namespaces USING (namespace_id)
LEFT JOIN software_rom_entries AS rom USING (occurrence_id, record_id)
LEFT JOIN software_disk_entries AS disk USING (occurrence_id, record_id)
LEFT JOIN software_areas AS areas ON areas.area_id = COALESCE(rom.area_id, disk.area_id)
LEFT JOIN software_data_areas AS data_area ON data_area.area_id = areas.area_id AND areas.area_kind = 'data'
LEFT JOIN software_disk_areas AS disk_area ON disk_area.area_id = areas.area_id AND areas.area_kind = 'disk'
LEFT JOIN software_parts AS parts USING (part_id)
WHERE occurrences.claim_kind IN ('software_rom_entry', 'software_rom_operation', 'software_disk_entry');
CREATE TRIGGER acquisition_attempt_transport_headers_are_immutable_delete
BEFORE DELETE ON acquisition_attempt_transport_headers
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempt transport headers are immutable');
END;
CREATE TRIGGER acquisition_attempt_transport_headers_are_immutable_update
BEFORE UPDATE ON acquisition_attempt_transport_headers
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempt transport headers are immutable');
END;
CREATE TRIGGER acquisition_attempts_are_immutable_delete
BEFORE DELETE ON acquisition_attempts
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;
CREATE TRIGGER acquisition_attempts_are_immutable_insert
BEFORE INSERT ON acquisition_attempts
WHEN EXISTS (SELECT 1 FROM acquisition_attempts WHERE attempt_key = NEW.attempt_key)
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;
CREATE TRIGGER acquisition_attempts_are_immutable_update
BEFORE UPDATE ON acquisition_attempts
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;
CREATE TRIGGER acquisition_transport_headers_are_immutable_delete
BEFORE DELETE ON acquisition_transport_headers
BEGIN
    SELECT RAISE(ABORT, 'acquisition transport headers are immutable');
END;
CREATE TRIGGER acquisition_transport_headers_are_immutable_update
BEFORE UPDATE ON acquisition_transport_headers
BEGIN
    SELECT RAISE(ABORT, 'acquisition transport headers are immutable');
END;
CREATE TRIGGER acquisitions_are_immutable_delete
BEFORE DELETE ON acquisitions
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;
CREATE TRIGGER acquisitions_are_immutable_insert
BEFORE INSERT ON acquisitions
WHEN EXISTS (SELECT 1 FROM acquisitions WHERE acquisition_key = NEW.acquisition_key)
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;
CREATE TRIGGER acquisitions_are_immutable_update
BEFORE UPDATE ON acquisitions
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;
CREATE TRIGGER asset_occurrences_are_immutable_delete BEFORE DELETE ON asset_occurrences
BEGIN SELECT RAISE(ABORT, 'asset occurrences are immutable'); END;
CREATE TRIGGER asset_occurrences_are_immutable_update BEFORE UPDATE ON asset_occurrences
BEGIN SELECT RAISE(ABORT, 'asset occurrences are immutable'); END;
CREATE TRIGGER asset_occurrences_reject_replacement BEFORE INSERT ON asset_occurrences
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences
    WHERE occurrence_id=NEW.occurrence_id
       OR (record_id=NEW.record_id AND occurrence_order=NEW.occurrence_order)
)
BEGIN SELECT RAISE(ABORT, 'asset occurrences are immutable'); END;
CREATE TRIGGER asset_requirement_digest_assertions_insert INSTEAD OF INSERT ON asset_requirement_digest_assertions
BEGIN
    INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
    SELECT occurrence_id,NEW.digest_id,NEW.scope,NEW.provenance FROM asset_occurrences
    WHERE record_id = NEW.set_id AND occurrence_order = NEW.component_order;
    SELECT CASE WHEN changes() = 0 THEN RAISE(ABORT,'digest assertion requires native occurrence') END;
END;
CREATE TRIGGER catalog_snapshots_are_immutable_delete
BEFORE DELETE ON catalog_snapshots
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;
CREATE TRIGGER catalog_snapshots_are_immutable_insert
BEFORE INSERT ON catalog_snapshots
WHEN EXISTS (SELECT 1 FROM catalog_snapshots WHERE snapshot_key = NEW.snapshot_key)
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;
CREATE TRIGGER catalog_snapshots_are_immutable_update
BEFORE UPDATE ON catalog_snapshots
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;
CREATE TRIGGER cmp_rom_claims_immutable_delete BEFORE DELETE ON cmp_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER cmp_rom_claims_immutable_update BEFORE UPDATE ON cmp_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER documents_are_immutable_delete
BEFORE DELETE ON documents
BEGIN
    SELECT RAISE(ABORT, 'source documents are immutable');
END;
CREATE TRIGGER documents_are_immutable_insert
BEFORE INSERT ON documents
WHEN EXISTS (
    SELECT 1 FROM documents
    WHERE document_key = NEW.document_key
       OR (NEW.sha256 IS NOT NULL AND sha256 = NEW.sha256)
)
BEGIN
    SELECT RAISE(ABORT, 'source documents are immutable');
END;
CREATE TRIGGER documents_are_immutable_update
BEFORE UPDATE ON documents
WHEN OLD.retention_status = 'retained'
BEGIN
    SELECT RAISE(ABORT, 'retained source documents are immutable');
END;
CREATE TRIGGER logiqx_disk_claims_immutable_delete BEFORE DELETE ON logiqx_disk_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER logiqx_disk_claims_immutable_update BEFORE UPDATE ON logiqx_disk_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER logiqx_rom_claims_immutable_delete BEFORE DELETE ON logiqx_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER logiqx_rom_claims_immutable_update BEFORE UPDATE ON logiqx_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER machine_bios_sets_are_immutable_delete
BEFORE DELETE ON mame_bios_sets
BEGIN
    SELECT RAISE(ABORT, 'machine BIOS sets are immutable');
END;
CREATE TRIGGER machine_bios_sets_are_immutable_update
BEFORE UPDATE ON mame_bios_sets
BEGIN
    SELECT RAISE(ABORT, 'machine BIOS sets are immutable');
END;
CREATE TRIGGER machine_switch_locations_are_immutable_delete
BEFORE DELETE ON machine_switch_locations
BEGIN
    SELECT RAISE(ABORT, 'machine switch locations are immutable');
END;
CREATE TRIGGER machine_switch_locations_are_immutable_update
BEFORE UPDATE ON machine_switch_locations
BEGIN
    SELECT RAISE(ABORT, 'machine switch locations are immutable');
END;
CREATE TRIGGER machine_switch_values_are_immutable_delete
BEFORE DELETE ON machine_switch_values
BEGIN
    SELECT RAISE(ABORT, 'machine switch values are immutable');
END;
CREATE TRIGGER machine_switch_values_are_immutable_update
BEFORE UPDATE ON machine_switch_values
BEGIN
    SELECT RAISE(ABORT, 'machine switch values are immutable');
END;
CREATE TRIGGER machine_switches_are_immutable_delete
BEFORE DELETE ON machine_switches
BEGIN
    SELECT RAISE(ABORT, 'machine switches are immutable');
END;
CREATE TRIGGER machine_switches_are_immutable_update
BEFORE UPDATE ON machine_switches
BEGIN
    SELECT RAISE(ABORT, 'machine switches are immutable');
END;
CREATE TRIGGER mame_disk_claims_immutable_delete BEFORE DELETE ON mame_disk_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER mame_disk_claims_immutable_update BEFORE UPDATE ON mame_disk_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER mame_document_facts_are_immutable_delete
BEFORE DELETE ON mame_document_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME document facts are immutable');
END;
CREATE TRIGGER mame_document_facts_are_immutable_update
BEFORE UPDATE ON mame_document_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME document facts are immutable');
END;
CREATE TRIGGER mame_machine_facts_are_immutable_delete
BEFORE DELETE ON mame_machines
BEGIN
    SELECT RAISE(ABORT, 'MAME machine facts are immutable');
END;
CREATE TRIGGER mame_machine_facts_are_immutable_update
BEFORE UPDATE ON mame_machines
BEGIN
    SELECT RAISE(ABORT, 'MAME machine facts are immutable');
END;
CREATE TRIGGER mame_rom_claims_immutable_delete BEFORE DELETE ON mame_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER mame_rom_claims_immutable_update BEFORE UPDATE ON mame_rom_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER no_intro_pc_file_claims_immutable_delete BEFORE DELETE ON no_intro_pc_file_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER no_intro_pc_file_claims_immutable_update BEFORE UPDATE ON no_intro_pc_file_claims
BEGIN SELECT RAISE(ABORT,'native asset claims are immutable'); END;
CREATE TRIGGER occurrences_require_native_record
BEFORE INSERT ON asset_occurrences
WHEN NOT EXISTS (
    SELECT 1 FROM records WHERE record_id = NEW.record_id AND (
        (kind = 'mame_machine' AND NEW.claim_kind IN ('mame_rom', 'mame_disk', 'mame_sample'))
        OR (kind = 'software_item' AND NEW.claim_kind IN ('software_rom_entry', 'software_rom_operation', 'software_disk_entry'))
        OR (kind = 'logiqx_game' AND NEW.claim_kind IN ('logiqx_rom', 'logiqx_disk', 'logiqx_sample'))
        OR (kind = 'cmp_set' AND NEW.claim_kind IN ('cmp_rom', 'cmp_sample'))
        OR (kind = 'no_intro_pc_game' AND NEW.claim_kind = 'no_intro_pc_file')
        OR (kind = 'no_intro_dat_game' AND NEW.claim_kind = 'no_intro_dat_rom')
        OR (kind = 'no_intro_database_game' AND NEW.claim_kind IN ('no_intro_database_source_file', 'no_intro_database_release_file'))
    )
)
BEGIN
    SELECT RAISE(ABORT, 'occurrence kind does not match its native record');
END;
CREATE TRIGGER occurrences_require_unpublished_snapshot
BEFORE INSERT ON asset_occurrences
WHEN EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE set_id = NEW.record_id
)
BEGIN
    SELECT RAISE(ABORT, 'published catalog occurrences are immutable');
END;
CREATE TRIGGER parser_interpretations_are_immutable_insert
BEFORE INSERT ON parser_interpretations
WHEN EXISTS (
    SELECT 1 FROM parser_interpretations AS prior
    WHERE prior.interpretation_key=NEW.interpretation_key AND (
        prior.format IS NOT NEW.format OR prior.parser_name IS NOT NEW.parser_name
        OR prior.parser_version IS NOT NEW.parser_version OR prior.rules_version IS NOT NEW.rules_version
    )
) AND (
    EXISTS (SELECT 1 FROM catalog_snapshots WHERE interpretation_key=NEW.interpretation_key)
    OR EXISTS (SELECT 1 FROM import_runs WHERE interpretation_key=NEW.interpretation_key)
)
BEGIN SELECT RAISE(ABORT,'referenced parser interpretations are immutable'); END;
CREATE TRIGGER parser_interpretations_are_immutable_delete
BEFORE DELETE ON parser_interpretations
WHEN EXISTS (SELECT 1 FROM catalog_snapshots WHERE interpretation_key=OLD.interpretation_key)
    OR EXISTS (SELECT 1 FROM import_runs WHERE interpretation_key=OLD.interpretation_key)
BEGIN SELECT RAISE(ABORT,'referenced parser interpretations are immutable'); END;
CREATE TRIGGER parser_interpretations_are_immutable_update
BEFORE UPDATE ON parser_interpretations
WHEN (
    OLD.interpretation_key IS NOT NEW.interpretation_key
    OR OLD.format IS NOT NEW.format
    OR OLD.parser_name IS NOT NEW.parser_name
    OR OLD.parser_version IS NOT NEW.parser_version
    OR OLD.rules_version IS NOT NEW.rules_version
) AND (
    EXISTS (
        SELECT 1 FROM catalog_snapshots
        WHERE interpretation_key = OLD.interpretation_key
    )
    OR EXISTS (
        SELECT 1 FROM import_runs
        WHERE interpretation_key = OLD.interpretation_key
    )
)
BEGIN
    SELECT RAISE(ABORT, 'referenced parser interpretations are immutable');
END;
CREATE TRIGGER record_namespaces_are_immutable_delete BEFORE DELETE ON catalog_set_groups
BEGIN SELECT RAISE(ABORT, 'record namespaces are immutable'); END;
CREATE TRIGGER record_namespaces_are_immutable_update BEFORE UPDATE ON catalog_set_groups
BEGIN SELECT RAISE(ABORT, 'record namespaces are immutable'); END;
CREATE TRIGGER records_are_immutable_delete BEFORE DELETE ON catalog_sets
BEGIN SELECT RAISE(ABORT, 'native records are immutable'); END;
CREATE TRIGGER records_are_immutable_update BEFORE UPDATE ON catalog_sets
BEGIN SELECT RAISE(ABORT, 'native records are immutable'); END;
CREATE TRIGGER catalog_sets_require_matching_group
BEFORE INSERT ON catalog_sets
WHEN (NEW.source_element_kind = 'software_item') IS NOT
     (SELECT kind = 'software_list' FROM catalog_set_groups WHERE set_group_id = NEW.set_group_id)
BEGIN
    SELECT RAISE(ABORT, 'set kind does not match its catalog group');
END;
CREATE TRIGGER catalog_sets_require_matching_format
BEFORE INSERT ON catalog_sets
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_set_groups
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_group_id = NEW.set_group_id AND (
        (format = 'mame-listxml' AND NEW.source_element_kind = 'mame_machine') OR
        (format = 'logiqx' AND NEW.source_element_kind = 'logiqx_game') OR
        (format = 'clrmamepro-dat' AND NEW.source_element_kind = 'cmp_set') OR
        (format = 'no-intro-pc-xml' AND NEW.source_element_kind = 'no_intro_pc_game') OR
        (format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                   'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
            AND NEW.source_element_kind = 'no_intro_dat_game') OR
        (format = 'mame-softwarelist-xml' AND NEW.source_element_kind = 'software_item')
        OR (format IN ('no-intro-database-xml-compatible', 'no-intro-database-xml-nul-compatible')
            AND NEW.source_element_kind = 'no_intro_database_game')
    )
)
BEGIN SELECT RAISE(ABORT, 'set kind does not match its parser format'); END;
CREATE TRIGGER relationship_rationales_require_owner BEFORE INSERT ON relationship_rationales
WHEN NOT EXISTS (SELECT 1 FROM relationship_assertions WHERE assertion_key=NEW.assertion_key AND origin<>'source_assertion')
 OR EXISTS (SELECT 1 FROM relationship_comparisons WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_rationales WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=NEW.assertion_key)
BEGIN SELECT RAISE(ABORT,'rationale requires an unsealed non-source relationship'); END;
CREATE TRIGGER relationship_comparisons_require_owner BEFORE INSERT ON relationship_comparisons
WHEN NOT EXISTS (SELECT 1 FROM relationship_assertions WHERE assertion_key=NEW.assertion_key AND origin='derived_candidate')
 OR EXISTS (SELECT 1 FROM relationship_rationales WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_comparisons WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=NEW.assertion_key)
BEGIN SELECT RAISE(ABORT,'comparison requires an unsealed derived relationship'); END;
CREATE TRIGGER relationship_comparison_fields_require_owner BEFORE INSERT ON relationship_comparison_fields
WHEN NOT EXISTS (SELECT 1 FROM relationship_comparisons WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_comparison_fields WHERE assertion_key=NEW.assertion_key
            AND (field=NEW.field OR (disposition=NEW.disposition AND position=NEW.position)))
BEGIN SELECT RAISE(ABORT,'assessment requires an unsealed comparison'); END;
CREATE TRIGGER relationship_evidence_publications_validate BEFORE INSERT ON relationship_evidence_publications
WHEN EXISTS (SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=NEW.assertion_key)
 OR NOT EXISTS (SELECT 1 FROM relationship_assertions a WHERE a.assertion_key=NEW.assertion_key AND (
     (NEW.evidence_kind='rationale' AND a.origin<>'source_assertion'
      AND EXISTS (SELECT 1 FROM relationship_rationales WHERE assertion_key=a.assertion_key)
      AND NOT EXISTS (SELECT 1 FROM relationship_comparisons WHERE assertion_key=a.assertion_key))
     OR (NEW.evidence_kind='catalog_comparison' AND a.origin='derived_candidate'
      AND EXISTS (SELECT 1 FROM relationship_comparisons WHERE assertion_key=a.assertion_key)
      AND NOT EXISTS (SELECT 1 FROM relationship_rationales WHERE assertion_key=a.assertion_key))
 ))
 OR EXISTS (SELECT 1 FROM relationship_comparison_fields WHERE assertion_key=NEW.assertion_key
     GROUP BY disposition HAVING MIN(position)<>0 OR MAX(position)<>COUNT(*)-1)
 OR EXISTS (SELECT COUNT(*) FROM relationship_assertion_support WHERE assertion_key=NEW.assertion_key
     HAVING COUNT(*)>0 AND (MIN(position)<>0 OR MAX(position)<>COUNT(*)-1))
BEGIN SELECT RAISE(ABORT,'relationship evidence is incomplete or has a mismatching owner'); END;
CREATE TRIGGER relationship_rationales_immutable_update BEFORE UPDATE ON relationship_rationales
BEGIN SELECT RAISE(ABORT,'relationship rationale is immutable'); END;
CREATE TRIGGER relationship_rationales_immutable_delete BEFORE DELETE ON relationship_rationales
BEGIN SELECT RAISE(ABORT,'relationship rationale is immutable'); END;
CREATE TRIGGER relationship_comparisons_immutable_update BEFORE UPDATE ON relationship_comparisons
BEGIN SELECT RAISE(ABORT,'relationship comparison is immutable'); END;
CREATE TRIGGER relationship_comparisons_immutable_delete BEFORE DELETE ON relationship_comparisons
BEGIN SELECT RAISE(ABORT,'relationship comparison is immutable'); END;
CREATE TRIGGER relationship_comparison_fields_immutable_update BEFORE UPDATE ON relationship_comparison_fields
BEGIN SELECT RAISE(ABORT,'relationship assessment is immutable'); END;
CREATE TRIGGER relationship_comparison_fields_immutable_delete BEFORE DELETE ON relationship_comparison_fields
BEGIN SELECT RAISE(ABORT,'relationship assessment is immutable'); END;
CREATE TRIGGER relationship_evidence_publications_immutable_update BEFORE UPDATE ON relationship_evidence_publications
BEGIN SELECT RAISE(ABORT,'relationship evidence publication is immutable'); END;
CREATE TRIGGER relationship_evidence_publications_immutable_delete BEFORE DELETE ON relationship_evidence_publications
BEGIN SELECT RAISE(ABORT,'relationship evidence publication is immutable'); END;
CREATE TRIGGER relationship_assertion_support_sealed_insert BEFORE INSERT ON relationship_assertion_support
WHEN EXISTS (SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=NEW.assertion_key)
 OR EXISTS (SELECT 1 FROM relationship_assertion_support WHERE assertion_key=NEW.assertion_key AND position=NEW.position)
BEGIN SELECT RAISE(ABORT,'published relationship support is immutable'); END;
CREATE TRIGGER relationship_assertion_support_immutable_delete
BEFORE DELETE ON relationship_assertion_support
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion support is immutable');
END;
CREATE TRIGGER relationship_assertion_support_immutable_update
BEFORE UPDATE ON relationship_assertion_support
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion support is immutable');
END;
CREATE TRIGGER relationship_assertion_support_origin_insert
BEFORE INSERT ON relationship_assertion_support
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions
    WHERE assertion_key = NEW.assertion_key AND origin = 'derived_candidate'
)
BEGIN
    SELECT RAISE(ABORT, 'only derived candidates may have supporting assertions');
END;
CREATE TRIGGER relationship_subject_owner_matches_endpoint BEFORE INSERT ON relationship_assertions
WHEN NEW.subject_set_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_sets AS sets
    JOIN catalog_set_groups AS groups USING (set_group_id)
    LEFT JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
    WHERE sets.set_id = NEW.subject_set_id
      AND groups.snapshot_key = NEW.subject_snapshot_key
      AND (
          (NEW.subject_kind IN ('catalog_set','asset_requirement') AND groups.kind = 'root'
           AND sets.set_name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_subject_a ELSE NEW.generic_subject_a END)
          OR
          (NEW.subject_kind = 'software_item' AND groups.kind = 'software_list'
           AND lists.name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_subject_a ELSE NEW.generic_subject_a END
           AND sets.set_name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_subject_b ELSE NEW.generic_subject_b END)
      )
)
BEGIN SELECT RAISE(ABORT, 'relationship owner does not match its catalog endpoint'); END;
CREATE TRIGGER relationship_target_owner_matches_endpoint BEFORE INSERT ON relationship_assertions
WHEN NEW.target_set_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_sets AS sets
    JOIN catalog_set_groups AS groups USING (set_group_id)
    LEFT JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
    WHERE sets.set_id = NEW.target_set_id
      AND groups.snapshot_key = NEW.target_snapshot_key
      AND (
          (NEW.target_kind IN ('catalog_set','asset_requirement') AND groups.kind = 'root'
           AND sets.set_name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_target_a ELSE NEW.generic_target_a END)
          OR
          (NEW.target_kind = 'software_item' AND groups.kind = 'software_list'
           AND lists.name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_target_a ELSE NEW.generic_target_a END
           AND sets.set_name = CASE WHEN NEW.origin = 'source_assertion' THEN NEW.source_target_b ELSE NEW.generic_target_b END)
      )
)
BEGIN SELECT RAISE(ABORT, 'relationship owner does not match its catalog endpoint'); END;
CREATE TRIGGER relationship_assertions_are_immutable_delete
BEFORE DELETE ON relationship_assertions
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_assertions_are_immutable_insert
BEFORE INSERT ON relationship_assertions
WHEN EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
) OR EXISTS (SELECT 1 FROM catalog_relationships WHERE assertion_key=NEW.assertion_key)
  OR NEW.assertion_key GLOB 'no-intro-dat-cloneof:*'
  OR (NEW.origin='source_assertion' AND NEW.source_field IN ('cloneof','romof','sampleof','device_ref','merge')
      AND EXISTS (SELECT 1 FROM catalog_snapshots AS snapshot
          JOIN parser_interpretations AS interpretation USING(interpretation_key)
          WHERE snapshot.snapshot_key=NEW.source_snapshot_key AND interpretation.format='mame-listxml'))
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_assertions_are_immutable_update
BEFORE UPDATE ON relationship_assertions
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_reviews_are_immutable_delete
BEFORE DELETE ON relationship_reviews
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
CREATE TRIGGER relationship_reviews_are_immutable_insert
BEFORE INSERT ON relationship_reviews
WHEN EXISTS (
    SELECT 1 FROM relationship_reviews
    WHERE review_key = NEW.review_key OR review_id = NEW.review_id
)
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
CREATE TRIGGER relationship_reviews_are_immutable_update
BEFORE UPDATE ON relationship_reviews
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
CREATE TRIGGER retained_documents_require_object_insert
BEFORE INSERT ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.object_key IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'retained document object metadata is incomplete');
END;
CREATE TRIGGER retained_documents_require_object_update
BEFORE UPDATE ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.object_key IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'retained document object metadata is incomplete');
END;
CREATE TRIGGER rom_files_archive_identity_insert
BEFORE INSERT ON rom_files
WHEN (NEW.archive_backend IS NULL) != (NEW.archive_member_index IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'archive backend and member index must be stored together');
END;
CREATE TRIGGER rom_files_archive_identity_update
BEFORE UPDATE OF archive_backend, archive_member_index ON rom_files
WHEN (NEW.archive_backend IS NULL) != (NEW.archive_member_index IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'archive backend and member index must be stored together');
END;
CREATE TRIGGER rom_files_scan_provenance_insert
BEFORE INSERT ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL
        AND NEW.observed_size IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL
        AND NEW.observed_size IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan metadata must be stored together');
END;
CREATE TRIGGER rom_files_scan_provenance_update
BEFORE UPDATE OF scan_root, scan_run, observed_size, source_fingerprint, scan_provenance ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL
        AND NEW.observed_size IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL
        AND NEW.observed_size IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan metadata must be stored together');
END;
CREATE TRIGGER snapshot_publications_are_immutable_delete
BEFORE DELETE ON snapshot_publications
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;
CREATE TRIGGER snapshot_publications_are_immutable_insert
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM snapshot_publications
    WHERE (catalog_key = NEW.catalog_key
       AND document_key = NEW.document_key
       AND interpretation_key = NEW.interpretation_key)
       OR snapshot_key = NEW.snapshot_key
)
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;
CREATE TRIGGER snapshot_publications_are_immutable_update
BEFORE UPDATE ON snapshot_publications
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;
