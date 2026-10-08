#!/usr/bin/env python3
"""Check No-Intro count mappings against independent constructed fixtures.

This validates candidate SQL routing only; it is not parser accumulation,
corpus coverage, or authentic DAT-o-MATIC P/C evidence.
"""

from pathlib import Path
from contextlib import closing
import sqlite3
import sys
import unittest
from types import SimpleNamespace

DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(DIRECTORY))

import no_intro_field_check
import assemble
import count_fixtures
import source_counts


# Independently transcribed from the named inserts in no_intro_field_witnesses.sql.
# These are keyed by counter name, never generated from inventory table metadata.
EXPECTED_TABLES = {
    ('no_intro_dat', 'header_count'): 'no_intro_dat_headers',
    ('no_intro_dat', 'header_text_child_count'): 'no_intro_dat_header_text_children',
    ('no_intro_dat', 'clrmamepro_element_count'): 'no_intro_dat_clrmamepro_options',
    ('no_intro_dat', 'romcenter_element_count'): 'no_intro_dat_romcenter_options',
    ('no_intro_dat', 'game_count'): 'no_intro_dat_games',
    ('no_intro_dat', 'game_description_count'): 'no_intro_dat_game_descriptions',
    ('no_intro_dat', 'category_count'): 'no_intro_dat_categories',
    ('no_intro_dat', 'identifier_count'): 'no_intro_dat_identifiers',
    ('no_intro_dat', 'release_count'): 'no_intro_dat_releases',
    ('no_intro_dat', 'rom_count'): 'no_intro_dat_rom_claims',
    ('no_intro_dat', 'game_attribute_position_count'): 'no_intro_dat_game_field_positions',
    ('no_intro_dat', 'release_attribute_position_count'): 'no_intro_dat_release_field_positions',
    ('no_intro_dat', 'clrmamepro_attribute_position_count'): 'no_intro_dat_clrmamepro_field_positions',
    ('no_intro_dat', 'romcenter_attribute_position_count'): 'no_intro_dat_romcenter_field_positions',
    ('no_intro_dat', 'rom_attribute_position_count'): 'no_intro_dat_rom_field_positions',
    ('no_intro_dat', 'document_xsi_attribute_count'): 'no_intro_dat_document_xsi_attributes',
    ('no_intro_dat', 'header_xsi_attribute_count'): 'no_intro_dat_header_xsi_attributes',
    ('no_intro_dat', 'header_text_child_xsi_attribute_count'): 'no_intro_dat_header_text_child_xsi_attributes',
    ('no_intro_dat', 'clrmamepro_xsi_attribute_count'): 'no_intro_dat_clrmamepro_xsi_attributes',
    ('no_intro_dat', 'romcenter_xsi_attribute_count'): 'no_intro_dat_romcenter_xsi_attributes',
    ('no_intro_dat', 'game_xsi_attribute_count'): 'no_intro_dat_game_xsi_attributes',
    ('no_intro_dat', 'game_description_xsi_attribute_count'): 'no_intro_dat_game_description_xsi_attributes',
    ('no_intro_dat', 'category_xsi_attribute_count'): 'no_intro_dat_category_xsi_attributes',
    ('no_intro_dat', 'identifier_xsi_attribute_count'): 'no_intro_dat_identifier_xsi_attributes',
    ('no_intro_dat', 'release_xsi_attribute_count'): 'no_intro_dat_release_xsi_attributes',
    ('no_intro_dat', 'rom_xsi_attribute_count'): 'no_intro_dat_rom_xsi_attributes',
    ('no_intro_pc_fixture', 'header_count'): 'no_intro_pc_headers',
    ('no_intro_pc_fixture', 'header_name_child_count'): 'no_intro_pc_header_names',
    ('no_intro_pc_fixture', 'header_description_child_count'): 'no_intro_pc_header_descriptions',
    ('no_intro_pc_fixture', 'header_version_child_count'): 'no_intro_pc_header_versions',
    ('no_intro_pc_fixture', 'game_count'): 'no_intro_pc_games',
    ('no_intro_pc_fixture', 'language_token_count'): 'no_intro_pc_languages',
    ('no_intro_pc_fixture', 'game_description_count'): 'no_intro_pc_game_descriptions',
    ('no_intro_pc_fixture', 'rom_count'): 'no_intro_pc_file_claims',
    ('no_intro_pc_fixture', 'game_attribute_position_count'): 'no_intro_pc_game_attribute_positions',
    ('no_intro_pc_fixture', 'rom_attribute_position_count'): 'no_intro_pc_rom_attribute_positions',
}

