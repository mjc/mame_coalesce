CREATE TABLE mame_document_facts (
    snapshot_key TEXT NOT NULL PRIMARY KEY,
    debug INTEGER NOT NULL CHECK (debug IN (0, 1)),
    config_version TEXT,
    source_line INTEGER NOT NULL CHECK (source_line > 0),
    source_column INTEGER NOT NULL CHECK (source_column > 0),
    FOREIGN KEY (snapshot_key)
        REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT
);

CREATE TRIGGER mame_document_facts_are_immutable_update
BEFORE UPDATE ON mame_document_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME document facts are immutable');
END;
CREATE TRIGGER mame_document_facts_are_immutable_delete
BEFORE DELETE ON mame_document_facts
BEGIN
    SELECT RAISE(ABORT, 'MAME document facts are immutable');
END;
