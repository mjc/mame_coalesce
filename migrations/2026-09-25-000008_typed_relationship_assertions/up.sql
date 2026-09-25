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

CREATE INDEX relationship_assertions_subject_index
    ON relationship_assertions (subject_kind, subject_key, relation_type);
CREATE INDEX relationship_assertions_target_index
    ON relationship_assertions (target_kind, target_key, relation_type);
CREATE INDEX relationship_assertions_snapshot_index
    ON relationship_assertions (source_snapshot_key, relation_type);

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

-- Preserve relationships normalized by earlier import versions. Their original attribute
-- names were not retained, so identify those rows as legacy-normalized source claims.
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))), 'source_parent_clone', 'source_assertion',
       snapshot_key, 'catalog_set', set_name, snapshot_key, 'catalog_set', parent_name, snapshot_key,
       'parent_name (legacy normalized)', source_line, source_column,
       '{"legacy_normalized":true}'
FROM snapshot_sets
WHERE parent_name IS NOT NULL;

INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))),
       CASE dependency_kind WHEN 'clone_of' THEN 'source_parent_clone' ELSE 'runtime_dependency' END,
       'source_assertion', snapshot_key, 'software_item', list_name || '/' || item_name,
       snapshot_key, 'software_item', list_name || '/' || target_item_name,
       snapshot_key, dependency_kind || ' (legacy normalized)', source_line, source_column,
       json_object('dependency_kind', dependency_kind, 'legacy_normalized', true)
FROM software_item_dependencies;
