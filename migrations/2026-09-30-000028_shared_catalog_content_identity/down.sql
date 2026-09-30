DROP TRIGGER asset_requirements_insert;
DROP TRIGGER asset_requirements_immutable_update;
DROP TRIGGER asset_requirements_immutable_delete;
DROP VIEW asset_requirements;

DROP INDEX software_component_content_uuid;
DROP INDEX asset_requirement_content_uuid;

DROP TABLE software_component_content_conflicts;
DROP TABLE asset_requirement_content_conflicts;

ALTER TABLE software_components DROP COLUMN content_uuid;
ALTER TABLE asset_requirement_rows DROP COLUMN content_uuid;

CREATE VIEW asset_requirements AS
SELECT sets.snapshot_key, sets.set_name, rows.component_order, rows.asset_name, rows.role,
       rows.size, rows.crc, rows.md5, rows.sha1, rows.evidence_scope,
       rows.evidence_provenance, rows.merge_name, rows.dump_status, rows.serial, rows.date,
       rows.metadata_json, rows.source_line, rows.source_column
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

DROP INDEX catalog_content_digest_lookup;
DROP TABLE catalog_content_digest_assertions;
DROP INDEX digest_value_lookup;
DROP TABLE digest_values;
DROP TABLE catalog_contents;
