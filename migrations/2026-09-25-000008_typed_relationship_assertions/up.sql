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
       'source_assertion', snapshot_key, 'software_item', json_array(list_name, item_name),
       snapshot_key, 'software_item', json_array(list_name, target_item_name),
       snapshot_key, dependency_kind || ' (legacy normalized)', source_line, source_column,
       json_object('dependency_kind', dependency_kind, 'legacy_normalized', true)
FROM software_item_dependencies;

-- Preserve merge identity claims that were already normalized onto ROM requirements.
-- Resolve the target set using romof when present, falling back to cloneof for older
-- catalogs that did not retain a separate runtime parent.
WITH source_sets AS (
    SELECT sets.snapshot_key, sets.set_name,
           COALESCE(
               json_extract(sets.metadata_json, '$.rom_of'),
               (SELECT json_extract(extension.raw_value_json, '$')
                FROM snapshot_extensions AS extension
                WHERE extension.snapshot_key = sets.snapshot_key
                  AND extension.record_kind = 'machine'
                  AND extension.record_name = sets.set_name
                  AND extension.field_name = 'romof'
                ORDER BY extension.extension_id LIMIT 1),
               sets.parent_name
           ) AS merge_set_name
    FROM snapshot_sets AS sets
)
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))), 'exact_content_identity', 'source_assertion',
       asset.snapshot_key, 'asset_requirement',
       json_array(asset.set_name, asset.asset_name, asset.component_order),
       asset.snapshot_key, 'asset_requirement',
       json_array(source_sets.merge_set_name, target.asset_name, target.component_order),
       asset.snapshot_key, 'merge', asset.source_line, asset.source_column,
       json_object('declared_merge_name', asset.merge_name,
                   'legacy_normalized', true,
                   'expected_sha1', lower(hex(asset.sha1)),
                   'expected_crc', lower(hex(asset.crc)),
                   'size', asset.size)
FROM asset_requirements AS asset
JOIN source_sets ON source_sets.snapshot_key = asset.snapshot_key
                AND source_sets.set_name = asset.set_name
JOIN asset_requirements AS target
  ON target.snapshot_key = asset.snapshot_key
 AND target.set_name = source_sets.merge_set_name
 AND target.asset_name = asset.merge_name
 AND target.role = asset.role
WHERE asset.merge_name IS NOT NULL
  AND (SELECT COUNT(*) FROM asset_requirements AS candidate
       WHERE candidate.snapshot_key = asset.snapshot_key
         AND candidate.set_name = source_sets.merge_set_name
         AND candidate.asset_name = asset.merge_name
         AND candidate.role = asset.role) = 1;

-- Earlier Logiqx imports retained these dependency attributes in normalized set metadata.
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))), 'runtime_dependency', 'source_assertion',
       sets.snapshot_key, 'catalog_set', sets.set_name,
       sets.snapshot_key, 'catalog_set', dependency.target_name,
       sets.snapshot_key, dependency.source_field, sets.source_line, sets.source_column,
       json_object('target_name', dependency.target_name, 'legacy_normalized', true)
FROM snapshot_sets AS sets
JOIN (
    SELECT snapshot_key, set_name, 'romof' AS source_field,
           json_extract(metadata_json, '$.rom_of') AS target_name
    FROM snapshot_sets WHERE json_type(metadata_json, '$.rom_of') = 'text'
    UNION ALL
    SELECT snapshot_key, set_name, 'sampleof', json_extract(metadata_json, '$.sample_of')
    FROM snapshot_sets WHERE json_type(metadata_json, '$.sample_of') = 'text'
) AS dependency USING (snapshot_key, set_name);

-- Older MAME imports retained machine romof/sampleof attributes as extensions instead.
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))), 'runtime_dependency', 'source_assertion',
       extension.snapshot_key, 'catalog_set', extension.record_name,
       extension.snapshot_key, 'catalog_set', json_extract(extension.raw_value_json, '$'),
       extension.snapshot_key, extension.field_name, extension.source_line,
       extension.source_column,
       json_object('target_name', json_extract(extension.raw_value_json, '$'),
                   'legacy_normalized', true)
FROM snapshot_extensions AS extension
WHERE extension.record_kind = 'machine'
  AND extension.record_name IS NOT NULL
  AND extension.field_name IN ('romof', 'sampleof')
  AND NOT EXISTS (
      SELECT 1 FROM snapshot_sets AS sets
      WHERE sets.snapshot_key = extension.snapshot_key
        AND sets.set_name = extension.record_name
        AND ((extension.field_name = 'romof'
              AND json_type(sets.metadata_json, '$.rom_of') = 'text')
          OR (extension.field_name = 'sampleof'
              AND json_type(sets.metadata_json, '$.sample_of') = 'text'))
  );

-- Preserve device references stored as either strings (Logiqx) or named objects (MAME).
INSERT INTO relationship_assertions (
    assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key,
    target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field,
    source_line, source_column, evidence_json
)
SELECT lower(hex(randomblob(16))), 'runtime_dependency', 'source_assertion',
       sets.snapshot_key, 'catalog_set', sets.set_name,
       sets.snapshot_key, 'catalog_set',
       CASE device.type WHEN 'object' THEN json_extract(device.value, '$.name')
                        ELSE device.value END,
       sets.snapshot_key, 'device_ref', sets.source_line, sets.source_column,
       json_object('target_name', CASE device.type
                     WHEN 'object' THEN json_extract(device.value, '$.name')
                     ELSE device.value END,
                   'legacy_normalized', true)
FROM snapshot_sets AS sets,
     json_each(sets.metadata_json, '$.device_refs') AS device
WHERE json_type(sets.metadata_json, '$.device_refs') = 'array'
  AND CASE device.type WHEN 'object' THEN json_extract(device.value, '$.name')
                       ELSE device.value END IS NOT NULL;
