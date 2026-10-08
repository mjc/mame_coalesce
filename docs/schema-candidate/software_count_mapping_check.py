#!/usr/bin/env python3
"""Independent software source-count route controls over constructed facts.

Run: python3 -Werror::ResourceWarning docs/schema-candidate/software_count_mapping_check.py
Literal seals below are fixture receipts, not parser totals or EOF evidence.
"""

import sqlite3
import unittest

import assemble
import source_counts


EDITION = 1

# Literal event vector transcribed from the populated software field fixture.
# Expected values are keyed by counter contract, never selected or derived from
# source_counts.inventory() table targets or candidate audit query results.
EXPECTED_COUNTS = {
    "list_count": 2,
    "list_note_count": 1,
    "title_count": 2,
    "title_text_element_count": 7,
    "title_info_count": 2,
    "shared_feature_count": 2,
    "part_count": 1,
    "part_feature_count": 2,
    "part_switch_count": 1,
    "part_switch_value_count": 2,
    "area_count": 3,
    "rom_entry_count": 4,
    "disk_entry_count": 4,
    "wrapper_attribute_position_count": 1,
    "list_attribute_position_count": 3,
    "title_attribute_position_count": 4,
    "title_info_attribute_position_count": 3,
    "shared_feature_attribute_position_count": 3,
    "part_attribute_position_count": 2,
    "part_feature_attribute_position_count": 3,
    "part_switch_attribute_position_count": 3,
    "part_switch_value_attribute_position_count": 5,
    "data_area_attribute_position_count": 6,
    "disk_area_attribute_position_count": 1,
    "rom_attribute_position_count": 21,
    "disk_attribute_position_count": 11,
}

EXPECTED_TABLES = {
    "list_count": "software_lists",
    "list_note_count": "software_list_notes",
    "title_count": "software_titles",
    "title_text_element_count": "software_title_text_elements",
    "title_info_count": "software_title_info",
    "shared_feature_count": "software_shared_features",
    "part_count": "software_parts",
    "part_feature_count": "software_part_features",
    "part_switch_count": "software_part_switches",
    "part_switch_value_count": "software_part_switch_values",
    "area_count": "software_areas",
    "rom_entry_count": "software_rom_load_entries",
    "disk_entry_count": "software_disks",
    "wrapper_attribute_position_count": "software_wrapper_attribute_positions",
    "list_attribute_position_count": "software_list_attribute_positions",
    "title_attribute_position_count": "software_title_attribute_positions",
    "title_info_attribute_position_count": "software_title_info_attribute_positions",
    "shared_feature_attribute_position_count": "software_shared_feature_attribute_positions",
    "part_attribute_position_count": "software_part_attribute_positions",
    "part_feature_attribute_position_count": "software_part_feature_attribute_positions",
    "part_switch_attribute_position_count": "software_part_switch_attribute_positions",
    "part_switch_value_attribute_position_count": "software_part_switch_value_attribute_positions",
    "data_area_attribute_position_count": "software_data_area_attribute_positions",
    "disk_area_attribute_position_count": "software_disk_area_attribute_positions",
    "rom_attribute_position_count": "software_rom_attribute_positions",
    "disk_attribute_position_count": "software_disk_attribute_positions",
}


class SoftwareCountMappings(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(source_counts.candidate())
        cls.db.executescript((assemble.ROOT / "software_field_witnesses.sql").read_text())
        cls.routes = tuple(row for row in source_counts.inventory() if row["family"] == "software")
        cls.routes_by_counter = {row["counter"]: row for row in cls.routes}
        cls.assert_route_contract()
        cls.insert_literal_seal()
        cls.db.commit()
        cls.assert_baseline()

    @classmethod
    def assert_route_contract(cls):
        expected = set(EXPECTED_COUNTS)
        actual = set(cls.routes_by_counter)
        if actual != expected:
            raise AssertionError(
                f"software route coverage differs: missing={sorted(expected-actual)}, "
                f"extra={sorted(actual-expected)}"
            )
        if len(cls.routes) != 26 or set(EXPECTED_TABLES) != expected:
            raise AssertionError(f"expected 26 closed software routes, found {len(cls.routes)}")
        for counter, route in cls.routes_by_counter.items():
            if route["table"] != EXPECTED_TABLES[counter]:
                raise AssertionError(
                    f"software:{counter} target {route['table']} != {EXPECTED_TABLES[counter]}"
                )
            # Positive route coverage is checked independently of seal values.
            found = cls.db.execute(
                f"SELECT 1 FROM {assemble.identifier(route['table'])} AS owner "
                f"WHERE ({route['scope_sql']})=? LIMIT 1", (EDITION,)
            ).fetchone()
            if found is None:
                raise AssertionError(f"software constructed fixture misses route {counter}")

    @classmethod
    def insert_literal_seal(cls):
        counters = tuple(sorted(EXPECTED_COUNTS))
        table = assemble.identifier("software_source_count_seals")
        columns = ",".join(assemble.identifier(counter) for counter in counters)
        placeholders = ",".join("?" for _ in range(len(counters) + 1))
        cls.db.execute(
            f"INSERT INTO {table}(edition_id,{columns}) VALUES({placeholders})",
            (EDITION, *(EXPECTED_COUNTS[counter] for counter in counters)),
        )

    @classmethod
    def assert_baseline(cls):
        problems = cls.findings()
        if problems:
            raise AssertionError(f"software literal fixture seal baseline is not quiet: {problems}")

    @classmethod
    def findings(cls):
        return cls.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_source_count_problems "
            "WHERE edition_id=? AND problem LIKE 'source_count:%' ORDER BY problem",
            (EDITION,),
        ).fetchall()

    @staticmethod
    def drop_delete_guards(db, table):
        for name, sql in db.execute(
            "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?", (table,)
        ).fetchall():
            if "BEFORE DELETE" in (sql or "").upper():
                db.execute("DROP TRIGGER " + assemble.identifier(name))

    def test_every_counter_has_a_literal_baseline_and_independent_route(self):
        self.assertEqual(len(EXPECTED_COUNTS), 26)
        self.assertEqual(self.findings(), [])

    def test_each_route_deletion_reports_only_its_sealed_counter(self):
        self.db.execute("PRAGMA foreign_keys=OFF")
        for counter, route in self.routes_by_counter.items():
            with self.subTest(counter=counter):
                savepoint = assemble.identifier("software_count_" + counter)
                self.db.execute(f"SAVEPOINT {savepoint}")
                try:
                    table = route["table"]
                    self.drop_delete_guards(self.db, table)
                    deleted = self.db.execute(
                        f"DELETE FROM {assemble.identifier(table)} AS owner "
                        f"WHERE ({route['scope_sql']})=?", (EDITION,)
                    ).rowcount
                    self.assertGreater(deleted, 0, f"no target-edition rows deleted for {counter}")
                    self.assertEqual(
                        self.findings(),
                        [(f"source_count:software:{counter}", EDITION, EDITION)],
                    )
                    self.db.execute(
                        f"UPDATE software_source_count_seals SET {assemble.identifier(counter)}=0 "
                        "WHERE edition_id=?", (EDITION,)
                    )
                    self.assertEqual(self.findings(), [])
                finally:
                    self.db.execute(f"ROLLBACK TO {savepoint}")
                    self.db.execute(f"RELEASE {savepoint}")
        self.db.execute("PRAGMA foreign_keys=ON")


if __name__ == "__main__":
    unittest.main()
