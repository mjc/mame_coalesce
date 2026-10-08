#!/usr/bin/env python3
"""Fast constructed-row witnesses, not authentic-import or production-gate proof."""

import argparse
import sqlite3
import unittest

import assemble


def shared_connection(foreign_keys=True):
    connection = sqlite3.connect(":memory:")
    connection.execute(f"PRAGMA foreign_keys={'ON' if foreign_keys else 'OFF'}")
    sql = (assemble.ROOT / "shared.sql").read_text().replace("/* SOURCE_ELEMENT_KINDS */", "'mame_machine','mame_rom'")
    connection.executescript(sql + (assemble.ROOT / "diagnostics.sql").read_text())
    guards, _ = assemble.foreign_key_guards(connection)
    connection.executescript("\n".join(guards + assemble.collision_guards(connection)
                                     + assemble.published_fact_guards(connection, ())
                                     + assemble.immutable_dictionary_sql()))
    return connection


def seed_identity(connection):
    connection.execute("INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL)")
    connection.execute("INSERT INTO catalogs VALUES(1,1,'catalog','Catalog')")
    connection.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,128,'object','zstd')", (b's' * 32,))
    connection.execute("INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','observed','pinned','candidate','v3')")
    connection.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    connection.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")


class SharedWitnesses(unittest.TestCase):
    def setUp(self):
        self.connection = shared_connection()
        self.addCleanup(self.connection.close)
        seed_identity(self.connection)

    def test_uuid_is_binary_and_exactly_sixteen_bytes(self):
        db = self.connection
        db.execute("INSERT INTO file_id_registries VALUES(1,?)", (b'r' * 16,))
        uuid = bytes(range(16))
        db.execute("INSERT INTO shared_catalog_files VALUES(?,1)", (uuid,))
        self.assertEqual(db.execute("SELECT file_uuid,typeof(file_uuid),length(file_uuid) FROM shared_catalog_files").fetchone(), (uuid, 'blob', 16))
        for malformed in (b'0' * 15, b'1' * 17, '0' * 32):
            with self.subTest(malformed=malformed), self.assertRaises(sqlite3.IntegrityError):
                db.execute("INSERT INTO shared_catalog_files VALUES(?,1)", (malformed,))

    def test_edition_identity_includes_coverage_not_import_attempt(self):
        db = self.connection
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,1,2,NULL,1)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO catalog_editions VALUES(3,1,1,1,1,NULL,NULL)")
        for run in (1, 2):
            db.execute("INSERT INTO catalog_imports VALUES(?,?,1,1,1,NULL,1,NULL,'succeeded','start','end')", (run, f'run-{run}'))
        self.assertEqual(db.execute("SELECT count(*) FROM catalog_editions").fetchone()[0], 2)
        self.assertEqual(db.execute("SELECT count(*) FROM catalog_imports WHERE edition_id=1").fetchone()[0], 2)

    def test_unknown_or_complete_coverage_has_no_members(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute("INSERT INTO catalog_covered_sets VALUES(1,0,'root',NULL,'name','covered')")

    def test_reverse_coverage_kind_change_rejects_and_audit_attributes_corruption(self):
        db = self.connection
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_covered_sets VALUES(2,0,'root',NULL,'name','unknown')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_coverage SET kind='complete' WHERE coverage_id=2")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,1,2,NULL,NULL)")
        db.execute('DROP TRIGGER candidate_coverage_kind_update')
        db.execute('DROP TRIGGER candidate_referenced_coverage_update')
        db.execute("UPDATE catalog_coverage SET kind='complete' WHERE coverage_id=2")
        self.assertEqual(db.execute("SELECT problem,edition_id FROM candidate_shared_integrity_problems WHERE problem='coverage_member'").fetchall(), [('coverage_member',2)])

    def test_hash_byte_algorithm_lengths_are_closed(self):
        for number, (algorithm, length) in enumerate((('crc32', 4), ('md5', 16), ('sha1', 20), ('sha256', 32)), 1):
            self.connection.execute("INSERT INTO hash_values VALUES(?,?,?)", (number, algorithm, b'x' * length))
            with self.subTest(algorithm=algorithm), self.assertRaises(sqlite3.IntegrityError):
                self.connection.execute("INSERT INTO hash_values VALUES(?,?,?)", (number + 10, algorithm, b'x' * (length + 1)))

    def test_required_integral_counts_reject_fractions(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.connection.execute("INSERT INTO catalog_source_files VALUES(2,?,NULL,0.5,'other','zstd')", (b't' * 32,))

    def test_fk_off_still_rejects_forward_reverse_and_replace_attacks(self):
        for enabled in (True, False):
            with self.subTest(foreign_keys=enabled), shared_connection(enabled) as db:
                seed_identity(db)
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("INSERT INTO catalogs VALUES(2,99,'orphan','Orphan')")
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("DELETE FROM catalog_publishers WHERE publisher_id=1")
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("UPDATE catalog_publishers SET publisher_id=2 WHERE publisher_id=1")
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("INSERT OR REPLACE INTO catalog_publishers VALUES(2,'publisher','Replaced',NULL)")
                db.execute("INSERT INTO catalog_publishers VALUES(2,'other','Other',NULL)")
                db.execute('PRAGMA recursive_triggers=OFF')
                with self.assertRaises(sqlite3.IntegrityError):
                    db.execute("UPDATE OR REPLACE catalog_publishers SET publisher_key='publisher' WHERE publisher_id=2")
                self.assertEqual(db.execute("SELECT publisher_id,display_name FROM catalog_publishers ORDER BY publisher_id").fetchall(), [(1, 'Publisher'), (2, 'Other')])

    def test_receipt_requires_success_and_exact_publisher_source_ancestry(self):
        db = self.connection
        db.execute("INSERT INTO catalog_fetch_attempts VALUES(1,1,'fetch','uri','GET','start','end','failed',NULL,'unavailable')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO catalog_file_receipts VALUES(1,'receipt',1,1)")
        db.execute("UPDATE catalog_fetch_attempts SET outcome='retained' WHERE fetch_attempt_id=1")
        db.execute("INSERT INTO catalog_file_receipts VALUES(1,'receipt',1,1)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_fetch_attempts SET outcome='failed' WHERE fetch_attempt_id=1")
        db.execute("INSERT INTO catalog_publishers VALUES(2,'second','Other',NULL)")
        db.execute("INSERT INTO catalogs VALUES(2,2,'other','Other')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO catalog_editions VALUES(2,2,1,1,1,1,NULL)")

    def test_partial_coordinates_null_empty_and_blob_excerpt_survive(self):
        db = self.connection
        db.execute("INSERT INTO catalog_imports VALUES(1,'failed',1,1,1,NULL,NULL,NULL,'failed','start','end')")
        excerpt = 'é☃'.encode()
        db.execute("""INSERT INTO catalog_import_messages
            (message_id,import_id,message_order,severity,code,message,record_kind,record_name,field_name,offending_text,
             source_file_id,reading_rules_id,excerpt,source_view,excerpt_source_start,
             excerpt_problem_start,excerpt_problem_end,source_problem_start,source_problem_end,line)
            VALUES(1,1,0,'error','bad','Message',NULL,'','field','',1,1,?,'retained_original_bytes',10,2,5,12,15,7)""", (excerpt,))
        self.assertEqual(db.execute("SELECT record_kind,record_name,offending_text,line,column,excerpt FROM catalog_import_messages").fetchone(), (None, '', '', 7, None, excerpt))
        self.assertEqual(db.execute("SELECT * FROM candidate_message_integrity_problems").fetchall(), [])
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO catalog_import_message_elements VALUES(1,2,1,'primary')")

    def test_hash_states_have_one_value_owner_and_lossless_override(self):
        db = self.connection
        db.execute("INSERT INTO catalog_source_elements VALUES(1,1,'mame_rom')")
        db.execute("INSERT INTO catalog_media_entries VALUES(1,NULL)")
        db.execute("INSERT INTO hash_values VALUES(1,'sha1',?)", (b'\xab' * 20,))
        db.execute("INSERT INTO catalog_entry_hashes VALUES(1,1,'sha1',0,'value','whole_file',1,?)", ('AB' * 20,))
        db.execute("INSERT INTO catalog_entry_hashes VALUES(2,1,'crc',0,'empty','whole_file',NULL,NULL)")
        db.execute("INSERT INTO catalog_entry_hashes VALUES(3,1,'md5',0,'invalid','whole_file',NULL,'bogus')")
        db.execute("INSERT INTO invalid_catalog_entry_hashes VALUES(3,'invalid_md5')")
        self.assertEqual(db.execute("SELECT declared_text FROM declared_catalog_hash_text ORDER BY reported_hash_id").fetchall(), [('AB' * 20,), ('',), ('bogus',)])
        self.assertEqual(db.execute("SELECT * FROM candidate_shared_integrity_problems").fetchall(), [])
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_entry_hashes SET reported_text=? WHERE reported_hash_id=1", ('AC' * 20,))
        # Deliberately bypass this guard to test the independent reverse audit.
        db.execute('DROP TRIGGER candidate_hash_semantics_update')
        db.execute("UPDATE catalog_entry_hashes SET reported_text=? WHERE reported_hash_id=1", ('AC' * 20,))
        self.assertEqual(db.execute("SELECT problem FROM candidate_shared_integrity_problems").fetchall(), [('hash_override_bytes',)])

    def test_publication_seal_cannot_be_redirected_or_removed(self):
        db = self.connection
        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,1,2,NULL,1)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE published_catalog_editions SET edition_id=2,coverage_id=2")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("DELETE FROM published_catalog_editions")

    def test_interned_hash_meaning_cannot_be_rewritten(self):
        db = self.connection
        db.execute("INSERT INTO hash_values VALUES(1,'sha1',?)", (b'a' * 20,))
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE hash_values SET bytes=? WHERE hash_id=1", (b'b' * 20,))

    def test_update_cannot_move_unpublished_hash_into_published_owner(self):
        db = self.connection
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,1,2,NULL,NULL)")
        db.execute("INSERT INTO catalog_source_elements VALUES(11,1,'mame_rom')")
        db.execute("INSERT INTO catalog_source_elements VALUES(21,2,'mame_rom')")
        db.execute("INSERT INTO catalog_media_entries VALUES(11,NULL)")
        db.execute("INSERT INTO catalog_media_entries VALUES(21,NULL)")
        db.execute("INSERT INTO catalog_entry_hashes VALUES(1,21,'crc',0,'empty','unknown',NULL,NULL)")
        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_entry_hashes SET media_entry_id=11 WHERE reported_hash_id=1")
        self.assertEqual(db.execute('SELECT media_entry_id FROM catalog_entry_hashes').fetchall(), [(21,)])

    def test_existing_messages_prevent_reverse_import_ancestry_rewrite(self):
        db = self.connection
        db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,NULL,NULL,NULL,'running','start',NULL)")
        db.execute("INSERT INTO catalog_import_messages(message_id,import_id,message_order,severity,code,message,source_file_id,reading_rules_id) VALUES(1,1,0,'warning','code','message',1,1)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_imports SET status='succeeded',edition_id=1,finished_at='end' WHERE import_id=1")

    def test_previous_edition_chain_cannot_cycle(self):
        db = self.connection
        db.execute("INSERT INTO catalog_coverage VALUES(2,'partial')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,1,2,NULL,1)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE catalog_editions SET previous_edition_id=2 WHERE edition_id=1")

    def test_populated_compatibility_reads_use_membership_indexes(self):
        db = self.connection
        db.execute("INSERT INTO file_id_registries VALUES(1,?)", (b'r' * 16,))
        files = [(number.to_bytes(16, 'big'), 1) for number in range(1, 129)]
        db.executemany("INSERT INTO shared_catalog_files VALUES(?,?)", files)
        db.executemany("INSERT INTO shared_file_sizes VALUES(?,?)", [(uuid, number) for number, (uuid, _) in enumerate(files)])
        db.executemany("INSERT INTO hash_values VALUES(?,'sha1',?)", [(number, number.to_bytes(20, 'big')) for number in range(1, 1025)])
        db.executemany("INSERT INTO shared_file_hashes VALUES(?,?)", [(files[(number - 1) % 128][0], number) for number in range(1, 1025)])
        query = "SELECT byte_length FROM shared_file_sizes WHERE file_uuid=?"
        plan = ' '.join(row[3] for row in db.execute('EXPLAIN QUERY PLAN ' + query, (files[0][0],)))
        self.assertIn('PRIMARY KEY', plan)
        self.assertNotIn('SCAN ', plan)
        reverse = ' '.join(row[3] for row in db.execute('EXPLAIN QUERY PLAN SELECT file_uuid FROM shared_file_hashes WHERE hash_id=1'))
        self.assertIn('shared_file_hash_lookup', reverse)
        self.assertEqual(db.execute("SELECT count(*) FROM shared_file_hashes").fetchone()[0], 1024)


