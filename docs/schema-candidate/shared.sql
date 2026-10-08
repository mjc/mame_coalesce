-- DESIGN ONLY. This fragment is not included by the application's SCHEMA.
-- Catalog facts are relational; source-file bytes remain in external objects.
CREATE TABLE catalog_publishers (
    publisher_id INTEGER PRIMARY KEY NOT NULL CHECK (publisher_id > 0),
    publisher_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    locator TEXT
) STRICT;

CREATE TABLE catalogs (
    catalog_id INTEGER PRIMARY KEY NOT NULL CHECK (catalog_id > 0),
    publisher_id INTEGER NOT NULL REFERENCES catalog_publishers(publisher_id),
    catalog_key TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL
) STRICT;

CREATE TABLE catalog_source_files (
    source_file_id INTEGER PRIMARY KEY NOT NULL CHECK (source_file_id > 0),
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    sha1 BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    object_key TEXT NOT NULL UNIQUE,
    codec TEXT NOT NULL CHECK (codec = 'zstd'),
    UNIQUE (sha256, byte_length)
) STRICT;

CREATE TABLE catalog_fetch_attempts (
    fetch_attempt_id INTEGER PRIMARY KEY NOT NULL CHECK (fetch_attempt_id > 0),
    publisher_id INTEGER NOT NULL REFERENCES catalog_publishers(publisher_id),
    attempt_key TEXT NOT NULL UNIQUE,
    uri TEXT NOT NULL,
    method TEXT NOT NULL,
    requested_at TEXT NOT NULL,
    responded_at TEXT,
    outcome TEXT NOT NULL CHECK (outcome IN ('failed', 'retained', 'unretained')),
    declared_filename TEXT,
    verification_status TEXT NOT NULL CHECK (verification_status IN ('not_requested', 'verified', 'mismatch', 'unavailable'))
) STRICT;

CREATE TABLE catalog_fetch_headers (
    fetch_attempt_id INTEGER NOT NULL REFERENCES catalog_fetch_attempts(fetch_attempt_id),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (fetch_attempt_id, list_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE catalog_fetch_hashes (
    fetch_attempt_id INTEGER NOT NULL REFERENCES catalog_fetch_attempts(fetch_attempt_id),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    expected_bytes BLOB NOT NULL,
    CHECK (length(expected_bytes) = CASE algorithm WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16 WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END),
    PRIMARY KEY (fetch_attempt_id, list_order)
) STRICT, WITHOUT ROWID;

CREATE TABLE catalog_file_receipts (
    file_receipt_id INTEGER PRIMARY KEY NOT NULL CHECK (file_receipt_id > 0),
    receipt_key TEXT NOT NULL UNIQUE,
    fetch_attempt_id INTEGER NOT NULL UNIQUE REFERENCES catalog_fetch_attempts(fetch_attempt_id),
    source_file_id INTEGER NOT NULL REFERENCES catalog_source_files(source_file_id)
) STRICT;

CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY NOT NULL CHECK (reading_rules_id > 0),
    rules_key TEXT NOT NULL UNIQUE,
    format_family TEXT NOT NULL CHECK (format_family IN ('mame', 'software', 'logiqx', 'clrmamepro', 'no_intro_dat', 'no_intro_database', 'no_intro_pc_fixture')),
    dialect TEXT NOT NULL,
    specification_version TEXT NOT NULL,
    parser_version TEXT NOT NULL,
    rules_version TEXT NOT NULL
) STRICT;

CREATE TABLE catalog_xml_repairs (
    reading_rules_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_reading_rules(reading_rules_id),
    replace_nul INTEGER NOT NULL CHECK (replace_nul IN (0, 1))
) STRICT;

-- Verified length of the reproducible byte view used for XML diagnostic ranges.
-- Retained-original length remains single-owned by catalog_source_files.
CREATE TABLE catalog_decoded_xml_views (
    source_file_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_files(source_file_id),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0)
) STRICT;

CREATE TABLE catalog_coverage (
    coverage_id INTEGER PRIMARY KEY NOT NULL CHECK (coverage_id > 0),
    kind TEXT NOT NULL CHECK (kind IN ('unknown', 'complete', 'filtered', 'partial'))
) STRICT;

