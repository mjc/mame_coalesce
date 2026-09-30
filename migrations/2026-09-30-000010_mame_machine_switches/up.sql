CREATE TABLE machine_switches (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('dipswitch', 'configuration')),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    mask INTEGER NOT NULL CHECK (mask >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, switch_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
);

CREATE TABLE machine_switch_locations (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    location_order INTEGER NOT NULL CHECK (location_order >= 0),
    name TEXT NOT NULL,
    number TEXT NOT NULL,
    inverted INTEGER NOT NULL CHECK (inverted IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, switch_order, location_order),
    FOREIGN KEY (snapshot_key, set_name, switch_order)
        REFERENCES machine_switches (snapshot_key, set_name, switch_order) ON DELETE RESTRICT
);

CREATE TABLE machine_switch_values (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    value_order INTEGER NOT NULL CHECK (value_order >= 0),
    name TEXT NOT NULL,
    value INTEGER NOT NULL CHECK (value >= 0),
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, switch_order, value_order),
    FOREIGN KEY (snapshot_key, set_name, switch_order)
        REFERENCES machine_switches (snapshot_key, set_name, switch_order) ON DELETE RESTRICT
);

CREATE INDEX machine_switches_tag_index ON machine_switches (tag, name);

CREATE TRIGGER machine_switches_are_immutable_update
BEFORE UPDATE ON machine_switches
BEGIN
    SELECT RAISE(ABORT, 'machine switches are immutable');
END;
CREATE TRIGGER machine_switches_are_immutable_delete
BEFORE DELETE ON machine_switches
BEGIN
    SELECT RAISE(ABORT, 'machine switches are immutable');
END;
CREATE TRIGGER machine_switch_locations_are_immutable_update
BEFORE UPDATE ON machine_switch_locations
BEGIN
    SELECT RAISE(ABORT, 'machine switch locations are immutable');
END;
CREATE TRIGGER machine_switch_locations_are_immutable_delete
BEFORE DELETE ON machine_switch_locations
BEGIN
    SELECT RAISE(ABORT, 'machine switch locations are immutable');
END;
CREATE TRIGGER machine_switch_values_are_immutable_update
BEFORE UPDATE ON machine_switch_values
BEGIN
    SELECT RAISE(ABORT, 'machine switch values are immutable');
END;
CREATE TRIGGER machine_switch_values_are_immutable_delete
BEFORE DELETE ON machine_switch_values
BEGIN
    SELECT RAISE(ABORT, 'machine switch values are immutable');
END;
