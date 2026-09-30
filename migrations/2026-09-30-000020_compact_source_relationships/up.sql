-- MAMEC-55 has no deployed/user database to preserve. Recreate the relationship subsystem
-- empty rather than extracting any legacy denormalized assertion, evidence, support, or
-- review values. All claim/review behavior is provided by the fresh relational writers.
DROP TRIGGER relationship_reviews_are_immutable_insert;
DROP TRIGGER relationship_reviews_are_immutable_delete;
DROP TRIGGER relationship_reviews_are_immutable_update;
DROP TRIGGER relationship_assertions_are_immutable_insert;
DROP TRIGGER relationship_assertions_are_immutable_delete;
DROP TRIGGER relationship_assertions_are_immutable_update;
DROP INDEX relationship_reviews_assertion_index;
DROP INDEX relationship_assertions_subject_index;
DROP INDEX relationship_assertions_target_index;
DROP INDEX relationship_assertions_snapshot_index;
DROP INDEX relationship_assertions_subject_snapshot_kind_index;
DROP INDEX relationship_assertions_target_snapshot_kind_index;

ALTER TABLE relationship_reviews RENAME TO relationship_reviews_legacy;
ALTER TABLE relationship_assertions RENAME TO relationship_assertions_legacy;

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
    source_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    source_field TEXT,
    source_line BIGINT CHECK (source_line IS NULL OR source_line > 0),
    source_column BIGINT CHECK (source_column IS NULL OR source_column > 0),

    generic_subject_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    subject_kind TEXT NOT NULL,
    generic_subject_a TEXT,
    generic_subject_b TEXT,
    generic_subject_c BIGINT,
    source_subject_a TEXT,
    source_subject_b TEXT,
    source_subject_c BIGINT,
    subject_snapshot_key TEXT GENERATED ALWAYS AS (
        CASE WHEN origin = 'source_assertion' THEN source_snapshot_key
             ELSE generic_subject_snapshot_key END
    ) VIRTUAL,
    generic_target_snapshot_key TEXT REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    target_kind TEXT NOT NULL,
    generic_target_a TEXT,
    generic_target_b TEXT,
    generic_target_c BIGINT,
    source_target_a TEXT,
    source_target_b TEXT,
    source_target_c BIGINT,
    target_snapshot_key TEXT GENERATED ALWAYS AS (
        CASE WHEN origin = 'source_assertion' THEN source_snapshot_key
             ELSE generic_target_snapshot_key END
    ) VIRTUAL,
    rule_version TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (origin = 'source_assertion' AND source_snapshot_key IS NOT NULL
            AND source_field IS NOT NULL AND generic_subject_snapshot_key IS NULL
            AND generic_target_snapshot_key IS NULL AND source_subject_a IS NOT NULL
            AND source_target_a IS NOT NULL AND generic_subject_a IS NULL
            AND generic_subject_b IS NULL AND generic_subject_c IS NULL
            AND generic_target_a IS NULL AND generic_target_b IS NULL
            AND generic_target_c IS NULL)
        OR (origin != 'source_assertion' AND source_snapshot_key IS NULL
            AND source_field IS NULL AND generic_subject_a IS NOT NULL
            AND generic_target_a IS NOT NULL AND source_subject_a IS NULL
            AND source_subject_b IS NULL AND source_subject_c IS NULL
            AND source_target_a IS NULL AND source_target_b IS NULL
            AND source_target_c IS NULL)
    ),
    CHECK (origin = 'source_assertion' OR
        ((subject_kind = 'catalog_set' AND generic_subject_b IS NULL AND generic_subject_c IS NULL)
         OR (subject_kind IN ('software_item', 'asset_requirement', 'content_object', 'external_record')
             AND generic_subject_b IS NOT NULL
             AND ((subject_kind = 'asset_requirement') = (generic_subject_c IS NOT NULL))))),
    CHECK (origin = 'source_assertion' OR
        ((target_kind = 'catalog_set' AND generic_target_b IS NULL AND generic_target_c IS NULL)
         OR (target_kind IN ('software_item', 'asset_requirement', 'content_object', 'external_record')
             AND generic_target_b IS NOT NULL
             AND ((target_kind = 'asset_requirement') = (generic_target_c IS NOT NULL))))),
    CHECK ((origin = 'derived_candidate') = (rule_version IS NOT NULL)),
    CHECK (origin != 'source_assertion' OR
        (subject_kind IN ('catalog_set', 'software_item', 'asset_requirement')
         AND target_kind IN ('catalog_set', 'software_item', 'asset_requirement'))),
    CHECK (origin != 'source_assertion' OR source_field != 'merge' OR
        (subject_kind = 'asset_requirement' AND target_kind = 'asset_requirement')),
    CHECK (origin != 'source_assertion' OR
        source_field NOT IN ('romof', 'sampleof', 'device_ref', 'parent_name') OR
        (subject_kind = 'catalog_set' AND target_kind = 'catalog_set')),
    CHECK (origin != 'source_assertion' OR source_field != 'cloneof' OR
        ((subject_kind = 'software_item' AND target_kind = 'software_item') OR
         (subject_kind = 'catalog_set' AND target_kind = 'catalog_set'))),
    CHECK (origin != 'source_assertion' OR (
        (subject_kind != 'catalog_set' OR
            (source_subject_b IS NULL AND source_subject_c IS NULL)) AND
        (target_kind != 'catalog_set' OR
            (source_target_b IS NULL AND source_target_c IS NULL)) AND
        (subject_kind != 'software_item' OR
            (source_subject_b IS NOT NULL AND source_subject_c IS NULL)) AND
        (target_kind != 'software_item' OR
            (source_target_b IS NOT NULL AND source_target_c IS NULL)) AND
        (subject_kind != 'asset_requirement' OR
            (source_subject_b IS NOT NULL AND source_subject_c IS NOT NULL)) AND
        (target_kind != 'asset_requirement' OR
            (source_target_b IS NOT NULL AND source_target_c IS NOT NULL))
    ))
);

