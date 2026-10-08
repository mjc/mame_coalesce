#!/usr/bin/env python3
"""Constructed receipt and terminal-diagnostic lifecycle regressions."""

import sqlite3
import unittest

import assemble


def seed(db):
    db.executemany('INSERT INTO catalog_publishers VALUES(?,?,?,NULL)', [
        (1, 'publisher', 'Publisher')])
    db.execute("INSERT INTO catalogs VALUES(1,1,'catalog','Catalog')")
    db.executemany("INSERT INTO catalog_source_files VALUES(?,?,NULL,10,?,'zstd')", [
        (1, b'a' * 32, 'object-a'), (2, b'b' * 32, 'object-b')])
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','test','spec','parser','rules')")
    db.executemany("INSERT INTO catalog_coverage VALUES(?,'complete')", [(1,), (2,)])
    db.executemany("""INSERT INTO catalog_fetch_attempts
        VALUES(?,?,?,'uri','GET','start','end','retained',NULL,'verified')""", [
        (1, 1, 'attempt-a'), (2, 1, 'attempt-b')])
    db.execute("""INSERT INTO catalog_fetch_attempts
        VALUES(3,1,'attempt-unused','draft-uri','GET','start',NULL,'unretained',NULL,'unavailable')""")
    db.executemany('INSERT INTO catalog_fetch_headers VALUES(?,?,?,?)', [
        (1, 0, 'etag', 'before-a'), (2, 0, 'etag', 'before-b')])
    db.executemany('INSERT INTO catalog_fetch_hashes VALUES(?,?,?,?)', [
        (1, 0, 'crc32', b'abcd'), (2, 0, 'crc32', b'efgh')])
    db.executemany('INSERT INTO catalog_file_receipts VALUES(?,?,?,?)', [
        (1, 'receipt-a', 1, 1), (2, 'receipt-b', 2, 2)])
    db.execute('INSERT INTO catalog_editions VALUES(1,1,1,1,1,1,NULL)')
    db.execute("INSERT INTO catalog_source_elements VALUES(700,1,'mame_machine')")
    db.execute('DROP TRIGGER candidate_publication_closure')
    db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")


class ReceiptEvidenceLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())
        seed(cls.db)
        cls.db.commit()

    def setUp(self):
        self.db = type(self).db
        self.db.execute('SAVEPOINT receipt_evidence_test')

    def tearDown(self):
        self.db.execute('ROLLBACK TO receipt_evidence_test')
        self.db.execute('RELEASE receipt_evidence_test')

    def test_draft_receipt_headers_and_hashes_remain_constructible(self):
        db = self.db
        db.execute("UPDATE catalog_fetch_headers SET value='draft-edit' WHERE fetch_attempt_id=2")
        db.execute("UPDATE catalog_fetch_hashes SET expected_bytes=? WHERE fetch_attempt_id=2", (b'1234',))
        db.execute("INSERT INTO catalog_fetch_headers VALUES(2,1,'last-modified','draft')")
        db.execute("INSERT INTO catalog_fetch_hashes VALUES(2,1,'md5',?)", (b'x' * 16,))
        db.execute('DELETE FROM catalog_fetch_headers WHERE fetch_attempt_id=2 AND list_order=1')
        db.execute('DELETE FROM catalog_fetch_hashes WHERE fetch_attempt_id=2 AND list_order=1')
        self.assertEqual(db.execute(
            'SELECT value FROM catalog_fetch_headers WHERE fetch_attempt_id=2').fetchone(), ('draft-edit',))
        # A receipt that is still unused does not seal its parent aggregate.
        # Retaining a receipt already freezes its parent fetch identity, even
        # before publication. The new child seal must not weaken that rule.
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("""UPDATE catalog_fetch_attempts
                SET uri='draft-uri-2',method='POST',verification_status='mismatch'
                WHERE fetch_attempt_id=2""")
        db.execute("""UPDATE catalog_fetch_attempts
            SET uri='retried-uri',method='HEAD',outcome='failed',
                verification_status='mismatch'
            WHERE fetch_attempt_id=3""")
        db.execute('DELETE FROM catalog_fetch_attempts WHERE fetch_attempt_id=3')

    def test_published_receipt_seals_every_acquisition_child_operation(self):
        db = self.db
        for table, value_column, value in (
            ('catalog_fetch_headers', 'value', 'late'),
            ('catalog_fetch_hashes', 'expected_bytes', b'wxyz'),
        ):
            with self.subTest(table=table, operation='update'), self.assertRaises(sqlite3.IntegrityError):
                db.execute(f'UPDATE {table} SET {value_column}=? WHERE fetch_attempt_id=1', (value,))
            with self.subTest(table=table, operation='delete'), self.assertRaises(sqlite3.IntegrityError):
                db.execute(f'DELETE FROM {table} WHERE fetch_attempt_id=1')
            with self.subTest(table=table, operation='insert'), self.assertRaises(sqlite3.IntegrityError):
                if table.endswith('headers'):
                    db.execute("INSERT INTO catalog_fetch_headers VALUES(1,1,'late','late')")
                else:
                    db.execute("INSERT INTO catalog_fetch_hashes VALUES(1,1,'crc32',?)", (b'late',))

            with self.subTest(table=table, reparent='draft into sealed'), self.assertRaises(sqlite3.IntegrityError):
                db.execute(f'UPDATE {table} SET fetch_attempt_id=1 WHERE fetch_attempt_id=2')
            with self.subTest(table=table, reparent='sealed into draft'), self.assertRaises(sqlite3.IntegrityError):
                db.execute(f'UPDATE {table} SET fetch_attempt_id=2 WHERE fetch_attempt_id=1')

    def test_published_receipt_seals_parent_fetch_attempt_update_delete_and_reparent(self):
        db = self.db
        for statement in (
            "UPDATE catalog_fetch_attempts SET uri='rewritten',method='POST',outcome='unretained',verification_status='mismatch' WHERE fetch_attempt_id=1",
            'UPDATE catalog_fetch_attempts SET fetch_attempt_id=3 WHERE fetch_attempt_id=1',
            'UPDATE catalog_fetch_attempts SET fetch_attempt_id=1 WHERE fetch_attempt_id=3',
            'DELETE FROM catalog_fetch_attempts WHERE fetch_attempt_id=1',
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                db.execute(statement)

    def test_terminal_failed_source_only_attempt_seals_receipt_acquisition(self):
        db = self.db
        db.execute("""INSERT INTO catalog_imports
            (import_id,import_key,catalog_id,source_file_id,reading_rules_id,file_receipt_id,
             edition_id,status,started_at)
            VALUES(2,'failed-source-only',1,2,1,2,NULL,'running','start')""")
        db.execute("""INSERT INTO catalog_import_messages
            (message_id,import_id,message_order,severity,code,message,edition_id,
             source_file_id,reading_rules_id)
            VALUES(2,2,0,'error','parse_error','Malformed input',NULL,2,1)""")
        db.execute("UPDATE catalog_import_messages SET message='Final failure detail' WHERE message_id=2")
        db.execute("UPDATE catalog_imports SET status='failed',finished_at='end' WHERE import_id=2")

        for statement, parameters in (
            ("UPDATE catalog_fetch_headers SET value='rewritten' WHERE fetch_attempt_id=2", ()),
            ("UPDATE catalog_fetch_hashes SET expected_bytes=X'31323334' WHERE fetch_attempt_id=2", ()),
            ("DELETE FROM catalog_fetch_headers WHERE fetch_attempt_id=2", ()),
            ("DELETE FROM catalog_fetch_hashes WHERE fetch_attempt_id=2", ()),
            ("INSERT INTO catalog_fetch_headers VALUES(2,1,'late','late')", ()),
            ("INSERT INTO catalog_fetch_hashes VALUES(2,1,'crc32',X'31323334')", ()),
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                db.execute(statement, parameters)
        for statement in (
            "UPDATE catalog_fetch_attempts SET uri='rewritten',method='POST',outcome='unretained',verification_status='mismatch' WHERE fetch_attempt_id=2",
            'DELETE FROM catalog_fetch_attempts WHERE fetch_attempt_id=2',
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                db.execute(statement)

    def test_failed_source_only_payload_and_external_evidence_freeze_at_terminalization(self):
        db = self.db
        db.execute("""INSERT INTO catalog_imports
            (import_id,import_key,catalog_id,source_file_id,reading_rules_id,file_receipt_id,
             edition_id,status,started_at)
            VALUES(2,'failed-source-only',1,2,1,2,NULL,'running','start')""")
        db.execute("""INSERT INTO catalog_import_messages
            (message_id,import_id,message_order,severity,code,message,edition_id,
             source_file_id,reading_rules_id)
            VALUES(2,2,0,'error','parse_error','Initial detail',NULL,2,1)""")
        db.execute("UPDATE catalog_import_messages SET message='Final detail' WHERE message_id=2")
        db.execute("""INSERT INTO catalog_import_message_external_evidence
            VALUES(2,700,1,'comparison_reference')""")
        db.execute("UPDATE catalog_imports SET status='failed',finished_at='end' WHERE import_id=2")

        for statement in (
            "UPDATE catalog_import_messages SET code='rewritten' WHERE message_id=2",
            'DELETE FROM catalog_import_messages WHERE message_id=2',
            "INSERT INTO catalog_import_messages (message_id,import_id,message_order,severity,code,message,edition_id,source_file_id,reading_rules_id) VALUES(3,2,1,'error','late','late',NULL,2,1)",
            "UPDATE catalog_import_message_external_evidence SET evidence_role='conflicting_candidate' WHERE message_id=2",
            'DELETE FROM catalog_import_message_external_evidence WHERE message_id=2',
            "INSERT INTO catalog_import_message_external_evidence VALUES(2,700,1,'conflicting_candidate')",
        ):
            with self.subTest(statement=statement), self.assertRaises(sqlite3.IntegrityError):
                db.execute(statement)

    def test_terminal_guard_inventory_covers_every_diagnostic_link_owner(self):
        db = self.db
        tables = {
            'catalog_import_messages',
            'catalog_import_message_elements',
            'catalog_import_message_external_evidence',
            *(table for table, _, _, _ in assemble.ROOT_LINKS),
        }
        for table in sorted(tables):
            for operation in ('insert', 'update', 'delete'):
                trigger = f'candidate_terminal_diagnostic_{table}_{operation}'
                with self.subTest(table=table, operation=operation):
                    self.assertIsNotNone(db.execute(
                        'SELECT 1 FROM sqlite_schema WHERE type=\'trigger\' AND name=?',
                        (trigger,)).fetchone(), trigger)


if __name__ == '__main__':
    unittest.main(verbosity=2)
