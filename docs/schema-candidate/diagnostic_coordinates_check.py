#!/usr/bin/env python3
"""Constructed coordinate regressions against the fully composed candidate DDL."""

import sqlite3
import unittest

import assemble


class DiagnosticCoordinates(unittest.TestCase):
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
            VALUES(1,'diagnostic-coordinates',10003,10300,10300,10300,'running','constructed')""")
        cls.db.execute('INSERT INTO catalog_decoded_xml_views VALUES(10300,10000)')
        cls.db.commit()

    def setUp(self):
        self.db = type(self).db
        self.db.execute('SAVEPOINT diagnostic_coordinates_test')

    def tearDown(self):
        self.db.execute('ROLLBACK TO diagnostic_coordinates_test')
        self.db.execute('RELEASE diagnostic_coordinates_test')

    def message(self, message_id, *, source_view='retained_original_bytes', excerpt_start=10,
                source_start=12, source_end=14, excerpt_start_at=2, excerpt_end_at=4,
                original_start=None, original_end=None):
        self.db.execute("""INSERT INTO catalog_import_messages
            (message_id,import_id,message_order,severity,code,message,edition_id,source_file_id,
             reading_rules_id,excerpt,source_view,excerpt_source_start,excerpt_problem_start,
             excerpt_problem_end,source_problem_start,source_problem_end,original_problem_start,
             original_problem_end)
            VALUES(?,1,?,'warning','constructed','Coordinate witness',10300,10300,10300,
                   ? ,?,?,?,?,?,?,?,?)""", (
            message_id, message_id - 1, b'0123456789', source_view, excerpt_start,
            excerpt_start_at, excerpt_end_at, source_start, source_end,
            source_start if original_start is None else original_start,
            source_end if original_end is None else original_end,
        ))

    def problems(self, message_id):
        return self.db.execute(
            'SELECT problem,owner_id,edition_id FROM candidate_message_integrity_problems WHERE owner_id=?',
            (message_id,),
        ).fetchall()

    def drop_message_guards(self):
        names = self.db.execute("""SELECT name FROM sqlite_schema
            WHERE type='trigger' AND tbl_name='catalog_import_messages'""").fetchall()
        for (name,) in names:
            self.db.execute(f'DROP TRIGGER {assemble.identifier(name)}')

    def test_highlight_corruption_is_auditable_for_both_source_views(self):
        self.drop_message_guards()
        for message_id, source_start, source_end, excerpt_start, highlight, source_view in (
            (1, 8, 12, 10, (0, 2), 'retained_original_bytes'),
            (2, 0, 100, 10, (0, 10), 'retained_original_bytes'),  # requested witness
            (3, 21, 22, 10, (10, 10), 'retained_original_bytes'), # beyond the excerpt
            (4, 0, 100, 10, (0, 10), 'transport_decoded_xml_bytes'),
        ):
            with self.subTest(span=(source_start, source_end)):
                self.message(message_id, excerpt_start=excerpt_start, source_start=source_start,
                             source_end=source_end,
                             source_view=source_view,
                             excerpt_start_at=highlight[0],
                             excerpt_end_at=highlight[1])
                self.assertEqual(self.problems(message_id), [
                    ('message_highlight_mapping', message_id, 10300),
                ])

    def test_invalid_highlight_insert_and_update_are_refused(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.message(1, source_start=0, source_end=100, excerpt_start=10,
                         excerpt_start_at=0, excerpt_end_at=10)

        self.message(2)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("""UPDATE catalog_import_messages
                SET source_problem_end=100,original_problem_end=100 WHERE message_id=2""")

    def test_contained_highlight_and_unmapped_out_of_excerpt_range_are_valid(self):
        self.message(1, source_start=12, source_end=15, excerpt_start=10,
                     excerpt_start_at=2, excerpt_end_at=5)
        self.assertEqual(self.problems(1), [])

        self.message(2, source_start=21, source_end=22, excerpt_start=10,
                     excerpt_start_at=None, excerpt_end_at=None)
        self.assertEqual(self.problems(2), [])

    def test_original_coordinate_corruption_is_auditable_and_equal_range_is_valid(self):
        self.drop_message_guards()
        self.message(1, source_view='retained_original_bytes', source_start=12,
                     source_end=14, original_start=40, original_end=41)
        self.assertEqual(self.problems(1), [
            ('message_coordinate_views', 1, 10300),
        ])

        self.message(2, source_view='retained_original_bytes', source_start=12,
                     source_end=14, original_start=12, original_end=14)
        self.assertEqual(self.problems(2), [])

    def test_invalid_original_coordinate_insert_and_update_are_refused(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.message(1, source_view='retained_original_bytes', source_start=12,
                         source_end=14, original_start=40, original_end=41)

        self.message(2, source_start=12, source_end=14)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("""UPDATE catalog_import_messages
                SET original_problem_start=40,original_problem_end=41 WHERE message_id=2""")

    def test_different_source_view_keeps_independent_original_coordinates(self):
        self.message(1, source_view='transport_decoded_xml_bytes', excerpt_start=10,
                     source_start=12, source_end=14, excerpt_start_at=2, excerpt_end_at=4,
                     original_start=40, original_end=41)
        self.assertEqual(self.problems(1), [])


if __name__ == '__main__':
    unittest.main(verbosity=2)
