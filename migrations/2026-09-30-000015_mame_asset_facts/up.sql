CREATE TABLE mame_asset_facts (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    region TEXT,
    bios TEXT,
    offset INTEGER CHECK (offset IS NULL OR offset >= 0),
    optional INTEGER CHECK (optional IS NULL OR optional IN (0, 1)),
    sound_only INTEGER CHECK (sound_only IS NULL OR sound_only IN (0, 1)),
    dispose INTEGER CHECK (dispose IS NULL OR dispose IN (0, 1)),
    load_flag TEXT,
    value TEXT,
    inverted INTEGER CHECK (inverted IS NULL OR inverted IN (0, 1)),
    ovha TEXT,
    no_thread INTEGER CHECK (no_thread IS NULL OR no_thread IN (0, 1)),
    disk_index TEXT,
    writable INTEGER CHECK (writable IS NULL OR writable IN (0, 1)),
    writeable INTEGER CHECK (writeable IS NULL OR writeable IN (0, 1)),
    PRIMARY KEY (snapshot_key, set_name, component_order),
    FOREIGN KEY (snapshot_key, set_name, component_order)
        REFERENCES asset_requirements (snapshot_key, set_name, component_order) ON DELETE RESTRICT
);

CREATE TRIGGER mame_asset_facts_are_immutable_update
BEFORE UPDATE ON mame_asset_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME asset facts are immutable');
END;
CREATE TRIGGER mame_asset_facts_are_immutable_delete
BEFORE DELETE ON mame_asset_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME asset facts are immutable');
END;
