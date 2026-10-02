// Native-family rules are compiled into the same canonical DDL as all consumers.
// The macros share enforcement mechanics, never source fact storage.
macro_rules! mame_publication_readiness {
    () => {
        concat!(
            "requested_mame_machines(set_id) AS (",
            "SELECT sets.set_id FROM requested JOIN catalog_set_groups USING(snapshot_key) ",
            "JOIN catalog_sets AS sets USING(set_group_id)), positions AS (",
            include_str!("mame_positions.sql"),
            ") ",
            include_str!("mame_publication_readiness.sql")
        )
    };
}

macro_rules! mame_insert_guard {
    ($table:literal,$identity:literal,$owner:literal,$published:literal,$invalid:literal) => {
        concat!(
            "CREATE TRIGGER ",$table,"_complete_native_insert BEFORE INSERT ON ",$table,
            " WHEN EXISTS(SELECT 1 FROM ",$table," WHERE ",$identity,") OR NOT EXISTS(",$owner,
            ") OR EXISTS(",$published,") OR (",$invalid,
            ") BEGIN SELECT RAISE(ABORT,'native MAME owner, immutable identity or source state is invalid'); END;\n"
        )
    };
}
macro_rules! mame_immutable_guard {
    ($table:literal) => {
        concat!(
            "CREATE TRIGGER ",$table,"_native_update BEFORE UPDATE ON ",$table,
            " BEGIN SELECT RAISE(ABORT,'native MAME facts are immutable'); END;\n",
            "CREATE TRIGGER ",$table,"_native_delete BEFORE DELETE ON ",$table,
            " BEGIN SELECT RAISE(ABORT,'native MAME facts are immutable'); END;\n"
        )
    };
}
macro_rules! mame_guards {
    () => {
        concat!(
            mame_insert_guard!("mame_machine_samples","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_samples"),
            mame_insert_guard!("mame_machine_chips","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_chips"),
            mame_insert_guard!("mame_machine_displays","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR (NEW.flip_x_specified=0 AND NEW.flip_x<>0)"),
            mame_immutable_guard!("mame_machine_displays"),
            mame_insert_guard!("mame_machine_sounds","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_sounds"),
            mame_insert_guard!("mame_machine_inputs","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR (NEW.service_specified=0 AND NEW.service<>0) OR (NEW.tilt_specified=0 AND NEW.tilt<>0)"),
            mame_immutable_guard!("mame_machine_inputs"),
            mame_insert_guard!("mame_machine_ports","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_ports"),
            mame_insert_guard!("mame_machine_adjusters","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_adjusters"),
            mame_insert_guard!("mame_machine_drivers","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR (NEW.requires_artwork_specified=0 AND NEW.requires_artwork<>0) OR (NEW.unofficial_specified=0 AND NEW.unofficial<>0) OR (NEW.no_sound_hardware_specified=0 AND NEW.no_sound_hardware<>0) OR (NEW.incomplete_specified=0 AND NEW.incomplete<>0)"),
            mame_immutable_guard!("mame_machine_drivers"),
            mame_insert_guard!("mame_machine_features","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_features"),
            mame_insert_guard!("mame_machine_devices","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_devices"),
            mame_insert_guard!("mame_machine_slots","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_slots"),
            mame_insert_guard!("mame_machine_software_lists","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_software_lists"),
            mame_insert_guard!("mame_machine_ram_options","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0"),
            mame_immutable_guard!("mame_machine_ram_options"),
            mame_insert_guard!("mame_machine_input_controls","set_id=NEW.set_id AND element_order=NEW.element_order AND control_order=NEW.control_order","SELECT 1 FROM mame_machine_inputs WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR typeof(NEW.control_order)<>'integer' OR NEW.control_order<0 OR (NEW.reverse_specified=0 AND NEW.reverse<>0)"),
            mame_immutable_guard!("mame_machine_input_controls"),
            mame_insert_guard!("mame_machine_analogs","set_id=NEW.set_id AND element_order=NEW.element_order AND analog_order=NEW.analog_order","SELECT 1 FROM mame_machine_ports WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR typeof(NEW.analog_order)<>'integer' OR NEW.analog_order<0 OR (0)"),
            mame_immutable_guard!("mame_machine_analogs"),
            mame_insert_guard!("mame_machine_device_instances","set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM mame_machine_devices WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR (0)"),
            mame_immutable_guard!("mame_machine_device_instances"),
            mame_insert_guard!("mame_machine_device_extensions","set_id=NEW.set_id AND element_order=NEW.element_order AND extension_order=NEW.extension_order","SELECT 1 FROM mame_machine_devices WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR typeof(NEW.extension_order)<>'integer' OR NEW.extension_order<0 OR (0)"),
            mame_immutable_guard!("mame_machine_device_extensions"),
            mame_insert_guard!("mame_machine_slot_options","set_id=NEW.set_id AND element_order=NEW.element_order AND option_order=NEW.option_order","SELECT 1 FROM mame_machine_slots WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR typeof(NEW.option_order)<>'integer' OR NEW.option_order<0 OR (NEW.default_specified=0 AND NEW.is_default<>0)"),
            mame_immutable_guard!("mame_machine_slot_options"),
            mame_insert_guard!("mame_machine_adjuster_conditions","set_id=NEW.set_id AND element_order=NEW.element_order AND condition_order=NEW.condition_order","SELECT 1 FROM mame_machine_adjusters WHERE set_id=NEW.set_id AND element_order=NEW.element_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.element_order)<>'integer' OR NEW.element_order<0 OR typeof(NEW.condition_order)<>'integer' OR NEW.condition_order<0 OR (NEW.condition_order<>0)"),
            mame_immutable_guard!("mame_machine_adjuster_conditions"),
            mame_insert_guard!("machine_switch_conditions","set_id=NEW.set_id AND switch_order=NEW.switch_order AND condition_order=NEW.condition_order","SELECT 1 FROM machine_switches WHERE set_id=NEW.set_id AND switch_order=NEW.switch_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.switch_order)<>'integer' OR NEW.switch_order<0 OR typeof(NEW.condition_order)<>'integer' OR NEW.condition_order<0 OR (NEW.condition_order<>0)"),
            mame_immutable_guard!("machine_switch_conditions"),
            mame_insert_guard!("machine_switch_value_conditions","set_id=NEW.set_id AND switch_order=NEW.switch_order AND value_order=NEW.value_order AND condition_order=NEW.condition_order","SELECT 1 FROM machine_switch_values WHERE set_id=NEW.set_id AND switch_order=NEW.switch_order AND value_order=NEW.value_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.switch_order)<>'integer' OR NEW.switch_order<0 OR typeof(NEW.value_order)<>'integer' OR NEW.value_order<0 OR typeof(NEW.condition_order)<>'integer' OR NEW.condition_order<0 OR (NEW.condition_order<>0)"),
            mame_immutable_guard!("machine_switch_value_conditions"),
            mame_insert_guard!("mame_machines","set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN catalog_snapshots USING(snapshot_key) JOIN parser_interpretations USING(interpretation_key) WHERE set_id=NEW.set_id AND source_element_kind='mame_machine' AND format='mame-listxml'","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","(NEW.is_device_specified=0 AND NEW.is_device<>0) OR (NEW.runnable_specified=0 AND NEW.runnable<>1) OR (NEW.is_bios_specified=0 AND NEW.is_bios<>0) OR (NEW.is_mechanical_specified=0 AND NEW.is_mechanical<>0) OR (NEW.is_consumable_specified=0 AND NEW.is_consumable<>0)"),
            mame_insert_guard!("mame_bios_sets","set_id=NEW.set_id AND bios_order=NEW.bios_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.bios_order)<>'integer' OR (NEW.default_specified=0 AND NEW.is_default<>0)"),
            mame_insert_guard!("mame_machine_dependencies","set_id=NEW.set_id AND dependency_order=NEW.dependency_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.dependency_order)<>'integer'"),
            mame_insert_guard!("machine_switches","set_id=NEW.set_id AND switch_order=NEW.switch_order","SELECT 1 FROM mame_machines WHERE set_id=NEW.set_id","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.switch_order)<>'integer'"),
            mame_insert_guard!("machine_switch_locations","set_id=NEW.set_id AND switch_order=NEW.switch_order AND location_order=NEW.location_order","SELECT 1 FROM machine_switches WHERE set_id=NEW.set_id AND switch_order=NEW.switch_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.switch_order)<>'integer' OR typeof(NEW.location_order)<>'integer' OR (NEW.inverted_specified=0 AND NEW.inverted<>0)"),
            mame_insert_guard!("machine_switch_values","set_id=NEW.set_id AND switch_order=NEW.switch_order AND value_order=NEW.value_order","SELECT 1 FROM machine_switches WHERE set_id=NEW.set_id AND switch_order=NEW.switch_order","SELECT 1 FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE set_id=NEW.set_id","typeof(NEW.switch_order)<>'integer' OR typeof(NEW.value_order)<>'integer' OR (NEW.default_specified=0 AND NEW.is_default<>0)"),
            mame_insert_guard!("mame_document_facts","snapshot_key=NEW.snapshot_key","SELECT 1 FROM catalog_snapshots JOIN parser_interpretations USING(interpretation_key) WHERE snapshot_key=NEW.snapshot_key AND format='mame-listxml'","SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key","NEW.debug_specified=0 AND NEW.debug<>0"),
            mame_insert_guard!("mame_rom_claims","occurrence_id=NEW.occurrence_id","SELECT 1 FROM asset_occurrences JOIN mame_machines ON mame_machines.set_id=asset_occurrences.record_id WHERE occurrence_id=NEW.occurrence_id AND claim_kind='mame_rom'","SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE occurrence_id=NEW.occurrence_id","NEW.dump_status IS NULL OR NEW.dump_status NOT IN ('good','baddump','nodump') OR (NEW.status_specified=0 AND NEW.dump_status<>'good') OR NEW.optional IS NULL OR (NEW.optional_specified=0 AND NEW.optional<>0) OR ((NEW.writable IS NULL)<>(NEW.writable_specified=0))"),
            mame_insert_guard!("mame_disk_claims","occurrence_id=NEW.occurrence_id","SELECT 1 FROM asset_occurrences JOIN mame_machines ON mame_machines.set_id=asset_occurrences.record_id WHERE occurrence_id=NEW.occurrence_id AND claim_kind='mame_disk'","SELECT 1 FROM asset_occurrences JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) WHERE occurrence_id=NEW.occurrence_id","NEW.dump_status IS NULL OR NEW.dump_status NOT IN ('good','baddump','nodump') OR (NEW.status_specified=0 AND NEW.dump_status<>'good') OR NEW.optional IS NULL OR (NEW.optional_specified=0 AND NEW.optional<>0) OR NEW.writable IS NULL OR (NEW.writable_specified=0 AND NEW.writable<>0)"),
        )
    };
}
