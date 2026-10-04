CREATE TABLE logiqx_clrmamepro_options (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES logiqx_document_facts(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    header TEXT,
    header_was_present INTEGER NOT NULL CHECK (header_was_present IN (0, 1)),
    forcemerging TEXT NOT NULL DEFAULT 'split'
        CHECK (forcemerging IN ('split', 'none', 'full')),
    forcemerging_was_present INTEGER NOT NULL CHECK (forcemerging_was_present IN (0, 1)),
    forcenodump TEXT NOT NULL DEFAULT 'obsolete'
        CHECK (forcenodump IN ('obsolete', 'required', 'ignore')),
    forcenodump_was_present INTEGER NOT NULL CHECK (forcenodump_was_present IN (0, 1)),
    forcepacking TEXT NOT NULL DEFAULT 'zip'
        CHECK (forcepacking IN ('zip', 'unzip')),
    forcepacking_was_present INTEGER NOT NULL CHECK (forcepacking_was_present IN (0, 1)),
    CHECK (header_was_present = (header IS NOT NULL)),
    CHECK (forcemerging_was_present = 1 OR forcemerging = 'split'),
    CHECK (forcenodump_was_present = 1 OR forcenodump = 'obsolete'),
    CHECK (forcepacking_was_present = 1 OR forcepacking = 'zip')
) WITHOUT ROWID;

CREATE TABLE logiqx_romcenter_options (
    snapshot_key TEXT PRIMARY KEY NOT NULL
        REFERENCES logiqx_document_facts(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    plugin TEXT,
    plugin_was_present INTEGER NOT NULL CHECK (plugin_was_present IN (0, 1)),
    rommode TEXT NOT NULL DEFAULT 'split'
        CHECK (rommode IN ('split', 'merged', 'unmerged')),
    rommode_was_present INTEGER NOT NULL CHECK (rommode_was_present IN (0, 1)),
    biosmode TEXT NOT NULL DEFAULT 'split'
        CHECK (biosmode IN ('split', 'merged', 'unmerged')),
    biosmode_was_present INTEGER NOT NULL CHECK (biosmode_was_present IN (0, 1)),
    samplemode TEXT NOT NULL DEFAULT 'merged'
        CHECK (samplemode IN ('merged', 'unmerged')),
    samplemode_was_present INTEGER NOT NULL CHECK (samplemode_was_present IN (0, 1)),
    lockrommode TEXT NOT NULL DEFAULT 'no' CHECK (lockrommode IN ('no', 'yes')),
    lockrommode_was_present INTEGER NOT NULL CHECK (lockrommode_was_present IN (0, 1)),
    lockbiosmode TEXT NOT NULL DEFAULT 'no' CHECK (lockbiosmode IN ('no', 'yes')),
    lockbiosmode_was_present INTEGER NOT NULL CHECK (lockbiosmode_was_present IN (0, 1)),
    locksamplemode TEXT NOT NULL DEFAULT 'no' CHECK (locksamplemode IN ('no', 'yes')),
    locksamplemode_was_present INTEGER NOT NULL CHECK (locksamplemode_was_present IN (0, 1)),
    CHECK (plugin_was_present = (plugin IS NOT NULL)),
    CHECK (rommode_was_present = 1 OR rommode = 'split'),
    CHECK (biosmode_was_present = 1 OR biosmode = 'split'),
    CHECK (samplemode_was_present = 1 OR samplemode = 'merged'),
    CHECK (lockrommode_was_present = 1 OR lockrommode = 'no'),
    CHECK (lockbiosmode_was_present = 1 OR lockbiosmode = 'no'),
    CHECK (locksamplemode_was_present = 1 OR locksamplemode = 'no')
) WITHOUT ROWID;

CREATE TABLE logiqx_game_comments (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    comment_order INTEGER NOT NULL CHECK (comment_order >= 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    comment_text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, comment_order)
) WITHOUT ROWID;

CREATE TABLE logiqx_releases (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    release_order INTEGER NOT NULL CHECK (release_order >= 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    name TEXT NOT NULL,
    region TEXT NOT NULL,
    language TEXT,
    date TEXT,
    "default" TEXT NOT NULL DEFAULT 'no' CHECK ("default" IN ('yes', 'no')),
    default_was_present INTEGER NOT NULL CHECK (default_was_present IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, release_order),
    CHECK (default_was_present = 1 OR "default" = 'no')
) WITHOUT ROWID;

CREATE TABLE logiqx_bios_sets (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    bios_order INTEGER NOT NULL CHECK (bios_order >= 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    is_default TEXT NOT NULL DEFAULT 'no' CHECK (is_default IN ('yes', 'no')),
    default_was_present INTEGER NOT NULL CHECK (default_was_present IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, bios_order),
    CHECK (default_was_present = 1 OR is_default = 'no')
) WITHOUT ROWID;

CREATE TABLE logiqx_archive_references (
    set_id INTEGER NOT NULL REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    archive_order INTEGER NOT NULL CHECK (archive_order >= 0),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    archive_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, archive_order)
) WITHOUT ROWID;

CREATE TABLE logiqx_file_merges (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    claim_kind TEXT NOT NULL CHECK (claim_kind IN ('logiqx_rom','logiqx_disk')),
    merge_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT GENERATED ALWAYS AS (
        CASE claim_kind WHEN 'logiqx_rom' THEN 'logiqx_rom_merge'
                        WHEN 'logiqx_disk' THEN 'logiqx_disk_merge' END
    ) VIRTUAL,
    FOREIGN KEY (occurrence_id,claim_kind)
        REFERENCES asset_occurrences(occurrence_id,claim_kind) ON DELETE RESTRICT,
    FOREIGN KEY (relationship_id,source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id,source_reference_kind) ON DELETE RESTRICT
);

CREATE TRIGGER logiqx_clrmamepro_options_native_owner_insert
BEFORE INSERT ON logiqx_clrmamepro_options
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_snapshots
    JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key
)
BEGIN SELECT RAISE(ABORT, 'native document details require an unpublished Logiqx snapshot'); END;
CREATE TRIGGER logiqx_clrmamepro_options_native_immutable_update
BEFORE UPDATE ON logiqx_clrmamepro_options
BEGIN SELECT RAISE(ABORT, 'native document details are immutable'); END;
CREATE TRIGGER logiqx_clrmamepro_options_native_immutable_delete
BEFORE DELETE ON logiqx_clrmamepro_options
BEGIN SELECT RAISE(ABORT, 'native document details are immutable'); END;

CREATE TRIGGER logiqx_romcenter_options_native_owner_insert
BEFORE INSERT ON logiqx_romcenter_options
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_snapshots
    JOIN parser_interpretations USING (interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key
)
BEGIN SELECT RAISE(ABORT, 'native document details require an unpublished Logiqx snapshot'); END;
CREATE TRIGGER logiqx_romcenter_options_native_immutable_update
BEFORE UPDATE ON logiqx_romcenter_options
BEGIN SELECT RAISE(ABORT, 'native document details are immutable'); END;
CREATE TRIGGER logiqx_romcenter_options_native_immutable_delete
BEFORE DELETE ON logiqx_romcenter_options
BEGIN SELECT RAISE(ABORT, 'native document details are immutable'); END;

CREATE TRIGGER logiqx_game_comments_native_owner_insert
BEFORE INSERT ON logiqx_game_comments
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game' AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE set_id = NEW.set_id
)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished Logiqx game'); END;
CREATE TRIGGER logiqx_game_comments_native_immutable_update
BEFORE UPDATE ON logiqx_game_comments
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;
CREATE TRIGGER logiqx_game_comments_native_immutable_delete
BEFORE DELETE ON logiqx_game_comments
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;

CREATE TRIGGER logiqx_releases_native_owner_insert
BEFORE INSERT ON logiqx_releases
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game' AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE set_id = NEW.set_id
)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished Logiqx game'); END;
CREATE TRIGGER logiqx_releases_native_immutable_update
BEFORE UPDATE ON logiqx_releases
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;
CREATE TRIGGER logiqx_releases_native_immutable_delete
BEFORE DELETE ON logiqx_releases
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;

