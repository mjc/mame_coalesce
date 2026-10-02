CREATE TABLE cmp_documents (
    snapshot_key TEXT PRIMARY KEY NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    header_present INTEGER NOT NULL CHECK (typeof(header_present) = 'integer' AND header_present IN (0,1)),
    comment_count INTEGER NOT NULL CHECK (typeof(comment_count) = 'integer' AND comment_count >= 0)
) WITHOUT ROWID;

CREATE TABLE cmp_header_field_positions (
    snapshot_key TEXT NOT NULL REFERENCES cmp_header_facts(snapshot_key) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 14),
    source_field TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    is_quoted INTEGER NOT NULL CHECK (typeof(is_quoted) = 'integer' AND is_quoted IN (0,1)),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key,field_kind),
    UNIQUE (snapshot_key,source_order),
    CHECK (lower(source_field) = CASE field_kind
      WHEN 0 THEN 'name'
      WHEN 1 THEN 'description'
      WHEN 2 THEN 'version'
      WHEN 3 THEN 'date'
      WHEN 4 THEN 'author'
      WHEN 5 THEN 'email'
      WHEN 6 THEN 'homepage'
      WHEN 7 THEN 'url'
      WHEN 8 THEN 'comment'
      WHEN 9 THEN 'category'
      WHEN 10 THEN 'header'
      WHEN 11 THEN 'forcemerging'
      WHEN 12 THEN 'forcezipping'
      WHEN 13 THEN 'forcepacking'
      WHEN 14 THEN 'forcenodump' END)
) WITHOUT ROWID;

CREATE TABLE cmp_set_field_positions (
    record_id INTEGER NOT NULL REFERENCES cmp_set_facts(record_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 11),
    source_field TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    is_quoted INTEGER NOT NULL CHECK (typeof(is_quoted) = 'integer' AND is_quoted IN (0,1)),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (record_id,field_kind),
    UNIQUE (record_id,source_order),
    CHECK (lower(source_field) = CASE field_kind
      WHEN 0 THEN 'name'
      WHEN 1 THEN 'cloneof'
      WHEN 2 THEN 'description'
      WHEN 3 THEN 'year'
      WHEN 4 THEN 'manufacturer'
      WHEN 5 THEN 'rebuildto'
      WHEN 6 THEN 'sampleof'
      WHEN 7 THEN 'region'
      WHEN 8 THEN 'releaseyear'
      WHEN 9 THEN 'releasemonth'
      WHEN 10 THEN 'releaseday'
      WHEN 11 THEN 'serial' END)
) WITHOUT ROWID;

CREATE TABLE cmp_set_rom_positions (
    occurrence_id INTEGER PRIMARY KEY NOT NULL REFERENCES cmp_rom_claims(occurrence_id) ON DELETE RESTRICT,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0)
) WITHOUT ROWID;

CREATE TABLE cmp_comments (
    snapshot_key TEXT NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    comment_order INTEGER NOT NULL CHECK (typeof(comment_order) = 'integer' AND comment_order >= 0),
    text TEXT NOT NULL,
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (snapshot_key,comment_order)
) WITHOUT ROWID;

CREATE VIEW cmp_header_declared_field_presence AS
SELECT snapshot_key,0 AS field_kind FROM cmp_header_facts WHERE name IS NOT NULL
UNION ALL SELECT snapshot_key,1 AS field_kind FROM cmp_header_facts WHERE description IS NOT NULL
UNION ALL SELECT snapshot_key,2 AS field_kind FROM cmp_header_facts WHERE version IS NOT NULL
UNION ALL SELECT snapshot_key,3 AS field_kind FROM cmp_header_facts WHERE date IS NOT NULL
UNION ALL SELECT snapshot_key,4 AS field_kind FROM cmp_header_facts WHERE author IS NOT NULL
UNION ALL SELECT snapshot_key,5 AS field_kind FROM cmp_header_facts WHERE email IS NOT NULL
UNION ALL SELECT snapshot_key,6 AS field_kind FROM cmp_header_facts WHERE homepage IS NOT NULL
UNION ALL SELECT snapshot_key,7 AS field_kind FROM cmp_header_facts WHERE url IS NOT NULL
UNION ALL SELECT snapshot_key,8 AS field_kind FROM cmp_header_facts WHERE comment IS NOT NULL
UNION ALL SELECT snapshot_key,9 AS field_kind FROM cmp_header_facts WHERE category IS NOT NULL
UNION ALL SELECT snapshot_key,10 AS field_kind FROM cmp_header_directives WHERE header_definition IS NOT NULL
UNION ALL SELECT snapshot_key,11 AS field_kind FROM cmp_header_directives WHERE forcemerging IS NOT NULL
UNION ALL SELECT snapshot_key,12 AS field_kind FROM cmp_header_directives WHERE forcezipping IS NOT NULL
UNION ALL SELECT snapshot_key,13 AS field_kind FROM cmp_header_directives WHERE forcepacking IS NOT NULL
UNION ALL SELECT snapshot_key,14 AS field_kind FROM cmp_header_directives WHERE forcenodump IS NOT NULL;

