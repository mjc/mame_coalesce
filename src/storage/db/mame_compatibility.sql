-- Historical observations have actual native owners and inherit their exact
-- snapshot interpretation. They never become fields of the pinned machine DTD.
CREATE TABLE mame_machine_compatibility (
    set_id INTEGER PRIMARY KEY NOT NULL REFERENCES mame_machines(set_id) ON DELETE RESTRICT,
    is_consumable INTEGER NOT NULL CHECK (is_consumable IN (0,1)),
    is_consumable_specified INTEGER NOT NULL CHECK (is_consumable_specified=1)
) WITHOUT ROWID;
CREATE TABLE mame_rom_compatibility (
    occurrence_id INTEGER PRIMARY KEY NOT NULL REFERENCES mame_rom_claims(occurrence_id) ON DELETE RESTRICT,
    md5_text TEXT,
    sound_only INTEGER CHECK (sound_only IS NULL OR sound_only IN (0,1)),
    dispose INTEGER CHECK (dispose IS NULL OR dispose IN (0,1)),
    load_flag TEXT,
    value TEXT,
    inverted INTEGER CHECK (inverted IS NULL OR inverted IN (0,1)),
    ovha TEXT,
    no_thread INTEGER CHECK (no_thread IS NULL OR no_thread IN (0,1)),
    CHECK (md5_text IS NOT NULL OR sound_only IS NOT NULL OR dispose IS NOT NULL
        OR load_flag IS NOT NULL OR value IS NOT NULL OR inverted IS NOT NULL
        OR ovha IS NOT NULL OR no_thread IS NOT NULL)
) WITHOUT ROWID;
CREATE TABLE mame_disk_compatibility (
    occurrence_id INTEGER PRIMARY KEY NOT NULL REFERENCES mame_disk_claims(occurrence_id) ON DELETE RESTRICT,
    writeable INTEGER NOT NULL CHECK (writeable IN (0,1))
) WITHOUT ROWID;
