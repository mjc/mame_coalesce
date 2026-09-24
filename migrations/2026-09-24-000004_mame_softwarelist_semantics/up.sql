CREATE TABLE software_lists (
    snapshot_key   TEXT NOT NULL REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    list_name      TEXT NOT NULL,
    list_order     INTEGER NOT NULL CHECK (list_order >= 0),
    description    TEXT,
    source_line    INTEGER NOT NULL CHECK (source_line > 0),
    source_column  INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name),
    UNIQUE (snapshot_key, list_order)
);

CREATE TABLE software_items (
    snapshot_key       TEXT NOT NULL,
    list_name          TEXT NOT NULL,
    item_name          TEXT NOT NULL,
    item_order         INTEGER NOT NULL CHECK (item_order >= 0),
    supported          TEXT NOT NULL CHECK (supported IN ('yes', 'partial', 'no')),
    description        TEXT NOT NULL,
    year               TEXT NOT NULL,
    publisher          TEXT NOT NULL,
    notes              TEXT,
    info_json          TEXT NOT NULL CHECK (json_valid(info_json)),
    shared_features_json TEXT NOT NULL CHECK (json_valid(shared_features_json)),
    source_line        INTEGER NOT NULL CHECK (source_line > 0),
    source_column      INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name, item_name),
    UNIQUE (snapshot_key, list_name, item_order),
    FOREIGN KEY (snapshot_key, list_name)
        REFERENCES software_lists (snapshot_key, list_name) ON DELETE RESTRICT
);

CREATE TABLE software_parts (
    snapshot_key   TEXT NOT NULL,
    list_name      TEXT NOT NULL,
    item_name      TEXT NOT NULL,
    part_name      TEXT NOT NULL,
    part_order     INTEGER NOT NULL CHECK (part_order >= 0),
    interface      TEXT NOT NULL,
    features_json  TEXT NOT NULL CHECK (json_valid(features_json)),
    source_line    INTEGER NOT NULL CHECK (source_line > 0),
    source_column  INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name, item_name, part_name),
    UNIQUE (snapshot_key, list_name, item_name, part_order),
    FOREIGN KEY (snapshot_key, list_name, item_name)
        REFERENCES software_items (snapshot_key, list_name, item_name) ON DELETE RESTRICT
);

CREATE TABLE software_areas (
    snapshot_key   TEXT NOT NULL,
    list_name      TEXT NOT NULL,
    item_name      TEXT NOT NULL,
    part_name      TEXT NOT NULL,
    area_name      TEXT NOT NULL,
    area_kind      TEXT NOT NULL CHECK (area_kind IN ('data', 'disk')),
    area_order     INTEGER NOT NULL CHECK (area_order >= 0),
    declared_size  INTEGER CHECK (declared_size IS NULL OR declared_size >= 0),
    width          INTEGER CHECK (width IS NULL OR width IN (8, 16, 32, 64)),
    endianness     TEXT,
    source_line    INTEGER NOT NULL CHECK (source_line > 0),
    source_column  INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name, item_name, part_name, area_kind, area_name),
    UNIQUE (snapshot_key, list_name, item_name, part_name, area_order),
    FOREIGN KEY (snapshot_key, list_name, item_name, part_name)
        REFERENCES software_parts (snapshot_key, list_name, item_name, part_name)
        ON DELETE RESTRICT
);

CREATE TABLE software_components (
    snapshot_key      TEXT NOT NULL,
    list_name         TEXT NOT NULL,
    item_name         TEXT NOT NULL,
    part_name         TEXT NOT NULL,
    area_kind         TEXT NOT NULL,
    area_name         TEXT NOT NULL,
    component_order   INTEGER NOT NULL CHECK (component_order >= 0),
    component_kind    TEXT NOT NULL CHECK (component_kind IN ('rom', 'disk')),
    component_name    TEXT,
    size              INTEGER CHECK (size IS NULL OR size >= 0),
    crc               BLOB CHECK (crc IS NULL OR length(crc) = 4),
    sha1              BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    offset            INTEGER CHECK (offset IS NULL OR offset >= 0),
    value             TEXT,
    dump_status       TEXT CHECK (dump_status IS NULL OR dump_status IN ('good', 'baddump', 'nodump')),
    writeable         INTEGER CHECK (writeable IS NULL OR writeable IN (0, 1)),
    load_instruction  TEXT,
    source_line       INTEGER NOT NULL CHECK (source_line > 0),
    source_column     INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (
        snapshot_key, list_name, item_name, part_name,
        area_kind, area_name, component_order
    ),
    FOREIGN KEY (snapshot_key, list_name, item_name, part_name, area_kind, area_name)
        REFERENCES software_areas (
            snapshot_key, list_name, item_name, part_name, area_kind, area_name
        ) ON DELETE RESTRICT,
    CHECK (
        (component_kind = 'rom' AND writeable IS NULL)
        OR (component_kind = 'disk' AND load_instruction IS NULL AND offset IS NULL AND value IS NULL)
    )
);

CREATE TABLE software_item_dependencies (
    snapshot_key       TEXT NOT NULL,
    list_name          TEXT NOT NULL,
    item_name          TEXT NOT NULL,
    dependency_kind    TEXT NOT NULL CHECK (dependency_kind = 'clone_of'),
    target_item_name   TEXT NOT NULL,
    source_line        INTEGER NOT NULL CHECK (source_line > 0),
    source_column      INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, list_name, item_name, dependency_kind),
    -- Keep unresolved targets when importing partial snapshots.
    FOREIGN KEY (snapshot_key, list_name, item_name)
        REFERENCES software_items (snapshot_key, list_name, item_name) ON DELETE RESTRICT
);

CREATE INDEX software_items_list_index
    ON software_items (snapshot_key, list_name, item_order);
CREATE INDEX software_parts_item_index
    ON software_parts (snapshot_key, list_name, item_name, part_order);
CREATE INDEX software_areas_part_index
    ON software_areas (snapshot_key, list_name, item_name, part_name, area_order);
CREATE INDEX software_components_area_index
    ON software_components (
        snapshot_key, list_name, item_name, part_name, area_kind, area_name, component_order
    );
