CREATE TABLE no_intro_game_facts (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    archive_id TEXT,
    description TEXT,
    description_line INTEGER,
    description_column INTEGER,
    PRIMARY KEY (snapshot_key, set_name),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT,
    CHECK ((description_line IS NULL) = (description_column IS NULL)),
    CHECK (description_line IS NULL OR description_line > 0),
    CHECK (description_column IS NULL OR description_column > 0)
) WITHOUT ROWID;
