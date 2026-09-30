CREATE TABLE catalog_contents (
    content_uuid BLOB PRIMARY KEY NOT NULL CHECK (length(content_uuid) = 16),
    expected_size INTEGER CHECK (expected_size IS NULL OR expected_size >= 0)
) WITHOUT ROWID;

CREATE TABLE digest_values (
    digest_id INTEGER PRIMARY KEY,
    algorithm TEXT NOT NULL CHECK (algorithm IN ('crc32', 'md5', 'sha1', 'sha256')),
    digest BLOB NOT NULL,
    UNIQUE (algorithm, digest),
    CHECK (
        (algorithm = 'crc32' AND length(digest) = 4) OR
        (algorithm = 'md5' AND length(digest) = 16) OR
        (algorithm = 'sha1' AND length(digest) = 20) OR
        (algorithm = 'sha256' AND length(digest) = 32)
    )
);

CREATE TABLE catalog_content_digest_assertions (
    content_uuid BLOB NOT NULL CHECK (length(content_uuid) = 16),
    digest_id INTEGER NOT NULL,
    scope TEXT NOT NULL CHECK (length(scope) > 0),
    PRIMARY KEY (content_uuid, digest_id, scope),
    FOREIGN KEY (content_uuid) REFERENCES catalog_contents (content_uuid) ON DELETE RESTRICT,
    FOREIGN KEY (digest_id) REFERENCES digest_values (digest_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE INDEX digest_value_lookup
    ON digest_values (algorithm, digest);

CREATE INDEX catalog_content_digest_lookup
    ON catalog_content_digest_assertions (digest_id, scope, content_uuid);

ALTER TABLE asset_requirement_rows
    ADD COLUMN content_uuid BLOB
    CHECK (content_uuid IS NULL OR length(content_uuid) = 16)
    REFERENCES catalog_contents (content_uuid) ON DELETE RESTRICT;

ALTER TABLE software_components
    ADD COLUMN content_uuid BLOB
    CHECK (content_uuid IS NULL OR length(content_uuid) = 16)
    REFERENCES catalog_contents (content_uuid) ON DELETE RESTRICT;

CREATE TABLE asset_requirement_content_conflicts (
    set_id INTEGER NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    candidate_content_uuid BLOB NOT NULL CHECK (length(candidate_content_uuid) = 16),
    reason TEXT NOT NULL CHECK (reason IN ('ambiguous_alias', 'contradictory_assertions')),
    PRIMARY KEY (set_id, component_order, candidate_content_uuid),
    FOREIGN KEY (set_id, component_order)
        REFERENCES asset_requirement_rows (set_id, component_order) ON DELETE RESTRICT,
    FOREIGN KEY (candidate_content_uuid)
        REFERENCES catalog_contents (content_uuid) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_component_content_conflicts (
    snapshot_key TEXT NOT NULL,
    list_name TEXT NOT NULL,
    item_name TEXT NOT NULL,
    part_name TEXT NOT NULL,
    area_order INTEGER NOT NULL CHECK (area_order >= 0),
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    candidate_content_uuid BLOB NOT NULL CHECK (length(candidate_content_uuid) = 16),
    reason TEXT NOT NULL CHECK (reason IN ('ambiguous_alias', 'contradictory_assertions')),
    PRIMARY KEY (
        snapshot_key, list_name, item_name, part_name,
        area_order, component_order, candidate_content_uuid
    ),
    FOREIGN KEY (
        snapshot_key, list_name, item_name, part_name, area_order, component_order
    ) REFERENCES software_components (
        snapshot_key, list_name, item_name, part_name, area_order, component_order
    ) ON DELETE RESTRICT,
    FOREIGN KEY (candidate_content_uuid)
        REFERENCES catalog_contents (content_uuid) ON DELETE RESTRICT
) WITHOUT ROWID;

DROP TRIGGER asset_requirements_insert;
DROP TRIGGER asset_requirements_immutable_update;
DROP TRIGGER asset_requirements_immutable_delete;
DROP VIEW asset_requirements;

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

CREATE INDEX asset_requirement_content_uuid
    ON asset_requirement_rows (content_uuid);

CREATE INDEX software_component_content_uuid
    ON software_components (content_uuid);