CREATE TABLE catalog_covered_sets (
    coverage_id INTEGER NOT NULL REFERENCES catalog_coverage(coverage_id),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    set_kind TEXT NOT NULL CHECK (set_kind IN ('root', 'software_item')),
    software_list_name TEXT,
    set_name TEXT NOT NULL,
    coverage TEXT NOT NULL CHECK (coverage IN ('covered', 'unknown')),
    CHECK ((set_kind = 'root' AND software_list_name IS NULL) OR (set_kind = 'software_item' AND software_list_name IS NOT NULL)),
    PRIMARY KEY (coverage_id, list_order)
) STRICT, WITHOUT ROWID;
CREATE UNIQUE INDEX covered_root_names ON catalog_covered_sets(coverage_id, set_name) WHERE set_kind = 'root';
CREATE UNIQUE INDEX covered_software_names ON catalog_covered_sets(coverage_id, software_list_name, set_name) WHERE set_kind = 'software_item';

CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY NOT NULL CHECK (edition_id > 0),
    catalog_id INTEGER NOT NULL REFERENCES catalogs(catalog_id),
    source_file_id INTEGER NOT NULL REFERENCES catalog_source_files(source_file_id),
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id),
    coverage_id INTEGER NOT NULL REFERENCES catalog_coverage(coverage_id),
    file_receipt_id INTEGER REFERENCES catalog_file_receipts(file_receipt_id),
    previous_edition_id INTEGER REFERENCES catalog_editions(edition_id),
    UNIQUE (catalog_id, source_file_id, reading_rules_id, coverage_id),
    UNIQUE (edition_id, catalog_id, source_file_id, reading_rules_id, coverage_id)
) STRICT;

CREATE TABLE published_catalog_editions (
    edition_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_editions(edition_id),
    catalog_id INTEGER NOT NULL,
    source_file_id INTEGER NOT NULL,
    reading_rules_id INTEGER NOT NULL,
    coverage_id INTEGER NOT NULL,
    published_at TEXT NOT NULL,
    FOREIGN KEY (edition_id, catalog_id, source_file_id, reading_rules_id, coverage_id)
        REFERENCES catalog_editions(edition_id, catalog_id, source_file_id, reading_rules_id, coverage_id),
    UNIQUE (catalog_id, source_file_id, reading_rules_id, coverage_id)
) STRICT;

CREATE TABLE catalog_imports (
    import_id INTEGER PRIMARY KEY NOT NULL CHECK (import_id > 0),
    import_key TEXT NOT NULL UNIQUE,
    catalog_id INTEGER NOT NULL REFERENCES catalogs(catalog_id),
    source_file_id INTEGER NOT NULL REFERENCES catalog_source_files(source_file_id),
    reading_rules_id INTEGER NOT NULL REFERENCES catalog_reading_rules(reading_rules_id),
    file_receipt_id INTEGER REFERENCES catalog_file_receipts(file_receipt_id),
    edition_id INTEGER REFERENCES catalog_editions(edition_id),
    source_format_hint TEXT,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    started_at TEXT NOT NULL,
    finished_at TEXT,
    CHECK ((status = 'running' AND finished_at IS NULL) OR (status = 'succeeded' AND edition_id IS NOT NULL AND finished_at IS NOT NULL) OR (status = 'failed' AND edition_id IS NULL AND finished_at IS NOT NULL)),
    UNIQUE (import_id, source_file_id, reading_rules_id)
) STRICT;

CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY NOT NULL CHECK (source_element_id > 0),
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    element_kind TEXT NOT NULL CHECK (element_kind IN (/* SOURCE_ELEMENT_KINDS */)),
    UNIQUE (source_element_id, edition_id)
) STRICT;
CREATE INDEX catalog_source_elements_edition_kind ON catalog_source_elements(edition_id, element_kind, source_element_id);

