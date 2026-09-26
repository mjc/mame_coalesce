CREATE TABLE publishing_sources (
    source_key  TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    locator     TEXT
);

CREATE TABLE catalogs (
    catalog_key TEXT PRIMARY KEY NOT NULL,
    source_key  TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    display_name TEXT NOT NULL
);

CREATE TABLE documents (
    document_key TEXT PRIMARY KEY NOT NULL,
    sha1         BLOB CHECK (sha1 IS NULL OR length(sha1) = 20),
    byte_length  INTEGER CHECK (byte_length IS NULL OR byte_length >= 0)
);

CREATE TABLE acquisitions (
    acquisition_key TEXT PRIMARY KEY NOT NULL,
    source_key      TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    document_key    TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    source_uri      TEXT,
    method          TEXT,
    acquired_at     DATETIME,
    UNIQUE (acquisition_key, document_key)
);

CREATE TABLE parser_interpretations (
    interpretation_key TEXT PRIMARY KEY NOT NULL,
    format              TEXT NOT NULL,
    parser_name         TEXT,
    parser_version      TEXT,
    rules_version       TEXT,
    options_json        TEXT
);

CREATE TABLE catalog_snapshots (
    snapshot_key        TEXT PRIMARY KEY NOT NULL,
    catalog_key         TEXT NOT NULL REFERENCES catalogs (catalog_key) ON DELETE RESTRICT,
    document_key        TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    interpretation_key  TEXT NOT NULL REFERENCES parser_interpretations (interpretation_key) ON DELETE RESTRICT,
    acquisition_key     TEXT REFERENCES acquisitions (acquisition_key) ON DELETE RESTRICT,
    declared_version    TEXT,
    scope_kind          TEXT NOT NULL DEFAULT 'unknown'
                                CHECK (scope_kind IN ('unknown', 'complete', 'filtered', 'partial')),
    scope_json          TEXT,
    parent_snapshot_key TEXT,
    CHECK (scope_kind NOT IN ('filtered', 'partial') OR scope_json IS NOT NULL),
    CHECK (parent_snapshot_key IS NULL OR parent_snapshot_key <> snapshot_key),
    UNIQUE (snapshot_key, catalog_key),
    UNIQUE (snapshot_key, catalog_key, document_key, interpretation_key),
    FOREIGN KEY (parent_snapshot_key, catalog_key)
        REFERENCES catalog_snapshots (snapshot_key, catalog_key)
        ON DELETE RESTRICT,
    FOREIGN KEY (acquisition_key, document_key)
        REFERENCES acquisitions (acquisition_key, document_key)
        ON DELETE RESTRICT
);

CREATE TABLE import_runs (
    run_key             TEXT PRIMARY KEY NOT NULL,
    catalog_key         TEXT NOT NULL REFERENCES catalogs (catalog_key) ON DELETE RESTRICT,
    document_key        TEXT NOT NULL REFERENCES documents (document_key) ON DELETE RESTRICT,
    interpretation_key  TEXT NOT NULL REFERENCES parser_interpretations (interpretation_key) ON DELETE RESTRICT,
    acquisition_key     TEXT REFERENCES acquisitions (acquisition_key) ON DELETE RESTRICT,
    snapshot_key        TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    status              TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'failed')),
    started_at          DATETIME,
    finished_at         DATETIME,
    diagnostic          TEXT,
    CHECK (finished_at IS NULL OR started_at IS NULL OR finished_at >= started_at),
    FOREIGN KEY (snapshot_key, catalog_key, document_key, interpretation_key)
        REFERENCES catalog_snapshots
            (snapshot_key, catalog_key, document_key, interpretation_key)
        ON DELETE RESTRICT,
    FOREIGN KEY (acquisition_key, document_key)
        REFERENCES acquisitions (acquisition_key, document_key)
        ON DELETE RESTRICT
);

CREATE INDEX catalogs_source_key_index ON catalogs (source_key);
CREATE INDEX documents_sha1_index ON documents (sha1);
CREATE INDEX acquisitions_source_key_index ON acquisitions (source_key);
CREATE INDEX acquisitions_document_key_index ON acquisitions (document_key);
CREATE INDEX snapshots_catalog_key_index ON catalog_snapshots (catalog_key);
CREATE INDEX snapshots_document_key_index ON catalog_snapshots (document_key);
CREATE INDEX snapshots_interpretation_key_index ON catalog_snapshots (interpretation_key);
CREATE INDEX snapshots_parent_key_index ON catalog_snapshots (parent_snapshot_key);
CREATE INDEX import_runs_catalog_key_index ON import_runs (catalog_key);
CREATE INDEX import_runs_document_key_index ON import_runs (document_key);
CREATE INDEX import_runs_snapshot_key_index ON import_runs (snapshot_key);

CREATE TRIGGER catalog_snapshots_are_immutable_update
BEFORE UPDATE ON catalog_snapshots
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;

CREATE TRIGGER catalog_snapshots_are_immutable_delete
BEFORE DELETE ON catalog_snapshots
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;

CREATE TRIGGER catalog_snapshots_are_immutable_insert
BEFORE INSERT ON catalog_snapshots
WHEN EXISTS (SELECT 1 FROM catalog_snapshots WHERE snapshot_key = NEW.snapshot_key)
BEGIN
    SELECT RAISE(ABORT, 'catalog snapshots are immutable');
END;
