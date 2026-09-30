DROP TRIGGER asset_requirements_insert;
DROP TRIGGER asset_requirements_immutable_update;
DROP TRIGGER asset_requirements_immutable_delete;
DROP TRIGGER asset_requirement_rows_are_immutable_update;
DROP TRIGGER asset_requirement_rows_are_immutable_delete;
DROP VIEW mame_asset_facts;
DROP VIEW asset_requirements;

CREATE TABLE asset_requirements (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    asset_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('rom', 'disk')),
    size INTEGER CHECK (size IS NULL OR size >= 0),
    crc BLOB CHECK (crc IS NULL OR length(crc) = 4),
    md5 BLOB CHECK (md5 IS NULL OR length(md5) = 16),
    sha1 BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    evidence_scope TEXT NOT NULL CHECK (evidence_scope IN ('whole_asset', 'disk_data', 'chd_header_sha1', 'track', 'unknown')),
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
) WITHOUT ROWID;

INSERT INTO asset_requirements (
    snapshot_key, set_name, component_order, asset_name, role, size, crc, md5, sha1,
    evidence_scope, evidence_provenance, merge_name, dump_status, serial, date,
    metadata_json, source_line, source_column
)
SELECT sets.snapshot_key, sets.set_name, rows.component_order, rows.asset_name, rows.role,
       rows.size, rows.crc, rows.md5, rows.sha1, rows.evidence_scope, rows.evidence_provenance,
       rows.merge_name, rows.dump_status, rows.serial, rows.date, rows.metadata_json,
       rows.source_line, rows.source_column
FROM asset_requirement_rows AS rows
JOIN snapshot_sets AS sets USING (set_id);

CREATE TABLE mame_asset_facts (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    region TEXT,
    bios TEXT,
    offset INTEGER CHECK (offset IS NULL OR offset >= 0),
    optional INTEGER CHECK (optional IS NULL OR optional IN (0, 1)),
    sound_only INTEGER CHECK (sound_only IS NULL OR sound_only IN (0, 1)),
    dispose INTEGER CHECK (dispose IS NULL OR dispose IN (0, 1)),
    load_flag TEXT,
    value TEXT,
    inverted INTEGER CHECK (inverted IS NULL OR inverted IN (0, 1)),
    ovha TEXT,
    no_thread INTEGER CHECK (no_thread IS NULL OR no_thread IN (0, 1)),
    disk_index TEXT,
    writable INTEGER CHECK (writable IS NULL OR writable IN (0, 1)),
    writeable INTEGER CHECK (writeable IS NULL OR writeable IN (0, 1)),
    PRIMARY KEY (snapshot_key, set_name, component_order),
    FOREIGN KEY (snapshot_key, set_name, component_order)
        REFERENCES asset_requirements (snapshot_key, set_name, component_order) ON DELETE RESTRICT
) WITHOUT ROWID;

INSERT INTO mame_asset_facts (
    snapshot_key, set_name, component_order, region, bios, offset, optional, sound_only,
    dispose, load_flag, value, inverted, ovha, no_thread, disk_index, writable, writeable
)
SELECT sets.snapshot_key, sets.set_name, rows.component_order, rows.region, rows.bios,
       rows.offset, rows.optional, rows.sound_only, rows.dispose, rows.load_flag, rows.value,
       rows.inverted, rows.ovha, rows.no_thread, rows.disk_index, rows.writable, rows.writeable
FROM asset_requirement_rows AS rows
JOIN snapshot_sets AS sets USING (set_id)
WHERE rows.optional IS NOT NULL;

DROP TABLE asset_requirement_rows;

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
CREATE TRIGGER mame_asset_facts_are_immutable_update
BEFORE UPDATE ON mame_asset_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME asset facts are immutable');
END;
CREATE TRIGGER mame_asset_facts_are_immutable_delete
BEFORE DELETE ON mame_asset_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME asset facts are immutable');
END;
