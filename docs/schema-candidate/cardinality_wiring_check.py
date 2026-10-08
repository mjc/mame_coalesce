#!/usr/bin/env python3
"""Fail-closed audit discovery; native suites test the actual format rules."""

from contextlib import closing
import sqlite3
import unittest

import assemble


class CardinalityWiring(unittest.TestCase):
    def connection(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        for family in assemble.FAMILIES:
            db.execute(f'''CREATE VIEW candidate_{family}_cardinality_problems
                AS SELECT 'missing child' AS problem,1 AS owner_id,7 AS edition_id''')
        return db

    def test_all_families_are_publication_inputs_with_exact_identity(self):
        db = self.connection()
        queries = assemble.format_audit_queries(db)
        self.assertEqual(len(queries), 4)
        self.assertEqual({query.split('FROM ')[1] for query in queries}, {
            f'"candidate_{family}_cardinality_problems"' for family in assemble.FAMILIES})
        for query in queries:
            self.assertEqual(db.execute(query).fetchall(), [('missing child', 1, 7)])

    def test_each_missing_or_misspelled_family_fails_assembly(self):
        for family in assemble.FAMILIES:
            with self.subTest(family=family), closing(sqlite3.connect(':memory:')) as db:
                for other in assemble.FAMILIES:
                    suffix = 'cardinality_problem' if family == other else 'cardinality_problems'
                    db.execute(f'''CREATE VIEW candidate_{other}_{suffix}
                        AS SELECT '' AS problem,1 AS owner_id,7 AS edition_id''')
                with self.assertRaisesRegex(ValueError, f'missing.*{family}'):
                    assemble.format_audit_queries(db)

    def test_wrong_missing_or_extra_columns_do_not_pass_selection(self):
        for projection in ('1 AS owner_id,7 AS edition_id',
                           "'' AS problem,7 AS edition_id,1 AS owner_id",
                           "'' AS problem,1 AS owner_id,7 AS edition_id,0 AS extra"):
            with self.subTest(projection=projection):
                db = self.connection()
                db.execute('DROP VIEW candidate_mame_cardinality_problems')
                db.execute('CREATE VIEW candidate_mame_cardinality_problems AS SELECT ' + projection)
                with self.assertRaisesRegex(ValueError, 'audit columns'):
                    assemble.format_audit_queries(db)

    def test_unresolved_native_dependencies_fail_before_emitting_sql(self):
        db = self.connection()
        db.execute('DROP VIEW candidate_software_cardinality_problems')
        db.execute('''CREATE VIEW candidate_software_cardinality_problems AS
            SELECT problem,owner_id,edition_id FROM absent_native_table''')
        with self.assertRaisesRegex(sqlite3.OperationalError, 'absent_native_table'):
            assemble.format_audit_queries(db)

    def test_existing_integrity_and_cycle_audits_remain_included_once(self):
        db = self.connection()
        for name in ('candidate_shared_integrity_problems', 'candidate_edition_cycle_problems'):
            db.execute(f'''CREATE VIEW {name} AS
                SELECT 'original' AS problem,2 AS owner_id,8 AS edition_id''')
        queries = assemble.format_audit_queries(db)
        self.assertEqual(len(queries), 6)
        self.assertEqual(len(set(queries)), 6)
        self.assertEqual(sum(db.execute(query).fetchone()[2] == 8 for query in queries), 2)


if __name__ == '__main__':
    unittest.main()
