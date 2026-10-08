#!/usr/bin/env python3
"""Independent constructed count-seal mutations, not a parser EOF proof."""

import sqlite3
import unittest

import assemble


# Explicit contract from DOC-24, not discovered from the SELECT being tested.
EDITION_COUNTS = (
    ('game_count', 1), ('archive_count', 2), ('dump_source_count', 1),
    ('dump_details_count', 1), ('dump_serials_count', 1), ('dump_file_count', 1),
    ('release_count', 1), ('release_details_count', 1), ('release_serials_count', 1),
    ('release_file_count', 1), ('header_field_count', 5),
    ('archive_position_count', 33), ('dump_details_position_count', 20),
    ('dump_serials_position_count', 14), ('dump_file_position_count', 23),
    ('release_details_position_count', 17), ('release_serials_position_count', 6),
    ('release_file_position_count', 17),
)
LOCAL_COUNTS = (
    ('no_intro_export_game_witnesses', 'set_id', 13010,
     'export_game_witness_missing_or_mismatch',
     (('archive_count', 2), ('dump_source_count', 1), ('release_count', 1))),
    ('no_intro_dump_source_witnesses', 'dump_source_id', 13030,
     'export_dump_source_witness_missing_or_mismatch',
     (('details_count', 1), ('serials_count', 1), ('file_count', 1))),
    ('no_intro_release_witnesses', 'release_id', 13040,
     'export_release_witness_missing_or_mismatch',
     (('details_count', 1), ('serials_count', 1), ('file_count', 1))),
)
EDITION_PROBLEM = 'export_count_witness_missing_or_mismatch'
# Independently specified effects of removing a native relation from the
# constructed fixture. In particular, details/serial/file totals start equal;
# perturbing their expected counters alone cannot expose swapped query bodies.
NATIVE_COUNT_REMOVALS = (
    ('no_intro_export_games', ('game_count', 'archive_count')),
    ('no_intro_archive_descriptions', ('archive_count', 'archive_position_count')),
    ('no_intro_dump_sources', ('dump_source_count', 'dump_details_count',
                             'dump_serials_count', 'dump_file_count',
                             'dump_details_position_count', 'dump_serials_position_count',
                             'dump_file_position_count')),
    ('no_intro_dump_details', ('dump_details_count', 'dump_details_position_count')),
    ('no_intro_dump_serials', ('dump_serials_count', 'dump_serials_position_count')),
    ('no_intro_dump_files', ('dump_file_count', 'dump_file_position_count')),
    ('no_intro_releases', ('release_count', 'release_details_count',
                          'release_serials_count', 'release_file_count',
                          'release_details_position_count', 'release_serials_position_count',
                          'release_file_position_count')),
    ('no_intro_release_details', ('release_details_count', 'release_details_position_count')),
    ('no_intro_release_serials', ('release_serials_count', 'release_serials_position_count')),
    ('no_intro_release_files', ('release_file_count', 'release_file_position_count')),
    ('no_intro_export_header_fields', ('header_field_count',)),
    ('no_intro_archive_field_positions', ('archive_position_count',)),
    ('no_intro_dump_details_field_positions', ('dump_details_position_count',)),
    ('no_intro_dump_serials_field_positions', ('dump_serials_position_count',)),
    ('no_intro_dump_file_field_positions', ('dump_file_position_count',)),
    ('no_intro_release_details_field_positions', ('release_details_position_count',)),
    ('no_intro_release_serials_field_positions', ('release_serials_position_count',)),
    ('no_intro_release_file_field_positions', ('release_file_position_count',)),
)


