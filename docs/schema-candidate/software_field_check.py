"""Focused, stdlib-only checks for the isolated software field candidate.

Run: python3 docs/schema-candidate/software_field_check.py
Uses assemble.py --emit and fresh in-memory SQLite databases only.
"""
import csv
from collections import defaultdict
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parent
HEADER = [
    "family", "owner_table", "field_code", "wire_name", "value_owner",
    "value_column", "position_table", "presence", "default_rule", "evidence",
]

# Independent DOC-12 seq 38114 inventory, transcribed from its two exact key
# crosswalks and complete field-state ledger. Do not generate this from the TSV,
# schema, fixture rows or their counts: substitutions must change the result.
# wire -> (typed owner, canonical code, value owner, value column(s), positions)
EXPECTED_ATTRIBUTES = {
    "softwarelists/@build": ("software_wrapper_headers", "0", "software_wrapper_headers", "build", "software_wrapper_attribute_positions"),
    "softwarelist/@name": ("software_lists", "0", "software_lists", "name", "software_list_attribute_positions"),
    "softwarelist/@description": ("software_lists", "1", "software_lists", "description", "software_list_attribute_positions"),
    "software/@name": ("software_titles", "0", "catalog_sets", "set_name", "software_title_attribute_positions"),
    "software/@cloneof": ("software_titles", "1", "software_clone_links", "target_name", "software_title_attribute_positions"),
    "software/@supported": ("software_titles", "2", "software_titles", "supported,supported_specified", "software_title_attribute_positions"),
    "info/@name": ("software_title_info", "0", "software_title_info", "name", "software_title_info_attribute_positions"),
    "info/@value": ("software_title_info", "1", "software_title_info", "value", "software_title_info_attribute_positions"),
    "sharedfeat/@name": ("software_shared_features", "0", "software_shared_features", "name", "software_shared_feature_attribute_positions"),
    "sharedfeat/@value": ("software_shared_features", "1", "software_shared_features", "value", "software_shared_feature_attribute_positions"),
    "part/@name": ("software_parts", "0", "software_parts", "name", "software_part_attribute_positions"),
    "part/@interface": ("software_parts", "1", "software_parts", "interface", "software_part_attribute_positions"),
    "feature/@name": ("software_part_features", "0", "software_part_features", "name", "software_part_feature_attribute_positions"),
    "feature/@value": ("software_part_features", "1", "software_part_features", "value", "software_part_feature_attribute_positions"),
    "dipswitch/@name": ("software_part_switches", "0", "software_part_switches", "name", "software_part_switch_attribute_positions"),
    "dipswitch/@tag": ("software_part_switches", "1", "software_part_switches", "tag", "software_part_switch_attribute_positions"),
    "dipswitch/@mask": ("software_part_switches", "2", "software_part_switches", "mask", "software_part_switch_attribute_positions"),
    "dipvalue/@name": ("software_part_switch_values", "0", "software_part_switch_values", "name", "software_part_switch_value_attribute_positions"),
    "dipvalue/@value": ("software_part_switch_values", "1", "software_part_switch_values", "value", "software_part_switch_value_attribute_positions"),
    "dipvalue/@default": ("software_part_switch_values", "2", "software_part_switch_values", "is_default,default_specified", "software_part_switch_value_attribute_positions"),
    "dataarea/@name": ("software_data_areas", "0", "software_data_areas", "name", "software_data_area_attribute_positions"),
    "dataarea/@size": ("software_data_areas", "1", "software_data_areas", "declared_size_text", "software_data_area_attribute_positions"),
    "dataarea/@width": ("software_data_areas", "2", "software_data_areas", "width,width_specified", "software_data_area_attribute_positions"),
    "dataarea/@endianness": ("software_data_areas", "3", "software_data_areas", "endianness,endianness_specified", "software_data_area_attribute_positions"),
    "diskarea/@name": ("software_disk_areas", "0", "software_disk_areas", "name", "software_disk_area_attribute_positions"),
    "rom/@name": ("software_rom_load_entries", "0", "software_rom_load_entries", "name", "software_rom_attribute_positions"),
    "rom/@size": ("software_rom_load_entries", "1", "software_rom_load_entries", "size_text", "software_rom_attribute_positions"),
    "rom/@crc": ("software_rom_load_entries", "2", "declared_catalog_hash_text", "declared_text", "software_rom_attribute_positions"),
    "rom/@sha1": ("software_rom_load_entries", "3", "declared_catalog_hash_text", "declared_text", "software_rom_attribute_positions"),
    "rom/@offset": ("software_rom_load_entries", "4", "software_rom_load_entries", "offset_text", "software_rom_attribute_positions"),
    "rom/@value": ("software_rom_load_entries", "5", "software_rom_load_entries", "value", "software_rom_attribute_positions"),
    "rom/@status": ("software_rom_load_entries", "6", "software_rom_load_entries", "status,status_specified", "software_rom_attribute_positions"),
    "rom/@loadflag": ("software_rom_load_entries", "7", "software_rom_load_entries", "loadflag", "software_rom_attribute_positions"),
    "disk/@name": ("software_disks", "0", "software_disks", "name", "software_disk_attribute_positions"),
    "disk/@sha1": ("software_disks", "1", "declared_catalog_hash_text", "declared_text", "software_disk_attribute_positions"),
    "disk/@status": ("software_disks", "2", "software_disks", "status,status_specified", "software_disk_attribute_positions"),
    "disk/@writeable": ("software_disks", "3", "software_disks", "writeable,writeable_specified", "software_disk_attribute_positions"),
}
EXPECTED_TEXT = {
    "softwarelist/notes/#text": ("software_list_notes", "text", "software_list_notes", "text_value", "-"),
    "software/description/#text": ("software_title_text_elements", "description", "software_title_text_elements", "text_value", "-"),
    "software/year/#text": ("software_title_text_elements", "year", "software_title_text_elements", "text_value", "-"),
    "software/publisher/#text": ("software_title_text_elements", "publisher", "software_title_text_elements", "text_value", "-"),
    "software/notes/#text": ("software_title_text_elements", "notes", "software_title_text_elements", "text_value", "-"),
}


