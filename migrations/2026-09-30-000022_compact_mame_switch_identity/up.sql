-- Replace repeated snapshot/set text keys in the high-cardinality switch facts with the
-- already allocated integer set identity. Keep order and source locations as query facts.
DROP TRIGGER mame_machine_conditions_are_immutable_delete;
DROP TRIGGER mame_machine_conditions_are_immutable_update;
DROP TRIGGER mame_machine_conditions_validate_owner_insert;
DROP TRIGGER machine_switch_values_are_immutable_delete;
DROP TRIGGER machine_switch_values_are_immutable_update;
DROP TRIGGER machine_switch_locations_are_immutable_delete;
DROP TRIGGER machine_switch_locations_are_immutable_update;
DROP TRIGGER machine_switches_are_immutable_delete;
DROP TRIGGER machine_switches_are_immutable_update;
DROP INDEX mame_machine_conditions_switch_value_owner_index;
DROP INDEX mame_machine_conditions_switch_owner_index;
DROP INDEX mame_machine_conditions_element_owner_index;
DROP INDEX machine_switches_tag_index;

CREATE TEMP TABLE legacy_mame_machine_conditions AS
    SELECT * FROM mame_machine_conditions;

CREATE TABLE machine_switches_compact (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('dipswitch', 'configuration')),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    mask INTEGER NOT NULL CHECK (mask >= 0),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order),
    FOREIGN KEY (set_id) REFERENCES snapshot_sets (set_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE machine_switch_locations_compact (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    location_order INTEGER NOT NULL CHECK (location_order >= 0),
    name TEXT NOT NULL,
    number TEXT NOT NULL,
    inverted INTEGER NOT NULL CHECK (inverted IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, location_order),
    FOREIGN KEY (set_id, switch_order)
        REFERENCES machine_switches_compact (set_id, switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE machine_switch_values_compact (
    set_id INTEGER NOT NULL,
    switch_order INTEGER NOT NULL CHECK (switch_order >= 0),
    value_order INTEGER NOT NULL CHECK (value_order >= 0),
    name TEXT NOT NULL,
    value INTEGER NOT NULL CHECK (value >= 0),
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (set_id, switch_order, value_order),
    FOREIGN KEY (set_id, switch_order)
        REFERENCES machine_switches_compact (set_id, switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE mame_machine_conditions_compact (
    set_id INTEGER NOT NULL,
    owner_kind TEXT NOT NULL CHECK (owner_kind IN ('element', 'switch', 'switch_value')),
    owner_element_order INTEGER,
    owner_switch_order INTEGER,
    owner_child_order INTEGER,
    condition_order INTEGER NOT NULL CHECK (condition_order >= 0),
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    relation TEXT NOT NULL CHECK (relation IN ('eq', 'ne', 'gt', 'le', 'lt', 'ge')),
    value TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (
        set_id, owner_kind, owner_element_order, owner_switch_order, owner_child_order,
        condition_order
    ),
    CHECK (
        (owner_kind = 'element' AND owner_element_order IS NOT NULL AND owner_switch_order IS NULL AND owner_child_order IS NULL)
        OR (owner_kind = 'switch' AND owner_element_order IS NULL AND owner_switch_order IS NOT NULL AND owner_child_order IS NULL)
        OR (owner_kind = 'switch_value' AND owner_element_order IS NULL AND owner_switch_order IS NOT NULL AND owner_child_order IS NOT NULL)
    ),
    FOREIGN KEY (set_id) REFERENCES snapshot_sets (set_id) ON DELETE RESTRICT,
    FOREIGN KEY (set_id, owner_switch_order)
        REFERENCES machine_switches_compact (set_id, switch_order) ON DELETE RESTRICT,
    FOREIGN KEY (set_id, owner_switch_order, owner_child_order)
        REFERENCES machine_switch_values_compact (set_id, switch_order, value_order) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX mame_machine_conditions_element_owner_index
    ON mame_machine_conditions_compact (set_id, owner_element_order, condition_order)
    WHERE owner_kind = 'element';
CREATE UNIQUE INDEX mame_machine_conditions_switch_owner_index
    ON mame_machine_conditions_compact (set_id, owner_switch_order, condition_order)
    WHERE owner_kind = 'switch';
CREATE UNIQUE INDEX mame_machine_conditions_switch_value_owner_index
    ON mame_machine_conditions_compact (set_id, owner_switch_order, owner_child_order, condition_order)
    WHERE owner_kind = 'switch_value';

INSERT INTO machine_switches_compact (
    set_id, switch_order, kind, name, tag, mask, source_line, source_column
)
SELECT sets.set_id, switches.switch_order, switches.kind, switches.name, switches.tag,
       switches.mask, switches.source_line, switches.source_column
FROM machine_switches AS switches
JOIN snapshot_sets AS sets
  ON sets.snapshot_key = switches.snapshot_key AND sets.set_name = switches.set_name;

INSERT INTO machine_switch_locations_compact (
    set_id, switch_order, location_order, name, number, inverted, source_line, source_column
)
SELECT sets.set_id, locations.switch_order, locations.location_order, locations.name,
       locations.number, locations.inverted, locations.source_line, locations.source_column
FROM machine_switch_locations AS locations
JOIN snapshot_sets AS sets
  ON sets.snapshot_key = locations.snapshot_key AND sets.set_name = locations.set_name;

INSERT INTO machine_switch_values_compact (
    set_id, switch_order, value_order, name, value, is_default, source_line, source_column
)
SELECT sets.set_id, switch_values.switch_order, switch_values.value_order, switch_values.name,
       switch_values.value, switch_values.is_default, switch_values.source_line,
       switch_values.source_column
FROM machine_switch_values AS switch_values
JOIN snapshot_sets AS sets
  ON sets.snapshot_key = switch_values.snapshot_key AND sets.set_name = switch_values.set_name;

INSERT INTO mame_machine_conditions_compact (
    set_id, owner_kind, owner_element_order, owner_switch_order, owner_child_order,
    condition_order, tag, mask, relation, value, source_line, source_column
)
SELECT sets.set_id, conditions.owner_kind, conditions.owner_element_order,
       conditions.owner_switch_order, conditions.owner_child_order,
       conditions.condition_order, conditions.tag, conditions.mask, conditions.relation,
       conditions.value, conditions.source_line, conditions.source_column
FROM legacy_mame_machine_conditions AS conditions
JOIN snapshot_sets AS sets
  ON sets.snapshot_key = conditions.snapshot_key AND sets.set_name = conditions.set_name;

DROP TABLE mame_machine_conditions;
DROP TABLE machine_switch_values;
DROP TABLE machine_switch_locations;
DROP TABLE machine_switches;
ALTER TABLE machine_switches_compact RENAME TO machine_switches;
ALTER TABLE machine_switch_locations_compact RENAME TO machine_switch_locations;
ALTER TABLE machine_switch_values_compact RENAME TO machine_switch_values;
ALTER TABLE mame_machine_conditions_compact RENAME TO mame_machine_conditions;
DROP TABLE legacy_mame_machine_conditions;

CREATE INDEX machine_switches_tag_index ON machine_switches (tag, name, set_id);

CREATE TRIGGER mame_machine_conditions_validate_owner_insert
BEFORE INSERT ON mame_machine_conditions
WHEN NOT (
    (NEW.owner_kind = 'element' AND EXISTS (
        SELECT 1 FROM mame_machine_spec_elements AS elements
        JOIN snapshot_sets AS sets
          ON sets.snapshot_key = elements.snapshot_key AND sets.set_name = elements.set_name
        WHERE sets.set_id = NEW.set_id
          AND elements.element_order = NEW.owner_element_order
          AND elements.element_type = 'adjuster'
    ))
    OR (NEW.owner_kind = 'switch' AND EXISTS (
        SELECT 1 FROM machine_switches
        WHERE set_id = NEW.set_id AND switch_order = NEW.owner_switch_order
    ))
    OR (NEW.owner_kind = 'switch_value' AND EXISTS (
        SELECT 1 FROM machine_switch_values
        WHERE set_id = NEW.set_id AND switch_order = NEW.owner_switch_order
          AND value_order = NEW.owner_child_order
    ))
)
BEGIN
    SELECT RAISE(ABORT, 'MAME condition owner does not match its DTD element');
END;

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
CREATE TRIGGER mame_machine_conditions_are_immutable_update
BEFORE UPDATE ON mame_machine_conditions
BEGIN
    SELECT RAISE(ABORT, 'MAME machine conditions are immutable');
END;
CREATE TRIGGER mame_machine_conditions_are_immutable_delete
BEFORE DELETE ON mame_machine_conditions
BEGIN
    SELECT RAISE(ABORT, 'MAME machine conditions are immutable');
END;
