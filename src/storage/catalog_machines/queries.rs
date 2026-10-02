pub(super) const PUBLISHED_SNAPSHOT: &str = "
SELECT registry.registry_uuid, snapshots.snapshot_key, sources.source_key,
       sources.display_name AS source_name, catalogs.catalog_key,
       catalogs.display_name AS catalog_name, documents.document_key,
       snapshots.interpretation_key, interpretations.format,
       facts.build, facts.debug, facts.debug_specified, facts.config_version,
       facts.source_line AS header_line, facts.source_column AS header_column
FROM catalog_snapshots AS snapshots
JOIN snapshot_publications AS publication
  ON publication.snapshot_key = snapshots.snapshot_key
 AND publication.catalog_key = snapshots.catalog_key
 AND publication.document_key = snapshots.document_key
 AND publication.interpretation_key = snapshots.interpretation_key
JOIN catalogs ON catalogs.catalog_key = snapshots.catalog_key
JOIN publishing_sources AS sources ON sources.source_key = catalogs.source_key
JOIN documents ON documents.document_key = snapshots.document_key
JOIN parser_interpretations AS interpretations
  ON interpretations.interpretation_key = snapshots.interpretation_key
JOIN mame_document_facts AS facts ON facts.snapshot_key = snapshots.snapshot_key
CROSS JOIN file_id_registries AS registry
WHERE snapshots.snapshot_key = ? AND registry.registry_id = 1
  AND interpretations.format = 'mame-listxml'
";

const MACHINE_COLUMNS: &str = "
SELECT sets.set_id AS id, sets.set_name AS name, sets.list_order,
       sets.source_line AS line, sets.source_column AS column,
       native.set_id AS native_id, native.source_file, native.description,
       native.description_source_order,
       native.description_line, native.description_column,
       native.year, native.year_source_order, native.year_line, native.year_column,
       native.manufacturer, native.manufacturer_source_order,
       native.manufacturer_line, native.manufacturer_column,
       native.is_device, native.is_device_specified, native.runnable,
       native.runnable_specified, native.is_bios, native.is_bios_specified,
       native.is_mechanical, native.is_mechanical_specified,
       COALESCE(compatibility.is_consumable, 0) AS is_consumable,
       COALESCE(compatibility.is_consumable_specified, 0) AS is_consumable_specified,
       native.attributes_line, native.attributes_column,
       groups.snapshot_key = ? AS group_snapshot_ok, groups.kind AS group_kind,
       sets.source_element_kind
FROM catalog_set_groups AS groups
JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id
LEFT JOIN mame_machines AS native ON native.set_id = sets.set_id
LEFT JOIN mame_machine_compatibility AS compatibility
  ON compatibility.set_id = native.set_id
WHERE groups.snapshot_key = ? AND groups.kind = 'root'
  AND sets.source_element_kind = 'mame_machine'
";

pub(super) fn first_machine_page() -> String {
    format!("{MACHINE_COLUMNS} ORDER BY sets.list_order, sets.set_id LIMIT ?")
}

pub(super) fn next_machine_page() -> String {
    format!(
        "{MACHINE_COLUMNS} AND (sets.list_order, sets.set_id) > (?, ?) \
         ORDER BY sets.list_order, sets.set_id LIMIT ?"
    )
}

pub(super) const CREATE_REQUESTED_OWNERS: &str = "CREATE TEMP TABLE IF NOT EXISTS catalog_machine_requested_owners (owner_id INTEGER PRIMARY KEY) WITHOUT ROWID; DELETE FROM temp.catalog_machine_requested_owners";
pub(super) const INSERT_REQUESTED_OWNER: &str =
    "INSERT INTO temp.catalog_machine_requested_owners(owner_id) VALUES (?)";
pub(super) const DROP_REQUESTED_OWNERS: &str = "DROP TABLE temp.catalog_machine_requested_owners";

pub(super) const CHILD_POSITIONS: &str = concat!(
    "WITH requested_mame_machines(set_id) AS (\
       SELECT owner_id FROM temp.catalog_machine_requested_owners\
     ), positions AS (",
    include_str!("../db/mame_positions.sql"),
    ") SELECT set_id AS owner_id, source_order, child_kind, child_order \
     FROM positions ORDER BY set_id, source_order"
);

pub(super) const BIOS_SETS: &str = "
SELECT native.set_id AS owner_id, native.bios_order AS row_order,
       native.name, native.description, native.is_default,
       native.default_specified, native.source_order,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_bios_sets AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.bios_order
";

pub(super) const CLONE_LINKS: &str = "
SELECT native.set_id AS owner_id, native.target_name,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_links AS native
WHERE native.set_id = requested.owner_id AND native.link_kind = 'cloneof'
ORDER BY native.set_id
";

pub(super) const DEPENDENCIES: &str = "
SELECT native.set_id AS owner_id, native.dependency_order AS row_order,
       native.dependency_kind, native.target_name, native.reference_tag,
       native.source_order, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_dependencies AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.dependency_order
";

pub(super) const ROM_ASSETS: &str = "
SELECT occurrences.record_id AS owner_id, occurrences.occurrence_id,
       occurrences.occurrence_order AS child_order, claims.source_order,
       claims.source_line AS line, claims.source_column AS column,
       occurrences.claim_kind
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN asset_occurrences AS occurrences
LEFT JOIN mame_rom_claims AS claims USING (occurrence_id)
WHERE occurrences.record_id = requested.owner_id
  AND occurrences.claim_kind = 'mame_rom'