CREATE TABLE catalog_set_groups (
    set_group_id INTEGER PRIMARY KEY NOT NULL CHECK (set_group_id > 0),
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id),
    group_kind TEXT NOT NULL CHECK (group_kind IN ('root', 'software_list')),
    UNIQUE (set_group_id, edition_id)
) STRICT;
CREATE UNIQUE INDEX catalog_root_group ON catalog_set_groups(edition_id) WHERE group_kind = 'root';

CREATE TABLE catalog_sets (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    set_group_id INTEGER NOT NULL REFERENCES catalog_set_groups(set_group_id),
    set_name TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    UNIQUE (set_group_id, source_order)
) STRICT;

CREATE TABLE file_id_registries (
    registry_id INTEGER PRIMARY KEY NOT NULL CHECK (registry_id > 0),
    registry_uuid BLOB NOT NULL UNIQUE CHECK (length(registry_uuid) = 16)
) STRICT;

CREATE TABLE shared_catalog_files (
    file_uuid BLOB PRIMARY KEY NOT NULL CHECK (length(file_uuid) = 16),
    registry_id INTEGER NOT NULL REFERENCES file_id_registries(registry_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE catalog_media_entries (
    media_entry_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_source_elements(source_element_id),
    file_uuid BLOB REFERENCES shared_catalog_files(file_uuid)
) STRICT;
CREATE INDEX catalog_media_entries_file ON catalog_media_entries(file_uuid, media_entry_id);

CREATE TABLE hash_values (
    hash_id INTEGER PRIMARY KEY NOT NULL CHECK (hash_id > 0),
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    bytes BLOB NOT NULL,
    CHECK (length(bytes) = CASE algorithm WHEN 'crc32' THEN 4 WHEN 'md5' THEN 16 WHEN 'sha1' THEN 20 WHEN 'sha256' THEN 32 END),
    UNIQUE (algorithm, bytes)
) STRICT;

CREATE TABLE catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY NOT NULL CHECK (reported_hash_id > 0),
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
    source_hash_field TEXT NOT NULL CHECK (source_hash_field IN ('crc', 'crc32', 'md5', 'sha1', 'sha256', 'origin_sha256')),
    field_occurrence INTEGER NOT NULL CHECK (field_occurrence >= 0),
    presence TEXT NOT NULL CHECK (presence IN ('empty', 'invalid', 'value')),
    hash_scope TEXT NOT NULL CHECK (hash_scope IN ('whole_file', 'whole_asset', 'unknown', 'chd_header_sha1', 'source_origin', 'nfo_companion')),
    hash_id INTEGER REFERENCES hash_values(hash_id),
    reported_text TEXT,
    CHECK ((presence = 'empty' AND hash_id IS NULL AND reported_text IS NULL) OR (presence = 'invalid' AND hash_id IS NULL AND reported_text IS NOT NULL AND reported_text <> '') OR (presence = 'value' AND hash_id IS NOT NULL)),
    UNIQUE (media_entry_id, source_hash_field, field_occurrence)
) STRICT;
CREATE INDEX catalog_entry_hashes_media ON catalog_entry_hashes(media_entry_id, reported_hash_id);

CREATE TABLE invalid_catalog_entry_hashes (
    reported_hash_id INTEGER PRIMARY KEY NOT NULL REFERENCES catalog_entry_hashes(reported_hash_id),
    diagnostic_code TEXT NOT NULL
) STRICT;

CREATE TABLE shared_file_sizes (
    file_uuid BLOB NOT NULL REFERENCES shared_catalog_files(file_uuid),
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    PRIMARY KEY (file_uuid, byte_length)
) STRICT, WITHOUT ROWID;

CREATE TABLE shared_file_hashes (
    file_uuid BLOB NOT NULL REFERENCES shared_catalog_files(file_uuid),
    hash_id INTEGER NOT NULL REFERENCES hash_values(hash_id),
    PRIMARY KEY (file_uuid, hash_id)
) STRICT, WITHOUT ROWID;
CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(hash_id, file_uuid);

CREATE VIEW declared_catalog_hash_text AS
SELECT declaration.reported_hash_id, CASE declaration.presence
    WHEN 'empty' THEN '' WHEN 'invalid' THEN declaration.reported_text
    WHEN 'value' THEN coalesce(declaration.reported_text, lower(hex(value.bytes))) END AS declared_text
FROM catalog_entry_hashes AS declaration LEFT JOIN hash_values AS value USING (hash_id);

CREATE VIEW candidate_shared_integrity_problems AS
SELECT 'receipt_outcome' AS problem, receipt.file_receipt_id AS owner_id, edition.edition_id
FROM catalog_file_receipts AS receipt JOIN catalog_fetch_attempts AS attempt USING (fetch_attempt_id)
LEFT JOIN catalog_editions AS edition USING(file_receipt_id)
WHERE attempt.outcome <> 'retained'
UNION ALL
SELECT 'retained_receipt_count', attempt.fetch_attempt_id, NULL
FROM catalog_fetch_attempts AS attempt LEFT JOIN catalog_file_receipts AS receipt USING (fetch_attempt_id)
GROUP BY attempt.fetch_attempt_id HAVING (attempt.outcome = 'retained') <> (count(receipt.file_receipt_id) = 1)
UNION ALL
SELECT 'edition_receipt_ancestry', edition.edition_id, edition.edition_id
FROM catalog_editions AS edition JOIN catalogs AS catalog USING (catalog_id)
JOIN catalog_file_receipts AS receipt ON receipt.file_receipt_id = edition.file_receipt_id
JOIN catalog_fetch_attempts AS attempt USING (fetch_attempt_id)
WHERE receipt.source_file_id <> edition.source_file_id OR attempt.publisher_id <> catalog.publisher_id
UNION ALL
SELECT 'edition_previous_catalog', edition.edition_id, edition.edition_id
FROM catalog_editions AS edition JOIN catalog_editions AS previous ON previous.edition_id = edition.previous_edition_id
WHERE previous.catalog_id <> edition.catalog_id OR previous.edition_id = edition.edition_id
UNION ALL
SELECT 'coverage_member', member.coverage_id, edition.edition_id
FROM catalog_covered_sets AS member JOIN catalog_coverage AS scope USING (coverage_id)
LEFT JOIN catalog_editions AS edition USING(coverage_id)
WHERE scope.kind IN ('unknown', 'complete') OR (scope.kind = 'filtered' AND member.coverage = 'unknown')
UNION ALL
SELECT 'import_edition_ancestry', run.import_id, run.edition_id
FROM catalog_imports AS run JOIN catalog_editions AS edition USING (edition_id)
WHERE run.catalog_id <> edition.catalog_id OR run.source_file_id <> edition.source_file_id OR run.reading_rules_id <> edition.reading_rules_id
UNION ALL
SELECT 'import_receipt_ancestry', run.import_id, run.edition_id
FROM catalog_imports AS run JOIN catalogs AS catalog USING (catalog_id)
JOIN catalog_file_receipts AS receipt ON receipt.file_receipt_id = run.file_receipt_id
JOIN catalog_fetch_attempts AS attempt USING (fetch_attempt_id)
WHERE receipt.source_file_id <> run.source_file_id OR attempt.publisher_id <> catalog.publisher_id OR attempt.outcome <> 'retained'
UNION ALL
SELECT 'set_edition_ancestry', entry.set_id, element.edition_id
FROM catalog_sets AS entry JOIN catalog_source_elements AS element ON element.source_element_id = entry.set_id
JOIN catalog_set_groups AS parent USING (set_group_id) WHERE element.edition_id <> parent.edition_id
UNION ALL
SELECT 'invalid_hash_subtype', declaration.reported_hash_id, element.edition_id
FROM catalog_entry_hashes AS declaration JOIN catalog_source_elements AS element ON element.source_element_id = declaration.media_entry_id
LEFT JOIN invalid_catalog_entry_hashes AS invalid USING (reported_hash_id)
WHERE (declaration.presence = 'invalid') <> (invalid.reported_hash_id IS NOT NULL)
UNION ALL
SELECT 'hash_field_algorithm', declaration.reported_hash_id, element.edition_id
FROM catalog_entry_hashes AS declaration JOIN hash_values AS value USING (hash_id)
JOIN catalog_source_elements AS element ON element.source_element_id = declaration.media_entry_id
WHERE value.algorithm <> CASE declaration.source_hash_field WHEN 'crc' THEN 'crc32' WHEN 'origin_sha256' THEN 'sha256' ELSE declaration.source_hash_field END
UNION ALL
SELECT 'hash_override_bytes', declaration.reported_hash_id, element.edition_id
FROM catalog_entry_hashes AS declaration JOIN hash_values AS value USING (hash_id)
JOIN catalog_source_elements AS element ON element.source_element_id = declaration.media_entry_id
WHERE declaration.presence = 'value' AND declaration.reported_text IS NOT NULL
AND (length(declaration.reported_text) <> 2 * length(value.bytes)
    OR declaration.reported_text GLOB '*[^0-9a-fA-F]*'
    OR lower(declaration.reported_text) <> lower(hex(value.bytes)));

CREATE TRIGGER candidate_receipt_outcome_insert BEFORE INSERT ON catalog_file_receipts
WHEN NOT EXISTS (SELECT 1 FROM catalog_fetch_attempts WHERE fetch_attempt_id=NEW.fetch_attempt_id AND outcome='retained')
BEGIN SELECT RAISE(ABORT,'receipt requires a retained successful attempt'); END;
CREATE TRIGGER candidate_receipt_outcome_update BEFORE UPDATE ON catalog_file_receipts
WHEN NOT EXISTS (SELECT 1 FROM catalog_fetch_attempts WHERE fetch_attempt_id=NEW.fetch_attempt_id AND outcome='retained')
BEGIN SELECT RAISE(ABORT,'receipt requires a retained successful attempt'); END;

CREATE TRIGGER candidate_edition_ancestry_insert BEFORE INSERT ON catalog_editions
WHEN (NEW.previous_edition_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_editions WHERE edition_id=NEW.previous_edition_id AND catalog_id=NEW.catalog_id AND edition_id<>NEW.edition_id
)) OR (NEW.file_receipt_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_file_receipts AS receipt JOIN catalog_fetch_attempts AS attempt USING(fetch_attempt_id)
    JOIN catalogs AS catalog ON catalog.catalog_id=NEW.catalog_id
    WHERE receipt.file_receipt_id=NEW.file_receipt_id AND receipt.source_file_id=NEW.source_file_id AND attempt.publisher_id=catalog.publisher_id
))
BEGIN SELECT RAISE(ABORT,'edition lineage or receipt ancestry'); END;

