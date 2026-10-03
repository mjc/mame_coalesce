-- Position-only companions named for their actual typed value owners.
CREATE TABLE mame_document_facts_attribute_positions (
 document_id INTEGER NOT NULL CHECK(typeof(document_id)='integer' AND document_id>0),
 field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2),
 source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
 source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(document_id,field_kind), UNIQUE(document_id,source_order),
 FOREIGN KEY(document_id) REFERENCES mame_document_facts(document_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machines_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 8),
 source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,field_kind), UNIQUE(set_id,source_order), FOREIGN KEY(set_id) REFERENCES mame_machines(set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_bios_sets_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), bios_order INTEGER NOT NULL CHECK(typeof(bios_order)='integer' AND bios_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,bios_order,field_kind), UNIQUE(set_id,bios_order,source_order), FOREIGN KEY(set_id,bios_order) REFERENCES mame_bios_sets(set_id,bios_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_rom_claims_attribute_positions (
 occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 9), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order), FOREIGN KEY(occurrence_id) REFERENCES mame_rom_claims(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_disk_claims_attribute_positions (
 occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 7), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order), FOREIGN KEY(occurrence_id) REFERENCES mame_disk_claims(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_device_references_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), reference_order INTEGER NOT NULL CHECK(typeof(reference_order)='integer' AND reference_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,reference_order,field_kind), UNIQUE(set_id,reference_order,source_order), FOREIGN KEY(set_id,reference_order) REFERENCES mame_device_references(set_id,reference_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_samples_attribute_positions (
 occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order), FOREIGN KEY(occurrence_id) REFERENCES mame_samples(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_chips_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_chips(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_displays_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 13), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_displays(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_sounds_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_sounds(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_inputs_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_inputs(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_input_controls_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), control_order INTEGER NOT NULL CHECK(typeof(control_order)='integer' AND control_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 10), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,control_order,field_kind), UNIQUE(set_id,element_order,control_order,source_order), FOREIGN KEY(set_id,element_order,control_order) REFERENCES mame_machine_input_controls(set_id,element_order,control_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switches_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), switch_order INTEGER NOT NULL CHECK(typeof(switch_order)='integer' AND switch_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,switch_order,field_kind), UNIQUE(set_id,switch_order,source_order), FOREIGN KEY(set_id,switch_order) REFERENCES machine_switches(set_id,switch_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switch_locations_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), switch_order INTEGER NOT NULL CHECK(typeof(switch_order)='integer' AND switch_order>=0), location_order INTEGER NOT NULL CHECK(typeof(location_order)='integer' AND location_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,switch_order,location_order,field_kind), UNIQUE(set_id,switch_order,location_order,source_order), FOREIGN KEY(set_id,switch_order,location_order) REFERENCES machine_switch_locations(set_id,switch_order,location_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switch_values_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), switch_order INTEGER NOT NULL CHECK(typeof(switch_order)='integer' AND switch_order>=0), value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,switch_order,value_order,field_kind), UNIQUE(set_id,switch_order,value_order,source_order), FOREIGN KEY(set_id,switch_order,value_order) REFERENCES machine_switch_values(set_id,switch_order,value_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switch_conditions_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), switch_order INTEGER NOT NULL CHECK(typeof(switch_order)='integer' AND switch_order>=0), condition_order INTEGER NOT NULL CHECK(typeof(condition_order)='integer' AND condition_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,switch_order,condition_order,field_kind), UNIQUE(set_id,switch_order,condition_order,source_order), FOREIGN KEY(set_id,switch_order,condition_order) REFERENCES machine_switch_conditions(set_id,switch_order,condition_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE machine_switch_value_conditions_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), switch_order INTEGER NOT NULL CHECK(typeof(switch_order)='integer' AND switch_order>=0), value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0), condition_order INTEGER NOT NULL CHECK(typeof(condition_order)='integer' AND condition_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,switch_order,value_order,condition_order,field_kind), UNIQUE(set_id,switch_order,value_order,condition_order,source_order), FOREIGN KEY(set_id,switch_order,value_order,condition_order) REFERENCES machine_switch_value_conditions(set_id,switch_order,value_order,condition_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_adjuster_conditions_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), condition_order INTEGER NOT NULL CHECK(typeof(condition_order)='integer' AND condition_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,condition_order,field_kind), UNIQUE(set_id,element_order,condition_order,source_order), FOREIGN KEY(set_id,element_order,condition_order) REFERENCES mame_machine_adjuster_conditions(set_id,element_order,condition_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_ports_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_ports(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_analogs_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), analog_order INTEGER NOT NULL CHECK(typeof(analog_order)='integer' AND analog_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,analog_order,field_kind), UNIQUE(set_id,element_order,analog_order,source_order), FOREIGN KEY(set_id,element_order,analog_order) REFERENCES mame_machine_analogs(set_id,element_order,analog_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_adjusters_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_adjusters(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_drivers_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 7), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_drivers(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_features_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_features(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_devices_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 4), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_devices(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_device_instances_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_device_instances(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_device_extensions_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), extension_order INTEGER NOT NULL CHECK(typeof(extension_order)='integer' AND extension_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,extension_order,field_kind), UNIQUE(set_id,element_order,extension_order,source_order), FOREIGN KEY(set_id,element_order,extension_order) REFERENCES mame_machine_device_extensions(set_id,element_order,extension_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_slots_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_slots(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_slot_options_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), option_order INTEGER NOT NULL CHECK(typeof(option_order)='integer' AND option_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,option_order,field_kind), UNIQUE(set_id,element_order,option_order,source_order), FOREIGN KEY(set_id,element_order,option_order) REFERENCES mame_machine_slot_options(set_id,element_order,option_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_software_lists_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_software_lists(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_ram_options_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), element_order INTEGER NOT NULL CHECK(typeof(element_order)='integer' AND element_order>=0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,element_order,field_kind), UNIQUE(set_id,element_order,source_order), FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_ram_options(set_id,element_order) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_machine_compatibility_attribute_positions (
 set_id INTEGER NOT NULL CHECK(typeof(set_id)='integer' AND set_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(set_id,field_kind), UNIQUE(set_id,source_order), FOREIGN KEY(set_id) REFERENCES mame_machine_compatibility(set_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_rom_compatibility_attribute_positions (
 occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 7), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order), FOREIGN KEY(occurrence_id) REFERENCES mame_rom_compatibility(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE mame_disk_compatibility_attribute_positions (
 occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0), field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind=0), source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0), source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0), source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
 PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order), FOREIGN KEY(occurrence_id) REFERENCES mame_disk_compatibility(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;

-- Edition predicates reach each branch before native-owner lookup.
-- Invalid SQLite storage types or coordinates become an impossible field code,
-- so both publication and source-free history reject them through the same set check.
CREATE VIEW mame_actual_attribute_positions AS
SELECT document.snapshot_key AS snapshot_key,0 AS family,position.document_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.document_id)='integer' AND position.document_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM mame_document_facts AS document CROSS JOIN mame_document_facts_attribute_positions AS position
WHERE position.document_id=document.document_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,1 AS family,position.set_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 8
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machines_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,2 AS family,position.set_id AS owner_a,position.bios_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.bios_order)='integer' AND position.bios_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_bios_sets_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,3 AS family,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 9
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_rom_claims_attribute_positions AS position ON position.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,4 AS family,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 7
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_disk_claims_attribute_positions AS position ON position.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,5 AS family,position.set_id AS owner_a,position.reference_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.reference_order)='integer' AND position.reference_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 1
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_device_references_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,6 AS family,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_samples_attribute_positions AS position ON position.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,7 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_chips_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,8 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 13
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_displays_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,9 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_sounds_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,10 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_inputs_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,11 AS family,position.set_id AS owner_a,position.element_order AS owner_b,position.control_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.control_order)='integer' AND position.control_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 10
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_input_controls_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,12 AS family,position.set_id AS owner_a,position.switch_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.switch_order)='integer' AND position.switch_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN machine_switches_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,13 AS family,position.set_id AS owner_a,position.switch_order AS owner_b,position.location_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.switch_order)='integer' AND position.switch_order>=0
    AND typeof(position.location_order)='integer' AND position.location_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN machine_switch_locations_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,14 AS family,position.set_id AS owner_a,position.switch_order AS owner_b,position.value_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.switch_order)='integer' AND position.switch_order>=0
    AND typeof(position.value_order)='integer' AND position.value_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN machine_switch_values_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,15 AS family,position.set_id AS owner_a,position.switch_order AS owner_b,position.condition_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.switch_order)='integer' AND position.switch_order>=0
    AND typeof(position.condition_order)='integer' AND position.condition_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN machine_switch_conditions_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,16 AS family,position.set_id AS owner_a,position.switch_order AS owner_b,position.value_order AS owner_c,position.condition_order AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.switch_order)='integer' AND position.switch_order>=0
    AND typeof(position.value_order)='integer' AND position.value_order>=0
    AND typeof(position.condition_order)='integer' AND position.condition_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN machine_switch_value_conditions_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,17 AS family,position.set_id AS owner_a,position.element_order AS owner_b,position.condition_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.condition_order)='integer' AND position.condition_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_adjuster_conditions_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,18 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_ports_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,19 AS family,position.set_id AS owner_a,position.element_order AS owner_b,position.analog_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.analog_order)='integer' AND position.analog_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_analogs_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,20 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 1
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_adjusters_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,21 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 7
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_drivers_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,22 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_features_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,23 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 4
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_devices_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,24 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 1
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_device_instances_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,25 AS family,position.set_id AS owner_a,position.element_order AS owner_b,position.extension_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.extension_order)='integer' AND position.extension_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_device_extensions_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,26 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_slots_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,27 AS family,position.set_id AS owner_a,position.element_order AS owner_b,position.option_order AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.option_order)='integer' AND position.option_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 2
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_slot_options_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,28 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 3
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_software_lists_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,29 AS family,position.set_id AS owner_a,position.element_order AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.element_order)='integer' AND position.element_order>=0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 1
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_ram_options_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,30 AS family,position.set_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.set_id)='integer' AND position.set_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machine_compatibility_attribute_positions AS position ON position.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,31 AS family,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 7
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_rom_compatibility_attribute_positions AS position ON position.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,32 AS family,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,
 CASE WHEN typeof(position.occurrence_id)='integer' AND position.occurrence_id>0
    AND typeof(position.field_kind)='integer' AND position.field_kind BETWEEN 0 AND 0
    AND typeof(position.source_order)='integer' AND position.source_order>=0
    AND typeof(position.source_line)='integer' AND position.source_line>0
    AND typeof(position.source_column)='integer' AND position.source_column>0 THEN position.field_kind ELSE -1 END AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_disk_compatibility_attribute_positions AS position ON position.occurrence_id=occurrence.occurrence_id;

