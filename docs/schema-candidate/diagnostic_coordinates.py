"""One coordinate policy for the candidate's corruption audit and DML guards."""


def highlight_problem(alias):
    start = (f"coalesce({alias}.source_problem_start,CASE WHEN "
             f"{alias}.source_view='retained_original_bytes' "
             f"THEN {alias}.original_problem_start END)")
    end = (f"coalesce({alias}.source_problem_end,CASE WHEN "
           f"{alias}.source_view='retained_original_bytes' "
           f"THEN {alias}.original_problem_end END)")
    origin = f'{alias}.excerpt_source_start'
    size = f'length({alias}.excerpt)'
    unmapped = (f'({alias}.excerpt_problem_start IS NOT NULL '
                f'OR {alias}.excerpt_problem_end IS NOT NULL)')
    mismatch = (f'({alias}.excerpt_problem_start IS NOT {start}-{origin} '
                f'OR {alias}.excerpt_problem_end IS NOT {end}-{origin})')
    return f'''{origin} IS NOT NULL AND (
        ({start} IS NULL AND {unmapped}) OR
        ({start} IS NOT NULL AND (
            ({start}>={origin} AND {end}-{origin}<={size} AND {mismatch}) OR
            (({start}<{origin} OR {end}-{origin}>{size}) AND {unmapped})
        ))
    )'''


def same_view_problem(alias):
    return f'''{alias}.source_view='retained_original_bytes'
        AND {alias}.source_problem_start IS NOT NULL
        AND {alias}.original_problem_start IS NOT NULL
        AND ({alias}.source_problem_start IS NOT {alias}.original_problem_start
             OR {alias}.source_problem_end IS NOT {alias}.original_problem_end)'''


def render(fragment):
    """Expand the two required slots; a missing/duplicate slot is an error."""
    policies = (
        ('message_highlight_mapping', highlight_problem),
        ('message_coordinate_views', same_view_problem),
    )
    audits = ' UNION ALL '.join(
        f"SELECT '{name}',message.message_id,message.edition_id "
        f'FROM catalog_import_messages AS message WHERE ({predicate("message")})'
        for name, predicate in policies)
    conditions = ' OR '.join(f'({predicate("NEW")})' for _, predicate in policies)
    guards = '\n'.join(
        f'''CREATE TRIGGER candidate_message_coordinates_{operation.lower()}
        BEFORE {operation} ON catalog_import_messages
        WHEN {conditions}
        BEGIN SELECT RAISE(ABORT,'diagnostic coordinates do not match their source views'); END;'''
        for operation in ('INSERT', 'UPDATE'))
    for marker, sql in (('/* COORDINATE_AUDITS */', audits),
                        ('/* COORDINATE_GUARDS */', guards)):
        if fragment.count(marker) != 1:
            raise ValueError(f'diagnostic fragment requires exactly one {marker}')
        fragment = fragment.replace(marker, sql)
    return fragment
