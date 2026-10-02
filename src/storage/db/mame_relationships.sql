CREATE TABLE catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL CHECK (typeof(relationship_id) = 'integer'),
    assertion_key TEXT NOT NULL UNIQUE,
    origin TEXT NOT NULL CHECK (origin IN ('source', 'derived', 'user')),
    snapshot_key TEXT REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (origin <> 'source' OR snapshot_key IS NOT NULL)
);

CREATE TABLE reported_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    source_reference_kind TEXT NOT NULL CHECK (source_reference_kind IN (
        'mame_cloneof', 'mame_romof', 'mame_sampleof', 'mame_device_ref'
    )),
    UNIQUE (relationship_id, source_reference_kind)
);

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

CREATE INDEX catalog_relationships_snapshot_origin_index
    ON catalog_relationships(snapshot_key, origin, relationship_id);

CREATE TRIGGER catalog_relationships_insert_guard
BEFORE INSERT ON catalog_relationships
WHEN EXISTS (
        SELECT 1 FROM catalog_relationships
        WHERE relationship_id = NEW.relationship_id OR assertion_key = NEW.assertion_key
    )
 OR EXISTS (SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key)
 OR NEW.assertion_key GLOB 'no-intro-dat-cloneof:*'
 OR (NEW.snapshot_key IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM catalog_snapshots WHERE snapshot_key = NEW.snapshot_key
    ))
 OR (NEW.origin = 'source' AND (
        NEW.snapshot_key IS NULL
        OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
    ))
BEGIN
    SELECT RAISE(ABORT, 'catalog relationship identity is invalid, reused, or published');
END;

CREATE TRIGGER catalog_relationships_immutable_update
BEFORE UPDATE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'catalog relationship identities are immutable'); END;

CREATE TRIGGER catalog_relationships_immutable_delete
BEFORE DELETE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'catalog relationship identities are immutable'); END;

CREATE TRIGGER reported_catalog_relationships_insert_guard
BEFORE INSERT ON reported_catalog_relationships
WHEN EXISTS (
        SELECT 1 FROM reported_catalog_relationships
        WHERE relationship_id = NEW.relationship_id
    )
 OR NOT EXISTS (
        SELECT 1 FROM catalog_relationships AS identity
        WHERE identity.relationship_id = NEW.relationship_id
          AND identity.origin = 'source'
          AND identity.snapshot_key IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM snapshot_publications AS publication
              WHERE publication.snapshot_key = identity.snapshot_key
          )
    )
BEGIN
    SELECT RAISE(ABORT, 'reported relationship requires an unused unpublished source identity');
END;

CREATE TRIGGER reported_catalog_relationships_immutable_update
BEFORE UPDATE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported catalog relationships are immutable'); END;

CREATE TRIGGER reported_catalog_relationships_immutable_delete
BEFORE DELETE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported catalog relationships are immutable'); END;

CREATE TRIGGER mame_machine_links_insert_guard
BEFORE INSERT ON mame_machine_links
WHEN EXISTS (
        SELECT 1 FROM mame_machine_links
        WHERE (set_id, link_kind) = (NEW.set_id, NEW.link_kind)
           OR relationship_id = NEW.relationship_id
    )
 OR EXISTS (SELECT 1 FROM mame_device_references WHERE relationship_id = NEW.relationship_id)
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
 OR EXISTS (SELECT 1 FROM mame_machine_links WHERE relationship_id = NEW.relationship_id)
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

