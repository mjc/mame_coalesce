#!/usr/bin/env python3
"""Independent MAME source-count route controls over constructed native facts.

Run: python3 -Werror::ResourceWarning docs/schema-candidate/mame_count_mapping_check.py
Literal seals below are fixture receipts, not parser totals or EOF evidence.
"""

import sqlite3
import unittest
from unittest.mock import patch

import assemble
import mame_presence_check
import source_counts


EDITION = 1

# Literal event vector, independently transcribed from the constructed MAME
# fixture. Keyed by the counter contract; never generated from route/table SQL.
# The fixture gets one additional valid ROM below, so ROM totals are asymmetric
# with disks for the same-key/same-scope route-body mutation control.
EXPECTED_COUNTS = {
    "machine_count": 1,
    "machine_text_element_count": 3,
    "bios_set_count": 1,
    "rom_count": 2,
    "disk_count": 1,
    "sample_count": 1,
    "device_reference_count": 1,
    "chip_count": 1,
    "display_count": 1,
    "sound_count": 1,
    "input_count": 1,
    "input_control_count": 1,
    "switch_count": 2,
    "switch_location_count": 2,
    "switch_value_count": 2,
    "switch_condition_count": 2,
    "switch_value_condition_count": 2,
    "adjuster_count": 1,
    "adjuster_condition_count": 1,
    "port_count": 1,
    "analog_count": 1,
    "driver_count": 1,
    "feature_count": 1,
    "device_count": 1,
    "device_instance_count": 1,
    "device_extension_count": 1,
    "slot_count": 1,
    "slot_option_count": 1,
    "softwarelist_reference_count": 1,
    "ram_option_count": 1,
    "machine_switch_condition_attribute_position_count": 8,
    "machine_switch_location_attribute_position_count": 6,
    "machine_switch_value_condition_attribute_position_count": 8,
    "machine_switch_value_attribute_position_count": 6,
    "machine_switch_attribute_position_count": 6,
    "bios_set_attribute_position_count": 3,
    "device_reference_attribute_position_count": 2,
    "disk_attribute_position_count": 8,
    "disk_compatibility_attribute_position_count": 1,
    "rom_attribute_position_count": 11,
    "rom_compatibility_attribute_position_count": 8,
    "document_attribute_position_count": 3,
    "adjuster_condition_attribute_position_count": 4,
    "adjuster_attribute_position_count": 2,
    "analog_attribute_position_count": 1,
    "chip_attribute_position_count": 4,
    "display_attribute_position_count": 14,
    "device_extension_attribute_position_count": 1,
    "device_instance_attribute_position_count": 2,
    "device_attribute_position_count": 5,
    "driver_attribute_position_count": 8,
    "feature_attribute_position_count": 3,
    "input_control_attribute_position_count": 11,
    "input_attribute_position_count": 4,
    "machine_compatibility_attribute_position_count": 1,
    "machine_attribute_position_count": 9,
    "port_attribute_position_count": 1,
    "ram_option_attribute_position_count": 2,
    "slot_option_attribute_position_count": 3,
    "slot_attribute_position_count": 1,
    "softwarelist_reference_attribute_position_count": 4,
    "sample_attribute_position_count": 1,
    "sound_attribute_position_count": 1,
}