CREATE VIEW cmp_set_declared_field_presence AS
SELECT record_id,0 AS field_kind FROM cmp_set_facts
UNION ALL SELECT set_id AS record_id,1 FROM clrmamepro_set_links WHERE link_kind='cloneof'
UNION ALL SELECT record_id,2 FROM cmp_set_facts WHERE description IS NOT NULL
UNION ALL SELECT record_id,3 FROM cmp_set_facts WHERE year IS NOT NULL
UNION ALL SELECT record_id,4 FROM cmp_set_facts WHERE manufacturer IS NOT NULL
UNION ALL SELECT record_id,5 FROM cmp_set_facts WHERE rebuildto IS NOT NULL
UNION ALL SELECT set_id AS record_id,6 FROM clrmamepro_set_links WHERE link_kind='sampleof'
UNION ALL SELECT record_id,7 FROM cmp_set_facts WHERE region IS NOT NULL
UNION ALL SELECT record_id,8 FROM cmp_set_facts WHERE release_year_text IS NOT NULL
UNION ALL SELECT record_id,9 FROM cmp_set_facts WHERE release_month_text IS NOT NULL
UNION ALL SELECT record_id,10 FROM cmp_set_facts WHERE release_day_text IS NOT NULL
UNION ALL SELECT record_id,11 FROM cmp_set_facts WHERE serial IS NOT NULL;

CREATE VIEW cmp_sample_parent_links AS
SELECT set_id AS record_id,target_name
FROM clrmamepro_set_links WHERE link_kind='sampleof';

CREATE VIEW cmp_set_native_layout AS
SELECT record_id,source_order,'field' AS kind,field_kind AS child_order FROM cmp_set_field_positions
UNION ALL SELECT record_id,source_order,'sample',sample.occurrence_id
FROM cmp_samples AS sample JOIN asset_occurrences USING(occurrence_id)
UNION ALL SELECT record_id,position.source_order,'rom',position.occurrence_id
FROM cmp_set_rom_positions AS position JOIN asset_occurrences USING(occurrence_id);