# Full literal vectors: every key is a counter name. 10100 and 10200 are the
# populated DAT/P-C fixture editions; 10400 is a second DAT edition for scope
# isolation and the required-header/no-game cardinality case.
EXPECTED_COUNTS = {
    ('no_intro_dat', 10100): {
        'header_count': 1, 'header_text_child_count': 12,
        'clrmamepro_element_count': 1, 'romcenter_element_count': 1,
        'game_count': 1, 'game_description_count': 1, 'category_count': 1,
        'identifier_count': 1, 'release_count': 1, 'rom_count': 1,
        'game_attribute_position_count': 4, 'release_attribute_position_count': 2,
        'clrmamepro_attribute_position_count': 2,
        'romcenter_attribute_position_count': 1, 'rom_attribute_position_count': 11,
        'document_xsi_attribute_count': 3, 'header_xsi_attribute_count': 3,
        'header_text_child_xsi_attribute_count': 48,
        'clrmamepro_xsi_attribute_count': 3, 'romcenter_xsi_attribute_count': 3,
        'game_xsi_attribute_count': 3, 'game_description_xsi_attribute_count': 4,
        'category_xsi_attribute_count': 4, 'identifier_xsi_attribute_count': 4,
        'release_xsi_attribute_count': 3, 'rom_xsi_attribute_count': 3,
    },
    ('no_intro_dat', 10400): {
        'header_count': 1, 'header_text_child_count': 4,
        'clrmamepro_element_count': 1, 'romcenter_element_count': 0,
        'game_count': 0, 'game_description_count': 0, 'category_count': 0,
        'identifier_count': 0, 'release_count': 0, 'rom_count': 0,
        'game_attribute_position_count': 0, 'release_attribute_position_count': 0,
        'clrmamepro_attribute_position_count': 0,
        'romcenter_attribute_position_count': 0, 'rom_attribute_position_count': 0,
        'document_xsi_attribute_count': 0, 'header_xsi_attribute_count': 0,
        'header_text_child_xsi_attribute_count': 0,
        'clrmamepro_xsi_attribute_count': 0, 'romcenter_xsi_attribute_count': 0,
        'game_xsi_attribute_count': 0, 'game_description_xsi_attribute_count': 0,
        'category_xsi_attribute_count': 0, 'identifier_xsi_attribute_count': 0,
        'release_xsi_attribute_count': 0, 'rom_xsi_attribute_count': 0,
    },
    ('no_intro_pc_fixture', 10200): {
        'header_count': 1, 'header_name_child_count': 1,
        'header_description_child_count': 1, 'header_version_child_count': 1,
        'game_count': 2, 'language_token_count': 3, 'game_description_count': 1,
        'rom_count': 3, 'game_attribute_position_count': 12,
        'rom_attribute_position_count': 7,
    },
}

NO_INTRO_FAMILIES = {'no_intro_dat', 'no_intro_pc_fixture'}
TARGET_EDITION = {'no_intro_dat': 10100, 'no_intro_pc_fixture': 10200}


def load_populated_fixture(connection):
    rows, positions = no_intro_field_check.validate_ledger(connection)
    fixture = DIRECTORY / 'no_intro_field_witnesses.sql'
    contents = fixture.read_text().replace('ROLLBACK TO no_intro_fields;\n', '')
    # Retain the fixture's constructed rows while reusing its SQL assertions,
    # expected-rejection checks, and position-coverage check unchanged.
    retained_fixture = SimpleNamespace(name=fixture.name, read_text=lambda: contents)
    no_intro_field_check.run_fixture(connection, retained_fixture, rows, positions)


