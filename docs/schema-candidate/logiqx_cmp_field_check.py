#!/usr/bin/env python3
"""Bounded design checks; stdlib SQLite only, never production SCHEMA.

Run from any directory: python3 docs/schema-candidate/logiqx_cmp_field_check.py
The independent SQL fixture supplies intentionally minimal common contracts.
"""

import csv
from pathlib import Path
import re
import sqlite3
import unittest


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
# Independent transcription of DOC11@38144, not inferred from candidate counts.
POSITION_WIRES = {
    "logiqx_document_attribute_positions": "build debug",
    "logiqx_clrmamepro_option_positions": "header forcemerging forcenodump forcepacking",
    "logiqx_romcenter_option_positions": "plugin rommode biosmode samplemode lockrommode lockbiosmode locksamplemode",
    "logiqx_game_attribute_positions": "name sourcefile isbios cloneof romof sampleof board rebuildto",
    "logiqx_release_attribute_positions": "name region language date default",
    "logiqx_bios_attribute_positions": "name description default",
    "logiqx_rom_attribute_positions": "name size crc sha1 md5 merge status date serial",
    "logiqx_disk_attribute_positions": "name sha1 md5 merge status",
    "logiqx_sample_attribute_positions": "name",
    "logiqx_archive_attribute_positions": "name",
    "logiqx_device_reference_attribute_positions": "name",
    "clrmamepro_header_field_positions": "name description version date author email homepage url comment category header forcemerging forcezipping forcepacking forcenodump",
    "clrmamepro_set_field_positions": "name cloneof description year manufacturer rebuildto sampleof region releaseyear releasemonth releaseday serial",
    "clrmamepro_rom_field_positions": "name size crc crc32 md5 sha1 merge date serial status nodump baddump",
}
TEXT_WIRES = {
    "logiqx_header_text_elements": "name description category version date author email homepage url comment",
    "logiqx_game_text_elements": "description year manufacturer",
}
OTHER_WIRES = {
    ("logiqx_game_comments", "comment", "<game>/<comment>"),
    ("logiqx_root_file_names", "file_name", "<datafile>/<file_name>"),
    ("logiqx_root_sha1_elements", "sha1", "<datafile>/<sha1>"),
    ("clrmamepro_samples", "sample", "sample"),
    ("clrmamepro_comments", "comment", ";comment"),
    ("clrmamepro_headers", "form", "clrmamepro"),
    ("clrmamepro_sets", "form", "game|set"),
}
DEFAULTS = [
    ("logiqx_documents", "debug", "debug_was_present", "no"),
    ("logiqx_games", "isbios", "isbios_was_present", "no"),
    ("logiqx_releases", "default", "default_was_present", "no"),
    ("logiqx_bios_sets", "is_default", "default_was_present", "no"),
    ("logiqx_roms", "status", "status_specified", "good"),
    ("logiqx_disks", "status", "status_specified", "good"),
    ("logiqx_clrmamepro_options", "forcemerging", "forcemerging_was_present", "split"),
    ("logiqx_clrmamepro_options", "forcenodump", "forcenodump_was_present", "obsolete"),
    ("logiqx_clrmamepro_options", "forcepacking", "forcepacking_was_present", "zip"),
    ("logiqx_romcenter_options", "rommode", "rommode_was_present", "split"),
    ("logiqx_romcenter_options", "biosmode", "biosmode_was_present", "split"),
    ("logiqx_romcenter_options", "samplemode", "samplemode_was_present", "merged"),
    ("logiqx_romcenter_options", "lockrommode", "lockrommode_was_present", "no"),
    ("logiqx_romcenter_options", "lockbiosmode", "lockbiosmode_was_present", "no"),
    ("logiqx_romcenter_options", "locksamplemode", "locksamplemode_was_present", "no"),
]


def quote(name):
    if not re.fullmatch(r"[a-z_][a-z0-9_]*", name):
        raise ValueError(name)
    return '"' + name + '"'