CREATE VIEW mame_expected_attribute_positions AS
SELECT snapshot.snapshot_key,0 AS family,owner.document_id AS owner_a,0 AS owner_b,0 AS owner_c,0 AS owner_d,field.field_kind
FROM catalog_snapshots AS snapshot CROSS JOIN mame_document_facts AS owner INDEXED BY sqlite_autoindex_mame_document_facts_1 ON owner.snapshot_key=snapshot.snapshot_key CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.build IS NOT NULL WHEN 1 THEN owner.debug_specified=1 WHEN 2 THEN owner.config_version IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,1,owner.set_id,0,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machines AS owner ON owner.set_id=sets.set_id
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.source_file IS NOT NULL WHEN 2 THEN owner.is_bios_specified=1 WHEN 3 THEN owner.is_device_specified=1 WHEN 4 THEN owner.is_mechanical_specified=1 WHEN 5 THEN owner.runnable_specified=1 WHEN 6 THEN EXISTS(SELECT 1 FROM mame_machine_links link WHERE link.set_id=owner.set_id AND link.link_kind='cloneof') WHEN 7 THEN EXISTS(SELECT 1 FROM mame_machine_links link WHERE link.set_id=owner.set_id AND link.link_kind='romof') WHEN 8 THEN EXISTS(SELECT 1 FROM mame_machine_links link WHERE link.set_id=owner.set_id AND link.link_kind='sampleof') ELSE 0 END
UNION ALL
SELECT groups.snapshot_key,2,owner.set_id,owner.bios_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_bios_sets AS owner ON owner.set_id=sets.set_id
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field WHERE field.field_kind<2 OR owner.default_specified=1
UNION ALL
SELECT groups.snapshot_key,3,owner.occurrence_id,0,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id CROSS JOIN mame_rom_claims AS owner ON owner.occurrence_id=occurrence.occurrence_id
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.bios IS NOT NULL WHEN 2 THEN owner.size_text IS NOT NULL WHEN 3 THEN owner.crc_text IS NOT NULL WHEN 4 THEN owner.sha1_text IS NOT NULL WHEN 5 THEN EXISTS(SELECT 1 FROM mame_rom_merges merge WHERE merge.occurrence_id=owner.occurrence_id) WHEN 6 THEN owner.region IS NOT NULL WHEN 7 THEN owner.offset_text IS NOT NULL WHEN 8 THEN owner.status_specified=1 WHEN 9 THEN owner.optional_specified=1 ELSE 0 END
UNION ALL
SELECT groups.snapshot_key,4,owner.occurrence_id,0,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id CROSS JOIN mame_disk_claims AS owner ON owner.occurrence_id=occurrence.occurrence_id
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.sha1_text IS NOT NULL WHEN 2 THEN EXISTS(SELECT 1 FROM mame_disk_merges merge WHERE merge.occurrence_id=owner.occurrence_id) WHEN 3 THEN owner.region IS NOT NULL WHEN 4 THEN owner.disk_index IS NOT NULL WHEN 5 THEN owner.writable_specified=1 WHEN 6 THEN owner.status_specified=1 WHEN 7 THEN owner.optional_specified=1 ELSE 0 END
UNION ALL
SELECT groups.snapshot_key,5,owner.set_id,owner.reference_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_device_references AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1) AS field
UNION ALL
SELECT groups.snapshot_key,6,owner.occurrence_id,0,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id CROSS JOIN mame_samples AS owner ON owner.occurrence_id=occurrence.occurrence_id
UNION ALL
SELECT groups.snapshot_key,7,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_chips AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.tag IS NOT NULL WHEN 2 THEN owner.kind IS NOT NULL WHEN 3 THEN owner.clock IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,8,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_displays AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9 UNION ALL SELECT 10 UNION ALL SELECT 11 UNION ALL SELECT 12 UNION ALL SELECT 13) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.tag IS NOT NULL WHEN 1 THEN owner.kind IS NOT NULL WHEN 2 THEN owner.rotation IS NOT NULL WHEN 3 THEN owner.flip_x_specified=1 WHEN 4 THEN owner.width IS NOT NULL WHEN 5 THEN owner.height IS NOT NULL WHEN 6 THEN owner.refresh IS NOT NULL WHEN 7 THEN owner.pixel_clock IS NOT NULL WHEN 8 THEN owner.horizontal_total IS NOT NULL WHEN 9 THEN owner.horizontal_blank_end IS NOT NULL WHEN 10 THEN owner.horizontal_blank_start IS NOT NULL WHEN 11 THEN owner.vertical_total IS NOT NULL WHEN 12 THEN owner.vertical_blank_end IS NOT NULL WHEN 13 THEN owner.vertical_blank_start IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,9,owner.set_id,owner.element_order,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_sounds AS owner ON owner.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key,10,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_inputs AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.service_specified=1 WHEN 1 THEN owner.tilt_specified=1 WHEN 2 THEN owner.players IS NOT NULL WHEN 3 THEN owner.coins IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,11,owner.set_id,owner.element_order,owner.control_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_input_controls AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9 UNION ALL SELECT 10) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.player IS NOT NULL WHEN 2 THEN owner.buttons IS NOT NULL WHEN 3 THEN owner.minimum IS NOT NULL WHEN 4 THEN owner.maximum IS NOT NULL WHEN 5 THEN owner.sensitivity IS NOT NULL WHEN 6 THEN owner.keydelta IS NOT NULL WHEN 7 THEN owner.reverse_specified=1 WHEN 8 THEN owner.ways IS NOT NULL WHEN 9 THEN owner.ways2 IS NOT NULL WHEN 10 THEN owner.ways3 IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,12,owner.set_id,owner.switch_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN machine_switches AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
UNION ALL
SELECT groups.snapshot_key,13,owner.set_id,owner.switch_order,owner.location_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN machine_switch_locations AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN 1 WHEN 2 THEN owner.inverted_specified=1 END
UNION ALL
SELECT groups.snapshot_key,14,owner.set_id,owner.switch_order,owner.value_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN machine_switch_values AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN 1 WHEN 2 THEN owner.default_specified=1 END
UNION ALL
SELECT groups.snapshot_key,15,owner.set_id,owner.switch_order,owner.condition_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN machine_switch_conditions AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
UNION ALL
SELECT groups.snapshot_key,16,owner.set_id,owner.switch_order,owner.value_order,owner.condition_order,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN machine_switch_value_conditions AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
UNION ALL
SELECT groups.snapshot_key,17,owner.set_id,owner.element_order,owner.condition_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_adjuster_conditions AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
UNION ALL
SELECT groups.snapshot_key,18,owner.set_id,owner.element_order,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_ports AS owner ON owner.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key,19,owner.set_id,owner.element_order,owner.analog_order,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_analogs AS owner ON owner.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key,20,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_adjusters AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1) AS field
UNION ALL
SELECT groups.snapshot_key,21,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_drivers AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.status IS NOT NULL WHEN 1 THEN owner.emulation IS NOT NULL WHEN 2 THEN owner.cocktail IS NOT NULL WHEN 3 THEN owner.savestate IS NOT NULL WHEN 4 THEN owner.requires_artwork_specified=1 WHEN 5 THEN owner.unofficial_specified=1 WHEN 6 THEN owner.no_sound_hardware_specified=1 WHEN 7 THEN owner.incomplete_specified=1 END
UNION ALL
SELECT groups.snapshot_key,22,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_features AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.kind IS NOT NULL WHEN 1 THEN owner.status IS NOT NULL WHEN 2 THEN owner.overall IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,23,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_devices AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.kind IS NOT NULL WHEN 1 THEN owner.tag IS NOT NULL WHEN 2 THEN owner.fixed_image IS NOT NULL WHEN 3 THEN owner.mandatory IS NOT NULL WHEN 4 THEN owner.interface IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,24,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_device_instances AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1) AS field
UNION ALL
SELECT groups.snapshot_key,25,owner.set_id,owner.element_order,owner.extension_order,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_device_extensions AS owner ON owner.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key,26,owner.set_id,owner.element_order,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_slots AS owner ON owner.set_id=sets.set_id
UNION ALL
SELECT groups.snapshot_key,27,owner.set_id,owner.element_order,owner.option_order,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_slot_options AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN 1 WHEN 2 THEN owner.default_specified=1 END
UNION ALL
SELECT groups.snapshot_key,28,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_software_lists AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN 1 WHEN 2 THEN owner.status IS NOT NULL WHEN 3 THEN owner.filter IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,29,owner.set_id,owner.element_order,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_ram_options AS owner ON owner.set_id=sets.set_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1) AS field
WHERE CASE field.field_kind WHEN 0 THEN 1 WHEN 1 THEN owner.default_value IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,30,owner.set_id,0,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN mame_machine_compatibility AS owner ON owner.set_id=sets.set_id WHERE owner.is_consumable_specified=1
UNION ALL
SELECT groups.snapshot_key,31,owner.occurrence_id,0,0,0,field.field_kind FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id CROSS JOIN mame_rom_compatibility AS owner ON owner.occurrence_id=occurrence.occurrence_id CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7) AS field
WHERE CASE field.field_kind WHEN 0 THEN owner.md5_text IS NOT NULL WHEN 1 THEN owner.sound_only IS NOT NULL WHEN 2 THEN owner.dispose IS NOT NULL WHEN 3 THEN owner.load_flag IS NOT NULL WHEN 4 THEN owner.value IS NOT NULL WHEN 5 THEN owner.inverted IS NOT NULL WHEN 6 THEN owner.ovha IS NOT NULL WHEN 7 THEN owner.no_thread IS NOT NULL END
UNION ALL
SELECT groups.snapshot_key,32,owner.occurrence_id,0,0,0,0 FROM catalog_set_groups AS groups CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id CROSS JOIN mame_disk_compatibility AS owner ON owner.occurrence_id=occurrence.occurrence_id;

