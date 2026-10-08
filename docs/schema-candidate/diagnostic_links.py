"""Closed native diagnostic routes and shared half-open containment SQL."""


def containment_sql():
    """Aliases are message, owner, source and decoded in both callers."""
    byte_start = "CASE WHEN message.source_view=owner.extent_view AND message.source_problem_start IS NOT NULL THEN message.source_problem_start WHEN owner.extent_view='retained_original_bytes' THEN message.original_problem_start END"
    byte_end = "CASE WHEN message.source_view=owner.extent_view AND message.source_problem_end IS NOT NULL THEN message.source_problem_end WHEN owner.extent_view='retained_original_bytes' THEN message.original_problem_end END"
    byte_available = f'(owner.extent_view IS NOT NULL AND ({byte_start}) IS NOT NULL)'
    source_length = "CASE owner.extent_view WHEN 'retained_original_bytes' THEN source.byte_length WHEN 'transport_decoded_xml_bytes' THEN decoded.byte_length END"
    byte_within = f'(owner.extent_start>=0 AND owner.extent_start<owner.extent_end AND owner.extent_end<=({source_length}) AND owner.extent_start<=({byte_start}) AND ({byte_start})<owner.extent_end AND ({byte_start})<=({byte_end}) AND ({byte_end})<=owner.extent_end)'
    coord_available = '(message.line IS NOT NULL AND message.column IS NOT NULL AND owner.start_line IS NOT NULL AND owner.start_column IS NOT NULL AND owner.end_line IS NOT NULL AND owner.end_column IS NOT NULL AND message.location_view=owner.location_view AND message.column_convention=owner.column_convention)'
    coord_within = '((owner.start_line,owner.start_column)<(owner.end_line,owner.end_column) AND (message.line,message.column)>=(owner.start_line,owner.start_column) AND (message.line,message.column)<(owner.end_line,owner.end_column))'
    return f'(({byte_available}) IS TRUE OR ({coord_available}) IS TRUE) AND (({byte_available}) IS NOT TRUE OR ({byte_within}) IS TRUE) AND (({coord_available}) IS NOT TRUE OR ({coord_within}) IS TRUE)'


# Actual end coordinates exist for these export owners. An opening-only owner
# cannot borrow its parent's interval. This is a build-time closed route list,
# not a stored generic owner/extent table.
ROUTES = (
    ('no_intro_export_header_field', 'no_intro_export_header_fields', 'source_element_id', 'header'),
    ('no_intro_export_game', 'no_intro_export_games', 'set_id', 'game'),
    ('no_intro_export_archive', 'no_intro_archive_descriptions', 'archive_id', 'game_child'),
    ('no_intro_export_dump_source', 'no_intro_dump_sources', 'dump_source_id', 'game_child'),
    ('no_intro_export_dump_details', 'no_intro_dump_details', 'details_element_id', 'dump'),
    ('no_intro_export_dump_serials', 'no_intro_dump_serials', 'serials_element_id', 'dump'),
    ('no_intro_export_source_file', 'no_intro_dump_files', 'media_entry_id', 'dump'),
    ('no_intro_export_release', 'no_intro_releases', 'release_id', 'game_child'),
    ('no_intro_export_release_details', 'no_intro_release_details', 'details_element_id', 'release'),
    ('no_intro_export_release_serials', 'no_intro_release_serials', 'serials_element_id', 'release'),
    ('no_intro_export_release_file', 'no_intro_release_files', 'media_entry_id', 'release'),
)