class AssembledWitnesses(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.db.execute('PRAGMA foreign_keys=ON')
        cls.db.executescript(assemble.assemble())

    @classmethod
    def tearDownClass(cls):
        cls.db.close()

    def setUp(self):
        self.db.execute('SAVEPOINT witness')
        seed_identity(self.db)

    def tearDown(self):
        self.db.execute('ROLLBACK TO witness')
        self.db.execute('RELEASE witness')

    def mame_root(self):
        self.db.execute("""INSERT INTO mame_documents
            (edition_id,debug,debug_specified,mameconfig,source_line,source_column,
             extent_view,extent_start,extent_end,location_view,start_line,start_column,end_line,end_column,column_convention)
            VALUES(1,0,0,'',1,1,'retained_original_bytes',0,128,'transport_decoded_xml_text',1,1,2,1,'one_based_unicode_scalar')""")
        self.db.execute("INSERT INTO mame_document_facts_attribute_positions VALUES(1,'mameconfig',0,0,1,7)")

    def test_every_view_prepares_and_all_foreign_key_targets_exist(self):
        for (view,) in self.db.execute("SELECT name FROM sqlite_schema WHERE type='view'").fetchall():
            with self.subTest(view=view):
                self.db.execute(f'SELECT * FROM {assemble.identifier(view)} LIMIT 0')
        self.assertEqual(self.db.execute('PRAGMA foreign_key_check').fetchall(), [])

    def test_common_set_and_list_notes_share_the_actual_group_sequence(self):
        db = self.db
        db.execute("INSERT INTO catalog_reading_rules VALUES(2,'software','software','single','pinned','candidate','v1')")
        db.execute("INSERT INTO catalog_editions VALUES(2,1,1,2,1,NULL,NULL)")
        db.execute("INSERT INTO software_documents(edition_id,envelope_kind,location_view,start_line,start_column,end_line,end_column,column_convention) VALUES(2,'single_list','transport_decoded_xml_text',1,1,2,1,'one_based_unicode_scalar')")
        db.execute("INSERT INTO catalog_set_groups VALUES(10,2,'software_list')")
        db.execute("INSERT INTO software_lists(set_group_id,name,location_view,start_line,start_column,end_line,end_column,column_convention) VALUES(10,'list','transport_decoded_xml_text',1,1,2,1,'one_based_unicode_scalar')")
        db.execute("INSERT INTO catalog_source_elements VALUES(100,2,'software_item')")
        db.execute("INSERT INTO catalog_sets VALUES(100,10,'title',0,1,1)")
        db.execute("INSERT INTO catalog_source_elements VALUES(101,2,'software_list_notes')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO software_list_notes VALUES(101,10,'',0,1,1)")
        db.execute("INSERT INTO software_list_notes VALUES(101,10,'',1,1,1)")

    def test_root_diagnostic_requires_proven_containment_and_excludes_eof_anchor(self):
        db = self.db
        self.mame_root()
        db.execute("INSERT INTO catalog_imports VALUES(1,'run',1,1,1,NULL,1,NULL,'running','start',NULL)")
        for number, start, end, line, column in ((1,127,128,7,None),(2,128,128,None,None),(3,127,128,2,1)):
            db.execute("""INSERT INTO catalog_import_messages
                (message_id,import_id,message_order,severity,code,message,edition_id,source_file_id,reading_rules_id,
                 source_view,source_problem_start,source_problem_end,line,column,location_view,column_convention)
                VALUES(?,1,?,'warning','code','message',1,1,1,'retained_original_bytes',?,?,?,?,
                       'transport_decoded_xml_text','one_based_unicode_scalar')""", (number,number,start,end,line,column))
        db.execute("INSERT INTO mame_root_import_messages VALUES(1,1,'primary')")
        for number in (2,3):
            with self.subTest(message=number), self.assertRaises(sqlite3.IntegrityError):
                db.execute("INSERT INTO mame_root_import_messages VALUES(?,1,'primary')", (number,))

    def test_published_root_and_attribute_positions_are_immutable(self):
        db = self.db
        self.mame_root()
        self.assertEqual(db.execute('SELECT * FROM candidate_integrity_problems WHERE edition_id=1').fetchall(), [])
        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE mame_documents SET build='changed'")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO mame_document_facts_attribute_positions VALUES(1,'debug',0,1,1,20)")
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute('UPDATE catalog_source_files SET byte_length=129')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core', action='store_true', help='only shared-core witnesses; not assembled-schema evidence')
    args = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SharedWitnesses)
    if not args.core:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(AssembledWitnesses))
    outcome = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not outcome.wasSuccessful())


if __name__ == '__main__':
    main()
