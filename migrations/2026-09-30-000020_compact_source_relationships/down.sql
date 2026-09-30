-- The up migration intentionally drops all pre-migration relationship rows and reviews.
-- This downgrade reconstructs only claims/reviews created against the compact schema.
CREATE TEMP TABLE relationship_assertion_evidence_backup AS
WITH RECURSIVE paths(assertion_key, node_id, path, value_type, text_value, integer_value, unsigned_integer_value, real_value) AS (
    SELECT assertion_key, node_id, '$', value_type, text_value, integer_value, unsigned_integer_value, real_value
    FROM relationship_assertion_evidence WHERE parent_node_id IS NULL
    UNION ALL
    SELECT child.assertion_key, child.node_id,
           parent.path || CASE WHEN child.array_index IS NOT NULL
                               THEN '[' || child.array_index || ']'
                               ELSE '.' || json_quote(child.object_key) END,
           child.value_type, child.text_value, child.integer_value, child.unsigned_integer_value, child.real_value
    FROM relationship_assertion_evidence AS child
    JOIN paths AS parent
      ON parent.assertion_key = child.assertion_key
     AND parent.node_id = child.parent_node_id
),
ordered AS (
    SELECT *, row_number() OVER (PARTITION BY assertion_key ORDER BY node_id) AS sequence
    FROM paths
),
rebuilt(assertion_key, sequence, evidence_json) AS (
    SELECT assertion_key, 0, json('{}') FROM relationship_assertion_evidence
    GROUP BY assertion_key
    UNION ALL
    SELECT rebuilt.assertion_key, ordered.sequence,
           json_set(rebuilt.evidence_json, ordered.path,
               CASE ordered.value_type
                   WHEN 'object' THEN json('{}')
                   WHEN 'array' THEN json('[]')
                   WHEN 'string' THEN json_quote(ordered.text_value)
                   WHEN 'integer' THEN json(ordered.integer_value)
                   WHEN 'unsigned_integer' THEN json(ordered.unsigned_integer_value)
                   WHEN 'real' THEN json(ordered.real_value)
                   WHEN 'true' THEN json('true')
                   WHEN 'false' THEN json('false')
                   ELSE json('null')
               END)
    FROM rebuilt
    JOIN ordered ON ordered.assertion_key = rebuilt.assertion_key
                AND ordered.sequence = rebuilt.sequence + 1
)
SELECT assertion_key, evidence_json
FROM rebuilt AS current
WHERE sequence = (SELECT max(sequence) FROM ordered WHERE ordered.assertion_key = current.assertion_key);

CREATE TEMP TABLE relationship_assertion_support_backup AS
SELECT assertion_key, json_group_array(supported_assertion_key) AS supporting_assertion_keys_json
FROM (
    SELECT assertion_key, supported_assertion_key
    FROM relationship_assertion_support ORDER BY assertion_key, position
)
GROUP BY assertion_key;

