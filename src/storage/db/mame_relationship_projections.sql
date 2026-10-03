CREATE VIEW mame_source_relationships AS
SELECT registry.assertion_key,
       CASE link.link_kind WHEN 'cloneof' THEN 'source_parent_clone' ELSE 'runtime_dependency' END AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       link.link_kind AS source_field,
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_line END AS source_line,
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_column END AS source_column,
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
LEFT JOIN mame_machines_attribute_positions AS position
  ON position.set_id=link.set_id
 AND position.field_kind=CASE link.link_kind WHEN 'cloneof' THEN 6 WHEN 'romof' THEN 7 WHEN 'sampleof' THEN 8 END
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND link.relationship_id=reported.relationship_id AND owner.set_id=link.set_id
UNION ALL
SELECT registry.assertion_key, 'runtime_dependency', 'source_assertion', registry.snapshot_key,
       'device_ref',
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_line END,
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_column END,
       NULL, 'catalog_set', reference.set_id, NULL, NULL, NULL,
       owner.set_name, NULL, NULL, registry.snapshot_key,
       NULL, 'catalog_set', NULL, NULL, NULL, NULL,
       reference.name, NULL, NULL, registry.snapshot_key, NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN mame_device_references AS reference CROSS JOIN catalog_sets AS owner
LEFT JOIN mame_device_references_attribute_positions AS position
  ON position.set_id=reference.set_id AND position.reference_order=reference.reference_order AND position.field_kind=1
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND reference.relationship_id=reported.relationship_id AND owner.set_id=reference.set_id
UNION ALL
SELECT registry.assertion_key, 'source_merge', 'source_assertion', registry.snapshot_key,
       'merge',
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_line END,
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_column END,
       NULL, 'catalog_media_entry', occurrence.record_id, NULL, NULL, NULL,
       owner.set_name, payload.name, occurrence.occurrence_id, registry.snapshot_key,
       NULL, 'catalog_rom_merge_reference', occurrence.record_id, NULL, NULL, NULL,
       COALESCE(rom_parent.target_name, clone_parent.target_name), declaration.merge_name, NULL,
       registry.snapshot_key, NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN mame_rom_merges AS declaration CROSS JOIN asset_occurrences AS occurrence
CROSS JOIN mame_rom_claims AS payload CROSS JOIN catalog_sets AS owner
LEFT JOIN mame_rom_claims_attribute_positions AS position
  ON position.occurrence_id=declaration.occurrence_id AND position.field_kind=5
LEFT JOIN mame_machine_links AS rom_parent ON rom_parent.set_id=owner.set_id AND rom_parent.link_kind='romof'
LEFT JOIN mame_machine_links AS clone_parent ON clone_parent.set_id=owner.set_id AND clone_parent.link_kind='cloneof'
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND reported.source_reference_kind='mame_rom_merge'
  AND declaration.relationship_id=reported.relationship_id
  AND occurrence.occurrence_id=declaration.occurrence_id
  AND payload.occurrence_id=occurrence.occurrence_id AND owner.set_id=occurrence.record_id
UNION ALL
SELECT registry.assertion_key, 'source_merge', 'source_assertion', registry.snapshot_key,
       'merge',
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_line END,
       CASE WHEN typeof(position.source_order)='integer' AND position.source_order>=0 THEN position.source_column END,
       NULL, 'catalog_media_entry', occurrence.record_id, NULL, NULL, NULL,
       owner.set_name, payload.name, occurrence.occurrence_id, registry.snapshot_key,
       NULL, 'catalog_disk_merge_reference', occurrence.record_id, NULL, NULL, NULL,
       COALESCE(rom_parent.target_name, clone_parent.target_name), declaration.merge_name, NULL,
       registry.snapshot_key, NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN mame_disk_merges AS declaration CROSS JOIN asset_occurrences AS occurrence
CROSS JOIN mame_disk_claims AS payload CROSS JOIN catalog_sets AS owner
LEFT JOIN mame_disk_claims_attribute_positions AS position
  ON position.occurrence_id=declaration.occurrence_id AND position.field_kind=2
LEFT JOIN mame_machine_links AS rom_parent ON rom_parent.set_id=owner.set_id AND rom_parent.link_kind='romof'
LEFT JOIN mame_machine_links AS clone_parent ON clone_parent.set_id=owner.set_id AND clone_parent.link_kind='cloneof'
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND reported.source_reference_kind='mame_disk_merge'
  AND declaration.relationship_id=reported.relationship_id
  AND occurrence.occurrence_id=declaration.occurrence_id
  AND payload.occurrence_id=occurrence.occurrence_id AND owner.set_id=occurrence.record_id;
