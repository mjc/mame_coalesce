DROP TRIGGER mame_machine_conditions_validate_owner_insert;
DROP TRIGGER mame_machine_conditions_are_immutable_update;
DROP TRIGGER mame_machine_conditions_are_immutable_delete;
DROP TRIGGER mame_machine_spec_elements_are_immutable_update;
DROP TRIGGER mame_machine_spec_elements_are_immutable_delete;
DROP TRIGGER mame_machine_input_controls_validate_owner_insert;
DROP TRIGGER mame_machine_input_controls_are_immutable_update;
DROP TRIGGER mame_machine_input_controls_are_immutable_delete;
DROP TRIGGER mame_machine_analogs_validate_owner_insert;
DROP TRIGGER mame_machine_analogs_are_immutable_update;
DROP TRIGGER mame_machine_analogs_are_immutable_delete;
DROP TRIGGER mame_machine_device_extensions_validate_owner_insert;
DROP TRIGGER mame_machine_device_extensions_are_immutable_update;
DROP TRIGGER mame_machine_device_extensions_are_immutable_delete;
DROP TRIGGER mame_machine_slot_options_validate_owner_insert;
DROP TRIGGER mame_machine_slot_options_are_immutable_update;
DROP TRIGGER mame_machine_slot_options_are_immutable_delete;
DROP INDEX mame_machine_spec_elements_type_index;