CREATE TEMP TABLE relationship_assertions_backup AS
SELECT a.assertion_key, a.relation_type, a.origin, a.subject_snapshot_key, a.subject_kind,
       CASE WHEN a.origin != 'source_assertion' AND a.subject_kind = 'catalog_set' THEN a.generic_subject_a
            WHEN a.origin != 'source_assertion' AND a.subject_kind = 'software_item'
                THEN json_array(a.generic_subject_a, a.generic_subject_b)
            WHEN a.origin != 'source_assertion' AND a.subject_kind = 'asset_requirement'
                THEN json_array(a.generic_subject_a, a.generic_subject_b, a.generic_subject_c)
            WHEN a.origin != 'source_assertion' AND a.subject_kind = 'content_object'
                THEN a.generic_subject_a || ':' || a.generic_subject_b
            WHEN a.origin != 'source_assertion' AND a.subject_kind = 'external_record'
                THEN json_object('namespace', a.generic_subject_a, 'key', a.generic_subject_b)
            WHEN a.subject_kind = 'catalog_set' THEN a.source_subject_a
            WHEN a.subject_kind = 'software_item'
                THEN json_array(a.source_subject_a, a.source_subject_b)
            ELSE json_array(a.source_subject_a, a.source_subject_b, a.source_subject_c)
       END AS subject_key,
       a.target_snapshot_key, a.target_kind,
       CASE WHEN a.origin != 'source_assertion' AND a.target_kind = 'catalog_set' THEN a.generic_target_a
            WHEN a.origin != 'source_assertion' AND a.target_kind = 'software_item'
                THEN json_array(a.generic_target_a, a.generic_target_b)
            WHEN a.origin != 'source_assertion' AND a.target_kind = 'asset_requirement'
                THEN json_array(a.generic_target_a, a.generic_target_b, a.generic_target_c)
            WHEN a.origin != 'source_assertion' AND a.target_kind = 'content_object'
                THEN a.generic_target_a || ':' || a.generic_target_b
            WHEN a.origin != 'source_assertion' AND a.target_kind = 'external_record'
                THEN json_object('namespace', a.generic_target_a, 'key', a.generic_target_b)
            WHEN a.target_kind = 'catalog_set' THEN a.source_target_a
            WHEN a.target_kind = 'software_item'
                THEN json_array(a.source_target_a, a.source_target_b)
            ELSE json_array(a.source_target_a, a.source_target_b, a.source_target_c)
       END AS target_key,
       a.source_snapshot_key, a.source_field, a.source_line, a.source_column,
       CASE WHEN a.origin != 'source_assertion' THEN evidence.evidence_json
            WHEN a.source_field = 'merge' THEN json_object(
                'declared_merge_name', source_asset.merge_name,
                'parent_set_name', a.source_target_a,
                'expected_sha1', lower(hex(source_asset.sha1)),
                'expected_crc', lower(hex(source_asset.crc)),
                'size', source_asset.size
            )
            WHEN a.source_field = 'cloneof' AND a.subject_kind = 'software_item' THEN
                json_object('list_name', a.source_subject_a,
                            'target_item_name', a.source_target_b)
            WHEN a.source_field IN ('romof', 'sampleof') THEN
                json_object('source_field', a.source_field,
                            'target_name', CASE WHEN a.target_kind = 'software_item'
                                                THEN a.source_target_b ELSE a.source_target_a END)
            ELSE json_object('target_name', CASE WHEN a.target_kind = 'software_item'
                                                 THEN a.source_target_b ELSE a.source_target_a END)
       END AS evidence_json,
       a.rule_version, coalesce(support.supporting_assertion_keys_json, '[]')
           AS supporting_assertion_keys_json, a.created_at
FROM relationship_assertions AS a
LEFT JOIN relationship_assertion_evidence_backup AS evidence
  ON evidence.assertion_key = a.assertion_key
LEFT JOIN relationship_assertion_support_backup AS support
  ON support.assertion_key = a.assertion_key
LEFT JOIN asset_requirements AS source_asset
  ON a.origin = 'source_assertion' AND a.subject_kind = 'asset_requirement'
 AND source_asset.snapshot_key = a.source_snapshot_key
 AND source_asset.set_name = a.source_subject_a
 AND source_asset.component_order = a.source_subject_c;
CREATE TEMP TABLE relationship_reviews_backup AS
SELECT review_id, review_key, assertion_key, decision, note,
       superseded_by_assertion_key, created_at
FROM relationship_reviews;

DROP TRIGGER relationship_reviews_are_immutable_insert;
DROP TRIGGER relationship_reviews_are_immutable_delete;
DROP TRIGGER relationship_reviews_are_immutable_update;
DROP TRIGGER relationship_assertions_are_immutable_insert;
DROP TRIGGER relationship_assertions_are_immutable_delete;
DROP TRIGGER relationship_assertions_are_immutable_update;
DROP TRIGGER relationship_assertion_evidence_source_insert;
DROP TRIGGER relationship_assertion_evidence_immutable_update;
DROP TRIGGER relationship_assertion_evidence_immutable_delete;
DROP TRIGGER relationship_assertion_support_origin_insert;
DROP TRIGGER relationship_assertion_support_immutable_update;
DROP TRIGGER relationship_assertion_support_immutable_delete;
DROP INDEX relationship_reviews_assertion_index;
DROP INDEX relationship_assertions_source_snapshot_index;
DROP INDEX relationship_assertions_subject_snapshot_kind_index;
DROP INDEX relationship_assertions_target_snapshot_kind_index;
DROP TABLE relationship_reviews;
DROP TABLE relationship_assertion_support;
DROP TABLE relationship_assertion_evidence;
DROP TABLE relationship_assertions;

