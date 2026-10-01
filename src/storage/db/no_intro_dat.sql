CREATE TABLE no_intro_dat_documents (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    schema_location TEXT,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_headers (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES no_intro_dat_documents(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    id_text TEXT,
    name TEXT,
    description TEXT,
    version_text TEXT,
    date TEXT,
    author TEXT,
    homepage TEXT,
    url TEXT,
    trademarks TEXT,
    piracy TEXT,
    subset TEXT,
    comment TEXT
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_clrmamepro_options (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES no_intro_dat_headers(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    forcenodump_text TEXT,
    forcenodump_effective TEXT GENERATED ALWAYS AS (
      CASE
        WHEN forcenodump_text IS NULL THEN 'obsolete'
        WHEN trim(forcenodump_text, char(9) || char(10) || char(13) || ' ') IN ('obsolete','required','ignore')
          THEN trim(forcenodump_text, char(9) || char(10) || char(13) || ' ')
      END
    ) VIRTUAL,
    header_text TEXT
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_romcenter_options (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES no_intro_dat_headers(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    plugin_text TEXT
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_games (
    set_id INTEGER PRIMARY KEY NOT NULL,
    source_element_kind TEXT NOT NULL DEFAULT 'no_intro_dat_game'
        CHECK (source_element_kind = 'no_intro_dat_game'),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    id_text TEXT,
    cloneof_text TEXT,
    cloneofid_text TEXT,
    description_text TEXT,
    FOREIGN KEY (set_id, source_element_kind)
        REFERENCES catalog_sets(set_id, source_element_kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_categories (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id) ON DELETE RESTRICT,
    category_order INTEGER NOT NULL CHECK (typeof(category_order) = 'integer' AND category_order >= 0),
    category TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (set_id, category_order),
    UNIQUE (set_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_identifiers (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id) ON DELETE RESTRICT,
    identifier_order INTEGER NOT NULL CHECK (typeof(identifier_order) = 'integer' AND identifier_order >= 0),
    identifier TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (set_id, identifier_order),
    UNIQUE (set_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_releases (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id) ON DELETE RESTRICT,
    release_order INTEGER NOT NULL CHECK (typeof(release_order) = 'integer' AND release_order >= 0),
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    name_source_order INTEGER NOT NULL CHECK (typeof(name_source_order) = 'integer' AND name_source_order >= 0),
    name_source_line INTEGER NOT NULL CHECK (typeof(name_source_line) = 'integer' AND name_source_line > 0),
    name_source_column INTEGER NOT NULL CHECK (typeof(name_source_column) = 'integer' AND name_source_column > 0),
    region_source_order INTEGER NOT NULL CHECK (typeof(region_source_order) = 'integer' AND region_source_order >= 0),
    region_source_line INTEGER NOT NULL CHECK (typeof(region_source_line) = 'integer' AND region_source_line > 0),
    region_source_column INTEGER NOT NULL CHECK (typeof(region_source_column) = 'integer' AND region_source_column > 0),
    PRIMARY KEY (set_id, release_order),
    UNIQUE (set_id, source_order)
) WITHOUT ROWID;

-- Invalid and overflowing size text is retained with a NULL projection.
CREATE TABLE no_intro_dat_rom_claims (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL DEFAULT 'no_intro_dat_rom' CHECK (claim_kind = 'no_intro_dat_rom'),
    name TEXT NOT NULL,
    size_text TEXT,
    size INTEGER GENERATED ALWAYS AS (
      CASE
        WHEN size_text IS NULL THEN NULL
        WHEN trim(size_text, char(9) || char(10) || char(13) || ' ') = '' THEN NULL
        WHEN (CASE WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) IN ('+','-')
          THEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2)
          ELSE trim(size_text, char(9) || char(10) || char(13) || ' ') END) = '' THEN NULL
        WHEN (CASE WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) IN ('+','-')
          THEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2)
          ELSE trim(size_text, char(9) || char(10) || char(13) || ' ') END) GLOB '*[^0-9]*' THEN NULL
        WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) NOT IN ('+','-')
          AND trim(size_text, char(9) || char(10) || char(13) || ' ') GLOB '*[^0-9]*' THEN NULL
        WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) = '-'
          AND ltrim(substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2),'0') <> '' THEN NULL
        WHEN length(ltrim(CASE WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) IN ('+','-')
          THEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2)
          ELSE trim(size_text, char(9) || char(10) || char(13) || ' ') END,'0')) > 19 THEN NULL
        WHEN length(ltrim(CASE WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) IN ('+','-')
          THEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2)
          ELSE trim(size_text, char(9) || char(10) || char(13) || ' ') END,'0')) = 19
          AND ltrim(CASE WHEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),1,1) IN ('+','-')
          THEN substr(trim(size_text, char(9) || char(10) || char(13) || ' '),2)
          ELSE trim(size_text, char(9) || char(10) || char(13) || ' ') END,'0') > '9223372036854775807' THEN NULL
        ELSE CAST(trim(size_text, char(9) || char(10) || char(13) || ' ') AS INTEGER)
      END
    ) VIRTUAL,
    status_text TEXT,
    serial_text TEXT,
    header_text TEXT,
    date_text TEXT,
    mia_text TEXT,
    evidence_scope TEXT NOT NULL CHECK (evidence_scope IN ('whole_file', 'unknown')),
    evidence_provenance TEXT NOT NULL DEFAULT 'source_declared'
        CHECK (evidence_provenance = 'source_declared'),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    FOREIGN KEY (occurrence_id, claim_kind)
        REFERENCES asset_occurrences(occurrence_id, claim_kind) ON DELETE RESTRICT,
    CHECK (evidence_scope IN ('whole_file', 'unknown'))
) WITHOUT ROWID;