CREATE TRIGGER candidate_edition_ancestry_update BEFORE UPDATE ON catalog_editions
WHEN (NEW.previous_edition_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_editions WHERE edition_id=NEW.previous_edition_id AND catalog_id=NEW.catalog_id AND edition_id<>NEW.edition_id
)) OR (NEW.file_receipt_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_file_receipts AS receipt JOIN catalog_fetch_attempts AS attempt USING(fetch_attempt_id)
    JOIN catalogs AS catalog ON catalog.catalog_id=NEW.catalog_id
    WHERE receipt.file_receipt_id=NEW.file_receipt_id AND receipt.source_file_id=NEW.source_file_id AND attempt.publisher_id=catalog.publisher_id
))
BEGIN SELECT RAISE(ABORT,'edition lineage or receipt ancestry'); END;

CREATE TRIGGER candidate_import_ancestry_insert BEFORE INSERT ON catalog_imports
WHEN (NEW.edition_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_editions WHERE edition_id=NEW.edition_id AND catalog_id=NEW.catalog_id AND source_file_id=NEW.source_file_id AND reading_rules_id=NEW.reading_rules_id
)) OR (NEW.file_receipt_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_file_receipts AS receipt JOIN catalog_fetch_attempts AS attempt USING(fetch_attempt_id)
    JOIN catalogs AS catalog ON catalog.catalog_id=NEW.catalog_id
    WHERE receipt.file_receipt_id=NEW.file_receipt_id AND receipt.source_file_id=NEW.source_file_id AND attempt.publisher_id=catalog.publisher_id
))
BEGIN SELECT RAISE(ABORT,'import edition or receipt ancestry'); END;

