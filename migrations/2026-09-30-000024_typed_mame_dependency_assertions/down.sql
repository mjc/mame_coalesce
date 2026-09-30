DROP VIEW relationship_assertion_explanations;

DROP TRIGGER relationship_assertions_are_immutable_insert;
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, source_snapshot_key, source_field,
    source_line, source_column, subject_kind, source_subject_a,
    target_kind, source_target_a
)
SELECT dependency.assertion_key, 'runtime_dependency', 'source_assertion',
       set_row.snapshot_key, dependency.dependency_kind, dependency.source_line,
       dependency.source_column, 'catalog_set', set_row.set_name,
       'catalog_set', dependency.target_name
FROM mame_machine_dependencies AS dependency
JOIN snapshot_sets AS set_row ON set_row.set_id = dependency.set_id;

DROP TRIGGER relationship_reviews_assertion_exists_insert;
DROP TRIGGER relationship_reviews_superseding_assertion_exists_insert;
DROP TRIGGER relationship_reviews_are_immutable_insert;
DROP TRIGGER relationship_reviews_are_immutable_delete;
DROP TRIGGER relationship_reviews_are_immutable_update;
CREATE TABLE relationship_reviews_with_assertion_fk (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE,
    assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected', 'withdrawn', 'superseded')),
    note TEXT NOT NULL,
    superseded_by_assertion_key TEXT REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'superseded') = (superseded_by_assertion_key IS NOT NULL))
);
INSERT INTO relationship_reviews_with_assertion_fk (
    review_id, review_key, assertion_key, decision, note, superseded_by_assertion_key, created_at
)
SELECT review_id, review_key, assertion_key, decision, note, superseded_by_assertion_key, created_at
FROM relationship_reviews;
DROP TABLE relationship_reviews;
ALTER TABLE relationship_reviews_with_assertion_fk RENAME TO relationship_reviews;

DROP TRIGGER relationship_assertion_support_origin_insert;
DROP TRIGGER relationship_assertion_support_target_exists_insert;
DROP TRIGGER relationship_assertion_support_immutable_update;
DROP TRIGGER relationship_assertion_support_immutable_delete;
CREATE TABLE relationship_assertion_support_with_assertion_fk (
    assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    position INTEGER NOT NULL CHECK (position >= 0),
    supported_assertion_key TEXT NOT NULL REFERENCES relationship_assertions (assertion_key) ON DELETE RESTRICT,
    PRIMARY KEY (assertion_key, position)
) WITHOUT ROWID;
INSERT INTO relationship_assertion_support_with_assertion_fk (
    assertion_key, position, supported_assertion_key
)
SELECT assertion_key, position, supported_assertion_key
FROM relationship_assertion_support;
DROP TABLE relationship_assertion_support;
ALTER TABLE relationship_assertion_support_with_assertion_fk
    RENAME TO relationship_assertion_support;

CREATE INDEX relationship_reviews_assertion_index
    ON relationship_reviews (assertion_key, review_id);

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
CREATE TRIGGER relationship_assertion_support_origin_insert
BEFORE INSERT ON relationship_assertion_support
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions
    WHERE assertion_key = NEW.assertion_key AND origin = 'derived_candidate'
)
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

DROP TRIGGER mame_machine_dependencies_assertion_key_insert;
DROP INDEX mame_machine_dependencies_assertion_key_index;
ALTER TABLE mame_machine_dependencies DROP COLUMN assertion_key;
