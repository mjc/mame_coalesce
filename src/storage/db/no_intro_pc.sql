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

-- These tables retain only source positions. Game codes are name=0, id=1,
-- namealt=2, region=3, languages=4, version=5, bios=6, clone=7, mergeof=8;
-- ROM codes are name=0, size=1, crc=2, md5=3, sha1=4.
CREATE TABLE no_intro_pc_game_attribute_positions (
    set_id INTEGER NOT NULL REFERENCES no_intro_pc_games(set_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind IN(0,1,2,3,4,5,6,7,8)),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(set_id,field_kind),
    UNIQUE(set_id,source_order)
) WITHOUT ROWID;

CREATE TABLE no_intro_pc_rom_attribute_positions (
    occurrence_id INTEGER NOT NULL REFERENCES no_intro_pc_file_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind IN(0,1,2,3,4)),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind),
    UNIQUE(occurrence_id,source_order)
) WITHOUT ROWID;

CREATE VIEW no_intro_pc_expected_game_attribute_positions AS
SELECT set_id,0 AS field_kind FROM no_intro_pc_games
UNION ALL SELECT set_id,1 FROM no_intro_pc_games WHERE archive_id IS NOT NULL
UNION ALL SELECT set_id,2 FROM no_intro_pc_games WHERE name_alt IS NOT NULL
UNION ALL SELECT set_id,3 FROM no_intro_pc_games WHERE region IS NOT NULL
UNION ALL SELECT set_id,4 FROM no_intro_pc_games WHERE languages_present=1
UNION ALL SELECT set_id,5 FROM no_intro_pc_games WHERE version IS NOT NULL
UNION ALL SELECT set_id,6 FROM no_intro_pc_games WHERE bios_text IS NOT NULL
UNION SELECT set_id,7 FROM no_intro_pc_clone_markers
UNION SELECT set_id,7 FROM no_intro_pc_clone_links
UNION SELECT set_id,8 FROM no_intro_pc_merge_links;

CREATE VIEW no_intro_pc_expected_rom_attribute_positions AS
SELECT occurrence_id,0 AS field_kind FROM no_intro_pc_file_claims
UNION ALL SELECT occurrence_id,1 FROM no_intro_pc_file_claims WHERE size_text IS NOT NULL
UNION SELECT assertion.occurrence_id,
       CASE digest.algorithm WHEN 'crc32' THEN 2 WHEN 'md5' THEN 3 WHEN 'sha1' THEN 4 END
FROM occurrence_digest_assertions AS assertion
JOIN digest_values AS digest USING(digest_id)
JOIN no_intro_pc_file_claims AS claim USING(occurrence_id)
WHERE assertion.scope='whole_asset' AND assertion.provenance='source_declared'
  AND digest.algorithm IN('crc32','md5','sha1');