CREATE TRIGGER candidate_import_ancestry_update BEFORE UPDATE ON catalog_imports
WHEN (NEW.edition_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_editions WHERE edition_id=NEW.edition_id AND catalog_id=NEW.catalog_id AND source_file_id=NEW.source_file_id AND reading_rules_id=NEW.reading_rules_id
)) OR (NEW.file_receipt_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM catalog_file_receipts AS receipt JOIN catalog_fetch_attempts AS attempt USING(fetch_attempt_id)
    JOIN catalogs AS catalog ON catalog.catalog_id=NEW.catalog_id
    WHERE receipt.file_receipt_id=NEW.file_receipt_id AND receipt.source_file_id=NEW.source_file_id AND attempt.publisher_id=catalog.publisher_id
))
BEGIN SELECT RAISE(ABORT,'import edition or receipt ancestry'); END;

CREATE TRIGGER candidate_coverage_member_insert BEFORE INSERT ON catalog_covered_sets
WHEN NOT EXISTS (SELECT 1 FROM catalog_coverage WHERE coverage_id=NEW.coverage_id AND (kind='partial' OR (kind='filtered' AND NEW.coverage='covered')))
BEGIN SELECT RAISE(ABORT,'coverage kind does not allow this member'); END;

CREATE TRIGGER candidate_coverage_kind_update BEFORE UPDATE OF kind ON catalog_coverage
WHEN EXISTS(SELECT 1 FROM catalog_covered_sets WHERE coverage_id=OLD.coverage_id
    AND (NEW.kind IN ('unknown','complete') OR (NEW.kind='filtered' AND coverage='unknown')))
