#!/usr/bin/env python3
"""Assemble the isolated schema proposal; never open an application database."""

import argparse
import csv
from collections import defaultdict
from contextlib import closing
from dataclasses import dataclass
from pathlib import Path
import re
import sqlite3
import sys


ROOT = Path(__file__).resolve().parent
FAMILIES = ("mame", "software", "logiqx_cmp", "no_intro")
FRAGMENTS = ("shared.sql", *(f"{family}.sql" for family in FAMILIES),
             "relationships.sql", "relationships_guards.sql", "diagnostics.sql",
             *(f"{family}_cardinality.sql" for family in FAMILIES))
IDENTIFIER = re.compile(r"[A-Za-z_][A-Za-z_0-9]*\Z")
ROOT_LINKS = (
    ('mame_root_import_messages', 'mame_documents', 'mame', None),
    ('logiqx_datafile_import_messages', 'logiqx_documents', 'logiqx', None),
    ('clrmamepro_document_import_messages', 'clrmamepro_documents', 'clrmamepro', None),
    ('software_document_import_messages', 'software_documents', 'software', None),
    ('software_list_root_import_messages', 'software_lists', 'software', 'set_group_id'),
    ('software_wrapper_import_messages', 'software_wrapper_headers', 'software', 'wrapper_id'),
    ('no_intro_dat_root_import_messages', 'no_intro_dat_documents', 'no_intro_dat', None),
    ('no_intro_export_document_import_messages', 'no_intro_export_documents', 'no_intro_database', None),
    ('no_intro_export_datafile_import_messages', 'no_intro_export_datafiles', 'no_intro_database', None),
    ('no_intro_export_header_import_messages', 'no_intro_export_headers', 'no_intro_database', 'header_id'),
    ('no_intro_pc_root_import_messages', 'no_intro_pc_documents', 'no_intro_pc_fixture', None),
)


def identifier(value):
    if not IDENTIFIER.fullmatch(value):
        raise ValueError(f"not a SQL identifier: {value!r}")
    return f'"{value}"'


def literal(value):
    return "'" + value.replace("'", "''") + "'"


def kind_family(kind):
    for prefix, family in (('mame_', 'mame'), ('software_', 'software'), ('logiqx_', 'logiqx'),
                           ('clrmamepro_', 'clrmamepro'), ('no_intro_dat_', 'no_intro_dat'),
                           ('no_intro_pc_', 'no_intro_pc_fixture'), ('no_intro_export_', 'no_intro_database')):
        if kind.startswith(prefix):
            return family
    raise ValueError(f'no supported reading-rule family for native kind {kind}')


@dataclass(frozen=True)
class NativeOwner:
    kind: str
    table: str
    id: str
    parent_table: str
    parent_column: str
    parent_key: str
    sequence: str
    media: bool


def owners():
    result = []
    for family in FAMILIES:
        with (ROOT / f"{family}-owners.tsv").open(newline="") as source:
            reader = csv.DictReader(source, delimiter="\t")
            expected = ("kind", "table", "id", "parent_table", "parent_column", "parent_key", "sequence", "media")
            if tuple(reader.fieldnames or ()) != expected:
                raise ValueError(f"{family}: owner manifest columns must be {expected}")
            for row in reader:
                row["sequence"] = row["sequence"] or "-"
                if row["media"] not in ("0", "1"):
                    raise ValueError(f"{family}: media is not 0/1: {row}")
                for key in expected[:-1]:
                    if key != "kind" and (key != "sequence" or row[key] != "-"):
                        identifier(row[key])
                result.append(NativeOwner(**{**row, "media": row["media"] == "1"}))
    kinds = [owner.kind for owner in result]
    if not result or len(kinds) != len(set(kinds)):
        raise ValueError("native kinds must be nonempty and have exactly one owner")
    return tuple(result)


def table_columns(connection, table):
    rows = connection.execute(f"PRAGMA table_info({identifier(table)})").fetchall()
    if not rows:
        raise ValueError(f"missing candidate table {table}")
    return rows


