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
