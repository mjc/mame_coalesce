#!/usr/bin/env python3
"""Independent composed-SQL witnesses for native MAME child cardinality.

Run in the repository devenv:
  python3 -Werror::ResourceWarning docs/schema-candidate/mame_cardinality_check.py
The checker uses in-memory SQLite and constructed rows only. It is not parser,
source-corpus, or persisted-count-seal evidence.
"""

from contextlib import closing
from pathlib import Path
import re
import sqlite3
import unittest

import assemble
import mame_presence_check


HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
VIEW = "candidate_mame_cardinality_problems"


def seed(db):
    db.execute("INSERT INTO catalog_publishers VALUES(1,'cardinality','Cardinality',NULL)")
    db.execute("INSERT INTO catalogs VALUES(1,1,'cardinality','Cardinality')")
    db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,100,'cardinality-strict','zstd')", (b'a' * 32,))
    db.execute("INSERT INTO catalog_source_files VALUES(2,?,NULL,100,'cardinality-observed','zstd')", (b'b' * 32,))
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'cardinality-strict','mame','strict-dtd','0.289','constructed','strict')")
    db.execute("INSERT INTO catalog_reading_rules VALUES(2,'cardinality-observed','mame','observed-v3','0.289','constructed','mame-observed-compat-declared-text-v3')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")
    db.execute("INSERT INTO catalog_editions VALUES(2,1,2,2,1,NULL,NULL)")
    for group_id, edition_id in ((10, 1), (20, 2)):
        db.execute("INSERT INTO catalog_set_groups VALUES(?,?, 'root')", (group_id, edition_id))
        db.execute(
            "INSERT INTO mame_documents(edition_id,build,debug,debug_specified,mameconfig,source_line,source_column,location_view,start_line,start_column,end_line,end_column,column_convention) "
            "VALUES(?,NULL,0,0,'',1,1,'retained_original_text',1,1,2,1,'one_based_unicode_scalar')",
            (edition_id,),
        )
    for set_id, group_id, edition_id, name, set_order in (
        (101, 10, 1, 'strict-primary', 0),
        (111, 10, 1, 'strict-secondary', 1),
        (201, 20, 2, 'observed', 0),
    ):
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_machine')", (set_id, edition_id))
        db.execute("INSERT INTO catalog_sets VALUES(?,?,?,?,1,1)", (set_id, group_id, name, set_order))
        db.execute(
            "INSERT INTO mame_machines(set_id,sourcefile,isbios,isbios_specified,isdevice,isdevice_specified,ismechanical,ismechanical_specified,runnable,runnable_specified) "
            "VALUES(?,NULL,0,0,0,0,0,0,1,0)", (set_id,),
        )
        text_id = set_id + 1
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_machine_text')", (text_id, edition_id))
        db.execute("INSERT INTO mame_machine_text_elements VALUES(?,?,'description','',0,1,1)", (text_id, set_id))

    # Every represented strict DTD child family has a populated positive row;
    # empty PCDATA is deliberate and remains a valid value.
    db.execute("INSERT INTO catalog_source_elements VALUES(103,1,'mame_sound')")
    db.execute("INSERT INTO mame_sound VALUES(103,101,'2',1,1,1)")
    db.execute("INSERT INTO catalog_source_elements VALUES(203,2,'mame_sound')")
    db.execute("INSERT INTO mame_sound VALUES(203,201,'2',1,1,1)")
    for device_id, instance_id, extension_id, machine_id, edition_id in (
        (108, 109, 110, 101, 1), (208, 209, 210, 201, 2),
    ):
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_device')", (device_id, edition_id))
        db.execute("INSERT INTO mame_devices VALUES(?,?,'floppy',NULL,NULL,NULL,NULL,2,1,1)", (device_id, machine_id))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_device_instance')", (instance_id, edition_id))
        db.execute("INSERT INTO mame_device_instances VALUES(?,?, 'main','main',0,1,1)", (instance_id, device_id))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_device_extension')", (extension_id, edition_id))
        db.execute("INSERT INTO mame_device_extensions VALUES(?,?, 'img',1,1,1)", (extension_id, device_id))
    return db


