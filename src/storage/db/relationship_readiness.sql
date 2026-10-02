-- `requested(assertion_key)` is supplied by the API or trigger wrapper.
-- One predicate owns publication policy; native aliases seek their real owners.
SELECT requested.assertion_key,
    EXISTS (
        SELECT 1 FROM relationship_assertions AS assertion
        WHERE assertion.assertion_key=requested.assertion_key AND (
            (assertion.origin='source_assertion' AND EXISTS (
                SELECT 1 FROM snapshot_publications WHERE snapshot_key=assertion.source_snapshot_key))
            OR (assertion.origin<>'source_assertion' AND EXISTS (
                SELECT 1 FROM relationship_evidence_publications WHERE assertion_key=assertion.assertion_key))
        )
    ) OR EXISTS (
        SELECT 1 FROM catalog_relationships AS registry
        JOIN reported_catalog_relationships AS reported USING(relationship_id)
        JOIN snapshot_publications AS publication ON publication.snapshot_key=registry.snapshot_key
        WHERE registry.assertion_key=requested.assertion_key AND registry.origin='source'
          AND (
              EXISTS (SELECT 1 FROM mame_machine_links AS link
                  JOIN mame_machines AS machine ON machine.set_id=link.set_id
                  JOIN catalog_sets AS owner ON owner.set_id=machine.set_id
                  JOIN catalog_set_groups AS owner_group ON owner_group.set_group_id=owner.set_group_id
                  WHERE link.relationship_id=registry.relationship_id
                    AND link.source_reference_kind=reported.source_reference_kind
                    AND owner_group.snapshot_key=registry.snapshot_key)
              OR EXISTS (SELECT 1 FROM mame_device_references AS reference
                  JOIN mame_machines AS machine ON machine.set_id=reference.set_id
                  JOIN catalog_sets AS owner ON owner.set_id=machine.set_id
                  JOIN catalog_set_groups AS owner_group ON owner_group.set_group_id=owner.set_group_id
                  WHERE reference.relationship_id=registry.relationship_id
                    AND reference.source_reference_kind=reported.source_reference_kind
                    AND owner_group.snapshot_key=registry.snapshot_key)
              OR EXISTS (SELECT 1 FROM mame_merge_relationship_owners AS native
                  WHERE native.relationship_id=registry.relationship_id
                    AND native.source_reference_kind=reported.source_reference_kind
                    AND native.snapshot_key=registry.snapshot_key)
          )
    ) OR EXISTS (
        SELECT 1 FROM no_intro_dat_cloneof_assertions AS native
        JOIN snapshot_publications AS publication ON publication.snapshot_key=native.source_snapshot_key
        WHERE requested.assertion_key GLOB 'no-intro-dat-cloneof:*'
          AND native.native_set_id=CAST(substr(requested.assertion_key,length('no-intro-dat-cloneof:')+1) AS INTEGER)
          AND native.native_position_field_kind=2
          AND native.assertion_key=requested.assertion_key
    ) AS is_published
FROM requested WHERE requested.assertion_key IS NOT NULL