-- A declared hash owns either one interned value or its uninterpretable literal.
-- Original spelling of usable hashes remains in the external source object.
CREATE TABLE no_intro_dat_rom_digest_fields (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_dat_rom_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 2 AND 5),
    digest_id INTEGER REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    invalid_text TEXT,
    PRIMARY KEY (occurrence_id, field_kind),
    CHECK ((digest_id IS NOT NULL AND invalid_text IS NULL)
      OR (digest_id IS NULL AND invalid_text IS NOT NULL
        AND NOT (length(invalid_text) = CASE field_kind WHEN 2 THEN 8 WHEN 3 THEN 32 WHEN 4 THEN 40 WHEN 5 THEN 64 END
          AND invalid_text NOT GLOB '*[^0-9A-Fa-f]*')))
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_header_field_positions (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_dat_headers(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 11),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key, field_kind),
    UNIQUE (snapshot_key, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_clrmamepro_field_positions (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_dat_clrmamepro_options(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key, field_kind),
    UNIQUE (snapshot_key, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_romcenter_field_positions (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_dat_romcenter_options(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind = 0),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key, field_kind),
    UNIQUE (snapshot_key, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_game_field_positions (
    set_id INTEGER NOT NULL REFERENCES no_intro_dat_games(set_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 4),
    position_domain INTEGER GENERATED ALWAYS AS (CASE WHEN field_kind = 4 THEN 1 ELSE 0 END) VIRTUAL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (set_id, field_kind),
    UNIQUE (set_id, position_domain, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_rom_field_positions (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_dat_rom_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 10),
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (occurrence_id, field_kind),
    UNIQUE (occurrence_id, source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_dat_parse_counts (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES no_intro_dat_documents(snapshot_key) ON DELETE RESTRICT,
    game_count INTEGER NOT NULL CHECK (typeof(game_count) = 'integer' AND game_count >= 0),
    rom_count INTEGER NOT NULL CHECK (typeof(rom_count) = 'integer' AND rom_count >= 0),
    category_count INTEGER NOT NULL CHECK (typeof(category_count) = 'integer' AND category_count >= 0),
    identifier_count INTEGER NOT NULL CHECK (typeof(identifier_count) = 'integer' AND identifier_count >= 0),
    release_count INTEGER NOT NULL CHECK (typeof(release_count) = 'integer' AND release_count >= 0),
    clrmamepro_option_count INTEGER NOT NULL CHECK (typeof(clrmamepro_option_count) = 'integer' AND clrmamepro_option_count IN (0, 1)),
    romcenter_option_count INTEGER NOT NULL CHECK (typeof(romcenter_option_count) = 'integer' AND romcenter_option_count IN (0, 1)),
    header_field_count INTEGER NOT NULL CHECK (typeof(header_field_count) = 'integer' AND header_field_count >= 0),
    clrmamepro_field_count INTEGER NOT NULL CHECK (typeof(clrmamepro_field_count) = 'integer' AND clrmamepro_field_count >= 0),
    romcenter_field_count INTEGER NOT NULL CHECK (typeof(romcenter_field_count) = 'integer' AND romcenter_field_count >= 0),
    game_field_count INTEGER NOT NULL CHECK (typeof(game_field_count) = 'integer' AND game_field_count >= 0),
    rom_field_count INTEGER NOT NULL CHECK (typeof(rom_field_count) = 'integer' AND rom_field_count >= 0)
) WITHOUT ROWID;

CREATE VIEW no_intro_dat_header_declared_field_presence AS
SELECT snapshot_key, 0 AS field_kind FROM no_intro_dat_headers WHERE id_text IS NOT NULL
UNION ALL SELECT snapshot_key, 1 FROM no_intro_dat_headers WHERE name IS NOT NULL
UNION ALL SELECT snapshot_key, 2 FROM no_intro_dat_headers WHERE description IS NOT NULL
UNION ALL SELECT snapshot_key, 3 FROM no_intro_dat_headers WHERE version_text IS NOT NULL
UNION ALL SELECT snapshot_key, 4 FROM no_intro_dat_headers WHERE date IS NOT NULL
UNION ALL SELECT snapshot_key, 5 FROM no_intro_dat_headers WHERE author IS NOT NULL
UNION ALL SELECT snapshot_key, 6 FROM no_intro_dat_headers WHERE homepage IS NOT NULL
UNION ALL SELECT snapshot_key, 7 FROM no_intro_dat_headers WHERE url IS NOT NULL
UNION ALL SELECT snapshot_key, 8 FROM no_intro_dat_headers WHERE trademarks IS NOT NULL
UNION ALL SELECT snapshot_key, 9 FROM no_intro_dat_headers WHERE piracy IS NOT NULL
UNION ALL SELECT snapshot_key, 10 FROM no_intro_dat_headers WHERE subset IS NOT NULL
UNION ALL SELECT snapshot_key, 11 FROM no_intro_dat_headers WHERE comment IS NOT NULL;

CREATE VIEW no_intro_dat_clrmamepro_declared_field_presence AS
SELECT snapshot_key, 0 AS field_kind FROM no_intro_dat_clrmamepro_options WHERE forcenodump_text IS NOT NULL
UNION ALL SELECT snapshot_key, 1 FROM no_intro_dat_clrmamepro_options WHERE header_text IS NOT NULL;

CREATE VIEW no_intro_dat_romcenter_declared_field_presence AS
SELECT snapshot_key, 0 AS field_kind FROM no_intro_dat_romcenter_options WHERE plugin_text IS NOT NULL;

CREATE VIEW no_intro_dat_game_declared_field_presence AS
SELECT set_id, 0 AS field_kind FROM catalog_sets WHERE source_element_kind = 'no_intro_dat_game'
UNION ALL SELECT set_id, 1 FROM no_intro_dat_games WHERE id_text IS NOT NULL
UNION ALL SELECT set_id, 2 FROM no_intro_dat_games WHERE cloneof_text IS NOT NULL
UNION ALL SELECT set_id, 3 FROM no_intro_dat_games WHERE cloneofid_text IS NOT NULL
UNION ALL SELECT set_id, 4 FROM no_intro_dat_games WHERE description_text IS NOT NULL;

CREATE VIEW no_intro_dat_rom_declared_field_presence AS
SELECT occurrence_id, 0 AS field_kind FROM no_intro_dat_rom_claims
UNION ALL SELECT occurrence_id, 1 FROM no_intro_dat_rom_claims WHERE size_text IS NOT NULL
UNION ALL SELECT occurrence_id, field_kind FROM no_intro_dat_rom_digest_fields
UNION ALL SELECT occurrence_id, 6 FROM no_intro_dat_rom_claims WHERE status_text IS NOT NULL
UNION ALL SELECT occurrence_id, 7 FROM no_intro_dat_rom_claims WHERE serial_text IS NOT NULL
UNION ALL SELECT occurrence_id, 8 FROM no_intro_dat_rom_claims WHERE header_text IS NOT NULL
UNION ALL SELECT occurrence_id, 9 FROM no_intro_dat_rom_claims WHERE date_text IS NOT NULL
UNION ALL SELECT occurrence_id, 10 FROM no_intro_dat_rom_claims WHERE mia_text IS NOT NULL;

CREATE VIEW no_intro_dat_rom_expected_digest_assertions AS
SELECT field.occurrence_id, field.digest_id, claim.evidence_scope AS scope
FROM no_intro_dat_rom_digest_fields AS field
JOIN no_intro_dat_rom_claims AS claim USING (occurrence_id)
WHERE field.digest_id IS NOT NULL;

CREATE TRIGGER no_intro_dat_rom_digest_fields_native_insert BEFORE INSERT ON no_intro_dat_rom_digest_fields
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_rom_claims WHERE occurrence_id = NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM no_intro_dat_rom_digest_fields
    WHERE occurrence_id = NEW.occurrence_id AND field_kind = NEW.field_kind)
 OR (NEW.digest_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM digest_values AS digest
    JOIN occurrence_digest_assertions AS assertion USING (digest_id)
    JOIN no_intro_dat_rom_claims AS claim USING (occurrence_id)
    WHERE assertion.occurrence_id = NEW.occurrence_id AND digest.digest_id = NEW.digest_id
      AND digest.algorithm = CASE NEW.field_kind WHEN 2 THEN 'crc32' WHEN 3 THEN 'md5' WHEN 4 THEN 'sha1' WHEN 5 THEN 'sha256' END
      AND assertion.scope = claim.evidence_scope AND assertion.provenance = 'source_declared'
 ))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT hash field requires a present unpublished matching owner'); END;
CREATE TRIGGER no_intro_dat_rom_digest_fields_immutable_update BEFORE UPDATE ON no_intro_dat_rom_digest_fields
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT hash fields are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_digest_fields_immutable_delete BEFORE DELETE ON no_intro_dat_rom_digest_fields
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT hash fields are immutable'); END;

-- SQLite REPLACE can avoid delete triggers when recursive_triggers is off.
CREATE TRIGGER no_intro_dat_documents_immutable_insert BEFORE INSERT ON no_intro_dat_documents
WHEN EXISTS (SELECT 1 FROM no_intro_dat_documents WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_headers_immutable_insert BEFORE INSERT ON no_intro_dat_headers
WHEN EXISTS (SELECT 1 FROM no_intro_dat_headers WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_options_immutable_insert BEFORE INSERT ON no_intro_dat_clrmamepro_options
WHEN EXISTS (SELECT 1 FROM no_intro_dat_clrmamepro_options WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_options_immutable_insert BEFORE INSERT ON no_intro_dat_romcenter_options
WHEN EXISTS (SELECT 1 FROM no_intro_dat_romcenter_options WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_games_immutable_insert BEFORE INSERT ON no_intro_dat_games
WHEN EXISTS (SELECT 1 FROM no_intro_dat_games WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_categories_immutable_insert BEFORE INSERT ON no_intro_dat_categories
WHEN EXISTS (SELECT 1 FROM no_intro_dat_categories WHERE set_id = NEW.set_id AND (category_order = NEW.category_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_identifiers_immutable_insert BEFORE INSERT ON no_intro_dat_identifiers
WHEN EXISTS (SELECT 1 FROM no_intro_dat_identifiers WHERE set_id = NEW.set_id AND (identifier_order = NEW.identifier_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_releases_immutable_insert BEFORE INSERT ON no_intro_dat_releases
WHEN EXISTS (SELECT 1 FROM no_intro_dat_releases WHERE set_id = NEW.set_id AND (release_order = NEW.release_order OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_claims_immutable_insert BEFORE INSERT ON no_intro_dat_rom_claims
WHEN EXISTS (SELECT 1 FROM no_intro_dat_rom_claims WHERE occurrence_id = NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_header_field_positions_immutable_insert BEFORE INSERT ON no_intro_dat_header_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dat_header_field_positions WHERE snapshot_key = NEW.snapshot_key
    AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_field_positions_immutable_insert BEFORE INSERT ON no_intro_dat_clrmamepro_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dat_clrmamepro_field_positions WHERE snapshot_key = NEW.snapshot_key
    AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_field_positions_immutable_insert BEFORE INSERT ON no_intro_dat_romcenter_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dat_romcenter_field_positions WHERE snapshot_key = NEW.snapshot_key
    AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_game_field_positions_immutable_insert BEFORE INSERT ON no_intro_dat_game_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dat_game_field_positions WHERE set_id = NEW.set_id
    AND (field_kind = NEW.field_kind OR (position_domain = NEW.position_domain AND source_order = NEW.source_order)))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_field_positions_immutable_insert BEFORE INSERT ON no_intro_dat_rom_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dat_rom_field_positions WHERE occurrence_id = NEW.occurrence_id
    AND (field_kind = NEW.field_kind OR source_order = NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_parse_counts_immutable_insert BEFORE INSERT ON no_intro_dat_parse_counts
WHEN EXISTS (SELECT 1 FROM no_intro_dat_parse_counts WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT parse counts are immutable'); END;

CREATE TRIGGER no_intro_dat_documents_native_insert BEFORE INSERT ON no_intro_dat_documents
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible', 'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible'))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT document requires an unpublished matching interpretation'); END;
CREATE TRIGGER no_intro_dat_headers_native_insert BEFORE INSERT ON no_intro_dat_headers
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_documents WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT header requires an unpublished document'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_options_native_insert BEFORE INSERT ON no_intro_dat_clrmamepro_options
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_headers WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT option requires an unpublished header'); END;
CREATE TRIGGER no_intro_dat_romcenter_options_native_insert BEFORE INSERT ON no_intro_dat_romcenter_options
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_headers WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT option requires an unpublished header'); END;
CREATE TRIGGER no_intro_dat_games_native_insert BEFORE INSERT ON no_intro_dat_games
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key) JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.set_id AND source_element_kind = 'no_intro_dat_game'
      AND catalog_set_groups.kind = 'root'
      AND format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible', 'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible'))
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT game requires an unpublished matching owner'); END;
CREATE TRIGGER no_intro_dat_rom_claims_native_insert BEFORE INSERT ON no_intro_dat_rom_claims
WHEN NOT EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE occurrence_id = NEW.occurrence_id AND claim_kind = 'no_intro_dat_rom'
      AND source_element_kind = 'no_intro_dat_game'
      AND catalog_set_groups.kind = 'root'
      AND format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible', 'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible'))
 OR EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id)
 OR NEW.evidence_scope <> CASE
     WHEN NEW.header_text IS NOT NULL OR EXISTS (
       SELECT 1 FROM asset_occurrences AS occurrence
       JOIN catalog_sets AS game ON game.set_id = occurrence.record_id
       JOIN catalog_set_groups AS grouping USING (set_group_id)
       JOIN no_intro_dat_clrmamepro_options AS options USING (snapshot_key)
       WHERE occurrence.occurrence_id = NEW.occurrence_id AND options.header_text IS NOT NULL
     ) THEN 'unknown' ELSE 'whole_file' END
 OR (NEW.evidence_scope = 'unknown' AND EXISTS (
     SELECT 1 FROM asset_occurrences WHERE occurrence_id = NEW.occurrence_id AND content_uuid IS NOT NULL
 ))
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT ROM requires an unpublished matching owner'); END;
CREATE TRIGGER no_intro_dat_parse_counts_native_insert BEFORE INSERT ON no_intro_dat_parse_counts
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_documents WHERE snapshot_key = NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT counts require an unpublished document'); END;

CREATE TRIGGER no_intro_dat_header_field_positions_native_insert BEFORE INSERT ON no_intro_dat_header_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_header_declared_field_presence WHERE snapshot_key = NEW.snapshot_key AND field_kind = NEW.field_kind)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT position requires a present unpublished field'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_field_positions_native_insert BEFORE INSERT ON no_intro_dat_clrmamepro_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_clrmamepro_declared_field_presence WHERE snapshot_key = NEW.snapshot_key AND field_kind = NEW.field_kind)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT position requires a present unpublished field'); END;
CREATE TRIGGER no_intro_dat_romcenter_field_positions_native_insert BEFORE INSERT ON no_intro_dat_romcenter_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_romcenter_declared_field_presence WHERE snapshot_key = NEW.snapshot_key AND field_kind = NEW.field_kind)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT position requires a present unpublished field'); END;
CREATE TRIGGER no_intro_dat_game_field_positions_native_insert BEFORE INSERT ON no_intro_dat_game_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_game_declared_field_presence WHERE set_id = NEW.set_id AND field_kind = NEW.field_kind)
 OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT position requires a present unpublished field'); END;
CREATE TRIGGER no_intro_dat_rom_field_positions_native_insert BEFORE INSERT ON no_intro_dat_rom_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dat_rom_declared_field_presence WHERE occurrence_id = NEW.occurrence_id AND field_kind = NEW.field_kind)
 OR EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT position requires a present unpublished field'); END;

CREATE TRIGGER no_intro_dat_documents_immutable_update BEFORE UPDATE ON no_intro_dat_documents
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_documents_immutable_delete BEFORE DELETE ON no_intro_dat_documents
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_headers_immutable_update BEFORE UPDATE ON no_intro_dat_headers
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_headers_immutable_delete BEFORE DELETE ON no_intro_dat_headers
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_options_immutable_update BEFORE UPDATE ON no_intro_dat_clrmamepro_options
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_options_immutable_delete BEFORE DELETE ON no_intro_dat_clrmamepro_options
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_options_immutable_update BEFORE UPDATE ON no_intro_dat_romcenter_options
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_options_immutable_delete BEFORE DELETE ON no_intro_dat_romcenter_options
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_games_immutable_update BEFORE UPDATE ON no_intro_dat_games
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_games_immutable_delete BEFORE DELETE ON no_intro_dat_games
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_categories_immutable_update BEFORE UPDATE ON no_intro_dat_categories
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_categories_immutable_delete BEFORE DELETE ON no_intro_dat_categories
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_identifiers_immutable_update BEFORE UPDATE ON no_intro_dat_identifiers
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_identifiers_immutable_delete BEFORE DELETE ON no_intro_dat_identifiers
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_releases_immutable_update BEFORE UPDATE ON no_intro_dat_releases
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_releases_immutable_delete BEFORE DELETE ON no_intro_dat_releases
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_claims_immutable_update BEFORE UPDATE ON no_intro_dat_rom_claims
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_claims_immutable_delete BEFORE DELETE ON no_intro_dat_rom_claims
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native rows are immutable'); END;
CREATE TRIGGER no_intro_dat_header_field_positions_immutable_update BEFORE UPDATE ON no_intro_dat_header_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_header_field_positions_immutable_delete BEFORE DELETE ON no_intro_dat_header_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_field_positions_immutable_update BEFORE UPDATE ON no_intro_dat_clrmamepro_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_clrmamepro_field_positions_immutable_delete BEFORE DELETE ON no_intro_dat_clrmamepro_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_field_positions_immutable_update BEFORE UPDATE ON no_intro_dat_romcenter_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_romcenter_field_positions_immutable_delete BEFORE DELETE ON no_intro_dat_romcenter_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_game_field_positions_immutable_update BEFORE UPDATE ON no_intro_dat_game_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_game_field_positions_immutable_delete BEFORE DELETE ON no_intro_dat_game_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_field_positions_immutable_update BEFORE UPDATE ON no_intro_dat_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_rom_field_positions_immutable_delete BEFORE DELETE ON no_intro_dat_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT positions are immutable'); END;
CREATE TRIGGER no_intro_dat_parse_counts_immutable_update BEFORE UPDATE ON no_intro_dat_parse_counts
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT counts are immutable'); END;
CREATE TRIGGER no_intro_dat_parse_counts_immutable_delete BEFORE DELETE ON no_intro_dat_parse_counts
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT counts are immutable'); END;

CREATE TRIGGER no_intro_dat_native_requires_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key)
    LEFT JOIN no_intro_dat_documents USING (snapshot_key)
    LEFT JOIN no_intro_dat_headers USING (snapshot_key)
    LEFT JOIN no_intro_dat_parse_counts USING (snapshot_key)
    WHERE snapshot_key = NEW.snapshot_key
      AND format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible', 'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible')
      AND (no_intro_dat_documents.snapshot_key IS NULL OR no_intro_dat_headers.snapshot_key IS NULL
        OR no_intro_dat_parse_counts.snapshot_key IS NULL
        OR game_count <> (SELECT COUNT(*) FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
            WHERE snapshot_key = NEW.snapshot_key AND kind = 'root' AND source_element_kind = 'no_intro_dat_game')
        OR rom_count <> (SELECT COUNT(*) FROM asset_occurrences JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id
            JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key AND asset_occurrences.claim_kind = 'no_intro_dat_rom')
        OR category_count <> (SELECT COUNT(*) FROM no_intro_dat_categories JOIN catalog_sets ON catalog_sets.set_id = no_intro_dat_categories.set_id
            JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key)
        OR identifier_count <> (SELECT COUNT(*) FROM no_intro_dat_identifiers JOIN catalog_sets ON catalog_sets.set_id = no_intro_dat_identifiers.set_id
            JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key)
        OR release_count <> (SELECT COUNT(*) FROM no_intro_dat_releases JOIN catalog_sets ON catalog_sets.set_id = no_intro_dat_releases.set_id
            JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key)
        OR clrmamepro_option_count <> (SELECT COUNT(*) FROM no_intro_dat_clrmamepro_options WHERE snapshot_key = NEW.snapshot_key)
        OR romcenter_option_count <> (SELECT COUNT(*) FROM no_intro_dat_romcenter_options WHERE snapshot_key = NEW.snapshot_key)
        OR header_field_count <> (SELECT COUNT(*) FROM no_intro_dat_header_field_positions WHERE snapshot_key = NEW.snapshot_key)
        OR clrmamepro_field_count <> (SELECT COUNT(*) FROM no_intro_dat_clrmamepro_field_positions WHERE snapshot_key = NEW.snapshot_key)
        OR romcenter_field_count <> (SELECT COUNT(*) FROM no_intro_dat_romcenter_field_positions WHERE snapshot_key = NEW.snapshot_key)
        OR game_field_count <> (SELECT COUNT(*) FROM no_intro_dat_game_field_positions JOIN catalog_sets USING (set_id)
            JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key)
        OR rom_field_count <> (SELECT COUNT(*) FROM no_intro_dat_rom_field_positions JOIN asset_occurrences USING (occurrence_id)
            JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = NEW.snapshot_key)
        OR NOT EXISTS (SELECT 1 FROM catalog_set_groups WHERE snapshot_key = NEW.snapshot_key AND kind = 'root')
        OR EXISTS (SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
            WHERE snapshot_key = NEW.snapshot_key AND source_element_kind = 'no_intro_dat_game' AND kind <> 'root')
        OR EXISTS (SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
            WHERE snapshot_key = NEW.snapshot_key AND kind = 'root' AND source_element_kind = 'no_intro_dat_game'
            GROUP BY set_group_id HAVING MIN(catalog_sets.list_order) <> 0 OR MAX(catalog_sets.list_order) + 1 <> COUNT(*))
        OR (SELECT COUNT(*) FROM no_intro_dat_headers WHERE snapshot_key = NEW.snapshot_key) <> 1)
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_header_declared_field_presence AS field
    LEFT JOIN no_intro_dat_header_field_positions AS position USING (snapshot_key, field_kind)
    WHERE field.snapshot_key = NEW.snapshot_key AND position.snapshot_key IS NULL
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_clrmamepro_declared_field_presence AS field
    LEFT JOIN no_intro_dat_clrmamepro_field_positions AS position USING (snapshot_key, field_kind)
    WHERE field.snapshot_key = NEW.snapshot_key AND position.snapshot_key IS NULL
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_romcenter_declared_field_presence AS field
    LEFT JOIN no_intro_dat_romcenter_field_positions AS position USING (snapshot_key, field_kind)
    WHERE field.snapshot_key = NEW.snapshot_key AND position.snapshot_key IS NULL
) OR EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_games ON catalog_sets.set_id = no_intro_dat_games.set_id
    WHERE snapshot_key = NEW.snapshot_key AND catalog_sets.source_element_kind = 'no_intro_dat_game' AND no_intro_dat_games.set_id IS NULL
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_game_declared_field_presence AS field
    JOIN catalog_sets USING (set_id) JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_game_field_positions AS position USING (set_id, field_kind)
    WHERE snapshot_key = NEW.snapshot_key AND position.set_id IS NULL
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id
    JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_rom_claims USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND asset_occurrences.claim_kind = 'no_intro_dat_rom' AND no_intro_dat_rom_claims.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_rom_declared_field_presence AS field
    JOIN asset_occurrences USING (occurrence_id) JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id
    JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_rom_field_positions AS position USING (occurrence_id, field_kind)
    WHERE snapshot_key = NEW.snapshot_key AND position.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_rom_claims AS claim
    JOIN asset_occurrences AS occurrence USING (occurrence_id)
    JOIN catalog_sets ON catalog_sets.set_id = occurrence.record_id
    JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_clrmamepro_options AS options USING (snapshot_key)
    WHERE catalog_set_groups.snapshot_key = NEW.snapshot_key
      AND (claim.evidence_scope <> CASE
             WHEN claim.header_text IS NOT NULL OR options.header_text IS NOT NULL THEN 'unknown'
             ELSE 'whole_file' END
        OR (claim.evidence_scope = 'unknown' AND occurrence.content_uuid IS NOT NULL)
        OR (occurrence.content_uuid IS NOT NULL AND (
            (claim.size_text IS NOT NULL AND claim.size IS NULL)
            OR EXISTS (SELECT 1 FROM no_intro_dat_rom_digest_fields AS field
                WHERE field.occurrence_id = claim.occurrence_id AND field.invalid_text IS NOT NULL)
        ))
        OR (occurrence.content_uuid IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM occurrence_digest_assertions AS assertion
            JOIN digest_values AS digest USING (digest_id)
            WHERE assertion.occurrence_id = occurrence.occurrence_id
              AND assertion.provenance = 'source_declared' AND assertion.scope = 'whole_file'
              AND digest.algorithm IN ('sha1', 'sha256')
        )))
) OR EXISTS (
    SELECT 1 FROM no_intro_dat_rom_expected_digest_assertions AS expected
    JOIN asset_occurrences AS occurrence USING (occurrence_id)
    JOIN catalog_sets ON catalog_sets.set_id = occurrence.record_id
    JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN occurrence_digest_assertions AS assertion ON assertion.occurrence_id = expected.occurrence_id
      AND assertion.digest_id = expected.digest_id AND assertion.scope = expected.scope
      AND assertion.provenance = 'source_declared'
    WHERE snapshot_key = NEW.snapshot_key
      AND assertion.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM occurrence_digest_assertions AS assertion
    JOIN asset_occurrences AS occurrence USING (occurrence_id)
    JOIN catalog_sets ON catalog_sets.set_id = occurrence.record_id
    JOIN catalog_set_groups USING (set_group_id)
    LEFT JOIN no_intro_dat_rom_expected_digest_assertions AS expected
      ON expected.occurrence_id = assertion.occurrence_id
      AND expected.digest_id = assertion.digest_id
      AND expected.scope = assertion.scope
    WHERE snapshot_key = NEW.snapshot_key AND occurrence.claim_kind = 'no_intro_dat_rom'
      AND assertion.provenance = 'source_declared' AND expected.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id
    JOIN catalog_set_groups USING (set_group_id)
    WHERE snapshot_key = NEW.snapshot_key AND asset_occurrences.claim_kind = 'no_intro_dat_rom'
    GROUP BY record_id HAVING MIN(occurrence_order) <> 0 OR MAX(occurrence_order) + 1 <> COUNT(*)
) OR EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN no_intro_dat_categories USING (set_id)
    WHERE snapshot_key = NEW.snapshot_key GROUP BY set_id
    HAVING MIN(category_order) <> 0 OR MAX(category_order) + 1 <> COUNT(*)
) OR EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN no_intro_dat_identifiers USING (set_id)
    WHERE snapshot_key = NEW.snapshot_key GROUP BY set_id
    HAVING MIN(identifier_order) <> 0 OR MAX(identifier_order) + 1 <> COUNT(*)
) OR EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN no_intro_dat_releases USING (set_id)
    WHERE snapshot_key = NEW.snapshot_key GROUP BY set_id
    HAVING MIN(release_order) <> 0 OR MAX(release_order) + 1 <> COUNT(*)
)
BEGIN SELECT RAISE(ABORT, 'No-Intro DAT native ownership is incomplete'); END;
