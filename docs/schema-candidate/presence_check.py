#!/usr/bin/env python3
"""Independent generator attacks; native family checks prove their own predicates."""

import sqlite3
import unittest
from contextlib import closing

import assemble


def route(code, predicate):
    return dict(owner_table='native_values', owner_key='owner_id',
                position_table='native_positions', position_owner='owner_id',
                field_code=str(code), present_sql=predicate)


MANIFEST = (assemble.NativeOwner('test_native', 'native_values', 'owner_id',
                                'catalog_source_elements', 'owner_id',
                                'source_element_id', '-', False),)
ROUTES = (route(0, '1'), route(1, 'owner.optional IS NOT NULL'),
          route(2, 'owner.specified=1'))


class PresenceGenerator(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(':memory:')
        cls.db.executescript('''
            CREATE TABLE catalog_source_elements(source_element_id INTEGER PRIMARY KEY,edition_id INTEGER NOT NULL);
            CREATE INDEX edition_lookup ON catalog_source_elements(edition_id,source_element_id);
            CREATE TABLE native_values(owner_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
                optional TEXT,specified INTEGER NOT NULL);
            CREATE TABLE native_positions(owner_id INTEGER REFERENCES native_values(owner_id),
                field_kind INTEGER NOT NULL,field_occurrence INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(owner_id,field_kind,field_occurrence));
        ''')
        views, _ = assemble.field_presence_sql(cls.db, MANIFEST, ROUTES)
        cls.db.executescript(views)

    @classmethod
    def tearDownClass(cls):
        cls.db.close()

    def setUp(self):
        self.db.execute('SAVEPOINT witness')
        self.db.executemany('INSERT INTO catalog_source_elements VALUES(?,?)', ((1,7),(2,8)))
        self.db.executemany('INSERT INTO native_values VALUES(?,NULL,0)', ((1,),(2,)))
        self.db.executemany('INSERT INTO native_positions VALUES(?,0,0)', ((1,),(2,)))

    def tearDown(self):
        self.db.execute('ROLLBACK TO witness')
        self.db.execute('RELEASE witness')

    def problems(self):
        return self.db.execute('SELECT * FROM candidate_field_presence_problems ORDER BY problem,owner_id').fetchall()

    def test_required_optional_empty_and_explicit_default(self):
        self.assertEqual(self.problems(), [])
        self.db.execute("UPDATE native_values SET optional='',specified=1 WHERE owner_id=1")
        self.assertEqual(self.problems(), [
            ('field_presence:native_positions:1',1,7),
            ('field_presence:native_positions:2',1,7)])
        self.db.executemany('INSERT INTO native_positions VALUES(1,?,0)', ((1,),(2,)))
        self.assertEqual(self.problems(), [])
        self.db.execute('DELETE FROM native_positions WHERE owner_id=1 AND field_kind=0')
        self.assertEqual(self.problems(), [('field_presence:native_positions:0',1,7)])

    def test_invented_default_position_and_nonboolean_predicates(self):
        self.db.execute('INSERT INTO native_positions VALUES(1,2,0)')
        self.assertEqual(self.problems(), [('field_presence:native_positions:2',1,7)])
        self.db.execute('DELETE FROM native_positions WHERE field_kind=2')
        # A malformed build predicate may not turn NULL/2 into absent/true.
        for predicate in ('NULL', '2'):
            with self.subTest(predicate=predicate):
                views, _ = assemble.field_presence_sql(self.db, MANIFEST, (route(0, predicate),))
                with closing(sqlite3.connect(':memory:')) as db:
                    db.executescript('''
                        CREATE TABLE catalog_source_elements(source_element_id INTEGER PRIMARY KEY,edition_id INTEGER);
                        CREATE TABLE native_values(owner_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements,optional TEXT,specified INTEGER);
                        CREATE TABLE native_positions(owner_id INTEGER REFERENCES native_values,field_kind INTEGER,field_occurrence INTEGER);
                        INSERT INTO catalog_source_elements VALUES(1,7);
                        INSERT INTO native_values VALUES(1,NULL,0);
                        INSERT INTO native_positions VALUES(1,0,0);
                    ''' + views)
                    self.assertEqual(db.execute('SELECT * FROM candidate_field_presence_problems').fetchall(),
                                     [('field_presence:native_positions:0',1,7)])

    def test_equal_count_owner_substitution_does_not_hide_two_mismatches(self):
        self.db.execute("UPDATE native_values SET optional='' WHERE owner_id=1")
        self.db.execute('INSERT INTO native_positions VALUES(1,1,0)')
        self.assertEqual(self.problems(), [])
        self.db.execute('UPDATE native_positions SET owner_id=2 WHERE field_kind=1')
        self.assertEqual(self.problems(), [('field_presence:native_positions:1',1,7),
                                          ('field_presence:native_positions:1',2,8)])

    def test_extra_and_nonzero_occurrences_are_not_canonical_presence(self):
        self.db.execute('UPDATE native_positions SET field_occurrence=1 WHERE owner_id=1')
        self.assertEqual(self.problems(), [('field_presence:native_positions:0',1,7)])
        self.db.execute('INSERT INTO native_positions VALUES(1,0,0)')
        self.assertEqual(self.problems(), [('field_presence:native_positions:0',1,7)])

    def test_detached_native_owner_remains_visible_with_unknown_edition(self):
        self.db.execute('DELETE FROM native_positions WHERE owner_id=1')
        self.db.execute('DELETE FROM catalog_source_elements WHERE source_element_id=1')
        self.assertEqual(self.problems(), [('field_presence:native_positions:0',1,None)])

    def test_edition_filter_uses_registry_and_owner_indexes_not_history_scan(self):
        plan = self.db.execute('EXPLAIN QUERY PLAN SELECT * FROM candidate_field_presence_problems WHERE edition_id=7').fetchall()
        self.assertFalse(any('SCAN' in step[3] for step in plan), plan)

    def test_payload_scope_joins_native_parent_and_keeps_detached_facet(self):
        db = self.db
        db.execute('CREATE TABLE native_facets(owner_id INTEGER PRIMARY KEY REFERENCES native_values(owner_id))')
        db.execute('INSERT INTO native_facets VALUES(1)')
        scope, joins = assemble.field_presence_scope(db, 'native_facets',
                                                   {owner.table:owner.id for owner in MANIFEST})
        query = f'SELECT owner.owner_id,{scope} AS edition_id FROM native_facets AS owner{joins}'
        self.assertEqual(db.execute(query).fetchall(), [(1,7)])
        plan = db.execute('EXPLAIN QUERY PLAN ' + query + ' WHERE field_scope.edition_id=7').fetchall()
        self.assertFalse(any('SCAN' in step[3] for step in plan), plan)
        db.execute('DELETE FROM catalog_source_elements WHERE source_element_id=1')
        self.assertEqual(db.execute(query).fetchall(), [(1,None)])
        db.execute('DELETE FROM native_values WHERE owner_id=1')
        self.assertEqual(db.execute(query).fetchall(), [(1,None)])

    def test_routes_require_real_typed_fk_unique_codes_and_valid_expressions(self):
        bad = ({**ROUTES[0], 'owner_key':'specified'},
               {**ROUTES[0], 'field_code': 'not-integer'},
               {**ROUTES[0], 'present_sql': 'owner.missing IS NOT NULL'},
               {**ROUTES[0], 'present_sql': '1; DELETE FROM native_values'})
        for entry in bad:
            with self.subTest(entry=entry), self.assertRaises((ValueError, sqlite3.Error)):
                assemble.field_presence_sql(self.db, MANIFEST, (entry,))
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            assemble.field_presence_sql(self.db, MANIFEST, (ROUTES[0],ROUTES[0]))
        with self.assertRaisesRegex(ValueError, 'empty'):
            assemble.field_presence_sql(self.db, MANIFEST, ())
        self.db.execute('''CREATE TABLE composite_positions(owner_id INTEGER,edition_id INTEGER,
            field_kind INTEGER,field_occurrence INTEGER,
            FOREIGN KEY(owner_id,edition_id) REFERENCES native_values(owner_id,specified))''')
        with self.assertRaisesRegex(ValueError, 'actual typed owner FK'):
            assemble.field_presence_sql(self.db, MANIFEST,
                                        ({**ROUTES[0],'position_table':'composite_positions'},))

    def test_large_closed_inventory_is_chunked_below_sqlite_union_limit(self):
        routes = tuple(route(code, '0') for code in range(601))
        views, _ = assemble.field_presence_sql(self.db, MANIFEST, routes)
        with closing(sqlite3.connect(':memory:')) as db:
            db.executescript('''
                CREATE TABLE catalog_source_elements(source_element_id INTEGER PRIMARY KEY,edition_id INTEGER);
                CREATE TABLE native_values(owner_id INTEGER PRIMARY KEY,optional TEXT,specified INTEGER);
                CREATE TABLE native_positions(owner_id INTEGER,field_kind INTEGER,field_occurrence INTEGER);
            ''' + views)
            self.assertEqual(db.execute('SELECT * FROM candidate_field_presence_problems').fetchall(), [])

    def test_only_typed_inline_xsi_key_can_omit_explicit_occurrence(self):
        with closing(sqlite3.connect(':memory:')) as db:
            db.executescript('''
                CREATE TABLE roots(edition_id INTEGER PRIMARY KEY);
                CREATE TABLE root_xsi_attributes(edition_id INTEGER REFERENCES roots(edition_id),
                    field_kind TEXT NOT NULL,value_text TEXT NOT NULL,
                    PRIMARY KEY(edition_id,field_kind));
                INSERT INTO roots VALUES(7);
                INSERT INTO root_xsi_attributes VALUES(7,'nil','false');
            ''')
            entry = dict(owner_table='roots',owner_key='edition_id',
                         position_table='root_xsi_attributes',position_owner='edition_id',
                         field_code='nil',present_sql="EXISTS(SELECT 1 FROM root_xsi_attributes AS field WHERE field.edition_id=owner.edition_id AND field.field_kind='nil')")
            views, _ = assemble.field_presence_sql(db, (), (entry,))
            db.executescript(views)
            self.assertEqual(db.execute('SELECT * FROM candidate_field_presence_problems').fetchall(), [])
            # This structural check does not interpret the lexical nil value.
            db.execute("UPDATE root_xsi_attributes SET value_text='uninterpreted'")
            self.assertEqual(db.execute('SELECT * FROM candidate_field_presence_problems').fetchall(), [])
            db.execute('CREATE TABLE ordinary_positions(edition_id INTEGER REFERENCES roots(edition_id),field_kind TEXT,PRIMARY KEY(edition_id,field_kind))')
            with self.assertRaisesRegex(ValueError, 'implicit occurrence'):
                assemble.field_presence_sql(db, (), ({**entry,'position_table':'ordinary_positions'},))
            db.execute('CREATE TABLE bad_xsi_attributes(edition_id INTEGER REFERENCES roots(edition_id),field_kind TEXT,other INTEGER,PRIMARY KEY(edition_id,field_kind,other))')
            with self.assertRaisesRegex(ValueError, 'implicit occurrence'):
                assemble.field_presence_sql(db, (), ({**entry,'position_table':'bad_xsi_attributes'},))


if __name__ == '__main__':
    unittest.main(verbosity=2)