CREATE TRIGGER logiqx_bios_sets_native_owner_insert
BEFORE INSERT ON logiqx_bios_sets
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game' AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE set_id = NEW.set_id
)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished Logiqx game'); END;
CREATE TRIGGER logiqx_bios_sets_native_immutable_update
BEFORE UPDATE ON logiqx_bios_sets
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;
CREATE TRIGGER logiqx_bios_sets_native_immutable_delete
BEFORE DELETE ON logiqx_bios_sets
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;

CREATE TRIGGER logiqx_archive_references_native_owner_insert
BEFORE INSERT ON logiqx_archive_references
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE set_id = NEW.set_id AND source_element_kind = 'logiqx_game' AND format = 'logiqx'
) OR EXISTS (
    SELECT 1 FROM catalog_sets
    JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE set_id = NEW.set_id
)
BEGIN SELECT RAISE(ABORT, 'native details require an unpublished Logiqx game'); END;
CREATE TRIGGER logiqx_archive_references_native_immutable_update
BEFORE UPDATE ON logiqx_archive_references
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;
CREATE TRIGGER logiqx_archive_references_native_immutable_delete
BEFORE DELETE ON logiqx_archive_references
BEGIN SELECT RAISE(ABORT, 'native game details are immutable'); END;

-- Positions identify a closed native scalar field; values remain in fixed columns.
CREATE TABLE logiqx_header_text_positions (
    snapshot_key TEXT NOT NULL REFERENCES logiqx_document_facts(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 9),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, field_kind),
    UNIQUE (snapshot_key, source_order)
) WITHOUT ROWID;
CREATE TABLE logiqx_game_text_positions (
    set_id INTEGER NOT NULL REFERENCES logiqx_games(set_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (field_kind BETWEEN 0 AND 2),
    source_order INTEGER NOT NULL CHECK (source_order >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, field_kind),
    UNIQUE (set_id, source_order)
) WITHOUT ROWID;
CREATE TRIGGER logiqx_header_text_positions_native_owner_insert
BEFORE INSERT ON logiqx_header_text_positions
WHEN NOT EXISTS (
    SELECT 1 FROM logiqx_document_facts AS details
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE details.snapshot_key = NEW.snapshot_key AND format = 'logiqx'
      AND CASE NEW.field_kind
        WHEN 0 THEN details.header_name
        WHEN 1 THEN details.header_description
        WHEN 2 THEN details.header_category
        WHEN 3 THEN details.header_version
        WHEN 4 THEN details.header_date
        WHEN 5 THEN details.header_author
        WHEN 6 THEN details.header_email
        WHEN 7 THEN details.header_homepage
        WHEN 8 THEN details.header_url
        WHEN 9 THEN details.header_comment
      END IS NOT NULL
) OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'text positions require a present unpublished native header field'); END;
CREATE TRIGGER logiqx_game_text_positions_native_owner_insert
BEFORE INSERT ON logiqx_game_text_positions
WHEN NOT EXISTS (
    SELECT 1 FROM logiqx_games AS details JOIN catalog_sets USING (set_id)
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE details.set_id = NEW.set_id AND source_element_kind = 'logiqx_game' AND format = 'logiqx'
      AND CASE NEW.field_kind
        WHEN 0 THEN details.description
        WHEN 1 THEN details.year
        WHEN 2 THEN details.manufacturer
      END IS NOT NULL
) OR EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key) WHERE set_id = NEW.set_id
)
BEGIN SELECT RAISE(ABORT, 'text positions require a present unpublished native game field'); END;
CREATE TRIGGER logiqx_header_text_positions_immutable_update BEFORE UPDATE ON logiqx_header_text_positions
BEGIN SELECT RAISE(ABORT, 'native scalar positions are immutable'); END;
CREATE TRIGGER logiqx_header_text_positions_immutable_delete BEFORE DELETE ON logiqx_header_text_positions
BEGIN SELECT RAISE(ABORT, 'native scalar positions are immutable'); END;
CREATE TRIGGER logiqx_game_text_positions_immutable_update BEFORE UPDATE ON logiqx_game_text_positions
BEGIN SELECT RAISE(ABORT, 'native scalar positions are immutable'); END;
CREATE TRIGGER logiqx_game_text_positions_immutable_delete BEFORE DELETE ON logiqx_game_text_positions
BEGIN SELECT RAISE(ABORT, 'native scalar positions are immutable'); END;
CREATE TRIGGER logiqx_scalar_positions_require_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots AS snapshot
    JOIN parser_interpretations AS parser USING (interpretation_key)
    WHERE snapshot.snapshot_key = NEW.snapshot_key AND parser.format = 'logiqx'
      AND NOT EXISTS (
          SELECT 1 FROM logiqx_document_facts AS document
          WHERE document.snapshot_key = snapshot.snapshot_key
      )
) OR EXISTS (
    SELECT 1 FROM logiqx_document_facts AS details
    LEFT JOIN logiqx_header_text_positions AS position0
      ON position0.snapshot_key = details.snapshot_key AND position0.field_kind = 0
    LEFT JOIN logiqx_header_text_positions AS position1
      ON position1.snapshot_key = details.snapshot_key AND position1.field_kind = 1
    LEFT JOIN logiqx_header_text_positions AS position2
      ON position2.snapshot_key = details.snapshot_key AND position2.field_kind = 2
    LEFT JOIN logiqx_header_text_positions AS position3
      ON position3.snapshot_key = details.snapshot_key AND position3.field_kind = 3
    LEFT JOIN logiqx_header_text_positions AS position4
      ON position4.snapshot_key = details.snapshot_key AND position4.field_kind = 4
    LEFT JOIN logiqx_header_text_positions AS position5
      ON position5.snapshot_key = details.snapshot_key AND position5.field_kind = 5
    LEFT JOIN logiqx_header_text_positions AS position6
      ON position6.snapshot_key = details.snapshot_key AND position6.field_kind = 6
    LEFT JOIN logiqx_header_text_positions AS position7
      ON position7.snapshot_key = details.snapshot_key AND position7.field_kind = 7
    LEFT JOIN logiqx_header_text_positions AS position8
      ON position8.snapshot_key = details.snapshot_key AND position8.field_kind = 8
    LEFT JOIN logiqx_header_text_positions AS position9
      ON position9.snapshot_key = details.snapshot_key AND position9.field_kind = 9
    WHERE details.snapshot_key = NEW.snapshot_key AND (
        (details.header_name IS NOT NULL) <> (position0.field_kind IS NOT NULL) OR
        (details.header_description IS NOT NULL) <> (position1.field_kind IS NOT NULL) OR
        (details.header_category IS NOT NULL) <> (position2.field_kind IS NOT NULL) OR
        (details.header_version IS NOT NULL) <> (position3.field_kind IS NOT NULL) OR
        (details.header_date IS NOT NULL) <> (position4.field_kind IS NOT NULL) OR
        (details.header_author IS NOT NULL) <> (position5.field_kind IS NOT NULL) OR
        (details.header_email IS NOT NULL) <> (position6.field_kind IS NOT NULL) OR
        (details.header_homepage IS NOT NULL) <> (position7.field_kind IS NOT NULL) OR
        (details.header_url IS NOT NULL) <> (position8.field_kind IS NOT NULL) OR
        (details.header_comment IS NOT NULL) <> (position9.field_kind IS NOT NULL)
    )
) OR EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN logiqx_games AS details USING (set_id)
    LEFT JOIN logiqx_game_text_positions AS position0
      ON position0.set_id = details.set_id AND position0.field_kind = 0
    LEFT JOIN logiqx_game_text_positions AS position1
      ON position1.set_id = details.set_id AND position1.field_kind = 1
    LEFT JOIN logiqx_game_text_positions AS position2
      ON position2.set_id = details.set_id AND position2.field_kind = 2
    WHERE snapshot_key = NEW.snapshot_key AND (
        (details.description IS NOT NULL) <> (position0.field_kind IS NOT NULL) OR
        (details.year IS NOT NULL) <> (position1.field_kind IS NOT NULL) OR
        (details.manufacturer IS NOT NULL) <> (position2.field_kind IS NOT NULL)
    )
)
BEGIN SELECT RAISE(ABORT, 'native scalar values require complete position ownership'); END;

