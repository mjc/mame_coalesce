"""Check native software child cardinalities against the composed candidate.

Run in the active repository devenv:
    python3 -Werror::ResourceWarning docs/schema-candidate/software_cardinality_check.py

The suite uses one assembled in-memory database and constructed native rows.
It checks relational closure; it does not prove parser or source-corpus counts.
"""
from contextlib import contextmanager
from pathlib import Path
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures
import software_presence_check

ROOT = Path(__file__).resolve().parent


class SoftwareCardinality(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        assembled = assemble.assemble()
        cls.db.executescript(assembled)
        cls.assert_view_interface()
        cls.db.executescript((ROOT / "software_field_witnesses.sql").read_text())
        count_fixtures.seal(
            cls.db, "software", 1, software_presence_check.SOFTWARE_FIELD_EVENTS
        )
        cls.add_envelope_controls()
        baseline = cls.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems"
        ).fetchall()
        if baseline:
            raise AssertionError(f"software cardinality baselines are not clean: {baseline}")
        cls.db.commit()
        cls.db.execute("PRAGMA foreign_keys=OFF")

    @classmethod
    def assert_view_interface(cls):
        columns = tuple(row[1] for row in cls.db.execute(
            "PRAGMA table_info(candidate_software_cardinality_problems)"))
        if columns != ("problem", "owner_id", "edition_id"):
            raise AssertionError(f"unexpected assembled cardinality view columns: {columns}")

    @classmethod
    def add_envelope_controls(cls):
        db = cls.db
        db.execute("INSERT INTO catalog_source_files VALUES(2,zeroblob(32),NULL,0,'cardinality-single','zstd')")
        db.execute("INSERT INTO catalog_source_files VALUES(3,zeroblob(32),NULL,1,'cardinality-empty-wrapper','zstd')")
        db.execute("INSERT INTO catalog_coverage VALUES(2,'unknown'),(3,'unknown')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,2,1,2,NULL,NULL),(3,1,3,1,3,NULL,NULL)")
        db.execute("""INSERT INTO software_documents
            (edition_id,envelope_kind,location_view,start_line,start_column,end_line,end_column,column_convention)
            VALUES (2,'single_list','retained_original_text',1,1,10,1,'one_based_unicode_scalar'),
                   (3,'plural_lists','retained_original_text',1,1,10,1,'one_based_unicode_scalar')""")
        db.execute("""INSERT INTO software_wrapper_headers
            (wrapper_id,edition_id,location_view,start_line,start_column,end_line,end_column,column_convention)
            VALUES (30,3,'retained_original_text',2,1,9,1,'one_based_unicode_scalar')""")
        db.execute("INSERT INTO catalog_set_groups VALUES(200,2,'software_list')")
        db.execute("""INSERT INTO software_lists
            (set_group_id,name,location_view,start_line,start_column,end_line,end_column,column_convention)
            VALUES (200,'minimal','retained_original_text',2,1,9,1,'one_based_unicode_scalar')""")
        db.execute("""INSERT INTO catalog_source_elements VALUES
            (2000,2,'software_item'),(2010,2,'software_title_text_element'),
            (2011,2,'software_title_text_element'),(2012,2,'software_title_text_element')""")
        db.execute("INSERT INTO catalog_sets VALUES(2000,200,'minimal',0,3,1)")
        db.execute("INSERT INTO software_titles VALUES(2000,'yes',0)")
        db.execute("""INSERT INTO software_title_text_elements VALUES
            (2010,2000,0,'',0,4,1),(2011,2000,1,'',1,5,1),(2012,2000,2,'',2,6,1)""")
        db.execute("INSERT INTO software_list_attribute_positions"
                   "(set_group_id,field_kind,source_order,source_line,source_column)"
                   " VALUES(200,0,0,2,15)")
        db.execute("INSERT INTO software_title_attribute_positions"
                   "(set_id,field_kind,source_order,source_line,source_column)"
                   " VALUES(2000,0,0,3,7)")
        count_fixtures.seal(db, "software", 2, {
            "list_count": 1,
            "title_count": 1,
            "title_text_element_count": 3,
            "list_attribute_position_count": 1,
            "title_attribute_position_count": 1,
        })
        count_fixtures.seal(db, "software", 3, {})

    def setUp(self):
        self.db.execute("SAVEPOINT software_cardinality_test")

    def tearDown(self):
        self.db.execute("ROLLBACK TO software_cardinality_test")
        self.db.execute("RELEASE software_cardinality_test")

    @contextmanager
    def case(self):
        self.db.execute("SAVEPOINT software_cardinality_case")
        try:
            yield
        finally:
            self.db.execute("ROLLBACK TO software_cardinality_case")
            self.db.execute("RELEASE software_cardinality_case")

    def drop_delete_guards(self, table):
        triggers = self.db.execute(
            "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=?", (table,)
        ).fetchall()
        for (name,) in triggers:
            self.db.execute('DROP TRIGGER "' + name.replace('"', '""') + '"')

    def problems(self, problem):
        return self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_software_cardinality_problems "
            "WHERE problem=? ORDER BY owner_id,edition_id", (problem,)
        ).fetchall()

    def test_minimal_complete_plural_fixture_and_empty_pcdata(self):
        # The independent native fixture has a plural envelope, empty required
        # title text, one empty list-notes child, and one absent title-notes child.
        self.assertEqual(self.problems("software_list_without_title"), [])
        self.assertEqual(self.problems("software_title_required_text_missing"), [])
        self.assertEqual(self.problems("software_area_subtype_closure"), [])
        self.assertEqual(self.db.execute(
            "SELECT envelope_kind FROM software_documents WHERE edition_id=1"
        ).fetchone(), ("plural_lists",))
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM software_title_text_elements "
            "WHERE set_id=100 AND field_kind IN (0,1,2) AND text_value=''"
        ).fetchone(), (3,))
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM software_title_text_elements WHERE set_id=101 AND field_kind=3"
        ).fetchone(), (0,))

    def test_bare_single_list_and_empty_plural_wrapper_compatibility(self):
        self.assertEqual(self.db.execute(
            "SELECT envelope_kind FROM software_documents WHERE edition_id IN (2,3) ORDER BY edition_id"
        ).fetchall(), [("single_list",), ("plural_lists",)])
        self.assertEqual(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_software_cardinality_problems "
            "WHERE edition_id IN (2,3) ORDER BY problem,owner_id"
        ).fetchall(), [])
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM software_list_wrapper_entries WHERE wrapper_id=30"
        ).fetchone(), (0,))
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM candidate_software_integrity_problems "
            "WHERE problem='software_envelope_mode_closure' AND edition_id IN (2,3)"
        ).fetchone(), (0,))

    def test_required_pcdata_erasure_is_detected_and_repair_clears_it(self):
        for field_kind in (0, 1, 2):
            with self.subTest(field_kind=field_kind), self.case():
                saved = self.db.execute(
                    "SELECT * FROM software_title_text_elements WHERE set_id=100 AND field_kind=?",
                    (field_kind,),
                ).fetchone()
                self.drop_delete_guards("software_title_text_elements")
                self.db.execute(
                    "DELETE FROM software_title_text_elements WHERE set_id=100 AND field_kind=?",
                    (field_kind,),
                )
                self.assertEqual(self.problems("software_title_required_text_missing"), [
                    ("software_title_required_text_missing", 100, 1)
                ])
                columns = [row[1] for row in self.db.execute(
                    "PRAGMA table_info(software_title_text_elements)"
                )]
                self.db.execute(
                    "INSERT INTO software_title_text_elements VALUES(" + ",".join("?" for _ in columns) + ")",
                    saved,
                )
                self.assertEqual(self.problems("software_title_required_text_missing"), [])

    def test_required_text_multiplicity_is_bounded_by_native_unique_key(self):
        # The table's UNIQUE(set_id, field_kind) is the at-most-one half of
        # exact cardinality; the view supplies the required-at-least-one half.
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "INSERT INTO software_title_text_elements VALUES(999,100,0,'extra',99,60,1)"
            )

    def test_list_without_typed_title_is_detected(self):
        with self.case():
            self.drop_delete_guards("software_titles")
            self.db.execute("DELETE FROM software_titles WHERE set_id=100")
            self.assertEqual(self.problems("software_list_without_title"), [
                ("software_list_without_title", 20, 1)
            ])

    def test_area_subtype_must_match_its_kind_and_uses_parent_edition(self):
        with self.case():
            self.db.execute("UPDATE software_areas SET kind='disk' WHERE area_id=300")
            self.assertEqual(self.problems("software_area_subtype_closure"), [
                ("software_area_subtype_closure", 300, 1)
            ])

    def test_detached_parent_falls_back_to_own_registry_then_unknown_scope(self):
        with self.case():
            self.db.execute("UPDATE software_areas SET kind='disk' WHERE area_id=300")
            self.db.execute("UPDATE catalog_source_elements SET edition_id=2 WHERE source_element_id=300")
            self.drop_delete_guards("software_parts")
            self.db.execute("DELETE FROM software_parts WHERE part_id=200")
            # The surviving parent registry is authoritative over the area's
            # deliberately different edition.
            self.assertEqual(self.problems("software_area_subtype_closure"), [
                ("software_area_subtype_closure", 300, 1)
            ])

            self.drop_delete_guards("catalog_source_elements")
            self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=200")
            # With no parent registry, use the area's own known edition.
            self.assertEqual(self.problems("software_area_subtype_closure"), [
                ("software_area_subtype_closure", 300, 2)
            ])

            self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=300")
            # Fully detached owners remain visible to the global audit with
            # NULL scope instead of borrowing an unrelated edition.
            self.assertEqual(self.problems("software_area_subtype_closure"), [
                ("software_area_subtype_closure", 300, None)
            ])
            self.assertEqual(self.db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems "
                "WHERE problem='software_area_subtype_closure' AND owner_id=300"
            ).fetchall(), [("software_area_subtype_closure", 300, None)])

    def test_known_parent_scope_uses_indexed_registry_and_facet_lookups(self):
        details = "\n".join(row[3] for row in self.db.execute(
            "EXPLAIN QUERY PLAN SELECT * FROM candidate_software_cardinality_problems "
            "WHERE problem='software_area_subtype_closure' AND edition_id=?", (1,)
        ))
        self.assertRegex(details, r"SEARCH parent_element USING .*PRIMARY KEY")
        self.assertRegex(details, r"SEARCH area_element USING .*PRIMARY KEY")
        self.assertRegex(details, r"SEARCH data USING .*PRIMARY KEY")
        self.assertNotRegex(details, r"\bSCAN (?:parent_element|area_element|data|disk)\b")

    def test_compatible_empty_switch_and_area_children_are_not_rejected(self):
        # The supported importer accepts empty dipswitch/dataarea/diskarea
        # children. In particular, the pinned DTD's dipvalue+ is not promoted
        # into a stricter native publication rule than importer compatibility.
        with self.case():
            self.drop_delete_guards("software_part_switch_values")
            self.db.execute("DELETE FROM software_part_switch_values WHERE switch_id=220")
            self.drop_delete_guards("software_rom_load_entries")
            self.drop_delete_guards("software_disks")
            self.db.execute("DELETE FROM software_rom_load_entries WHERE data_area_id=300")
            self.db.execute("DELETE FROM software_disks WHERE disk_area_id=302")
            self.assertEqual(self.problems("software_list_without_title"), [])
            self.assertEqual(self.problems("software_title_required_text_missing"), [])
            self.assertEqual(self.problems("software_area_subtype_closure"), [])

    def test_publication_gate_when_cardinality_view_is_assembled(self):
        with self.case():
            row = self.db.execute(
                "SELECT * FROM software_title_text_elements WHERE set_id=100 AND field_kind=0"
            ).fetchone()
            self.drop_delete_guards("software_title_text_elements")
            self.db.execute(
                "DELETE FROM software_title_text_elements WHERE set_id=100 AND field_kind=0"
            )
            integrated = self.db.execute(
                "SELECT 1 FROM candidate_integrity_problems "
                "WHERE problem='software_title_required_text_missing' AND owner_id=100 AND edition_id=1"
            ).fetchone()
            self.assertIsNotNone(integrated)
            with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate publication requires complete closure"):
                self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
            self.db.execute(
                "INSERT INTO software_title_text_elements VALUES(" + ",".join("?" for _ in row) + ")", row
            )
            self.assertEqual(self.db.execute(
                "SELECT count(*) FROM candidate_integrity_problems WHERE edition_id=1"
            ).fetchone(), (0,))
            self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")


if __name__ == "__main__":
    unittest.main()
