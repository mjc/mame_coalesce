#!/usr/bin/env python3
"""Bounded in-memory witnesses for reverse receipt provenance guards."""

import sqlite3
import unittest

import assemble
import receipt_ancestry


def seed(db):
    db.executemany('INSERT INTO catalog_publishers VALUES(?,?,?,NULL)', [
        (1, 'publisher-a', 'Publisher A'), (2, 'publisher-b', 'Publisher B')])
    db.executemany('INSERT INTO catalogs VALUES(?,?,?,?)', [
        (1, 1, 'catalog-a', 'Catalog A'), (2, 2, 'catalog-b', 'Catalog B')])
    db.executemany('INSERT INTO catalog_source_files VALUES(?,?,NULL,10,?,\'zstd\')', [
        (1, b'a' * 32, 'object-a'), (2, b'b' * 32, 'object-b')])
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','test','spec','parser','rules')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    db.execute("INSERT INTO catalog_fetch_attempts VALUES(1,1,'attempt-a','uri','GET','start','end','retained',NULL,'unavailable')")
    db.execute("INSERT INTO catalog_fetch_attempts VALUES(2,2,'attempt-b','uri','GET','start','end','retained',NULL,'unavailable')")
    db.execute("INSERT INTO catalog_fetch_attempts VALUES(3,1,'attempt-c','uri','GET','start','end','retained',NULL,'unavailable')")
    db.execute("INSERT INTO catalog_file_receipts VALUES(1,'receipt-a',1,1)")
    db.execute("INSERT INTO catalog_file_receipts VALUES(2,'receipt-b',2,1)")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,1,NULL)")
    # Bounded seed only: omit native publication closure to create the seal.
    # This does not claim the minimal fixture satisfies full publication closure.
    db.execute('DROP TRIGGER candidate_publication_closure')
    db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")