-- REPLACE may bypass delete triggers when recursive_triggers is disabled.
-- Reject both primary and secondary unique conflicts before conflict handling.
CREATE TRIGGER cmp_documents_immutable_insert BEFORE INSERT ON cmp_documents
WHEN EXISTS (SELECT 1 FROM cmp_documents WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_header_facts_immutable_insert BEFORE INSERT ON cmp_header_facts
WHEN EXISTS (SELECT 1 FROM cmp_header_facts WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_header_directives_immutable_insert BEFORE INSERT ON cmp_header_directives
WHEN EXISTS (SELECT 1 FROM cmp_header_directives WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_comments_immutable_insert BEFORE INSERT ON cmp_comments
WHEN EXISTS (SELECT 1 FROM cmp_comments WHERE snapshot_key=NEW.snapshot_key AND comment_order=NEW.comment_order)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_set_facts_immutable_insert BEFORE INSERT ON cmp_set_facts
WHEN EXISTS (SELECT 1 FROM cmp_set_facts WHERE record_id=NEW.record_id)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER clrmamepro_set_links_immutable_insert BEFORE INSERT ON clrmamepro_set_links
WHEN EXISTS (SELECT 1 FROM clrmamepro_set_links WHERE set_id=NEW.set_id AND link_kind=NEW.link_kind)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_samples_immutable_insert BEFORE INSERT ON cmp_samples
WHEN EXISTS (SELECT 1 FROM cmp_samples WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_header_field_positions_immutable_insert BEFORE INSERT ON cmp_header_field_positions
WHEN EXISTS (SELECT 1 FROM cmp_header_field_positions WHERE snapshot_key=NEW.snapshot_key AND (field_kind=NEW.field_kind OR source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_set_field_positions_immutable_insert BEFORE INSERT ON cmp_set_field_positions
WHEN EXISTS (SELECT 1 FROM cmp_set_field_positions WHERE record_id=NEW.record_id AND (field_kind=NEW.field_kind OR source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_rom_claims_immutable_insert BEFORE INSERT ON cmp_rom_claims
WHEN EXISTS (SELECT 1 FROM cmp_rom_claims WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_set_rom_positions_immutable_insert BEFORE INSERT ON cmp_set_rom_positions
WHEN EXISTS (SELECT 1 FROM cmp_set_rom_positions WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;

CREATE TRIGGER cmp_documents_native_insert BEFORE INSERT ON cmp_documents
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'clrmamepro-dat') OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP document facts require an unpublished matching interpretation'); END;
CREATE TRIGGER cmp_documents_immutable_update BEFORE UPDATE ON cmp_documents
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;
CREATE TRIGGER cmp_documents_immutable_delete BEFORE DELETE ON cmp_documents
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;

CREATE TRIGGER cmp_header_facts_native_insert BEFORE INSERT ON cmp_header_facts
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'clrmamepro-dat') OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP document facts require an unpublished matching interpretation'); END;
CREATE TRIGGER cmp_header_facts_immutable_update BEFORE UPDATE ON cmp_header_facts
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;
CREATE TRIGGER cmp_header_facts_immutable_delete BEFORE DELETE ON cmp_header_facts
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;

CREATE TRIGGER cmp_header_directives_native_insert BEFORE INSERT ON cmp_header_directives
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'clrmamepro-dat') OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP document facts require an unpublished matching interpretation'); END;
CREATE TRIGGER cmp_header_directives_immutable_update BEFORE UPDATE ON cmp_header_directives
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;
CREATE TRIGGER cmp_header_directives_immutable_delete BEFORE DELETE ON cmp_header_directives
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;

CREATE TRIGGER cmp_comments_native_insert BEFORE INSERT ON cmp_comments
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key)
    WHERE snapshot_key = NEW.snapshot_key AND format = 'clrmamepro-dat') OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP document facts require an unpublished matching interpretation'); END;
CREATE TRIGGER cmp_comments_immutable_update BEFORE UPDATE ON cmp_comments
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;
CREATE TRIGGER cmp_comments_immutable_delete BEFORE DELETE ON cmp_comments
BEGIN SELECT RAISE(ABORT, 'CMP document facts are immutable'); END;

CREATE TRIGGER cmp_samples_native_insert BEFORE INSERT ON cmp_samples
WHEN NOT EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id=record_id
    WHERE occurrence_id=NEW.occurrence_id AND claim_kind='cmp_sample' AND source_element_kind='cmp_set')
 OR EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id=record_id
    JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key)
    WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'CMP samples require an unpublished matching media entry'); END;
CREATE TRIGGER cmp_samples_immutable_update BEFORE UPDATE ON cmp_samples
BEGIN SELECT RAISE(ABORT, 'CMP child facts are immutable'); END;
CREATE TRIGGER cmp_samples_immutable_delete BEFORE DELETE ON cmp_samples
BEGIN SELECT RAISE(ABORT, 'CMP child facts are immutable'); END;

CREATE TRIGGER cmp_header_field_positions_native_insert BEFORE INSERT ON cmp_header_field_positions
WHEN NOT EXISTS (SELECT 1 FROM cmp_header_declared_field_presence WHERE snapshot_key = NEW.snapshot_key AND field_kind = NEW.field_kind)
  OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT, 'CMP positions require a present unpublished native field'); END;
CREATE TRIGGER cmp_header_field_positions_immutable_update BEFORE UPDATE ON cmp_header_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP field positions are immutable'); END;
CREATE TRIGGER cmp_header_field_positions_immutable_delete BEFORE DELETE ON cmp_header_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP field positions are immutable'); END;

CREATE TRIGGER cmp_set_field_positions_native_insert BEFORE INSERT ON cmp_set_field_positions
WHEN NOT EXISTS (SELECT 1 FROM cmp_set_declared_field_presence WHERE record_id = NEW.record_id AND field_kind = NEW.field_kind)
  OR EXISTS (SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id)
    JOIN snapshot_publications USING(snapshot_key) WHERE set_id = NEW.record_id)