ORDER BY occurrences.record_id, occurrences.occurrence_order
";

pub(super) const DISK_ASSETS: &str = "
SELECT occurrences.record_id AS owner_id, occurrences.occurrence_id,
       occurrences.occurrence_order AS child_order, claims.source_order,
       claims.source_line AS line, claims.source_column AS column,
       occurrences.claim_kind
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN asset_occurrences AS occurrences
LEFT JOIN mame_disk_claims AS claims USING (occurrence_id)
WHERE occurrences.record_id = requested.owner_id
  AND occurrences.claim_kind = 'mame_disk'
ORDER BY occurrences.record_id, occurrences.occurrence_order
";

pub(super) const SWITCHES: &str = "
SELECT native.set_id AS owner_id, native.switch_order AS row_order,
       native.kind, native.name, native.tag, native.mask, native.source_order,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN machine_switches AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.switch_order
";

pub(super) const SWITCH_CONDITIONS: &str = "
SELECT native.set_id AS owner_id, native.switch_order,
       native.condition_order AS child_order, native.condition_order,
       native.tag, native.mask, native.relation, native.value,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN machine_switch_conditions AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.switch_order, native.condition_order
";

pub(super) const SWITCH_LOCATIONS: &str = "
SELECT native.set_id AS owner_id, native.switch_order,
       native.location_order AS row_order, native.source_order,
       native.name, native.number, native.inverted, native.inverted_specified,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN machine_switch_locations AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.switch_order, native.location_order
";

pub(super) const SWITCH_VALUES: &str = "
SELECT native.set_id AS owner_id, native.switch_order,
       native.value_order AS row_order, native.source_order, native.name,
       native.value, native.is_default, native.default_specified,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN machine_switch_values AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.switch_order, native.value_order
";

pub(super) const SWITCH_VALUE_CONDITIONS: &str = "
SELECT native.set_id AS owner_id, native.switch_order,
       native.value_order AS child_order, native.condition_order,
       native.tag, native.mask, native.relation,
       native.value, native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN machine_switch_value_conditions AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.switch_order, native.value_order,
         native.condition_order
";

pub(super) const SPEC_SAMPLES: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_samples AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_CHIPS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name, native.tag,
       native.kind, native.clock, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_chips AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_DISPLAYS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.tag, native.kind,
       native.rotation, native.flip_x, native.flip_x_specified, native.width,
       native.height, native.refresh, native.pixel_clock,
       native.horizontal_total, native.horizontal_blank_end,
       native.horizontal_blank_start, native.vertical_total,
       native.vertical_blank_end, native.vertical_blank_start,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_displays AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_SOUNDS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.channels,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_sounds AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_INPUTS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.service,
       native.service_specified, native.tilt, native.tilt_specified,
       native.players, native.coins, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_inputs AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_CONTROLS: &str = "
SELECT native.set_id AS owner_id, native.element_order,
       native.control_order AS row_order, native.control_type, native.player,
       native.buttons, native.minimum, native.maximum, native.sensitivity,
       native.keydelta, native.reverse, native.reverse_specified,
       native.ways, native.ways2, native.ways3,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_input_controls AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order, native.control_order
";
pub(super) const SPEC_PORTS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.tag,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_ports AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_ANALOGS: &str = "
SELECT native.set_id AS owner_id, native.element_order,
       native.analog_order AS row_order, native.mask,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_analogs AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order, native.analog_order
";
pub(super) const SPEC_ADJUSTERS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name,
       native.default_value, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_adjusters AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_ADJUSTER_CONDITIONS: &str = "
SELECT native.set_id AS owner_id, native.element_order AS switch_order,
       native.condition_order AS child_order, native.condition_order,
       native.tag, native.mask, native.relation,
       native.value, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_adjuster_conditions AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order, native.condition_order
";
pub(super) const SPEC_DRIVERS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.status,
       native.emulation, native.cocktail, native.savestate,
       native.requires_artwork, native.requires_artwork_specified,
       native.unofficial, native.unofficial_specified,
       native.no_sound_hardware, native.no_sound_hardware_specified,
       native.incomplete, native.incomplete_specified,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_drivers AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_FEATURES: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.kind,
       native.status, native.overall, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_features AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_DEVICES: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.kind,
       native.tag, native.fixed_image, native.mandatory, native.interface,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_devices AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_DEVICE_INSTANCES: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name,
       native.brief_name, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_device_instances AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_DEVICE_EXTENSIONS: &str = "
SELECT native.set_id AS owner_id, native.element_order,
       native.extension_order AS row_order, native.name,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_device_extensions AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order, native.extension_order
";
pub(super) const SPEC_SLOTS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_slots AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_SLOT_OPTIONS: &str = "
SELECT native.set_id AS owner_id, native.element_order,
       native.option_order AS row_order, native.name, native.devname,
       native.is_default, native.default_specified,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_slot_options AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order, native.option_order
";
pub(super) const SPEC_SOFTWARE_LISTS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.tag,
       native.name, native.status, native.filter,
       native.source_line AS line, native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_software_lists AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
pub(super) const SPEC_RAM_OPTIONS: &str = "
SELECT native.set_id AS owner_id, native.element_order, native.name,
       native.default_value, native.text, native.source_line AS line,
       native.source_column AS column
FROM temp.catalog_machine_requested_owners AS requested
CROSS JOIN mame_machine_ram_options AS native
WHERE native.set_id = requested.owner_id
ORDER BY native.set_id, native.element_order
";
