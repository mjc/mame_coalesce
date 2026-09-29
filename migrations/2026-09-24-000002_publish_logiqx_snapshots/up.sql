-- Pre-existing identity-only snapshots may contain duplicate natural keys.
-- Preserve them and claim one published snapshot per identity separately.
CREATE TABLE snapshot_publications (
    catalog_key TEXT NOT NULL,
    document_key TEXT NOT NULL,
    interpretation_key TEXT NOT NULL,
    snapshot_key TEXT NOT NULL UNIQUE,
    published_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (catalog_key, document_key, interpretation_key),
    FOREIGN KEY (snapshot_key, catalog_key, document_key, interpretation_key)
        REFERENCES catalog_snapshots
            (snapshot_key, catalog_key, document_key, interpretation_key)
        ON DELETE RESTRICT
);

CREATE TRIGGER snapshot_publications_are_immutable_update
BEFORE UPDATE ON snapshot_publications
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;
CREATE TRIGGER snapshot_publications_are_immutable_insert
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM snapshot_publications
    WHERE (catalog_key = NEW.catalog_key
       AND document_key = NEW.document_key
       AND interpretation_key = NEW.interpretation_key)
       OR snapshot_key = NEW.snapshot_key
)
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;
CREATE TRIGGER snapshot_publications_are_immutable_delete
BEFORE DELETE ON snapshot_publications
BEGIN
    SELECT RAISE(ABORT, 'snapshot publications are immutable');
END;

CREATE TABLE snapshot_sets (
    snapshot_key TEXT NOT NULL REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    set_name TEXT NOT NULL,
    parent_name TEXT,
    metadata_json TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name)
);

CREATE TABLE asset_requirements (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    component_order INTEGER NOT NULL CHECK (component_order >= 0),
    asset_name TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('rom')),
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
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    PRIMARY KEY (snapshot_key, set_name, component_order),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
);

CREATE TABLE snapshot_extensions (
    extension_id INTEGER PRIMARY KEY NOT NULL,
    snapshot_key TEXT NOT NULL REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    record_kind TEXT NOT NULL,
    record_name TEXT,
    field_name TEXT NOT NULL,
    namespace_uri TEXT,
    raw_value_json TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0)
);

CREATE TABLE import_diagnostics (
    diagnostic_key TEXT PRIMARY KEY NOT NULL,
    run_key TEXT NOT NULL REFERENCES import_runs (run_key) ON DELETE RESTRICT,
    code TEXT NOT NULL,
    message TEXT NOT NULL,
    record_kind TEXT,
    record_name TEXT,
    field_name TEXT,
    raw_value_json TEXT,
    source_line INTEGER,
    source_column INTEGER
);

CREATE INDEX snapshot_sets_snapshot_index ON snapshot_sets (snapshot_key);
CREATE INDEX requirements_asset_name_index ON asset_requirements (asset_name);
CREATE INDEX extensions_snapshot_index ON snapshot_extensions (snapshot_key);
CREATE INDEX diagnostics_run_index ON import_diagnostics (run_key);

CREATE TRIGGER snapshot_sets_are_immutable_update
BEFORE UPDATE ON snapshot_sets
BEGIN
    SELECT RAISE(ABORT, 'snapshot sets are immutable');
END;
CREATE TRIGGER snapshot_sets_are_immutable_delete
BEFORE DELETE ON snapshot_sets
BEGIN
    SELECT RAISE(ABORT, 'snapshot sets are immutable');
END;
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
CREATE TRIGGER snapshot_extensions_are_immutable_update
BEFORE UPDATE ON snapshot_extensions
BEGIN
    SELECT RAISE(ABORT, 'snapshot extensions are immutable');
END;
CREATE TRIGGER snapshot_extensions_are_immutable_delete
BEFORE DELETE ON snapshot_extensions
BEGIN
    SELECT RAISE(ABORT, 'snapshot extensions are immutable');
END;
