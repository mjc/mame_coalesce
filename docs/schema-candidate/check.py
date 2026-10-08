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


class HashRoutingWitnesses(unittest.TestCase):
    """Positive and adversarial identity routing, not algorithm/format validity."""

    def test_all_32_hash_routes_accept_their_actual_field_code_types(self):
        db = sqlite3.connect(':memory:')
        self.addCleanup(db.close)
        db.executescript('''
            CREATE TABLE catalog_source_elements(source_element_id INTEGER PRIMARY KEY,edition_id INTEGER);
            CREATE TABLE catalog_entry_hashes(reported_hash_id INTEGER PRIMARY KEY,media_entry_id INTEGER,source_hash_field TEXT,field_occurrence INTEGER);
        ''')
        routes = assemble.hash_routes()
        self.assertEqual(len(routes),32)
        tables = set()
        for row in routes:
            if row['table'] in tables:
                continue
            affinity = 'INTEGER' if row['field_code'].isdigit() else 'TEXT'
            role = ',value_line INTEGER,value_column INTEGER' if row.get('role_constraint')=='Value' else ''
            db.execute(f"CREATE TABLE {row['table']}({row['owner_column']} INTEGER,{row['code_column']} {affinity},{row['occurrence_column']} INTEGER,reported_hash_id INTEGER{role})")
            tables.add(row['table'])
        view,guards,problems = assemble.hash_position_sql(db)
        db.executescript(view + '\n' + '\n'.join(guards) + '\nCREATE VIEW routing_problems AS ' + ' UNION ALL '.join(problems))
        for row in routes:
            with self.subTest(table=row['table'],code=row['field_code']):
                db.execute('SAVEPOINT hash_route')
                db.execute('INSERT INTO catalog_source_elements VALUES(10,1)')
                db.execute('INSERT INTO catalog_entry_hashes VALUES(1,10,?,0)',(row['source_hash_field'],))
                role = (1,1) if row.get('role_constraint')=='Value' else ()
                values = (10,row['field_code'],0,1,*role)
                query = f"INSERT INTO {row['table']} VALUES({','.join('?' for _ in values)})"
                for wrong in ((11,*values[1:]),(*values[:2],1,*values[3:]),(10,'999' if row['field_code'].isdigit() else 'unknown',*values[2:])):
                    with self.assertRaisesRegex(sqlite3.IntegrityError,'exact native declaration'):
                        db.execute(query,wrong)
                if role:
                    with self.assertRaisesRegex(sqlite3.IntegrityError,'exact native declaration'):
                        db.execute(query,(*values[:-2],None,None))
                db.execute(query,values)
                self.assertEqual(db.execute('SELECT * FROM routing_problems').fetchall(),[])
                db.execute(f"DELETE FROM {row['table']}")
                self.assertEqual(db.execute('SELECT problem,edition_id FROM routing_problems').fetchall(),[('hash_position_count',1)])
                db.execute('ROLLBACK TO hash_route')
                db.execute('RELEASE hash_route')


