CREATE TABLE mame_machine_facts (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    source_file TEXT,
    description TEXT NOT NULL,
    description_line INTEGER NOT NULL CHECK (description_line > 0),
    description_column INTEGER NOT NULL CHECK (description_column > 0),
    year TEXT,
    year_line INTEGER,
    year_column INTEGER,
    manufacturer TEXT,
    manufacturer_line INTEGER,
    manufacturer_column INTEGER,
    is_device INTEGER NOT NULL CHECK (is_device IN (0, 1)),
    runnable INTEGER NOT NULL CHECK (runnable IN (0, 1)),
    is_bios INTEGER NOT NULL CHECK (is_bios IN (0, 1)),
    is_mechanical INTEGER NOT NULL CHECK (is_mechanical IN (0, 1)),
    is_consumable INTEGER NOT NULL CHECK (is_consumable IN (0, 1)),
    attributes_line INTEGER NOT NULL CHECK (attributes_line > 0),
    attributes_column INTEGER NOT NULL CHECK (attributes_column > 0),
    CHECK ((year_line IS NULL) = (year_column IS NULL)),
    CHECK ((year IS NULL) = (year_line IS NULL)),
    CHECK ((manufacturer_line IS NULL) = (manufacturer_column IS NULL)),
    CHECK ((manufacturer IS NULL) = (manufacturer_line IS NULL)),
    PRIMARY KEY (snapshot_key, set_name),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE INDEX mame_machine_facts_source_file_index ON mame_machine_facts (source_file);

CREATE TRIGGER mame_machine_facts_are_immutable_update
BEFORE UPDATE ON mame_machine_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME machine facts are immutable');
END;
CREATE TRIGGER mame_machine_facts_are_immutable_delete
BEFORE DELETE ON mame_machine_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME machine facts are immutable');
END;
