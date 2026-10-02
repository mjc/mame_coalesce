-- Callers supply requested_decision_snapshots and requested_relationship_kinds.
-- Walk actual edition owners first, then seek their interned targets. Shared
-- UUIDs and unscoped external/digest declarations do not acquire an edition.
scoped_native_targets(target_id) AS MATERIALIZED (
    SELECT target.target_id
    FROM requested_decision_snapshots AS requested
    CROSS JOIN catalog_set_groups AS groups ON groups.snapshot_key=requested.snapshot_key
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
    CROSS JOIN catalog_set_targets AS target ON target.set_id=sets.set_id
    WHERE 'catalog_set' IN (SELECT kind FROM requested_relationship_kinds)
    UNION ALL
    SELECT target.target_id
    FROM requested_decision_snapshots AS requested
    CROSS JOIN catalog_set_groups AS groups ON groups.snapshot_key=requested.snapshot_key
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
    CROSS JOIN asset_occurrences AS media ON media.record_id=sets.set_id
    CROSS JOIN catalog_media_entry_targets AS target ON target.occurrence_id=media.occurrence_id
    WHERE 'catalog_media_entry' IN (SELECT kind FROM requested_relationship_kinds)
    UNION ALL
    SELECT target.target_id
    FROM requested_decision_snapshots AS requested
    CROSS JOIN catalog_set_groups AS groups ON groups.snapshot_key=requested.snapshot_key
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
    CROSS JOIN no_intro_archive_descriptions AS archive ON archive.set_id=sets.set_id
    CROSS JOIN no_intro_archive_targets AS target ON target.archive_id=archive.archive_id
    WHERE 'no_intro_archive' IN (SELECT kind FROM requested_relationship_kinds)
    UNION ALL
    SELECT target_id FROM requested_decision_snapshots AS requested
    CROSS JOIN unresolved_catalog_targets AS unresolved ON unresolved.snapshot_key=requested.snapshot_key
    WHERE (record_kind IN ('catalog_set','software_item')
           OR 'catalog_media_entry' IN (SELECT kind FROM requested_relationship_kinds))
), scoped_decision_ids(relationship_id) AS MATERIALIZED (
    SELECT inferred.relationship_id FROM scoped_native_targets AS selected_target
    CROSS JOIN inferred_catalog_relationships AS inferred ON inferred.from_target_id=selected_target.target_id
    UNION
    SELECT inferred.relationship_id FROM scoped_native_targets AS selected_target
    CROSS JOIN inferred_catalog_relationships AS inferred ON inferred.to_target_id=selected_target.target_id
    UNION
    SELECT manual.relationship_id FROM scoped_native_targets AS selected_target
    CROSS JOIN manual_catalog_relationships AS manual ON manual.from_target_id=selected_target.target_id
    UNION
    SELECT manual.relationship_id FROM scoped_native_targets AS selected_target
    CROSS JOIN manual_catalog_relationships AS manual ON manual.to_target_id=selected_target.target_id
), scoped_generic_assertions AS NOT MATERIALIZED (
    SELECT assertion.* FROM scoped_decision_ids AS selected
    CROSS JOIN catalog_relationships AS identity USING(relationship_id)
    CROSS JOIN inferred_relationship_assertions AS assertion USING(assertion_key)
    UNION ALL
    SELECT assertion.* FROM scoped_decision_ids AS selected
    CROSS JOIN catalog_relationships AS identity USING(relationship_id)
    CROSS JOIN manual_relationship_assertions AS assertion USING(assertion_key)
)