CREATE TABLE mame_machine_spec_elements_legacy (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    element_type TEXT NOT NULL CHECK (element_type IN (
        'sample', 'chip', 'display', 'sound', 'input', 'port', 'adjuster',
        'driver', 'feature', 'device', 'slot', 'softwarelist', 'ramoption'
    )),
    sample_name TEXT,
    chip_name TEXT, chip_tag TEXT,
    chip_type TEXT CHECK (chip_type IS NULL OR chip_type IN ('cpu', 'audio')),
    chip_clock TEXT,
    display_tag TEXT,
    display_type TEXT CHECK (display_type IS NULL OR display_type IN ('raster', 'vector', 'lcd', 'svg', 'unknown')),
    display_rotate TEXT CHECK (display_rotate IS NULL OR display_rotate IN ('0', '90', '180', '270')),
    flipx INTEGER NOT NULL DEFAULT 0 CHECK (flipx IN (0, 1)),
    display_width TEXT, display_height TEXT, display_refresh TEXT, display_pixclock TEXT,
    display_htotal TEXT, display_hbend TEXT, display_hbstart TEXT, display_vtotal TEXT,
    display_vbend TEXT, display_vbstart TEXT,
    sound_channels TEXT,
    input_service INTEGER NOT NULL DEFAULT 0 CHECK (input_service IN (0, 1)),
    input_tilt INTEGER NOT NULL DEFAULT 0 CHECK (input_tilt IN (0, 1)),
    input_players TEXT, input_coins TEXT, port_tag TEXT,
    adjuster_name TEXT, adjuster_default TEXT,
    driver_status TEXT CHECK (driver_status IS NULL OR driver_status IN ('good', 'imperfect', 'preliminary')),
    driver_emulation TEXT CHECK (driver_emulation IS NULL OR driver_emulation IN ('good', 'imperfect', 'preliminary')),
    driver_cocktail TEXT CHECK (driver_cocktail IS NULL OR driver_cocktail IN ('good', 'imperfect', 'preliminary')),
    driver_savestate TEXT CHECK (driver_savestate IS NULL OR driver_savestate IN ('supported', 'unsupported')),
    driver_requiresartwork INTEGER NOT NULL DEFAULT 0 CHECK (driver_requiresartwork IN (0, 1)),
    driver_unofficial INTEGER NOT NULL DEFAULT 0 CHECK (driver_unofficial IN (0, 1)),
    driver_nosoundhardware INTEGER NOT NULL DEFAULT 0 CHECK (driver_nosoundhardware IN (0, 1)),
    driver_incomplete INTEGER NOT NULL DEFAULT 0 CHECK (driver_incomplete IN (0, 1)),
    feature_type TEXT CHECK (feature_type IS NULL OR feature_type IN (
        'protection', 'timing', 'graphics', 'palette', 'sound', 'capture', 'camera',
        'microphone', 'controls', 'keyboard', 'mouse', 'media', 'disk', 'printer',
        'tape', 'punch', 'drum', 'rom', 'comms', 'lan', 'wan'
    )),
    feature_status TEXT CHECK (feature_status IS NULL OR feature_status IN ('unemulated', 'imperfect')),
    feature_overall TEXT CHECK (feature_overall IS NULL OR feature_overall IN ('unemulated', 'imperfect')),
    device_type TEXT, device_tag TEXT, device_fixed_image TEXT, device_mandatory TEXT,
    device_interface TEXT, device_instance_name TEXT, device_instance_briefname TEXT,
    device_instance_line INTEGER, device_instance_column INTEGER, slot_name TEXT,
    softwarelist_tag TEXT, softwarelist_name TEXT,
    softwarelist_status TEXT CHECK (softwarelist_status IS NULL OR softwarelist_status IN ('original', 'compatible')),
    softwarelist_filter TEXT, ramoption_name TEXT, ramoption_default TEXT, ramoption_text TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, element_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT,
    CHECK (
        (element_type = 'sample' AND sample_name IS NOT NULL)
        OR (element_type = 'chip' AND chip_name IS NOT NULL AND chip_type IS NOT NULL)
        OR (element_type = 'display' AND display_type IS NOT NULL AND display_refresh IS NOT NULL)
        OR (element_type = 'sound' AND sound_channels IS NOT NULL)
        OR (element_type = 'input' AND input_players IS NOT NULL)
        OR (element_type = 'port' AND port_tag IS NOT NULL)
        OR (element_type = 'adjuster' AND adjuster_name IS NOT NULL AND adjuster_default IS NOT NULL)
        OR (element_type = 'driver' AND driver_status IS NOT NULL AND driver_emulation IS NOT NULL AND driver_savestate IS NOT NULL)
        OR (element_type = 'feature' AND feature_type IS NOT NULL)
        OR (element_type = 'device' AND device_type IS NOT NULL)
        OR (element_type = 'slot' AND slot_name IS NOT NULL)
        OR (element_type = 'softwarelist' AND softwarelist_tag IS NOT NULL AND softwarelist_name IS NOT NULL AND softwarelist_status IS NOT NULL)
        OR (element_type = 'ramoption' AND ramoption_name IS NOT NULL)
    ),
    CHECK ((device_instance_name IS NULL) = (device_instance_briefname IS NULL)),
    CHECK ((device_instance_name IS NULL) = (device_instance_line IS NULL)),
    CHECK ((device_instance_name IS NULL) = (device_instance_column IS NULL)),
    CHECK (device_instance_line IS NULL OR device_instance_line > 0),
    CHECK (device_instance_column IS NULL OR device_instance_column > 0),
    CHECK (element_type = 'sample' OR sample_name IS NULL),
    CHECK (element_type = 'chip' OR (chip_name IS NULL AND chip_tag IS NULL AND chip_type IS NULL AND chip_clock IS NULL)),
    CHECK (element_type = 'display' OR (
        display_tag IS NULL AND display_type IS NULL AND display_rotate IS NULL AND flipx = 0
        AND display_width IS NULL AND display_height IS NULL AND display_refresh IS NULL
        AND display_pixclock IS NULL AND display_htotal IS NULL AND display_hbend IS NULL
        AND display_hbstart IS NULL AND display_vtotal IS NULL AND display_vbend IS NULL AND display_vbstart IS NULL
    )),
    CHECK (element_type = 'sound' OR sound_channels IS NULL),
    CHECK (element_type = 'input' OR (input_service = 0 AND input_tilt = 0 AND input_players IS NULL AND input_coins IS NULL)),
    CHECK (element_type = 'port' OR port_tag IS NULL),
    CHECK (element_type = 'adjuster' OR (adjuster_name IS NULL AND adjuster_default IS NULL)),
    CHECK (element_type = 'driver' OR (
        driver_status IS NULL AND driver_emulation IS NULL AND driver_cocktail IS NULL AND driver_savestate IS NULL
        AND driver_requiresartwork = 0 AND driver_unofficial = 0 AND driver_nosoundhardware = 0 AND driver_incomplete = 0
    )),
    CHECK (element_type = 'feature' OR (feature_type IS NULL AND feature_status IS NULL AND feature_overall IS NULL)),
    CHECK (element_type = 'device' OR (
        device_type IS NULL AND device_tag IS NULL AND device_fixed_image IS NULL AND device_mandatory IS NULL
        AND device_interface IS NULL AND device_instance_name IS NULL AND device_instance_briefname IS NULL
        AND device_instance_line IS NULL AND device_instance_column IS NULL
    )),
    CHECK (element_type = 'slot' OR slot_name IS NULL),
    CHECK (element_type = 'softwarelist' OR (softwarelist_tag IS NULL AND softwarelist_name IS NULL AND softwarelist_status IS NULL AND softwarelist_filter IS NULL)),
    CHECK (element_type = 'ramoption' OR (ramoption_name IS NULL AND ramoption_default IS NULL AND ramoption_text IS NULL))
) WITHOUT ROWID;