CREATE TABLE relationship_assertions (
    assertion_key TEXT PRIMARY KEY NOT NULL,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    origin TEXT NOT NULL CHECK (origin IN (
        'source_assertion', 'derived_candidate', 'user_conclusion'
    )),
    subject_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    subject_kind TEXT NOT NULL,
    subject_key TEXT NOT NULL,
    target_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    target_kind TEXT NOT NULL,
    target_key TEXT NOT NULL,
    source_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    source_field TEXT,
    source_line BIGINT CHECK (source_line IS NULL OR source_line > 0),
    source_column BIGINT CHECK (source_column IS NULL OR source_column > 0),
    evidence_json TEXT NOT NULL DEFAULT '{}',
    rule_version TEXT,
    supporting_assertion_keys_json TEXT NOT NULL DEFAULT '[]',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (origin = 'source_assertion' AND source_snapshot_key IS NOT NULL AND source_field IS NOT NULL)
        OR (origin != 'source_assertion' AND source_snapshot_key IS NULL AND source_field IS NULL)
    ),
    CHECK ((origin = 'derived_candidate') = (rule_version IS NOT NULL)),
    CHECK (json_valid(evidence_json) AND json_type(evidence_json) = 'object'),
    CHECK (json_valid(supporting_assertion_keys_json)
        AND json_type(supporting_assertion_keys_json) = 'array')
);
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json, rule_version,
    supporting_assertion_keys_json, created_at
)
SELECT assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
       target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
       source_line, source_column, evidence_json, rule_version,
       supporting_assertion_keys_json, created_at
FROM relationship_assertions_backup;

CREATE TABLE relationship_reviews (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE,
    assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected', 'withdrawn', 'superseded')),
    note TEXT NOT NULL,
    superseded_by_assertion_key TEXT REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'superseded') = (superseded_by_assertion_key IS NOT NULL))
);
INSERT INTO relationship_reviews
SELECT r.review_id, r.review_key, r.assertion_key, r.decision, r.note,
       r.superseded_by_assertion_key, r.created_at
FROM relationship_reviews_backup AS r
JOIN relationship_assertions AS a ON a.assertion_key = r.assertion_key;
DROP TABLE relationship_reviews_backup;
DROP TABLE relationship_assertions_backup;
DROP TABLE relationship_assertion_evidence_backup;
DROP TABLE relationship_assertion_support_backup;

CREATE INDEX relationship_assertions_subject_index
    ON relationship_assertions (subject_kind, subject_key, relation_type);
CREATE INDEX relationship_assertions_target_index
    ON relationship_assertions (target_kind, target_key, relation_type);
CREATE INDEX relationship_assertions_snapshot_index
    ON relationship_assertions (source_snapshot_key, relation_type);
CREATE INDEX relationship_assertions_subject_snapshot_kind_index
    ON relationship_assertions (subject_snapshot_key, subject_kind);
CREATE INDEX relationship_assertions_target_snapshot_kind_index
    ON relationship_assertions (target_snapshot_key, target_kind);
CREATE INDEX relationship_reviews_assertion_index
    ON relationship_reviews (assertion_key, review_id);

CREATE TRIGGER relationship_assertions_are_immutable_update
BEFORE UPDATE ON relationship_assertions
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_assertions_are_immutable_delete
BEFORE DELETE ON relationship_assertions
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_assertions_are_immutable_insert
BEFORE INSERT ON relationship_assertions
WHEN EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
)
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;
CREATE TRIGGER relationship_reviews_are_immutable_update
BEFORE UPDATE ON relationship_reviews
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
CREATE TRIGGER relationship_reviews_are_immutable_delete
BEFORE DELETE ON relationship_reviews
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
CREATE TRIGGER relationship_reviews_are_immutable_insert
BEFORE INSERT ON relationship_reviews
WHEN EXISTS (
    SELECT 1 FROM relationship_reviews
    WHERE review_key = NEW.review_key OR review_id = NEW.review_id
)
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;