BEGIN SELECT RAISE(ABORT, 'CMP positions require a present unpublished native field'); END;
CREATE TRIGGER cmp_set_field_positions_immutable_update BEFORE UPDATE ON cmp_set_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP field positions are immutable'); END;
CREATE TRIGGER cmp_set_field_positions_immutable_delete BEFORE DELETE ON cmp_set_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP field positions are immutable'); END;

CREATE TRIGGER cmp_set_rom_positions_native_insert BEFORE INSERT ON cmp_set_rom_positions
WHEN NOT EXISTS (SELECT 1 FROM cmp_rom_claims WHERE occurrence_id = NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM asset_occurrences JOIN catalog_sets ON record_id=set_id
    JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key)
    WHERE occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT, 'CMP ROM layout requires an unpublished native ROM'); END;
CREATE TRIGGER cmp_set_rom_positions_immutable_update BEFORE UPDATE ON cmp_set_rom_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM layout is immutable'); END;
CREATE TRIGGER cmp_set_rom_positions_immutable_delete BEFORE DELETE ON cmp_set_rom_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM layout is immutable'); END;

CREATE TRIGGER cmp_native_document_requires_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key)
    LEFT JOIN cmp_documents USING(snapshot_key)
    WHERE snapshot_key=NEW.snapshot_key AND format='clrmamepro-dat' AND (
        cmp_documents.snapshot_key IS NULL
        OR header_present <> EXISTS(SELECT 1 FROM cmp_header_facts WHERE snapshot_key=NEW.snapshot_key)
        OR comment_count <> (SELECT COUNT(*) FROM cmp_comments WHERE snapshot_key=NEW.snapshot_key)
        OR (comment_count > 0 AND (
          (SELECT MIN(comment_order) FROM cmp_comments WHERE snapshot_key=NEW.snapshot_key) <> 0
          OR (SELECT MAX(comment_order) FROM cmp_comments WHERE snapshot_key=NEW.snapshot_key) <> comment_count-1))
    )
) OR EXISTS (
    SELECT 1 FROM cmp_header_facts LEFT JOIN cmp_header_directives USING(snapshot_key)
    WHERE snapshot_key=NEW.snapshot_key AND cmp_header_directives.snapshot_key IS NULL
) OR EXISTS (
    SELECT 1 FROM cmp_header_declared_field_presence AS field
    LEFT JOIN cmp_header_field_positions AS position USING(snapshot_key,field_kind)
    WHERE field.snapshot_key=NEW.snapshot_key AND position.snapshot_key IS NULL
)
BEGIN SELECT RAISE(ABORT, 'CMP document facts require complete native ownership'); END;

CREATE TRIGGER cmp_native_sets_require_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id)
    LEFT JOIN cmp_set_facts ON record_id=set_id
    WHERE snapshot_key=NEW.snapshot_key AND source_element_kind='cmp_set' AND record_id IS NULL
) OR EXISTS (
    SELECT 1 FROM cmp_set_declared_field_presence AS field
    JOIN catalog_sets ON set_id=field.record_id JOIN catalog_set_groups USING(set_group_id)
    LEFT JOIN cmp_set_field_positions AS position USING(record_id,field_kind)
    WHERE snapshot_key=NEW.snapshot_key AND position.record_id IS NULL
) OR EXISTS (
    SELECT 1 FROM cmp_rom_claims JOIN asset_occurrences USING(occurrence_id)
    JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id)
    LEFT JOIN cmp_set_rom_positions AS position USING(occurrence_id)
    WHERE snapshot_key=NEW.snapshot_key AND position.occurrence_id IS NULL
)
BEGIN SELECT RAISE(ABORT, 'CMP set facts require complete native ownership'); END;