def validate_inventory(fields):
    actual = {}
    for field in fields:
        wire = field["wire_name"]
        if wire in actual:
            raise AssertionError(f"duplicate wire field: {wire}")
        actual[wire] = tuple(field[key] for key in (
            "owner_table", "field_code", "value_owner", "value_column", "position_table"
        ))
    expected = EXPECTED_ATTRIBUTES | EXPECTED_TEXT
    if actual != expected:
        missing = expected.keys() - actual.keys()
        extra = actual.keys() - expected.keys()
        changed = {wire for wire in expected.keys() & actual.keys() if expected[wire] != actual[wire]}
        raise AssertionError(f"DOC-12 field inventory: missing={missing}, extra={extra}, changed={changed}")


def identifier(name):
    if not re.fullmatch(r"[a-z][a-z0-9_]*", name):
        raise ValueError(f"not a SQL identifier: {name!r}")
    return '"' + name + '"'


def closed_codes(ddl):
    match = re.search(
        r"field_kind INTEGER NOT NULL CHECK \(field_kind (?:BETWEEN (\d+) AND (\d+)|= (\d+))\)",
        ddl,
    )
    if not match:
        raise AssertionError("unrecognized closed field_kind constraint")
    if match[3] is not None:
        return {int(match[3])}
    return set(range(int(match[1]), int(match[2]) + 1))