def read_fixture():
    lines = []
    for line in (HERE / "logiqx_cmp_field_witnesses.sql").read_text().splitlines():
        if line == ".bail on":
            continue
        if line.startswith(".read "):
            included = ROOT / line.removeprefix(".read ")
            if included.resolve() != HERE / "logiqx_cmp.sql":
                raise ValueError("unexpected fixture include")
            lines.append(included.read_text())
        else:
            lines.append(line)
    return "\n".join(lines)


def closed_codes(sql):
    # Deliberately reject unfamiliar constraint syntax instead of guessing.
    expression = re.search(r"field_kind INTEGER NOT NULL CHECK \(field_kind ([^)]+)\)", sql).group(1)
    if match := re.fullmatch(r"BETWEEN (\d+) AND (\d+)", expression):
        return set(range(int(match[1]), int(match[2]) + 1))
    if match := re.fullmatch(r"IN \(([\d, ]+)\)", expression + ")"):
        return {int(part) for part in match[1].split(",")}
    if match := re.fullmatch(r"= (\d+)", expression):
        return {int(match[1])}
    raise ValueError("unrecognized closed field_kind check: " + expression)


class FieldChecks(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.addCleanup(self.db.close)
        self.db.executescript(read_fixture())
        with (HERE / "logiqx_cmp-field-coverage.tsv").open(newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            self.assertEqual(reader.fieldnames, "family owner_table field_code wire_name value_owner value_column position_table presence default_rule evidence".split())
            self.ledger = list(reader)

    def rollback_probe(self, action):
        self.db.execute("SAVEPOINT probe")
        try:
            action()
        finally:
            self.db.execute("ROLLBACK TO probe")
            self.db.execute("RELEASE probe")

    def reject(self, sql, parameters=(), layer=None):
        with self.assertRaises(sqlite3.IntegrityError) as error:
            self.db.execute(sql, parameters)
        if layer == "SQLITE_CONSTRAINT_DATATYPE":
            # Some Python/SQLite header combinations expose code 3091 as
            # sqlite_errorname='unknown'. Still require the exact layer.
            self.assertEqual(error.exception.sqlite_errorcode, 3091)
        elif layer:
            self.assertEqual(error.exception.sqlite_errorname, layer)

    def test_ledger_actual_relations_columns_and_independent_inventory(self):
        columns = {}
        for name, in self.db.execute("SELECT name FROM sqlite_schema WHERE type IN ('table','view')"):
            columns[name] = {row[1] for row in self.db.execute("PRAGMA table_xinfo(" + quote(name) + ")")}
        actual = set()
        expected = set()
        for table, wires in POSITION_WIRES.items():
            for code, wire in enumerate(wires.split()):
                expected.add((table, str(code), "@" + wire if table.startswith("logiqx_") else wire))
        for table, wires in TEXT_WIRES.items():
            parent = "header" if table == "logiqx_header_text_elements" else "game"
            for wire in wires.split():
                expected.add((table, wire, f"<{parent}>/<{wire}>"))
        expected |= OTHER_WIRES
        for row in self.ledger:
            self.assertIn(row["family"], ("logiqx", "clrmamepro"))
            self.assertIn(row["owner_table"], columns)
            self.assertIn(row["value_owner"], columns)
            for column in row["value_column"].split(","):
                self.assertIn(column, columns[row["value_owner"]], row)
            for required in ("presence", "default_rule", "evidence"):
                self.assertTrue(row[required], row)
            if row["position_table"] != "-":
                self.assertIn(row["position_table"], POSITION_WIRES)
                key = (row["position_table"], row["field_code"], row["wire_name"])
            else:
                key = (row["owner_table"], row["field_code"], row["wire_name"])
            self.assertNotIn(key, actual, row)
            actual.add(key)
        self.assertEqual(actual, expected)

    def test_actual_closed_ddl_and_populated_codes(self):
        for table, wires in {**POSITION_WIRES, **TEXT_WIRES}.items():
            with self.subTest(table=table):
                expected = set(range(len(wires.split())))
                ddl = self.db.execute("SELECT sql FROM sqlite_schema WHERE name=?", (table,)).fetchone()[0]
                self.assertEqual(closed_codes(ddl), expected)
                populated = {r[0] for r in self.db.execute("SELECT field_kind FROM " + quote(table))}
                self.assertEqual(populated, expected)
                for bad, layer in [(-1, "SQLITE_CONSTRAINT_CHECK"), (max(expected)+1, "SQLITE_CONSTRAINT_CHECK"),
                                   (0.5, "SQLITE_CONSTRAINT_DATATYPE"), ("bad", "SQLITE_CONSTRAINT_DATATYPE"),
                                   (None, "SQLITE_CONSTRAINT_NOTNULL")]:
                    self.rollback_probe(lambda: self.reject("UPDATE " + quote(table) + " SET field_kind=? WHERE field_kind=0", (bad,), layer))

    def test_every_declared_position_has_field_presence_diagnostic_except_flags(self):
        for table, wires in POSITION_WIRES.items():
            pk = next(row[1] for row in self.db.execute("PRAGMA table_info(" + quote(table) + ")") if row[5] == 1)
            owner = self.db.execute("SELECT " + quote(pk) + " FROM " + quote(table) + " LIMIT 1").fetchone()[0]
            for code, wire in enumerate(wires.split()):
                with self.subTest(table=table, wire=wire):
                    def probe():
                        self.db.execute("DELETE FROM " + quote(table) + " WHERE field_kind=?", (code,))
                        reports = self.db.execute("SELECT problem,owner_id FROM candidate_logiqx_cmp_integrity_problems").fetchall()
                        if table == "clrmamepro_rom_field_positions" and code in (10, 11):
                            self.assertEqual(reports, [])  # Flag presence IS the declaration.
                        else:
                            self.assertIn(("missing_position", owner), reports)
                    self.rollback_probe(probe)

    def test_default_vocabulary_and_presence_storage_layers(self):
        for table, value, bit, default in DEFAULTS:
            with self.subTest(table=table, value=value):
                self.assertEqual(self.db.execute(f"SELECT {quote(value)},{quote(bit)} FROM {quote(table)}").fetchone(), (default, 1))
                for bad in ("", "invalid"):
                    self.reject(f"UPDATE {quote(table)} SET {quote(value)}=?", (bad,), "SQLITE_CONSTRAINT_CHECK")
                self.reject(f"UPDATE {quote(table)} SET {quote(bit)}=0.5", layer="SQLITE_CONSTRAINT_DATATYPE")
                self.reject(f"UPDATE {quote(table)} SET {quote(bit)}=2", layer="SQLITE_CONSTRAINT_CHECK")

    def test_required_empty_is_not_null_and_malformed_size_layers(self):
        required = {
            "catalog_sets": "set_name", "logiqx_roms": "name", "logiqx_disks": "name",
            "logiqx_samples": "name", "logiqx_releases": "name region",
            "logiqx_bios_sets": "name description", "logiqx_archive_references": "archive_name",
            "logiqx_device_references": "target_name", "clrmamepro_roms": "name",
            "clrmamepro_samples": "sample_name", "logiqx_header_text_elements": "text_value",
            "logiqx_game_text_elements": "text_value",
        }
        for table, names in required.items():
            for name in names.split():
                with self.subTest(table=table, column=name):
                    self.reject(f"UPDATE {quote(table)} SET {quote(name)}=NULL", layer="SQLITE_CONSTRAINT_NOTNULL")
        for malformed in ("", " 8", "8x", "-1", "9223372036854775808", "8\x00"):
            self.reject("UPDATE clrmamepro_roms SET size_text=?", (malformed,), "SQLITE_CONSTRAINT_CHECK")
            def probe():
                self.db.execute("UPDATE logiqx_roms SET size_text=?", (malformed,))
                self.assertEqual(self.db.execute("SELECT size_text,size_value FROM logiqx_roms").fetchone(), (malformed, None))
            self.rollback_probe(probe)

    def test_cmp_keyword_code_shape_and_root_sha1_update_layers(self):
        for table in POSITION_WIRES:
            if table.startswith("clrmamepro_"):
                self.reject("UPDATE " + quote(table) + " SET keyword='wrong' WHERE field_kind=0", layer="SQLITE_CONSTRAINT_CHECK")
        self.reject("UPDATE clrmamepro_rom_field_positions SET value_line=NULL WHERE field_kind=2", layer="SQLITE_CONSTRAINT_CHECK")
        self.reject("UPDATE clrmamepro_rom_field_positions SET value_column=0 WHERE field_kind=2", layer="SQLITE_CONSTRAINT_CHECK")
        self.reject("UPDATE logiqx_root_sha1_elements SET hash_id=1", layer="SQLITE_CONSTRAINT_TRIGGER")
        self.reject("UPDATE clrmamepro_sets SET source_block='wrong'", layer="SQLITE_CONSTRAINT_CHECK")
        def probe():
            self.db.execute("UPDATE clrmamepro_sets SET source_block='GaMe'")
            self.assertEqual(self.db.execute("SELECT source_block FROM clrmamepro_sets").fetchone()[0], "GaMe")
        self.rollback_probe(probe)

    def test_fresh_hash_reference_reaches_owner_mapping_report_not_unique(self):
        def probe():
            # Fresh declaration, not an already-claimed ID; valid field/bytes.
            self.db.execute("INSERT INTO catalog_entry_hashes VALUES(25,109,'crc',0,'value','unknown',1,NULL)")
            self.db.execute("UPDATE clrmamepro_rom_field_positions SET reported_hash_id=25 WHERE field_kind=2")
            self.assertIn(("cmp_hash_mapping", 2, 25), self.db.execute(
                "SELECT problem,edition_id,owner_id FROM candidate_logiqx_cmp_integrity_problems").fetchall())
        self.rollback_probe(probe)

    def test_field_assertions_reject_missing_and_multiple_selector_matches(self):
        fixture = read_fixture()
        selectors = (
            ("quoted empty CMP header scalar has both lexical anchors",
             "WHERE p.field_kind=0", "WHERE p.field_kind=999",
             "WHERE p.field_kind IN(0,1)"),
            ("unknown and empty CMP directives retain text without effective modes",
             "FROM clrmamepro_header_options WHERE header_id=201",
             "FROM clrmamepro_header_options WHERE header_id=999",
             "FROM clrmamepro_header_options CROSS JOIN (SELECT 1 UNION ALL SELECT 2) AS matches WHERE header_id=201"),
            ("present CMP header omitted forcenodump derives obsolete",
             "FROM clrmamepro_header_options WHERE header_id=201",
             "FROM clrmamepro_header_options WHERE header_id=999",
             "FROM clrmamepro_header_options CROSS JOIN (SELECT 1 UNION ALL SELECT 2) AS matches WHERE header_id=201"),
        )
        for label, selector, missing, multiple in selectors:
            pattern = r"INSERT INTO field_assert SELECT '" + re.escape(label) + r"',[^;]+;"
            statements = re.findall(pattern, fixture)
            self.assertEqual(len(statements), 1, label)
            statement = statements[0]
            self.assertEqual(statement.count(selector), 1, label)
            for case, replacement in (("missing-999", missing), ("multiple", multiple)):
                with self.subTest(assertion=label, case=case):
                    mutated = fixture.replace(statement, statement.replace(selector, replacement), 1)
                    probe = sqlite3.connect(":memory:")
                    try:
                        with self.assertRaises(sqlite3.IntegrityError) as error:
                            probe.executescript(mutated)
                        self.assertEqual(error.exception.sqlite_errorname, "SQLITE_CONSTRAINT_CHECK")
                        self.assertIn("ok=1", str(error.exception))
                    finally:
                        probe.close()

    def test_populated_parent_and_annotation_query_plans(self):
        for query in (
            "SELECT source_element_id,text_value FROM logiqx_header_text_elements WHERE header_id=101 ORDER BY source_order",
            "SELECT field_kind FROM clrmamepro_rom_field_positions WHERE media_entry_id=203 ORDER BY source_order",
            "SELECT comment_text FROM clrmamepro_comments WHERE edition_id=2 ORDER BY source_line,source_column",
        ):
            self.assertTrue(self.db.execute(query).fetchall())
            plan = " ".join(row[3] for row in self.db.execute("EXPLAIN QUERY PLAN " + query))
            self.assertIn("SEARCH", plan)
            self.assertNotIn("TEMP B-TREE", plan)


if __name__ == "__main__":
    unittest.main(verbosity=2)