CREATE TRIGGER cmp_samples_require_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id=record_id
    JOIN catalog_set_groups USING(set_group_id) LEFT JOIN cmp_samples USING(occurrence_id)
    WHERE snapshot_key=NEW.snapshot_key AND asset_occurrences.claim_kind='cmp_sample'
      AND cmp_samples.occurrence_id IS NULL
)
BEGIN SELECT RAISE(ABORT, 'CMP samples require complete native ownership'); END;

-- Media order is derived once from the native ROM/sample layout, not from
-- the position of a sample within a separate sample-only list.
CREATE TRIGGER cmp_media_order_matches_source_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM (
      SELECT occurrence.occurrence_order,
             ROW_NUMBER() OVER (PARTITION BY layout.record_id ORDER BY layout.source_order)-1 AS expected_order
      FROM cmp_set_native_layout AS layout
      JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=layout.child_order
      JOIN catalog_sets ON set_id=layout.record_id JOIN catalog_set_groups USING(set_group_id)
      WHERE snapshot_key=NEW.snapshot_key AND layout.kind IN ('sample','rom')
    ) WHERE occurrence_order<>expected_order
)
BEGIN SELECT RAISE(ABORT, 'CMP media order must match native source order'); END;

CREATE TRIGGER cmp_media_occurrences_immutable_insert BEFORE INSERT ON asset_occurrences
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence JOIN catalog_sets ON set_id=record_id
    WHERE source_element_kind='cmp_set' AND (occurrence_id=NEW.occurrence_id
      OR (record_id=NEW.record_id AND occurrence_order=NEW.occurrence_order))
)
BEGIN SELECT RAISE(ABORT, 'CMP conflicting media inserts are immutable'); END;

CREATE TRIGGER cmp_samples_have_no_declared_digests BEFORE INSERT ON occurrence_digest_assertions
WHEN NEW.provenance='source_declared' AND EXISTS (
    SELECT 1 FROM asset_occurrences WHERE occurrence_id=NEW.occurrence_id AND claim_kind='cmp_sample'
)
BEGIN SELECT RAISE(ABORT, 'CMP scalar samples cannot declare digests'); END;

CREATE TRIGGER cmp_native_layout_requires_unique_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM cmp_set_native_layout AS layout
    JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id)
    WHERE snapshot_key=NEW.snapshot_key GROUP BY record_id,source_order HAVING COUNT(*)>1
) OR EXISTS (
    SELECT 1 FROM (
      SELECT document_order AS source_order FROM cmp_set_facts JOIN catalog_sets ON record_id=set_id
      JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=NEW.snapshot_key
      UNION ALL SELECT source_order FROM cmp_header_facts WHERE snapshot_key=NEW.snapshot_key
    ) GROUP BY source_order HAVING COUNT(*)>1
)
BEGIN SELECT RAISE(ABORT, 'CMP native source positions must be unique'); END;

CREATE TABLE cmp_rom_field_positions (
    occurrence_id INTEGER NOT NULL REFERENCES cmp_rom_claims(occurrence_id) ON DELETE RESTRICT,
    field_kind INTEGER NOT NULL CHECK (typeof(field_kind) = 'integer' AND field_kind BETWEEN 0 AND 11),
    source_field TEXT NOT NULL,
    source_order INTEGER NOT NULL CHECK (typeof(source_order) = 'integer' AND source_order >= 0),
    is_quoted INTEGER NOT NULL CHECK (typeof(is_quoted) = 'integer' AND is_quoted IN (0,1)),
    source_line INTEGER NOT NULL CHECK (typeof(source_line) = 'integer' AND source_line > 0),
    source_column INTEGER NOT NULL CHECK (typeof(source_column) = 'integer' AND source_column > 0),
    PRIMARY KEY (occurrence_id,field_kind),
    UNIQUE (occurrence_id,source_order),
    CHECK (lower(source_field) = CASE field_kind
      WHEN 0 THEN 'name' WHEN 1 THEN 'size' WHEN 2 THEN 'crc' WHEN 3 THEN 'crc32'
      WHEN 4 THEN 'md5' WHEN 5 THEN 'sha1' WHEN 6 THEN 'merge' WHEN 7 THEN 'date'
      WHEN 8 THEN 'serial' WHEN 9 THEN 'status' WHEN 10 THEN 'nodump' WHEN 11 THEN 'baddump' END),
    CHECK (field_kind < 10 OR is_quoted = 0)
) WITHOUT ROWID;