class SoftwareFields(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = subprocess.run(
            [sys.executable, str(ROOT / "assemble.py"), "--emit"],
            check=True, capture_output=True, text=True,
        ).stdout
        with (ROOT / "software-field-coverage.tsv").open(newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            if reader.fieldnames != HEADER:
                raise AssertionError(f"wrong coverage header: {reader.fieldnames}")
            cls.fields = list(reader)
        cls.witness = (ROOT / "software_field_witnesses.sql").read_text()
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(cls.schema)
        trace = []
        cls.db.set_trace_callback(trace.append)
        try:
            cls.db.executescript(cls.witness)
        except sqlite3.Error as error:
            raise AssertionError(f"constructed field witness: {error}; SQL: {trace[-1:]}") from error
        finally:
            cls.db.set_trace_callback(None)

    def setUp(self):
        # executescript commits pending transactions: prepare only in setUpClass.
        self.db.execute("SAVEPOINT software_field_test")

    def tearDown(self):
        self.db.execute("ROLLBACK TO software_field_test")
        self.db.execute("RELEASE software_field_test")

    def columns(self, table):
        return {row[1] for row in self.db.execute(f"PRAGMA table_xinfo({identifier(table)})")}

    def position_domains(self):
        return {
            name: closed_codes(ddl)
            for name, ddl in self.db.execute(
                "SELECT name,sql FROM sqlite_schema WHERE type='table' "
                "AND name GLOB 'software_*_attribute_positions'"
            )
        }

    def test_ledger_references_and_complete_closed_code_domains(self):
        validate_inventory(self.fields)
        tables = {row[0] for row in self.db.execute(
            "SELECT name FROM sqlite_schema WHERE type IN ('table','view')"
        )}
        positions = defaultdict(set)
        wires = set()
        text_fields = set()
        for field in self.fields:
            with self.subTest(wire=field["wire_name"]):
                self.assertEqual(field["family"], "software")
                self.assertNotIn(field["wire_name"], wires)
                wires.add(field["wire_name"])
                self.assertIn(field["owner_table"], tables)
                self.assertIn(field["value_owner"], tables)
                for column in field["value_column"].split(","):
                    self.assertIn(column, self.columns(field["value_owner"]))
                self.assertIn("MAMEC-DOC-12@38114:", field["evidence"])
                if field["wire_name"].endswith("/#text"):
                    self.assertEqual(field["position_table"], "-")
                    self.assertIn("text_value", self.columns(field["owner_table"]))
                    text_fields.add((field["owner_table"], field["field_code"], field["wire_name"]))
                else:
                    self.assertIn("/@", field["wire_name"])
                    self.assertIn(field["position_table"], tables)
                    positions[field["position_table"]].add(int(field["field_code"]))
                    # Each companion's actual typed-parent FK must match the ledger.
                    parents = {row[2] for row in self.db.execute(
                        f"PRAGMA foreign_key_list({identifier(field['position_table'])})"
                    ) if row[3] not in ("reported_hash_id", "relationship_id")}
                    self.assertEqual(parents, {field["owner_table"]})
        self.assertEqual(dict(positions), self.position_domains())
        self.assertEqual(text_fields, {
            ("software_list_notes", "text", "softwarelist/notes/#text"),
            ("software_title_text_elements", "description", "software/description/#text"),
            ("software_title_text_elements", "year", "software/year/#text"),
            ("software_title_text_elements", "publisher", "software/publisher/#text"),
            ("software_title_text_elements", "notes", "software/notes/#text"),
        })
        text_ddl = self.db.execute(
            "SELECT sql FROM sqlite_schema WHERE name='software_title_text_elements'"
        ).fetchone()[0]
        self.assertEqual(closed_codes(text_ddl), {0, 1, 2, 3})

    def test_inventory_rejects_every_removal_and_same_count_substitution(self):
        for index, field in enumerate(self.fields):
            with self.subTest(wire=field["wire_name"], mutation="remove"):
                with self.assertRaisesRegex(AssertionError, "DOC-12 field inventory"):
                    validate_inventory(self.fields[:index] + self.fields[index + 1:])
            with self.subTest(wire=field["wire_name"], mutation="same-count-wire"):
                substituted = [dict(row) for row in self.fields]
                substituted[index]["wire_name"] = "invented/@same-row-count"
                with self.assertRaisesRegex(AssertionError, "DOC-12 field inventory"):
                    validate_inventory(substituted)
            with self.subTest(wire=field["wire_name"], mutation="same-count-mapping"):
                substituted = [dict(row) for row in self.fields]
                substituted[index]["value_column"] = "source_order"
                with self.assertRaisesRegex(AssertionError, "DOC-12 field inventory"):
                    validate_inventory(substituted)

    def test_each_position_code_executes_and_boundaries_reject(self):
        for table, codes in self.position_domains().items():
            with self.subTest(table=table):
                actual = {row[0] for row in self.db.execute(
                    f"SELECT DISTINCT field_kind FROM {identifier(table)}"
                )}
                self.assertEqual(actual, codes)
                owner_column = next(row[3] for row in self.db.execute(
                    f"PRAGMA foreign_key_list({identifier(table)})"
                ) if row[3] not in ("reported_hash_id", "relationship_id"))
                owner = self.db.execute(
                    f"SELECT {identifier(owner_column)} FROM {identifier(table)} "
                    "WHERE field_kind=0 LIMIT 1"
                ).fetchone()[0]
                where = f"{identifier(owner_column)}=? AND field_kind=0"
                # Successful mutation on that exact row rules out an immutable owner.
                self.db.execute(f"UPDATE {identifier(table)} SET source_column=source_column+1 WHERE {where}", (owner,))
                for invalid in (-1, max(codes) + 1):
                    with self.subTest(code=invalid), self.assertRaisesRegex(
                        sqlite3.IntegrityError, "CHECK constraint failed: field_kind"
                    ):
                        self.db.execute(
                            f"UPDATE {identifier(table)} SET field_kind=? WHERE {where}",
                            (invalid, owner),
                        )
                with self.assertRaisesRegex(sqlite3.IntegrityError, "cannot store REAL value"):
                    self.db.execute(
                        f"UPDATE {identifier(table)} SET field_kind=0.5 WHERE {where}", (owner,)
                    )

    def test_required_values_reject_null_after_empty_controls_succeed(self):
        for field in self.fields:
            if not field["presence"].startswith("required;"):
                continue
            table, column = field["value_owner"], field["value_column"]
            with self.subTest(wire=field["wire_name"]):
                self.assertGreater(self.db.execute(
                    f"SELECT count(*) FROM {identifier(table)} WHERE {identifier(column)}=''"
                ).fetchone()[0], 0)
                with self.assertRaisesRegex(
                    sqlite3.IntegrityError,
                    "NOT NULL constraint failed: " + re.escape(table + "." + column),
                ):
                    self.db.execute(f"UPDATE {identifier(table)} SET {identifier(column)}=NULL")

    def test_effective_default_pairs_reject_only_omitted_nondefaults(self):
        cases = [
            ("software_titles", "set_id", 100, "supported", "supported_specified", "partial"),
            ("software_part_switch_values", "source_element_id", 230, "is_default", "default_specified", 1),
            ("software_data_areas", "area_id", 300, "width", "width_specified", 16),
            ("software_data_areas", "area_id", 300, "endianness", "endianness_specified", "big"),
            ("software_rom_load_entries", "media_entry_id", 400, "status", "status_specified", "baddump"),
            ("software_disks", "media_entry_id", 410, "status", "status_specified", "baddump"),
            ("software_disks", "media_entry_id", 410, "writeable", "writeable_specified", 1),
        ]
        for table, key, owner, value, bit, nondefault in cases:
            with self.subTest(table=table, value=value):
                self.db.execute("SAVEPOINT default_pair")
                try:
                    statement = f"UPDATE {identifier(table)} SET {identifier(value)}=?,{identifier(bit)}=? WHERE {identifier(key)}=?"
                    self.db.execute(statement, (nondefault, 1, owner))
                    with self.assertRaisesRegex(
                        sqlite3.IntegrityError, "CHECK constraint failed: " + bit + " = 1 OR"
                    ):
                        self.db.execute(statement, (nondefault, 0, owner))
                finally:
                    self.db.execute("ROLLBACK TO default_pair")
                    self.db.execute("RELEASE default_pair")

    def test_closed_enums_reject_invalid_values_on_existing_valid_owners(self):
        cases = [
            ("software_titles", "supported", "'yes'", "'maybe'", "set_id=100"),
            ("software_titles", "supported", "'yes'", "''", "set_id=100"),
            ("software_data_areas", "width", "8", "12", "area_id=300"),
            ("software_data_areas", "endianness", "'little'", "'middle'", "area_id=300"),
            ("software_rom_load_entries", "status", "'good'", "'unknown'", "media_entry_id=400"),
            ("software_rom_load_entries", "loadflag", "'load16_byte'", "'load'", "media_entry_id=400"),
            ("software_rom_load_entries", "loadflag", "'load16_byte'", "''", "media_entry_id=400"),
            ("software_disks", "status", "'good'", "'unknown'", "media_entry_id=410"),
            ("software_disks", "writeable", "0", "2", "media_entry_id=410"),
            ("software_part_switch_values", "is_default", "0", "2", "source_element_id=230"),
        ]
        for table, column, valid, invalid, where in cases:
            with self.subTest(table=table, column=column, invalid=invalid):
                self.db.execute(f"UPDATE {identifier(table)} SET {identifier(column)}={valid} WHERE {where}")
                with self.assertRaisesRegex(
                    sqlite3.IntegrityError, "CHECK constraint failed: " + column + " IN"
                    if column != "loadflag" else "CHECK constraint failed: loadflag IS NULL OR"
                ):
                    self.db.execute(f"UPDATE {identifier(table)} SET {identifier(column)}={invalid} WHERE {where}")

    def test_all_loadflag_tokens_store_without_claiming_checked_chain_qualification(self):
        tokens = [
            "reload", "fill", "continue", "reload_plain", "ignore",
            "load16_byte", "load16_word", "load16_word_swap", "load32_byte",
            "load32_word", "load32_word_swap", "load32_dword", "load64_word",
            "load64_word_swap",
        ]
        for token in tokens:
            with self.subTest(token=token):
                self.db.execute("SAVEPOINT source_token")
                try:
                    self.db.execute(
                        "UPDATE software_rom_load_entries SET loadflag=? WHERE media_entry_id=400", (token,)
                    )
                    self.assertEqual(self.db.execute(
                        "SELECT loadflag FROM software_rom_load_entries WHERE media_entry_id=400"
                    ).fetchone()[0], token)
                finally:
                    self.db.execute("ROLLBACK TO source_token")
                    self.db.execute("RELEASE source_token")


if __name__ == "__main__":
    unittest.main(verbosity=2)