CREATE TRIGGER no_intro_pc_game_attribute_positions_insert_guard
BEFORE INSERT ON no_intro_pc_game_attribute_positions
WHEN EXISTS(SELECT 1 FROM no_intro_pc_game_attribute_positions AS old
            WHERE old.set_id=NEW.set_id
              AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
OR NOT EXISTS(
    SELECT 1 FROM no_intro_pc_games AS game
    JOIN catalog_sets AS sets USING(set_id)
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN no_intro_pc_documents AS document USING(snapshot_key)
    JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
    WHERE game.set_id=NEW.set_id AND groups.kind='root'
      AND sets.source_element_kind='no_intro_pc_game'
      AND CASE NEW.field_kind
          WHEN 0 THEN sets.set_name IS NOT NULL
          WHEN 1 THEN game.archive_id IS NOT NULL
          WHEN 2 THEN game.name_alt IS NOT NULL
          WHEN 3 THEN game.region IS NOT NULL
          WHEN 4 THEN game.languages_present=1
          WHEN 5 THEN game.version IS NOT NULL
          WHEN 6 THEN game.bios_text IS NOT NULL
          WHEN 7 THEN EXISTS(SELECT 1 FROM no_intro_pc_clone_markers WHERE set_id=game.set_id)
                    OR EXISTS(SELECT 1 FROM no_intro_pc_clone_links WHERE set_id=game.set_id)
          WHEN 8 THEN EXISTS(SELECT 1 FROM no_intro_pc_merge_links WHERE set_id=game.set_id)
          ELSE 0 END
)
BEGIN SELECT RAISE(ABORT,'P/C game attribute position requires its present field on an unpublished native game'); END;
CREATE TRIGGER no_intro_pc_game_attribute_positions_immutable_update
BEFORE UPDATE ON no_intro_pc_game_attribute_positions
BEGIN SELECT RAISE(ABORT,'P/C game attribute positions are immutable'); END;
CREATE TRIGGER no_intro_pc_game_attribute_positions_immutable_delete
BEFORE DELETE ON no_intro_pc_game_attribute_positions
BEGIN SELECT RAISE(ABORT,'P/C game attribute positions are immutable'); END;

CREATE TRIGGER no_intro_pc_rom_attribute_positions_insert_guard
BEFORE INSERT ON no_intro_pc_rom_attribute_positions
WHEN EXISTS(SELECT 1 FROM no_intro_pc_rom_attribute_positions AS old
            WHERE old.occurrence_id=NEW.occurrence_id
              AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
OR NOT EXISTS(
    SELECT 1 FROM no_intro_pc_file_claims AS claim
    JOIN asset_occurrences AS occurrence USING(occurrence_id)
    JOIN no_intro_pc_games AS game ON game.set_id=occurrence.record_id
    JOIN catalog_sets AS sets ON sets.set_id=game.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN no_intro_pc_documents AS document USING(snapshot_key)
    JOIN no_intro_pc_pending_snapshots AS pending USING(snapshot_key)
    WHERE claim.occurrence_id=NEW.occurrence_id
      AND occurrence.claim_kind='no_intro_pc_file'
      AND sets.source_element_kind='no_intro_pc_game' AND groups.kind='root'
      AND CASE NEW.field_kind
          WHEN 0 THEN claim.name IS NOT NULL
          WHEN 1 THEN claim.size_text IS NOT NULL
          ELSE NEW.field_kind IN(2,3,4) AND
               (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion
                JOIN digest_values AS digest USING(digest_id)
                WHERE assertion.occurrence_id=claim.occurrence_id
                  AND assertion.scope='whole_asset'
                  AND assertion.provenance='source_declared'
                  AND digest.algorithm=CASE NEW.field_kind
                      WHEN 2 THEN 'crc32' WHEN 3 THEN 'md5' WHEN 4 THEN 'sha1' END)=1 END
)
BEGIN SELECT RAISE(ABORT,'P/C ROM attribute position requires its present field on an unpublished native ROM'); END;
CREATE TRIGGER no_intro_pc_rom_attribute_positions_immutable_update
BEFORE UPDATE ON no_intro_pc_rom_attribute_positions
BEGIN SELECT RAISE(ABORT,'P/C ROM attribute positions are immutable'); END;
CREATE TRIGGER no_intro_pc_rom_attribute_positions_immutable_delete
BEFORE DELETE ON no_intro_pc_rom_attribute_positions
BEGIN SELECT RAISE(ABORT,'P/C ROM attribute positions are immutable'); END;

CREATE VIEW no_intro_pc_actual_game_attribute_positions AS
SELECT groups.snapshot_key,position.set_id,position.field_kind
FROM no_intro_pc_game_attribute_positions AS position
JOIN catalog_sets AS sets USING(set_id)
JOIN catalog_set_groups AS groups USING(set_group_id);

CREATE VIEW no_intro_pc_actual_rom_attribute_positions AS
SELECT groups.snapshot_key,position.occurrence_id,position.field_kind
FROM no_intro_pc_rom_attribute_positions AS position
JOIN asset_occurrences AS occurrence USING(occurrence_id)
JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id);

CREATE VIEW no_intro_pc_expected_game_attribute_positions_in_snapshot AS
SELECT groups.snapshot_key,expected.set_id,expected.field_kind
FROM no_intro_pc_expected_game_attribute_positions AS expected
JOIN catalog_sets AS sets USING(set_id)
JOIN catalog_set_groups AS groups USING(set_group_id);

CREATE VIEW no_intro_pc_expected_rom_attribute_positions_in_snapshot AS
SELECT groups.snapshot_key,expected.occurrence_id,expected.field_kind
FROM no_intro_pc_expected_rom_attribute_positions AS expected
JOIN asset_occurrences AS occurrence USING(occurrence_id)
JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
JOIN catalog_set_groups AS groups USING(set_group_id);

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
OR EXISTS(
    SELECT snapshot_key,set_id,field_kind
    FROM no_intro_pc_expected_game_attribute_positions_in_snapshot
    WHERE snapshot_key=NEW.snapshot_key
    EXCEPT
    SELECT snapshot_key,set_id,field_kind
    FROM no_intro_pc_actual_game_attribute_positions
    WHERE snapshot_key=NEW.snapshot_key
)
OR EXISTS(
    SELECT snapshot_key,set_id,field_kind
    FROM no_intro_pc_actual_game_attribute_positions
    WHERE snapshot_key=NEW.snapshot_key
    EXCEPT
    SELECT snapshot_key,set_id,field_kind
    FROM no_intro_pc_expected_game_attribute_positions_in_snapshot
    WHERE snapshot_key=NEW.snapshot_key
)
OR EXISTS(
    SELECT snapshot_key,occurrence_id,field_kind
    FROM no_intro_pc_expected_rom_attribute_positions_in_snapshot
    WHERE snapshot_key=NEW.snapshot_key
    EXCEPT
    SELECT snapshot_key,occurrence_id,field_kind
    FROM no_intro_pc_actual_rom_attribute_positions
    WHERE snapshot_key=NEW.snapshot_key
)
OR EXISTS(
    SELECT snapshot_key,occurrence_id,field_kind
    FROM no_intro_pc_actual_rom_attribute_positions
    WHERE snapshot_key=NEW.snapshot_key
    EXCEPT
    SELECT snapshot_key,occurrence_id,field_kind
    FROM no_intro_pc_expected_rom_attribute_positions_in_snapshot
    WHERE snapshot_key=NEW.snapshot_key
)
OR EXISTS(
    SELECT 1
    FROM no_intro_pc_rom_attribute_positions AS position
    JOIN no_intro_pc_actual_rom_attribute_positions AS actual
      ON actual.occurrence_id=position.occurrence_id
     AND actual.field_kind=position.field_kind
    WHERE actual.snapshot_key=NEW.snapshot_key AND position.field_kind IN(2,3,4)
      AND (SELECT COUNT(*)
           FROM occurrence_digest_assertions AS assertion
           JOIN digest_values AS digest USING(digest_id)
           WHERE assertion.occurrence_id=position.occurrence_id
             AND assertion.scope='whole_asset'
             AND assertion.provenance='source_declared'
             AND digest.algorithm=CASE position.field_kind
                 WHEN 2 THEN 'crc32' WHEN 3 THEN 'md5' WHEN 4 THEN 'sha1' END)<>1
)
OR NOT EXISTS(SELECT 1 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id)
JOIN no_intro_pc_games AS game USING(set_id) WHERE groups.snapshot_key=NEW.snapshot_key))
BEGIN SELECT RAISE(ABORT,'P/C publication requires its native document, declared header and games'); END;
