#!/usr/bin/env python3
"""Reading-policy issuance witnesses; no parser or publication-completeness proof."""

import sqlite3
import unittest

import assemble


POLICY_FACETS = (
    ('catalog_xml_repairs', (0, 1)),
    ('catalog_file_byte_contracts', ('mame_0289_machine_rom',)),
)


class ReadingPolicyIssuance(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())
        cls.db.execute("INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL)")
        cls.db.execute("INSERT INTO catalogs VALUES(1,1,'catalog','Catalog')")
        cls.db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,100,'object','zstd')",
                       (b's' * 32,))
        cls.db.executemany(
            "INSERT INTO catalog_reading_rules VALUES(?,?,'mame','observed','0.289','parser','rules')",
            ((number, f'rules-{number}') for number in (1, 2)))
        cls.db.execute("INSERT INTO catalog_coverage VALUES(1,'unknown')")
        cls.db.commit()

    def setUp(self):
        self.db.execute('SAVEPOINT reading_policy_test')

    def tearDown(self):
        self.db.execute('ROLLBACK TO reading_policy_test')
        self.db.execute('RELEASE reading_policy_test')

    def refuse_late_policy(self):
        for table, values in POLICY_FACETS:
            for value in values:
                self.db.execute('SAVEPOINT late_policy_probe')
                try:
                    with self.subTest(table=table, value=value), self.assertRaisesRegex(
                            sqlite3.IntegrityError, 'reading policy is sealed at first use'):
                        self.db.execute(f'INSERT INTO {table} VALUES(1,?)', (value,))
                finally:
                    self.db.execute('ROLLBACK TO late_policy_probe')
                    self.db.execute('RELEASE late_policy_probe')
            self.assertEqual(self.db.execute(f'SELECT * FROM {table}').fetchall(), [])

    def test_policy_can_be_selected_before_first_use(self):
        self.db.execute('INSERT INTO catalog_xml_repairs VALUES(1,1)')
        self.db.execute("INSERT INTO catalog_file_byte_contracts VALUES(1,'mame_0289_machine_rom')")
        self.db.execute('INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)')
        self.assertEqual(self.db.execute('SELECT * FROM catalog_xml_repairs').fetchall(), [(1, 1)])
        self.assertEqual(self.db.execute('SELECT * FROM catalog_file_byte_contracts').fetchall(),
                         [(1, 'mame_0289_machine_rom')])

    def test_explicit_no_repair_can_be_selected_before_first_use(self):
        self.db.execute('INSERT INTO catalog_xml_repairs VALUES(1,0)')
        self.db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,NULL,NULL,NULL,'running','start',NULL)")
        self.assertEqual(self.db.execute('SELECT * FROM catalog_xml_repairs').fetchall(), [(1, 0)])

    def test_draft_edition_seals_absent_policy(self):
        self.db.execute('INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)')
        self.refuse_late_policy()

    def test_running_source_only_attempt_seals_absent_policy(self):
        self.db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,NULL,NULL,NULL,'running','start',NULL)")
        self.refuse_late_policy()

    def test_failed_source_only_attempt_seals_absent_policy(self):
        self.db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,NULL,NULL,NULL,'failed','start','end')")
        self.refuse_late_policy()

    def test_changed_policy_requires_new_reading_identity(self):
        self.db.execute('INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)')
        self.db.execute('INSERT INTO catalog_xml_repairs VALUES(2,1)')
        self.db.execute("INSERT INTO catalog_file_byte_contracts VALUES(2,'mame_0289_machine_rom')")
        self.db.execute('INSERT INTO catalog_editions VALUES(2,1,1,2,1,NULL,NULL)')
        self.assertEqual(self.db.execute('SELECT * FROM catalog_xml_repairs').fetchall(), [(2, 1)])
        self.assertEqual(self.db.execute('SELECT * FROM catalog_file_byte_contracts').fetchall(),
                         [(2, 'mame_0289_machine_rom')])

    def test_existing_policy_cannot_be_rewritten_deleted_or_replaced(self):
        self.db.execute('INSERT INTO catalog_xml_repairs VALUES(1,1)')
        for statement in (
                'UPDATE catalog_xml_repairs SET replace_nul=0 WHERE reading_rules_id=1',
                'DELETE FROM catalog_xml_repairs WHERE reading_rules_id=1',
                'INSERT OR REPLACE INTO catalog_xml_repairs VALUES(1,0)'):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(statement)
        self.assertEqual(self.db.execute('SELECT * FROM catalog_xml_repairs').fetchall(), [(1, 1)])

    def test_used_rules_lookups_use_reverse_indexes(self):
        for table in ('catalog_editions', 'catalog_imports'):
            plan = ' '.join(row[3] for row in self.db.execute(
                f'EXPLAIN QUERY PLAN SELECT 1 FROM {table} WHERE reading_rules_id=?', (1,)))
            self.assertIn(f'SEARCH {table} USING COVERING INDEX {table}_by_rules', plan)


if __name__ == '__main__':
    unittest.main()
