-- Both branches seek only the machines supplied by requested_mame_machines.
SELECT native.set_id, native.reference_order AS dependency_order,
       'device_ref' AS dependency_kind, native.name AS target_name,
       native.tag AS reference_tag, native.source_order,
       native.source_line, native.source_column
FROM requested_mame_machines AS requested
CROSS JOIN mame_device_references AS native
WHERE native.set_id = requested.set_id
UNION ALL
SELECT native.set_id,
       (SELECT count(*) FROM mame_device_references AS reference WHERE reference.set_id=native.set_id)
         + CASE WHEN native.link_kind='sampleof' AND EXISTS (
             SELECT 1 FROM mame_machine_links AS rom WHERE rom.set_id=native.set_id AND rom.link_kind='romof'
           ) THEN 1 ELSE 0 END AS dependency_order,
       native.link_kind AS dependency_kind, native.target_name,
       NULL AS reference_tag, NULL AS source_order, native.source_line, native.source_column
FROM requested_mame_machines AS requested
CROSS JOIN mame_machine_links AS native
WHERE native.set_id = requested.set_id AND native.link_kind IN ('romof','sampleof')
