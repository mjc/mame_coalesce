#!/usr/bin/env python3
"""Compile the proposed independent source-count contract over candidate SQL.

This next design layer is not yet part of assemble.py's ordinary fixture path.
--emit emits the entire base candidate plus mandatory source-count publication
checks. Expected values must come from checked parser events, never this audit.
"""

import argparse
import csv
from collections import defaultdict
from contextlib import closing
import sqlite3
import sys

import assemble


ROOTS = {
    'mame': 'mame_documents', 'software': 'software_documents',
    'logiqx': 'logiqx_documents', 'clrmamepro': 'clrmamepro_documents',
    'no_intro_dat': 'no_intro_dat_documents',
    'no_intro_pc_fixture': 'no_intro_pc_documents',
}
HEADER = ('family', 'counter', 'table', 'key', 'scope_sql', 'source_event')
MAME_SWITCH_POSITIONS = {
    'machine_switch_conditions_attribute_positions',
    'machine_switch_locations_attribute_positions',
    'machine_switch_value_conditions_attribute_positions',
    'machine_switch_values_attribute_positions',
    'machine_switches_attribute_positions',
}


def inventory():
    routes = []
    for family in assemble.FAMILIES:
        with (assemble.ROOT / f'{family}-counts.tsv').open(newline='') as source:
            reader = csv.DictReader(source, delimiter='\t')
            if tuple(reader.fieldnames or ()) != HEADER:
                raise ValueError(f'{family}: count inventory columns must be {HEADER}')
            routes.extend(reader)
    if {row['family'] for row in routes} != set(ROOTS):
        raise ValueError('count inventory must cover all six non-export reading families')
    return tuple(routes)


def validate(connection, routes):
    """Fail closed before emitting any DDL; expressions are closed build inputs."""
    if not routes:
        raise ValueError('source count inventory must not be empty')
    names, relations = set(), set()
    for row in routes:
        if set(row) != set(HEADER) or any(not row[key] for key in HEADER):
            raise ValueError('incomplete source count route')
        family = row['family']
        if family not in ROOTS:
            raise ValueError(f'unknown count family {family}')
        for key in ('counter', 'table'):
            assemble.identifier(row[key])
        keys = row['key'].split(',')
        for key in keys:
            assemble.identifier(key)
        if len(keys) != len(set(keys)):
            raise ValueError('duplicate source count key component')
        if not row['counter'].endswith('_count') or row['counter'] == 'edition_id':
            raise ValueError('source counters need a named _count column')
        if any(character in row['scope_sql'] for character in ';\n\r'):
            raise ValueError('source count scope must be one SQL expression')
        prefix = ROOTS[family].removesuffix('documents')
        if not row['table'].startswith(prefix) and not (
                family == 'mame' and row['table'] in MAME_SWITCH_POSITIONS):
            raise ValueError('count relation belongs to a different native family')
        if connection.execute("SELECT type FROM sqlite_schema WHERE name=?", (row['table'],)).fetchone() != ('table',):
            raise ValueError('source counts require physical native tables')
        info = assemble.table_columns(connection, row['table'])
        if not set(keys) <= {column[1] for column in info if column[5]}:
            raise ValueError('source count key must be part of the real primary key')
        if (family, row['counter']) in names or (family, row['table']) in relations:
            raise ValueError('duplicate source counter or physical relation')
        names.add((family, row['counter']))
        relations.add((family, row['table']))
        connection.execute(f"SELECT ({row['scope_sql']}) FROM "
                           f"{assemble.identifier(row['table'])} AS owner LIMIT 0")
        assemble.table_columns(connection, ROOTS[family])


def validate_inventory(connection, routes):
    """Cross-check count coverage against independent owner/field inventories."""
    validate(connection, routes)
    exempt = {'clrmamepro_headers', 'clrmamepro_comments',
              'software_list_wrapper_entries'}
    required = {(assemble.kind_family(owner.kind), owner.table)
                for owner in assemble.owners()
                if assemble.kind_family(owner.kind) in ROOTS and owner.table not in exempt}
    # The MAME field ledger's family column names tag contexts, not formats.
    # Enumerate physical position/XSI tables independently of that ledger so
    # omitting both a field route and its counter cannot hide a stored table.
    for table in assemble.schema_tables(connection):
        if not table.endswith(('_positions', '_xsi_attributes')):
            continue
        if table in MAME_SWITCH_POSITIONS:
            required.add(('mame', table))
        else:
            for family, root in ROOTS.items():
                if table.startswith(root.removesuffix('documents')):
                    required.add((family, table))
    required.add(('software', 'software_lists'))
    # Tokens are canonical values of one P/C languages declaration, not new
    # wire attributes. A token may disappear while its position remains.
    required.add(('no_intro_pc_fixture', 'no_intro_pc_languages'))
    actual = {(row['family'], row['table']) for row in routes}
    if actual != required:
        raise ValueError(f'source count coverage mismatch: missing={sorted(required-actual)}, '
                         f'extra={sorted(actual-required)}')


