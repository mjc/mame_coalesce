-- Set rows are the stable identity boundary for catalog facts. Assign compact integer IDs
-- while preserving their human-facing composite key and all existing data.
DROP TRIGGER snapshot_sets_are_immutable_update;
DROP TRIGGER mame_machine_dependencies_are_immutable_update;
DROP TRIGGER mame_machine_dependencies_are_immutable_delete;
DROP INDEX mame_machine_dependencies_target_index;

ALTER TABLE snapshot_sets ADD COLUMN set_id INTEGER;

WITH ranked_sets AS (
    SELECT snapshot_key, set_name, row_number() OVER (ORDER BY snapshot_key, set_name) AS set_id
    FROM snapshot_sets
)
UPDATE snapshot_sets
SET set_id = (
    SELECT ranked_sets.set_id
    FROM ranked_sets
    WHERE ranked_sets.snapshot_key = snapshot_sets.snapshot_key
      AND ranked_sets.set_name = snapshot_sets.set_name
);

CREATE UNIQUE INDEX snapshot_sets_set_id_index ON snapshot_sets (set_id);

CREATE TABLE mame_machine_dependencies_compact (
    set_id INTEGER NOT NULL,
    dependency_order INTEGER NOT NULL CHECK (dependency_order >= 0),
    dependency_kind TEXT NOT NULL CHECK (dependency_kind IN ('device_ref', 'romof', 'sampleof')),
    target_name TEXT NOT NULL CHECK (length(target_name) > 0),
    reference_tag TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, dependency_order),
    FOREIGN KEY (set_id) REFERENCES snapshot_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;

INSERT INTO mame_machine_dependencies_compact (
    set_id, dependency_order, dependency_kind, target_name, reference_tag, source_line, source_column
)
SELECT sets.set_id, dependencies.dependency_order, dependencies.dependency_kind,
       dependencies.target_name, dependencies.reference_tag,
       dependencies.source_line, dependencies.source_column
FROM mame_machine_dependencies AS dependencies
JOIN snapshot_sets AS sets
  ON sets.snapshot_key = dependencies.snapshot_key AND sets.set_name = dependencies.set_name;

DROP TABLE mame_machine_dependencies;
ALTER TABLE mame_machine_dependencies_compact RENAME TO mame_machine_dependencies;

CREATE INDEX mame_machine_dependencies_target_index
    ON mame_machine_dependencies (target_name, dependency_kind, set_id);

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

CREATE TRIGGER mame_machine_dependencies_require_set_identity
BEFORE INSERT ON mame_machine_dependencies
WHEN NOT EXISTS (SELECT 1 FROM snapshot_sets WHERE set_id = NEW.set_id)
BEGIN
    SELECT RAISE(ABORT, 'MAME machine dependency requires a catalog set');
END;
