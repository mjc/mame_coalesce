DROP VIEW relationship_assertion_explanations;
DROP TRIGGER mame_machine_dependencies_assertion_key_insert;
DROP TRIGGER relationship_assertions_are_immutable_insert;
DROP TRIGGER relationship_reviews_assertion_exists_insert;
DROP TRIGGER relationship_reviews_superseding_assertion_exists_insert;
DROP TRIGGER relationship_reviews_are_immutable_update;
DROP TRIGGER relationship_assertion_support_target_exists_insert;
DROP TRIGGER relationship_assertion_support_immutable_update;

UPDATE relationship_reviews
SET assertion_key = (
    SELECT 'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
    FROM mame_machine_dependencies AS dependency
    WHERE dependency.assertion_key = relationship_reviews.assertion_key
)
WHERE assertion_key IN (
    SELECT assertion_key FROM mame_machine_dependencies
);

UPDATE relationship_reviews
SET superseded_by_assertion_key = (
    SELECT 'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
    FROM mame_machine_dependencies AS dependency
    WHERE dependency.assertion_key = relationship_reviews.superseded_by_assertion_key
)
WHERE superseded_by_assertion_key IN (
    SELECT assertion_key FROM mame_machine_dependencies
);

UPDATE relationship_assertion_support
SET assertion_key = (
    SELECT 'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
    FROM mame_machine_dependencies AS dependency
    WHERE dependency.assertion_key = relationship_assertion_support.assertion_key
)
WHERE assertion_key IN (SELECT assertion_key FROM mame_machine_dependencies);

UPDATE relationship_assertion_support
SET supported_assertion_key = (
    SELECT 'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
    FROM mame_machine_dependencies AS dependency
    WHERE dependency.assertion_key = relationship_assertion_support.supported_assertion_key
)
WHERE supported_assertion_key IN (SELECT assertion_key FROM mame_machine_dependencies);

DROP INDEX mame_machine_dependencies_assertion_key_index;
ALTER TABLE mame_machine_dependencies DROP COLUMN assertion_key;

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
SELECT 'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order,
       'runtime_dependency', 'source_assertion', set_row.snapshot_key,
       dependency.dependency_kind, dependency.source_line, dependency.source_column,
       NULL, 'catalog_set', NULL, NULL, NULL,
       set_row.set_name, NULL, NULL, set_row.snapshot_key,
       NULL, 'catalog_set', NULL, NULL, NULL,
       dependency.target_name, NULL, NULL, set_row.snapshot_key, NULL
FROM mame_machine_dependencies AS dependency
JOIN snapshot_sets AS set_row ON set_row.set_id = dependency.set_id;

-- Source assertion keys are UUIDs; reserve this disjoint prefix for projected keys
-- instead of scanning every typed dependency on each source assertion insert.
CREATE TRIGGER relationship_assertions_are_immutable_insert
BEFORE INSERT ON relationship_assertions
WHEN EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
) OR NEW.assertion_key GLOB 'mame-dependency:*'
BEGIN
    SELECT RAISE(ABORT, 'relationship assertions are immutable');
END;

CREATE TRIGGER relationship_reviews_are_immutable_update
BEFORE UPDATE ON relationship_reviews
BEGIN
    SELECT RAISE(ABORT, 'relationship reviews are append-only');
END;

CREATE TRIGGER relationship_reviews_assertion_exists_insert
BEFORE INSERT ON relationship_reviews
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key
) AND NOT EXISTS (
    SELECT 1 FROM mame_machine_dependencies AS dependency
    WHERE NEW.assertion_key =
          'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
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
    SELECT 1 FROM mame_machine_dependencies AS dependency
    WHERE NEW.superseded_by_assertion_key =
          'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
 )
BEGIN
    SELECT RAISE(ABORT, 'superseding relationship assertion does not exist');
END;

CREATE TRIGGER relationship_assertion_support_target_exists_insert
BEFORE INSERT ON relationship_assertion_support
WHEN NOT EXISTS (
    SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.supported_assertion_key
) AND NOT EXISTS (
    SELECT 1 FROM mame_machine_dependencies AS dependency
    WHERE NEW.supported_assertion_key =
          'mame-dependency:' || dependency.set_id || ':' || dependency.dependency_order
)
BEGIN
    SELECT RAISE(ABORT, 'supported relationship assertion does not exist');
END;

CREATE TRIGGER relationship_assertion_support_immutable_update
BEFORE UPDATE ON relationship_assertion_support
BEGIN
    SELECT RAISE(ABORT, 'relationship assertion support is immutable');
END;
