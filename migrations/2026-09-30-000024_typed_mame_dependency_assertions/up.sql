-- MAME runtime dependencies already live as typed rows. Keep their source identity
-- on that row and expose a read-only assertion projection instead of a second wide
-- relationship_assertions row.
DROP TRIGGER mame_machine_dependencies_are_immutable_update;
DROP TRIGGER relationship_assertions_are_immutable_insert;
DROP TRIGGER relationship_assertions_are_immutable_delete;
ALTER TABLE mame_machine_dependencies ADD COLUMN assertion_key TEXT;

UPDATE mame_machine_dependencies AS dependency
SET assertion_key = COALESCE(
    (
        SELECT assertion.assertion_key
        FROM snapshot_sets AS set_row
        JOIN relationship_assertions AS assertion
          ON assertion.source_snapshot_key = set_row.snapshot_key
         AND assertion.source_subject_a = set_row.set_name
         AND assertion.source_field = dependency.dependency_kind
         AND assertion.source_target_a = dependency.target_name
         AND assertion.source_line = dependency.source_line
         AND assertion.source_column = dependency.source_column
        WHERE set_row.set_id = dependency.set_id
          AND assertion.relation_type = 'runtime_dependency'
          AND assertion.origin = 'source_assertion'
          AND assertion.subject_kind = 'catalog_set'
          AND assertion.target_kind = 'catalog_set'
        LIMIT 1
    ),
    lower(hex(randomblob(16)))
);

CREATE UNIQUE INDEX mame_machine_dependencies_assertion_key_index
    ON mame_machine_dependencies (assertion_key);

CREATE TRIGGER mame_machine_dependencies_assertion_key_insert
BEFORE INSERT ON mame_machine_dependencies
WHEN NEW.assertion_key IS NULL OR EXISTS (
    SELECT 1 FROM relationship_assertions
    WHERE assertion_key = NEW.assertion_key
)
BEGIN
    SELECT RAISE(ABORT, 'MAME dependency assertion key must be unique and present');
END;
CREATE TRIGGER mame_machine_dependencies_are_immutable_update
BEFORE UPDATE ON mame_machine_dependencies
BEGIN
    SELECT RAISE(ABORT, 'MAME machine dependencies are immutable');
END;

-- Relationship reviews and support can refer to assertions projected from the typed
-- dependency table, so their identities cannot have a single-table foreign key.
DROP TRIGGER relationship_reviews_are_immutable_insert;
DROP TRIGGER relationship_reviews_are_immutable_delete;
DROP TRIGGER relationship_reviews_are_immutable_update;
CREATE TABLE relationship_reviews_without_assertion_fk (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE,
    assertion_key TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected', 'withdrawn', 'superseded')),
    note TEXT NOT NULL,
    superseded_by_assertion_key TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'superseded') = (superseded_by_assertion_key IS NOT NULL))
);
INSERT INTO relationship_reviews_without_assertion_fk (
    review_id, review_key, assertion_key, decision, note, superseded_by_assertion_key, created_at
)
SELECT review_id, review_key, assertion_key, decision, note, superseded_by_assertion_key, created_at
FROM relationship_reviews;
DROP TABLE relationship_reviews;
ALTER TABLE relationship_reviews_without_assertion_fk RENAME TO relationship_reviews;

DROP TRIGGER relationship_assertion_support_origin_insert;
DROP TRIGGER relationship_assertion_support_immutable_update;
DROP TRIGGER relationship_assertion_support_immutable_delete;
CREATE TABLE relationship_assertion_support_without_assertion_fk (
    assertion_key TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    supported_assertion_key TEXT NOT NULL,
    PRIMARY KEY (assertion_key, position)
) WITHOUT ROWID;
INSERT INTO relationship_assertion_support_without_assertion_fk (
    assertion_key, position, supported_assertion_key
)
SELECT assertion_key, position, supported_assertion_key
FROM relationship_assertion_support;
DROP TABLE relationship_assertion_support;
ALTER TABLE relationship_assertion_support_without_assertion_fk
    RENAME TO relationship_assertion_support;

CREATE INDEX relationship_reviews_assertion_index
    ON relationship_reviews (assertion_key, review_id);

CREATE TRIGGER relationship_assertions_are_immutable_insert
BEFORE INSERT ON relationship_assertions
WHEN EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
) OR EXISTS (
    SELECT 1 FROM mame_machine_dependencies WHERE assertion_key = NEW.assertion_key
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
CREATE TRIGGER relationship_reviews_assertion_exists_insert
BEFORE INSERT ON relationship_reviews
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
) AND NOT EXISTS (
    SELECT 1 FROM mame_machine_dependencies WHERE assertion_key = NEW.assertion_key
)
BEGIN
    SELECT RAISE(ABORT, 'relationship review assertion does not exist');
END;
CREATE TRIGGER relationship_reviews_superseding_assertion_exists_insert
BEFORE INSERT ON relationship_reviews
WHEN NEW.superseded_by_assertion_key IS NOT NULL
 AND NOT EXISTS (
    SELECT 1 FROM relationship_assertions
    WHERE assertion_key = NEW.superseded_by_assertion_key
 ) AND NOT EXISTS (
    SELECT 1 FROM mame_machine_dependencies
    WHERE assertion_key = NEW.superseded_by_assertion_key
 )
BEGIN
    SELECT RAISE(ABORT, 'superseding relationship assertion does not exist');
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
CREATE TRIGGER relationship_assertion_support_target_exists_insert
BEFORE INSERT ON relationship_assertion_support
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.supported_assertion_key
) AND NOT EXISTS (
    SELECT 1 FROM mame_machine_dependencies WHERE assertion_key = NEW.supported_assertion_key
)
BEGIN
    SELECT RAISE(ABORT, 'supported relationship assertion does not exist');
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

DELETE FROM relationship_assertions
WHERE assertion_key IN (SELECT assertion_key FROM mame_machine_dependencies);

CREATE TRIGGER relationship_assertions_are_immutable_delete
BEFORE DELETE ON relationship_assertions
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;

CREATE VIEW relationship_assertion_explanations AS
SELECT assertion_key, relation_type, origin, source_snapshot_key, source_field,
       source_line, source_column, generic_subject_snapshot_key, subject_kind,
       generic_subject_a, generic_subject_b, generic_subject_c, source_subject_a,
       source_subject_b, source_subject_c, subject_snapshot_key,
       generic_target_snapshot_key, target_kind, generic_target_a, generic_target_b,
       generic_target_c, source_target_a, source_target_b, source_target_c,
       target_snapshot_key, rule_version
FROM relationship_assertions
UNION ALL
SELECT dependency.assertion_key, 'runtime_dependency', 'source_assertion',
       set_row.snapshot_key, dependency.dependency_kind,
       dependency.source_line, dependency.source_column,
       NULL, 'catalog_set', NULL, NULL, NULL,
       set_row.set_name, NULL, NULL, set_row.snapshot_key,
       NULL, 'catalog_set', NULL, NULL, NULL,
       dependency.target_name, NULL, NULL, set_row.snapshot_key, NULL
FROM mame_machine_dependencies AS dependency
JOIN snapshot_sets AS set_row ON set_row.set_id = dependency.set_id;
