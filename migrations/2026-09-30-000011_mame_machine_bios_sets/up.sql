CREATE TABLE machine_bios_sets (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    bios_order INTEGER NOT NULL CHECK (bios_order >= 0),
    name TEXT NOT NULL,
    description TEXT,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, bios_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
);

CREATE INDEX machine_bios_sets_name_index ON machine_bios_sets (name);

CREATE TRIGGER machine_bios_sets_are_immutable_update
BEFORE UPDATE ON machine_bios_sets
BEGIN
    SELECT RAISE(ABORT, 'machine BIOS sets are immutable');
END;
CREATE TRIGGER machine_bios_sets_are_immutable_delete
BEFORE DELETE ON machine_bios_sets
BEGIN
    SELECT RAISE(ABORT, 'machine BIOS sets are immutable');
END;
