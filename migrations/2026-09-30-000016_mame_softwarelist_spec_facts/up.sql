ALTER TABLE software_lists ADD COLUMN notes TEXT;

CREATE TABLE software_part_dipswitches (
    snapshot_key TEXT NOT NULL,
    list_name TEXT NOT NULL,
    item_name TEXT NOT NULL,
    part_name TEXT NOT NULL,
    dipswitch_order INTEGER NOT NULL CHECK (dipswitch_order >= 0),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    mask TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name, item_name, part_name, dipswitch_order),
    FOREIGN KEY (snapshot_key, list_name, item_name, part_name)
        REFERENCES software_parts (snapshot_key, list_name, item_name, part_name) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_part_dip_values (
    snapshot_key TEXT NOT NULL,
    list_name TEXT NOT NULL,
    item_name TEXT NOT NULL,
    part_name TEXT NOT NULL,
    dipswitch_order INTEGER NOT NULL CHECK (dipswitch_order >= 0),
    value_order INTEGER NOT NULL CHECK (value_order >= 0),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    is_default INTEGER NOT NULL CHECK (is_default IN (0, 1)),
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (
        snapshot_key, list_name, item_name, part_name, dipswitch_order, value_order
    ),
    FOREIGN KEY (
        snapshot_key, list_name, item_name, part_name, dipswitch_order
    ) REFERENCES software_part_dipswitches (
        snapshot_key, list_name, item_name, part_name, dipswitch_order
    ) ON DELETE RESTRICT
) WITHOUT ROWID;
