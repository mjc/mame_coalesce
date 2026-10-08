#!/usr/bin/env python3
"""Independent position/value closure tests against the composed design SQL.

Run: python3 -Werror::ResourceWarning docs/schema-candidate/logiqx_cmp_presence_check.py
Only in-memory SQLite is opened. No application schema/importer is used.
"""

import csv
from pathlib import Path
import re
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures

HERE = Path(__file__).resolve().parent
# Independent DOC11@38144 physical code domains; not inferred from route count.
DOMAINS = {
    "logiqx_document_attribute_positions": 2,
    "logiqx_clrmamepro_option_positions": 4,
    "logiqx_romcenter_option_positions": 7,
    "logiqx_game_attribute_positions": 8,
    "logiqx_release_attribute_positions": 5,
    "logiqx_bios_attribute_positions": 3,
    "logiqx_rom_attribute_positions": 9,
    "logiqx_disk_attribute_positions": 5,
    "logiqx_sample_attribute_positions": 1,
    "logiqx_archive_attribute_positions": 1,
    "logiqx_device_reference_attribute_positions": 1,
    "clrmamepro_header_field_positions": 15,
    "clrmamepro_set_field_positions": 12,
    "clrmamepro_rom_field_positions": 12,
}
DEFAULTS = (
    ("logiqx_documents", "debug", "debug_was_present", "no", 1),
    ("logiqx_games", "isbios", "isbios_was_present", "no", 2),
    ("logiqx_releases", "default", "default_was_present", "no", 4),
    ("logiqx_bios_sets", "is_default", "default_was_present", "no", 2),
    ("logiqx_roms", "status", "status_specified", "good", 6),
    ("logiqx_disks", "status", "status_specified", "good", 4),
    ("logiqx_clrmamepro_options", "forcemerging", "forcemerging_was_present", "split", 1),
    ("logiqx_clrmamepro_options", "forcenodump", "forcenodump_was_present", "obsolete", 2),
    ("logiqx_clrmamepro_options", "forcepacking", "forcepacking_was_present", "zip", 3),
    ("logiqx_romcenter_options", "rommode", "rommode_was_present", "split", 1),
    ("logiqx_romcenter_options", "biosmode", "biosmode_was_present", "split", 2),
    ("logiqx_romcenter_options", "samplemode", "samplemode_was_present", "merged", 3),
    ("logiqx_romcenter_options", "lockrommode", "lockrommode_was_present", "no", 4),
    ("logiqx_romcenter_options", "lockbiosmode", "lockbiosmode_was_present", "no", 5),
    ("logiqx_romcenter_options", "locksamplemode", "locksamplemode_was_present", "no", 6),
)
NULLABLE = {
    "logiqx_documents": (("build", 0),),
    "logiqx_clrmamepro_options": (("header", 0),),
    "logiqx_romcenter_options": (("plugin", 0),),
    "logiqx_games": (("sourcefile", 1), ("board", 6), ("rebuildto", 7)),
    "logiqx_releases": (("language", 2), ("date", 3)),
    "logiqx_roms": (("size_text", 1), ("date", 7)),
    "clrmamepro_headers": tuple((column, code) for code, column in enumerate(
        "name description version date author email homepage url comment category".split())),
    "clrmamepro_sets": (("description", 2), ("year", 3), ("manufacturer", 4),
                        ("rebuildto", 5), ("region", 7), ("releaseyear", 8),
                        ("releasemonth", 9), ("releaseday", 10), ("serial", 11)),
    "clrmamepro_roms": (("size_text", 1),),
}

