#!/usr/bin/env python3
"""Source-view bounds independent of diagnostics, using constructed facts."""

import sqlite3
import re
import unittest

import assemble
import count_fixtures
from software_presence_check import SOFTWARE_FIELD_EVENTS


class PhysicalExtents(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.executescript(assemble.assemble())
        cls.db.executescript((assemble.ROOT / 'software_field_witnesses.sql').read_text())
        count_fixtures.seal(cls.db,'software',1,SOFTWARE_FIELD_EVENTS)
        cls.db.commit()

    def setUp(self):
        self.db.execute('SAVEPOINT extent_case')

    def tearDown(self):
        self.db.execute('ROLLBACK TO extent_case')
        self.db.execute('RELEASE extent_case')

    def test_oversized_roots_cannot_publish_without_any_diagnostics(self):
        for table,key,value in (('software_documents','edition_id',1),
                                ('software_wrapper_headers','wrapper_id',10),
                                ('software_lists','set_group_id',20)):
            with self.subTest(table=table):
                self.db.execute('SAVEPOINT owner_case')
                try:
                    self.db.execute(f'''UPDATE {table} SET extent_view='retained_original_bytes',
                        extent_start=0,extent_end=1001 WHERE {key}=?''',(value,))
                    with self.assertRaisesRegex(sqlite3.IntegrityError,'complete closure'):
                        self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
                finally:
                    self.db.execute('ROLLBACK TO owner_case')
                    self.db.execute('RELEASE owner_case')

    def test_every_native_extent_table_is_in_the_independent_audit(self):
        expected = {'mame_documents','logiqx_documents','clrmamepro_documents',
                    'software_documents','software_wrapper_headers','software_lists',
                    'no_intro_dat_documents','no_intro_pc_documents',
                    'no_intro_export_documents','no_intro_export_datafiles',
                    'no_intro_export_headers','no_intro_export_games'}
        sql = self.db.execute("SELECT sql FROM sqlite_schema WHERE name='candidate_physical_extent_problems'").fetchone()[0]
        self.assertEqual(set(re.findall(r'FROM "([a-z_]+)" AS owner',sql)),expected)

    def test_original_and_decoded_lengths_are_not_interchangeable(self):
        self.db.execute('INSERT INTO catalog_decoded_xml_views VALUES(1,1200)')
        for view,end,valid in (('retained_original_bytes',1000,True),
                               ('retained_original_bytes',1001,False),
                               ('transport_decoded_xml_bytes',1200,True),
                               ('transport_decoded_xml_bytes',1201,False)):
            with self.subTest(view=view,end=end):
                self.db.execute('UPDATE software_documents SET extent_view=?,extent_start=0,extent_end=? WHERE edition_id=1',(view,end))
                problems = self.db.execute('SELECT * FROM candidate_physical_extent_problems').fetchall()
                self.assertEqual(problems,[] if valid else [('physical_extent:software_documents',1,1)])

    def test_unavailable_is_valid_but_unknown_decoded_length_is_not_proof(self):
        self.assertEqual(self.db.execute('SELECT * FROM candidate_physical_extent_problems').fetchall(),[])
        self.db.execute("UPDATE software_documents SET extent_view='transport_decoded_xml_bytes',extent_start=0,extent_end=1 WHERE edition_id=1")
        self.assertEqual(self.db.execute('SELECT * FROM candidate_physical_extent_problems').fetchall(),[('physical_extent:software_documents',1,1)])

    def test_bypassed_local_checks_do_not_hide_partial_or_reversed_ranges(self):
        self.db.execute('PRAGMA ignore_check_constraints=ON')
        try:
            for start,end in ((None,10),(0,None),(10,10),(10,9),(-1,10)):
                with self.subTest(start=start,end=end):
                    self.db.execute("UPDATE software_documents SET extent_view='retained_original_bytes',extent_start=?,extent_end=? WHERE edition_id=1",(start,end))
                    self.assertEqual(self.db.execute('SELECT * FROM candidate_physical_extent_problems').fetchall(),[('physical_extent:software_documents',1,1)])
        finally:
            self.db.execute('PRAGMA ignore_check_constraints=OFF')

    def test_valid_bounds_publish_and_freeze_without_diagnostic_links(self):
        self.db.execute("UPDATE software_documents SET extent_view='retained_original_bytes',extent_start=0,extent_end=1000 WHERE edition_id=1")
        self.assertEqual(self.db.execute('SELECT * FROM candidate_integrity_problems').fetchall(),[])
        self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'constructed')")
        with self.assertRaisesRegex(sqlite3.IntegrityError,'immutable'):
            self.db.execute('UPDATE software_documents SET extent_end=999 WHERE edition_id=1')


if __name__ == '__main__':
    unittest.main(verbosity=2)
