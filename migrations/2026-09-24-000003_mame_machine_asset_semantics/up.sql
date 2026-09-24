DROP TRIGGER asset_requirements_are_immutable_update;
DROP TRIGGER asset_requirements_are_immutable_delete;
DROP INDEX requirements_asset_name_index;

CREATE TABLE asset_requirements_expanded (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    asset_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('rom', 'disk')),
    size INTEGER CHECK (size IS NULL OR size >= 0),
    crc BLOB CHECK (crc IS NULL OR length(crc) = 4),
    md5 BLOB CHECK (md5 IS NULL OR length(md5) = 16),
    sha1 BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    evidence_scope TEXT NOT NULL CHECK (evidence_scope IN ('whole_asset', 'disk_data', 'track', 'unknown')),
    evidence_provenance TEXT NOT NULL CHECK (evidence_provenance IN ('source_declared', 'computed', 'legacy_cache', 'unknown')),
    merge_name TEXT,
    dump_status TEXT,
    serial TEXT,
    date TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, component_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
);

INSERT INTO asset_requirements_expanded (
    snapshot_key, set_name, component_order, asset_name, role, size, crc, md5, sha1,
    evidence_scope, evidence_provenance, merge_name, dump_status, serial, date,
    source_line, source_column
)
SELECT
    snapshot_key, set_name, component_order, asset_name, role, size, crc, md5, sha1,
    evidence_scope, evidence_provenance, merge_name, dump_status, serial, date,
    source_line, source_column
FROM asset_requirements;

DROP TABLE asset_requirements;
ALTER TABLE asset_requirements_expanded RENAME TO asset_requirements;
CREATE INDEX requirements_asset_name_index ON asset_requirements (asset_name);

CREATE TRIGGER asset_requirements_are_immutable_update
BEFORE UPDATE ON asset_requirements
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;
CREATE TRIGGER asset_requirements_are_immutable_delete
BEFORE DELETE ON asset_requirements
BEGIN
    SELECT RAISE(ABORT, 'asset requirements are immutable');
END;
