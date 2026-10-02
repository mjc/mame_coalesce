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
        SELECT 1 FROM mame_machine_dependencies AS dependency
        JOIN catalog_sets AS sets ON sets.set_id=dependency.set_id
        JOIN catalog_set_groups AS groups USING(set_group_id)
        JOIN snapshot_publications AS publication ON publication.snapshot_key=groups.snapshot_key
        WHERE requested.assertion_key GLOB 'mame-dependency:*'
          AND dependency.set_id=CAST(substr(
              substr(requested.assertion_key,length('mame-dependency:')+1),1,
              instr(substr(requested.assertion_key,length('mame-dependency:')+1),':')-1) AS INTEGER)
          AND dependency.dependency_order=CAST(substr(
              substr(requested.assertion_key,length('mame-dependency:')+1),
              instr(substr(requested.assertion_key,length('mame-dependency:')+1),':')+1) AS INTEGER)
          AND requested.assertion_key='mame-dependency:'||dependency.set_id||':'||dependency.dependency_order
    ) OR EXISTS (
        SELECT 1 FROM no_intro_dat_cloneof_assertions AS native
        JOIN snapshot_publications AS publication ON publication.snapshot_key=native.source_snapshot_key
        WHERE requested.assertion_key GLOB 'no-intro-dat-cloneof:*'
          AND native.native_set_id=CAST(substr(requested.assertion_key,length('no-intro-dat-cloneof:')+1) AS INTEGER)
          AND native.native_position_field_kind=2
          AND native.assertion_key=requested.assertion_key
    ) AS is_published
FROM requested WHERE requested.assertion_key IS NOT NULL