# Independent physical targets. This catches route-table substitutions even
# when a fixture happens to have equal values for the affected counters.
EXPECTED_TABLES = {
    "machine_count": "mame_machines",
    "machine_text_element_count": "mame_machine_text_elements",
    "bios_set_count": "mame_bios_sets",
    "rom_count": "mame_roms",
    "disk_count": "mame_disks",
    "sample_count": "mame_samples",
    "device_reference_count": "mame_device_references",
    "chip_count": "mame_chips",
    "display_count": "mame_displays",
    "sound_count": "mame_sound",
    "input_count": "mame_inputs",
    "input_control_count": "mame_input_controls",
    "switch_count": "mame_switches",
    "switch_location_count": "mame_switch_locations",
    "switch_value_count": "mame_switch_values",
    "switch_condition_count": "mame_switch_conditions",
    "switch_value_condition_count": "mame_switch_value_conditions",
    "adjuster_count": "mame_adjusters",
    "adjuster_condition_count": "mame_adjuster_conditions",
    "port_count": "mame_ports",
    "analog_count": "mame_analogs",
    "driver_count": "mame_drivers",
    "feature_count": "mame_features",
    "device_count": "mame_devices",
    "device_instance_count": "mame_device_instances",
    "device_extension_count": "mame_device_extensions",
    "slot_count": "mame_slots",
    "slot_option_count": "mame_slot_options",
    "softwarelist_reference_count": "mame_softwarelist_references",
    "ram_option_count": "mame_ram_options",
    "machine_switch_condition_attribute_position_count": "machine_switch_conditions_attribute_positions",
    "machine_switch_location_attribute_position_count": "machine_switch_locations_attribute_positions",
    "machine_switch_value_condition_attribute_position_count": "machine_switch_value_conditions_attribute_positions",
    "machine_switch_value_attribute_position_count": "machine_switch_values_attribute_positions",
    "machine_switch_attribute_position_count": "machine_switches_attribute_positions",
    "bios_set_attribute_position_count": "mame_bios_sets_attribute_positions",
    "device_reference_attribute_position_count": "mame_device_references_attribute_positions",
    "disk_attribute_position_count": "mame_disk_claims_attribute_positions",
    "disk_compatibility_attribute_position_count": "mame_disk_compatibility_attribute_positions",
    "rom_attribute_position_count": "mame_rom_claims_attribute_positions",
    "rom_compatibility_attribute_position_count": "mame_rom_compatibility_attribute_positions",
    "document_attribute_position_count": "mame_document_facts_attribute_positions",
    "adjuster_condition_attribute_position_count": "mame_machine_adjuster_conditions_attribute_positions",
    "adjuster_attribute_position_count": "mame_machine_adjusters_attribute_positions",
    "analog_attribute_position_count": "mame_machine_analogs_attribute_positions",
    "chip_attribute_position_count": "mame_machine_chips_attribute_positions",
    "display_attribute_position_count": "mame_machine_displays_attribute_positions",
    "device_extension_attribute_position_count": "mame_machine_device_extensions_attribute_positions",
    "device_instance_attribute_position_count": "mame_machine_device_instances_attribute_positions",
    "device_attribute_position_count": "mame_machine_devices_attribute_positions",
    "driver_attribute_position_count": "mame_machine_drivers_attribute_positions",
    "feature_attribute_position_count": "mame_machine_features_attribute_positions",
    "input_control_attribute_position_count": "mame_machine_input_controls_attribute_positions",
    "input_attribute_position_count": "mame_machine_inputs_attribute_positions",
    "machine_compatibility_attribute_position_count": "mame_machine_compatibility_attribute_positions",
    "machine_attribute_position_count": "mame_machines_attribute_positions",
    "port_attribute_position_count": "mame_machine_ports_attribute_positions",
    "ram_option_attribute_position_count": "mame_machine_ram_options_attribute_positions",
    "slot_option_attribute_position_count": "mame_machine_slot_options_attribute_positions",
    "slot_attribute_position_count": "mame_machine_slots_attribute_positions",
    "softwarelist_reference_attribute_position_count": "mame_machine_software_lists_attribute_positions",
    "sample_attribute_position_count": "mame_samples_attribute_positions",
    "sound_attribute_position_count": "mame_machine_sounds_attribute_positions",
}


def candidate(routes=None):
    if routes is None:
        return source_counts.candidate()
    with patch.object(source_counts, "inventory", return_value=tuple(routes)):
        return source_counts.candidate()


def seed_facts(db):
    mame_presence_check.seed(db)
    db.execute("INSERT INTO catalog_source_elements VALUES(237,1,'mame_rom')")
    db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(237)")
    db.execute(
        "INSERT INTO mame_roms(media_entry_id,machine_id,name,size_text,bios,region,offset,"
        "status,status_specified,optional,optional_specified,source_order,source_line,source_column) "
        "VALUES(237,100,'asymmetric-extra.rom',NULL,NULL,NULL,NULL,'good',0,0,0,100,47,3)"
    )
    db.execute(
        "INSERT INTO mame_rom_claims_attribute_positions "
        "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
        "VALUES(237,'name',0,0,47,10)"
    )
    db.commit()


def insert_literal_seal(db, counts=EXPECTED_COUNTS):
    counters = tuple(sorted(counts))
    table = assemble.identifier("mame_source_count_seals")
    columns = ",".join(assemble.identifier(counter) for counter in counters)
    placeholders = ",".join("?" for _ in range(len(counters) + 1))
    db.execute(
        f"INSERT INTO {table}(edition_id,{columns}) VALUES({placeholders})",
        (EDITION, *(counts[counter] for counter in counters)),
    )


def source_findings(db):
    return db.execute(
        "SELECT problem,owner_id,edition_id FROM candidate_source_count_problems "
        "WHERE edition_id=? AND problem LIKE 'source_count:%' ORDER BY problem",
        (EDITION,),
    ).fetchall()