def fragment(connection, routes):
    """Return concrete typed seals, audits and guards for the selected contracts."""
    routes = tuple(routes)
    validate(connection, routes)
    grouped = defaultdict(list)
    for row in routes:
        grouped[row['family']].append(row)
    definitions, audits, tables = [], [], []
    for family, members in sorted(grouped.items()):
        table = family + '_source_count_seals'
        tables.append(table)
        quote = assemble.identifier(table)
        root = assemble.identifier(ROOTS[family])
        columns = ','.join(f'{assemble.identifier(row["counter"])} INTEGER NOT NULL '
                           f'CHECK({assemble.identifier(row["counter"])}>=0)' for row in members)
        definitions.append(f'CREATE TABLE {quote}(edition_id INTEGER PRIMARY KEY '
                           f'REFERENCES {root}(edition_id),{columns}) STRICT, WITHOUT ROWID;')
        audits.append(f"SELECT {assemble.literal('source_count_seal_missing:' + family)} AS problem,"
                      f'root.edition_id AS owner_id,root.edition_id FROM {root} AS root '
                      f'LEFT JOIN {quote} AS seal USING(edition_id) WHERE seal.edition_id IS NULL')
        for row in members:
            actual = (f'SELECT count(*) FROM {assemble.identifier(row["table"])} AS owner '
                      f'WHERE ({row["scope_sql"]})=root.edition_id')
            audits.append(f"SELECT {assemble.literal('source_count:' + family + ':' + row['counter'])},"
                          f'root.edition_id,root.edition_id FROM {root} AS root '
                          f'JOIN {quote} AS seal USING(edition_id) '
                          f'WHERE seal.{assemble.identifier(row["counter"])} IS NOT ({actual})')
    # Introspect the extension without mutating the caller's transaction/schema.
    with closing(sqlite3.connect(':memory:')) as compiler:
        # Copy definitions only. Backing up a caller's open write transaction
        # can wait forever, and audit generation has no reason to copy data.
        for (sql,) in connection.execute("SELECT sql FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'"):
            compiler.execute(sql)
        compiler.executescript('\n'.join(definitions))
        fk_guards, fk_audits = assemble.foreign_key_guards(compiler, tables=tables)
        guards = [*fk_guards, *assemble.collision_guards(compiler, tables=tables),
                  *assemble.published_fact_guards(compiler, (), tables=tables)]
    audits.extend(fk_audits)
    chunks = [audits[offset:offset + 100] for offset in range(0, len(audits), 100)]
    views = [f'CREATE VIEW candidate_source_count_chunk_{number} AS ' +
             ' UNION ALL '.join(chunk) + ';' for number, chunk in enumerate(chunks)]
    views.append('CREATE VIEW candidate_source_count_problems AS ' + ' UNION ALL '.join(
        f'SELECT * FROM candidate_source_count_chunk_{number}' for number in range(len(chunks))) + ';')
    gate = '''CREATE TRIGGER candidate_source_count_publication
        BEFORE INSERT ON published_catalog_editions
        WHEN EXISTS(SELECT 1 FROM candidate_source_count_problems WHERE edition_id=NEW.edition_id)
        BEGIN SELECT RAISE(ABORT,'candidate publication requires independent source counts'); END;'''
    return '\n'.join([*definitions, *views, *guards, gate])


def candidate():
    base = assemble.assemble()
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.executescript(base)
        routes = inventory()
        validate_inventory(connection, routes)
        extension = fragment(connection, routes)
    return base + '\n' + extension


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--emit', action='store_true')
    args = parser.parse_args()
    sql = candidate()
    if args.emit:
        sys.stdout.write(sql + '\n')
        return
    with closing(sqlite3.connect(':memory:')) as connection:
        connection.executescript(sql)
        views = connection.execute("SELECT name FROM sqlite_schema WHERE type='view'").fetchall()
        for (view,) in views:
            connection.execute(f'SELECT * FROM {assemble.identifier(view)} LIMIT 0')
        if connection.execute('PRAGMA foreign_key_check').fetchall():
            raise RuntimeError('source count candidate FK failure')
        print(f'Source-count candidate prepares {len(views)} views; empty-schema check only.')


if __name__ == '__main__':
    main()
