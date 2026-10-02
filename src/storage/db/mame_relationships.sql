CREATE TABLE mame_machine_links (
    set_id INTEGER NOT NULL REFERENCES mame_machines(set_id) ON DELETE RESTRICT,
    link_kind TEXT NOT NULL CHECK (link_kind IN ('cloneof', 'romof', 'sampleof')),
    target_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT GENERATED ALWAYS AS ('mame_' || link_kind) VIRTUAL,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (set_id, link_kind),
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE mame_device_references (
    set_id INTEGER NOT NULL REFERENCES mame_machines(set_id) ON DELETE RESTRICT,
    reference_order INTEGER NOT NULL
        CHECK (typeof(reference_order) = 'integer' AND reference_order >= 0),
    name TEXT NOT NULL,
    tag TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT NOT NULL DEFAULT 'mame_device_ref'
        CHECK (source_reference_kind = 'mame_device_ref'),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (set_id, reference_order),
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE mame_rom_merges (
    occurrence_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES mame_rom_claims(occurrence_id) ON DELETE RESTRICT,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT NOT NULL DEFAULT 'mame_rom_merge'
        CHECK (source_reference_kind = 'mame_rom_merge'),
    merge_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
);

CREATE TABLE mame_disk_merges (
    occurrence_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES mame_disk_claims(occurrence_id) ON DELETE RESTRICT,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT NOT NULL DEFAULT 'mame_disk_merge'
        CHECK (source_reference_kind = 'mame_disk_merge'),
    merge_name TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    FOREIGN KEY (relationship_id, source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id, source_reference_kind)
        ON DELETE RESTRICT
);

CREATE TRIGGER mame_machine_links_insert_guard
BEFORE INSERT ON mame_machine_links
WHEN EXISTS (
        SELECT 1 FROM mame_machine_links
        WHERE (set_id, link_kind) = (NEW.set_id, NEW.link_kind)
           OR relationship_id = NEW.relationship_id
    )
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id = NEW.relationship_id)
 OR NOT EXISTS (
        SELECT 1
        FROM mame_machines AS machine
        JOIN catalog_sets AS catalog_set ON catalog_set.set_id = machine.set_id
            AND catalog_set.source_element_kind = 'mame_machine'
        JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id
        JOIN catalog_relationships AS identity ON identity.relationship_id = NEW.relationship_id
            AND identity.origin = 'source'
            AND identity.snapshot_key = group_row.snapshot_key
        JOIN reported_catalog_relationships AS reported
            ON reported.relationship_id = identity.relationship_id
            AND reported.source_reference_kind = 'mame_' || NEW.link_kind
        WHERE machine.set_id = NEW.set_id
          AND NOT EXISTS (
              SELECT 1 FROM snapshot_publications AS publication
              WHERE publication.snapshot_key = group_row.snapshot_key
          )
    )
BEGIN
    SELECT RAISE(ABORT, 'MAME link requires an unused source identity and unpublished machine owner in the same snapshot');
END;

CREATE TRIGGER mame_machine_links_immutable_update
BEFORE UPDATE ON mame_machine_links
BEGIN SELECT RAISE(ABORT, 'MAME machine links are immutable'); END;

CREATE TRIGGER mame_machine_links_immutable_delete
BEFORE DELETE ON mame_machine_links
BEGIN SELECT RAISE(ABORT, 'MAME machine links are immutable'); END;

CREATE TRIGGER mame_device_references_insert_guard
BEFORE INSERT ON mame_device_references
WHEN EXISTS (
        SELECT 1 FROM mame_device_references
        WHERE (set_id, reference_order) = (NEW.set_id, NEW.reference_order)
           OR relationship_id = NEW.relationship_id
    )
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id = NEW.relationship_id)
 OR NOT EXISTS (
        SELECT 1
        FROM mame_machines AS machine
        JOIN catalog_sets AS catalog_set ON catalog_set.set_id = machine.set_id
            AND catalog_set.source_element_kind = 'mame_machine'
        JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id
        JOIN catalog_relationships AS identity ON identity.relationship_id = NEW.relationship_id
            AND identity.origin = 'source'
            AND identity.snapshot_key = group_row.snapshot_key
        JOIN reported_catalog_relationships AS reported
            ON reported.relationship_id = identity.relationship_id
            AND reported.source_reference_kind = 'mame_device_ref'
        WHERE machine.set_id = NEW.set_id
          AND NOT EXISTS (
              SELECT 1 FROM snapshot_publications AS publication
              WHERE publication.snapshot_key = group_row.snapshot_key
          )
    )
BEGIN
    SELECT RAISE(ABORT, 'MAME device reference requires an unused source identity and unpublished machine owner in the same snapshot');
END;

CREATE TRIGGER mame_device_references_immutable_update
BEFORE UPDATE ON mame_device_references
BEGIN SELECT RAISE(ABORT, 'MAME device references are immutable'); END;

CREATE TRIGGER mame_device_references_immutable_delete
BEFORE DELETE ON mame_device_references
BEGIN SELECT RAISE(ABORT, 'MAME device references are immutable'); END;

CREATE TRIGGER mame_device_reference_positions_publication_guard
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups AS group_row
    CROSS JOIN catalog_sets AS catalog_set
    CROSS JOIN mame_device_references AS reference
    WHERE group_row.snapshot_key = NEW.snapshot_key
      AND catalog_set.set_group_id = group_row.set_group_id
      AND reference.set_id = catalog_set.set_id
    GROUP BY reference.set_id
    HAVING MAX(reference.reference_order) <> COUNT(*) - 1
)
BEGIN
    SELECT RAISE(ABORT, 'MAME device-reference positions must be dense');
END;