def schema_tables(connection, tables=None):
    """Validate an explicit extension's tables, or visit the whole schema."""
    known = {row[0] for row in connection.execute(
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'")}
    selected = tuple(sorted(known)) if tables is None else tuple(tables)
    if len(selected) != len(set(selected)):
        raise ValueError('duplicate guard table')
    for table in selected:
        identifier(table)
        if table not in known:
            raise ValueError(f'missing guard table {table}')
    return selected


def edition_expression(connection, table, alias, native_keys=None, seen=()):
    """Resolve an immediate parent's edition without storing copied ancestry."""
    if table in seen:
        raise ValueError(f"cyclic edition path at {table}")
    columns = {row[1] for row in table_columns(connection, table)}
    if "edition_id" in columns:
        return f"{alias}.edition_id"
    if native_keys and table in native_keys:
        return f"(SELECT edition_id FROM catalog_source_elements WHERE source_element_id={alias}.{identifier(native_keys[table])})"
    if table == "catalog_sets":
        return f"(SELECT edition_id FROM catalog_set_groups WHERE set_group_id={alias}.set_group_id)"
    if table == "catalog_media_entries":
        return f"(SELECT edition_id FROM catalog_source_elements WHERE source_element_id={alias}.media_entry_id)"
    references = connection.execute(f"PRAGMA foreign_key_list({identifier(table)})").fetchall()
    registry_keys = [row[3] for row in references if row[2] == "catalog_source_elements" and row[4] == "source_element_id"]
    if len(registry_keys) == 1:
        return f"(SELECT edition_id FROM catalog_source_elements WHERE source_element_id={alias}.{identifier(registry_keys[0])})"
    groups = [row[3] for row in references if row[2] == "catalog_set_groups" and row[4] == "set_group_id"]
    if len(groups) == 1:
        return f"(SELECT edition_id FROM catalog_set_groups WHERE set_group_id={alias}.{identifier(groups[0])})"
    paths = defaultdict(list)
    for row in references:
        paths[row[0]].append(row)
    for rows in sorted(paths.values(), key=lambda rows: rows[0][2] not in (native_keys or {})):
        target = rows[0][2]
        if target in (*seen, table):
            continue
        target_alias = 'scope_' + str(len(seen))
        try:
            scope = edition_expression(connection, target, target_alias, native_keys, (*seen, table))
        except ValueError:
            continue
        match = ' AND '.join(f'{target_alias}.{identifier(row[4])}={alias}.{identifier(row[3])}' for row in rows)
        return f'(SELECT {scope} FROM {identifier(target)} AS {target_alias} WHERE {match})'
    raise ValueError(f"no unambiguous immediate edition path for {table}")


def foreign_key_guards(connection, manifest=(), *, tables=None):
    """Emit concrete SQL for every FK, including reverse checks with FKs off."""
    guards, problems = [], []
    for table in schema_tables(connection, tables):
        grouped = defaultdict(list)
        for row in connection.execute(f"PRAGMA foreign_key_list({identifier(table)})"):
            grouped[row[0]].append(row)
        columns = {row[1] for row in table_columns(connection, table)}
        try:
            edition = edition_expression(connection, table, "child", {owner.table: owner.id for owner in manifest})
        except ValueError:
            edition = "NULL"
        for number, rows in grouped.items():
            rows.sort(key=lambda row: row[1])
            parent = rows[0][2]
            pairs = [(row[3], row[4]) for row in rows]
            if any(target is None for _, target in pairs):
                raise ValueError(f"{table}: implicit FK targets must be explicit")
            present = " AND ".join(f"NEW.{identifier(child)} IS NOT NULL" for child, _ in pairs)
            match = " AND ".join(f"parent.{identifier(target)}=NEW.{identifier(child)}" for child, target in pairs)
            condition = f"({present}) AND NOT EXISTS(SELECT 1 FROM {identifier(parent)} AS parent WHERE {match})"
            for operation in ("INSERT", "UPDATE"):
                guards.append(f"CREATE TRIGGER {identifier(f'candidate_fk_{table}_{number}_{operation.lower()}')} BEFORE {operation} ON {identifier(table)} WHEN {condition} BEGIN SELECT RAISE(ABORT,'candidate foreign key closure'); END;")
            reverse = " AND ".join(f"child.{identifier(child)}=OLD.{identifier(target)}" for child, target in pairs)
            changes = " OR ".join(f"NEW.{identifier(target)} IS NOT OLD.{identifier(target)}" for _, target in pairs)
            exists = f"EXISTS(SELECT 1 FROM {identifier(table)} AS child WHERE {reverse})"
            for operation, when in (("DELETE", exists), ("UPDATE", f"({changes}) AND {exists}")):
                guards.append(f"CREATE TRIGGER {identifier(f'candidate_fk_reverse_{table}_{number}_{operation.lower()}')} BEFORE {operation} ON {identifier(parent)} WHEN {when} BEGIN SELECT RAISE(ABORT,'candidate referenced owner is immutable'); END;")
            audit_present = " AND ".join(f"child.{identifier(child)} IS NOT NULL" for child, _ in pairs)
            audit_match = " AND ".join(f"parent.{identifier(target)}=child.{identifier(child)}" for child, target in pairs)
            primary = sorted((row for row in table_columns(connection, table) if row[5]), key=lambda row: row[5])
            owner = f"child.{identifier(primary[0][1])}" if primary else "NULL"
            # owner_id is presentation only: UUID and composite-key rows stay typed in their tables.
            problems.append(f"SELECT {literal('foreign_key:' + table + ':' + str(number))} AS problem,{owner} AS owner_id,{edition} AS edition_id FROM {identifier(table)} AS child WHERE {audit_present} AND NOT EXISTS(SELECT 1 FROM {identifier(parent)} AS parent WHERE {audit_match})")
    return guards, problems


def collision_guards(connection, *, tables=None):
    """Protect PK and alternate-key REPLACE without recursive DELETE triggers.

    Candidate callers deduplicate batches with set-based NOT EXISTS before INSERT.
    Blind INSERT OR IGNORE/UPSERT is intentionally not a supported replacement API.
    """
    result = []
    for table in schema_tables(connection, tables):
        info = table_columns(connection, table)
        primary = tuple(row[1] for row in sorted(info, key=lambda row: row[5]) if row[5])
        keys = [(primary, None)] if primary else []
        for index in connection.execute(f"PRAGMA index_list({identifier(table)})").fetchall():
            if not index[2]:
                continue
            key = tuple(row[2] for row in connection.execute(f"PRAGMA index_info({identifier(index[1])})"))
            if any(column is None for column in key):
                raise ValueError(f"{table}: expression unique index needs explicit collision policy")
            predicate = None
            if index[4]:
                sql = connection.execute("SELECT sql FROM sqlite_schema WHERE name=?", (index[1],)).fetchone()[0]
                predicate = re.split(r"\bWHERE\b", sql, maxsplit=1, flags=re.IGNORECASE)[1]
            if (key, predicate) not in keys:
                keys.append((key, predicate))
        collisions, update_collisions = [], []
        for key, predicate in keys:
            match = " AND ".join(f"existing.{identifier(column)}=NEW.{identifier(column)}" for column in key)
            test = f"EXISTS(SELECT 1 FROM {identifier(table)} AS existing WHERE {match}"
            if predicate:
                new_row = ",".join(f"NEW.{identifier(row[1])} AS {identifier(row[1])}" for row in info)
                test += f" AND ({predicate})) AND (SELECT ({predicate}) FROM (SELECT {new_row}))"
            else:
                test += ")"
            collisions.append(f"({test})")
            old_identity = ' AND '.join(f'existing.{identifier(column)} IS OLD.{identifier(column)}' for column in primary)
            if not old_identity:
                raise ValueError(f'{table}: collision checks require a primary key')
            update_collisions.append('(' + test.replace(' WHERE ', f' WHERE NOT ({old_identity}) AND ', 1) + ')')
        if collisions:
            result.append(f"CREATE TRIGGER {identifier('candidate_collision_' + table)} BEFORE INSERT ON {identifier(table)} WHEN {' OR '.join(collisions)} BEGIN SELECT RAISE(ABORT,'candidate identity collision: deduplicate before insert'); END;")
            result.append(f"CREATE TRIGGER {identifier('candidate_update_collision_' + table)} BEFORE UPDATE ON {identifier(table)} WHEN {' OR '.join(update_collisions)} BEGIN SELECT RAISE(ABORT,'candidate update identity collision'); END;")
    return result


def published_fact_guards(connection, manifest, *, tables=None):
    """Native facets and positions are immutable too, not just manifest owners."""
    result = []
    native_keys = {owner.table: owner.id for owner in manifest}
    excluded = {'published_catalog_editions', 'catalog_imports', 'catalog_import_messages',
                'catalog_import_message_elements', 'catalog_import_message_external_evidence'}
    for table in schema_tables(connection, tables):
        if table in excluded or 'import_messages' in table or table.startswith('catalog_relationship'):
            continue
        if any(token in table for token in ('review', 'decision', 'conflict')) or table == 'merged_file_ids':
            continue
        try:
            scope = edition_expression(connection, table, 'owner', native_keys)
        except ValueError:
            continue
        primary = sorted((row for row in table_columns(connection, table) if row[5]), key=lambda row: row[5])
        if not primary:
            raise ValueError(f'{table}: an immutable fact needs a primary key')
        for operation, aliases in (('INSERT', ('NEW',)), ('UPDATE', ('OLD','NEW')), ('DELETE', ('OLD',))):
            # Resolve from the actual row/parent. On INSERT use the proposed row,
            # so new positions cannot be appended to an already published owner.
            conditions = []
            for alias in aliases:
                proposed = ','.join(f'{alias}.{identifier(row[1])} AS {identifier(row[1])}' for row in table_columns(connection, table))
                conditions.append(f'EXISTS(SELECT 1 FROM (SELECT {proposed}) AS owner JOIN published_catalog_editions AS publication ON publication.edition_id={scope})')
            condition = ' OR '.join(conditions)
            result.append(f"CREATE TRIGGER {identifier('candidate_immutable_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN {condition} BEGIN SELECT RAISE(ABORT,'published candidate payload and positions are immutable'); END;")
    return result


def immutable_dictionary_sql():
    tables = ('catalog_source_files', 'catalog_reading_rules', 'catalog_xml_repairs',
              'file_id_registries', 'shared_catalog_files', 'catalog_decoded_xml_views')
    return [f"CREATE TRIGGER {identifier('candidate_dictionary_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} BEGIN SELECT RAISE(ABORT,'source/rules/issued identity facts are immutable'); END;" for table in tables for operation in ('UPDATE', 'DELETE')]


def root_diagnostic_sql():
    tables, guards, links, problems = [], [], [], []
    for table, owner, family, extra in ROOT_LINKS:
        extra_column = f'{identifier(extra)} INTEGER NOT NULL,' if extra else ''
        if extra == 'set_group_id':
            owner_fk = 'FOREIGN KEY(set_group_id) REFERENCES software_lists(set_group_id), FOREIGN KEY(set_group_id,edition_id) REFERENCES catalog_set_groups(set_group_id,edition_id)'
            owner_join = 'JOIN software_lists AS owner ON owner.set_group_id=link.set_group_id JOIN catalog_set_groups AS owner_group ON owner_group.set_group_id=owner.set_group_id AND owner_group.edition_id=link.edition_id'
            mode = "AND EXISTS(SELECT 1 FROM software_documents WHERE edition_id=link.edition_id AND envelope_kind='single_list')"
        elif extra:
            owner_fk = f'FOREIGN KEY({identifier(extra)},edition_id) REFERENCES {identifier(owner)}({identifier(extra)},edition_id)'
            owner_join = f'JOIN {identifier(owner)} AS owner ON owner.{identifier(extra)}=link.{identifier(extra)} AND owner.edition_id=link.edition_id'
            mode = "AND EXISTS(SELECT 1 FROM software_documents WHERE edition_id=link.edition_id AND envelope_kind='plural_lists')" if extra == 'wrapper_id' else ''
        else:
            owner_fk = f'FOREIGN KEY(edition_id) REFERENCES {identifier(owner)}(edition_id)'
            owner_join = f'JOIN {identifier(owner)} AS owner ON owner.edition_id=link.edition_id'
            mode = ''
        tables.append(f"CREATE TABLE {identifier(table)}(message_id INTEGER NOT NULL,edition_id INTEGER NOT NULL,{extra_column}role TEXT NOT NULL CHECK(role IN ('primary','related')),PRIMARY KEY(message_id,edition_id,role),FOREIGN KEY(message_id,edition_id) REFERENCES catalog_import_messages(message_id,edition_id),{owner_fk}) STRICT, WITHOUT ROWID;")
        byte_start = "CASE WHEN message.source_view=owner.extent_view AND message.source_problem_start IS NOT NULL THEN message.source_problem_start WHEN owner.extent_view='retained_original_bytes' THEN message.original_problem_start END"
        byte_end = "CASE WHEN message.source_view=owner.extent_view AND message.source_problem_end IS NOT NULL THEN message.source_problem_end WHEN owner.extent_view='retained_original_bytes' THEN message.original_problem_end END"
        byte_available = f'(owner.extent_view IS NOT NULL AND ({byte_start}) IS NOT NULL)'
        source_length = "CASE owner.extent_view WHEN 'retained_original_bytes' THEN source.byte_length WHEN 'transport_decoded_xml_bytes' THEN decoded.byte_length END"
        byte_within = f'(owner.extent_start>=0 AND owner.extent_start<owner.extent_end AND owner.extent_end<=({source_length}) AND owner.extent_start<=({byte_start}) AND ({byte_start})<owner.extent_end AND ({byte_start})<=({byte_end}) AND ({byte_end})<=owner.extent_end)'
        coord_available = '(message.line IS NOT NULL AND message.column IS NOT NULL AND owner.start_line IS NOT NULL AND owner.start_column IS NOT NULL AND owner.end_line IS NOT NULL AND owner.end_column IS NOT NULL AND message.location_view=owner.location_view AND message.column_convention=owner.column_convention)'
        coord_within = '((message.line>owner.start_line OR (message.line=owner.start_line AND message.column>=owner.start_column)) AND (message.line<owner.end_line OR (message.line=owner.end_line AND message.column<owner.end_column)))'
        proof = f'(({byte_available}) IS TRUE OR ({coord_available}) IS TRUE) AND (({byte_available}) IS NOT TRUE OR ({byte_within}) IS TRUE) AND (({coord_available}) IS NOT TRUE OR ({coord_within}) IS TRUE)'
        valid = f"SELECT 1 FROM {identifier(table)} AS link JOIN catalog_import_messages AS message ON message.message_id=link.message_id AND message.edition_id=link.edition_id JOIN catalog_imports AS run ON run.import_id=message.import_id AND run.edition_id=link.edition_id JOIN catalog_editions AS edition ON edition.edition_id=link.edition_id AND edition.catalog_id=run.catalog_id AND edition.source_file_id=message.source_file_id AND edition.reading_rules_id=message.reading_rules_id JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id AND rules.format_family={literal(family)} JOIN catalog_source_files AS source ON source.source_file_id=edition.source_file_id LEFT JOIN catalog_decoded_xml_views AS decoded ON decoded.source_file_id=source.source_file_id {owner_join} WHERE link.message_id=PROPOSED.message_id AND link.edition_id=PROPOSED.edition_id AND link.role=PROPOSED.role {mode} AND {proof}"
        # The proposal is tested without temporarily inserting the real link.
        columns = ['message_id', 'edition_id', *([extra] if extra else []), 'role']
        proposed = ','.join(f'NEW.{identifier(column)} AS {identifier(column)}' for column in columns)
        select_new = valid.replace(f'FROM {identifier(table)} AS link', f'FROM (SELECT {proposed}) AS link').replace('PROPOSED.', 'NEW.')
        for operation in ('INSERT', 'UPDATE'):
            guards.append(f"CREATE TRIGGER {identifier('candidate_root_link_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN NOT EXISTS({select_new}) BEGIN SELECT RAISE(ABORT,'root diagnostic requires matching ancestry, mode and proven containment'); END;")
        problems.append(f"SELECT {literal('root_message:' + table)} AS problem,proposed.message_id AS owner_id,proposed.edition_id FROM {identifier(table)} AS proposed WHERE NOT EXISTS({valid.replace('PROPOSED.', 'proposed.')})")
        links.append(f"SELECT message_id,edition_id FROM {identifier(table)} WHERE role='primary'")
    links.append("SELECT message_id,edition_id FROM catalog_import_message_elements WHERE role='primary'")
    primary = ' UNION ALL '.join(links)
    problems.append(f"SELECT 'message_primary_count',message_id,edition_id FROM ({primary}) GROUP BY message_id HAVING count(*)>1")
    primary_tables = [table for table, *_ in ROOT_LINKS] + ['catalog_import_message_elements']
    for table in primary_tables:
        for operation in ('INSERT', 'UPDATE'):
            branches = []
            for other in primary_tables:
                exclude = ''
                if operation == 'UPDATE' and other == table:
                    exclude = ' AND NOT(message_id=OLD.message_id AND edition_id=OLD.edition_id AND role=OLD.role'
                    if table == 'catalog_import_message_elements':
                        exclude += ' AND source_element_id=OLD.source_element_id'
                    exclude += ')'
                branches.append(f"SELECT 1 FROM {identifier(other)} WHERE message_id=NEW.message_id AND role='primary'{exclude}")
            guards.append(f"CREATE TRIGGER {identifier('candidate_primary_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN NEW.role='primary' AND EXISTS({' UNION ALL '.join(branches)}) BEGIN SELECT RAISE(ABORT,'a diagnostic has at most one primary owner'); END;")
    return '\n'.join(tables), guards, problems


def hash_routes():
    rows = []
    for family in FAMILIES:
        with (ROOT / f'{family}-hash-positions.tsv').open(newline='') as source:
            rows.extend(csv.DictReader(source, delimiter='\t'))
    return tuple(rows)


def hash_position_sql(connection):
    branches, policies, guards = [], defaultdict(list), []
    for row in hash_routes():
        table, owner, code, occurrence = (identifier(row[key]) for key in ('table', 'owner_column', 'code_column', 'occurrence_column'))
        field_code = field_code_literal(connection,row['table'],row['code_column'],row['field_code'])
        source_field = literal(row['source_hash_field'])
        role = row.get('role_constraint', '-')
        if role not in ('-', 'Value'):
            raise ValueError(f'unknown canonical hash position role {role}')
        role_check = ' AND value_line IS NOT NULL AND value_column IS NOT NULL' if role == 'Value' else ''
        branches.append(f'SELECT reported_hash_id,{owner} AS media_entry_id,{source_field} AS source_hash_field,{occurrence} AS field_occurrence FROM {table} WHERE {code}={field_code} AND reported_hash_id IS NOT NULL{role_check}')
        new_role = ' AND NEW.value_line IS NOT NULL AND NEW.value_column IS NOT NULL' if role == 'Value' else ''
        policies[row['table']].append(f'(NEW.{code}={field_code} AND hash.media_entry_id=NEW.{owner} AND hash.source_hash_field={source_field} AND hash.field_occurrence=NEW.{occurrence}{new_role})')
    view = 'CREATE VIEW candidate_canonical_hash_positions AS ' + ' UNION ALL '.join(branches) + ';'
    for table, choices in policies.items():
        valid = f"SELECT 1 FROM catalog_entry_hashes AS hash WHERE hash.reported_hash_id=NEW.reported_hash_id AND ({' OR '.join(choices)})"
        for operation in ('INSERT', 'UPDATE'):
            guards.append(f"CREATE TRIGGER {identifier('candidate_hash_position_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN NEW.reported_hash_id IS NOT NULL AND NOT EXISTS({valid}) BEGIN SELECT RAISE(ABORT,'hash position must reference its exact native declaration'); END;")
    problems = [
        "SELECT 'hash_position_count' AS problem,hash.reported_hash_id AS owner_id,element.edition_id FROM catalog_entry_hashes AS hash LEFT JOIN catalog_source_elements AS element ON element.source_element_id=hash.media_entry_id LEFT JOIN candidate_canonical_hash_positions AS position USING(reported_hash_id) GROUP BY hash.reported_hash_id HAVING count(position.reported_hash_id)<>1",
        "SELECT 'hash_position_identity',position.reported_hash_id,element.edition_id FROM candidate_canonical_hash_positions AS position LEFT JOIN catalog_entry_hashes AS hash USING(reported_hash_id) LEFT JOIN catalog_source_elements AS element ON element.source_element_id=position.media_entry_id WHERE hash.media_entry_id IS NOT position.media_entry_id OR hash.source_hash_field IS NOT position.source_hash_field OR hash.field_occurrence IS NOT position.field_occurrence",
    ]
    return view, guards, problems


def field_code_literal(connection,table,column,value):
    """NEW fields in triggers do not inherit column comparison affinity."""
    kind = next((row[2] for row in table_columns(connection,table) if row[1]==column),None)
    if kind == 'INTEGER':
        if not re.fullmatch(r'0|[1-9][0-9]*',value):
            raise ValueError(f'{table}.{column}: expected an integer field code, got {value!r}')
        return value
    if kind == 'TEXT':
        return literal(value)
    raise ValueError(f'{table}.{column}: unsupported field-code type {kind}')


def field_coverage(connection):
    """Validate the independent field crosswalk's references, not its completeness."""
    entries, seen = [], set()
    expected = ('family','owner_table','field_code','wire_name','value_owner',
                'value_column','position_table','presence','default_rule','evidence')
    for family in FAMILIES:
        with (ROOT / f'{family}-field-coverage.tsv').open(newline='') as source:
            reader = csv.DictReader(source,delimiter='\t')
            if tuple(reader.fieldnames or ()) != expected:
                raise ValueError(f'{family}: field crosswalk columns must be {expected}')
            for row in reader:
                if None in row or any(not row[column] for column in expected):
                    raise ValueError(f'{family}: incomplete field crosswalk row {row}')
                key = (row['family'],row['owner_table'],row['wire_name'])
                if key in seen:
                    raise ValueError(f'duplicate field crosswalk entry {key}')
                seen.add(key)
                table_columns(connection,row['owner_table'])
                columns = {column[1] for column in connection.execute(f"PRAGMA table_xinfo({identifier(row['value_owner'])})")}
                requested = set(row['value_column'].split(','))
                if not requested <= columns:
                    raise ValueError(f"{key}: nonexistent canonical value column {requested-columns}")
                if row['position_table'] != '-':
                    position_columns = {column[1] for column in table_columns(connection,row['position_table'])}
                    if 'field_kind' in position_columns:
                        field_code_literal(connection,row['position_table'],'field_kind',row['field_code'])
                entries.append(row)
    return tuple(entries)


def field_presence_routes(families=FAMILIES):
    """Closed native predicates are build inputs, never stored catalog fields."""
    expected = ('owner_table', 'owner_key', 'position_table', 'position_owner',
                'field_code', 'present_sql')
    routes, seen = [], set()
    for family in families:
        if family not in FAMILIES:
            raise ValueError(f'unknown native family {family}')
        with (ROOT / f'{family}-field-presence.tsv').open(newline='') as source:
            reader = csv.DictReader(source, delimiter='\t')
            if tuple(reader.fieldnames or ()) != expected:
                raise ValueError(f'{family}: presence columns must be {expected}')
            for row in reader:
                if None in row or any(not row[column] for column in expected):
                    raise ValueError(f'{family}: incomplete presence route {row}')
                for column in expected[:4]:
                    identifier(row[column])
                if any(character in row['present_sql'] for character in ';\n\r'):
                    raise ValueError('presence predicates must be single SQL expressions')
                key = (row['position_table'], row['field_code'])
                if key in seen:
                    raise ValueError(f'duplicate presence route {key}')
                seen.add(key)
                routes.append(row)
    if not routes:
        raise ValueError('native presence routes must not be empty')
    return tuple(routes)


def field_presence_scope(connection, table, native_keys):
    """Join known edition paths so edition publication can use scope indexes."""
    columns = {column[1] for column in table_columns(connection, table)}
    if 'edition_id' in columns:
        return 'owner.edition_id', ''
    if table in native_keys:
        return ('field_scope.edition_id',
                f' LEFT JOIN catalog_source_elements AS field_scope ON '
                f'field_scope.source_element_id=owner.{identifier(native_keys[table])}')
    if table == 'catalog_sets':
        return ('field_scope.edition_id',
                ' LEFT JOIN catalog_set_groups AS field_scope USING(set_group_id)')
    group_keys = [row[3] for row in connection.execute(f'PRAGMA foreign_key_list({identifier(table)})')
                  if row[2] == 'catalog_set_groups' and row[4] == 'set_group_id']
    if len(group_keys) == 1:
        return ('field_scope.edition_id',
                f' LEFT JOIN catalog_set_groups AS field_scope ON '
                f'field_scope.set_group_id=owner.{identifier(group_keys[0])}')
    references = defaultdict(list)
    for entry in connection.execute(f'PRAGMA foreign_key_list({identifier(table)})'):
        references[entry[0]].append(entry)
    native_parents = [rows[0] for rows in references.values() if len(rows) == 1
                      and rows[0][2] in native_keys and rows[0][4] == native_keys[rows[0][2]]]
    if len(native_parents) == 1:
        parent = native_parents[0]
        # A native parent's PK is the issued source-element ID. Resolve that
        # actual FK directly: chained LEFT JOINs force an owner/history scan.
        # Missing parent payload is independently diagnosed by FK/owner audits;
        # its surviving registry identity still gives a truthful edition.
        return ('field_scope.edition_id',
                f' LEFT JOIN catalog_source_elements AS field_scope ON '
                f'field_scope.source_element_id=owner.{identifier(parent[3])}')
    return edition_expression(connection, table, 'owner', native_keys), ''


def field_presence_sql(connection, manifest, routes=None):
    """Retained native values/default flags and exact positions must agree."""
    routes = field_presence_routes() if routes is None else routes
    native_keys = {owner.table: owner.id for owner in manifest}
    branches, seen = [], set()
    for row in routes:
        table, key, position, position_owner = (row[column] for column in
                ('owner_table', 'owner_key', 'position_table', 'position_owner'))
        route_key = (position, row['field_code'])
        if route_key in seen:
            raise ValueError(f'duplicate presence route {route_key}')
        seen.add(route_key)
        owner_info = table_columns(connection, table)
        owner_columns = {column[1] for column in owner_info}
        position_info = table_columns(connection, position)
        position_columns = {column[1] for column in position_info}
        if key not in owner_columns or not {position_owner, 'field_kind'} <= position_columns:
            raise ValueError(f'{route_key}: missing native owner or canonical position columns')
        if 'field_occurrence' not in position_columns:
            primary = {column[1] for column in position_info if column[5]}
            if not position.endswith('_xsi_attributes') or primary != {position_owner, 'field_kind'}:
                raise ValueError(f'{route_key}: implicit occurrence needs the typed XSI singleton key')
        parents = defaultdict(list)
        for entry in connection.execute(f'PRAGMA foreign_key_list({identifier(position)})'):
            parents[entry[0]].append((entry[2], entry[3], entry[4]))
        if [(table, position_owner, key)] not in parents.values():
            raise ValueError(f'{route_key}: presence route must use the actual typed owner FK')
        if {column[1] for column in owner_info if column[5]} != {key}:
            raise ValueError(f'{route_key}: presence owner needs its complete primary key')
        predicate = row['present_sql']
        if not predicate or any(character in predicate for character in ';\n\r'):
            raise ValueError('presence predicates must be single SQL expressions')
        connection.execute(f'SELECT ({predicate}) FROM {identifier(table)} AS owner LIMIT 0')
        code = field_code_literal(connection, position, 'field_kind', row['field_code'])
        match = (f'position.{identifier(position_owner)}=owner.{identifier(key)} '
                 f'AND position.field_kind={code}')
        scope, join = field_presence_scope(connection, table, native_keys)
        extra_occurrence = (f' OR EXISTS(SELECT 1 FROM {identifier(position)} AS position '
                            f'WHERE {match} AND position.field_occurrence<>0)'
                            if 'field_occurrence' in position_columns else '')
        branches.append(
            f"SELECT {literal('field_presence:' + position + ':' + row['field_code'])} AS problem,"
            f'owner.{identifier(key)} AS owner_id,{scope} AS edition_id '
            f'FROM {identifier(table)} AS owner{join} '
            f'WHERE ({predicate}) IS NOT (SELECT count(*) FROM {identifier(position)} AS position WHERE {match})'
            f'{extra_occurrence}')
    if not branches:
        raise ValueError('native presence routes must not be empty')
    chunks = [branches[offset:offset + 100] for offset in range(0, len(branches), 100)]
    views = [f'CREATE VIEW candidate_field_presence_chunk_{number} AS ' + ' UNION ALL '.join(chunk) + ';'
             for number, chunk in enumerate(chunks)]
    views.append('CREATE VIEW candidate_field_presence_problems AS ' + ' UNION ALL '.join(
        f'SELECT * FROM candidate_field_presence_chunk_{number}' for number in range(len(chunks))) + ';')
    return '\n'.join(views), ['SELECT problem,owner_id,edition_id FROM candidate_field_presence_problems']


def relationship_routes():
    """Closed DOC-23 source-field routes; never persisted as catalog data."""
    with (ROOT / 'relationship-positions.tsv').open(newline='') as source:
        reader = csv.DictReader(source, delimiter='\t')
        expected = ('kind', 'declaration_table', 'declaration_owner', 'link_kind',
                    'position_table', 'position_owner', 'field_code', 'marker_table')
        if tuple(reader.fieldnames or ()) != expected:
            raise ValueError(f'relationship manifest columns must be {expected}')
        routes = tuple(reader)
    kinds = [row['kind'] for row in routes]
    if len(kinds) != 22 or len(set(kinds)) != 22:
        raise ValueError('DOC-23 requires exactly 22 unique reported relationship routes')
    for row in routes:
        for key in ('declaration_table','declaration_owner','position_table','position_owner'):
            identifier(row[key])
        if row['marker_table'] != '-':
            identifier(row['marker_table'])
    return routes


def relationship_position_sql(connection):
    """Close typed declarations, canonical positions and reported identity both ways."""
    routes = relationship_routes()
    declarations, positions, guards, problems = [], [], [], []
    by_position, by_declaration = defaultdict(list), defaultdict(list)
    reported_sql = connection.execute("SELECT sql FROM sqlite_schema WHERE name='reported_catalog_relationships'").fetchone()[0]
    listed = re.search(r'reported_kind\s+IN\s*\((.*?)\)', reported_sql, re.S).group(1)
    if set(re.findall(r"'([^']+)'", listed)) != {row['kind'] for row in routes}:
        raise ValueError('reported registry and DOC-23 relationship routes disagree')
    for row in routes:
        table, owner = identifier(row['declaration_table']), identifier(row['declaration_owner'])
        position, position_owner = identifier(row['position_table']), identifier(row['position_owner'])
        for name, columns in ((row['declaration_table'], (row['declaration_owner'],'relationship_id')),
                              (row['position_table'], (row['position_owner'],'field_kind','field_occurrence','relationship_id'))):
            if not set(columns) <= {column[1] for column in table_columns(connection,name)}:
                raise ValueError(f'relationship route does not match {name}')
        selected = '' if row['link_kind']=='-' else f" WHERE declaration.link_kind={literal(row['link_kind'])}"
        kind = literal(row['kind'])
        declarations.append(f'SELECT declaration.relationship_id,{kind} AS reported_kind,declaration.{owner} AS source_element_id,element.edition_id FROM {table} AS declaration LEFT JOIN catalog_source_elements AS element ON element.source_element_id=declaration.{owner}{selected}')
        by_position[row['position_table']].append(row)
        by_declaration[row['declaration_table']].append(row)
    for name, members in by_position.items():
        owner = identifier(members[0]['position_owner'])
        if any(row['position_owner'] != members[0]['position_owner'] for row in members):
            raise ValueError(f'{name}: inconsistent canonical position owner')
        cases = 'CASE position.field_kind ' + ' '.join(f"WHEN {field_code_literal(connection,name,'field_kind',row['field_code'])} THEN {literal(row['kind'])}" for row in members) + ' END'
        positions.append(f'SELECT position.relationship_id,{cases} AS reported_kind,position.{owner} AS source_element_id,position.field_occurrence,element.edition_id FROM {identifier(name)} AS position LEFT JOIN catalog_source_elements AS element ON element.source_element_id=position.{owner} WHERE position.relationship_id IS NOT NULL')
        valid_choices = []
        for row in members:
            valid_choices.append(f"(NEW.field_kind={field_code_literal(connection,name,'field_kind',row['field_code'])} AND declaration.reported_kind={literal(row['kind'])})")
        valid = f"SELECT 1 FROM candidate_relationship_declarations AS declaration JOIN reported_catalog_relationships AS reported USING(relationship_id,reported_kind) JOIN catalog_relationships AS identity USING(relationship_id) WHERE declaration.relationship_id=NEW.relationship_id AND declaration.source_element_id=NEW.{owner} AND declaration.edition_id=identity.edition_id AND identity.origin='source' AND NEW.field_occurrence=0 AND ({' OR '.join(valid_choices)})"
        for operation in ('INSERT','UPDATE'):
            guards.append(f"CREATE TRIGGER {identifier('candidate_relationship_position_' + name + '_' + operation.lower())} BEFORE {operation} ON {identifier(name)} WHEN NEW.relationship_id IS NOT NULL AND NOT EXISTS({valid}) BEGIN SELECT RAISE(ABORT,'relationship position requires its exact typed declaration'); END;")
    for name, members in by_declaration.items():
        owner = identifier(members[0]['declaration_owner'])
        kind = literal(members[0]['kind']) if len(members)==1 else 'CASE NEW.link_kind ' + ' '.join(f"WHEN {literal(row['link_kind'])} THEN {literal(row['kind'])}" for row in members) + ' END'
        match = f'position.relationship_id IS NEW.relationship_id AND position.source_element_id IS NEW.{owner} AND position.reported_kind IS ({kind})'
        guards.append(f"CREATE TRIGGER {identifier('candidate_relationship_declaration_' + name + '_update')} BEFORE UPDATE ON {identifier(name)} WHEN EXISTS(SELECT 1 FROM candidate_relationship_positions AS position WHERE position.relationship_id=OLD.relationship_id AND NOT({match})) BEGIN SELECT RAISE(ABORT,'relationship declaration cannot detach its canonical position'); END;")
        guards.append(f"CREATE TRIGGER {identifier('candidate_relationship_declaration_' + name + '_delete')} BEFORE DELETE ON {identifier(name)} WHEN EXISTS(SELECT 1 FROM candidate_relationship_positions WHERE relationship_id=OLD.relationship_id) BEGIN SELECT RAISE(ABORT,'remove draft positions before their declaration'); END;")
    views = '\n'.join((
        'CREATE VIEW candidate_relationship_declarations AS ' + ' UNION ALL '.join(declarations) + ';',
        'CREATE VIEW candidate_relationship_positions AS ' + ' UNION ALL '.join(positions) + ';'))
    problems.extend((
        "SELECT 'relationship_position_count' AS problem,declaration.relationship_id AS owner_id,coalesce(declaration.edition_id,identity.edition_id) AS edition_id FROM candidate_relationship_declarations AS declaration LEFT JOIN catalog_relationships AS identity USING(relationship_id) LEFT JOIN candidate_relationship_positions AS position ON position.relationship_id=declaration.relationship_id AND position.reported_kind=declaration.reported_kind AND position.source_element_id=declaration.source_element_id AND position.field_occurrence=0 GROUP BY declaration.relationship_id,declaration.reported_kind,declaration.source_element_id HAVING count(position.relationship_id)<>1",
        "SELECT 'relationship_position_identity',position.relationship_id,coalesce(position.edition_id,identity.edition_id) FROM candidate_relationship_positions AS position LEFT JOIN catalog_relationships AS identity USING(relationship_id) WHERE NOT EXISTS(SELECT 1 FROM candidate_relationship_declarations AS declaration JOIN reported_catalog_relationships AS reported USING(relationship_id,reported_kind) WHERE declaration.relationship_id=position.relationship_id AND declaration.source_element_id=position.source_element_id AND declaration.reported_kind=position.reported_kind AND declaration.edition_id=position.edition_id AND identity.origin='source' AND identity.edition_id=position.edition_id AND position.field_occurrence=0)"))
    guards.extend((
        "CREATE TRIGGER candidate_relationship_identity_update BEFORE UPDATE ON catalog_relationships WHEN EXISTS(SELECT 1 FROM candidate_relationship_declarations WHERE relationship_id=OLD.relationship_id AND (NEW.origin<>'source' OR edition_id IS NOT NEW.edition_id)) BEGIN SELECT RAISE(ABORT,'reported identity must retain its native edition'); END;",
        "CREATE TRIGGER candidate_relationship_kind_update BEFORE UPDATE ON reported_catalog_relationships WHEN EXISTS(SELECT 1 FROM candidate_relationship_declarations WHERE relationship_id=OLD.relationship_id AND reported_kind IS NOT NEW.reported_kind) BEGIN SELECT RAISE(ABORT,'reported kind must match its native declaration'); END;"))
    for row in routes:
        if row['marker_table']=='-':
            continue
        marker, declaration, position = map(identifier,(row['marker_table'],row['declaration_table'],row['position_table']))
        owner, position_owner = map(identifier,(row['declaration_owner'],row['position_owner']))
        code = field_code_literal(connection,row['position_table'],'field_kind',row['field_code'])
        problems.append(f"SELECT {literal('relationship_marker:' + row['kind'])},marker.{owner},element.edition_id FROM {marker} AS marker LEFT JOIN catalog_source_elements AS element ON element.source_element_id=marker.{owner} WHERE EXISTS(SELECT 1 FROM {declaration} WHERE {owner}=marker.{owner}) OR (SELECT count(*) FROM {position} WHERE {position_owner}=marker.{owner} AND field_kind={code} AND field_occurrence=0 AND relationship_id IS NULL)<>1")
        problems.append(f"SELECT {literal('relationship_missing_marker:' + row['kind'])},position.{position_owner},element.edition_id FROM {position} AS position LEFT JOIN catalog_source_elements AS element ON element.source_element_id=position.{position_owner} WHERE position.field_kind={code} AND position.relationship_id IS NULL AND NOT EXISTS(SELECT 1 FROM {marker} WHERE {owner}=position.{position_owner})")
        for operation in ('INSERT','UPDATE'):
            guards.append(f"CREATE TRIGGER {identifier('candidate_relationship_marker_position_' + row['kind'] + '_' + operation.lower())} BEFORE {operation} ON {position} WHEN NEW.field_kind={code} AND NEW.relationship_id IS NULL AND NOT EXISTS(SELECT 1 FROM {marker} WHERE {owner}=NEW.{position_owner}) BEGIN SELECT RAISE(ABORT,'NULL clone identity requires its typed P marker'); END;")
            for target,other in ((row['marker_table'],declaration),(row['declaration_table'],marker)):
                guards.append(f"CREATE TRIGGER {identifier('candidate_relationship_marker_exclusion_' + target + '_' + operation.lower())} BEFORE {operation} ON {identifier(target)} WHEN EXISTS(SELECT 1 FROM {other} WHERE {owner}=NEW.{owner}) BEGIN SELECT RAISE(ABORT,'clone marker and relationship declaration are mutually exclusive'); END;")
    return views,guards,problems


def format_root_sql(connection, manifest):
    roots = (('mame_documents','mame'), ('software_documents','software'),
             ('logiqx_documents','logiqx'), ('clrmamepro_documents','clrmamepro'),
             ('no_intro_dat_documents','no_intro_dat'), ('no_intro_export_documents','no_intro_database'),
             ('no_intro_pc_documents','no_intro_pc_fixture'))
    union = ' UNION ALL '.join(f'SELECT edition_id,{literal(family)} AS format_family FROM {identifier(table)}' for table,family in roots)
    view = f'CREATE VIEW candidate_document_roots AS {union};'
    problems = ["SELECT 'document_root_count' AS problem,edition.edition_id AS owner_id,edition.edition_id FROM catalog_editions AS edition JOIN catalog_reading_rules AS rules USING(reading_rules_id) LEFT JOIN candidate_document_roots AS root ON root.edition_id=edition.edition_id GROUP BY edition.edition_id HAVING count(root.edition_id)<>1 OR sum(root.format_family=rules.format_family)<>1"]
    cases = 'CASE element.element_kind ' + ' '.join(f'WHEN {literal(owner.kind)} THEN {literal(kind_family(owner.kind))}' for owner in manifest) + ' END'
    problems.append(f"SELECT 'element_reading_family',element.source_element_id,element.edition_id FROM catalog_source_elements AS element JOIN catalog_editions AS edition USING(edition_id) JOIN catalog_reading_rules AS rules USING(reading_rules_id) WHERE rules.format_family IS NOT ({cases})")
    guards = []
    physical = {(owner,family) for _,owner,family,_ in ROOT_LINKS}
    for table, family in sorted(physical):
        scope = edition_expression(connection, table, 'owner', {entry.table:entry.id for entry in manifest})
        proposed = ','.join(f'NEW.{identifier(row[1])} AS {identifier(row[1])}' for row in table_columns(connection, table))
        valid = f'SELECT 1 FROM (SELECT {proposed}) AS owner JOIN catalog_editions AS edition ON edition.edition_id={scope} JOIN catalog_reading_rules AS rules USING(reading_rules_id) WHERE rules.format_family={literal(family)}'
        for operation in ('INSERT','UPDATE'):
            guards.append(f"CREATE TRIGGER {identifier('candidate_root_format_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN NOT EXISTS({valid}) BEGIN SELECT RAISE(ABORT,'physical owner requires its selected reading family'); END;")
    return view,guards,problems


def position_order_sql(connection, manifest):
    """The native and compatibility/XSI fields of one tag share one ordinal."""
    groups, guards, problems = defaultdict(list), [], []
    native_keys = {owner.table:owner.id for owner in manifest}
    for (table,) in connection.execute("SELECT name FROM sqlite_schema WHERE type='table'").fetchall():
        if not (table.endswith('_positions') or table.endswith('_xsi_attributes')):
            continue
        columns = {row[1] for row in table_columns(connection, table)}
        ordinal = next((column for column in ('source_order','attribute_order','attribute_ordinal') if column in columns), None)
        if ordinal is None:
            continue
        fks = [row for row in connection.execute(f'PRAGMA foreign_key_list({identifier(table)})')
               if row[2] not in ('catalog_entry_hashes','reported_catalog_relationships','no_intro_release_nfo_hashes')]
        parents = {(row[2],row[3],row[4]) for row in fks}
        if len(parents) != 1:
            raise ValueError(f'{table}: position order needs exactly one actual owner FK, got {parents}')
        parent, column, target = parents.pop()
        groups[(parent,target)].append((table,column,ordinal))
    # CMP field pairs and media forms share set-body Form.items order.
    groups[('clrmamepro_sets','set_id')].extend((table,'set_id','source_order') for table in ('clrmamepro_roms','clrmamepro_samples'))
    for (parent,target), members in groups.items():
        branches = [f'SELECT {identifier(column)} AS owner_id,{identifier(ordinal)} AS source_order FROM {identifier(table)}' for table,column,ordinal in members]
        scope = edition_expression(connection, parent, 'parent', native_keys)
        problems.append(f"SELECT {literal('position_order:' + parent)} AS problem,position.owner_id,{scope} AS edition_id FROM ({' UNION ALL '.join(branches)}) AS position LEFT JOIN {identifier(parent)} AS parent ON parent.{identifier(target)}=position.owner_id GROUP BY position.owner_id,position.source_order HAVING count(*)>1")
        for table,column,ordinal in members:
            primary = [row[1] for row in table_columns(connection,table) if row[5]]
            for operation in ('INSERT','UPDATE'):
                checks = []
                for other,other_column,other_ordinal in members:
                    exclude = ''
                    if other == table and operation == 'UPDATE':
                        exclude = ' AND NOT(' + ' AND '.join(f'other.{identifier(key)} IS OLD.{identifier(key)}' for key in primary) + ')'
                    checks.append(f'SELECT 1 FROM {identifier(other)} AS other WHERE other.{identifier(other_column)}=NEW.{identifier(column)} AND other.{identifier(other_ordinal)}=NEW.{identifier(ordinal)}{exclude}')
                guards.append(f"CREATE TRIGGER {identifier('candidate_position_order_' + table + '_' + operation.lower())} BEFORE {operation} ON {identifier(table)} WHEN EXISTS({' UNION ALL '.join(checks)}) BEGIN SELECT RAISE(ABORT,'native, compatibility and lexical item ordinals are unique per owner'); END;")
    return guards,problems


def ownership_sql(connection, manifest):
    rows, problems, guards = [], [], []
    sequences = defaultdict(list)
    for owner in manifest:
        table, key = identifier(owner.table), identifier(owner.id)
        parent, parent_key, parent_column = map(identifier, (owner.parent_table, owner.parent_key, owner.parent_column))
        columns = {row[1] for row in table_columns(connection, owner.table)}
        if owner.id not in columns or owner.parent_column not in columns:
            raise ValueError(f"owner manifest does not match {owner.table} columns")
        parent_scope = edition_expression(connection, owner.parent_table, "parent", {entry.table: entry.id for entry in manifest})
        rows.append(f"SELECT {literal(owner.kind)} AS element_kind,owner.{key} AS source_element_id FROM {table} AS owner")
        problems.append(f"SELECT {literal('owner_kind:' + owner.table)} AS problem,owner.{key} AS owner_id,element.edition_id FROM {table} AS owner LEFT JOIN catalog_source_elements AS element ON element.source_element_id=owner.{key} WHERE element.element_kind IS NOT {literal(owner.kind)}")
        ancestry = (f' FROM {table} AS owner LEFT JOIN catalog_source_elements AS element '
                    f'ON element.source_element_id=owner.{key} LEFT JOIN {parent} AS parent '
                    f'ON parent.{parent_key}=owner.{parent_column}')
        problem = literal('owner_ancestry:' + owner.table)
        problems.append(f'SELECT {problem},owner.{key},coalesce(element.edition_id,{parent_scope})'
                        f'{ancestry} WHERE element.edition_id IS NOT {parent_scope}')
        # Contradictory known ancestry affects both editions. A registry-only
        # projection would let the parent's edition publish corrupted children.
        problems.append(f'SELECT {problem},owner.{key},{parent_scope}{ancestry} '
                        f'WHERE element.edition_id IS NOT NULL AND {parent_scope} IS NOT NULL '
                        f'AND element.edition_id<>{parent_scope}')
        good = f"SELECT 1 FROM catalog_source_elements AS element JOIN {parent} AS parent ON parent.{parent_key}=NEW.{parent_column} JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id JOIN catalog_reading_rules AS rules USING(reading_rules_id) WHERE element.source_element_id=NEW.{key} AND element.element_kind={literal(owner.kind)} AND element.edition_id={parent_scope} AND rules.format_family={literal(kind_family(owner.kind))}"
        for operation in ("INSERT", "UPDATE"):
            guards.append(f"CREATE TRIGGER {identifier('candidate_native_' + owner.table + '_' + operation.lower())} BEFORE {operation} ON {table} WHEN NOT EXISTS({good}) BEGIN SELECT RAISE(ABORT,'candidate native kind or ancestry'); END;")
        if owner.sequence != "-":
            if "source_order" not in columns:
                raise ValueError(f"{owner.table}: sequenced owner has no source_order")
            parent_domain = ('catalog_set_groups', 'set_group_id') if owner.parent_table == 'software_lists' else (owner.parent_table, owner.parent_key)
            sequences[parent_domain].append(owner)
        if owner.media:
            problems.append(f"SELECT {literal('missing_media:' + owner.table)},owner.{key},element.edition_id FROM {table} AS owner LEFT JOIN catalog_source_elements AS element ON element.source_element_id=owner.{key} LEFT JOIN catalog_media_entries AS media ON media.media_entry_id=owner.{key} WHERE media.media_entry_id IS NULL")
    sequences[('catalog_set_groups', 'set_group_id')].append(NativeOwner('common_set_placement', 'catalog_sets', 'set_id', 'catalog_set_groups', 'set_group_id', 'set_group_id', 'source_order', False))
    for (parent_table, parent_key), members in sequences.items():
        sequence = parent_table + '_' + parent_key
        branches = [f"SELECT {identifier(owner.parent_column)} AS parent_id,source_order,{identifier(owner.id)} AS source_element_id FROM {identifier(owner.table)}" for owner in members]
        union = " UNION ALL ".join(branches)
        problems.append(f"SELECT {literal('mixed_order:' + sequence)},min(child.source_element_id),element.edition_id FROM ({union}) AS child LEFT JOIN catalog_source_elements AS element USING(source_element_id) GROUP BY child.parent_id,child.source_order HAVING count(*)<>1")
        for owner in members:
            for operation in ("INSERT", "UPDATE"):
                other = f" AND child.source_element_id<>OLD.{identifier(owner.id)}" if operation == "UPDATE" else ""
                duplicate = f"SELECT 1 FROM ({union}) AS child WHERE child.parent_id=NEW.{identifier(owner.parent_column)} AND child.source_order=NEW.source_order{other}"
                guards.append(f"CREATE TRIGGER {identifier('candidate_siblings_' + owner.table + '_' + operation.lower())} BEFORE {operation} ON {identifier(owner.table)} WHEN EXISTS({duplicate}) BEGIN SELECT RAISE(ABORT,'candidate mixed sibling order collision'); END;")
    owner_union = " UNION ALL ".join(rows)
    registry_problem = "SELECT 'native_owner_count' AS problem,element.source_element_id AS owner_id,element.edition_id FROM catalog_source_elements AS element LEFT JOIN candidate_native_owners AS owner ON owner.source_element_id=element.source_element_id AND owner.element_kind=element.element_kind GROUP BY element.source_element_id HAVING count(owner.source_element_id)<>1"
    media_kinds = ",".join(literal(owner.kind) for owner in manifest if owner.media) or "NULL"
    problems.append(f"SELECT 'unexpected_common_media',media.media_entry_id,element.edition_id FROM catalog_media_entries AS media LEFT JOIN catalog_source_elements AS element ON element.source_element_id=media.media_entry_id WHERE element.element_kind IS NULL OR element.element_kind NOT IN ({media_kinds})")
    return f"CREATE VIEW candidate_native_owners AS {owner_union};", [registry_problem, *problems], guards


def format_audit_queries(connection):
    """Require every native cardinality audit in the publication input set.

    A misspelled view must fail assembly, not silently disappear from the
    discovery glob and leave a family publishable without its child checks.
    """
    required = {f'candidate_{family}_cardinality_problems' for family in FAMILIES}
    names = {row[0] for row in connection.execute(
        "SELECT name FROM sqlite_schema WHERE type='view' AND "
        "(name GLOB 'candidate_*_integrity_problems' OR "
        "name GLOB 'candidate_*_cardinality_problems' OR "
        "name='candidate_edition_cycle_problems')")}
    if missing := required - names:
        raise ValueError(f'missing native cardinality audits: {sorted(missing)}')
    queries = []
    for name in sorted(names):
        query = f'SELECT problem,owner_id,edition_id FROM {identifier(name)}'
        # Validate the complete declared interface, including unused extra
        # columns and dependencies that SQLite defers when CREATE VIEW runs.
        if name.endswith('_cardinality_problems'):
            columns = tuple(row[1] for row in connection.execute(
                f'PRAGMA table_info({identifier(name)})'))
            if columns != ('problem', 'owner_id', 'edition_id'):
                raise ValueError(f'{name}: audit columns must be problem,owner_id,edition_id')
            connection.execute(query + ' LIMIT 0')
        # Other integrity views may depend on generated relationship views not
        # yet installed in this compiler connection. The completed assembly's
        # all-view prepare check validates those after their dependencies exist.
        queries.append(query)
    return queries


def publication_closure_sql(audit='candidate_integrity_problems'):
    """Single aggregate integrity gate, also used by thin compiler witnesses."""
    return f"CREATE TRIGGER candidate_publication_closure BEFORE INSERT ON published_catalog_editions WHEN EXISTS(SELECT 1 FROM {identifier(audit)} WHERE edition_id=NEW.edition_id) BEGIN SELECT RAISE(ABORT,'candidate publication requires complete closure'); END;"


def assemble():
    import dat_xsi
    import native_file_qualification
    import native_file_sizes
    import shared_file_facts
    import source_counts
    import software_file_lengths
    import software_numbers

    manifest = owners()
    source = "\n\n".join((ROOT / fragment).read_text() for fragment in FRAGMENTS)
    source = source.replace("/* SOURCE_ELEMENT_KINDS */", ",".join(literal(owner.kind) for owner in manifest))
    source += '\n' + software_numbers.sql() + '\n' + software_file_lengths.sql()
    source += '\n' + native_file_sizes.sql() + '\n' + native_file_qualification.sql()
    source += '\n' + shared_file_facts.sql()
    source += '\n' + dat_xsi.sql()
    root_tables, root_guards, root_problems = root_diagnostic_sql()
    source += '\n' + root_tables
    with closing(sqlite3.connect(":memory:")) as connection:
        connection.executescript(source)
        ownership, native_problems, native_guards = ownership_sql(connection, manifest)
        fk_guards, fk_problems = foreign_key_guards(connection, manifest)
        collisions = collision_guards(connection)
        immutable = published_fact_guards(connection, manifest)
        hash_positions, hash_guards, hash_problems = hash_position_sql(connection)
        # Qualification consumes the canonical native hash selector. Install it
        # in the introspection DB before preparing the composed audit views.
        connection.executescript(hash_positions)
        format_roots,format_guards,format_problems = format_root_sql(connection, manifest)
        position_guards,position_problems = position_order_sql(connection, manifest)
        relationship_views,relationship_guards,relationship_problems = relationship_position_sql(connection)
        presence_routes = field_presence_routes()
        expected_fields = {(row['position_table'], row['field_code']) for row in field_coverage(connection)
                           if row['position_table'] != '-'
                           and 'field_kind' in {column[1] for column in table_columns(connection, row['position_table'])}}
        actual_fields = {(row['position_table'], row['field_code']) for row in presence_routes}
        if actual_fields != expected_fields:
            raise ValueError(f'presence routes differ from independent field crosswalk: '
                             f'missing={sorted(expected_fields-actual_fields)}, extra={sorted(actual_fields-expected_fields)}')
        presence_views, presence_problems = field_presence_sql(connection, manifest, presence_routes)
        format_audits = format_audit_queries(connection)
        count_routes = source_counts.inventory()
        source_counts.validate_inventory(connection, count_routes)
        count_contract = source_counts.fragment(connection, count_routes)
    audits = [*format_audits, *native_problems, *fk_problems, *root_problems, *hash_problems, *format_problems, *position_problems, *relationship_problems, *presence_problems,
              'SELECT * FROM candidate_source_count_problems',
              'SELECT * FROM candidate_dat_xsi_problems',
              'SELECT * FROM candidate_file_qualification_problems',
              'SELECT * FROM candidate_shared_identity_problems']
    # SQLite limits a single compound SELECT to 500 terms. Keep the exhaustive
    # reverse audit in named bounded chunks, not one oversized UNION statement.
    chunks = [audits[offset:offset + 100] for offset in range(0, len(audits), 100)]
    chunk_views = [f"CREATE VIEW candidate_integrity_chunk_{number} AS " + " UNION ALL ".join(chunk) + ';' for number, chunk in enumerate(chunks)]
    integrity = '\n'.join([*chunk_views, "CREATE VIEW candidate_integrity_problems AS " + " UNION ALL ".join(f'SELECT * FROM candidate_integrity_chunk_{number}' for number in range(len(chunks))) + ';'])
    return "\n\n".join([source, ownership, hash_positions,format_roots,relationship_views, presence_views, count_contract, integrity, *native_guards, *fk_guards, *collisions, *immutable, *immutable_dictionary_sql(), *root_guards, *hash_guards,*format_guards,*position_guards,*relationship_guards, publication_closure_sql()])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emit", action="store_true", help="emit the complete concrete candidate SQL to stdout")
    args = parser.parse_args()
    sql = assemble()
    if args.emit:
        sys.stdout.write(sql + "\n")
        return
    with closing(sqlite3.connect(":memory:")) as connection:
        connection.execute("PRAGMA foreign_keys=ON")
        connection.executescript(sql)
        views = connection.execute("SELECT name FROM sqlite_schema WHERE type='view'").fetchall()
        for (view,) in views:
            try:
                connection.execute(f"SELECT * FROM {identifier(view)} LIMIT 0")
            except sqlite3.Error as error:
                raise RuntimeError(f'candidate view {view} does not prepare: {error}') from error
        if connection.execute("PRAGMA foreign_key_check").fetchall():
            raise RuntimeError("candidate foreign-key check failed")
        print(f"Candidate DDL prepares: {len(owners())} closed native kinds, {len(views)} prepared views; empty-schema check only.")


if __name__ == "__main__":
    main()