CREATE TRIGGER logiqx_device_positions_require_valid_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets
    CROSS JOIN logiqx_device_references AS reference
    WHERE groups.snapshot_key=NEW.snapshot_key AND sets.set_group_id=groups.set_group_id
      AND reference.set_id=sets.set_id AND (
        typeof(reference.reference_order)<>'integer' OR reference.reference_order<0
        OR typeof(reference.source_order)<>'integer' OR reference.source_order<0
        OR typeof(reference.source_line)<>'integer' OR reference.source_line<=0
        OR typeof(reference.source_column)<>'integer' OR reference.source_column<=0
      )
)
BEGIN SELECT RAISE(ABORT, 'Logiqx device references require valid element positions'); END;

-- Interpretation is derived, never another stored source-field projection.
CREATE VIEW logiqx_rom_declared_field_states AS
SELECT occurrence_id,
    CASE WHEN size_text IS NULL THEN 'missing' WHEN size IS NULL THEN 'uninterpreted' ELSE 'usable' END AS size_state,
    CASE WHEN crc_text IS NULL THEN 'missing'
    WHEN length(crc_text) = 8 AND instr(crc_text, char(0)) = 0
      AND crc_text NOT GLOB '*[^0-9a-fA-F]*' THEN 'usable'
    ELSE 'uninterpreted' END AS crc_state,
    CASE WHEN md5_text IS NULL THEN 'missing'
    WHEN length(md5_text) = 32 AND instr(md5_text, char(0)) = 0
      AND md5_text NOT GLOB '*[^0-9a-fA-F]*' THEN 'usable'
    ELSE 'uninterpreted' END AS md5_state,
    CASE WHEN sha1_text IS NULL THEN 'missing'
    WHEN length(sha1_text) = 40 AND instr(sha1_text, char(0)) = 0
      AND sha1_text NOT GLOB '*[^0-9a-fA-F]*' THEN 'usable'
    ELSE 'uninterpreted' END AS sha1_state