def seed_strict_publication_fixture(db):
    """Load the complete field witness under strict rules for publication tests."""
    db.execute("INSERT INTO catalog_publishers VALUES(1,'strict-cardinality','Strict cardinality',NULL)")
    db.execute("INSERT INTO catalogs VALUES(1,1,'strict-cardinality','Strict cardinality')")
    db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,10000,'constructed','zstd')", (b'c' * 32,))
    db.execute("INSERT INTO catalog_decoded_xml_views VALUES(1,10000)")
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'strict-cardinality','mame','strict-dtd','0.289','constructed','strict-dtd')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")
    db.execute("INSERT INTO catalog_set_groups VALUES(10,1,'root')")

    source = (HERE / "mame_field_witnesses.sql").read_text()
    source = source.split("INSERT INTO mame_field_assertions", 1)[0]
    source, instance_count = re.subn(
        r"(INSERT INTO mame_device_instances .*?VALUES \(227,226,)1(,37,3,'',''\);)",
        r"\g<1>0\g<2>", source,
    )
    source, extension_count = re.subn(
        r"(INSERT INTO mame_device_extensions .*?VALUES \(228,226,)0(,38,3,''\);)",
        r"\g<1>1\g<2>", source,
    )
    if instance_count != 1 or extension_count != 1:
        raise AssertionError("strict publication witness no longer has the expected device child fixture")

    for line in source.splitlines():
        match = re.match(r"INSERT INTO (\w+)", line)
        if not match or match[1] in ("catalog_reading_rules", "catalog_editions", "catalog_set_groups"):
            continue
        if match[1] == "catalog_relationships":
            number = int(re.search(r"VALUES \((\d+),1\)", line)[1])
            db.execute(
                "INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES(?,?,'source',1)",
                (number, f"constructed-{number}"),
            )
        else:
            db.execute(line)
    db.commit()