class ReceiptAncestry(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())
        seed(cls.db)
        cls.db.commit()

        # The corruption-only witness needs FK enforcement disabled before a
        # savepoint begins. Back up the one composed, seeded fixture once.
        cls.corruption_db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.corruption_db.close)
        cls.corruption_db.execute('PRAGMA foreign_keys=OFF')
        cls.db.backup(cls.corruption_db)

    def setUp(self):
        self.db = (self.corruption_db
                   if self._testMethodName == 'test_corruption_only_late_receipt_insert_checks_preexisting_edition_reference'
                   else type(self).db)
        self.db.execute('SAVEPOINT receipt_ancestry_test')

    def tearDown(self):
        self.db.execute('ROLLBACK TO receipt_ancestry_test')
        self.db.execute('RELEASE receipt_ancestry_test')

    def test_composed_published_receipt_same_source_attempt_rewrite_is_rejected(self):
        db = self.db
        self.assertEqual(db.execute('PRAGMA foreign_keys').fetchone(), (1,))
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'candidate foreign key closure'):
            db.execute("INSERT INTO catalogs VALUES(3,99,'orphan','Orphan')")

        with self.assertRaisesRegex(sqlite3.IntegrityError, 'published receipt provenance is immutable'):
            db.execute('UPDATE catalog_file_receipts SET fetch_attempt_id=3 WHERE file_receipt_id=1')
        self.assertEqual(db.execute(
            'SELECT fetch_attempt_id,source_file_id FROM catalog_file_receipts WHERE file_receipt_id=1').fetchone(), (1, 1))

        with self.assertRaisesRegex(sqlite3.IntegrityError, 'published receipt provenance is immutable'):
            db.execute('UPDATE catalog_file_receipts SET source_file_id=2 WHERE file_receipt_id=1')
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'published receipt provenance is immutable'):
            db.execute("UPDATE catalog_file_receipts SET receipt_key='rewritten' WHERE file_receipt_id=1")

    def test_unreferenced_receipt_remains_editable(self):
        db = self.db

        db.execute('UPDATE catalog_file_receipts SET source_file_id=2 WHERE file_receipt_id=2')
        self.assertEqual(db.execute(
            'SELECT source_file_id FROM catalog_file_receipts WHERE file_receipt_id=2').fetchone(), (2,))

    def test_populated_receipt_reference_lookups_use_reverse_indexes(self):
        db = self.db
        db.executemany('INSERT INTO catalog_coverage VALUES(?,\'partial\')',
                       ((number,) for number in range(2, 130)))
        db.executemany('INSERT INTO catalog_editions VALUES(?,1,1,1,?,NULL,NULL)',
                       ((number, number) for number in range(2, 130)))
        db.execute("INSERT INTO catalog_imports VALUES(1,'run-1',1,1,1,1,1,NULL,'succeeded','start','end')")
        db.executemany(
            "INSERT INTO catalog_imports VALUES(?,?,1,1,1,NULL,NULL,NULL,'running','start',NULL)",
            ((number, f'run-{number}') for number in range(2, 130)))

        edition_plan = ' '.join(row[3] for row in db.execute(
            'EXPLAIN QUERY PLAN SELECT 1 FROM catalog_editions AS edition '
            'JOIN published_catalog_editions AS publication USING (edition_id) '
            'WHERE edition.file_receipt_id=?', (1,)))
        import_plan = ' '.join(row[3] for row in db.execute(
            "EXPLAIN QUERY PLAN SELECT 1 FROM catalog_imports AS run "
            "WHERE run.file_receipt_id=? AND run.status='succeeded'", (1,)))

        self.assertIn('catalog_editions_by_receipt', edition_plan)
        self.assertIn('catalog_imports_by_receipt', import_plan)

    def test_independent_audit_finds_bypassed_published_rewrite(self):
        db = self.db
        db.execute('DROP TRIGGER candidate_receipt_reverse_ancestry_update')
        db.execute('DROP TRIGGER candidate_receipt_provenance_immutable_update')
        db.execute('UPDATE catalog_file_receipts SET source_file_id=2 WHERE file_receipt_id=1')

        self.assertEqual(db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_shared_integrity_problems "
            "WHERE problem='edition_receipt_ancestry'").fetchall(), [('edition_receipt_ancestry', 1, 1)])

    def test_import_receipt_rewrite_is_rejected_and_independently_audited(self):
        db = self.db
        db.execute('DROP TRIGGER candidate_publication_immutable_delete')
        db.execute('DELETE FROM published_catalog_editions WHERE edition_id=1')
        db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,1,1,NULL,'succeeded','start','end')")

        with self.assertRaisesRegex(sqlite3.IntegrityError, 'published receipt provenance is immutable'):
            db.execute('UPDATE catalog_file_receipts SET fetch_attempt_id=3 WHERE file_receipt_id=1')

        db.execute('DROP TRIGGER candidate_receipt_reverse_ancestry_update')
        db.execute('DROP TRIGGER candidate_receipt_provenance_immutable_update')
        db.execute('UPDATE catalog_file_receipts SET source_file_id=2 WHERE file_receipt_id=1')
        self.assertEqual(receipt_ancestry.sql(db)[1], [])
        self.assertEqual(db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_shared_integrity_problems "
            "WHERE problem='import_receipt_ancestry'").fetchall(), [('import_receipt_ancestry', 1, 1)])

    def test_terminal_imports_freeze_and_running_imports_can_finish_with_fk_on_and_off(self):
        for db in (type(self).db, type(self).corruption_db):
            foreign_keys = db.execute('PRAGMA foreign_keys').fetchone()[0]
            self.assertEqual(foreign_keys, int(db is type(self).db))
            db.execute('SAVEPOINT terminal_import_witness')
            try:
                db.execute('DROP TRIGGER candidate_publication_immutable_delete')
                db.execute('DELETE FROM published_catalog_editions WHERE edition_id=1')
                db.execute("INSERT INTO catalog_fetch_attempts VALUES(4,1,'attempt-d','uri','GET','start','end','retained',NULL,'unavailable')")
                db.execute("INSERT INTO catalog_file_receipts VALUES(3,'receipt-c',3,1)")
                db.execute("INSERT INTO catalog_imports VALUES(1,'succeeded-run',1,1,1,1,1,NULL,'succeeded','start','end')")
                db.execute("INSERT INTO catalog_imports VALUES(2,'failed-run',1,1,1,1,NULL,NULL,'failed','start','end')")
                db.execute("INSERT INTO catalog_imports VALUES(3,'running-run',1,1,1,3,NULL,NULL,'running','start',NULL)")

                db.execute("UPDATE catalog_imports SET status='succeeded',edition_id=1,finished_at='end' WHERE import_id=3")
                self.assertEqual(db.execute(
                    'SELECT status FROM catalog_imports WHERE import_id=3').fetchone(), ('succeeded',))

                for import_id, fields in (
                    (1, "file_receipt_id=NULL"),
                    (2, "status='running',finished_at=NULL"),
                    (3, "status='running',edition_id=NULL,finished_at=NULL"),
                ):
                    with self.subTest(foreign_keys=foreign_keys, import_id=import_id):
                        with self.assertRaisesRegex(sqlite3.IntegrityError, 'terminal catalog imports are immutable'):
                            db.execute(f'UPDATE catalog_imports SET {fields} WHERE import_id={import_id}')
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'terminal catalog imports are immutable'):
                        db.execute(f'DELETE FROM catalog_imports WHERE import_id={import_id}')

                for sql in (
                    "UPDATE catalog_file_receipts SET fetch_attempt_id=4 WHERE file_receipt_id=1",
                    "UPDATE catalog_file_receipts SET receipt_key='rewritten' WHERE file_receipt_id=1",
                ):
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'published receipt provenance is immutable'):
                        db.execute(sql)
            finally:
                db.execute('ROLLBACK TO terminal_import_witness')
                db.execute('RELEASE terminal_import_witness')

    def test_catalog_publisher_change_cannot_break_receipt_ancestry(self):
        db = self.db

        with self.assertRaisesRegex(sqlite3.IntegrityError, 'catalog publisher conflicts'):
            db.execute('UPDATE catalogs SET publisher_id=2 WHERE catalog_id=1')

    def test_corruption_only_late_receipt_insert_checks_preexisting_edition_reference(self):
        # Deliberately disable FK enforcement and the forward edition/FK guards
        # to construct an invalid prior reference. This is not normal-writer
        # proof; the ordinary integration witness above keeps composed FK guards active.
        db = self.db
        db.execute('DROP TRIGGER candidate_edition_ancestry_insert')
        receipt_fk = next(row[0] for row in db.execute('PRAGMA foreign_key_list(catalog_editions)')
                          if row[2] == 'catalog_file_receipts')
        db.execute(f'DROP TRIGGER candidate_fk_catalog_editions_{receipt_fk}_insert')
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,2,1,2,9,NULL)")

        with self.assertRaisesRegex(sqlite3.IntegrityError, 'receipt reference ancestry'):
            db.execute("INSERT INTO catalog_file_receipts VALUES(9,'late-receipt',3,1)")


if __name__ == '__main__':
    unittest.main()
