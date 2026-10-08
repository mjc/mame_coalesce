#!/usr/bin/env python3
"""Independent Logiqx/CMP source-count mapping controls.

Run: python3 -Werror::ResourceWarning docs/schema-candidate/logiqx_cmp_count_mapping_check.py
The populated rows come from the existing constructed presence fixture. Literal
event totals and physical route targets below are independent of that fixture
and of the count inventory. This is not parser accumulation or EOF evidence.
"""

import sqlite3
import unittest

import assemble
import logiqx_cmp_presence_check
import source_counts


EDITIONS = {"logiqx": 1, "clrmamepro": 2}

# Independent event vectors, keyed only by counter name. Do not derive expected
# values or table targets from the TSV, fixture SQL, or the query under test.
EXPECTED_COUNTS = {
    "logiqx": {
        "root_file_name_count": 1,
        "root_sha1_count": 1,
        "header_count": 1,
        "clrmamepro_options_count": 1,
        "romcenter_options_count": 1,
        "game_count": 1,
        "header_text_count": 10,
        "game_text_count": 3,
        "game_comment_count": 1,
        "release_count": 1,
        "bios_set_count": 1,
        "archive_reference_count": 1,
        "device_reference_count": 1,
        "rom_count": 1,
        "disk_count": 1,
        "sample_count": 1,
        "document_attribute_position_count": 2,
        "clrmamepro_option_position_count": 4,
        "romcenter_option_position_count": 7,
        "game_attribute_position_count": 8,
        "release_attribute_position_count": 5,
        "bios_attribute_position_count": 3,
        "archive_attribute_position_count": 1,
        "device_reference_attribute_position_count": 1,
        "rom_attribute_position_count": 9,
        "disk_attribute_position_count": 5,
        "sample_attribute_position_count": 1,
    },
    "clrmamepro": {
        "set_count": 1,
        "rom_count": 1,
        "sample_count": 1,
        "header_field_position_count": 15,
        "set_field_position_count": 12,
        "rom_field_position_count": 12,
    },
}

# Independently pinned physical deletion targets. Keeping these separate from
# EXPECTED_COUNTS catches a TSV route swap even when two fixture totals match.
EXPECTED_TABLES = {
    "logiqx": {
        "root_file_name_count": "logiqx_root_file_names",
        "root_sha1_count": "logiqx_root_sha1_elements",
        "header_count": "logiqx_headers",
        "clrmamepro_options_count": "logiqx_clrmamepro_options",
        "romcenter_options_count": "logiqx_romcenter_options",
        "game_count": "logiqx_games",
        "header_text_count": "logiqx_header_text_elements",
        "game_text_count": "logiqx_game_text_elements",
        "game_comment_count": "logiqx_game_comments",
        "release_count": "logiqx_releases",
        "bios_set_count": "logiqx_bios_sets",
        "archive_reference_count": "logiqx_archive_references",
        "device_reference_count": "logiqx_device_references",
        "rom_count": "logiqx_roms",
        "disk_count": "logiqx_disks",
        "sample_count": "logiqx_samples",
        "document_attribute_position_count": "logiqx_document_attribute_positions",
        "clrmamepro_option_position_count": "logiqx_clrmamepro_option_positions",
        "romcenter_option_position_count": "logiqx_romcenter_option_positions",
        "game_attribute_position_count": "logiqx_game_attribute_positions",
        "release_attribute_position_count": "logiqx_release_attribute_positions",
        "bios_attribute_position_count": "logiqx_bios_attribute_positions",
        "archive_attribute_position_count": "logiqx_archive_attribute_positions",
        "device_reference_attribute_position_count": "logiqx_device_reference_attribute_positions",
        "rom_attribute_position_count": "logiqx_rom_attribute_positions",
        "disk_attribute_position_count": "logiqx_disk_attribute_positions",
        "sample_attribute_position_count": "logiqx_sample_attribute_positions",
    },
    "clrmamepro": {
        "set_count": "clrmamepro_sets",
        "rom_count": "clrmamepro_roms",
        "sample_count": "clrmamepro_samples",
        "header_field_position_count": "clrmamepro_header_field_positions",
        "set_field_position_count": "clrmamepro_set_field_positions",
        "rom_field_position_count": "clrmamepro_rom_field_positions",
    },
}