CREATE TABLE mame_machine_input_controls_legacy (
    snapshot_key TEXT NOT NULL, set_name TEXT NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    control_order INTEGER NOT NULL CHECK (control_order >= 0),
    control_type TEXT NOT NULL, player TEXT, buttons TEXT, minimum TEXT, maximum TEXT,
    sensitivity TEXT, keydelta TEXT,
    reverse INTEGER NOT NULL DEFAULT 0 CHECK (reverse IN (0, 1)),
    ways TEXT, ways2 TEXT, ways3 TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, element_order, control_order),
    FOREIGN KEY (snapshot_key, set_name, element_order)
        REFERENCES mame_machine_spec_elements_legacy (snapshot_key, set_name, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_analogs_legacy (
    snapshot_key TEXT NOT NULL, set_name TEXT NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    analog_order INTEGER NOT NULL CHECK (analog_order >= 0), mask TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, element_order, analog_order),
    FOREIGN KEY (snapshot_key, set_name, element_order)
        REFERENCES mame_machine_spec_elements_legacy (snapshot_key, set_name, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_device_extensions_legacy (
    snapshot_key TEXT NOT NULL, set_name TEXT NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    extension_order INTEGER NOT NULL CHECK (extension_order >= 0), name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, element_order, extension_order),
    FOREIGN KEY (snapshot_key, set_name, element_order)
        REFERENCES mame_machine_spec_elements_legacy (snapshot_key, set_name, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_slot_options_legacy (
    snapshot_key TEXT NOT NULL, set_name TEXT NOT NULL,
    element_order INTEGER NOT NULL CHECK (element_order >= 0),
    option_order INTEGER NOT NULL CHECK (option_order >= 0),
    name TEXT NOT NULL, devname TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, element_order, option_order),
    FOREIGN KEY (snapshot_key, set_name, element_order)
        REFERENCES mame_machine_spec_elements_legacy (snapshot_key, set_name, element_order) ON DELETE RESTRICT
) WITHOUT ROWID;

INSERT INTO mame_machine_spec_elements_legacy
SELECT sets.snapshot_key, sets.set_name, elements.element_order, elements.element_type,
       elements.sample_name, elements.chip_name, elements.chip_tag, elements.chip_type,
       elements.chip_clock, elements.display_tag, elements.display_type, elements.display_rotate,
       elements.flipx, elements.display_width, elements.display_height, elements.display_refresh,
       elements.display_pixclock, elements.display_htotal, elements.display_hbend,
       elements.display_hbstart, elements.display_vtotal, elements.display_vbend,
       elements.display_vbstart, elements.sound_channels, elements.input_service,
       elements.input_tilt, elements.input_players, elements.input_coins, elements.port_tag,
       elements.adjuster_name, elements.adjuster_default, elements.driver_status,
       elements.driver_emulation, elements.driver_cocktail, elements.driver_savestate,
       elements.driver_requiresartwork, elements.driver_unofficial, elements.driver_nosoundhardware,
       elements.driver_incomplete, elements.feature_type, elements.feature_status,
       elements.feature_overall, elements.device_type, elements.device_tag,
       elements.device_fixed_image, elements.device_mandatory, elements.device_interface,
       elements.device_instance_name, elements.device_instance_briefname,
       elements.device_instance_line, elements.device_instance_column, elements.slot_name,
       elements.softwarelist_tag, elements.softwarelist_name, elements.softwarelist_status,
       elements.softwarelist_filter, elements.ramoption_name, elements.ramoption_default,
       elements.ramoption_text, elements.source_line, elements.source_column
FROM mame_machine_spec_elements AS elements
JOIN snapshot_sets AS sets USING (set_id);
INSERT INTO mame_machine_input_controls_legacy
SELECT sets.snapshot_key, sets.set_name, facts.element_order, facts.control_order,
       facts.control_type, facts.player, facts.buttons, facts.minimum, facts.maximum,
       facts.sensitivity, facts.keydelta, facts.reverse, facts.ways, facts.ways2,
       facts.ways3, facts.source_line, facts.source_column
FROM mame_machine_input_controls AS facts JOIN snapshot_sets AS sets USING (set_id);
INSERT INTO mame_machine_analogs_legacy
SELECT sets.snapshot_key, sets.set_name, facts.element_order, facts.analog_order,
       facts.mask, facts.source_line, facts.source_column
FROM mame_machine_analogs AS facts JOIN snapshot_sets AS sets USING (set_id);
INSERT INTO mame_machine_device_extensions_legacy
SELECT sets.snapshot_key, sets.set_name, facts.element_order, facts.extension_order,
       facts.name, facts.source_line, facts.source_column
FROM mame_machine_device_extensions AS facts JOIN snapshot_sets AS sets USING (set_id);
INSERT INTO mame_machine_slot_options_legacy
SELECT sets.snapshot_key, sets.set_name, facts.element_order, facts.option_order,
       facts.name, facts.devname, facts.is_default, facts.source_line, facts.source_column
FROM mame_machine_slot_options AS facts JOIN snapshot_sets AS sets USING (set_id);

DROP TABLE mame_machine_input_controls;
DROP TABLE mame_machine_analogs;
DROP TABLE mame_machine_device_extensions;
DROP TABLE mame_machine_slot_options;
DROP TABLE mame_machine_spec_elements;
ALTER TABLE mame_machine_spec_elements_legacy RENAME TO mame_machine_spec_elements;
ALTER TABLE mame_machine_input_controls_legacy RENAME TO mame_machine_input_controls;
ALTER TABLE mame_machine_analogs_legacy RENAME TO mame_machine_analogs;
ALTER TABLE mame_machine_device_extensions_legacy RENAME TO mame_machine_device_extensions;
ALTER TABLE mame_machine_slot_options_legacy RENAME TO mame_machine_slot_options;

CREATE INDEX mame_machine_spec_elements_type_index
    ON mame_machine_spec_elements (snapshot_key, element_type, set_name);

CREATE TRIGGER mame_machine_conditions_validate_owner_insert
BEFORE INSERT ON mame_machine_conditions
WHEN NOT (
    (NEW.owner_kind = 'element' AND EXISTS (
        SELECT 1 FROM mame_machine_spec_elements AS elements
        JOIN snapshot_sets AS sets USING (snapshot_key, set_name)
        WHERE sets.set_id = NEW.set_id
          AND elements.element_order = NEW.owner_element_order
          AND elements.element_type = 'adjuster'
    ))
    OR (NEW.owner_kind = 'switch' AND EXISTS (
        SELECT 1 FROM machine_switches WHERE set_id = NEW.set_id AND switch_order = NEW.owner_switch_order
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

CREATE TRIGGER mame_machine_input_controls_validate_owner_insert
BEFORE INSERT ON mame_machine_input_controls
WHEN NOT EXISTS (
    SELECT 1 FROM mame_machine_spec_elements
    WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name
      AND element_order = NEW.element_order AND element_type = 'input'
)
BEGIN
    SELECT RAISE(ABORT, 'MAME control must belong to an input element');
END;
CREATE TRIGGER mame_machine_analogs_validate_owner_insert
BEFORE INSERT ON mame_machine_analogs
WHEN NOT EXISTS (
    SELECT 1 FROM mame_machine_spec_elements
    WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name
      AND element_order = NEW.element_order AND element_type = 'port'
)
BEGIN
    SELECT RAISE(ABORT, 'MAME analog must belong to a port element');
END;
CREATE TRIGGER mame_machine_device_extensions_validate_owner_insert
BEFORE INSERT ON mame_machine_device_extensions
WHEN NOT EXISTS (
    SELECT 1 FROM mame_machine_spec_elements
    WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name
      AND element_order = NEW.element_order AND element_type = 'device'
)
BEGIN
    SELECT RAISE(ABORT, 'MAME device extension must belong to a device element');
END;
CREATE TRIGGER mame_machine_slot_options_validate_owner_insert
BEFORE INSERT ON mame_machine_slot_options
WHEN NOT EXISTS (
    SELECT 1 FROM mame_machine_spec_elements
    WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name
      AND element_order = NEW.element_order AND element_type = 'slot'
)
BEGIN
    SELECT RAISE(ABORT, 'MAME slot option must belong to a slot element');
END;

CREATE TRIGGER mame_machine_spec_elements_are_immutable_update
BEFORE UPDATE ON mame_machine_spec_elements
BEGIN SELECT RAISE(ABORT, 'MAME machine specification elements are immutable'); END;
CREATE TRIGGER mame_machine_spec_elements_are_immutable_delete
BEFORE DELETE ON mame_machine_spec_elements
BEGIN SELECT RAISE(ABORT, 'MAME machine specification elements are immutable'); END;
CREATE TRIGGER mame_machine_input_controls_are_immutable_update
BEFORE UPDATE ON mame_machine_input_controls
BEGIN SELECT RAISE(ABORT, 'MAME machine input controls are immutable'); END;
CREATE TRIGGER mame_machine_input_controls_are_immutable_delete
BEFORE DELETE ON mame_machine_input_controls
BEGIN SELECT RAISE(ABORT, 'MAME machine input controls are immutable'); END;
CREATE TRIGGER mame_machine_analogs_are_immutable_update
BEFORE UPDATE ON mame_machine_analogs
BEGIN SELECT RAISE(ABORT, 'MAME machine analog facts are immutable'); END;
CREATE TRIGGER mame_machine_analogs_are_immutable_delete
BEFORE DELETE ON mame_machine_analogs
BEGIN SELECT RAISE(ABORT, 'MAME machine analog facts are immutable'); END;
CREATE TRIGGER mame_machine_device_extensions_are_immutable_update
BEFORE UPDATE ON mame_machine_device_extensions
BEGIN SELECT RAISE(ABORT, 'MAME machine device extensions are immutable'); END;
CREATE TRIGGER mame_machine_device_extensions_are_immutable_delete
BEFORE DELETE ON mame_machine_device_extensions
BEGIN SELECT RAISE(ABORT, 'MAME machine device extensions are immutable'); END;
CREATE TRIGGER mame_machine_slot_options_are_immutable_update
BEFORE UPDATE ON mame_machine_slot_options
BEGIN SELECT RAISE(ABORT, 'MAME machine slot options are immutable'); END;
CREATE TRIGGER mame_machine_slot_options_are_immutable_delete
BEFORE DELETE ON mame_machine_slot_options
BEGIN SELECT RAISE(ABORT, 'MAME machine slot options are immutable'); END;