def drop_delete_guards(db, table):
    for name, sql in db.execute(
        "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?", (table,)
    ).fetchall():
        if "BEFORE DELETE" in (sql or "").upper():
            db.execute("DROP TRIGGER " + assemble.identifier(name))


class MameCountMappings(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(candidate())
        seed_facts(cls.db)
        cls.routes = tuple(row for row in source_counts.inventory() if row["family"] == "mame")
        cls.routes_by_counter = {row["counter"]: row for row in cls.routes}
        cls.assert_route_contract()
        insert_literal_seal(cls.db)
        cls.db.commit()
        cls.assert_baseline()

    @classmethod
    def assert_route_contract(cls):
        expected = set(EXPECTED_COUNTS)
        actual = set(cls.routes_by_counter)
        if actual != expected:
            raise AssertionError(
                f"MAME route coverage differs: missing={sorted(expected-actual)}, "
                f"extra={sorted(actual-expected)}"
            )
        if len(cls.routes) != 63 or set(EXPECTED_TABLES) != expected:
            raise AssertionError(f"expected 63 closed MAME routes, found {len(cls.routes)}")
        for counter, route in cls.routes_by_counter.items():
            if route["table"] != EXPECTED_TABLES[counter]:
                raise AssertionError(
                    f"mame:{counter} target {route['table']} != {EXPECTED_TABLES[counter]}"
                )
            # This is fixture-coverage evidence only, not an expected total.
            found = cls.db.execute(
                f"SELECT 1 FROM {assemble.identifier(route['table'])} AS owner "
                f"WHERE ({route['scope_sql']})=? LIMIT 1", (EDITION,)
            ).fetchone()
            if found is None:
                raise AssertionError(f"MAME constructed fixture misses route {counter}")

    @classmethod
    def assert_baseline(cls):
        problems = source_findings(cls.db)
        if problems:
            raise AssertionError(f"MAME literal fixture seal baseline is not quiet: {problems}")

    def test_every_counter_has_a_literal_baseline_and_independent_route(self):
        self.assertEqual(len(EXPECTED_COUNTS), 63)
        self.assertEqual(source_findings(self.db), [])

    def test_each_route_deletion_reports_only_its_sealed_counter(self):
        self.db.execute("PRAGMA foreign_keys=OFF")
        for counter, route in self.routes_by_counter.items():
            with self.subTest(counter=counter):
                savepoint = assemble.identifier("mame_count_" + counter)
                self.db.execute(f"SAVEPOINT {savepoint}")
                try:
                    table = route["table"]
                    drop_delete_guards(self.db, table)
                    deleted = self.db.execute(
                        f"DELETE FROM {assemble.identifier(table)} AS owner "
                        f"WHERE ({route['scope_sql']})=?", (EDITION,)
                    ).rowcount
                    self.assertGreater(deleted, 0, f"no target-edition rows deleted for {counter}")
                    self.assertEqual(
                        source_findings(self.db),
                        [(f"source_count:mame:{counter}", EDITION, EDITION)],
                    )
                    self.db.execute(
                        f"UPDATE mame_source_count_seals SET {assemble.identifier(counter)}=0 "
                        "WHERE edition_id=?", (EDITION,)
                    )
                    self.assertEqual(source_findings(self.db), [])
                finally:
                    self.db.execute(f"ROLLBACK TO {savepoint}")
                    self.db.execute(f"RELEASE {savepoint}")
        self.db.execute("PRAGMA foreign_keys=ON")

    def test_rom_disk_same_scope_route_body_swap_fails_baseline_mapping_assertion(self):
        routes = [dict(row) for row in source_counts.inventory()]
        by_counter = {row["counter"]: row for row in routes if row["family"] == "mame"}
        by_counter["rom_count"]["table"], by_counter["disk_count"]["table"] = (
            by_counter["disk_count"]["table"], by_counter["rom_count"]["table"]
        )
        db = sqlite3.connect(":memory:")
        self.addCleanup(db.close)
        db.execute("PRAGMA foreign_keys=ON")
        # Candidate compilation and all fixture inserts succeed with the swap;
        # only the independent literal mapping assertion should reject it.
        db.executescript(candidate(routes))
        seed_facts(db)
        insert_literal_seal(db)
        problems = source_findings(db)
        with self.assertRaisesRegex(AssertionError, "mutated mapping baseline"):
            if problems:
                raise AssertionError(f"mutated mapping baseline: {problems}")
        self.assertEqual(
            problems,
            [
                ("source_count:mame:disk_count", EDITION, EDITION),
                ("source_count:mame:rom_count", EDITION, EDITION),
            ],
        )


if __name__ == "__main__":
    unittest.main()
