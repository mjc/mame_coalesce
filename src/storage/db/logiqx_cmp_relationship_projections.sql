-- All source-native explanations use the same 28-column relationship endpoint ABI.
CREATE VIEW logiqx_cmp_source_relationships AS
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
CROSS JOIN logiqx_set_links AS link
CROSS JOIN catalog_sets AS owner
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND link.relationship_id=reported.relationship_id AND owner.set_id=link.set_id
UNION ALL
SELECT registry.assertion_key,'runtime_dependency','source_assertion',registry.snapshot_key,
       'device_ref',reference.source_line,reference.source_column,
       NULL,'catalog_set',reference.set_id,NULL,NULL,NULL,
       owner.set_name,NULL,NULL,registry.snapshot_key,
       NULL,'catalog_set',NULL,NULL,NULL,NULL,reference.target_name,NULL,NULL,
       registry.snapshot_key,NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN logiqx_device_references AS reference
CROSS JOIN catalog_sets AS owner
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND reference.relationship_id=reported.relationship_id AND owner.set_id=reference.set_id
UNION ALL
SELECT registry.assertion_key,
       CASE link.link_kind WHEN 'cloneof' THEN 'source_parent_clone' ELSE 'runtime_dependency' END,
       'source_assertion',registry.snapshot_key,lower(position.source_field),
       position.source_line,position.source_column,
       NULL,'catalog_set',link.set_id,NULL,NULL,NULL,owner.set_name,NULL,NULL,registry.snapshot_key,
       NULL,'catalog_set',NULL,NULL,NULL,NULL,link.target_name,NULL,NULL,registry.snapshot_key,NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN clrmamepro_set_links AS link
CROSS JOIN cmp_set_field_positions AS position
CROSS JOIN catalog_sets AS owner
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND link.relationship_id=reported.relationship_id AND owner.set_id=link.set_id
  AND position.record_id=link.set_id
  AND position.field_kind=CASE link.link_kind WHEN 'cloneof' THEN 1 WHEN 'sampleof' THEN 6 END
UNION ALL
SELECT registry.assertion_key,'source_merge','source_assertion',registry.snapshot_key,
       'merge',declaration.source_line,declaration.source_column,
       NULL,'catalog_media_entry',occurrence.record_id,NULL,NULL,NULL,
       owner.set_name,payload.name,occurrence.occurrence_id,registry.snapshot_key,
       NULL,'catalog_rom_merge_reference',occurrence.record_id,NULL,NULL,NULL,
       COALESCE(rom_parent.target_name,clone_parent.target_name),declaration.merge_name,NULL,
       registry.snapshot_key,NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN logiqx_file_merges AS declaration
CROSS JOIN asset_occurrences AS occurrence
CROSS JOIN logiqx_rom_claims AS payload
CROSS JOIN catalog_sets AS owner
LEFT JOIN logiqx_set_links AS rom_parent
  ON rom_parent.set_id=owner.set_id AND rom_parent.link_kind='romof'
LEFT JOIN logiqx_set_links AS clone_parent
  ON clone_parent.set_id=owner.set_id AND clone_parent.link_kind='cloneof'
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND declaration.relationship_id=reported.relationship_id
  AND reported.source_reference_kind='logiqx_rom_merge'
  AND occurrence.occurrence_id=declaration.occurrence_id
  AND payload.occurrence_id=occurrence.occurrence_id AND owner.set_id=occurrence.record_id
UNION ALL
SELECT registry.assertion_key,'source_merge','source_assertion',registry.snapshot_key,
       'merge',declaration.source_line,declaration.source_column,
       NULL,'catalog_media_entry',occurrence.record_id,NULL,NULL,NULL,
       owner.set_name,payload.name,occurrence.occurrence_id,registry.snapshot_key,
       NULL,'catalog_disk_merge_reference',occurrence.record_id,NULL,NULL,NULL,
       COALESCE(rom_parent.target_name,clone_parent.target_name),declaration.merge_name,NULL,
       registry.snapshot_key,NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN logiqx_file_merges AS declaration
CROSS JOIN asset_occurrences AS occurrence
CROSS JOIN logiqx_disk_claims AS payload
CROSS JOIN catalog_sets AS owner
LEFT JOIN logiqx_set_links AS rom_parent
  ON rom_parent.set_id=owner.set_id AND rom_parent.link_kind='romof'
LEFT JOIN logiqx_set_links AS clone_parent
  ON clone_parent.set_id=owner.set_id AND clone_parent.link_kind='cloneof'
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND declaration.relationship_id=reported.relationship_id
  AND reported.source_reference_kind='logiqx_disk_merge'
  AND occurrence.occurrence_id=declaration.occurrence_id
  AND payload.occurrence_id=occurrence.occurrence_id AND owner.set_id=occurrence.record_id
UNION ALL
SELECT registry.assertion_key,'source_merge','source_assertion',registry.snapshot_key,
       lower(position.source_field),position.source_line,position.source_column,
       NULL,'catalog_media_entry',occurrence.record_id,NULL,NULL,NULL,
       owner.set_name,payload.name,occurrence.occurrence_id,registry.snapshot_key,
       NULL,'catalog_rom_merge_reference',occurrence.record_id,NULL,NULL,NULL,
       parent.target_name,declaration.merge_name,NULL,registry.snapshot_key,NULL
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN clrmamepro_rom_merges AS declaration
CROSS JOIN cmp_rom_field_positions AS position
CROSS JOIN asset_occurrences AS occurrence
CROSS JOIN cmp_rom_claims AS payload
CROSS JOIN catalog_sets AS owner
LEFT JOIN clrmamepro_set_links AS parent
  ON parent.set_id=owner.set_id AND parent.link_kind='cloneof'
