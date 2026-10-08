#!/usr/bin/env python3
"""Candidate-only retained-field/position closure against actual assembled SQL.

Run in the active repository devenv:
  python3 -Werror::ResourceWarning docs/schema-candidate/mame_presence_check.py
Uses in-memory SQLite only; no production parser, import, build or count seal.
"""

from contextlib import contextmanager, closing
import argparse
import csv
from pathlib import Path
import re
import sqlite3
import sys
import unittest
from unittest.mock import patch

import assemble
import count_fixtures
import mame_field_check


HERE = Path(__file__).resolve().parent
HEADER = "owner_table owner_key position_table position_owner field_code present_sql".split()

# Literal receipt for the rows loaded from mame_field_witnesses.sql. The
# count-mapping fixture adds one ROM and its name position, so it supplies its
# own asymmetric vector instead.
MAME_FIELD_EVENTS = {
    "machine_count": 1,
    "machine_text_element_count": 3,
    "bios_set_count": 1,
    "rom_count": 1,
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
    "rom_attribute_position_count": 10,
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


def rows(name):
    with (HERE / name).open(newline="") as source:
        reader = csv.DictReader(source, delimiter="\t")
        return reader.fieldnames, list(reader)


def seed(db, *, seal_counts=True):
    """Reuse only concrete INSERTs, never the thin fixture's schema/TEMP audit."""
    db.execute("INSERT INTO catalog_publishers VALUES(1,'presence','Presence fixture',NULL)")
    db.execute("INSERT INTO catalogs VALUES(1,1,'presence','Presence fixture')")
    db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,10000,'constructed','zstd')", (b's' * 32,))
    db.execute("INSERT INTO catalog_decoded_xml_views VALUES(1,10000)")
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'presence-v3','mame','observed','0.289','constructed','mame-observed-compat-declared-text-v3')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")
    db.execute("INSERT INTO catalog_set_groups VALUES(10,1,'root')")
    source = (HERE / "mame_field_witnesses.sql").read_text()
    source = source.split("INSERT INTO mame_field_assertions", 1)[0]
    for line in source.splitlines():
        match = re.match(r"INSERT INTO (\w+)", line)
        if not match or match[1] in ("catalog_reading_rules", "catalog_editions", "catalog_set_groups"):
            continue
        if match[1] == "catalog_relationships":
            number = int(re.search(r"VALUES \((\d+),1\)", line)[1])
            db.execute("INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES(?,?,'source',1)",
                       (number, f"constructed-{number}"))
        else:
            db.execute(line)
    if seal_counts:
        count_fixtures.seal(db, "mame", 1, MAME_FIELD_EVENTS)
    db.commit()


class Inventory(unittest.TestCase):
    def test_exact_physical_codes_and_real_fk_owners(self):
        header, routes = rows("mame-field-presence.tsv")
        self.assertEqual(header, HEADER)
        _, coverage = rows("mame-field-coverage.tsv")
        mame_field_check.identity(coverage)  # Independent pinned DTD/compatibility identities.
        expected = {(row["position_table"], row["field_code"]) for row in coverage if row["position_table"] != "-"}
        actual = {(row["position_table"], row["field_code"]) for row in routes}
        self.assertEqual(len(routes), 134)
        self.assertEqual(len(actual), len(routes))
        self.assertEqual(actual, expected)
        with closing(sqlite3.connect(":memory:")) as db:
            # SQL declarations are data: sem does not expose their field CHECKs.
            db.executescript((HERE / "mame.sql").read_text())
            ddl_codes = set()
            position_tables = {row[0]: row[1] for row in db.execute(
                "SELECT name,sql FROM sqlite_schema WHERE type='table' AND name LIKE '%attribute_positions'")}
            self.assertEqual(len(position_tables), 33)
            for table, sql in position_tables.items():
                match = re.search(r"CHECK\s*\(\s*field_kind\s*(?:IN\s*\(([^)]+)\)|=\s*'([^']+)')", sql)
                self.assertIsNotNone(match)
                codes = set(re.findall(r"'([^']+)'", match[1])) if match[1] else {match[2]}
                ddl_codes.update((table, code) for code in codes)
            self.assertEqual(actual, ddl_codes)
            for route in routes:
                with self.subTest(table=route["position_table"], code=route["field_code"]):
                    parents = {(entry[2], entry[3], entry[4]) for entry in
                               db.execute(f'PRAGMA foreign_key_list({assemble.identifier(route["position_table"])})')}
                    self.assertIn((route["owner_table"], route["position_owner"], route["owner_key"]), parents)
                    self.assertNotRegex(route["present_sql"], r"[;\n\r]|\bNEW\.")