# Literal events for the shared constructed fixture. Kept independent from the
# native count inventory and from the mapping suite's separately transcribed
# vectors; these make the ordinary full integrity audit clean at baseline.
WITNESS_EVENTS = {
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


def seal_transplanted_fixture(connection):
    """Seal the two constructed editions from literal fixture events."""
    for family, edition_id in (("logiqx", 1), ("clrmamepro", 2)):
        count_fixtures.seal(connection, family, edition_id, WITNESS_EVENTS[family])


def transplanted_fixture():
    """Reuse only constructed native INSERTs, never the witness's stub schema.

    Replace its two shortened common identity declarations and one placeholder
    relationship INSERT with the real common rows. Do not rewrite assertions.
    """
    source = (HERE / "logiqx_cmp_field_witnesses.sql").read_text()
    marker = ".read docs/schema-candidate/logiqx_cmp.sql\n\n"
    if source.count(marker) != 1:
        raise AssertionError("native fixture boundary changed")
    body = source.split(marker, 1)[1].split("CREATE TEMP TABLE field_assert", 1)[0]
    for table in ("catalog_reading_rules", "catalog_editions"):
        body, count = re.subn(r"INSERT INTO " + table + r" VALUES[^;]+;", "", body, count=1)
        if count != 1:
            raise AssertionError("common fixture boundary changed: " + table)
    relationships = []
    kinds = ("logiqx_cloneof", "logiqx_romof", "logiqx_sampleof", "logiqx_rom_merge",
             "logiqx_disk_merge", "logiqx_device_ref", "clrmamepro_cloneof",
             "clrmamepro_sampleof", "clrmamepro_rom_merge")
    for identity, kind in enumerate(kinds, 1):
        edition = 1 if kind.startswith("logiqx_") else 2
        relationships.append(f"INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES({identity},'fixture-{identity}','source',{edition});")
        relationships.append(f"INSERT INTO reported_catalog_relationships VALUES({identity},'{kind}');")
    body, count = re.subn(r"INSERT INTO reported_catalog_relationships VALUES[^;]+;",
                          "\n".join(relationships), body, count=1)
    if count != 1:
        raise AssertionError("relationship fixture boundary changed")
    shared = """
INSERT INTO catalog_publishers VALUES(1,'presence','Presence witness',NULL);
INSERT INTO catalogs VALUES(1,1,'presence','Presence witness');
INSERT INTO catalog_source_files VALUES(1,zeroblob(32),NULL,10000,'presence','zstd');
INSERT INTO catalog_coverage VALUES(1,'complete');
INSERT INTO catalog_reading_rules VALUES
 (1,'logiqx-presence','logiqx','compat','1.5','candidate','logiqx-declared-text-compat-v2'),
 (2,'cmp-presence','clrmamepro','compat','observed','candidate','clrmamepro-declared-text-compat-v1');
INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL),(2,1,1,2,1,NULL,NULL);
"""
    return shared + body


class PresenceChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.routes = assemble.field_presence_routes(("logiqx_cmp",))
        cls.by_code = {(row["position_table"], int(row["field_code"])): row for row in cls.routes}
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        # Compile the real assembler/generator once per class, not per case.
        cls.db.executescript(assemble.assemble())
        cls.db.executescript(transplanted_fixture())
        seal_transplanted_fixture(cls.db)
        cls.db.commit()

    def setUp(self):
        self.db.execute("PRAGMA foreign_keys=ON")
        if self._testMethodName == "test_detached_registry_does_not_hide_native_presence_problem":
            self.db.execute("PRAGMA foreign_keys=OFF")
        self.db.execute("SAVEPOINT presence_case")

    def tearDown(self):
        self.db.execute("ROLLBACK TO presence_case")
        self.db.execute("RELEASE presence_case")
        self.db.execute("PRAGMA foreign_keys=ON")

    def probe(self, action):
        self.db.execute("SAVEPOINT mutation")
        try:
            action()
        finally:
            self.db.execute("ROLLBACK TO mutation")
            self.db.execute("RELEASE mutation")

    def route(self, table, code):
        matches = [row for row in self.routes if row["owner_table"] == table and int(row["field_code"]) == code]
        self.assertEqual(len(matches), 1, (table, code))
        return matches[0]

    def owner_id(self, route):
        values = self.db.execute(f'SELECT {assemble.identifier(route["owner_key"])} FROM {assemble.identifier(route["owner_table"])}').fetchall()
        self.assertEqual(len(values), 1, route)
        return values[0][0]

    def problem(self, route):
        return "field_presence:" + route["position_table"] + ":" + route["field_code"]

    def reports(self):
        return set(self.db.execute("SELECT problem,owner_id,edition_id FROM candidate_field_presence_problems"))

    def expected(self, route, edition=None):
        owner = self.owner_id(route)
        if edition is None:
            edition = 2 if route["owner_table"].startswith("clrmamepro_") else 1
        return (self.problem(route), owner, edition)

    def position(self, route):
        table, key = assemble.identifier(route["position_table"]), assemble.identifier(route["position_owner"])
        rows = self.db.execute(f"SELECT * FROM {table} WHERE {key}=? AND field_kind=?", (self.owner_id(route), int(route["field_code"]))).fetchall()
        self.assertEqual(len(rows), 1, route)
        return rows[0]

    def remove_position(self, route):
        self.db.execute(f'DELETE FROM {assemble.identifier(route["position_table"])} WHERE {assemble.identifier(route["position_owner"])}=? AND field_kind=?',
                        (self.owner_id(route), int(route["field_code"])))

    def restore_position(self, route, row):
        self.db.execute(f'INSERT INTO {assemble.identifier(route["position_table"])} VALUES({",".join("?" for _ in row)})', row)

    def test_exact_physical_coverage_independent_of_route_count(self):
        expected = {(table, code) for table, limit in DOMAINS.items() for code in range(limit)}
        self.assertEqual(set(self.by_code), expected)
        self.assertEqual(len(self.routes), len(expected))
        with (HERE / "logiqx_cmp-field-coverage.tsv").open(newline="") as source:
            coverage = {(row["position_table"], int(row["field_code"])) for row in csv.DictReader(source, delimiter="\t") if row["position_table"] != "-"}
        self.assertEqual(coverage, expected)
        physical = {name for name, in self.db.execute("SELECT name FROM sqlite_schema WHERE type='table'")
                    if (name.startswith("logiqx_") or name.startswith("clrmamepro_")) and name.endswith("_positions")}
        self.assertEqual(physical, set(DOMAINS))
        for table, limit in DOMAINS.items():
            ddl = self.db.execute("SELECT sql FROM sqlite_schema WHERE name=?", (table,)).fetchone()[0]
            expression = re.search(r"field_kind INTEGER NOT NULL CHECK \(field_kind ([^)]+)\)", ddl).group(1)
            if expression.startswith("BETWEEN "):
                low, high = map(int, re.findall(r"\d+", expression))
                codes = set(range(low, high + 1))
            else:
                codes = set(map(int, re.findall(r"\d+", expression)))
            self.assertEqual(codes, set(range(limit)), table)
            self.assertEqual({r[0] for r in self.db.execute(f'SELECT field_kind FROM {assemble.identifier(table)}')}, codes)

    def test_baseline_is_real_composed_schema_and_clean(self):
        self.assertEqual(self.reports(), set())
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT problem,owner_id,edition_id FROM candidate_integrity_problems").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_cardinality_problems").fetchall(), [])
        gate = self.db.execute("SELECT sql FROM sqlite_schema WHERE name='candidate_publication_closure'").fetchone()[0]
        self.assertIn("candidate_integrity_problems", gate)
        chunks = [row[0] for row in self.db.execute(
            "SELECT sql FROM sqlite_schema WHERE type='view' AND name GLOB 'candidate_integrity_chunk_*'"
        )]
        self.assertTrue(any("candidate_logiqx_cmp_cardinality_problems" in sql for sql in chunks))
        self.assertTrue(self.db.execute("SELECT sql FROM sqlite_schema WHERE name='candidate_field_presence_problems'").fetchone())

    def test_each_value_backed_position_is_required_by_its_exact_native_owner(self):
        for route in self.routes:
            if route["position_table"] == "clrmamepro_rom_field_positions" and int(route["field_code"]) in (10, 11):
                continue
            with self.subTest(position=route["position_table"], code=route["field_code"]):
                def mutation():
                    self.remove_position(route)
                    expected = self.expected(route)
                    self.assertEqual(self.reports(), {expected})
                    self.assertIn(expected, set(self.db.execute("SELECT problem,owner_id,edition_id FROM candidate_integrity_problems")))
                self.probe(mutation)

    def test_optional_absent_empty_and_invented_positions(self):
        for table, fields in NULLABLE.items():
            for column, code in fields:
                route = self.route(table, code)
                with self.subTest(table=table, field=column):
                    def mutation():
                        saved = self.position(route)
                        self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(column)}=NULL')
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.remove_position(route)
                        self.assertEqual(self.reports(), set())
                        self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(column)}=?', ("0008" if table == "clrmamepro_roms" else "",))
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.restore_position(route, saved)
                        self.assertEqual(self.reports(), set())
                    self.probe(mutation)

    def test_all_fifteen_explicit_defaults_differ_from_omitted_defaults(self):
        for table, column, bit, value, code in DEFAULTS:
            route = self.route(table, code)
            with self.subTest(table=table, field=column):
                def mutation():
                    self.assertEqual(self.db.execute(f'SELECT {assemble.identifier(column)},{assemble.identifier(bit)} FROM {assemble.identifier(table)}').fetchone(), (value, 1))
                    saved = self.position(route)
                    self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(bit)}=0')
                    self.assertEqual(self.reports(), {self.expected(route)})
                    self.remove_position(route)
                    self.assertEqual(self.reports(), set())
                    self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(bit)}=1')
                    self.assertEqual(self.reports(), {self.expected(route)})
                    self.restore_position(route, saved)
                    self.assertEqual(self.reports(), set())
                self.probe(mutation)

    def test_nullable_payload_facets_do_not_default_or_erase_presence(self):
        cases = (
            ("clrmamepro_headers", "clrmamepro_header_options", "header_id", 201,
             (("header_definition", 10), ("forcemerging", 11), ("forcezipping", 12),
              ("forcepacking", 13), ("forcenodump", 14))),
            ("clrmamepro_roms", "clrmamepro_rom_details", "media_entry_id", 203,
             (("date", 7), ("serial", 8), ("status_text", 9))),
        )
        for owner, facet, key, identity, fields in cases:
            for column, code in fields:
                route = self.route(owner, code)
                with self.subTest(facet=facet, column=column):
                    def mutation():
                        saved = self.position(route)
                        self.db.execute(f'UPDATE {assemble.identifier(facet)} SET {assemble.identifier(column)}=NULL WHERE {assemble.identifier(key)}=?', (identity,))
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.remove_position(route)
                        self.assertEqual(self.reports(), set())
                        self.db.execute(f'UPDATE {assemble.identifier(facet)} SET {assemble.identifier(column)}=? WHERE {assemble.identifier(key)}=?', ("", identity))
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.restore_position(route, saved)
                        self.assertEqual(self.reports(), set())
                    self.probe(mutation)

    def test_relationship_and_compatibility_facets_require_exact_link_kind(self):
        cases = [
            ("logiqx_games", code, "logiqx_set_links", "set_id", 104, "link_kind", kind)
            for code, kind in ((3, "cloneof"), (4, "romof"), (5, "sampleof"))
        ] + [
            ("clrmamepro_sets", code, "clrmamepro_set_links", "set_id", 202, "link_kind", kind)
            for code, kind in ((1, "cloneof"), (6, "sampleof"))
        ] + [
            ("logiqx_roms", 5, "logiqx_rom_merges", "media_entry_id", 107, None, None),
            ("logiqx_disks", 3, "logiqx_disk_merges", "media_entry_id", 108, None, None),
            ("clrmamepro_roms", 6, "clrmamepro_rom_merges", "media_entry_id", 203, None, None),
            ("logiqx_roms", 8, "logiqx_rom_compatibility", "media_entry_id", 107, None, None),
        ]
        for owner, code, facet, key, identity, discriminator, kind in cases:
            route = self.route(owner, code)
            with self.subTest(facet=facet, kind=kind):
                def mutation():
                    where = assemble.identifier(key) + "=?"
                    values = (identity,)
                    if discriminator:
                        where += " AND " + assemble.identifier(discriminator) + "=?"
                        values += (kind,)
                    delete = f'DELETE FROM {assemble.identifier(facet)} WHERE {where}'
                    if facet != "logiqx_rom_compatibility":
                        # Existing relationship guards are stronger than the
                        # publication-time presence audit. Preserve that rule.
                        with self.assertRaisesRegex(sqlite3.IntegrityError, "remove draft positions"):
                            self.db.execute(delete, values)
                        self.remove_position(route)
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.db.execute(delete, values)
                    else:
                        self.db.execute(delete, values)
                        self.assertEqual(self.reports(), {self.expected(route)})
                        self.remove_position(route)
                    # Other kinds on the same typed owner cannot substitute
                    # for the removed kind. Shared relationship identity
                    # closure is separate from this presence-only assertion.
                    self.assertEqual(self.db.execute(f'SELECT ({route["present_sql"]}) FROM {assemble.identifier(owner)} AS owner').fetchone()[0], 0)
                    self.assertEqual(self.reports(), set())
                self.probe(mutation)

    def test_hash_presence_uses_declaration_field_and_occurrence_not_digest_bytes(self):
        cases = (
            ("logiqx_roms", 2, 11), ("logiqx_roms", 3, 12), ("logiqx_roms", 4, 13),
            ("logiqx_disks", 1, 14), ("logiqx_disks", 2, 15),
            ("clrmamepro_roms", 2, 21), ("clrmamepro_roms", 3, 22),
            ("clrmamepro_roms", 4, 23), ("clrmamepro_roms", 5, 24),
        )
        for table, code, declaration in cases:
            route = self.route(table, code)
            with self.subTest(table=table, code=code):
                def mutation():
                    self.remove_position(route)
                    self.assertEqual(self.reports(), {self.expected(route)})
                    # A declaration with the same owner/field but occurrence 1
                    # must not satisfy the singleton occurrence-0 route.
                    self.db.execute("UPDATE catalog_entry_hashes SET field_occurrence=1 WHERE reported_hash_id=?", (declaration,))
                    self.assertEqual(self.reports(), set())
                    self.db.execute("DELETE FROM catalog_entry_hashes WHERE reported_hash_id=?", (declaration,))
                    self.assertEqual(self.reports(), set())
                self.probe(mutation)
        # CMP crc and crc32 have equal interned bytes in this fixture. Removing
        # crc's declaration must not be filled by the remaining crc32 alias.
        route = self.route("clrmamepro_roms", 2)
        self.remove_position(route)
        self.db.execute("DELETE FROM catalog_entry_hashes WHERE reported_hash_id=21")
        self.assertEqual(self.reports(), set())
        self.assertEqual(self.db.execute("SELECT source_hash_field,hash_id FROM catalog_entry_hashes WHERE reported_hash_id=22").fetchone(), ("crc32", 1))

    def test_empty_and_invalid_logiqx_hashes_still_require_positions(self):
        route = self.route("logiqx_roms", 2)
        for presence, text in (("empty", None), ("invalid", "not-hex")):
            with self.subTest(presence=presence):
                def mutation():
                    self.db.execute("UPDATE catalog_entry_hashes SET presence=?,hash_id=NULL,reported_text=? WHERE reported_hash_id=11", (presence, text))
                    self.assertEqual(self.reports(), set())
                    self.remove_position(route)
                    self.assertEqual(self.reports(), {self.expected(route)})
                self.probe(mutation)

    def test_cmp_game_and_set_spelling_does_not_change_field_routes(self):
        for spelling in ("game", "GaMe", "set", "SeT"):
            self.db.execute("UPDATE clrmamepro_sets SET source_block=? WHERE set_id=202", (spelling,))
            self.assertEqual(self.reports(), set())
            self.assertEqual(self.db.execute("SELECT source_block FROM clrmamepro_sets WHERE set_id=202").fetchone()[0], spelling)

    def test_cmp_token_only_flags_are_not_invented_independent_values(self):
        for code in (10, 11):
            route = self.route("clrmamepro_roms", code)
            saved = self.position(route)
            self.assertIsNone(saved[7])  # value_line; keyword-only declaration.
            self.remove_position(route)
            self.assertEqual(self.reports(), set())
            self.restore_position(route, saved)
        self.assertEqual(self.db.execute("SELECT count(*) FROM clrmamepro_rom_details").fetchone()[0], 1)
        columns = {row[1] for row in self.db.execute("PRAGMA table_info(clrmamepro_rom_details)")}
        self.assertNotIn("nodump_present", columns)
        self.assertNotIn("baddump_present", columns)

    def test_equal_count_position_owner_substitution_is_not_presence(self):
        self.db.execute("INSERT INTO catalog_source_elements VALUES(150,1,'logiqx_game')")
        self.db.execute("INSERT INTO catalog_sets VALUES(150,10,'other',4,70,1)")
        self.db.execute("INSERT INTO logiqx_games VALUES(150,NULL,'no',0,NULL,NULL)")
        self.db.execute("INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column) VALUES(150,0,0,70,8)")
        before = self.db.execute("SELECT count(*) FROM logiqx_game_attribute_positions").fetchone()[0]
        self.db.execute("UPDATE logiqx_game_attribute_positions SET set_id=150 WHERE set_id=104 AND field_kind=1")
        self.assertEqual(self.db.execute("SELECT count(*) FROM logiqx_game_attribute_positions").fetchone()[0], before)
        problem = "field_presence:logiqx_game_attribute_positions:1"
        self.assertEqual(self.reports(), {(problem, 104, 1), (problem, 150, 1)})

    def test_detached_registry_does_not_hide_native_presence_problem(self):
        # Simulate pre-existing FK-off corruption, not an authorized write path:
        # remove only registry DELETE guards inside this rollback-only test.
        for name, in self.db.execute("SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name='catalog_source_elements'").fetchall():
            self.db.execute("DROP TRIGGER " + assemble.identifier(name))
        self.remove_position(self.route("logiqx_roms", 0))
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=107")
        expected = ("field_presence:logiqx_rom_attribute_positions:0", 107, None)
        self.assertIn(expected, self.reports())
        self.assertTrue(self.db.execute("PRAGMA foreign_key_check").fetchall())

    def test_publication_rejects_missing_and_invented_positions_then_locks_facts(self):
        for edition, table, key in ((1, "logiqx_documents", "build"), (2, "clrmamepro_headers", "name")):
            route = self.route(table, 0)
            for attack in ("missing", "invented"):
                with self.subTest(edition=edition, attack=attack):
                    def mutation():
                        if attack == "missing":
                            self.remove_position(route)
                        else:
                            self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(key)}=NULL')
                        self.assertEqual(self.reports(), {self.expected(route)})
                        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
                            self.db.execute("INSERT INTO published_catalog_editions VALUES(?,1,1,?,1,'presence')", (edition, edition))
                    self.probe(mutation)
            self.db.execute("INSERT INTO published_catalog_editions VALUES(?,1,1,?,1,'presence')", (edition, edition))
            with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
                self.remove_position(route)


if __name__ == "__main__":
    unittest.main(verbosity=2)
