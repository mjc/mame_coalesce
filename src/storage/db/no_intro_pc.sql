-- Synthetic application P/C dialect, not authenticated DAT-o-MATIC wire data.
CREATE TABLE no_intro_pc_documents (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    header_present INTEGER NOT NULL CHECK(typeof(header_present)='integer' AND header_present IN(0,1)),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0)
) WITHOUT ROWID;

CREATE TABLE no_intro_pc_headers (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES no_intro_pc_documents(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    version_text TEXT,
    version_order INTEGER CHECK(version_order IS NULL OR (typeof(version_order)='integer' AND version_order>=0)),
    version_line INTEGER CHECK(version_line IS NULL OR (typeof(version_line)='integer' AND version_line>0)),
    version_column INTEGER CHECK(version_column IS NULL OR (typeof(version_column)='integer' AND version_column>0)),
    CHECK((version_text IS NULL)=(version_order IS NULL)),
    CHECK((version_text IS NULL)=(version_line IS NULL)),
    CHECK((version_text IS NULL)=(version_column IS NULL))
) WITHOUT ROWID;

CREATE TABLE no_intro_pc_header_names (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_pc_headers(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    name_text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(snapshot_key,source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_pc_header_descriptions (
    snapshot_key TEXT NOT NULL REFERENCES no_intro_pc_headers(snapshot_key) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    description_text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(snapshot_key,source_order)
) WITHOUT ROWID;

CREATE VIEW no_intro_pc_header_child_positions AS
SELECT snapshot_key, version_order AS source_order FROM no_intro_pc_headers WHERE version_text IS NOT NULL
UNION ALL SELECT snapshot_key,source_order FROM no_intro_pc_header_names
UNION ALL SELECT snapshot_key,source_order FROM no_intro_pc_header_descriptions;

CREATE VIEW no_intro_pc_pending_snapshots AS
SELECT snapshot.snapshot_key FROM catalog_snapshots AS snapshot
JOIN parser_interpretations AS parser USING(interpretation_key)
WHERE parser.format='no-intro-pc-xml'
AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS publication WHERE publication.snapshot_key=snapshot.snapshot_key);

CREATE TRIGGER no_intro_pc_rom_owner_insert_guard BEFORE INSERT ON no_intro_pc_file_claims
WHEN EXISTS(SELECT 1 FROM no_intro_pc_file_claims WHERE occurrence_id=NEW.occurrence_id)
OR NOT EXISTS(SELECT 1 FROM asset_occurrences AS occurrence
JOIN no_intro_pc_games AS game ON game.set_id=occurrence.record_id
JOIN catalog_sets AS sets ON sets.set_id=game.set_id
JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN no_intro_pc_documents AS document USING(snapshot_key)
JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
WHERE occurrence.occurrence_id=NEW.occurrence_id AND occurrence.claim_kind='no_intro_pc_file'
AND sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
AND (game.description_order IS NULL OR game.description_order<>NEW.source_order))
OR EXISTS(SELECT 1 FROM asset_occurrences AS requested
JOIN asset_occurrences AS sibling ON sibling.record_id=requested.record_id
JOIN no_intro_pc_file_claims AS rom ON rom.occurrence_id=sibling.occurrence_id
WHERE requested.occurrence_id=NEW.occurrence_id AND rom.source_order=NEW.source_order)
BEGIN SELECT RAISE(ABORT,'P/C ROM requires its unused unpublished native owner and a unique child position'); END;

CREATE TRIGGER no_intro_pc_game_document_insert_guard BEFORE INSERT ON no_intro_pc_games
WHEN EXISTS(SELECT 1 FROM no_intro_pc_games WHERE set_id=NEW.set_id)
OR NOT EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN no_intro_pc_documents AS document USING(snapshot_key)
JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
WHERE sets.set_id=NEW.set_id AND groups.kind='root' AND sets.source_element_kind='no_intro_pc_game')
OR EXISTS(SELECT 1 FROM catalog_sets AS requested JOIN catalog_set_groups AS request_group ON request_group.set_group_id=requested.set_group_id
JOIN catalog_set_groups AS sibling_group ON sibling_group.snapshot_key=request_group.snapshot_key
JOIN catalog_sets AS sibling ON sibling.set_group_id=sibling_group.set_group_id
JOIN no_intro_pc_games AS game ON game.set_id=sibling.set_id
WHERE requested.set_id=NEW.set_id AND game.document_order=NEW.document_order)
OR EXISTS(SELECT 1 FROM catalog_sets AS requested JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN no_intro_pc_headers AS header USING(snapshot_key)
WHERE requested.set_id=NEW.set_id AND header.source_order=NEW.document_order)
BEGIN SELECT RAISE(ABORT,'P/C game requires its unpublished document and a unique root position'); END;

CREATE TRIGGER no_intro_pc_documents_insert_guard BEFORE INSERT ON no_intro_pc_documents
WHEN EXISTS(SELECT 1 FROM no_intro_pc_documents WHERE snapshot_key=NEW.snapshot_key)
OR NOT EXISTS(SELECT 1 FROM no_intro_pc_pending_snapshots WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'P/C document requires an unused unpublished synthetic snapshot'); END;
CREATE TRIGGER no_intro_pc_documents_update_guard BEFORE UPDATE ON no_intro_pc_documents
BEGIN SELECT RAISE(ABORT,'P/C document is immutable'); END;
CREATE TRIGGER no_intro_pc_documents_delete_guard BEFORE DELETE ON no_intro_pc_documents
BEGIN SELECT RAISE(ABORT,'P/C document is immutable'); END;

CREATE TRIGGER no_intro_pc_headers_insert_guard BEFORE INSERT ON no_intro_pc_headers
WHEN EXISTS(SELECT 1 FROM no_intro_pc_headers WHERE snapshot_key=NEW.snapshot_key)
OR NOT EXISTS(SELECT 1 FROM no_intro_pc_documents AS document
JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
WHERE document.snapshot_key=NEW.snapshot_key AND document.header_present=1)
OR EXISTS(SELECT 1 FROM no_intro_pc_games AS game JOIN catalog_sets AS sets USING(set_id)
JOIN catalog_set_groups AS groups USING(set_group_id)
WHERE groups.snapshot_key=NEW.snapshot_key AND game.document_order=NEW.source_order)
BEGIN SELECT RAISE(ABORT,'P/C header requires its unpublished native document and a unique root position'); END;
CREATE TRIGGER no_intro_pc_headers_update_guard BEFORE UPDATE ON no_intro_pc_headers
BEGIN SELECT RAISE(ABORT,'P/C header is immutable'); END;
CREATE TRIGGER no_intro_pc_headers_delete_guard BEFORE DELETE ON no_intro_pc_headers
BEGIN SELECT RAISE(ABORT,'P/C header is immutable'); END;

CREATE TRIGGER no_intro_pc_header_names_insert_guard BEFORE INSERT ON no_intro_pc_header_names
WHEN EXISTS(SELECT 1 FROM no_intro_pc_header_child_positions WHERE snapshot_key=NEW.snapshot_key AND source_order=NEW.source_order)
OR NOT EXISTS(SELECT 1 FROM no_intro_pc_headers AS header JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
WHERE header.snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'P/C name requires its unpublished header and a unique child position'); END;
CREATE TRIGGER no_intro_pc_header_names_update_guard BEFORE UPDATE ON no_intro_pc_header_names
BEGIN SELECT RAISE(ABORT,'P/C header names are immutable'); END;
CREATE TRIGGER no_intro_pc_header_names_delete_guard BEFORE DELETE ON no_intro_pc_header_names
BEGIN SELECT RAISE(ABORT,'P/C header names are immutable'); END;

CREATE TRIGGER no_intro_pc_header_descriptions_insert_guard BEFORE INSERT ON no_intro_pc_header_descriptions
WHEN EXISTS(SELECT 1 FROM no_intro_pc_header_child_positions WHERE snapshot_key=NEW.snapshot_key AND source_order=NEW.source_order)
OR NOT EXISTS(SELECT 1 FROM no_intro_pc_headers AS header JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
WHERE header.snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'P/C description requires its unpublished header and a unique child position'); END;
CREATE TRIGGER no_intro_pc_header_descriptions_update_guard BEFORE UPDATE ON no_intro_pc_header_descriptions
BEGIN SELECT RAISE(ABORT,'P/C header descriptions are immutable'); END;
CREATE TRIGGER no_intro_pc_header_descriptions_delete_guard BEFORE DELETE ON no_intro_pc_header_descriptions
BEGIN SELECT RAISE(ABORT,'P/C header descriptions are immutable'); END;

CREATE TRIGGER no_intro_pc_document_publication_guard BEFORE INSERT ON snapshot_publications
WHEN EXISTS(SELECT 1 FROM catalog_snapshots AS snapshot JOIN parser_interpretations AS parser USING(interpretation_key)
WHERE snapshot.snapshot_key=NEW.snapshot_key AND parser.format='no-intro-pc-xml')
AND (NOT EXISTS(SELECT 1 FROM no_intro_pc_documents WHERE snapshot_key=NEW.snapshot_key)
OR EXISTS(SELECT 1 FROM catalog_snapshots WHERE snapshot_key=NEW.snapshot_key AND declared_version IS NOT NULL)
OR EXISTS(SELECT 1 FROM no_intro_pc_documents AS document WHERE document.snapshot_key=NEW.snapshot_key
AND document.header_present<>EXISTS(SELECT 1 FROM no_intro_pc_headers WHERE snapshot_key=NEW.snapshot_key))
OR EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
WHERE groups.snapshot_key=NEW.snapshot_key AND sets.source_element_kind='no_intro_pc_game'
AND NOT EXISTS(SELECT 1 FROM no_intro_pc_games WHERE set_id=sets.set_id))
OR EXISTS(SELECT 1 FROM catalog_set_groups AS groups
JOIN catalog_sets AS sets USING(set_group_id)
JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
WHERE groups.snapshot_key=NEW.snapshot_key AND occurrence.claim_kind='no_intro_pc_file'
AND NOT EXISTS(SELECT 1 FROM no_intro_pc_file_claims AS rom WHERE rom.occurrence_id=occurrence.occurrence_id))
OR NOT EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN no_intro_pc_games AS game USING(set_id) WHERE groups.snapshot_key=NEW.snapshot_key))
BEGIN SELECT RAISE(ABORT,'P/C publication requires its native document, declared header and games'); END;
