#!/usr/bin/env python3
"""Independent count-compiler controls; not parser accumulation/EOF evidence."""

import sqlite3
import unittest

import source_counts


def route(counter, table):
    return dict(family='mame', counter=counter, table=table, key='item_id',
                scope_sql='owner.edition_id', source_event=table + ' declaration')


class CountCompiler(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(':memory:')
        self.addCleanup(self.db.close)
        self.db.executescript('''
            CREATE TABLE mame_documents(edition_id INTEGER PRIMARY KEY);
            CREATE TABLE published_catalog_editions(edition_id INTEGER PRIMARY KEY);
            CREATE TABLE mame_alpha(item_id INTEGER PRIMARY KEY, edition_id INTEGER NOT NULL REFERENCES mame_documents(edition_id));
            CREATE TABLE mame_beta(item_id INTEGER PRIMARY KEY, edition_id INTEGER NOT NULL REFERENCES mame_documents(edition_id));
            INSERT INTO mame_documents VALUES(1),(2);
            INSERT INTO mame_alpha VALUES(10,1),(20,2);
            INSERT INTO mame_beta VALUES(11,1);
        ''')
        self.routes = (route('alpha_count', 'mame_alpha'), route('beta_count', 'mame_beta'))

    def install(self, routes=None):
        self.db.executescript(source_counts.fragment(self.db, self.routes if routes is None else routes))

    def publish(self):
        self.db.execute('INSERT INTO published_catalog_editions VALUES(1)')

    def test_missing_seal_refuses_publication_then_literal_counts_publish(self):
        self.install()
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'source count'):
            self.publish()
        self.db.execute('INSERT INTO mame_source_count_seals VALUES(1,1,1)')
        self.publish()  # Edition 2's missing seal does not block edition 1.

    def test_each_counter_and_equal_sized_native_mapping_are_independent(self):
        self.install()
        self.db.execute('INSERT INTO mame_source_count_seals VALUES(1,1,1)')
        for counter in ('alpha_count', 'beta_count'):
            for bad in (0, 2):
                with self.subTest(counter=counter, bad=bad):
                    self.db.execute(f'UPDATE mame_source_count_seals SET {counter}=? WHERE edition_id=1', (bad,))
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'source count'):
                        self.publish()
                    self.db.execute(f'UPDATE mame_source_count_seals SET {counter}=1 WHERE edition_id=1')
        # Both start at one: a swapped COUNT body must not pass this control.
        self.db.execute('DELETE FROM mame_alpha WHERE edition_id=1')
        self.assertEqual(self.db.execute('SELECT problem FROM candidate_source_count_problems WHERE edition_id=1').fetchall(),
                         [('source_count:mame:alpha_count',)])
        self.db.execute('UPDATE mame_source_count_seals SET alpha_count=0 WHERE edition_id=1')
        self.publish()

    def test_counts_are_exact_nonnegative_integers(self):
        self.install()
        for value in (-1, None, 0.5, 'invalid'):
            with self.subTest(value=value), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute('INSERT INTO mame_source_count_seals VALUES(1,?,1)', (value,))

    def test_published_seal_cannot_be_changed_deleted_replaced_or_redirected(self):
        self.install()
        self.db.execute('INSERT INTO mame_source_count_seals VALUES(1,1,1)')
        self.db.execute('INSERT INTO mame_source_count_seals VALUES(2,1,0)')
        self.publish()
        for sql in (
            'UPDATE mame_source_count_seals SET alpha_count=0 WHERE edition_id=1',
            'DELETE FROM mame_source_count_seals WHERE edition_id=1',
            'INSERT OR REPLACE INTO mame_source_count_seals VALUES(1,1,1)',
            'UPDATE OR REPLACE mame_source_count_seals SET edition_id=1 WHERE edition_id=2',
        ):
            with self.subTest(sql=sql), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(sql)

    def test_foreign_keys_off_does_not_allow_orphan_or_reverse_root_rewrite(self):
        self.install()
        self.db.execute('INSERT INTO mame_source_count_seals VALUES(1,1,1)')
        for sql in (
            'INSERT INTO mame_source_count_seals VALUES(9,0,0)',
            'DELETE FROM mame_documents WHERE edition_id=1',
            'UPDATE mame_documents SET edition_id=9 WHERE edition_id=1',
        ):
            with self.subTest(sql=sql), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(sql)

    def test_bad_inventory_never_emits_a_partial_contract(self):
        for change in (
            dict(counter='edition_id'), dict(table='absent'), dict(key='absent'),
            dict(scope_sql='owner.absent'), dict(scope_sql='owner.edition_id; SELECT 1'),
            dict(family='invented'), dict(source_event=''),
        ):
            with self.subTest(change=change):
                bad = self.routes[0] | change
                with self.assertRaises((ValueError, sqlite3.Error)):
                    source_counts.fragment(self.db, (bad,))
        for routes in ((), (self.routes[0], self.routes[0]),
                       (self.routes[0], self.routes[1] | dict(counter='alpha_count')),
                       (self.routes[0], self.routes[0] | dict(counter='duplicate_count'))):
            with self.subTest(routes=routes), self.assertRaises(ValueError):
                source_counts.fragment(self.db, routes)

    def test_compilation_is_schema_only_even_inside_a_populated_write_transaction(self):
        self.db.execute('UPDATE mame_alpha SET item_id=12 WHERE item_id=10')
        self.assertTrue(self.db.in_transaction)
        schema = self.db.execute('SELECT name,sql FROM sqlite_schema ORDER BY name').fetchall()
        sql = source_counts.fragment(self.db, self.routes)
        self.assertIn('candidate_source_count_publication', sql)
        self.assertEqual(self.db.execute('SELECT name,sql FROM sqlite_schema ORDER BY name').fetchall(), schema)
        self.assertTrue(self.db.in_transaction)

    def test_guard_subset_is_closed_and_preserves_the_selected_tables_guards(self):
        import assemble
        all_guards, _ = assemble.foreign_key_guards(self.db)
        selected, _ = assemble.foreign_key_guards(self.db, tables=('mame_alpha',))
        self.assertEqual(len(selected), 4)
        self.assertTrue(set(selected) <= set(all_guards))
        collisions = assemble.collision_guards(self.db, tables=('mame_alpha',))
        immutable = assemble.published_fact_guards(self.db, (), tables=('mame_alpha',))
        self.assertEqual(len(collisions), 2)
        self.assertEqual(len(immutable), 3)
        self.assertTrue(set(collisions) <= set(assemble.collision_guards(self.db)))
        self.assertTrue(set(immutable) <= set(assemble.published_fact_guards(self.db, ())))
        for invalid in (('missing',), ('mame_alpha', 'mame_alpha')):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                assemble.foreign_key_guards(self.db, tables=invalid)