CREATE TRIGGER mame_relationships_publication_guard
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
 OR EXISTS (
    SELECT 1
    FROM catalog_relationships AS identity INDEXED BY catalog_relationships_snapshot_origin_index
    LEFT JOIN reported_catalog_relationships AS reported
        ON reported.relationship_id = identity.relationship_id
    WHERE identity.snapshot_key = NEW.snapshot_key
      AND identity.origin = 'source'
      AND (
          reported.relationship_id IS NULL
          OR (reported.source_reference_kind IN (
              'mame_cloneof', 'mame_romof', 'mame_sampleof'
          ) AND (
              (SELECT COUNT(*) FROM mame_machine_links AS link
               WHERE link.relationship_id = identity.relationship_id
                 AND link.source_reference_kind = reported.source_reference_kind) <> 1
              OR (SELECT COUNT(*) FROM mame_machine_links
                  WHERE relationship_id = identity.relationship_id)
                 + (SELECT COUNT(*) FROM mame_device_references
                    WHERE relationship_id = identity.relationship_id) <> 1
              OR NOT EXISTS (
                  SELECT 1 FROM mame_machine_links AS link
                  JOIN mame_machines AS machine ON machine.set_id = link.set_id
                  JOIN catalog_sets AS catalog_set ON catalog_set.set_id = link.set_id
                      AND catalog_set.source_element_kind = 'mame_machine'
                  JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id
                  WHERE link.relationship_id = identity.relationship_id
                    AND link.source_reference_kind = reported.source_reference_kind
                    AND group_row.snapshot_key = identity.snapshot_key
              )
          ))
          OR (reported.source_reference_kind = 'mame_device_ref' AND (
              (SELECT COUNT(*) FROM mame_device_references AS reference
               WHERE reference.relationship_id = identity.relationship_id
                 AND reference.source_reference_kind = reported.source_reference_kind) <> 1
              OR (SELECT COUNT(*) FROM mame_machine_links
                  WHERE relationship_id = identity.relationship_id)
                 + (SELECT COUNT(*) FROM mame_device_references
                    WHERE relationship_id = identity.relationship_id) <> 1
              OR NOT EXISTS (
                  SELECT 1 FROM mame_device_references AS reference
                  JOIN mame_machines AS machine ON machine.set_id = reference.set_id
                  JOIN catalog_sets AS catalog_set ON catalog_set.set_id = reference.set_id
                      AND catalog_set.source_element_kind = 'mame_machine'
                  JOIN catalog_set_groups AS group_row ON group_row.set_group_id = catalog_set.set_group_id
                  WHERE reference.relationship_id = identity.relationship_id
                    AND reference.source_reference_kind = reported.source_reference_kind
                    AND group_row.snapshot_key = identity.snapshot_key
              )
          ))
      )
)
 OR EXISTS (
    SELECT 1
    FROM catalog_set_groups AS group_row
    CROSS JOIN catalog_sets AS catalog_set
    CROSS JOIN mame_machines AS machine
    CROSS JOIN mame_machine_links AS link
    WHERE group_row.snapshot_key = NEW.snapshot_key
      AND catalog_set.set_group_id = group_row.set_group_id
      AND catalog_set.source_element_kind = 'mame_machine'
      AND machine.set_id = catalog_set.set_id
      AND link.set_id = machine.set_id
      AND NOT EXISTS (
          SELECT 1
          FROM catalog_relationships AS identity
          JOIN reported_catalog_relationships AS reported
              ON reported.relationship_id = identity.relationship_id
          WHERE identity.relationship_id = link.relationship_id
            AND identity.origin = 'source'
            AND identity.snapshot_key = NEW.snapshot_key
            AND reported.source_reference_kind = link.source_reference_kind
      )
)
 OR EXISTS (
    SELECT 1
    FROM catalog_set_groups AS group_row
    CROSS JOIN catalog_sets AS catalog_set
    CROSS JOIN mame_machines AS machine
    CROSS JOIN mame_device_references AS reference
    WHERE group_row.snapshot_key = NEW.snapshot_key
      AND catalog_set.set_group_id = group_row.set_group_id
      AND catalog_set.source_element_kind = 'mame_machine'
      AND machine.set_id = catalog_set.set_id
      AND reference.set_id = machine.set_id
      AND NOT EXISTS (
          SELECT 1
          FROM catalog_relationships AS identity
          JOIN reported_catalog_relationships AS reported
              ON reported.relationship_id = identity.relationship_id
          WHERE identity.relationship_id = reference.relationship_id
            AND identity.origin = 'source'
            AND identity.snapshot_key = NEW.snapshot_key
            AND reported.source_reference_kind = reference.source_reference_kind
      )
)
BEGIN
    SELECT RAISE(ABORT, 'MAME relationship identities and native owners must close within the published snapshot');
END;