-- Native and compatibility attributes occupy the same opening-tag ordinal domain.
CREATE VIEW mame_attribute_ordinal_collisions AS
SELECT groups.snapshot_key,1 AS family,native.set_id AS owner_a,0 AS owner_b,
       0 AS owner_c,0 AS owner_d,native.source_order AS field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN mame_machines_attribute_positions AS native ON native.set_id=sets.set_id
CROSS JOIN mame_machine_compatibility_attribute_positions AS compat
 ON compat.set_id=native.set_id AND compat.source_order=native.source_order
UNION ALL
SELECT groups.snapshot_key,3,native.occurrence_id,0,0,0,native.source_order
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_rom_claims_attribute_positions AS native ON native.occurrence_id=occurrence.occurrence_id
CROSS JOIN mame_rom_compatibility_attribute_positions AS compat
 ON compat.occurrence_id=native.occurrence_id AND compat.source_order=native.source_order
UNION ALL
SELECT groups.snapshot_key,4,native.occurrence_id,0,0,0,native.source_order
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id
CROSS JOIN mame_disk_claims_attribute_positions AS native ON native.occurrence_id=occurrence.occurrence_id
CROSS JOIN mame_disk_compatibility_attribute_positions AS compat
 ON compat.occurrence_id=native.occurrence_id AND compat.source_order=native.source_order;