def sql(manifest):
    # Import lazily: assemble owns identifier validation, the manifest and entry.
    from assemble import identifier, literal

    native = {entry.kind: entry for entry in manifest}
    branches, paths, intervals = [], [], []
    for kind, table, key, path in ROUTES:
        if kind not in native or (native[kind].table, native[kind].id) != (table,key):
            raise ValueError(f'ordinary diagnostic route differs from native owner: {kind}')
        aliases = [(table, 'native', key)]
        joins = []
        if path == 'header':
            joins.append('JOIN no_intro_export_headers AS header ON header.header_id=native.header_id')
            aliases.append(('no_intro_export_headers', 'header', 'header_id'))
            edition = 'header.edition_id'
            joins.append('JOIN no_intro_export_documents AS document ON document.edition_id=header.edition_id')
            joins.append("JOIN catalog_set_groups AS root_group ON root_group.set_group_id=document.root_set_group_id AND root_group.edition_id=header.edition_id AND root_group.group_kind='root'")
            aliases.append(('catalog_set_groups','root_group','set_group_id'))
        else:
            if path in ('dump','release'):
                parent_table, parent_key = ('no_intro_dump_sources','dump_source_id') if path == 'dump' else ('no_intro_releases','release_id')
                joins.append(f'JOIN {parent_table} AS parent ON parent.{parent_key}=native.{parent_key}')
                aliases.append((parent_table, 'parent', parent_key))
                game_id = 'parent.set_id'
            else:
                game_id = 'native.set_id'
            if path == 'game':
                game = 'native'
            else:
                joins.append(f'JOIN no_intro_export_games AS game ON game.set_id={game_id}')
                aliases.append(('no_intro_export_games', 'game', 'set_id'))
                game = 'game'
            joins += [f'JOIN catalog_sets AS sets ON sets.set_id={game}.set_id',
                      'JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id',
                      'JOIN no_intro_export_documents AS document ON document.edition_id=groups.edition_id AND document.root_set_group_id=groups.set_group_id']
            aliases += [('catalog_sets','sets','set_id'), ('catalog_set_groups','groups','set_group_id')]
            edition = 'groups.edition_id'
        aliases.append(('no_intro_export_documents','document','edition_id'))
        raw_path = f'FROM {identifier(table)} AS native ' + ' '.join(joins)
        checks = [] if path == 'header' else ["groups.group_kind='root'"]
        for index, (owner_table, alias, owner_key) in enumerate(aliases):
            expected = next((entry.kind for entry in manifest if entry.table==owner_table), None)
            if expected is not None:
                registry = f'element{index}'
                joins.append(f'JOIN catalog_source_elements AS {registry} ON {registry}.source_element_id={alias}.{owner_key} AND {registry}.element_kind={literal(expected)} AND {registry}.edition_id={edition}')
        if native[kind].media:
            joins.append(f'JOIN catalog_media_entries AS media ON media.media_entry_id=native.{key}')
        if path == 'game':
            start_line, start_column = 'sets.source_line','sets.source_column'
            byte_view, byte_start, byte_end = 'native.extent_view','native.extent_start','native.extent_end'
        else:
            start_line, start_column = 'native.source_line','native.source_column'
            byte_view = byte_start = byte_end = 'NULL'
        # Fix the requested native key first. Otherwise ANALYZE may choose an
        # edition-first join and scan unrelated native owners in this union.
        select = f'''SELECT native.{identifier(key)} AS source_element_id,{edition} AS edition_id,
            {literal(kind)} AS element_kind,{byte_view} AS extent_view,{byte_start} AS extent_start,{byte_end} AS extent_end,
            'transport_decoded_xml_text' AS location_view,{start_line} AS start_line,{start_column} AS start_column,
            native.source_end_line AS end_line,native.source_end_column AS end_column,
            'one_based_unicode_scalar' AS column_convention
            FROM {identifier(table)} AS native {' '.join(joins).replace('JOIN ', 'CROSS JOIN ')}'''
        if checks:
            select += ' WHERE ' + ' AND '.join(checks)
        branches.append(select)
        paths.append((key, raw_path, aliases))
        start_join = ' LEFT JOIN catalog_sets AS sets ON sets.set_id=native.set_id' if path == 'game' else ''
        interval = f'''({start_line}>0 AND {start_column}>0
            AND native.source_end_line>0 AND native.source_end_column>0
            AND ({start_line},{start_column})<(native.source_end_line,native.source_end_column))'''
        intervals.append(f'''SELECT {literal('ordinary_owner_interval:' + table)} AS problem,
            native.{identifier(key)} AS owner_id,element.edition_id
            FROM {identifier(table)} AS native
            LEFT JOIN catalog_source_elements AS element ON element.source_element_id=native.{identifier(key)}
            {start_join}
            WHERE (native.source_end_line IS NOT NULL OR native.source_end_column IS NOT NULL)
                AND ({interval}) IS NOT TRUE''')
    owners = 'CREATE VIEW candidate_ordinary_message_owners AS ' + ' UNION ALL '.join(branches) + ';'
    valid = f'''SELECT 1 FROM catalog_import_messages AS message
        JOIN catalog_imports AS run ON run.import_id=message.import_id
        JOIN catalog_editions AS edition ON edition.edition_id=run.edition_id
            AND edition.catalog_id=run.catalog_id AND edition.source_file_id=run.source_file_id
            AND edition.reading_rules_id=run.reading_rules_id
        JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
            AND rules.format_family='no_intro_database'
        JOIN catalog_source_files AS source ON source.source_file_id=edition.source_file_id
        LEFT JOIN catalog_decoded_xml_views AS decoded ON decoded.source_file_id=source.source_file_id
        JOIN candidate_ordinary_message_owners AS owner ON owner.source_element_id=link.source_element_id
            AND owner.edition_id=link.edition_id
        WHERE message.message_id=link.message_id AND message.edition_id=link.edition_id
            AND run.edition_id=link.edition_id AND message.source_file_id=run.source_file_id
            AND message.reading_rules_id=run.reading_rules_id AND run.status<>'failed'
            AND {containment_sql()}'''
    problems = f'''CREATE VIEW candidate_ordinary_message_problems AS
        SELECT 'ordinary_message_owner' AS problem,link.message_id AS owner_id,link.edition_id
        FROM catalog_import_message_elements AS link WHERE NOT EXISTS({valid});'''
    guards = []
    for _, table, key, _ in ROUTES:
        guards.append(f'''CREATE TRIGGER {identifier('candidate_ordinary_identity_' + table)}
            BEFORE UPDATE OF {identifier(key)} ON {identifier(table)}
            WHEN NEW.{identifier(key)} IS NOT OLD.{identifier(key)}
            BEGIN SELECT RAISE(ABORT,'native diagnostic owner identity is immutable'); END;''')
        guards.append(f'''CREATE TRIGGER {identifier('candidate_ordinary_linked_delete_' + table)}
            BEFORE DELETE ON {identifier(table)}
            WHEN EXISTS(SELECT 1 FROM catalog_import_message_elements AS link
                WHERE link.source_element_id=OLD.{identifier(key)})
            BEGIN SELECT RAISE(ABORT,'remove diagnostic links before deleting their native owner'); END;''')
    for operation in ('INSERT','UPDATE'):
        proposed = '(SELECT NEW.message_id AS message_id,NEW.source_element_id AS source_element_id,NEW.edition_id AS edition_id) AS link'
        guards.append(f'''CREATE TRIGGER candidate_ordinary_message_{operation.lower()}
            BEFORE {operation} ON catalog_import_message_elements
            WHEN NOT EXISTS(SELECT 1 FROM {proposed} WHERE EXISTS({valid}))
            BEGIN SELECT RAISE(ABORT,'ordinary diagnostic requires actual ancestry and containment'); END;''')
    # Keep already-linked draft facts valid too. Each reverse check is seeded by
    # the changed message/edition/native owner, not by a database-wide audit.
    affected = {
        'catalog_import_messages': 'link.message_id=NEW.message_id',
        'catalog_imports': 'link.message_id IN (SELECT message_id FROM catalog_import_messages WHERE import_id=NEW.import_id)',
        'catalog_editions': 'link.edition_id=NEW.edition_id',
        'catalog_reading_rules': 'link.edition_id IN (SELECT edition_id FROM catalog_editions WHERE reading_rules_id=NEW.reading_rules_id)',
        'catalog_source_files': 'link.edition_id IN (SELECT edition_id FROM catalog_editions WHERE source_file_id=NEW.source_file_id)',
        'catalog_decoded_xml_views': 'link.edition_id IN (SELECT edition_id FROM catalog_editions WHERE source_file_id=NEW.source_file_id)',
        'catalog_source_elements': 'link.source_element_id=NEW.source_element_id',
        'catalog_media_entries': 'link.source_element_id=NEW.media_entry_id',
    }
    native_scopes = {}
    registry_scopes = []
    for key, raw_path, aliases in paths:
        for table, alias, owner_key in aliases:
            # Stop at the changed ancestor: a rewrite can invalidate a later
            # join, and that invalidated path must not hide existing links.
            if alias == 'native':
                prefix = raw_path.split(' JOIN ',1)[0]
            else:
                marker = f' AS {alias} ON '
                end = raw_path.find(' JOIN ',raw_path.index(marker)+len(marker))
                prefix = raw_path if end == -1 else raw_path[:end]
            scope = f'link.source_element_id IN (SELECT native.{key} {prefix} WHERE {alias}.{owner_key}=NEW.{owner_key})'
            native_scopes.setdefault(table, []).append(scope)
            if table in {entry.table for entry in manifest}:
                registry_scopes.append(f'link.source_element_id IN (SELECT native.{key} {prefix} WHERE {alias}.{owner_key}=NEW.source_element_id)')
    for table, scopes in native_scopes.items():
        affected[table] = ' OR '.join(dict.fromkeys(scopes))
    # Reparenting can make the old ancestry vanish from raw_path. Edition/root
    # changes must therefore seed from stable link identity, not that new path.
    affected['no_intro_export_documents'] = 'link.edition_id IN (OLD.edition_id,NEW.edition_id)'
    affected['catalog_set_groups'] += ' OR link.edition_id IN (OLD.edition_id,NEW.edition_id)'
    affected['catalog_source_elements'] += ' OR ' + ' OR '.join(dict.fromkeys(registry_scopes))
    for table, scope in affected.items():
        guards.append(f'''CREATE TRIGGER {identifier('candidate_ordinary_reverse_' + table)}
            AFTER UPDATE ON {identifier(table)}
            WHEN EXISTS(SELECT 1 FROM catalog_import_message_elements AS link
                WHERE ({scope}) AND NOT EXISTS({valid}))
            BEGIN SELECT RAISE(ABORT,'rewrite invalidates ordinary diagnostic ancestry or containment'); END;''')
    coordinate_audit = 'CREATE VIEW candidate_ordinary_owner_interval_problems AS ' + ' UNION ALL '.join(intervals) + ';'
    return owners + '\n' + problems + '\n' + coordinate_audit, guards