FROM logiqx_rom_claims;
CREATE VIEW logiqx_disk_declared_field_states AS
SELECT occurrence_id,
    CASE WHEN md5_text IS NULL THEN 'missing'
    WHEN length(md5_text) = 32 AND instr(md5_text, char(0)) = 0
      AND md5_text NOT GLOB '*[^0-9a-fA-F]*' THEN 'usable'
    ELSE 'uninterpreted' END AS md5_state,
    CASE WHEN sha1_text IS NULL THEN 'missing'
    WHEN length(sha1_text) = 40 AND instr(sha1_text, char(0)) = 0
      AND sha1_text NOT GLOB '*[^0-9a-fA-F]*' THEN 'usable'
    ELSE 'uninterpreted' END AS sha1_state
FROM logiqx_disk_claims;
CREATE TRIGGER logiqx_uuid_requires_interpretable_source_fields
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    JOIN logiqx_rom_claims AS rom USING (occurrence_id)
    JOIN logiqx_rom_declared_field_states AS states USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND content_uuid IS NOT NULL
      AND (states.size_state = 'uninterpreted' OR states.crc_state = 'uninterpreted'
        OR states.md5_state = 'uninterpreted' OR states.sha1_state <> 'usable'
        OR NOT EXISTS (
            SELECT 1 FROM occurrence_digest_assertions AS assertion
            JOIN digest_values AS digest USING (digest_id)
            WHERE assertion.occurrence_id = rom.occurrence_id
              AND assertion.provenance = 'source_declared'
              AND assertion.scope IN ('whole_asset', 'whole_file')
              AND digest.algorithm = 'sha1'
              AND hex(digest.digest) = upper(rom.sha1_text)
        ))
) OR EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    JOIN logiqx_disk_claims USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND content_uuid IS NOT NULL
)
BEGIN SELECT RAISE(ABORT, 'shared file UUID requires interpretable whole-file source evidence'); END;