BEGIN SELECT RAISE(ABORT,'new coverage kind conflicts with existing members'); END;
CREATE TRIGGER candidate_referenced_coverage_update BEFORE UPDATE ON catalog_coverage
WHEN EXISTS(SELECT 1 FROM catalog_editions WHERE coverage_id=OLD.coverage_id)
BEGIN SELECT RAISE(ABORT,'edition coverage identity is immutable'); END;
CREATE TRIGGER candidate_referenced_coverage_member_insert BEFORE INSERT ON catalog_covered_sets
WHEN EXISTS(SELECT 1 FROM catalog_editions WHERE coverage_id=NEW.coverage_id)
BEGIN SELECT RAISE(ABORT,'edition coverage members are immutable'); END;
CREATE TRIGGER candidate_referenced_coverage_member_update BEFORE UPDATE ON catalog_covered_sets
WHEN EXISTS(SELECT 1 FROM catalog_editions WHERE coverage_id IN (OLD.coverage_id,NEW.coverage_id))
BEGIN SELECT RAISE(ABORT,'edition coverage members are immutable'); END;
CREATE TRIGGER candidate_referenced_coverage_member_delete BEFORE DELETE ON catalog_covered_sets
WHEN EXISTS(SELECT 1 FROM catalog_editions WHERE coverage_id=OLD.coverage_id)
BEGIN SELECT RAISE(ABORT,'edition coverage members are immutable'); END;

CREATE TRIGGER candidate_coverage_member_update BEFORE UPDATE ON catalog_covered_sets
WHEN NOT EXISTS (SELECT 1 FROM catalog_coverage WHERE coverage_id=NEW.coverage_id AND (kind='partial' OR (kind='filtered' AND NEW.coverage='covered')))
BEGIN SELECT RAISE(ABORT,'coverage kind does not allow this member'); END;

CREATE TRIGGER candidate_publication_immutable_update BEFORE UPDATE ON published_catalog_editions
BEGIN SELECT RAISE(ABORT,'publication seals are immutable'); END;
CREATE TRIGGER candidate_publication_immutable_delete BEFORE DELETE ON published_catalog_editions
BEGIN SELECT RAISE(ABORT,'publication seals are immutable'); END;

CREATE TRIGGER candidate_hash_value_immutable_update BEFORE UPDATE ON hash_values
BEGIN SELECT RAISE(ABORT,'interned hash meanings are immutable'); END;
CREATE TRIGGER candidate_hash_value_immutable_delete BEFORE DELETE ON hash_values
BEGIN SELECT RAISE(ABORT,'interned hash identities are immutable'); END;

