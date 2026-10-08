"""Software value/position closure against the actual composed design candidate.

Run in the active repository devenv:
    python3 -Werror::ResourceWarning docs/schema-candidate/software_presence_check.py

One assembled database and the existing constructed family fixture are prepared
per class. Test and case savepoints restore data and temporary corruption guards.
These are retained-field consistency checks, not parser count/EOF/recipe proof.
"""
import csv
from contextlib import contextmanager
from pathlib import Path
import re
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures

ROOT = Path(__file__).resolve().parent

# Literal receipt transcribed from software_field_witnesses.sql. The table
# owners and position rows remain the fixture input; omitted inventory routes
# intentionally mean zero.
SOFTWARE_FIELD_EVENTS = {
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
POSITION_COUNTERS = {
    "software_wrapper_attribute_positions": "wrapper_attribute_position_count",
    "software_list_attribute_positions": "list_attribute_position_count",
    "software_title_attribute_positions": "title_attribute_position_count",
    "software_title_info_attribute_positions": "title_info_attribute_position_count",
    "software_shared_feature_attribute_positions": "shared_feature_attribute_position_count",
    "software_part_attribute_positions": "part_attribute_position_count",
    "software_part_feature_attribute_positions": "part_feature_attribute_position_count",
    "software_part_switch_attribute_positions": "part_switch_attribute_position_count",
    "software_part_switch_value_attribute_positions": "part_switch_value_attribute_position_count",
    "software_data_area_attribute_positions": "data_area_attribute_position_count",
    "software_disk_area_attribute_positions": "disk_area_attribute_position_count",
    "software_rom_attribute_positions": "rom_attribute_position_count",
    "software_disk_attribute_positions": "disk_attribute_position_count",
}
HEADER = (
    "owner_table", "owner_key", "position_table", "position_owner",
    "field_code", "present_sql",
)

# Independent DOC-12 seq 38114 physical inventory and populated control owners.
# Field names/order are transcribed from the two exact crosswalks, not read from
# the presence manifest or inferred from its row count / generated SQL.
OWNERS = (
    ("software_wrapper_headers", "wrapper_id", "software_wrapper_attribute_positions", 10, "softwarelists", ("build",)),
    ("software_lists", "set_group_id", "software_list_attribute_positions", 20, "softwarelist", ("name", "description")),
    ("software_titles", "set_id", "software_title_attribute_positions", 100, "software", ("name", "cloneof", "supported")),
    ("software_title_info", "source_element_id", "software_title_info_attribute_positions", 120, "info", ("name", "value")),
    ("software_shared_features", "source_element_id", "software_shared_feature_attribute_positions", 122, "sharedfeat", ("name", "value")),
    ("software_parts", "part_id", "software_part_attribute_positions", 200, "part", ("name", "interface")),
    ("software_part_features", "source_element_id", "software_part_feature_attribute_positions", 210, "feature", ("name", "value")),
    ("software_part_switches", "switch_id", "software_part_switch_attribute_positions", 220, "dipswitch", ("name", "tag", "mask")),
    ("software_part_switch_values", "source_element_id", "software_part_switch_value_attribute_positions", 230, "dipvalue", ("name", "value", "default")),
    ("software_data_areas", "area_id", "software_data_area_attribute_positions", 300, "dataarea", ("name", "size", "width", "endianness")),
    ("software_disk_areas", "area_id", "software_disk_area_attribute_positions", 302, "diskarea", ("name",)),
    ("software_rom_load_entries", "media_entry_id", "software_rom_attribute_positions", 400, "rom", ("name", "size", "crc", "sha1", "offset", "value", "status", "loadflag")),
    ("software_disks", "media_entry_id", "software_disk_attribute_positions", 410, "disk", ("name", "sha1", "status", "writeable")),
)
OPTIONALS = (
    ("software_wrapper_headers", "wrapper_id", 10, "build", "software_wrapper_attribute_positions", 0, ""),
    ("software_lists", "set_group_id", 20, "description", "software_list_attribute_positions", 1, ""),
    ("software_title_info", "source_element_id", 120, "value", "software_title_info_attribute_positions", 1, ""),
    ("software_shared_features", "source_element_id", 122, "value", "software_shared_feature_attribute_positions", 1, ""),
    ("software_part_features", "source_element_id", 210, "value", "software_part_feature_attribute_positions", 1, ""),
    ("software_rom_load_entries", "media_entry_id", 400, "name", "software_rom_attribute_positions", 0, ""),
    ("software_rom_load_entries", "media_entry_id", 400, "size_text", "software_rom_attribute_positions", 1, ""),
    ("software_rom_load_entries", "media_entry_id", 400, "offset_text", "software_rom_attribute_positions", 4, ""),
    ("software_rom_load_entries", "media_entry_id", 400, "value", "software_rom_attribute_positions", 5, ""),
    ("software_rom_load_entries", "media_entry_id", 400, "loadflag", "software_rom_attribute_positions", 7, "load16_byte"),
)
DEFAULTS = (
    ("software_titles", "set_id", 100, "supported_specified", "software_title_attribute_positions", 2),
    ("software_part_switch_values", "source_element_id", 230, "default_specified", "software_part_switch_value_attribute_positions", 2),
    ("software_data_areas", "area_id", 300, "width_specified", "software_data_area_attribute_positions", 2),
    ("software_data_areas", "area_id", 300, "endianness_specified", "software_data_area_attribute_positions", 3),
    ("software_rom_load_entries", "media_entry_id", 400, "status_specified", "software_rom_attribute_positions", 6),
    ("software_disks", "media_entry_id", 410, "status_specified", "software_disk_attribute_positions", 2),
    ("software_disks", "media_entry_id", 410, "writeable_specified", "software_disk_attribute_positions", 3),
)
HASH_STATES = (
    ("software_rom_attribute_positions", 2, ((400, "empty"), (403, "invalid"), (402, "value"))),
    ("software_rom_attribute_positions", 3, ((403, "empty"), (400, "invalid"), (402, "value"))),
    ("software_disk_attribute_positions", 1, ((410, "empty"), (412, "invalid"), (413, "value"))),
)


def quoted(name):
    if not re.fullmatch(r"[a-z][a-z0-9_]*", name):
        raise ValueError(f"not a SQL identifier: {name!r}")
    return '"' + name + '"'


def inventory(rows):
    result = {}
    for row in rows:
        slot = (row["position_table"], row["field_code"])
        if slot in result:
            raise AssertionError(f"duplicate physical position slot: {slot}")
        result[slot] = tuple(row[key] for key in HEADER[:4])
    return result


def expected_inventory():
    return {
        (position, str(code)): (table, key, position, key)
        for table, key, position, _, _, fields in OWNERS
        for code, _ in enumerate(fields)
    }


class SoftwarePresence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.routes = assemble.field_presence_routes(("software",))
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        # assemble() invokes the real shared generator and publication wiring.
        # No substitute view / no family-specific fake publication trigger.
        cls.db.executescript(assemble.assemble())
        cls.db.executescript((ROOT / "software_field_witnesses.sql").read_text())
        count_fixtures.seal(cls.db, "software", 1, SOFTWARE_FIELD_EVENTS)
        baseline = cls.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
        ).fetchall()
        if baseline:
            raise AssertionError(f"software witness baseline is not clean: {baseline}")
        cls.db.commit()
        # Explicit generated FK guards remain active. OFF is needed only for
        # deliberate corruption after guards are dropped within a test savepoint.
        # PRAGMA cannot be changed once the per-test savepoint has begun.
        cls.db.execute("PRAGMA foreign_keys=OFF")

    def setUp(self):
        self.db.execute("SAVEPOINT software_presence_test")

    def tearDown(self):
        self.db.execute("ROLLBACK TO software_presence_test")
        self.db.execute("RELEASE software_presence_test")

    @contextmanager
    def case(self):
        self.db.execute("SAVEPOINT software_presence_case")
        try:
            yield
        finally:
            self.db.execute("ROLLBACK TO software_presence_case")
            self.db.execute("RELEASE software_presence_case")

    def clean(self):
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
        ).fetchall(), [])

    def field_presence_clean(self, counter):
        # The deleted position no longer has a presence violation, but its
        # independent source event remains sealed and keeps publication closed.
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_field_presence_problems"
        ).fetchall(), [])
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
        ).fetchall(), [(f"source_count:software:{counter}", 1, 1)])
        self.blocked()

    def blocked(self):
        with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate publication requires complete closure"):
            self.publish()

    def publish(self):
        self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")

    def problem(self, position, code, owner, edition=1):
        name = f"field_presence:{position}:{code}"
        expected = [(owner, edition)]
        self.assertEqual(self.db.execute(
            "SELECT owner_id,edition_id FROM candidate_field_presence_problems WHERE problem=? AND owner_id=?",
            (name, owner),
        ).fetchall(), expected)
        self.assertEqual(self.db.execute(
            "SELECT owner_id,edition_id FROM candidate_integrity_problems WHERE problem=? AND owner_id=?",
            (name, owner),
        ).fetchall(), expected)

    def take_position(self, position, key, owner, code):
        where = f"{quoted(key)}=? AND field_kind=? AND field_occurrence=0"
        row = self.db.execute(f"SELECT * FROM {quoted(position)} WHERE {where}", (owner, code)).fetchone()
        self.assertIsNotNone(row, (position, owner, code))
        cursor = self.db.execute(f"DELETE FROM {quoted(position)} WHERE {where}", (owner, code))
        self.assertEqual(cursor.rowcount, 1)
        return row

    def restore_position(self, position, row):
        placeholders = ",".join("?" for _ in row)
        self.db.execute(f"INSERT INTO {quoted(position)} VALUES({placeholders})", row)

    def corrupt_delete(self, table, key, owner):
        # Deliberate bypass only, in an in-memory rollback savepoint; the normal
        # FK guards reject this deletion. No persistent candidate DDL changes.
        self.assertEqual(self.db.execute("PRAGMA foreign_keys").fetchone()[0], 0)
        triggers = self.db.execute(
            "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=?", (table,)
        ).fetchall()
        self.assertTrue(triggers)
        for (name,) in triggers:
            self.db.execute(f"DROP TRIGGER {quoted(name)}")
        cursor = self.db.execute(f"DELETE FROM {quoted(table)} WHERE {quoted(key)}=?", (owner,))
        self.assertEqual(cursor.rowcount, 1)

    def test_exact_independent_physical_inventory_and_predicate_shape(self):
        self.assertEqual(inventory(self.routes), expected_inventory())
        with (ROOT / "software-field-presence.tsv").open(newline="") as source:
            self.assertEqual(tuple(csv.DictReader(source, delimiter="\t").fieldnames), HEADER)
        with (ROOT / "software-field-coverage.tsv").open(newline="") as source:
            fields = list(csv.DictReader(source, delimiter="\t"))
        actual = {(row["position_table"], row["field_code"], row["wire_name"])
                  for row in fields if row["position_table"] != "-"}
        expected = {(position, str(code), f"{element}/@{name}")
                    for _, _, position, _, element, names in OWNERS
                    for code, name in enumerate(names)}
        self.assertEqual(actual, expected)
        for _, _, position, _, _, names in OWNERS:
            ddl = self.db.execute(
                "SELECT sql FROM sqlite_schema WHERE name=?", (position,)
            ).fetchone()[0]
            domain = re.search(r"field_kind INTEGER NOT NULL CHECK \(field_kind (?:BETWEEN (\d+) AND (\d+)|= (\d+))\)", ddl)
            self.assertIsNotNone(domain, position)
            codes = {int(domain[3])} if domain[3] is not None else set(range(int(domain[1]), int(domain[2]) + 1))
            self.assertEqual(codes, set(range(len(names))))
            self.assertIn("CHECK (field_occurrence = 0)", ddl)
        for row in self.routes:
            with self.subTest(position=row["position_table"], code=row["field_code"]):
                self.assertFalse(any(char in row["present_sql"] for char in ";\n\r"))
                self.assertNotRegex(row["present_sql"], r"\bNEW\s*\.")
                parents = {}
                for fk in self.db.execute(f"PRAGMA foreign_key_list({quoted(row['position_table'])})"):
                    parents.setdefault(fk[0], []).append(tuple(fk[i] for i in (2, 3, 4)))
                self.assertIn([(row["owner_table"], row["position_owner"], row["owner_key"])], parents.values())
                primary = {column[1] for column in self.db.execute(
                    f"PRAGMA table_info({quoted(row['owner_table'])})"
                ) if column[5]}
                self.assertEqual(primary, {row["owner_key"]})
                self.db.execute(
                    f"SELECT ({row['present_sql']}) FROM {quoted(row['owner_table'])} AS owner LIMIT 0"
                )

    def test_all_missing_codes_are_detected_per_owner_and_block_publication(self):
        self.clean()
        for _, key, position, owner, _, fields in OWNERS:
            for code, field in enumerate(fields):
                with self.subTest(position=position, field=field), self.case():
                    self.take_position(position, key, owner, code)
                    self.problem(position, code, owner)
                    self.blocked()

    def test_optional_absent_empty_missing_and_invented_positions(self):
        for table, key, owner, column, position, code, present_value in OPTIONALS:
            with self.subTest(table=table, column=column), self.case():
                # Native omission with a retained position is invented evidence.
                update = f"UPDATE {quoted(table)} SET {quoted(column)}=? WHERE {quoted(key)}=?"
                self.db.execute(update, (None, owner))
                self.problem(position, code, owner)
                self.blocked()
                saved = self.take_position(position, key, owner, code)
                self.field_presence_clean(POSITION_COUNTERS[position])
                self.db.execute(update, (present_value, owner))
                self.problem(position, code, owner)  # empty still requires a position.
                self.blocked()
                self.restore_position(position, saved)
                self.clean()

    def test_seven_explicit_default_pairs_use_presence_not_effective_value(self):
        for table, key, owner, bit, position, code in DEFAULTS:
            with self.subTest(table=table, bit=bit), self.case():
                # Every control owner stores the effective default with bit=1.
                self.assertEqual(self.db.execute(
                    f"SELECT {quoted(bit)} FROM {quoted(table)} WHERE {quoted(key)}=?", (owner,)
                ).fetchone()[0], 1)
                self.db.execute(
                    f"UPDATE {quoted(table)} SET {quoted(bit)}=0 WHERE {quoted(key)}=?", (owner,)
                )
                self.problem(position, code, owner)
                self.blocked()
                saved = self.take_position(position, key, owner, code)
                self.field_presence_clean(POSITION_COUNTERS[position])
                self.db.execute(
                    f"UPDATE {quoted(table)} SET {quoted(bit)}=1 WHERE {quoted(key)}=?", (owner,)
                )
                self.problem(position, code, owner)
                self.blocked()
                self.restore_position(position, saved)
                self.clean()

    def test_hash_empty_invalid_and_value_declarations_all_require_positions(self):
        for position, code, states in HASH_STATES:
            for owner, state in states:
                with self.subTest(position=position, code=code, state=state), self.case():
                    actual = self.db.execute(
                        f"SELECT hash.presence FROM {quoted(position)} AS pos "
                        "JOIN catalog_entry_hashes AS hash USING(reported_hash_id) "
                        "WHERE pos.media_entry_id=? AND pos.field_kind=?", (owner, code)
                    ).fetchone()[0]
                    self.assertEqual(actual, state)
                    self.take_position(position, "media_entry_id", owner, code)
                    self.problem(position, code, owner)
                    self.blocked()
        # A position with a lost canonical declaration is invented, not absent
        # merely because normalized bytes or a declaration join disappeared.
        with self.case():
            self.corrupt_delete("catalog_entry_hashes", "reported_hash_id", 600)
            self.problem("software_rom_attribute_positions", 2, 400)
            self.blocked()

    def test_empty_clone_and_exact_typed_facet_owner(self):
        self.assertEqual(self.db.execute(
            "SELECT target_name,relationship_id FROM software_clone_links WHERE set_id=100"
        ).fetchone(), ("", 900))
        self.take_position("software_title_attribute_positions", "set_id", 100, 1)
        self.problem("software_title_attribute_positions", 1, 100)
        # Move the typed literal, keeping its identity and edition stable. The
        # expected position moves too; no same-name or other-title fan-out.
        self.db.execute("UPDATE software_clone_links SET set_id=101 WHERE set_id=100")
        self.problem("software_title_attribute_positions", 1, 101)
        self.assertEqual(self.db.execute(
            "SELECT owner_id FROM candidate_field_presence_problems "
            "WHERE problem='field_presence:software_title_attribute_positions:1'"
        ).fetchall(), [(101,)])
        self.blocked()
        self.db.execute(
            "INSERT INTO software_title_attribute_positions"
            "(set_id,field_kind,source_order,source_line,source_column,relationship_id)"
            " VALUES(101,1,1,52,20,900)"
        )
        self.clean()

    def test_crc_nonzero_occurrence_only_does_not_invent_canonical_presence(self):
        self.clean()
        self.take_position("software_rom_attribute_positions", "media_entry_id", 400, 2)
        deleted = self.db.execute("DELETE FROM catalog_entry_hashes WHERE reported_hash_id=600")
        self.assertEqual(deleted.rowcount, 1)
        self.db.execute(
            "INSERT INTO catalog_entry_hashes"
            "(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,"
            "presence,hash_scope,hash_id,reported_text)"
            " VALUES(609,400,'crc',1,'empty','unknown',NULL,NULL)"
        )
        # No guard bypass: the shared declaration table admits occurrence 1,
        # but it cannot stand in for software's canonical occurrence-0 field.
        self.assertEqual(self.db.execute(
            "SELECT reported_hash_id,field_occurrence FROM catalog_entry_hashes "
            "WHERE media_entry_id=400 AND source_hash_field='crc'"
        ).fetchall(), [(609, 1)])
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM software_rom_attribute_positions "
            "WHERE media_entry_id=400 AND field_kind=2"
        ).fetchone()[0], 0)
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_field_presence_problems"
        ).fetchall(), [])
        # The staged occurrence-1 declaration is independently unpublishable
        # for lack of a canonical position; those audits are not our oracle.
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems ORDER BY problem"
        ).fetchall(), [
            ("hash_position_count", 609, 1),
            ("software_hash_without_matching_position", 609, 1),
            ("source_count:software:rom_attribute_position_count", 1, 1),
        ])

    def test_equal_count_owner_substitution_is_not_a_count_seal(self):
        position = "software_title_info_attribute_positions"
        before = self.db.execute(f"SELECT count(*) FROM {position}").fetchone()[0]
        self.db.execute(
            f"UPDATE {position} SET source_element_id=121 WHERE source_element_id=120 AND field_kind=1"
        )
        self.assertEqual(self.db.execute(f"SELECT count(*) FROM {position}").fetchone()[0], before)
        self.assertEqual(self.db.execute(
            "SELECT owner_id,edition_id FROM candidate_field_presence_problems "
            "WHERE problem='field_presence:software_title_info_attribute_positions:1' ORDER BY owner_id"
        ).fetchall(), [(120, 1), (121, 1)])
        self.blocked()

    def test_detached_registry_and_root_group_do_not_hide_native_owners(self):
        cases = (
            ("catalog_source_elements", "source_element_id", 120,
             "software_title_info_attribute_positions", "source_element_id", 120, 1),
            ("catalog_set_groups", "set_group_id", 20,
             "software_list_attribute_positions", "set_group_id", 20, 1),
        )
        for parent, parent_key, parent_id, position, key, owner, code in cases:
            with self.subTest(parent=parent), self.case():
                self.take_position(position, key, owner, code)
                # Actual guards prevent detachment before any deliberate bypass.
                with self.assertRaises(sqlite3.IntegrityError):
                    self.db.execute(
                        f"DELETE FROM {quoted(parent)} WHERE {quoted(parent_key)}=?", (parent_id,)
                    )
                self.corrupt_delete(parent, parent_key, parent_id)
                self.problem(position, code, owner, edition=None)
                self.assertTrue(self.db.execute("PRAGMA foreign_key_check").fetchall())
                # Unknown edition stays NULL; do not falsely attribute a
                # detached owner's corruption to an edition publication gate.

    def test_required_field_gap_blocks_then_repair_publishes_and_freezes(self):
        position = "software_part_attribute_positions"
        saved = self.take_position(position, "part_id", 200, 1)
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
        ).fetchall(), [
            (f"field_presence:{position}:1", 200, 1),
            ("source_count:software:part_attribute_position_count", 1, 1),
        ])
        self.blocked()
        self.restore_position(position, saved)
        self.clean()
        self.publish()
        for statement in (
            "UPDATE software_title_info SET value=NULL WHERE source_element_id=120",
            "DELETE FROM software_title_info_attribute_positions WHERE source_element_id=120 AND field_kind=1",
            "UPDATE catalog_sets SET set_name='changed' WHERE set_id=100",
        ):
            with self.subTest(statement=statement), self.assertRaisesRegex(
                sqlite3.IntegrityError, "published candidate payload and positions are immutable"
            ):
                self.db.execute(statement)

    def test_actual_area_facet_edition_plans_use_indexed_fk_and_registry(self):
        definitions = self.db.execute(
            "SELECT sql FROM sqlite_schema WHERE type='view' "
            "AND name GLOB 'candidate_field_presence_chunk_*'"
        ).fetchall()
        for position in (
            "software_data_area_attribute_positions",
            "software_disk_area_attribute_positions",
        ):
            with self.subTest(position=position):
                # Isolate the installed audit branch, not a handwritten query
                # or a second generator. Other families share the chunk views.
                marker = f"'field_presence:{position}:0' AS problem"
                branches = [branch for (definition,) in definitions
                            for branch in definition.split(" AS ", 1)[1].split(" UNION ALL ")
                            if marker in branch]
                self.assertEqual(len(branches), 1)
                plan = [row[3] for row in self.db.execute(
                    "EXPLAIN QUERY PLAN SELECT * FROM (" + branches[0] + ") WHERE edition_id=?",
                    (1,),
                )]
                details = "\n".join(plan)
                self.assertRegex(details, r"SEARCH field_scope USING .*\(edition_id=\?\)")
                self.assertRegex(details, r"SEARCH owner USING .*PRIMARY KEY")
                self.assertNotRegex(details, r"\bSCAN (?:owner|field_parent|field_scope|catalog_source_elements)\b")
                self.assertNotIn("scope_0", details)  # old scalar ancestry fallback
                self.assertNotIn("field_parent", branches[0])  # chained LEFT JOIN also scanned

    def test_both_area_facets_preserve_truthful_scope_after_detachment(self):
        for position, owner in (
            ("software_data_area_attribute_positions", 300),
            ("software_disk_area_attribute_positions", 302),
        ):
            for removed, edition in (
                ((("software_areas", "area_id"),), 1),
                ((("catalog_source_elements", "source_element_id"),), None),
                ((("software_areas", "area_id"), ("catalog_source_elements", "source_element_id")), None),
            ):
                with self.subTest(position=position, removed=removed), self.case():
                    self.take_position(position, "area_id", owner, 0)
                    self.problem(position, 0, owner)
                    parent, key = removed[0]
                    with self.assertRaises(sqlite3.IntegrityError):
                        self.db.execute(
                            f"DELETE FROM {quoted(parent)} WHERE {quoted(key)}=?", (owner,)
                        )
                    for parent, key in removed:
                        self.corrupt_delete(parent, key, owner)
                    # The actual parent FK is the issued source ID: surviving
                    # registry identity truthfully scopes a missing payload.
                    # Registry loss (alone or with payload) is unknown here.
                    self.problem(position, 0, owner, edition=edition)
                    self.assertTrue(self.db.execute("PRAGMA foreign_key_check").fetchall())
                    if edition is not None:
                        self.blocked()


if __name__ == "__main__":
    unittest.main(verbosity=2)
