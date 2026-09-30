DROP TRIGGER software_components_immutable_delete;
DROP TRIGGER software_components_immutable_update;
DROP TRIGGER software_components_insert;
DROP VIEW software_components;

DROP TRIGGER asset_requirements_immutable_delete;
DROP TRIGGER asset_requirements_immutable_update;
DROP TRIGGER asset_requirements_insert;
DROP VIEW asset_requirements;
DROP TRIGGER asset_requirement_rows_are_immutable_update;
DROP TRIGGER asset_requirement_rows_are_immutable_delete;

ALTER TABLE software_component_occurrences
    ADD COLUMN crc BLOB CHECK (crc IS NULL OR length(crc) = 4);
ALTER TABLE software_component_occurrences
    ADD COLUMN sha1 BLOB CHECK (sha1 IS NULL OR length(sha1) = 20);
UPDATE software_component_occurrences AS occurrence
SET crc = (
        SELECT digest.digest
        FROM software_component_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.snapshot_key = occurrence.snapshot_key
          AND assertion.list_name = occurrence.list_name
          AND assertion.item_name = occurrence.item_name
          AND assertion.part_name = occurrence.part_name
          AND assertion.area_order = occurrence.area_order
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'crc32' AND assertion.scope = occurrence.evidence_scope
    ),
    sha1 = (
        SELECT digest.digest
        FROM software_component_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.snapshot_key = occurrence.snapshot_key
          AND assertion.list_name = occurrence.list_name
          AND assertion.item_name = occurrence.item_name
          AND assertion.part_name = occurrence.part_name
          AND assertion.area_order = occurrence.area_order
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'sha1' AND assertion.scope = occurrence.evidence_scope
    );
DROP TABLE software_component_digest_assertions;
ALTER TABLE software_component_occurrences RENAME TO software_components;

ALTER TABLE asset_requirement_rows ADD COLUMN crc BLOB CHECK (crc IS NULL OR length(crc) = 4);
ALTER TABLE asset_requirement_rows ADD COLUMN md5 BLOB CHECK (md5 IS NULL OR length(md5) = 16);
ALTER TABLE asset_requirement_rows ADD COLUMN sha1 BLOB CHECK (sha1 IS NULL OR length(sha1) = 20);
UPDATE asset_requirement_rows AS occurrence
SET crc = (
        SELECT digest.digest
        FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = occurrence.set_id
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'crc32' AND assertion.scope = occurrence.evidence_scope
    ),
    md5 = (
        SELECT digest.digest
        FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = occurrence.set_id
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'md5' AND assertion.scope = occurrence.evidence_scope
    ),
    sha1 = (
        SELECT digest.digest
        FROM asset_requirement_digest_assertions AS assertion
        JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.set_id = occurrence.set_id
          AND assertion.component_order = occurrence.component_order
          AND digest.algorithm = 'sha1' AND assertion.scope = occurrence.evidence_scope
    );
DROP TABLE asset_requirement_digest_assertions;

CREATE VIEW asset_requirements AS
SELECT sets.snapshot_key, sets.set_name, rows.component_order, rows.asset_name, rows.role,
       rows.size, rows.crc, rows.md5, rows.sha1, rows.evidence_scope,
       rows.evidence_provenance, rows.merge_name, rows.dump_status, rows.serial, rows.date,
       rows.metadata_json, rows.source_line, rows.source_column, rows.content_uuid
FROM asset_requirement_rows AS rows
JOIN snapshot_sets AS sets USING (set_id);

CREATE TRIGGER asset_requirements_insert
INSTEAD OF INSERT ON asset_requirements
BEGIN
    INSERT INTO asset_requirement_rows (
        set_id, component_order, asset_name, role, size, crc, md5, sha1,
        evidence_scope, evidence_provenance, merge_name, dump_status, serial, date,
        metadata_json, source_line, source_column
    ) VALUES (
        (SELECT set_id FROM snapshot_sets
         WHERE snapshot_key = NEW.snapshot_key AND set_name = NEW.set_name),
        NEW.component_order, NEW.asset_name, NEW.role, NEW.size, NEW.crc, NEW.md5, NEW.sha1,
        NEW.evidence_scope, NEW.evidence_provenance, NEW.merge_name, NEW.dump_status,
        NEW.serial, NEW.date, COALESCE(NEW.metadata_json, '{}'), NEW.source_line, NEW.source_column
    );
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