class MameCardinality(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(assemble.assemble())
        names = {row[0] for row in cls.db.execute("SELECT name FROM sqlite_schema WHERE type='view'")}
        if VIEW not in names:
            raise AssertionError(f"assemble() did not install required family audit {VIEW}")
        columns = [row[1] for row in cls.db.execute(f'PRAGMA table_xinfo("{VIEW}")')]
        if columns != ["problem", "owner_id", "edition_id"]:
            raise AssertionError(f"{VIEW} has unexpected columns: {columns!r}")
        seed(cls.db)

    def setUp(self):
        self.db.execute("SAVEPOINT mame_cardinality_case")

    def tearDown(self):
        self.db.execute("ROLLBACK TO mame_cardinality_case")
        self.db.execute("RELEASE mame_cardinality_case")

    def findings(self):
        return self.db.execute(
            f'SELECT problem,owner_id,edition_id FROM "{VIEW}" ORDER BY problem,owner_id,edition_id'
        ).fetchall()

    def test_minimal_complete_roots_allow_empty_pcdata(self):
        self.assertEqual(self.findings(), [])

    def test_optional_empty_text_and_extra_or_malformed_children_follow_local_constraints(self):
        for source_id, field_kind, source_order in (
            (301, "year", 1),
            (302, "manufacturer", 2),
        ):
            self.db.execute(
                "INSERT INTO catalog_source_elements VALUES(?,1,'mame_machine_text')",
                (source_id,),
            )
            self.db.execute(
                "INSERT INTO mame_machine_text_elements VALUES(?,?,?,'',?,1,1)",
                (source_id, 111, field_kind, source_order),
            )
        self.assertEqual(self.findings(), [])

        self.db.execute("INSERT INTO catalog_source_elements VALUES(303,1,'mame_machine_text')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "INSERT INTO mame_machine_text_elements VALUES(303,111,'year','extra',3,1,1)"
            )
        self.db.execute("INSERT INTO catalog_source_elements VALUES(304,1,'mame_machine_text')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "INSERT INTO mame_machine_text_elements VALUES(304,111,'unexpected','bad',3,1,1)"
            )
        self.assertEqual(self.findings(), [])

    def test_required_description_deletion_is_attributed_to_its_machine_edition(self):
        self.db.execute("DELETE FROM mame_machine_text_elements WHERE source_element_id=102")
        self.assertIn(("machine_requires_description", 101, 1), self.findings())
        self.db.execute("INSERT INTO mame_machine_text_elements VALUES(102,101,'description','',0,1,1)")
        self.assertNotIn(("machine_requires_description", 101, 1), self.findings())

    def test_mame_root_rejects_zero_machines_and_repairs_after_machine_restoration(self):
        for table, predicate in (
            ("mame_device_extensions", "device_id=108"),
            ("mame_device_instances", "device_id=108"),
            ("mame_devices", "source_element_id=108"),
            ("mame_sound", "source_element_id=103"),
            ("mame_machine_text_elements", "machine_id=101"),
            ("mame_machines", "set_id=101"),
            ("mame_machine_text_elements", "machine_id=111"),
            ("mame_machines", "set_id=111"),
        ):
            self.db.execute(f"DELETE FROM {table} WHERE {predicate}")
        self.assertIn(("mame_root_requires_machine", 1, 1), self.findings())
        self.db.execute("INSERT INTO mame_machines VALUES(101,NULL,0,0,0,0,0,0,1,0)")
        self.db.execute("INSERT INTO mame_machine_text_elements VALUES(102,101,'description','',0,1,1)")
        self.assertNotIn(("mame_root_requires_machine", 1, 1), self.findings())

    def test_strict_machine_and_device_sequences_reject_then_repair(self):
        self.db.execute("UPDATE mame_machine_text_elements SET source_order=4 WHERE machine_id=101")
        self.db.execute("UPDATE mame_sound SET source_order=0 WHERE machine_id=101")
        self.db.execute("UPDATE mame_device_instances SET source_order=2 WHERE device_id=108")
        self.db.execute("UPDATE mame_device_extensions SET source_order=0 WHERE device_id=108")
        self.db.execute("UPDATE mame_device_instances SET source_order=1 WHERE device_id=108")
        problems = set(self.findings())
        self.assertIn(("strict_machine_child_sequence", 101, 1), problems)
        self.assertIn(("strict_device_child_sequence", 108, 1), problems)
        self.db.execute("UPDATE mame_sound SET source_order=3 WHERE machine_id=101")
        self.db.execute("UPDATE mame_machine_text_elements SET source_order=0 WHERE machine_id=101")
        self.db.execute("UPDATE mame_sound SET source_order=1 WHERE machine_id=101")
        self.db.execute("UPDATE mame_device_extensions SET source_order=2 WHERE device_id=108")
        self.db.execute("UPDATE mame_device_instances SET source_order=0 WHERE device_id=108")
        self.db.execute("UPDATE mame_device_extensions SET source_order=1 WHERE device_id=108")
        self.assertNotIn(("strict_machine_child_sequence", 101, 1), self.findings())
        self.assertNotIn(("strict_device_child_sequence", 108, 1), self.findings())

    def test_observed_compatibility_keeps_parser_accepted_order(self):
        self.db.execute("UPDATE mame_machine_text_elements SET source_order=4 WHERE machine_id=201")
        self.db.execute("UPDATE mame_sound SET source_order=0 WHERE machine_id=201")
        self.db.execute("UPDATE mame_device_instances SET source_order=2 WHERE device_id=208")
        self.db.execute("UPDATE mame_device_extensions SET source_order=0 WHERE device_id=208")
        self.db.execute("UPDATE mame_device_instances SET source_order=1 WHERE device_id=208")
        self.assertEqual(self.findings(), [])

    def test_switch_sequence_rule_is_strict_only_and_uses_typed_parent_order(self):
        for switch_id, machine_id, edition_id in ((120, 101, 1), (220, 201, 2)):
            self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'mame_switch')", (switch_id, edition_id))
            self.db.execute("INSERT INTO mame_switches VALUES(?,?, 'dipswitch','SW','tag','0f',3,1,1)", (switch_id, machine_id))
            value_id, location_id, condition_id = switch_id+3, switch_id+4, switch_id+5
            for source_id, kind in ((value_id,'mame_switch_value'), (location_id,'mame_switch_location'), (condition_id,'mame_switch_condition')):
                self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?,?)", (source_id, edition_id, kind))
            self.db.execute("INSERT INTO mame_switch_values VALUES(?,?, 'value','1',0,0,0,1,1)", (value_id,switch_id))
            self.db.execute("INSERT INTO mame_switch_locations VALUES(?,?, 'A','1',0,0,1,1,1)", (location_id,switch_id))
            self.db.execute("INSERT INTO mame_switch_conditions VALUES(?,?, 'tag','1','eq','1',2,1,1)", (condition_id,switch_id))
        self.assertIn(("strict_switch_child_sequence",120,1), self.findings())
        self.assertNotIn(("strict_switch_child_sequence",220,2), self.findings())
        self.db.execute("UPDATE mame_switch_conditions SET source_order=3 WHERE switch_id=120")
        self.db.execute("UPDATE mame_switch_values SET source_order=4 WHERE switch_id=120")
        self.db.execute("UPDATE mame_switch_conditions SET source_order=0 WHERE switch_id=120")
        self.db.execute("UPDATE mame_switch_values SET source_order=2 WHERE switch_id=120")
        self.assertNotIn(("strict_switch_child_sequence",120,1), self.findings())

    def test_publication_rejects_cardinality_problem_when_family_is_integrated(self):
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("PRAGMA foreign_keys=OFF")
            db.executescript(assemble.assemble())
            mame_presence_check.seed(db)
            baseline = db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
            ).fetchall()
            self.assertEqual(baseline, [])
            db.execute("DELETE FROM mame_machine_text_elements WHERE source_element_id=233")
            with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate publication requires complete closure"):
                db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            db.execute("INSERT INTO mame_machine_text_elements VALUES(233,100,'description','',0,3,3)")
            db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            self.assertEqual(db.execute("SELECT edition_id FROM published_catalog_editions").fetchall(), [(1,)])

    def test_strict_rom_size_joint_erasure_blocks_publication_then_repair_succeeds(self):
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("PRAGMA foreign_keys=OFF")
            db.executescript(assemble.assemble())
            seed_strict_publication_fixture(db)
            self.assertEqual(db.execute("SELECT * FROM candidate_integrity_problems").fetchall(), [])

            value = db.execute(
                "SELECT size_text FROM mame_roms WHERE media_entry_id=201"
            ).fetchone()
            position = db.execute(
                "SELECT media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column "
                "FROM mame_rom_claims_attribute_positions "
                "WHERE media_entry_id=201 AND field_kind='size'"
            ).fetchone()
            self.assertEqual(value, ("18446744073709551615",))
            self.assertEqual(position, (201, "size", 0, 2, 11, 34))

            for retained_text in ("", "not-a-decimal-size"):
                db.execute(
                    "UPDATE mame_roms SET size_text=? WHERE media_entry_id=201",
                    (retained_text,),
                )
                self.assertNotIn(
                    ("strict_rom_requires_size", 201, 1),
                    db.execute(
                        "SELECT problem,owner_id,edition_id FROM candidate_mame_cardinality_problems"
                    ).fetchall(),
                )
            db.execute("UPDATE mame_roms SET size_text=? WHERE media_entry_id=201", value)

            db.execute("UPDATE mame_roms SET size_text=NULL WHERE media_entry_id=201")
            db.execute(
                "DELETE FROM mame_rom_claims_attribute_positions "
                "WHERE media_entry_id=201 AND field_kind='size'"
            )
            # The ordinary value/position audit sees a consistent absence; the
            # strict DTD cardinality rule must still catch the jointly erased fact.
            self.assertNotIn(
                ("field_presence:mame_rom_claims_attribute_positions:size", 201, 1),
                db.execute("SELECT problem,owner_id,edition_id FROM candidate_integrity_problems").fetchall(),
            )
            self.assertEqual(
                db.execute("SELECT * FROM candidate_mame_cardinality_problems WHERE owner_id=201").fetchall(),
                [("strict_rom_requires_size", 201, 1)],
            )
            self.assertEqual(
                db.execute("SELECT * FROM candidate_integrity_problems").fetchall(),
                [("strict_rom_requires_size", 201, 1)],
            )
            with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate publication requires complete closure"):
                db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")

            db.execute("UPDATE mame_roms SET size_text=? WHERE media_entry_id=201", value)
            db.execute(
                "INSERT INTO mame_rom_claims_attribute_positions "
                "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
                "VALUES(?,?,?,?,?,?)",
                position,
            )
            self.assertEqual(db.execute("SELECT * FROM candidate_integrity_problems").fetchall(), [])
            db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            self.assertEqual(db.execute("SELECT edition_id FROM published_catalog_editions").fetchall(), [(1,)])

    def test_observed_rom_size_joint_absence_remains_publishable(self):
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("PRAGMA foreign_keys=OFF")
            db.executescript(assemble.assemble())
            mame_presence_check.seed(db)
            value = db.execute("SELECT size_text FROM mame_roms WHERE media_entry_id=201").fetchone()
            position = db.execute(
                "SELECT media_entry_id,field_kind,field_occurrence FROM mame_rom_claims_attribute_positions "
                "WHERE media_entry_id=201 AND field_kind='size'"
            ).fetchone()
            self.assertEqual(value, ("18446744073709551615",))
            self.assertEqual(position, (201, "size", 0))
            db.execute("UPDATE mame_roms SET size_text=NULL WHERE media_entry_id=201")
            db.execute(
                "DELETE FROM mame_rom_claims_attribute_positions "
                "WHERE media_entry_id=201 AND field_kind='size'"
            )
            self.assertEqual(db.execute("SELECT * FROM candidate_integrity_problems").fetchall(), [])
            db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            self.assertEqual(db.execute("SELECT edition_id FROM published_catalog_editions").fetchall(), [(1,)])


if __name__ == "__main__":
    unittest.main(verbosity=2)