class NoIntroCountMappings(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ddl = assemble.assemble()
        with closing(sqlite3.connect(':memory:')) as connection:
            connection.executescript(cls.ddl)
            view_count = connection.execute(
                "SELECT count(*) FROM sqlite_schema WHERE type='view'").fetchone()[0]
            if view_count != 48:
                raise AssertionError(f'expected 48 full-candidate views, found {view_count}')
            cls.routes = tuple(row for row in source_counts.inventory()
                               if row['family'] in NO_INTRO_FAMILIES)
        cls.routes_by_counter = {(row['family'], row['counter']): row for row in cls.routes}
        if set(cls.routes_by_counter) != set(EXPECTED_TABLES) or len(cls.routes_by_counter) != 36:
            raise AssertionError('No-Intro routes differ from the 36 literal expected counters')
        for (family, counter), table in EXPECTED_TABLES.items():
            actual_table = cls.routes_by_counter[(family, counter)]['table']
            if actual_table != table:
                raise AssertionError(f'{family}/{counter}: expected table {table}, got {actual_table}')
        for family, edition in EXPECTED_COUNTS:
            names = {row['counter'] for row in cls.routes if row['family'] == family}
            if set(EXPECTED_COUNTS[(family, edition)]) != names:
                raise AssertionError(f'{family}/{edition}: literal vector coverage mismatch')

    def populated_database(self):
        connection = sqlite3.connect(':memory:')
        connection.execute('PRAGMA foreign_keys=ON')
        connection.executescript(self.ddl)
        load_populated_fixture(connection)
        for (family, edition), vector in EXPECTED_COUNTS.items():
            count_fixtures.seal(connection, family, edition, vector)
        # Count-deletion controls deliberately bypass only the composed
        # in-memory FK reverse guards; the FK graph is never published.
        for (trigger,) in connection.execute(
                "SELECT name FROM sqlite_schema WHERE type='trigger' "
                "AND name LIKE 'candidate_fk_reverse_%'").fetchall():
            connection.execute('DROP TRIGGER "' + trigger.replace('"', '""') + '"')
        if connection.execute('SELECT count(*) FROM published_catalog_editions').fetchone()[0]:
            raise AssertionError('mapping controls require an unpublished fixture graph')
        connection.commit()
        # These deliberate prepublication erasure controls test mapping views,
        # not a publishable FK graph. No edition is inserted into the registry.
        connection.execute('PRAGMA foreign_keys=OFF')
        return connection

    def actual_count(self, connection, row, edition):
        return connection.execute(
            f'SELECT count(*) FROM {row["table"]} AS owner '
            f'WHERE ({row["scope_sql"]})=?', (edition,)).fetchone()[0]

    def problems(self, connection, edition):
        return connection.execute(
            'SELECT problem FROM candidate_source_count_problems '
            'WHERE edition_id=? ORDER BY problem', (edition,)).fetchall()

    def test_literal_fixture_vectors_match_all_36_independent_routes(self):
        connection = self.populated_database()
        self.addCleanup(connection.close)
        for (family, edition), vector in EXPECTED_COUNTS.items():
            for counter, expected in vector.items():
                with self.subTest(family=family, edition=edition, counter=counter):
                    row = self.routes_by_counter[(family, counter)]
                    self.assertEqual(row['family'], family)
                    self.assertEqual(self.actual_count(connection, row, edition), expected)
        self.assertEqual(connection.execute(
            'SELECT problem,edition_id FROM candidate_source_count_problems').fetchall(), [])

    def test_each_counter_maps_only_its_literal_table_and_edition(self):
        connection = self.populated_database()
        self.addCleanup(connection.close)
        for (family, counter), table in EXPECTED_TABLES.items():
            row = self.routes_by_counter[(family, counter)]
            edition = TARGET_EDITION[family]
            expected = EXPECTED_COUNTS[(family, edition)][counter]
            self.assertGreater(expected, 0, f'fixture must exercise {counter}')
            with self.subTest(counter=counter):
                connection.execute('SAVEPOINT route_control')
                deleted = connection.execute(
                    f'DELETE FROM {table} AS owner WHERE ({row["scope_sql"]})=?',
                    (edition,)).rowcount
                self.assertEqual(deleted, expected, f'{counter}: literal table/scope population')
                self.assertEqual(self.problems(connection, edition),
                                 [(f'source_count:{family}:{counter}',)])
                unrelated = [other for other in EXPECTED_COUNTS
                             if other[0] == family and other[1] != edition]
                for _, other_edition in unrelated:
                    self.assertEqual(self.problems(connection, other_edition), [])
                connection.execute(
                    f'UPDATE {family}_source_count_seals SET {counter}=0 WHERE edition_id=?',
                    (edition,))
                self.assertEqual(self.problems(connection, edition), [])
                connection.execute('ROLLBACK TO route_control')
                connection.execute('RELEASE route_control')

    def test_header_xsi_scope_uses_registry_then_typed_parent_and_never_guesses(self):
        row = self.routes_by_counter[('no_intro_dat', 'header_xsi_attribute_count')]
        expression = row['scope_sql']

        connection = self.populated_database()
        self.addCleanup(connection.close)
        # Payload missing, owner registry retained: the actual header-XSI rows
        # remain scoped even though the typed header row has gone.
        connection.execute('SAVEPOINT header_payload_missing')
        connection.execute('DELETE FROM no_intro_dat_headers WHERE header_id=11000')
        self.assertEqual(connection.execute(
            f'SELECT DISTINCT ({expression}) FROM {row["table"]} AS owner '
            'WHERE owner.header_id=11000').fetchall(), [(10100,)])
        self.assertEqual(self.problems(connection, 10100),
                         [('source_count:no_intro_dat:header_count',)])
        connection.execute('ROLLBACK TO header_payload_missing')
        connection.execute('RELEASE header_payload_missing')

        # Registry absent but typed parent/group retained: use the typed path.
        connection.execute('SAVEPOINT header_registry_missing')
        connection.execute('DELETE FROM catalog_source_elements WHERE source_element_id=11000')
        self.assertEqual(connection.execute(
            f'SELECT DISTINCT ({expression}) FROM {row["table"]} AS owner '
            'WHERE owner.header_id=11000').fetchall(), [(10100,)])
        self.assertEqual(self.problems(connection, 10100), [])
        connection.execute('ROLLBACK TO header_registry_missing')
        connection.execute('RELEASE header_registry_missing')

        # When both paths exist but disagree, preserve registry scope and let
        # the composed owner-ancestry audit report the contradictory graph.
        connection.execute('SAVEPOINT header_registry_conflict')
        connection.execute(
            'UPDATE catalog_source_elements SET edition_id=10400 '
            'WHERE source_element_id=11000')
        self.assertEqual(connection.execute(
            f'SELECT DISTINCT ({expression}) FROM {row["table"]} AS owner '
            'WHERE owner.header_id=11000').fetchall(), [(10400,)])
        ancestry = connection.execute(
            "SELECT 1 FROM candidate_integrity_problems "
            "WHERE problem='owner_ancestry:no_intro_dat_headers' "
            'AND edition_id IN (10100,10400) LIMIT 1').fetchone()
        self.assertIsNotNone(ancestry)
        connection.execute('ROLLBACK TO header_registry_conflict')
        connection.execute('RELEASE header_registry_conflict')

        # Neither registry nor parent survives: scope stays SQL NULL. The
        # mismatch remains attributable to the independently sealed edition;
        # FK diagnostics, not a guessed edition, describe the orphan graph.
        connection.execute('SAVEPOINT header_scope_unknown')
        connection.execute('DELETE FROM catalog_source_elements WHERE source_element_id=11000')
        connection.execute('DELETE FROM no_intro_dat_headers WHERE header_id=11000')
        self.assertEqual(connection.execute(
            f'SELECT DISTINCT ({expression}) FROM {row["table"]} AS owner '
            'WHERE owner.header_id=11000').fetchall(), [(None,)])
        self.assertEqual(self.problems(connection, 10100), [
            ('source_count:no_intro_dat:header_count',),
            ('source_count:no_intro_dat:header_xsi_attribute_count',),
        ])
        fk_tables = {check[0] for check in connection.execute('PRAGMA foreign_key_check')}
        self.assertIn('no_intro_dat_header_xsi_attributes', fk_tables)
        self.assertEqual(self.problems(connection, 10400), [])
        connection.execute('ROLLBACK TO header_scope_unknown')
        connection.execute('RELEASE header_scope_unknown')

    def test_language_token_rows_are_separate_from_language_position(self):
        connection = self.populated_database()
        self.addCleanup(connection.close)
        connection.execute('SAVEPOINT language_token_loss')
        connection.execute(
            'DELETE FROM no_intro_pc_languages WHERE set_id=12010 AND language_order=0')
        self.assertEqual(self.problems(connection, 10200), [
            ('source_count:no_intro_pc_fixture:language_token_count',)])
        self.assertEqual(connection.execute(
            "SELECT count(*) FROM no_intro_pc_game_attribute_positions "
            "WHERE source_element_id=12010 AND field_kind='languages'").fetchone()[0], 1)
        connection.execute(
            'UPDATE no_intro_pc_fixture_source_count_seals '
            'SET language_token_count=2 WHERE edition_id=10200')
        self.assertEqual(self.problems(connection, 10200), [])
        connection.execute('ROLLBACK TO language_token_loss')
        connection.execute('RELEASE language_token_loss')


if __name__ == '__main__':
    unittest.main()
