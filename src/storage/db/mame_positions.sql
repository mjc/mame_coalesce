SELECT mame_machines.set_id, mame_machines.description_source_order AS source_order, 'description' AS child_kind, 0 AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machines
WHERE mame_machines.set_id=requested.set_id
UNION ALL
SELECT mame_machines.set_id, mame_machines.year_source_order AS source_order, 'year' AS child_kind, 0 AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machines
WHERE mame_machines.set_id=requested.set_id AND mame_machines.year_source_order IS NOT NULL
UNION ALL
SELECT mame_machines.set_id, mame_machines.manufacturer_source_order AS source_order, 'manufacturer' AS child_kind, 0 AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machines
WHERE mame_machines.set_id=requested.set_id AND mame_machines.manufacturer_source_order IS NOT NULL
UNION ALL
SELECT mame_bios_sets.set_id, mame_bios_sets.source_order AS source_order, 'biosset' AS child_kind, mame_bios_sets.bios_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_bios_sets
WHERE mame_bios_sets.set_id=requested.set_id
UNION ALL
SELECT mame_device_references.set_id, mame_device_references.source_order, 'device_ref' AS child_kind, mame_device_references.reference_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_device_references
WHERE mame_device_references.set_id=requested.set_id
UNION ALL
SELECT machine_switches.set_id, machine_switches.source_order AS source_order, machine_switches.kind AS child_kind, machine_switches.switch_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN machine_switches
WHERE machine_switches.set_id=requested.set_id
UNION ALL
SELECT occurrence.record_id AS set_id, mame_samples.source_order, 'sample' AS child_kind, mame_samples.source_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN asset_occurrences AS occurrence
JOIN mame_samples USING(occurrence_id)
WHERE occurrence.record_id=requested.set_id AND occurrence.claim_kind='mame_sample'
UNION ALL
SELECT mame_machine_chips.set_id, mame_machine_chips.element_order AS source_order, 'chip' AS child_kind, mame_machine_chips.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_chips
WHERE mame_machine_chips.set_id=requested.set_id
UNION ALL
SELECT mame_machine_displays.set_id, mame_machine_displays.element_order AS source_order, 'display' AS child_kind, mame_machine_displays.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_displays
WHERE mame_machine_displays.set_id=requested.set_id
UNION ALL
SELECT mame_machine_sounds.set_id, mame_machine_sounds.element_order AS source_order, 'sound' AS child_kind, mame_machine_sounds.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_sounds
WHERE mame_machine_sounds.set_id=requested.set_id
UNION ALL
SELECT mame_machine_inputs.set_id, mame_machine_inputs.element_order AS source_order, 'input' AS child_kind, mame_machine_inputs.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_inputs
WHERE mame_machine_inputs.set_id=requested.set_id
UNION ALL
SELECT mame_machine_ports.set_id, mame_machine_ports.element_order AS source_order, 'port' AS child_kind, mame_machine_ports.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_ports
WHERE mame_machine_ports.set_id=requested.set_id
UNION ALL
SELECT mame_machine_adjusters.set_id, mame_machine_adjusters.element_order AS source_order, 'adjuster' AS child_kind, mame_machine_adjusters.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_adjusters
WHERE mame_machine_adjusters.set_id=requested.set_id
UNION ALL
SELECT mame_machine_drivers.set_id, mame_machine_drivers.element_order AS source_order, 'driver' AS child_kind, mame_machine_drivers.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_drivers
WHERE mame_machine_drivers.set_id=requested.set_id
UNION ALL
SELECT mame_machine_features.set_id, mame_machine_features.element_order AS source_order, 'feature' AS child_kind, mame_machine_features.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_features
WHERE mame_machine_features.set_id=requested.set_id
UNION ALL
SELECT mame_machine_devices.set_id, mame_machine_devices.element_order AS source_order, 'device' AS child_kind, mame_machine_devices.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_devices
WHERE mame_machine_devices.set_id=requested.set_id
UNION ALL
SELECT mame_machine_slots.set_id, mame_machine_slots.element_order AS source_order, 'slot' AS child_kind, mame_machine_slots.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_slots
WHERE mame_machine_slots.set_id=requested.set_id
UNION ALL
SELECT mame_machine_software_lists.set_id, mame_machine_software_lists.element_order AS source_order, 'softwarelist' AS child_kind, mame_machine_software_lists.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_software_lists
WHERE mame_machine_software_lists.set_id=requested.set_id
UNION ALL
SELECT mame_machine_ram_options.set_id, mame_machine_ram_options.element_order AS source_order, 'ramoption' AS child_kind, mame_machine_ram_options.element_order AS child_order
FROM requested_mame_machines AS requested CROSS JOIN mame_machine_ram_options
WHERE mame_machine_ram_options.set_id=requested.set_id
UNION ALL
SELECT asset_occurrences.record_id AS set_id, mame_rom_claims.source_order, 'rom' AS child_kind, asset_occurrences.occurrence_order AS child_order
FROM requested_mame_machines AS requested
CROSS JOIN asset_occurrences CROSS JOIN mame_rom_claims
WHERE asset_occurrences.record_id=requested.set_id AND asset_occurrences.claim_kind='mame_rom'
AND mame_rom_claims.occurrence_id=asset_occurrences.occurrence_id
UNION ALL
SELECT asset_occurrences.record_id AS set_id, mame_disk_claims.source_order, 'disk' AS child_kind, asset_occurrences.occurrence_order AS child_order
FROM requested_mame_machines AS requested
CROSS JOIN asset_occurrences CROSS JOIN mame_disk_claims
WHERE asset_occurrences.record_id=requested.set_id AND asset_occurrences.claim_kind='mame_disk'
AND mame_disk_claims.occurrence_id=asset_occurrences.occurrence_id
