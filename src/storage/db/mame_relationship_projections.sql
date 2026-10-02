CREATE VIEW mame_source_relationships AS
SELECT registry.assertion_key,
       CASE link.link_kind WHEN 'cloneof' THEN 'source_parent_clone' ELSE 'runtime_dependency' END AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       link.link_kind AS source_field, link.source_line, link.source_column,
       NULL AS generic_subject_snapshot_key, 'catalog_set' AS subject_kind,
       link.set_id AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, owner.set_name AS source_subject_a,
       NULL AS source_subject_b, NULL AS source_subject_c, registry.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key, 'catalog_set' AS target_kind,
       NULL AS target_set_id, NULL AS generic_target_a, NULL AS generic_target_b,
       NULL AS generic_target_c, link.target_name AS source_target_a,
       NULL AS source_target_b, NULL AS source_target_c, registry.snapshot_key AS target_snapshot_key,
       NULL AS rule_version
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN mame_machine_links AS link CROSS JOIN catalog_sets AS owner
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND link.relationship_id=reported.relationship_id AND owner.set_id=link.set_id
UNION ALL
SELECT registry.assertion_key, 'runtime_dependency', 'source_assertion', registry.snapshot_key,
       'device_ref', reference.source_line, reference.source_column,
       NULL, 'catalog_set', reference.set_id, NULL, NULL, NULL,
       owner.set_name, NULL, NULL, registry.snapshot_key,
       NULL, 'catalog_set', NULL, NULL, NULL, NULL,
       reference.name, NULL, NULL, registry.snapshot_key, NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN mame_device_references AS reference CROSS JOIN catalog_sets AS owner
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND reference.relationship_id=reported.relationship_id AND owner.set_id=reference.set_id;
