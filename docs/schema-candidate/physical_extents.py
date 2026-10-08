"""Validate supplied native byte ranges independently of diagnostic links."""


def sql(connection, manifest):
    from assemble import identifier, literal, schema_tables, table_columns

    native_keys = {owner.table: owner.id for owner in manifest}
    branches = []
    for table in schema_tables(connection):
        columns = table_columns(connection, table)
        if 'extent_view' not in {row[1] for row in columns}:
            continue
        primary = sorted((row for row in columns if row[5]), key=lambda row: row[5])
        if len(primary) != 1:
            raise ValueError(f'{table}: physical extent needs one typed owner key')
        names = {row[1] for row in columns}
        ancestry = ''
        if 'edition_id' in names:
            scope = 'owner.edition_id'
        elif table in native_keys:
            ancestry = f' LEFT JOIN catalog_source_elements AS element ON element.source_element_id=owner.{identifier(native_keys[table])}'
            scope = 'element.edition_id'
        elif 'set_group_id' in names:
            ancestry = ' LEFT JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.set_group_id'
            scope = 'groups.edition_id'
        else:
            raise ValueError(f'{table}: no indexed physical extent edition path')
        available = 'owner.extent_view IS NOT NULL OR owner.extent_start IS NOT NULL OR owner.extent_end IS NOT NULL'
        valid = f'''SELECT 1 FROM catalog_editions AS edition
            JOIN catalog_source_files AS source ON source.source_file_id=edition.source_file_id
            LEFT JOIN catalog_decoded_xml_views AS decoded ON decoded.source_file_id=source.source_file_id
            WHERE edition.edition_id={scope} AND owner.extent_start>=0
                AND owner.extent_start<owner.extent_end
                AND owner.extent_end<=CASE owner.extent_view
                    WHEN 'retained_original_bytes' THEN source.byte_length
                    WHEN 'transport_decoded_xml_bytes' THEN decoded.byte_length END'''
        branches.append(f'''SELECT {literal('physical_extent:' + table)} AS problem,
            owner.{identifier(primary[0][1])} AS owner_id,{scope} AS edition_id
            FROM {identifier(table)} AS owner{ancestry} WHERE ({available}) AND NOT EXISTS({valid})''')
    if not branches:
        raise ValueError('no native physical extents found')
    return 'CREATE VIEW candidate_physical_extent_problems AS ' + ' UNION ALL '.join(branches) + ';'