class ExportCountSeals(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        # Generated FK/reverse guards stay on. OFF permits only the deliberately
        # guard-bypassed native-row deletion below, inside a rolled-back savepoint.
        cls.db.execute('PRAGMA foreign_keys=OFF')
        cls.db.executescript(assemble.assemble())
        fixture = (assemble.ROOT / 'no_intro_field_witnesses.sql').read_text()
        cls.db.executescript(fixture.split('\n-- expect-error:', 1)[0])
        cls.db.execute('RELEASE no_intro_fields')

    def setUp(self):
        self.db.execute('SAVEPOINT count_witness')

    def tearDown(self):
        self.db.execute('ROLLBACK TO count_witness')
        self.db.execute('RELEASE count_witness')

    def publish(self):
        self.db.execute('''INSERT INTO published_catalog_editions
            VALUES(10300,10003,10300,10300,10000,'count-witness')''')

    def require_problem(self, problem, owner):
        self.assertEqual(self.db.execute('''SELECT owner_id,edition_id
            FROM candidate_no_intro_integrity_problems WHERE problem=?''',
                                        (problem,)).fetchall(), [(owner, 10300)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
            self.publish()
        self.assertEqual(self.db.execute('SELECT count(*) FROM published_catalog_editions').fetchone(), (0,))

    def bypass_delete_guards(self, table):
        for name, sql in self.db.execute('''SELECT name,sql FROM sqlite_schema
            WHERE type='trigger' AND tbl_name=?''', (table,)).fetchall():
            if 'BEFORE DELETE' in sql.upper():
                self.db.execute(f'DROP TRIGGER {assemble.identifier(name)}')

    def test_literal_contract_matches_populated_source_fixture_and_can_publish(self):
        columns = ','.join(column for column, _ in EDITION_COUNTS)
        self.assertEqual(self.db.execute(f'''SELECT {columns}
            FROM no_intro_export_parse_witnesses WHERE edition_id=10300''').fetchone(),
                         tuple(value for _, value in EDITION_COUNTS))
        for table, key, owner, _, counts in LOCAL_COUNTS:
            self.assertEqual(self.db.execute(
                f'SELECT {",".join(column for column, _ in counts)} FROM {table} WHERE {key}=?',
                (owner,)).fetchone(), tuple(value for _, value in counts))
        self.assertEqual(self.db.execute('''SELECT * FROM candidate_integrity_problems
            WHERE edition_id=10300''').fetchall(), [])
        self.publish()

    def test_details_opening_endpoint_is_independent_and_frozen(self):
        for table, owner in (('no_intro_dump_details', 13031),
                             ('no_intro_release_details', 13041)):
            with self.subTest(table=table):
                self.assertEqual(self.db.execute(f'''SELECT source_line,source_column,
                    opening_end_line,opening_end_column,source_end_line,source_end_column
                    FROM {table} WHERE details_element_id=?''', (owner,)).fetchone(),
                    (10, 1, 10, 2, 10, 1000))
        self.publish()
        for table, owner in (('no_intro_dump_details', 13031),
                             ('no_intro_release_details', 13041)):
            with self.subTest(table=table), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(f'UPDATE {table} SET opening_end_column=3 WHERE details_element_id=?',
                                (owner,))

    def test_details_opening_endpoint_has_checked_coordinate_bounds(self):
        for table, owner in (('no_intro_dump_details', 13031),
                             ('no_intro_release_details', 13041)):
            for line, column in ((None, 2), (10, None), (0, 2), (10, 0),
                                 ('bad', 2), (10, 1), (9, 999), (10, 1001), (11, 1)):
                with self.subTest(table=table, line=line, column=column):
                    with self.assertRaises(sqlite3.IntegrityError):
                        self.db.execute(f'''UPDATE {table} SET opening_end_line=?,
                            opening_end_column=? WHERE details_element_id=?''', (line, column, owner))
            # A self-closing element ends at its opening-tag endpoint.
            self.db.execute(f'''UPDATE {table} SET opening_end_line=10,
                opening_end_column=1000 WHERE details_element_id=?''', (owner,))
            self.db.execute(f'''UPDATE {table} SET source_end_line=12,
                opening_end_line=11,opening_end_column=2 WHERE details_element_id=?''', (owner,))
            self.assertEqual(self.db.execute(f'''SELECT opening_end_line,opening_end_column
                FROM {table} WHERE details_element_id=?''', (owner,)).fetchone(), (11, 2))

    def test_every_edition_counter_detects_independent_under_and_overcount(self):
        for column, expected in EDITION_COUNTS:
            for delta in (-1, 1):
                with self.subTest(column=column, delta=delta):
                    self.db.execute(f'''UPDATE no_intro_export_parse_witnesses
                        SET {column}=? WHERE edition_id=10300''', (expected + delta,))
                    self.require_problem(EDITION_PROBLEM, 10300)
                    self.db.execute(f'''UPDATE no_intro_export_parse_witnesses
                        SET {column}=? WHERE edition_id=10300''', (expected,))
                    self.assertEqual(self.db.execute('''SELECT * FROM candidate_no_intro_integrity_problems
                        WHERE edition_id=10300''').fetchall(), [])

    def test_every_parent_local_counter_is_independent_of_the_edition_seal(self):
        for table, key, owner, problem, counts in LOCAL_COUNTS:
            for column, expected in counts:
                with self.subTest(table=table, column=column):
                    self.db.execute(f'UPDATE {table} SET {column}=? WHERE {key}=?',
                                    (expected - 1, owner))
                    self.require_problem(problem, owner)
                    self.assertEqual(self.db.execute('''SELECT * FROM candidate_no_intro_integrity_problems
                        WHERE problem=?''', (EDITION_PROBLEM,)).fetchall(), [])
                    self.db.execute(f'UPDATE {table} SET {column}=? WHERE {key}=?',
                                    (expected, owner))

    def test_each_counter_reads_its_own_native_relation_not_an_equal_sized_neighbor(self):
        for table, expected_zero in NATIVE_COUNT_REMOVALS:
            with self.subTest(table=table):
                self.db.execute('SAVEPOINT count_mapping')
                try:
                    self.bypass_delete_guards(table)
                    self.db.execute(f'DELETE FROM {table}')
                    self.require_problem(EDITION_PROBLEM, 10300)
                    # This is an intentionally corrupt graph: generated owner
                    # and FK audits can still report dangling descendants. Test
                    # only the edition count mapping against the independent
                    # literal effects above; do not publish this damaged graph.
                    assignments = ','.join(f'{column}=0' for column in expected_zero)
                    self.db.execute(f'''UPDATE no_intro_export_parse_witnesses
                        SET {assignments} WHERE edition_id=10300''')
                    self.assertEqual(self.db.execute('''SELECT *
                        FROM candidate_no_intro_integrity_problems
                        WHERE problem=?''', (EDITION_PROBLEM,)).fetchall(), [])
                finally:
                    self.db.execute('ROLLBACK TO count_mapping')
                    self.db.execute('RELEASE count_mapping')

    def test_missing_edition_and_local_witnesses_cannot_publish(self):
        rows = (('no_intro_export_parse_witnesses', 'edition_id', 10300, EDITION_PROBLEM),
                *((table, key, owner, problem) for table, key, owner, problem, _ in LOCAL_COUNTS))
        for table, key, owner, problem in rows:
            with self.subTest(table=table):
                self.db.execute('SAVEPOINT missing_seal')
                try:
                    self.db.execute(f'DELETE FROM {table} WHERE {key}=?', (owner,))
                    self.require_problem(problem, owner)
                finally:
                    self.db.execute('ROLLBACK TO missing_seal')
                    self.db.execute('RELEASE missing_seal')

    def test_seals_require_nonnegative_exact_integers(self):
        tables = (('no_intro_export_parse_witnesses', 'edition_id', 10300, EDITION_COUNTS),
                  *((table, key, owner, counts) for table, key, owner, _, counts in LOCAL_COUNTS))
        for table, key, owner, counts in tables:
            for column, _ in counts:
                for invalid in (-1, None, 0.5, 'not-an-integer'):
                    with self.subTest(table=table, column=column, invalid=invalid), self.assertRaises(sqlite3.IntegrityError):
                        self.db.execute(f'UPDATE {table} SET {column}=? WHERE {key}=?', (invalid, owner))

    def test_whole_optional_child_erasure_does_not_erase_the_independent_seal(self):
        # Remove both payload and registry to leave no native owner-count error.
        # No expected count is recalculated from the now-damaged stored rows.
        owner = self.db.execute('''SELECT source_element_id FROM no_intro_export_header_fields
            ORDER BY source_element_id LIMIT 1''').fetchone()[0]
        for table in ('no_intro_export_header_fields', 'catalog_source_elements'):
            self.bypass_delete_guards(table)
            self.db.execute(f'DELETE FROM {table} WHERE source_element_id=?', (owner,))
        self.require_problem(EDITION_PROBLEM, 10300)
        self.assertEqual(self.db.execute('''SELECT * FROM candidate_integrity_problems
            WHERE problem='native_owner_count' AND owner_id=?''', (owner,)).fetchall(), [])
        self.assertEqual(self.db.execute('''SELECT header_field_count
            FROM no_intro_export_parse_witnesses WHERE edition_id=10300''').fetchone(), (5,))

    def test_repaired_seals_publish_and_then_are_immutable(self):
        self.db.execute('UPDATE no_intro_export_parse_witnesses SET game_count=0 WHERE edition_id=10300')
        self.require_problem(EDITION_PROBLEM, 10300)
        self.db.execute('UPDATE no_intro_export_parse_witnesses SET game_count=1 WHERE edition_id=10300')
        self.publish()
        rows = (('no_intro_export_parse_witnesses', 'edition_id', 10300, 'game_count'),
                *((table, key, owner, counts[0][0]) for table, key, owner, _, counts in LOCAL_COUNTS))
        for table, key, owner, column in rows:
            for statement in (f'UPDATE {table} SET {column}=0 WHERE {key}=?',
                              f'DELETE FROM {table} WHERE {key}=?'):
                with self.subTest(statement=statement), self.assertRaisesRegex(sqlite3.IntegrityError, 'immutable'):
                    self.db.execute(statement, (owner,))


if __name__ == '__main__':
    unittest.main()
