CREATE TABLE mame_machine_dependencies (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    dependency_order INTEGER NOT NULL CHECK (dependency_order >= 0),
    dependency_kind TEXT NOT NULL CHECK (dependency_kind IN ('device_ref', 'romof', 'sampleof')),
    target_name TEXT NOT NULL CHECK (length(target_name) > 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, dependency_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES mame_machine_facts (snapshot_key, set_name) ON DELETE RESTRICT
);

CREATE INDEX mame_machine_dependencies_target_index
    ON mame_machine_dependencies (snapshot_key, target_name, dependency_kind);

CREATE TRIGGER mame_machine_dependencies_are_immutable_update
BEFORE UPDATE ON mame_machine_dependencies
BEGIN
    SELECT RAISE(ABORT, 'MAME machine dependencies are immutable');
END;
CREATE TRIGGER mame_machine_dependencies_are_immutable_delete
BEFORE DELETE ON mame_machine_dependencies
BEGIN
    SELECT RAISE(ABORT, 'MAME machine dependencies are immutable');
END;