class ComposedSourceCounts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.addClassCleanup(cls.db.close)
        cls.db.executescript(source_counts.candidate())
        cls.routes = source_counts.inventory()
        import check
        check.seed_identity(cls.db)
        fixture = check.AssembledWitnesses()
        fixture.db = cls.db
        fixture.complete_mame_root()
        cls.db.commit()

    def setUp(self):
        self.db.execute('SAVEPOINT source_counts')

    def tearDown(self):
        self.db.execute('ROLLBACK TO source_counts')
        self.db.execute('RELEASE source_counts')

    def seal(self):
        # Independent description of the four fixture events. Do not SELECT
        # expectations from retained rows or from the query under test.
        events = {'machine_count': 1, 'machine_text_element_count': 1,
                  'document_attribute_position_count': 1,
                  'machine_attribute_position_count': 1}
        names = [row['counter'] for row in self.routes if row['family'] == 'mame']
        self.assertTrue(set(events) <= set(names))
        columns = ','.join(names)
        self.db.execute(f'INSERT INTO mame_source_count_seals(edition_id,{columns}) '
                        f'VALUES({",".join("?" for _ in range(len(names)+1))})',
                        (1, *(events.get(name, 0) for name in names)))

    def publish(self):
        self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'source-counts')")

    def test_actual_candidate_requires_literal_seal_and_preserves_other_publication_checks(self):
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'source count'):
            self.publish()
        self.seal()
        self.assertEqual(self.db.execute('SELECT * FROM candidate_source_count_problems WHERE edition_id=1').fetchall(), [])
        self.db.execute("UPDATE mame_documents SET build='' WHERE edition_id=1")
        self.assertEqual(self.db.execute('SELECT * FROM candidate_source_count_problems WHERE edition_id=1').fetchall(), [])
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'complete closure'):
            self.publish()
        self.db.execute('UPDATE mame_documents SET build=NULL WHERE edition_id=1')
        self.publish()
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'immutable'):
            self.db.execute('UPDATE mame_source_count_seals SET machine_count=0')

    def test_every_mame_expected_counter_independently_blocks_actual_publication(self):
        self.seal()
        for row in self.routes:
            if row['family'] != 'mame':
                continue
            name = row['counter']
            with self.subTest(counter=name):
                self.db.execute(f'UPDATE mame_source_count_seals SET {name}={name}+1 WHERE edition_id=1')
                with self.assertRaisesRegex(sqlite3.IntegrityError, 'source count'):
                    self.publish()
                self.db.execute(f'UPDATE mame_source_count_seals SET {name}={name}-1 WHERE edition_id=1')
        self.publish()

    def test_whole_optional_field_and_position_erasure_leaves_source_count_error(self):
        self.db.execute("UPDATE mame_documents SET build='' WHERE edition_id=1")
        self.db.execute("INSERT INTO mame_document_facts_attribute_positions VALUES(1,'build',0,1,1,21)")
        self.seal()
        self.db.execute('UPDATE mame_source_count_seals SET document_attribute_position_count=2 WHERE edition_id=1')
        self.db.execute("DELETE FROM mame_document_facts_attribute_positions WHERE field_kind='build'")
        self.db.execute('UPDATE mame_documents SET build=NULL WHERE edition_id=1')
        self.assertEqual(self.db.execute('SELECT * FROM candidate_integrity_problems WHERE edition_id=1').fetchall(), [])
        self.assertEqual(self.db.execute('SELECT problem FROM candidate_source_count_problems WHERE edition_id=1').fetchall(),
                         [('source_count:mame:document_attribute_position_count',)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'source count'):
            self.publish()

    def test_missing_inventory_route_is_a_compile_error_not_a_silent_missing_check(self):
        for removed in (self.routes[0], self.routes[-1]):
            with self.subTest(removed=removed['table']), self.assertRaisesRegex(ValueError, 'coverage mismatch'):
                source_counts.validate_inventory(self.db, tuple(row for row in self.routes if row != removed))


if __name__ == '__main__':
    unittest.main()