-- Generic evidence is an arbitrary JSON value tree at the API boundary, but is stored as
-- typed relational nodes. The migration deliberately does not import legacy JSON values.
CREATE TABLE relationship_assertion_evidence (
    assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    node_id INTEGER NOT NULL,
    parent_node_id INTEGER,
    object_key TEXT,
    array_index INTEGER,
    value_type TEXT NOT NULL CHECK (value_type IN (
        'object', 'array', 'string', 'integer', 'unsigned_integer', 'real', 'true', 'false', 'null'
    )),
    text_value TEXT,
    integer_value BIGINT,
    unsigned_integer_value TEXT,
    real_value REAL,
    PRIMARY KEY (assertion_key, node_id),
    FOREIGN KEY (assertion_key, parent_node_id)
        REFERENCES relationship_assertion_evidence (assertion_key, node_id) ON DELETE CASCADE,
    CHECK ((parent_node_id IS NULL AND object_key IS NULL AND array_index IS NULL)
        OR (parent_node_id IS NOT NULL AND ((object_key IS NOT NULL) != (array_index IS NOT NULL)))),
    CHECK ((value_type = 'string' AND text_value IS NOT NULL AND integer_value IS NULL AND unsigned_integer_value IS NULL AND real_value IS NULL)
        OR (value_type = 'integer' AND text_value IS NULL AND integer_value IS NOT NULL AND unsigned_integer_value IS NULL AND real_value IS NULL)
        OR (value_type = 'unsigned_integer' AND text_value IS NULL AND integer_value IS NULL AND unsigned_integer_value IS NOT NULL AND real_value IS NULL)
        OR (value_type = 'real' AND text_value IS NULL AND integer_value IS NULL AND unsigned_integer_value IS NULL AND real_value IS NOT NULL)
        OR (value_type NOT IN ('string', 'integer', 'unsigned_integer', 'real')
            AND text_value IS NULL AND integer_value IS NULL AND unsigned_integer_value IS NULL AND real_value IS NULL))
) WITHOUT ROWID;
CREATE TABLE relationship_assertion_support (
    assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    position INTEGER NOT NULL CHECK (position >= 0),
    supported_assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    PRIMARY KEY (assertion_key, position)
) WITHOUT ROWID;
-- Both the claim and its support identities are relational references. The composite
-- primary key is also the lookup index, so no duplicate support index is needed.

CREATE TRIGGER relationship_assertion_evidence_source_insert
BEFORE INSERT ON relationship_assertion_evidence
WHEN (SELECT origin FROM relationship_assertions WHERE assertion_key = NEW.assertion_key) = 'source_assertion'
BEGIN
    SELECT RAISE(ABORT, 'source assertion evidence is derived from typed catalog facts');
END;
CREATE TRIGGER relationship_assertion_evidence_immutable_update
BEFORE UPDATE ON relationship_assertion_evidence
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion evidence is immutable');
END;
CREATE TRIGGER relationship_assertion_evidence_immutable_delete
BEFORE DELETE ON relationship_assertion_evidence
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion evidence is immutable');
END;
CREATE TRIGGER relationship_assertion_support_origin_insert
BEFORE INSERT ON relationship_assertion_support
WHEN (SELECT origin FROM relationship_assertions WHERE assertion_key = NEW.assertion_key) != 'derived_candidate'
BEGIN
    SELECT RAISE(ABORT, 'only derived candidates may have supporting assertions');
END;
CREATE TRIGGER relationship_assertion_support_immutable_update
BEFORE UPDATE ON relationship_assertion_support
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion support is immutable');
END;
CREATE TRIGGER relationship_assertion_support_immutable_delete
BEFORE DELETE ON relationship_assertion_support
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion support is immutable');
END;

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
DROP TABLE relationship_reviews_legacy;
DROP TABLE relationship_assertions_legacy;

CREATE INDEX relationship_assertions_source_snapshot_index
    ON relationship_assertions (source_snapshot_key)
    WHERE origin = 'source_assertion';
CREATE INDEX relationship_assertions_subject_snapshot_kind_index
    ON relationship_assertions (generic_subject_snapshot_key, subject_kind)
    WHERE origin != 'source_assertion';
CREATE INDEX relationship_assertions_target_snapshot_kind_index
    ON relationship_assertions (generic_target_snapshot_key, target_kind)
    WHERE origin != 'source_assertion';
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