class RelationshipRoutingWitnesses(unittest.TestCase):
    """All routes with thin typed owners: routing proof, not native field proof."""

    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        self.addCleanup(self.db.close)
        db = self.db
        self.routes = assemble.relationship_routes()
        db.executescript('''
            CREATE TABLE catalog_source_elements(source_element_id INTEGER PRIMARY KEY,edition_id INTEGER);
            CREATE TABLE catalog_relationships(relationship_id INTEGER PRIMARY KEY,origin TEXT,edition_id INTEGER);
        ''')
        kinds = ','.join(assemble.literal(row['kind']) for row in self.routes)
        db.execute(f'CREATE TABLE reported_catalog_relationships(relationship_id INTEGER PRIMARY KEY,reported_kind TEXT CHECK(reported_kind IN ({kinds})))')
        tables = set()
        for row in self.routes:
            table,owner = row['declaration_table'],row['declaration_owner']
            if table not in tables:
                link = ',link_kind TEXT' if row['link_kind']!='-' else ''
                db.execute(f'CREATE TABLE {table}({owner} INTEGER NOT NULL,relationship_id INTEGER UNIQUE NOT NULL{link})')
                tables.add(table)
            table,owner = row['position_table'],row['position_owner']
            if table not in tables:
                affinity = 'INTEGER' if row['field_code'].isdigit() else 'TEXT'
                db.execute(f'CREATE TABLE {table}({owner} INTEGER,field_kind {affinity},field_occurrence INTEGER,relationship_id INTEGER UNIQUE)')
                tables.add(table)
            if row['marker_table']!='-':
                db.execute(f"CREATE TABLE {row['marker_table']}({row['declaration_owner']} INTEGER PRIMARY KEY,marker TEXT)")
        views,guards,problems = assemble.relationship_position_sql(db)
        db.executescript(views + '\n' + '\n'.join(guards) + '\nCREATE VIEW routing_problems AS ' + ' UNION ALL '.join(problems))

    def seed_route(self,row):
        db = self.db
        db.execute('INSERT INTO catalog_source_elements VALUES(10,1),(11,1),(12,2)')
        db.execute("INSERT INTO catalog_relationships VALUES(1,'source',1)")
        db.execute('INSERT INTO reported_catalog_relationships VALUES(1,?)',(row['kind'],))
        link_column,link_value = (',link_kind',','+assemble.literal(row['link_kind'])) if row['link_kind']!='-' else ('','')
        db.execute(f"INSERT INTO {row['declaration_table']}({row['declaration_owner']},relationship_id{link_column}) VALUES(10,1{link_value})")

    def position(self,row,owner=10,code=None,occurrence=0,relationship=1):
        self.db.execute(f"INSERT INTO {row['position_table']} VALUES(?,?,?,?)",(owner,row['field_code'] if code is None else code,occurrence,relationship))

    def test_all_22_routes_require_exact_owner_kind_occurrence_and_edition(self):
        for row in self.routes:
            with self.subTest(kind=row['kind']):
                self.db.execute('SAVEPOINT route')
                self.seed_route(row)
                for owner,code,occurrence in ((11,None,0),(12,None,0),(10,'999' if row['field_code'].isdigit() else 'unknown',0),(10,None,1)):
                    with self.assertRaisesRegex(sqlite3.IntegrityError,'exact typed declaration'):
                        self.position(row,owner,code,occurrence)
                self.position(row)
                self.assertEqual(self.db.execute('SELECT * FROM routing_problems').fetchall(),[])
                # Reverse corruption must stay visible without a registry seed.
                # The real schema's FK guards reject this deletion; the thin
                # fixture bypasses that layer to test the independent audit.
                self.db.execute('DELETE FROM catalog_source_elements WHERE source_element_id=10')
                self.assertIn(('relationship_position_identity',1,1),
                              self.db.execute('SELECT * FROM routing_problems').fetchall())
                self.db.execute('INSERT INTO catalog_source_elements VALUES(10,1)')
                for sql in (f"UPDATE {row['declaration_table']} SET {row['declaration_owner']}=11",
                            f"DELETE FROM {row['declaration_table']}",
                            "UPDATE catalog_relationships SET edition_id=2",
                            "UPDATE reported_catalog_relationships SET reported_kind='not-the-kind'"):
                    with self.assertRaises(sqlite3.IntegrityError):
                        self.db.execute(sql)
                self.db.execute(f"DELETE FROM {row['position_table']}")
                self.assertEqual(self.db.execute("SELECT problem,owner_id,edition_id FROM routing_problems WHERE problem='relationship_position_count'").fetchall(),[('relationship_position_count',1,1)])
                self.db.execute('ROLLBACK TO route')
                self.db.execute('RELEASE route')

    def test_clone_markers_are_not_relationship_declarations(self):
        for row in (row for row in self.routes if row['marker_table']!='-'):
            with self.subTest(kind=row['kind']):
                db = self.db
                db.execute('SAVEPOINT marker')
                db.execute('INSERT INTO catalog_source_elements VALUES(10,1)')
                with self.assertRaisesRegex(sqlite3.IntegrityError,'typed P marker'):
                    self.position(row,relationship=None)
                db.execute(f"INSERT INTO {row['marker_table']} VALUES(10,'P')")
                self.position(row,relationship=None)
                self.assertEqual(db.execute('SELECT * FROM routing_problems').fetchall(),[])
                db.execute("INSERT INTO catalog_relationships VALUES(1,'source',1)")
                db.execute('INSERT INTO reported_catalog_relationships VALUES(1,?)',(row['kind'],))
                with self.assertRaisesRegex(sqlite3.IntegrityError,'mutually exclusive'):
                    db.execute(f"INSERT INTO {row['declaration_table']} VALUES(10,1)")
                db.execute(f"DELETE FROM {row['position_table']}")
                self.assertEqual(len(db.execute('SELECT * FROM routing_problems').fetchall()),1)
                db.execute('ROLLBACK TO marker')
                db.execute('RELEASE marker')


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

    def mame_root(self, edition_id=1):
        self.db.execute("""INSERT INTO mame_documents
            (edition_id,debug,debug_specified,mameconfig,source_line,source_column,
             extent_view,extent_start,extent_end,location_view,start_line,start_column,end_line,end_column,column_convention)
            VALUES(?,0,0,'',1,1,'retained_original_bytes',0,128,'transport_decoded_xml_text',1,1,2,1,'one_based_unicode_scalar')""", (edition_id,))
        self.db.execute("INSERT INTO mame_document_facts_attribute_positions VALUES(?,'mameconfig',0,0,1,7)", (edition_id,))

    def test_every_view_prepares_and_all_foreign_key_targets_exist(self):
        for (view,) in self.db.execute("SELECT name FROM sqlite_schema WHERE type='view'").fetchall():
            with self.subTest(view=view):
                self.db.execute(f'SELECT * FROM {assemble.identifier(view)} LIMIT 0')
        self.assertEqual(self.db.execute('PRAGMA foreign_key_check').fetchall(), [])

    def test_field_crosswalks_name_real_typed_value_and_position_owners(self):
        rows = assemble.field_coverage(self.db)
        self.assertTrue(rows)
        for row in rows:
            self.assertTrue(row['presence'])
            self.assertTrue(row['evidence'])

    def test_relationship_position_must_name_its_exact_declaration(self):
        db = self.db
        self.mame_root()
        db.execute("INSERT INTO catalog_set_groups VALUES(1,1,'root')")
        for owner in (10,11):
            db.execute("INSERT INTO catalog_source_elements VALUES(?,1,'mame_machine')", (owner,))
            db.execute("INSERT INTO catalog_sets VALUES(?,1,'machine',?,1,1)", (owner,owner))
            db.execute("INSERT INTO mame_machines VALUES(?,NULL,0,0,0,0,0,0,1,0)", (owner,))
        db.execute("INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES(1,'clone','source',1)")
        db.execute("INSERT INTO reported_catalog_relationships VALUES(1,'mame_cloneof')")
        db.execute("INSERT INTO mame_machine_links VALUES(10,'cloneof','',1)")
        for owner,code in ((11,'cloneof'), (10,'romof')):
            with self.subTest(owner=owner,code=code), self.assertRaises(sqlite3.IntegrityError):
                db.execute("INSERT INTO mame_machines_attribute_positions VALUES(?,?,0,1,0,1,1)", (owner,code))
        db.execute("INSERT INTO mame_machines_attribute_positions VALUES(10,'cloneof',0,1,0,1,1)")
        for view in ('candidate_relationship_declarations','candidate_relationship_positions'):
            result = db.execute(f'SELECT relationship_id,reported_kind,source_element_id,edition_id FROM {view} WHERE relationship_id=1').fetchall()
            self.assertEqual(result,[(1,'mame_cloneof',10,1)])
            plan = db.execute(f'EXPLAIN QUERY PLAN SELECT * FROM {view} WHERE relationship_id=1').fetchall()
            self.assertFalse(any('SCAN' in step[3] for step in plan),plan)
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("UPDATE mame_machine_links SET machine_id=11")
        db.execute("DELETE FROM mame_machines_attribute_positions WHERE relationship_id=1")
        self.assertEqual(db.execute("SELECT owner_id,edition_id FROM candidate_integrity_problems WHERE problem='relationship_position_count'").fetchall(), [(1,1)])

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

    def test_publication_rejects_missing_and_invented_native_field_positions(self):
        db = self.db
        self.mame_root()
        mutations = (
            "DELETE FROM mame_document_facts_attribute_positions WHERE field_kind='mameconfig'",
            "UPDATE mame_documents SET build=''",
            "INSERT INTO mame_document_facts_attribute_positions VALUES(1,'debug',0,1,1,20)",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                db.execute('SAVEPOINT field_attack')
                try:
                    db.execute(mutation)
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
                        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
                finally:
                    db.execute('ROLLBACK TO field_attack')
                    db.execute('RELEASE field_attack')

    def test_draft_presence_can_be_repaired_then_is_frozen_by_publication(self):
        db = self.db
        self.mame_root()
        db.execute("UPDATE mame_documents SET build='' WHERE edition_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
            db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        db.execute("INSERT INTO mame_document_facts_attribute_positions VALUES(1,'build',0,1,1,21)")
        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        for mutation in ("UPDATE mame_documents SET build=NULL WHERE edition_id=1",
                         "DELETE FROM mame_document_facts_attribute_positions WHERE field_kind='build'"):
            with self.subTest(mutation=mutation), self.assertRaisesRegex(sqlite3.IntegrityError, 'immutable'):
                db.execute(mutation)

    def test_bad_draft_in_another_edition_does_not_block_publication(self):
        db = self.db
        self.mame_root()
        db.execute("INSERT INTO catalog_source_files VALUES(2,?,NULL,128,'other','zstd')", (b't' * 32,))
        db.execute('INSERT INTO catalog_editions VALUES(2,1,2,1,1,NULL,NULL)')
        self.mame_root(2)
        db.execute("UPDATE mame_documents SET build='' WHERE edition_id=2")
        self.assertEqual(db.execute('SELECT * FROM candidate_field_presence_problems').fetchall(),
                         [('field_presence:mame_document_facts_attribute_positions:build',2,2)])
        db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
            db.execute("INSERT INTO published_catalog_editions VALUES(2,1,2,1,1,'published')")

    def test_contradictory_native_ancestry_blocks_both_known_editions(self):
        db = self.db
        self.mame_root()
        db.execute("INSERT INTO catalog_source_files VALUES(2,?,NULL,128,'other','zstd')", (b't' * 32,))
        db.execute('INSERT INTO catalog_editions VALUES(2,1,2,1,1,NULL,NULL)')
        self.mame_root(2)
        db.execute("INSERT INTO catalog_set_groups VALUES(2,2,'root')")
        db.execute("INSERT INTO catalog_source_elements VALUES(10,1,'mame_machine')")
        db.execute("INSERT INTO catalog_sets VALUES(10,2,'machine',0,1,1)")
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'kind or ancestry'):
            db.execute('INSERT INTO mame_machines VALUES(10,NULL,0,0,0,0,0,0,1,0)')
        # Independently audit pre-existing corruption, without weakening the guard.
        db.execute('DROP TRIGGER candidate_native_mame_machines_insert')
        db.execute('INSERT INTO mame_machines VALUES(10,NULL,0,0,0,0,0,0,1,0)')
        db.execute("INSERT INTO mame_machines_attribute_positions VALUES(10,'name',0,NULL,0,1,1)")
        for edition in (2,1):
            with self.subTest(edition=edition), self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
                db.execute("INSERT INTO published_catalog_editions VALUES(?,1,?,1,1,'published')", (edition,edition))
        self.assertEqual(db.execute("SELECT owner_id,edition_id FROM candidate_integrity_problems WHERE problem='owner_ancestry:mame_machines' ORDER BY edition_id").fetchall(),
                         [(10,1),(10,2)])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core', action='store_true', help='only shared-core witnesses; not assembled-schema evidence')
    args = parser.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SharedWitnesses)
    if not args.core:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(HashRoutingWitnesses))
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(RelationshipRoutingWitnesses))
        suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(AssembledWitnesses))
    outcome = unittest.TextTestRunner(verbosity=2).run(suite)
    raise SystemExit(not outcome.wasSuccessful())


if __name__ == '__main__':
    main()
