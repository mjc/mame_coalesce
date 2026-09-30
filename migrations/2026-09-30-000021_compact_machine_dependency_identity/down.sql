DROP TRIGGER mame_machine_dependencies_require_set_identity;
DROP TRIGGER mame_machine_dependencies_are_immutable_delete;
DROP TRIGGER mame_machine_dependencies_are_immutable_update;
DROP TRIGGER snapshot_sets_are_immutable_update;
DROP INDEX mame_machine_dependencies_target_index;

CREATE TABLE mame_machine_dependencies_legacy (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    dependency_order INTEGER NOT NULL CHECK (dependency_order >= 0),
    dependency_kind TEXT NOT NULL CHECK (dependency_kind IN ('device_ref', 'romof', 'sampleof')),
    target_name TEXT NOT NULL CHECK (length(target_name) > 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    reference_tag TEXT,
    PRIMARY KEY (snapshot_key, set_name, dependency_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES mame_machine_facts (snapshot_key, set_name) ON DELETE RESTRICT
) WITHOUT ROWID;

INSERT INTO mame_machine_dependencies_legacy (
    snapshot_key, set_name, dependency_order, dependency_kind, target_name,
    source_line, source_column, reference_tag
)
SELECT sets.snapshot_key, sets.set_name, dependencies.dependency_order,
       dependencies.dependency_kind, dependencies.target_name,
       dependencies.source_line, dependencies.source_column, dependencies.reference_tag
FROM mame_machine_dependencies AS dependencies
JOIN snapshot_sets AS sets ON sets.set_id = dependencies.set_id;

DROP TABLE mame_machine_dependencies;
ALTER TABLE mame_machine_dependencies_legacy RENAME TO mame_machine_dependencies;
CREATE INDEX mame_machine_dependencies_target_index
    ON mame_machine_dependencies (snapshot_key, target_name, dependency_kind);

CREATE TRIGGER snapshot_sets_are_immutable_update
BEFORE UPDATE ON snapshot_sets
BEGIN
    SELECT RAISE(ABORT, 'snapshot sets are immutable');
END;
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

DROP INDEX snapshot_sets_set_id_index;
ALTER TABLE snapshot_sets DROP COLUMN set_id;