class LogiqxCmpCountMappings(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(source_counts.candidate())
        cls.db.executescript(logiqx_cmp_presence_check.transplanted_fixture())
        cls.routes = tuple(row for row in source_counts.inventory()
                           if row["family"] in EXPECTED_COUNTS)
        cls.routes_by_family_counter = {
            (row["family"], row["counter"]): row for row in cls.routes
        }
        cls.assert_route_contract()
        cls.insert_literal_seals()
        cls.db.commit()

    @classmethod
    def assert_route_contract(cls):
        expected = {(family, counter)
                    for family, counters in EXPECTED_COUNTS.items()
                    for counter in counters}
        actual = set(cls.routes_by_family_counter)
        if actual != expected:
            raise AssertionError(
                f"Logiqx/CMP count route coverage differs: "
                f"missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
            )
        for (family, counter), row in cls.routes_by_family_counter.items():
            expected_table = EXPECTED_TABLES[family][counter]
            if row["table"] != expected_table:
                raise AssertionError(
                    f"{family}:{counter} inventory target {row['table']} "
                    f"does not match independent target {expected_table}"
                )
        if len(cls.routes) != 33:
            raise AssertionError(f"expected 33 Logiqx/CMP routes, found {len(cls.routes)}")

    @classmethod
    def insert_literal_seals(cls):
        for family, counters in EXPECTED_COUNTS.items():
            routes = [row for row in cls.routes if row["family"] == family]
            columns = [row["counter"] for row in routes]
            table = assemble.identifier(f"{family}_source_count_seals")
            column_sql = ",".join(assemble.identifier(column) for column in columns)
            placeholders = ",".join("?" for _ in range(len(columns) + 1))
            values = tuple(counters[column] for column in columns)
            cls.db.execute(
                f"INSERT INTO {table}(edition_id,{column_sql}) VALUES({placeholders})",
                (EDITIONS[family], *values),
            )

    def setUp(self):
        # Only the mutation case needs detached rows; no FK error is inspected
        # or used as evidence. The publication control runs with SQLite FKs on.
        foreign_keys = "OFF" if self._testMethodName == "test_each_route_maps_its_own_physical_table" else "ON"
        self.db.execute(f"PRAGMA foreign_keys={foreign_keys}")
        self.db.execute("SAVEPOINT cmp_count_mapping_case")

    def tearDown(self):
        self.db.execute("ROLLBACK TO cmp_count_mapping_case")
        self.db.execute("RELEASE cmp_count_mapping_case")
        self.db.execute("PRAGMA foreign_keys=ON")

    def count_problems(self, edition):
        return self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_source_count_problems "
            "WHERE edition_id=? AND problem LIKE 'source_count:%' ORDER BY problem",
            (edition,),
        ).fetchall()

    def bypass_delete_guards(self, table):
        triggers = self.db.execute(
            "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?",
            (table,),
        ).fetchall()
        for name, sql in triggers:
            if "BEFORE DELETE" in sql.upper():
                self.db.execute("DROP TRIGGER " + assemble.identifier(name))

    def test_literal_seals_allow_actual_publication_for_both_fixture_editions(self):
        self.assertEqual(self.count_problems(EDITIONS["logiqx"]), [])
        self.assertEqual(self.count_problems(EDITIONS["clrmamepro"]), [])
        self.db.execute(
            "INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'cmp-count-mapping')"
        )
        self.db.execute(
            "INSERT INTO published_catalog_editions VALUES(2,1,1,2,1,'cmp-count-mapping')"
        )

    def test_each_route_maps_its_own_physical_table(self):
        for family, counters in EXPECTED_COUNTS.items():
            edition = EDITIONS[family]
            for counter, expected_count in counters.items():
                table = EXPECTED_TABLES[family][counter]
                with self.subTest(family=family, counter=counter, table=table):
                    self.assertEqual(self.count_problems(edition), [])
                    savepoint = assemble.identifier(f"route_{family}_{counter}")
                    self.db.execute(f"SAVEPOINT {savepoint}")
                    try:
                        self.bypass_delete_guards(table)
                        deleted = self.db.execute(
                            f"DELETE FROM {assemble.identifier(table)}"
                        ).rowcount
                        self.assertEqual(deleted, expected_count)
                        expected_problem = (
                            f"source_count:{family}:{counter}", edition, edition
                        )
                        self.assertEqual(self.count_problems(edition), [expected_problem])
                        seal_table = assemble.identifier(f"{family}_source_count_seals")
                        counter_column = assemble.identifier(counter)
                        self.db.execute(
                            f"UPDATE {seal_table} SET {counter_column}=0 WHERE edition_id=?",
                            (edition,),
                        )
                        self.assertEqual(self.count_problems(edition), [])
                    finally:
                        self.db.execute(f"ROLLBACK TO {savepoint}")
                        self.db.execute(f"RELEASE {savepoint}")


if __name__ == "__main__":
    unittest.main(verbosity=2)
