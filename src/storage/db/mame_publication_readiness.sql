SELECT 1
FROM catalog_snapshots AS snapshot JOIN parser_interpretations USING (interpretation_key)
JOIN requested USING (snapshot_key)
WHERE format='mame-listxml' AND (
    snapshot.declared_version IS NOT NULL
    OR NOT EXISTS (SELECT 1 FROM mame_document_facts WHERE snapshot_key=snapshot.snapshot_key)
    OR (SELECT count(*) FROM catalog_set_groups WHERE snapshot_key=snapshot.snapshot_key AND kind='root')<>1
    OR EXISTS (SELECT 1 FROM catalog_set_groups WHERE snapshot_key=snapshot.snapshot_key AND kind<>'root')
    OR NOT EXISTS (
        SELECT 1 FROM catalog_set_groups JOIN catalog_sets USING(set_group_id)
        WHERE snapshot_key=snapshot.snapshot_key
    )
    OR EXISTS (
        SELECT 1 FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING (set_group_id)
        WHERE groups.snapshot_key=snapshot.snapshot_key AND (
            sets.source_element_kind<>'mame_machine'
            OR NOT EXISTS (SELECT 1 FROM mame_machines WHERE set_id=sets.set_id)
            OR EXISTS (
                SELECT 1 FROM asset_occurrences AS occurrence WHERE record_id=sets.set_id AND (
                    claim_kind NOT IN ('mame_rom','mame_disk','mame_sample')
                    OR (claim_kind='mame_sample' AND (
                        occurrence.content_uuid IS NOT NULL
                        OR NOT EXISTS (SELECT 1 FROM mame_samples WHERE occurrence_id=occurrence.occurrence_id)
                        OR EXISTS (SELECT 1 FROM occurrence_digest_assertions
                            WHERE occurrence_id=occurrence.occurrence_id AND provenance='source_declared')
                    ))
                    OR (claim_kind='mame_rom' AND NOT EXISTS (
                        SELECT 1 FROM mame_rom_claims WHERE occurrence_id=occurrence.occurrence_id
                    ))
                    OR (claim_kind='mame_disk' AND NOT EXISTS (
                        SELECT 1 FROM mame_disk_claims WHERE occurrence_id=occurrence.occurrence_id
                    ))
                    OR (claim_kind='mame_rom' AND EXISTS (
                        SELECT 1 FROM mame_rom_claims AS rom
                        LEFT JOIN mame_rom_compatibility AS compatibility USING(occurrence_id)
                        WHERE rom.occurrence_id=occurrence.occurrence_id AND (
                            rom.evidence_scope NOT IN ('whole_asset','unknown')
                            OR rom.evidence_provenance<>'source_declared'
                            OR ((rom.dump_status='nodump' OR compatibility.load_flag IS NOT NULL
                                OR compatibility.value IS NOT NULL OR compatibility.inverted IS NOT NULL
                                OR compatibility.ovha IS NOT NULL OR compatibility.no_thread IS NOT NULL)
                                AND rom.evidence_scope<>'unknown')
                            OR (occurrence.content_uuid IS NOT NULL AND (
                                rom.evidence_scope<>'whole_asset' OR rom.dump_status='nodump'
                                OR (rom.size_text IS NOT NULL AND rom.size IS NULL)
                                OR NOT COALESCE(length(rom.sha1_text)=40
                                    AND instr(rom.sha1_text,char(0))=0
                                    AND rom.sha1_text NOT GLOB '*[^0-9a-fA-F]*',0)
                                OR EXISTS (
                                    SELECT 1 FROM (
                                        SELECT rom.crc_text AS literal,8 AS width
                                        UNION ALL SELECT compatibility.md5_text,32
                                    ) AS field WHERE field.literal IS NOT NULL
                                    AND NOT (length(field.literal)=field.width
                                        AND instr(field.literal,char(0))=0
                                        AND field.literal NOT GLOB '*[^0-9a-fA-F]*')
                                )
                            ))
                            OR EXISTS (
                                SELECT 1 FROM occurrence_digest_assertions AS assertion
                                LEFT JOIN digest_values AS digest USING(digest_id)
                                WHERE assertion.occurrence_id=rom.occurrence_id
                                    AND assertion.provenance='source_declared'
                                    AND (digest.digest_id IS NULL
                                        OR NOT COALESCE(assertion.scope=rom.evidence_scope
                                        AND hex(digest.digest)=upper(CASE digest.algorithm
                                            WHEN 'crc32' THEN rom.crc_text
                                            WHEN 'md5' THEN compatibility.md5_text
                                            WHEN 'sha1' THEN rom.sha1_text END),0))
                            )
                            OR EXISTS (
                                SELECT 1 FROM (
                                    SELECT 'crc32' AS algorithm,rom.crc_text AS literal,8 AS width
                                    UNION ALL SELECT 'md5',compatibility.md5_text,32
                                    UNION ALL SELECT 'sha1',rom.sha1_text,40
                                ) AS field
                                WHERE length(field.literal)=field.width
                                    AND instr(field.literal,char(0))=0
                                    AND field.literal NOT GLOB '*[^0-9a-fA-F]*'
                                    AND NOT EXISTS (
                                        SELECT 1 FROM occurrence_digest_assertions AS assertion
                                        JOIN digest_values AS digest USING(digest_id)
                                        WHERE assertion.occurrence_id=rom.occurrence_id
                                            AND assertion.provenance='source_declared'
                                            AND assertion.scope=rom.evidence_scope
                                            AND digest.algorithm=field.algorithm
                                            AND hex(digest.digest)=upper(field.literal)
                                    )
                            )
                        )
                    ))
                    OR (claim_kind='mame_disk' AND EXISTS (
                        SELECT 1 FROM mame_disk_claims AS disk
                        WHERE disk.occurrence_id=occurrence.occurrence_id AND (
                            occurrence.content_uuid IS NOT NULL
                            OR disk.evidence_scope NOT IN ('chd_header_sha1','unknown')
                            OR disk.evidence_provenance<>'source_declared'
                            OR (disk.dump_status='nodump' AND disk.evidence_scope<>'unknown')
                            OR EXISTS (
                                SELECT 1 FROM occurrence_digest_assertions AS assertion
                                LEFT JOIN digest_values AS digest USING(digest_id)
                                WHERE assertion.occurrence_id=disk.occurrence_id
                                    AND assertion.provenance='source_declared'
                                    AND (digest.digest_id IS NULL
                                        OR NOT COALESCE(assertion.scope=disk.evidence_scope
                                        AND digest.algorithm='sha1'
                                        AND hex(digest.digest)=upper(disk.sha1_text),0))
                            )
                            OR (length(disk.sha1_text)=40
                                AND instr(disk.sha1_text,char(0))=0
                                AND disk.sha1_text NOT GLOB '*[^0-9a-fA-F]*'
                                AND NOT EXISTS (
                                    SELECT 1 FROM occurrence_digest_assertions AS assertion
                                    JOIN digest_values AS digest USING(digest_id)
                                    WHERE assertion.occurrence_id=disk.occurrence_id
                                        AND assertion.provenance='source_declared'
                                        AND assertion.scope=disk.evidence_scope
                                        AND digest.algorithm='sha1'
                                        AND hex(digest.digest)=upper(disk.sha1_text)
                                ))
                        )
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