CREATE TABLE clrmamepro_rom_merges (
    occurrence_id INTEGER PRIMARY KEY NOT NULL,
    merge_name TEXT NOT NULL,
    relationship_id INTEGER NOT NULL UNIQUE CHECK (typeof(relationship_id) = 'integer'),
    source_reference_kind TEXT NOT NULL DEFAULT 'clrmamepro_rom_merge'
        CHECK (source_reference_kind='clrmamepro_rom_merge'),
    FOREIGN KEY (occurrence_id) REFERENCES cmp_rom_claims(occurrence_id) ON DELETE RESTRICT,
    FOREIGN KEY (relationship_id,source_reference_kind)
        REFERENCES reported_catalog_relationships(relationship_id,source_reference_kind) ON DELETE RESTRICT
);

CREATE TRIGGER cmp_rom_claims_native_owner_insert
BEFORE INSERT ON cmp_rom_claims
WHEN NOT EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE occurrence_id = NEW.occurrence_id AND claim_kind = 'cmp_rom'
      AND source_element_kind = 'cmp_set' AND format = 'clrmamepro-dat'
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'CMP ROM claims require an unpublished native owner'); END;

CREATE VIEW cmp_rom_declared_field_presence AS
SELECT occurrence_id, 0 AS field_kind FROM cmp_rom_claims
UNION ALL SELECT occurrence_id,1 FROM cmp_rom_claims WHERE size_text IS NOT NULL
UNION ALL SELECT occurrence_id,2 FROM cmp_rom_claims WHERE crc_text IS NOT NULL
UNION ALL SELECT occurrence_id,3 FROM cmp_rom_claims WHERE crc32_text IS NOT NULL
UNION ALL SELECT occurrence_id,4 FROM cmp_rom_claims WHERE md5_text IS NOT NULL
UNION ALL SELECT occurrence_id,5 FROM cmp_rom_claims WHERE sha1_text IS NOT NULL
UNION ALL SELECT occurrence_id,6 FROM clrmamepro_rom_merges
UNION ALL SELECT occurrence_id,7 FROM cmp_rom_claims WHERE date IS NOT NULL
UNION ALL SELECT occurrence_id,8 FROM cmp_rom_claims WHERE serial IS NOT NULL
UNION ALL SELECT occurrence_id,9 FROM cmp_rom_claims WHERE status_text IS NOT NULL
UNION ALL SELECT occurrence_id,10 FROM cmp_rom_claims WHERE nodump_present = 1
UNION ALL SELECT occurrence_id,11 FROM cmp_rom_claims WHERE baddump_present = 1;

CREATE TRIGGER cmp_rom_field_positions_immutable_insert BEFORE INSERT ON cmp_rom_field_positions
WHEN EXISTS (SELECT 1 FROM cmp_rom_field_positions WHERE occurrence_id=NEW.occurrence_id AND (field_kind=NEW.field_kind OR source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT, 'CMP conflicting native inserts are immutable'); END;
CREATE TRIGGER cmp_rom_field_positions_native_owner_insert
BEFORE INSERT ON cmp_rom_field_positions
WHEN NOT EXISTS (
    SELECT 1 FROM cmp_rom_declared_field_presence AS field
    JOIN asset_occurrences USING (occurrence_id)
    JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id)
    JOIN catalog_snapshots USING (snapshot_key)
    JOIN parser_interpretations USING (interpretation_key)
    WHERE field.occurrence_id = NEW.occurrence_id AND field.field_kind = NEW.field_kind
      AND source_element_kind = 'cmp_set' AND format = 'clrmamepro-dat'
) OR EXISTS (
    SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id = record_id
    JOIN catalog_set_groups USING (set_group_id) JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'CMP positions require a present unpublished native ROM field'); END;
CREATE TRIGGER cmp_rom_field_positions_immutable_update BEFORE UPDATE ON cmp_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM positions are immutable'); END;
CREATE TRIGGER cmp_rom_field_positions_immutable_delete BEFORE DELETE ON cmp_rom_field_positions
BEGIN SELECT RAISE(ABORT, 'CMP ROM positions are immutable'); END;