class ComposedPresence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        # Generated FK guards remain active. FK-OFF also permits the explicit,
        # savepoint-contained reverse-guard bypass used for registry corruption.
        cls.db.execute("PRAGMA foreign_keys=OFF")
        cls.db.executescript(assemble.assemble())  # Real generator/publication wiring, once per class.
        cls.routes = assemble.field_presence_routes(("mame",))
        cls.coverage = {(row["position_table"], row["field_code"]): row
                        for row in rows("mame-field-coverage.tsv")[1]}
        seed(cls.db)
        if cls.db.execute("SELECT * FROM candidate_integrity_problems").fetchall():
            raise AssertionError("constructed baseline has integrity problems: " + repr(
                cls.db.execute("SELECT * FROM candidate_integrity_problems").fetchall()))

    @contextmanager
    def probe(self):
        self.db.execute("SAVEPOINT mame_presence")
        try:
            yield
        finally:
            self.db.execute("ROLLBACK TO mame_presence")
            self.db.execute("RELEASE mame_presence")

    def problem(self, table, code):
        return "field_presence:" + table + ":" + code

    def findings(self, table, code):
        return self.db.execute("SELECT owner_id,edition_id FROM candidate_field_presence_problems WHERE problem=? ORDER BY owner_id",
                               (self.problem(table, code),)).fetchall()

    def publication_rejected(self):
        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
            self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")

    def test_every_physical_route_missing_position_blocks_publication(self):
        for route in self.routes:
            table, code = route["position_table"], route["field_code"]
            with self.subTest(table=table, code=code), self.probe():
                ids = [row[0] for row in self.db.execute(
                    f'SELECT {assemble.identifier(route["position_owner"])} FROM {assemble.identifier(table)} WHERE field_kind=?', (code,))]
                self.assertTrue(ids, "each physical code must have a constructed source owner")
                self.db.execute(f'DELETE FROM {assemble.identifier(table)} WHERE field_kind=?', (code,))
                self.assertEqual(self.findings(table, code), [(number, 1) for number in sorted(ids)])
                # The aggregate's exact same finding, not a separate TEMP audit.
                self.assertEqual(self.db.execute("SELECT owner_id,edition_id FROM candidate_integrity_problems WHERE problem=? ORDER BY owner_id",
                                                 (self.problem(table, code),)).fetchall(), [(number, 1) for number in sorted(ids)])
                self.publication_rejected()

    def test_absent_empty_required_cdata_and_uninterpretable_literals(self):
        table = "mame_machine_chips_attribute_positions"
        with self.probe():
            self.db.execute("UPDATE mame_chips SET clock='' WHERE source_element_id=205")
            self.assertEqual(self.findings(table, "clock"), [])
            self.db.execute("UPDATE mame_chips SET clock=NULL WHERE source_element_id=205")
            self.assertEqual(self.findings(table, "clock"), [(205, 1)])
            self.publication_rejected()
            self.db.execute(f"DELETE FROM {table} WHERE source_element_id=205 AND field_kind='clock'")
            self.assertEqual(self.findings(table, "clock"), [])
        # Empty required CDATA is present; source text is not numeric narrowing.
        self.assertEqual(self.db.execute("SELECT mameconfig FROM mame_documents").fetchone(), ("",))
        self.assertEqual(self.db.execute('SELECT "default" FROM mame_adjusters').fetchone(), ("source CDATA default",))
        self.assertEqual(self.db.execute('SELECT "default",text FROM mame_ram_options').fetchone(), ("not-a-boolean", "128K & direct text"))
        self.assertEqual(self.findings("mame_rom_claims_attribute_positions", "size"), [])

    def test_all_default_bits_and_both_switch_contexts(self):
        defaults = [route for route in self.routes if re.fullmatch(r'owner\."\w+_specified"=1', route["present_sql"])]
        self.assertEqual(len(defaults), 22)
        contexts = 0
        for route in defaults:
            specified = re.search(r'"(\w+)"', route["present_sql"])[1]
            table, key, position, code = (route[column] for column in ("owner_table", "owner_key", "position_table", "field_code"))
            ids = [row[0] for row in self.db.execute(f'SELECT {assemble.identifier(key)} FROM {assemble.identifier(table)}')]
            for number in ids:
                contexts += 1
                with self.subTest(table=table, code=code, owner=number), self.probe():
                    self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(specified)}=0 WHERE {assemble.identifier(key)}=?', (number,))
                    self.assertEqual(self.findings(position, code), [(number, 1)])  # Invented explicit-default position.
                    self.db.execute(f'DELETE FROM {assemble.identifier(position)} WHERE {assemble.identifier(route["position_owner"])}=? AND field_kind=?', (number, code))
                    self.assertEqual(self.findings(position, code), [])  # Absent with effective default.
                    self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(specified)}=1 WHERE {assemble.identifier(key)}=?', (number,))
                    self.assertEqual(self.findings(position, code), [(number, 1)])  # Explicit default needs position.
        self.assertEqual(contexts, 24)

    def test_each_nullable_native_field_preserves_empty_and_absent(self):
        for route in self.routes:
            match = re.fullmatch(r'owner\."(\w+)" IS NOT NULL', route["present_sql"])
            if not match:
                continue
            column = match[1]
            metadata = {row[1]: row for row in self.db.execute(
                f'PRAGMA table_info({assemble.identifier(route["owner_table"])})')}
            if metadata[column][3]:
                continue  # Required NOT NULL fields covered by position deletion.
            table, key, position, code = (route[name] for name in
                                          ("owner_table", "owner_key", "position_table", "field_code"))
            number = self.db.execute(f'SELECT {assemble.identifier(key)} FROM {assemble.identifier(table)}').fetchone()[0]
            with self.subTest(table=table, code=code), self.probe():
                policy = self.coverage[(position, code)]["presence"]
                # Closed optional enums cannot accept empty; literal CDATA can.
                if "present-empty" in policy or "empty/malformed/overflow" in policy:
                    self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(column)}=\'\' WHERE {assemble.identifier(key)}=?', (number,))
                    self.assertEqual(self.findings(position, code), [])
                self.db.execute(f'UPDATE {assemble.identifier(table)} SET {assemble.identifier(column)}=NULL WHERE {assemble.identifier(key)}=?', (number,))
                self.assertEqual(self.findings(position, code), [(number, 1)])
                self.db.execute(f'DELETE FROM {assemble.identifier(position)} WHERE {assemble.identifier(route["position_owner"])}=? AND field_kind=?', (number, code))
                self.assertEqual(self.findings(position, code), [])

    def test_relationship_presence_uses_exact_typed_declaration(self):
        for table, code, declaration, predicate in (
            *(("mame_machines_attribute_positions", code, "mame_machine_links",
               "machine_id=100 AND link_kind='" + code + "'") for code in ("cloneof", "romof", "sampleof")),
            ("mame_rom_claims_attribute_positions", "merge", "mame_rom_merges", "media_entry_id=201"),
            ("mame_disk_claims_attribute_positions", "merge", "mame_disk_merges", "media_entry_id=202"),
        ):
            with self.subTest(table=table, code=code), self.probe():
                # Exact identity guards require draft positions removed first.
                self.db.execute(f'DELETE FROM {assemble.identifier(table)} WHERE field_kind=?', (code,))
                self.assertEqual(len(self.findings(table, code)), 1)
                self.db.execute(f'DELETE FROM {assemble.identifier(declaration)} WHERE {predicate}')
                self.assertEqual(self.findings(table, code), [])
                if declaration == "mame_machine_links":
                    for other in {"cloneof", "romof", "sampleof"} - {code}:
                        self.assertEqual(self.findings(table, other), [])
        with self.probe(), self.assertRaisesRegex(sqlite3.IntegrityError, "exact typed declaration"):
            self.db.execute("UPDATE mame_device_references_attribute_positions SET relationship_id=401 WHERE source_element_id=203 AND field_kind='name'")

    def test_all_sparse_compatibility_fields_and_md5_without_payload(self):
        for route in (row for row in self.routes if "compatibility_attribute_positions" in row["position_table"]):
            table, code = route["position_table"], route["field_code"]
            with self.subTest(code=code), self.probe():
                if code == "md5":
                    continue  # All hash states tested separately below.
                if code == "isconsumable":
                    self.db.execute("DELETE FROM mame_machine_compatibility WHERE set_id=100")
                    number = 100
                elif code == "writeable":
                    self.db.execute("DELETE FROM mame_disk_compatibility WHERE media_entry_id=202")
                    number = 202
                else:
                    self.db.execute(f'UPDATE mame_rom_compatibility SET {assemble.identifier(code)}=NULL WHERE media_entry_id=201')
                    number = 201
                self.assertEqual(self.findings(table, code), [(number, 1)])
                self.publication_rejected()
                self.db.execute(f'DELETE FROM {assemble.identifier(table)} WHERE field_kind=?', (code,))
                self.assertEqual(self.findings(table, code), [])
        with self.probe():
            self.db.execute("DELETE FROM mame_rom_compatibility WHERE media_entry_id=201")
            self.db.execute("DELETE FROM mame_rom_compatibility_attribute_positions WHERE field_kind<>'md5'")
            self.assertEqual(self.db.execute("SELECT * FROM candidate_field_presence_problems").fetchall(), [])
        with self.probe():
            self.db.execute("UPDATE mame_disk_compatibility SET writeable=1 WHERE media_entry_id=202")
            self.assertEqual(self.db.execute("SELECT writable,writeable FROM mame_disks JOIN mame_disk_compatibility USING(media_entry_id)").fetchone(), (0, 1))
            self.assertEqual(self.findings("mame_disk_claims_attribute_positions", "writable"), [])
            self.assertEqual(self.findings("mame_disk_compatibility_attribute_positions", "writeable"), [])

    def test_exact_hash_fields_include_empty_and_invalid_states(self):
        for table, code, number in (("mame_rom_claims_attribute_positions", "crc", 501),
                                     ("mame_rom_claims_attribute_positions", "sha1", 502),
                                     ("mame_rom_compatibility_attribute_positions", "md5", 503),
                                     ("mame_disk_claims_attribute_positions", "sha1", 504)):
            with self.subTest(table=table, code=code), self.probe():
                self.db.execute("UPDATE catalog_entry_hashes SET presence='empty',hash_id=NULL,reported_text=NULL WHERE reported_hash_id=?", (number,))
                self.assertEqual(self.findings(table, code), [])
                self.db.execute("UPDATE catalog_entry_hashes SET presence='invalid',reported_text='invalid-hex' WHERE reported_hash_id=?", (number,))
                self.db.execute("INSERT INTO invalid_catalog_entry_hashes VALUES(?,'constructed_invalid_hex')", (number,))
                self.assertEqual(self.findings(table, code), [])
                self.db.execute(f'DELETE FROM {assemble.identifier(table)} WHERE field_kind=?', (code,))
                self.assertEqual(len(self.findings(table, code)), 1)
                self.publication_rejected()

    def test_crc_declaration_removal_does_not_select_sha1(self):
        with self.probe():
            self.db.execute("DELETE FROM mame_rom_claims_attribute_positions WHERE media_entry_id=201 AND field_kind='crc'")
            self.db.execute("DELETE FROM catalog_entry_hashes WHERE media_entry_id=201 AND source_hash_field='crc' AND field_occurrence=0")
            self.assertEqual(self.db.execute("SELECT reported_hash_id FROM catalog_entry_hashes WHERE media_entry_id=201 AND source_hash_field='sha1' AND field_occurrence=0").fetchall(), [(502,)])
            self.assertEqual(self.db.execute("SELECT reported_hash_id FROM mame_rom_claims_attribute_positions WHERE media_entry_id=201 AND field_kind='sha1'").fetchall(), [(502,)])
            self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
            self.assertEqual(self.findings("mame_rom_claims_attribute_positions", "sha1"), [])
            # CRC and its position are absent, regardless of the retained SHA1.
            # The presence slice stays clean; the independent event receipt
            # still detects deleting an accepted source position after import.
            self.assertEqual(self.findings("mame_rom_claims_attribute_positions", "crc"), [],
                             "native CRC presence must not select the retained SHA1 declaration")
            self.assertEqual(self.db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
            ).fetchall(), [("source_count:mame:rom_attribute_position_count", 1, 1)])
            self.publication_rejected()

    def test_crc_nonzero_declaration_does_not_supply_occurrence_zero(self):
        with self.probe():
            self.db.execute("DELETE FROM mame_rom_claims_attribute_positions WHERE media_entry_id=201 AND field_kind='crc'")
            self.db.execute("DELETE FROM catalog_entry_hashes WHERE media_entry_id=201 AND source_hash_field='crc' AND field_occurrence=0")
            # Synthetic out-of-MAME-contract declaration, not a position. The
            # real shared table permits nonnegative occurrences for all formats;
            # no CHECK, trigger, FK or UNIQUE bypass is needed for this control.
            self.db.execute("INSERT INTO catalog_entry_hashes VALUES(505,201,'crc',1,'empty','whole_file',NULL,NULL)")
            self.assertEqual(self.db.execute("SELECT field_occurrence FROM catalog_entry_hashes WHERE media_entry_id=201 AND source_hash_field='crc'").fetchall(), [(1,)])
            self.assertEqual(self.db.execute("SELECT count(*) FROM mame_rom_claims_attribute_positions WHERE media_entry_id=201 AND field_kind='crc'").fetchone(), (0,))
            self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
            self.assertEqual(self.findings("mame_rom_claims_attribute_positions", "crc"), [],
                             "native CRC occurrence-zero presence must ignore an occurrence-one-only declaration")
            # A separate hash-closure obligation still rejects this synthetic
            # unpositioned declaration. It cannot mask the native assertion above.
            self.assertEqual(self.db.execute("SELECT owner_id,edition_id FROM candidate_integrity_problems WHERE problem='hash_position_count'").fetchall(), [(505, 1)])
            self.publication_rejected()

    def test_equal_count_owner_substitution_and_joint_erasure_boundary(self):
        table = "mame_machine_chips_attribute_positions"
        with self.probe():
            self.db.execute("INSERT INTO catalog_source_elements VALUES(236,1,'mame_chip')")
            self.db.execute("INSERT INTO mame_chips VALUES(236,100,'',NULL,'cpu',NULL,22,43,3)")
            self.db.execute(f"INSERT INTO {table} VALUES(236,'name',0,0,43,10),(236,'type',0,1,43,20)")
            self.assertEqual(self.findings(table, "clock"), [])
            count = self.db.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
            self.db.execute(f"UPDATE {table} SET source_element_id=236 WHERE source_element_id=205 AND field_kind='clock'")
            self.assertEqual(self.db.execute(f"SELECT count(*) FROM {table}").fetchone()[0], count)
            self.assertEqual(self.findings(table, "clock"), [(205, 1), (236, 1)])
            self.publication_rejected()
        with self.probe():
            self.db.execute("UPDATE mame_chips SET clock=NULL WHERE source_element_id=205")
            self.db.execute(f"DELETE FROM {table} WHERE source_element_id=205 AND field_kind='clock'")
            self.assertEqual(self.findings(table, "clock"), [])  # Deliberate parser-count-seal limit.

    def test_detached_registry_does_not_hide_native_presence_problem(self):
        with self.probe():
            with self.assertRaises(sqlite3.IntegrityError):
                self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=205")
            # Explicit corruption injection only: remove reverse guards inside
            # this savepoint. All schema changes are rolled back after the probe.
            triggers = self.db.execute("SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name='catalog_source_elements'").fetchall()
            for (name,) in triggers:
                self.db.execute(f"DROP TRIGGER {assemble.identifier(name)}")
            self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=205")
            self.db.execute("DELETE FROM mame_machine_chips_attribute_positions WHERE source_element_id=205 AND field_kind='clock'")
            # LEFT JOIN must retain the owner even without registry scope.
            # Edition may be NULL or recovered through the real parent FK.
            finding = self.findings("mame_machine_chips_attribute_positions", "clock")
            self.assertEqual([row[0] for row in finding], [205])
            self.assertIn(finding[0][1], (None, 1))
            self.assertTrue(self.db.execute("PRAGMA foreign_key_check").fetchall())
            self.publication_rejected()

    def test_publication_accepts_baseline_and_freezes_values_and_positions(self):
        with self.probe():
            self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            for sql in ("UPDATE mame_chips SET clock='other' WHERE source_element_id=205",
                        "DELETE FROM mame_machine_chips_attribute_positions WHERE source_element_id=205 AND field_kind='clock'"):
                with self.subTest(sql=sql), self.assertRaises(sqlite3.IntegrityError):
                    self.db.execute(sql)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--crc-presence-mutant", choices=("sha1", "any-hash", "any-occurrence"))
    args, remaining = parser.parse_known_args()
    if args.crc_presence_mutant:
        original_routes = assemble.field_presence_routes

        def mutant_routes(families=assemble.FAMILIES):
            result = [dict(row) for row in original_routes(families)]
            route = next(row for row in result if row["position_table"] == "mame_rom_claims_attribute_positions" and row["field_code"] == "crc")
            before, after = {
                "sha1": ("value.source_hash_field='crc'", "value.source_hash_field='sha1'"),
                "any-hash": (" AND value.source_hash_field='crc'", ""),
                "any-occurrence": (" AND value.field_occurrence=0", ""),
            }[args.crc_presence_mutant]
            if route["present_sql"].count(before) != 1:
                raise ValueError("CRC mutant no longer matches the single expected predicate term")
            route["present_sql"] = route["present_sql"].replace(before, after)
            return tuple(result)

        # Test-only in-memory generator input, never a manifest/shared-file edit.
        # The actual assembler, DDL, guards and seed all still run unchanged.
        with patch.object(assemble, "field_presence_routes", mutant_routes):
            unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
    else:
        unittest.main(argv=[sys.argv[0], *remaining], verbosity=2)
