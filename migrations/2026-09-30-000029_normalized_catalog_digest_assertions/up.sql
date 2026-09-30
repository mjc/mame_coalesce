-- Make each source occurrence point to interned typed digest assertions instead
-- of duplicating digest bytes in the wide occurrence rows.
CREATE TABLE asset_requirement_digest_assertions (
    set_id INTEGER NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    digest_id INTEGER NOT NULL,
    scope TEXT NOT NULL CHECK (length(scope) > 0),
    provenance TEXT NOT NULL CHECK (
        provenance IN ('source_declared', 'computed', 'legacy_cache', 'unknown')
    ),
    PRIMARY KEY (set_id, component_order, digest_id, scope, provenance),
    FOREIGN KEY (set_id, component_order)
        REFERENCES asset_requirement_rows (set_id, component_order) ON DELETE RESTRICT,
    FOREIGN KEY (digest_id) REFERENCES digest_values (digest_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_component_digest_assertions (
    snapshot_key TEXT NOT NULL,
    list_name TEXT NOT NULL,
    item_name TEXT NOT NULL,
    part_name TEXT NOT NULL,
    area_order INTEGER NOT NULL CHECK (area_order >= 0),
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    digest_id INTEGER NOT NULL,
    scope TEXT NOT NULL CHECK (length(scope) > 0),
    provenance TEXT NOT NULL CHECK (
        provenance IN ('source_declared', 'computed', 'legacy_cache', 'unknown')
    ),
    PRIMARY KEY (
        snapshot_key, list_name, item_name, part_name,
        area_order, component_order, digest_id, scope, provenance
    ),
    FOREIGN KEY (
        snapshot_key, list_name, item_name, part_name, area_order, component_order
    ) REFERENCES software_components (
        snapshot_key, list_name, item_name, part_name, area_order, component_order
    ) ON DELETE RESTRICT,
    FOREIGN KEY (digest_id) REFERENCES digest_values (digest_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE INDEX asset_requirement_digest_lookup
    ON asset_requirement_digest_assertions (digest_id, set_id, component_order);
CREATE INDEX software_component_digest_lookup
    ON software_component_digest_assertions (
        digest_id, snapshot_key, list_name, item_name, part_name, area_order, component_order
    );

INSERT OR IGNORE INTO digest_values (algorithm, digest)
SELECT 'crc32', crc FROM asset_requirement_rows WHERE crc IS NOT NULL
UNION ALL
SELECT 'md5', md5 FROM asset_requirement_rows WHERE md5 IS NOT NULL
UNION ALL
SELECT 'sha1', sha1 FROM asset_requirement_rows WHERE sha1 IS NOT NULL
UNION ALL
SELECT 'crc32', crc FROM software_components WHERE crc IS NOT NULL
UNION ALL
SELECT 'sha1', sha1 FROM software_components WHERE sha1 IS NOT NULL;

INSERT INTO asset_requirement_digest_assertions (
    set_id, component_order, digest_id, scope, provenance
)
SELECT rows.set_id, rows.component_order, digest.digest_id,
       rows.evidence_scope, rows.evidence_provenance
FROM asset_requirement_rows AS rows
JOIN digest_values AS digest
  ON ((digest.algorithm = 'crc32' AND digest.digest = rows.crc)
   OR (digest.algorithm = 'md5' AND digest.digest = rows.md5)
   OR (digest.algorithm = 'sha1' AND digest.digest = rows.sha1));

INSERT INTO software_component_digest_assertions (
    snapshot_key, list_name, item_name, part_name, area_order, component_order,
    digest_id, scope, provenance
)
SELECT component.snapshot_key, component.list_name, component.item_name, component.part_name,
       component.area_order, component.component_order, digest.digest_id,
       component.evidence_scope, 'source_declared'
FROM software_components AS component
JOIN digest_values AS digest
  ON ((digest.algorithm = 'crc32' AND digest.digest = component.crc)
   OR (digest.algorithm = 'sha1' AND digest.digest = component.sha1));

-- Preserve the public occurrence projection while making normalized assertion
-- rows authoritative for both catalog families.
DROP TRIGGER asset_requirements_insert;
DROP TRIGGER asset_requirements_immutable_update;
DROP TRIGGER asset_requirements_immutable_delete;
DROP VIEW asset_requirements;

DROP TRIGGER asset_requirement_rows_are_immutable_update;
DROP TRIGGER asset_requirement_rows_are_immutable_delete;
ALTER TABLE asset_requirement_rows DROP COLUMN crc;
ALTER TABLE asset_requirement_rows DROP COLUMN md5;
ALTER TABLE asset_requirement_rows DROP COLUMN sha1;

CREATE VIEW asset_requirements AS
SELECT sets.snapshot_key, sets.set_name, rows.component_order, rows.asset_name, rows.role,
       rows.size,
       (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = rows.set_id AND assertion.component_order = rows.component_order
          AND digest.algorithm = 'crc32' AND assertion.scope = rows.evidence_scope) AS crc,
       (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = rows.set_id AND assertion.component_order = rows.component_order
          AND digest.algorithm = 'md5' AND assertion.scope = rows.evidence_scope) AS md5,
       (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = rows.set_id AND assertion.component_order = rows.component_order
          AND digest.algorithm = 'sha1' AND assertion.scope = rows.evidence_scope) AS sha1,
       rows.evidence_scope, rows.evidence_provenance, rows.merge_name, rows.dump_status,
       rows.serial, rows.date, rows.metadata_json, rows.source_line, rows.source_column,
       rows.content_uuid
FROM asset_requirement_rows AS rows
JOIN snapshot_sets AS sets USING (set_id);

CREATE TRIGGER asset_requirements_insert
INSTEAD OF INSERT ON asset_requirements
BEGIN
    INSERT INTO asset_requirement_rows (
        set_id, component_order, asset_name, role, size, evidence_scope,
        evidence_provenance, merge_name, dump_status, serial, date, metadata_json,
        source_line, source_column
    ) VALUES (
        (SELECT set_id FROM snapshot_sets
         WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name),
        NEW.component_order, NEW.asset_name, NEW.role, NEW.size, NEW.evidence_scope,
        NEW.evidence_provenance, NEW.merge_name, NEW.dump_status, NEW.serial, NEW.date,
        COALESCE(NEW.metadata_json, '{}'), NEW.source_line, NEW.source_column
    );

    INSERT OR IGNORE INTO digest_values (algorithm, digest)
        SELECT 'crc32', NEW.crc WHERE NEW.crc IS NOT NULL;
    INSERT OR IGNORE INTO digest_values (algorithm, digest)
        SELECT 'md5', NEW.md5 WHERE NEW.md5 IS NOT NULL;
    INSERT OR IGNORE INTO digest_values (algorithm, digest)
        SELECT 'sha1', NEW.sha1 WHERE NEW.sha1 IS NOT NULL;

    INSERT OR IGNORE INTO asset_requirement_digest_assertions (
        set_id, component_order, digest_id, scope, provenance
    )
    SELECT (SELECT set_id FROM snapshot_sets
            WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name),
           NEW.component_order, digest_id, NEW.evidence_scope, NEW.evidence_provenance
    FROM digest_values
    WHERE ((algorithm = 'crc32' AND digest = NEW.crc)
        OR (algorithm = 'md5' AND digest = NEW.md5)
        OR (algorithm = 'sha1' AND digest = NEW.sha1));
END;

CREATE TRIGGER asset_requirements_immutable_update
INSTEAD OF UPDATE ON asset_requirements
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;

CREATE TRIGGER asset_requirements_immutable_delete
INSTEAD OF DELETE ON asset_requirements
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;

CREATE TRIGGER asset_requirement_rows_are_immutable_update
BEFORE UPDATE ON asset_requirement_rows
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;

CREATE TRIGGER asset_requirement_rows_are_immutable_delete
BEFORE DELETE ON asset_requirement_rows
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;

ALTER TABLE software_components RENAME TO software_component_occurrences;
ALTER TABLE software_component_occurrences DROP COLUMN crc;
ALTER TABLE software_component_occurrences DROP COLUMN sha1;

CREATE VIEW software_components AS
SELECT occurrence.snapshot_key, occurrence.list_name, occurrence.item_name, occurrence.part_name,
       occurrence.area_order, occurrence.area_kind, occurrence.area_name,
       occurrence.component_order, occurrence.component_kind, occurrence.component_name,
       occurrence.size,
       (SELECT digest.digest FROM software_component_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.snapshot_key = occurrence.snapshot_key
          AND assertion.list_name = occurrence.list_name
          AND assertion.item_name = occurrence.item_name
          AND assertion.part_name = occurrence.part_name
          AND assertion.area_order = occurrence.area_order
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'crc32' AND assertion.scope = occurrence.evidence_scope) AS crc,
       (SELECT digest.digest FROM software_component_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.snapshot_key = occurrence.snapshot_key
          AND assertion.list_name = occurrence.list_name
          AND assertion.item_name = occurrence.item_name
          AND assertion.part_name = occurrence.part_name
          AND assertion.area_order = occurrence.area_order
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'sha1' AND assertion.scope = occurrence.evidence_scope) AS sha1,
       occurrence.offset, occurrence.value, occurrence.dump_status, occurrence.writeable,
       occurrence.load_instruction, occurrence.source_line, occurrence.source_column,
       occurrence.evidence_scope, occurrence.content_uuid
FROM software_component_occurrences AS occurrence;

CREATE TRIGGER software_components_insert
INSTEAD OF INSERT ON software_components
BEGIN
    INSERT INTO software_component_occurrences (
        snapshot_key, list_name, item_name, part_name, area_order, area_kind, area_name,
        component_order, component_kind, component_name, size, offset, value, dump_status,
        writeable, load_instruction, source_line, source_column, evidence_scope, content_uuid
    ) VALUES (
        NEW.snapshot_key, NEW.list_name, NEW.item_name, NEW.part_name, NEW.area_order,
        NEW.area_kind, NEW.area_name, NEW.component_order, NEW.component_kind, NEW.component_name,
        NEW.size, NEW.offset, NEW.value, NEW.dump_status, NEW.writeable, NEW.load_instruction,
        NEW.source_line, NEW.source_column, NEW.evidence_scope, NEW.content_uuid
    );

    INSERT OR IGNORE INTO digest_values (algorithm, digest)
        SELECT 'crc32', NEW.crc WHERE NEW.crc IS NOT NULL;
    INSERT OR IGNORE INTO digest_values (algorithm, digest)
        SELECT 'sha1', NEW.sha1 WHERE NEW.sha1 IS NOT NULL;

    INSERT OR IGNORE INTO software_component_digest_assertions (
        snapshot_key, list_name, item_name, part_name, area_order, component_order,
        digest_id, scope, provenance
    )
    SELECT NEW.snapshot_key, NEW.list_name, NEW.item_name, NEW.part_name,
           NEW.area_order, NEW.component_order, digest_id, NEW.evidence_scope, 'source_declared'
    FROM digest_values
    WHERE ((algorithm = 'crc32' AND digest = NEW.crc)
        OR (algorithm = 'sha1' AND digest = NEW.sha1));
END;

CREATE TRIGGER software_components_immutable_update
INSTEAD OF UPDATE ON software_components
BEGIN
    SELECT RAISE(ABORT, 'software components are immutable');
END;

CREATE TRIGGER software_components_immutable_delete
INSTEAD OF DELETE ON software_components
BEGIN
    SELECT RAISE(ABORT, 'software components are immutable');
END;