CREATE TRIGGER cmp_rom_fields_require_complete_publication
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    LEFT JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND asset_occurrences.claim_kind = 'cmp_rom' AND rom.occurrence_id IS NULL
) OR EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id
    JOIN cmp_rom_declared_field_presence AS field USING (occurrence_id)
    LEFT JOIN cmp_rom_field_positions AS position USING (occurrence_id,field_kind)
    WHERE snapshot_key = NEW.snapshot_key AND position.occurrence_id IS NULL
)
BEGIN SELECT RAISE(ABORT, 'CMP ROM fields require complete position ownership'); END;

CREATE VIEW cmp_rom_declaration_conflicts AS
SELECT occurrence_id,'crc_aliases' AS conflict_kind FROM cmp_rom_claims
WHERE crc_text IS NOT NULL AND crc32_text IS NOT NULL AND upper(crc_text) <> upper(crc32_text)
UNION ALL
SELECT occurrence_id,'dump_markers' FROM cmp_rom_claims
WHERE nodump_present + baddump_present > 1
   OR (status_text IS NOT NULL AND nodump_present + baddump_present > 0);

CREATE TRIGGER cmp_uuid_requires_consistent_source_fields
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND content_uuid IS NOT NULL
      AND (rom.sha1_text IS NULL
        OR EXISTS (SELECT 1 FROM cmp_rom_declaration_conflicts AS conflict
                   WHERE conflict.occurrence_id = rom.occurrence_id)
        OR NOT EXISTS (
            SELECT 1 FROM usable_occurrence_digest_assertions AS assertion
            WHERE assertion.occurrence_id = rom.occurrence_id
              AND assertion.provenance = 'source_declared'
              AND assertion.scope = rom.evidence_scope AND assertion.scope IN ('whole_file','whole_asset')
              AND assertion.algorithm = 'sha1' AND hex(assertion.digest) = upper(rom.sha1_text)
        ))
)
BEGIN SELECT RAISE(ABORT, 'CMP UUID requires consistent whole-file source evidence'); END;

CREATE VIEW cmp_rom_declared_digests AS
SELECT occurrence_id,evidence_scope AS scope,'crc32' AS algorithm,upper(crc_text) AS digest_hex
FROM cmp_rom_claims WHERE crc_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'crc32',upper(crc32_text) FROM cmp_rom_claims WHERE crc32_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'md5',upper(md5_text) FROM cmp_rom_claims WHERE md5_text IS NOT NULL
UNION ALL
SELECT occurrence_id,evidence_scope,'sha1',upper(sha1_text) FROM cmp_rom_claims WHERE sha1_text IS NOT NULL;

CREATE TRIGGER cmp_rom_digests_match_source_declarations
BEFORE INSERT ON snapshot_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING (set_group_id)
    JOIN asset_occurrences ON record_id = set_id JOIN cmp_rom_claims AS rom USING (occurrence_id)
    WHERE snapshot_key = NEW.snapshot_key AND (
      EXISTS (
        SELECT 1 FROM cmp_rom_declared_digests AS field WHERE field.occurrence_id = rom.occurrence_id
          AND NOT EXISTS (
            SELECT 1 FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING (digest_id)
            WHERE assertion.occurrence_id = field.occurrence_id AND assertion.scope = field.scope
              AND assertion.provenance = 'source_declared' AND digest.algorithm = field.algorithm
              AND hex(digest.digest) = field.digest_hex
          )
      ) OR EXISTS (
        SELECT 1 FROM occurrence_digest_assertions AS assertion JOIN digest_values AS digest USING (digest_id)
        WHERE assertion.occurrence_id = rom.occurrence_id AND assertion.provenance = 'source_declared'
          AND NOT EXISTS (
            SELECT 1 FROM cmp_rom_declared_digests AS field
            WHERE field.occurrence_id = rom.occurrence_id AND field.scope = assertion.scope
              AND field.algorithm = digest.algorithm AND field.digest_hex = hex(digest.digest)
          )
      )
    )
)
BEGIN SELECT RAISE(ABORT, 'CMP source assertions must match declared ROM fields'); END;