CREATE TRIGGER candidate_retained_fetch_immutable BEFORE UPDATE ON catalog_fetch_attempts
WHEN EXISTS(SELECT 1 FROM catalog_file_receipts WHERE fetch_attempt_id=OLD.fetch_attempt_id)
BEGIN SELECT RAISE(ABORT,'retained fetch provenance is immutable'); END;

CREATE TRIGGER candidate_edition_cycle_insert BEFORE INSERT ON catalog_editions
WHEN NEW.previous_edition_id IS NOT NULL AND EXISTS (
    WITH RECURSIVE ancestors(edition_id) AS (
        VALUES(NEW.previous_edition_id)
        UNION SELECT edition.previous_edition_id FROM catalog_editions AS edition JOIN ancestors ON edition.edition_id=ancestors.edition_id WHERE edition.previous_edition_id IS NOT NULL
    ) SELECT 1 FROM ancestors WHERE edition_id=NEW.edition_id
)
BEGIN SELECT RAISE(ABORT,'previous-edition ancestry cycle'); END;
CREATE TRIGGER candidate_edition_cycle_update BEFORE UPDATE ON catalog_editions
WHEN NEW.previous_edition_id IS NOT NULL AND EXISTS (
    WITH RECURSIVE ancestors(edition_id) AS (
        VALUES(NEW.previous_edition_id)
        UNION SELECT edition.previous_edition_id FROM catalog_editions AS edition JOIN ancestors ON edition.edition_id=ancestors.edition_id WHERE edition.previous_edition_id IS NOT NULL
    ) SELECT 1 FROM ancestors WHERE edition_id=NEW.edition_id
)
BEGIN SELECT RAISE(ABORT,'previous-edition ancestry cycle'); END;

CREATE VIEW candidate_edition_cycle_problems AS
WITH RECURSIVE ancestry(start_id,ancestor_id) AS (
    SELECT edition_id,previous_edition_id FROM catalog_editions WHERE previous_edition_id IS NOT NULL
    UNION SELECT ancestry.start_id,parent.previous_edition_id FROM ancestry JOIN catalog_editions AS parent ON parent.edition_id=ancestry.ancestor_id WHERE parent.previous_edition_id IS NOT NULL
)
SELECT 'edition_ancestry_cycle' AS problem,start_id AS owner_id,start_id AS edition_id FROM ancestry WHERE start_id=ancestor_id;

CREATE TRIGGER candidate_hash_semantics_insert BEFORE INSERT ON catalog_entry_hashes
WHEN NEW.presence='value' AND NOT EXISTS(
    SELECT 1 FROM hash_values AS value WHERE value.hash_id=NEW.hash_id
    AND value.algorithm=CASE NEW.source_hash_field WHEN 'crc' THEN 'crc32' WHEN 'origin_sha256' THEN 'sha256' ELSE NEW.source_hash_field END
    AND (NEW.reported_text IS NULL OR (length(NEW.reported_text)=2*length(value.bytes)
        AND NEW.reported_text NOT GLOB '*[^0-9a-fA-F]*' AND lower(NEW.reported_text)=lower(hex(value.bytes))))
)
BEGIN SELECT RAISE(ABORT,'declared hash spelling, algorithm and bytes must agree'); END;
CREATE TRIGGER candidate_hash_semantics_update BEFORE UPDATE ON catalog_entry_hashes
WHEN NEW.presence='value' AND NOT EXISTS(
    SELECT 1 FROM hash_values AS value WHERE value.hash_id=NEW.hash_id
    AND value.algorithm=CASE NEW.source_hash_field WHEN 'crc' THEN 'crc32' WHEN 'origin_sha256' THEN 'sha256' ELSE NEW.source_hash_field END
    AND (NEW.reported_text IS NULL OR (length(NEW.reported_text)=2*length(value.bytes)
        AND NEW.reported_text NOT GLOB '*[^0-9a-fA-F]*' AND lower(NEW.reported_text)=lower(hex(value.bytes))))
)
BEGIN SELECT RAISE(ABORT,'declared hash spelling, algorithm and bytes must agree'); END;
