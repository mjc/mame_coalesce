SELECT 1
FROM catalog_snapshots AS snapshot JOIN parser_interpretations USING (interpretation_key)
JOIN requested USING (snapshot_key)
WHERE format='mame-listxml' AND (
    snapshot.declared_version IS NOT NULL
    OR NOT EXISTS (SELECT 1 FROM mame_document_facts WHERE snapshot_key=snapshot.snapshot_key)
    OR (SELECT count(*) FROM catalog_set_groups WHERE snapshot_key=snapshot.snapshot_key AND kind='root')<>1
    OR EXISTS (SELECT 1 FROM catalog_set_groups WHERE snapshot_key=snapshot.snapshot_key AND kind<>'root')
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING (set_group_id)
        WHERE groups.snapshot_key=snapshot.snapshot_key AND (
            sets.source_element_kind<>'mame_machine'
            OR NOT EXISTS (SELECT 1 FROM mame_machines WHERE set_id=sets.set_id)
            OR EXISTS (
                SELECT 1 FROM asset_occurrences AS occurrence WHERE record_id=sets.set_id AND (
                    claim_kind NOT IN ('mame_rom','mame_disk')
                    OR (claim_kind='mame_rom' AND NOT EXISTS (
                        SELECT 1 FROM mame_rom_claims WHERE occurrence_id=occurrence.occurrence_id
                    ))
                    OR (claim_kind='mame_disk' AND NOT EXISTS (
                        SELECT 1 FROM mame_disk_claims WHERE occurrence_id=occurrence.occurrence_id
                    ))
                )
            )
            OR EXISTS (
                SELECT source_order FROM positions
                WHERE set_id=sets.set_id GROUP BY source_order HAVING count(*)>1
            )
            OR (SELECT count(*) FROM mame_machine_sounds WHERE set_id=sets.set_id)>1
            OR (SELECT count(*) FROM mame_machine_inputs WHERE set_id=sets.set_id)>1
            OR (SELECT count(*) FROM mame_machine_drivers WHERE set_id=sets.set_id)>1
            OR EXISTS (
                SELECT 1 FROM machine_switches AS switches WHERE switches.set_id=sets.set_id
                AND EXISTS (
                    SELECT source_order FROM (
                        SELECT source_order FROM machine_switch_locations
                        WHERE set_id=switches.set_id AND switch_order=switches.switch_order
                        UNION ALL SELECT source_order FROM machine_switch_values
                        WHERE set_id=switches.set_id AND switch_order=switches.switch_order
                    ) GROUP BY source_order HAVING count(*)>1
                )
            )
        )
    )
)
