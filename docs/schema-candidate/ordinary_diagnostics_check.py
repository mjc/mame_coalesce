#!/usr/bin/env python3
"""Constructed ordinary-owner diagnostic links, not authentic parser proof."""

import sqlite3
import unittest

import assemble
from export_query_check import add_unrelated_owners, vm_steps


# Independent literal current-owner inventory, not discovered from ROUTES.
OWNERS = (13001,13010,13020,13030,13031,13032,13033,13040,13041,13042,13043)


class OrdinaryDiagnosticLinks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())
        fixture = (assemble.ROOT / 'no_intro_field_witnesses.sql').read_text()
        cls.db.executescript(fixture.split('\n-- expect-error:', 1)[0])
        cls.db.execute('RELEASE no_intro_fields')
        cls.db.execute("""INSERT INTO catalog_imports
            (import_id,import_key,catalog_id,source_file_id,reading_rules_id,edition_id,status,started_at)
            VALUES(1,'ordinary-links',10003,10300,10300,10300,'running','constructed')""")
        cls.db.commit()
        cls.corrupt = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.corrupt.close)
        cls.db.backup(cls.corrupt)
        cls.corrupt.execute('PRAGMA foreign_keys=OFF')
        names = [row[0] for row in cls.corrupt.execute("""SELECT name FROM sqlite_schema
            WHERE type='trigger' AND tbl_name IN ('catalog_import_messages',
            'catalog_import_message_elements','catalog_source_elements',
            'no_intro_dump_details','no_intro_dump_sources')""")]
        cls.corrupt.execute('BEGIN')
        for name in names:
            cls.corrupt.execute(f'DROP TRIGGER {assemble.identifier(name)}')
        cls.corrupt.commit()

    def setUp(self):
        self.db = self.corrupt if self._testMethodName.startswith('test_corrupt_') else self.__class__.db
        self.db.execute('SAVEPOINT diagnostic_case')

    def tearDown(self):
        self.db.execute('ROLLBACK TO diagnostic_case')
        self.db.execute('RELEASE diagnostic_case')

    def message(self, line=10, column=2):
        self.db.execute("""INSERT INTO catalog_import_messages
            (message_id,import_id,message_order,severity,code,message,edition_id,
             source_file_id,reading_rules_id,line,column,location_view,column_convention)
            VALUES(1,1,0,'warning','constructed','Problem',10300,10300,10300,?,?,
                   'transport_decoded_xml_text','one_based_unicode_scalar')""", (line,column))

    def link(self, owner=13031):
        self.db.execute("INSERT INTO catalog_import_message_elements VALUES(1,?,10300,'primary')", (owner,))

    def test_outside_ordinary_owner_does_not_pass_just_because_fks_match(self):
        self.message(line=9)
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'ordinary diagnostic'):
            self.db.execute("INSERT INTO catalog_import_message_elements VALUES(1,13031,10300,'primary')")

    def test_game_end_coordinates_have_a_persistent_native_owner(self):
        self.assertEqual(self.db.execute('''SELECT source_end_line,source_end_column
            FROM no_intro_export_games WHERE set_id=13010''').fetchone(), (None,None))
        for line,column in ((None,1),(10,None),(0,1),(10,0),('bad',1)):
            with self.subTest(line=line,column=column), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute('UPDATE no_intro_export_games SET source_end_line=?,source_end_column=? WHERE set_id=13010', (line,column))

    def test_unlinked_game_interval_still_blocks_publication_then_repairs(self):
        self.db.execute('UPDATE no_intro_export_games SET source_end_line=10,source_end_column=1 WHERE set_id=13010')
        expected = [('ordinary_owner_interval:no_intro_export_games',13010,10300)]
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_owner_interval_problems').fetchall(),expected)
        self.assertEqual(self.db.execute("SELECT * FROM candidate_integrity_problems WHERE problem LIKE 'ordinary_owner_interval:%'").fetchall(),expected)
        with self.assertRaisesRegex(sqlite3.IntegrityError,'complete closure'):
            self.db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'candidate')")
        self.db.execute('UPDATE no_intro_export_games SET source_end_column=1000 WHERE set_id=13010')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_owner_interval_problems').fetchall(),[])
        self.db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'candidate')")

    def test_corrupt_unlinked_owner_interval_survives_missing_registry(self):
        self.db.execute('UPDATE no_intro_dump_sources SET source_end_line=9 WHERE dump_source_id=13030')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_owner_interval_problems').fetchall(),[('ordinary_owner_interval:no_intro_dump_sources',13030,10300)])
        self.db.execute('DELETE FROM catalog_source_elements WHERE source_element_id=13030')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_owner_interval_problems').fetchall(),[('ordinary_owner_interval:no_intro_dump_sources',13030,None)])

    def test_all_eleven_native_routes_preserve_actual_owner_and_interval(self):
        self.db.execute('UPDATE no_intro_export_games SET source_end_line=10,source_end_column=1000 WHERE set_id=13010')
        self.message()
        for owner in OWNERS:
            with self.subTest(owner=owner):
                self.link(owner)
                self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(), [])
                row = self.db.execute('''SELECT edition_id,start_line,start_column,end_line,end_column
                    FROM candidate_ordinary_message_owners WHERE source_element_id=?''', (owner,)).fetchone()
                self.assertEqual(row,(10300,10,1,10,1000))
                self.db.execute('DELETE FROM catalog_import_message_elements')

    def test_partial_wrong_view_and_exclusive_end_cannot_prove_a_child_link(self):
        for line,column,view in ((10,None,'transport_decoded_xml_text'),
                                 (None,2,'transport_decoded_xml_text'),
                                 (10,1000,'transport_decoded_xml_text'),
                                 (10,2,'retained_original_text')):
            with self.subTest(line=line,column=column,view=view):
                self.message(line,column)
                self.db.execute('UPDATE catalog_import_messages SET location_view=? WHERE message_id=1',(view,))
                with self.assertRaisesRegex(sqlite3.IntegrityError,'ordinary diagnostic'):
                    self.link()
                self.db.execute('DELETE FROM catalog_import_messages')

    def test_opening_only_other_format_owner_cannot_borrow_an_interval(self):
        self.message()
        for owner in (11030,12010):
            with self.subTest(owner=owner), self.assertRaises(sqlite3.IntegrityError):
                self.link(owner)

    def test_link_update_and_linked_draft_message_owner_rewrites_are_checked(self):
        self.message()
        self.link()
        for sql in ('UPDATE catalog_import_messages SET column=1000 WHERE message_id=1',
                    'UPDATE no_intro_dump_details SET source_end_column=2 WHERE details_element_id=13031',
                    "UPDATE catalog_source_elements SET element_kind='no_intro_export_release' WHERE source_element_id=13030",
                    "UPDATE catalog_set_groups SET group_kind='software_list' WHERE set_group_id=10310",
                    'UPDATE catalog_import_message_elements SET source_element_id=12010 WHERE message_id=1'):
            with self.subTest(sql=sql), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(sql)
        self.db.execute("UPDATE no_intro_dump_details SET comment1='still mutable draft' WHERE details_element_id=13031")
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[])

    def test_native_key_rewrite_cannot_detach_an_existing_link(self):
        self.message()
        self.link(13001)
        self.db.execute("INSERT INTO catalog_source_elements VALUES(19001,10300,'no_intro_export_header_field')")
        with self.assertRaisesRegex(sqlite3.IntegrityError,'owner identity is immutable'):
            self.db.execute('UPDATE no_intro_export_header_fields SET source_element_id=19001 WHERE source_element_id=13001')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[])

    def test_native_deletion_requires_explicit_link_removal(self):
        self.message()
        self.link()
        self.db.execute('DELETE FROM no_intro_dump_details_field_positions WHERE details_element_id=13031')
        with self.assertRaisesRegex(sqlite3.IntegrityError,'remove diagnostic links'):
            self.db.execute('DELETE FROM no_intro_dump_details WHERE details_element_id=13031')
        self.db.execute('DELETE FROM catalog_import_message_elements')
        self.db.execute('DELETE FROM no_intro_dump_details WHERE details_element_id=13031')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[])

    def test_game_byte_and_coordinate_proofs_must_agree_and_eof_stays_unlinked(self):
        self.db.execute("""UPDATE no_intro_export_games SET source_end_line=10,source_end_column=1000,
            extent_view='retained_original_bytes',extent_start=100,extent_end=200 WHERE set_id=13010""")
        self.message()
        for start,end,valid in ((110,120,True),(99,120,False),(200,200,False),(199,200,True)):
            with self.subTest(start=start,end=end):
                self.db.execute('UPDATE catalog_import_messages SET original_problem_start=?,original_problem_end=? WHERE message_id=1',(start,end))
                if valid:
                    self.link(13010)
                    self.db.execute('DELETE FROM catalog_import_message_elements')
                else:
                    with self.assertRaisesRegex(sqlite3.IntegrityError,'ordinary diagnostic'):
                        self.link(13010)
        self.db.execute('UPDATE catalog_import_messages SET original_problem_start=110,original_problem_end=120,column=1000 WHERE message_id=1')
        with self.assertRaisesRegex(sqlite3.IntegrityError,'ordinary diagnostic'):
            self.link(13010)

    def test_corrupt_link_is_globally_reported_blocks_publication_and_repairs(self):
        self.message(line=9)
        self.link()
        expected = [('ordinary_message_owner',1,10300)]
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),expected)
        self.assertEqual(self.db.execute("SELECT * FROM candidate_integrity_problems WHERE problem='ordinary_message_owner'").fetchall(),expected)
        with self.assertRaisesRegex(sqlite3.IntegrityError,'complete closure'):
            self.db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'candidate')")
        self.db.execute('UPDATE catalog_import_messages SET line=10 WHERE message_id=1')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[])
        self.db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'candidate')")

    def test_corrupt_intermediate_kind_or_missing_owner_never_disappears(self):
        self.message()
        self.link()
        self.db.execute("UPDATE catalog_source_elements SET element_kind='no_intro_export_release' WHERE source_element_id=13030")
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[('ordinary_message_owner',1,10300)])
        self.db.execute("UPDATE catalog_source_elements SET element_kind='no_intro_export_dump_source' WHERE source_element_id=13030")
        self.db.execute('DELETE FROM no_intro_dump_details WHERE details_element_id=13031')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_ordinary_message_problems').fetchall(),[('ordinary_message_owner',1,10300)])

    def test_requested_owner_projection_has_bounded_populated_work(self):
        query = 'SELECT * FROM candidate_ordinary_message_owners WHERE source_element_id=?'
        before = vm_steps(self.db,query,(13031,))
        # This test's savepoint owns all changes; don't commit through the helper.
        class NoCommit:
            def execute(_, *args):
                return self.db.execute(*args)
            def commit(_):
                pass
        add_unrelated_owners(NoCommit(),512)
        # Populate every branch, including singleton-per-parent details/serials.
        # ANALYZE legitimately chooses a scan for a one-row native table.
        for offset in range(512):
            edition = 20000 + offset
            for kind, table, key, parent_key, parent, order in (
                ('no_intro_export_archive','no_intro_archive_descriptions','archive_id','set_id',40000+offset,2),
                ('no_intro_export_dump_details','no_intro_dump_details','details_element_id','dump_source_id',60000+offset,1),
                ('no_intro_export_dump_serials','no_intro_dump_serials','serials_element_id','dump_source_id',60000+offset,2),
                ('no_intro_export_release_details','no_intro_release_details','details_element_id','release_id',80000+offset,1),
                ('no_intro_export_release_serials','no_intro_release_serials','serials_element_id','release_id',80000+offset,2),
            ):
                element = 100000 + offset * 8 + order + {'set_id':0,'dump_source_id':2,'release_id':4}[parent_key]
                self.db.execute('INSERT INTO catalog_source_elements VALUES(?,?,?)',(element,edition,kind))
                opening = ',opening_end_line,opening_end_column' if table.endswith('_details') else ''
                opening_values = ',1,2' if opening else ''
                self.db.execute(f'''INSERT INTO {table}({key},{parent_key},source_order,
                    source_line,source_column,source_end_line,source_end_column{opening})
                    VALUES(?,?,?,1,1,1,3{opening_values})''',(element,parent,order))
        self.db.execute('ANALYZE')
        after = vm_steps(self.db,query,(13031,))
        self.assertLessEqual(after,before+32,(before,after))
        plan = [row[3] for row in self.db.execute('EXPLAIN QUERY PLAN '+query,(13031,))]
        # SQLite may stream the compound view's bounded result coroutine; no
        # underlying native/ancestor table may be scanned for this point read.
        self.assertFalse([line for line in plan if 'SCAN ' in line.upper()
                          and line != 'SCAN candidate_ordinary_message_owners'],plan)
        self.assertTrue(any('SEARCH native USING INTEGER PRIMARY KEY' in line for line in plan),plan)
        print(f'ordinary requested-owner VM instructions: {before} -> {after} with 512 unrelated owners')


if __name__ == '__main__':
    unittest.main(verbosity=2)