WHERE registry.origin='source' AND reported.relationship_id=registry.relationship_id
  AND declaration.relationship_id=reported.relationship_id
  AND reported.source_reference_kind='clrmamepro_rom_merge'
  AND position.occurrence_id=declaration.occurrence_id AND position.field_kind=6
  AND occurrence.occurrence_id=declaration.occurrence_id
  AND payload.occurrence_id=occurrence.occurrence_id AND owner.set_id=occurrence.record_id;

CREATE VIEW relationship_assertion_explanations AS
SELECT * FROM stored_relationship_assertion_explanations
UNION ALL SELECT * FROM mame_source_relationships
UNION ALL SELECT * FROM logiqx_cmp_source_relationships
UNION ALL SELECT * FROM software_dat_source_relationships;

CREATE VIEW software_dat_source_relationships AS
SELECT registry.assertion_key, 'source_parent_clone' AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       'cloneof' AS source_field, owner.source_line, owner.source_column,
       NULL AS generic_subject_snapshot_key, 'software_item' AS subject_kind,
       link.set_id AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, list.name AS source_subject_a,
       owner.set_name AS source_subject_b, NULL AS source_subject_c,
       registry.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key, 'software_item' AS target_kind,
       NULL AS target_set_id, NULL AS generic_target_a, NULL AS generic_target_b,
       NULL AS generic_target_c, list.name AS source_target_a,
       link.target_name AS source_target_b, NULL AS source_target_c,
       registry.snapshot_key AS target_snapshot_key, NULL AS rule_version
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN software_clone_links AS link
CROSS JOIN software_items AS native ON native.record_id = link.set_id
CROSS JOIN catalog_sets AS owner ON owner.set_id = native.record_id
CROSS JOIN catalog_set_groups AS groups ON groups.set_group_id = owner.set_group_id
CROSS JOIN software_lists AS list ON list.namespace_id = groups.set_group_id
CROSS JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = groups.snapshot_key
CROSS JOIN parser_interpretations AS interpretation
  ON interpretation.interpretation_key = snapshot.interpretation_key
WHERE registry.origin = 'source' AND reported.relationship_id = registry.relationship_id
  AND reported.source_reference_kind = link.source_reference_kind
  AND link.relationship_id = reported.relationship_id
  AND owner.source_element_kind = 'software_item' AND groups.kind = 'software_list'
  AND registry.snapshot_key = groups.snapshot_key
  AND interpretation.format = 'mame-softwarelist-xml'
UNION ALL
SELECT registry.assertion_key, 'source_parent_clone' AS relation_type,
       'source_assertion' AS origin, registry.snapshot_key AS source_snapshot_key,
       link.link_kind AS source_field, position.source_line, position.source_column,
       NULL AS generic_subject_snapshot_key, 'catalog_set' AS subject_kind,
       link.set_id AS subject_set_id, NULL AS generic_subject_a, NULL AS generic_subject_b,
       NULL AS generic_subject_c, owner.set_name AS source_subject_a,
       NULL AS source_subject_b, NULL AS source_subject_c,
       registry.snapshot_key AS subject_snapshot_key,
       NULL AS generic_target_snapshot_key,
       CASE link.link_kind WHEN 'cloneof' THEN 'catalog_set'
                           ELSE 'no_intro_dat_id_reference' END AS target_kind,
       CASE link.link_kind WHEN 'cloneof' THEN NULL ELSE link.set_id END AS target_set_id,
       NULL AS generic_target_a, NULL AS generic_target_b, NULL AS generic_target_c,
       link.target_literal AS source_target_a, NULL AS source_target_b,
       NULL AS source_target_c, registry.snapshot_key AS target_snapshot_key,
       NULL AS rule_version
FROM catalog_relationships AS registry
CROSS JOIN reported_catalog_relationships AS reported
CROSS JOIN no_intro_dat_set_links AS link
CROSS JOIN no_intro_dat_games AS native ON native.set_id = link.set_id
CROSS JOIN no_intro_dat_game_field_positions AS position
  ON position.set_id = link.set_id
 AND position.field_kind = CASE link.link_kind WHEN 'cloneof' THEN 2 ELSE 3 END
CROSS JOIN catalog_sets AS owner ON owner.set_id = native.set_id
CROSS JOIN catalog_set_groups AS groups ON groups.set_group_id = owner.set_group_id
CROSS JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key = groups.snapshot_key
CROSS JOIN parser_interpretations AS interpretation
  ON interpretation.interpretation_key = snapshot.interpretation_key
WHERE registry.origin = 'source' AND reported.relationship_id = registry.relationship_id
  AND reported.source_reference_kind = link.source_reference_kind
  AND link.relationship_id = reported.relationship_id
  AND owner.source_element_kind = 'no_intro_dat_game' AND groups.kind = 'root'
  AND registry.snapshot_key = groups.snapshot_key
  AND interpretation.format IN ('no-intro-dat-v3-strict', 'no-intro-dat-v3-compatible',
                                 'no-intro-dat-v4-strict', 'no-intro-dat-v4-compatible');
