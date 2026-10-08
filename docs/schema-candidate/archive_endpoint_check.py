#!/usr/bin/env python3
"""Constructed native archive endpoint checks against the composed candidate."""

import sqlite3
import unittest

import assemble
import export_query_check
import relationship_closure
import relationship_closure_check as relationships


class ArchiveEndpoints(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())
        fixture = (assemble.ROOT / 'no_intro_field_witnesses.sql').read_text()
        cls.db.executescript(fixture.split('\n-- expect-error:',1)[0])
        cls.db.execute('RELEASE no_intro_fields')
        # Two native records deliberately repeat the publisher's number.
        # Add the independently known extra position to the constructed witness.
        cls.db.execute("UPDATE no_intro_archive_descriptions SET number='0001' WHERE archive_id IN (13020,13021)")
        cls.db.execute("INSERT INTO no_intro_archive_field_positions"
                       "(archive_id,field_kind,field_occurrence,source_order,source_line,source_column) "
                       "VALUES(13021,'number',0,1,10,5)")
        cls.db.execute('UPDATE no_intro_export_parse_witnesses SET archive_position_count=34 WHERE edition_id=10300')
        cls.db.commit()
        cls.baseline = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.baseline.close)
        cls.db.backup(cls.baseline)
        # Prove full native publication once; reuse that immutable fixture rather
        # than rerunning the global readiness audit for every endpoint attack.
        cls.db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'constructed')")
        cls.db.commit()
        cls.published_baseline = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.published_baseline.close)
        cls.db.backup(cls.published_baseline)

    def setUp(self):
        self.db.execute('SAVEPOINT archive_endpoint_case')

    def tearDown(self):
        self.db.execute('ROLLBACK TO archive_endpoint_case')
        self.db.execute('RELEASE archive_endpoint_case')

    def endpoint(self, target_id=1, archive_id=13020, db=None):
        db = self.db if db is None else db
        db.execute("INSERT INTO catalog_relationship_targets VALUES(?,'no_intro_archive')", (target_id,))
        db.execute('INSERT INTO no_intro_archive_targets(target_id,archive_id) VALUES(?,?)', (target_id,archive_id))

    def publish(self, db=None):
        db = self.db if db is None else db
        if not db.execute('SELECT 1 FROM published_catalog_editions WHERE edition_id=10300').fetchone():
            db.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'constructed')")

    def claim(self, origin='user', db=None):
        db = self.db if db is None else db
        self.endpoint(1,13020,db)
        self.endpoint(2,13021,db)
        relationships.identity(db,origin)
        if origin == 'user':
            db.execute("INSERT INTO manual_catalog_relationships VALUES(1,'revision_of',1,2)")
        else:
            db.execute("INSERT INTO catalog_relationship_rules VALUES(1,'archive-rule','1','constructed rule')")
            relationships.derived(db)
        relationships.rationale(db)

    def seal(self, db=None):
        db = self.db if db is None else db
        db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")

    def test_archive_endpoint_retains_the_actual_native_owner(self):
        self.endpoint()
        self.assertEqual(self.db.execute('SELECT archive_id FROM no_intro_archive_targets WHERE target_id=1').fetchone(),(13020,))

    def test_endpoint_can_be_created_after_its_native_edition_is_published(self):
        self.publish()
        self.endpoint()

    def test_set_endpoint_can_also_be_created_after_publication(self):
        self.publish()
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(1,'catalog_set')")
        self.db.execute('INSERT INTO catalog_set_targets(target_id,set_id) VALUES(1,13010)')

    def test_repeated_publisher_numbers_do_not_merge_distinct_archives(self):
        self.publish()
        self.claim()
        self.seal()
        self.assertEqual(self.db.execute(
            'SELECT target_id,archive_id,number,source_order FROM no_intro_archive_targets '
            'JOIN no_intro_archive_descriptions USING(archive_id) ORDER BY target_id').fetchall(),
            [(1,13020,'0001',0),(2,13021,'0001',1)])
        self.assertEqual(self.db.execute('SELECT * FROM candidate_relationship_closure_problems').fetchall(),[])

    def test_both_origins_can_seal_and_publish_an_accepted_review(self):
        for origin in ('user','derived'):
            with self.subTest(origin=origin):
                self.db.execute('SAVEPOINT origin_case')
                self.publish()
                self.claim(origin)
                self.seal()
                review_id = relationships.review(self.db)
                self.db.execute("INSERT INTO catalog_relationship_review_publications VALUES(?,'constructed')",(review_id,))
                self.assertEqual(self.db.execute('SELECT relationship_id FROM active_catalog_relationship_reviews').fetchall(),[(1,)])
                self.db.execute('ROLLBACK TO origin_case')
                self.db.execute('RELEASE origin_case')

    def test_unpublished_native_owner_cannot_seal(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        self.baseline.backup(db)
        self.claim(db=db)
        with self.assertRaisesRegex(sqlite3.IntegrityError,'relationship evidence'):
            self.seal(db)
        self.publish(db)
        self.seal(db)

    def test_sealed_endpoint_identity_cannot_change_or_disappear(self):
        self.publish()
        self.claim()
        self.seal()
        for statement in (
            'UPDATE no_intro_archive_targets SET archive_id=13021 WHERE target_id=1',
            'DELETE FROM no_intro_archive_targets WHERE target_id=1',
            'UPDATE catalog_relationship_targets SET target_id=3 WHERE target_id=1',
            "INSERT OR REPLACE INTO no_intro_archive_targets VALUES(1,'no_intro_archive',13020)",
            'UPDATE no_intro_archive_descriptions SET number=NULL WHERE archive_id=13020',
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(statement)

    def test_native_owner_cannot_be_reused_and_target_id_cannot_be_issued_implicitly(self):
        self.endpoint()
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(2,'no_intro_archive')")
        for statement in (
            'INSERT INTO no_intro_archive_targets(target_id,archive_id) VALUES(2,13020)',
            'INSERT INTO no_intro_archive_targets(archive_id) VALUES(13021)',
            'INSERT INTO no_intro_archive_targets(target_id,archive_id) VALUES(2,13010)',
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(statement)

    def test_closed_endpoint_tables_use_relationship_not_native_publication_freezes(self):
        for table, _ in relationship_closure.ENDPOINTS:
            with self.subTest(table=table):
                self.assertEqual(self.db.execute(
                    "SELECT name FROM sqlite_schema WHERE type='trigger' AND name GLOB ?",
                    (f'candidate_immutable_{table}_*',)).fetchall(),[])
                self.assertEqual(self.db.execute(
                    "SELECT count(*) FROM sqlite_schema WHERE type='trigger' AND name GLOB ?",
                    (f'relationship_closure_{table}_*',)).fetchone(),(3,))

    def test_guards_hold_without_foreign_keys_or_recursive_triggers(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        self.published_baseline.backup(db)
        db.execute('PRAGMA foreign_keys=OFF')
        db.execute('PRAGMA recursive_triggers=OFF')
        self.claim(db=db)
        self.seal(db)
        for statement in (
            'DELETE FROM no_intro_archive_targets WHERE target_id=1',
            "INSERT OR REPLACE INTO no_intro_archive_targets VALUES(1,'no_intro_archive',13020)",
            "INSERT INTO no_intro_archive_targets VALUES(3,'no_intro_archive',999999)",
            'UPDATE no_intro_archive_descriptions SET number=NULL WHERE archive_id=13020',
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                db.execute(statement)

    def test_requested_archive_validation_is_bounded_with_512_unrelated_owners(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.execute('PRAGMA foreign_keys=ON')
        self.published_baseline.backup(db)
        self.endpoint(db=db)
        predicate = relationship_closure._target_valid(
            'requested.target_id',relationship_closure._native_kinds(assemble.owners()))
        query = f'SELECT {predicate} FROM (SELECT ? AS target_id) AS requested'

        def measure():
            # Schema/statistics reload after ANALYZE is connection setup, not
            # endpoint execution. Warm the exact prepared point query first.
            self.assertEqual(db.execute(query,(1,)).fetchall(),[(1,)])
            steps = 0

            def step():
                nonlocal steps
                steps += 1
                return 0

            db.set_progress_handler(step,1)
            try:
                self.assertEqual(db.execute(query,(1,)).fetchall(),[(1,)])
            finally:
                db.set_progress_handler(None,0)
            return steps

        db.execute('ANALYZE')
        before = measure()
        export_query_check.add_unrelated_owners(db,512)
        for offset in range(512):
            archive = 100000 + offset
            db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'no_intro_export_archive')",
                       (archive,20000+offset))
            db.execute('INSERT INTO no_intro_archive_descriptions'
                       '(archive_id,set_id,number,source_order,source_line,source_column,source_end_line,source_end_column) '
                       "VALUES(?,?,'0001',2,1,1,1,3)",(archive,40000+offset))
            self.endpoint(1000+offset,archive,db)
        db.execute('ANALYZE')
        after = measure()
        self.assertLessEqual(after,before+20,(before,after))
        self.assertEqual(db.execute(query,(999999,)).fetchall(),[(0,)])
        print(f'archive endpoint VM steps with 512 unrelated owners: {before} -> {after}')

    def corrupt_copy(self):
        # Bypass selected physical-owner guards only to exercise the independent
        # closure audit. No application database or original source is opened.
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        self.published_baseline.backup(db)
        db.execute('PRAGMA foreign_keys=OFF')
        for (name,) in db.execute("SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name IN "
                                 "('no_intro_archive_descriptions','no_intro_export_games','catalog_source_elements',"
                                 "'catalog_sets','catalog_set_groups','no_intro_export_documents','catalog_reading_rules')").fetchall():
            db.execute(f'DROP TRIGGER {assemble.identifier(name)}')
        return db

    def test_corrupted_native_ancestry_rejects_seal_and_remains_visible_in_audit(self):
        for statement in (
            'DELETE FROM no_intro_archive_descriptions WHERE archive_id=13020',
            "UPDATE catalog_source_elements SET element_kind='no_intro_export_game' WHERE source_element_id=13020",
            'UPDATE catalog_source_elements SET edition_id=10200 WHERE source_element_id=13020',
            'DELETE FROM no_intro_export_games WHERE set_id=13010',
            'UPDATE catalog_source_elements SET edition_id=10200 WHERE source_element_id=13010',
            'UPDATE catalog_sets SET set_group_id=10210 WHERE set_id=13010',
            'UPDATE no_intro_export_documents SET root_set_group_id=10210 WHERE edition_id=10300',
            "UPDATE catalog_reading_rules SET format_family='no_intro_dat' WHERE reading_rules_id=10300",
        ):
            with self.subTest(statement=statement):
                db = self.corrupt_copy()
                self.publish(db)
                self.claim(db=db)
                db.execute(statement)
                with self.assertRaisesRegex(sqlite3.IntegrityError,'relationship evidence'):
                    self.seal(db)
                db.execute('DROP TRIGGER relationship_closure_evidence_guard')
                self.seal(db)
                self.assertTrue(db.execute('SELECT * FROM candidate_relationship_closure_problems').fetchall())
                db.close()

    def test_competing_archive_or_game_native_owners_reject_seal_and_are_audited(self):
        for owner in (13010,13020):
            with self.subTest(owner=owner):
                db = self.corrupt_copy()
                self.claim(db=db)
                for (name,) in db.execute("SELECT name FROM sqlite_schema WHERE type='trigger' "
                                         "AND tbl_name='no_intro_dat_games'").fetchall():
                    db.execute(f'DROP TRIGGER {assemble.identifier(name)}')
                db.execute('INSERT INTO no_intro_dat_games VALUES(?,NULL)',(owner,))
                with self.assertRaisesRegex(sqlite3.IntegrityError,'relationship evidence'):
                    self.seal(db)
                db.execute('DROP TRIGGER relationship_closure_evidence_guard')
                self.seal(db)
                self.assertTrue(db.execute('SELECT * FROM candidate_relationship_closure_problems').fetchall())
                db.close()


if __name__ == '__main__':
    unittest.main(verbosity=2)
