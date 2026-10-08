#!/usr/bin/env python3
"""Independent candidate-only field inventory and constructed SQLite witnesses.

Run from the repository devenv: python3 docs/schema-candidate/mame_field_check.py
No production modules, parser/import execution, builds or on-disk DB writes.
This does not provide an independent parser count seal or source-span proof.
"""

import csv
import io
from pathlib import Path
import re
import sqlite3
import unittest


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
HEADER = "family owner_table field_code wire_name value_owner value_column position_table presence default_rule evidence".split()
COMPAT = {
    ("compat.machine", "isconsumable"),
    *(("compat.rom", field) for field in
      ("md5", "soundonly", "dispose", "loadflag", "value", "inverted", "ovha", "nothread")),
    ("compat.disk", "writeable"),
}


def quote(name):
    if not re.fullmatch(r"[a-z_][a-z0-9_]*", name):
        raise ValueError(name)
    return '"' + name + '"'


def dictionary():
    dtd = (ROOT / "fixtures/specifications/mame-0.289.dtd").read_text()
    attributes = re.findall(r'<!ATTLIST\s+(\w+)\s+(\w+)\s+(CDATA|\([^>]+?\))\s+(#REQUIRED|#IMPLIED|"[^"]*")>', dtd)
    elements = dict(re.findall(r"<!ELEMENT\s+(\w+)\s+([^>]+)>", dtd))
    parents = {name for name, content in elements.items()
               if re.search(r"\bcondition\b", content)}
    expected = set(COMPAT)
    defaults = set()
    for element, field, _, presence in attributes:
        families = {parent + ".condition" for parent in parents} if element == "condition" else {element}
        expected.update((family, field) for family in families)
        if presence.startswith('"'):
            defaults.update((family, field) for family in families)
    expected.update(("ramoption.text" if name == "ramoption" else name,
                     "text" if name == "ramoption" else name)
                    for name, content in elements.items() if "#PCDATA" in content)
    return attributes, expected, defaults


def identity(rows):
    keys = [(row["family"], row["field_code"]) for row in rows]
    if len(keys) != len(set(keys)):
        raise ValueError("duplicate contextual field identity")
    _, expected, _ = dictionary()
    actual = set(keys)
    if actual != expected:
        raise ValueError(f"field identity mismatch: missing={expected - actual}; extra={actual - expected}")
    for row in rows:
        expected_wire = "#PCDATA" if row["position_table"] == "-" else row["field_code"]
        if row["wire_name"] != expected_wire:
            raise ValueError("wire/code identity mismatch")


def fixture():
    lines = []
    for line in (HERE / "mame_field_witnesses.sql").read_text().splitlines():
        if line.startswith(".read "):
            path = ROOT / line.removeprefix(".read ")
            if path.resolve() != HERE / "mame.sql":
                raise ValueError("unexpected fixture include")
            lines.append(path.read_text())
        elif not line.startswith("."):
            lines.append(line)
        elif line not in (".bail on", ".eqp on", ".eqp off"):
            raise ValueError("unexpected CLI directive")
    return "\n".join(lines)


class FieldChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        reader = csv.DictReader(io.StringIO((HERE / "mame-field-coverage.tsv").read_text()), delimiter="\t")
        if reader.fieldnames != HEADER:
            raise ValueError("incorrect ledger header")
        cls.rows = list(reader)
        cls.db = sqlite3.connect(":memory:")
        cls.executed = set()

        def trace(sql):
            match = re.search(r"INSERT INTO mame_field_assertions VALUES \('([^']+)'", sql)
            if match:
                cls.executed.add(match[1])

        cls.db.set_trace_callback(trace)
        cls.db.executescript(fixture())
        cls.db.set_trace_callback(None)

    @classmethod
    def tearDownClass(cls):
        cls.db.close()

    def columns(self, table, db=None):
        return {row[1] for row in (db or self.db).execute(f"PRAGMA table_xinfo({quote(table)})")}

    def test_independent_dictionary_identity(self):
        attributes, expected, _ = dictionary()
        self.assertEqual(len(attributes), 125)
        self.assertEqual(len(COMPAT), 10)
        self.assertEqual(len(expected), 155)  # 135 attributes + 16 condition-context expansions + four text nodes.
        identity(self.rows)
        self.assertTrue(all(all(row.get(key) for key in HEADER) for row in self.rows))

    def test_equal_count_identity_mutations_rejected(self):
        for family, code, wrong in (
            ("machine", "sourcefile", "invented"),
            ("compat.disk", "writeable", "writable"),
            ("configuration", "mask", "name"),
            ("confsetting", "default", "inverted"),
        ):
            rows = [dict(row) for row in self.rows]
            row = next(row for row in rows if (row["family"], row["field_code"]) == (family, code))
            row["field_code"] = wrong
            with self.subTest(family=family, code=code), self.assertRaises(ValueError):
                identity(rows)
        with self.assertRaises(ValueError):
            identity(self.rows[:-1])

    def test_current_and_candidate_column_references(self):
        current = sqlite3.connect(":memory:")
        self.addCleanup(current.close)
        # Only CREATE TABLE declarations for column inspection; no production
        # modules, fixture import, migration, view evaluation or persistent DB.
        for name in ("schema.sql", "mame_compatibility.sql", "mame_relationships.sql"):
            sql = (ROOT / "src/storage/db" / name).read_text()
            for statement in re.findall(r'CREATE TABLE\s+"?\w+"?\s*\([\s\S]*?\)\s*(?:WITHOUT ROWID)?\s*;', sql):
                current.execute(statement)
        for row in self.rows:
            with self.subTest(family=row["family"], code=row["field_code"]):
                self.assertTrue(self.columns(row["owner_table"]))
                for column in row["value_column"].split(","):
                    self.assertIn(column, self.columns(row["value_owner"]))
                match = re.search(r"current=(\w+)\.([\w,]+) ->", row["evidence"])
                self.assertIsNotNone(match)
                for column in match[2].split(","):
                    self.assertIn(column, self.columns(match[1], current))

    def test_source_macro_and_closed_position_codes(self):
        source = (ROOT / "src/mame/attributes.rs").read_text()
        macros = {name: set(re.findall(r'=>\s*"([^"]+)"', body)) for name, body in
                  re.findall(r"attribute_fields!\((\w+)\s*\{(.*?)\}\);", source, re.S)}
        self.assertEqual(sum(map(len, macros.values())), 126)
        ledgers = {}
        macro_rows = {}
        for row in self.rows:
            if row["position_table"] == "-":
                self.assertEqual(row["wire_name"], "#PCDATA")
                continue
            ledgers.setdefault(row["position_table"], set()).add(row["field_code"])
            macro = re.search(r"enum=(\w+)", row["evidence"])[1]
            macro_rows.setdefault(macro, set()).add(row["wire_name"])
        self.assertEqual(macro_rows, macros)
        tables = dict(self.db.execute("SELECT name,sql FROM sqlite_schema WHERE type='table' AND name LIKE '%attribute_positions'"))
        self.assertEqual(set(ledgers), set(tables))
        self.assertEqual(len(tables), 33)
        self.assertEqual(sum(map(len, ledgers.values())), 134)
        for table, sql in tables.items():
            with self.subTest(table=table):
                match = re.search(r"CHECK\s*\(\s*field_kind\s*(?:IN\s*\(([^)]+)\)|=\s*'([^']+)')", sql)
                self.assertIsNotNone(match)
                codes = set(re.findall(r"'([^']+)'", match[1])) if match[1] else {match[2]}
                self.assertEqual(ledgers[table], codes)
                populated = {row[0] for row in self.db.execute(f"SELECT DISTINCT field_kind FROM {quote(table)}")}
                self.assertEqual(populated, codes)

    def test_all_fields_and_default_states_executed(self):
        expected = {"field:" + row["family"] + ":" + row["field_code"] for row in self.rows}
        self.assertEqual({label for label in self.executed if label.startswith("field:")}, expected)
        _, _, defaults = dictionary()
        self.assertEqual(len(defaults), 24)
        stored = {(row["value_owner"], row["value_column"]) for row in self.rows
                  if (row["family"], row["field_code"]) in defaults}
        self.assertEqual(len(stored), 22)
        for prefix in ("default-absent:", "default-absence-audit:", "unspecified-nondefault:"):
            self.assertEqual({label.removeprefix(prefix) for label in self.executed if label.startswith(prefix)},
                             {family + ":" + code for family, code in defaults})
        for family, field, _, presence in dictionary()[0]:
            if presence == "#REQUIRED" and (family, field) != ("rom", "size"):
                families = {parent + ".condition" for parent in ("dipswitch", "configuration", "dipvalue", "confsetting", "adjuster")} if family == "condition" else {family}
                for context in families:
                    self.assertIn("required:" + context + ":" + field, self.executed)
        for context, code in (("rom", "crc"), ("rom", "sha1"), ("disk", "sha1"), ("compat.rom", "md5")):
            for state in ("absent", "empty", "invalid", "override"):
                self.assertIn(f"hash-{state}:{context}:{code}", self.executed)

    def test_corruption_controls_boundaries_and_populated_plan(self):
        for label in ("attack:position-deleted", "attack:absent-value-present-position",
                      "attack:invented-default-position", "attack:equal-count-move",
                      "boundary:joint-optional-erasure"):
            self.assertIn(label, self.executed)
        self.assertEqual(self.db.execute("SELECT * FROM mame_fixture_presence_problems").fetchall(), [])
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT * FROM candidate_mame_integrity_problems").fetchall(), [])
        plan = self.db.execute("EXPLAIN QUERY PLAN SELECT source_element_id FROM mame_displays WHERE machine_id=100 ORDER BY source_order").fetchall()
        self.assertTrue(any("mame_displays_parent_order" in row[3] for row in plan))


if __name__ == "__main__":
    unittest.main(verbosity=2)
