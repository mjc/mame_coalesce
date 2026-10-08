"""Emit one set-based maintenance contract for shared hashes and sizes."""


LANES = (
    ('shared_file_hashes', 'hash_id', 'accepted_catalog_file_hash_evidence'),
    ('shared_file_sizes', 'byte_length', 'accepted_catalog_file_size_evidence'),
)


def rebuild(roots):
    """Rebuild only selected components, including their old issued aliases."""
    members = (
        'SELECT member.file_uuid FROM candidate_file_component_members AS member '
        f'WHERE member.canonical_file_uuid IN ({roots})'
    )
    statements = []
    for table, value, evidence in LANES:
        # Select roots BEFORE grouping witnesses. Filtering an already-grouped
        # global view by a correlated IN query can materialize every component.
        accepted = (
            f'SELECT selected.canonical_file_uuid AS file_uuid,evidence.{value} '
            f'FROM (SELECT DISTINCT canonical_file_uuid FROM ({roots})) AS selected '
            'CROSS JOIN candidate_file_component_members AS member '
            'ON member.canonical_file_uuid=selected.canonical_file_uuid '
            f'JOIN {evidence} AS evidence ON evidence.file_uuid=member.file_uuid '
            f'GROUP BY selected.canonical_file_uuid,evidence.{value}'
        )
        statements.extend((
            f'DELETE FROM {table} WHERE file_uuid IN ({members});',
            f'INSERT INTO {table}(file_uuid,{value}) {accepted};',
        ))
        stored = f'SELECT file_uuid,{value} FROM {table} WHERE file_uuid IN ({members})'
        statements.append(
            "SELECT RAISE(ABORT,'shared file membership is not its accepted witness set') "
            f'WHERE EXISTS({stored} EXCEPT {accepted}) OR EXISTS({accepted} EXCEPT {stored});'
        )
    statements.extend((
        'SELECT RAISE(ABORT,\'shared file has contradictory accepted facts\') '
        f'WHERE EXISTS(SELECT 1 FROM shared_file_sizes WHERE file_uuid IN ({roots}) '
        'GROUP BY file_uuid HAVING count(*)>1) '
        'OR EXISTS(SELECT 1 FROM shared_file_hashes AS membership '
        'JOIN hash_values AS value USING(hash_id) '
        f'WHERE membership.file_uuid IN ({roots}) '
        'GROUP BY membership.file_uuid,value.algorithm HAVING count(*)>1);',
    ))
    return '\n'.join(statements)


def sql():
    review_roots = ('SELECT canonical_file_uuid FROM candidate_review_file_components '
                    'WHERE decision_id=NEW.decision_id')
    edition_roots = (
        'SELECT canonical.canonical_file_uuid FROM catalog_source_elements AS element '
        'JOIN catalog_media_entries AS media ON media.media_entry_id=element.source_element_id '
        'JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=media.file_uuid '
        'WHERE element.edition_id=NEW.edition_id'
    )
    review_owners = f'''
        SELECT media.media_entry_id
        FROM (SELECT DISTINCT canonical_file_uuid FROM ({review_roots})) AS roots
        CROSS JOIN candidate_file_component_members AS member
          ON member.canonical_file_uuid=roots.canonical_file_uuid
        JOIN catalog_media_entries AS media ON media.file_uuid=member.file_uuid
        UNION ALL
        SELECT conflict.incoming_media_entry_id
        FROM file_match_decision_conflicts AS settled
        JOIN file_match_conflicts AS conflict USING(conflict_id)
        WHERE settled.decision_id=NEW.decision_id
        UNION ALL
        SELECT declaration.media_entry_id
        FROM file_match_decision_conflicts AS settled
        JOIN file_match_conflict_hashes AS witness USING(conflict_id)
        LEFT JOIN catalog_entry_hashes AS declaration USING(reported_hash_id)
        WHERE settled.decision_id=NEW.decision_id
        UNION ALL
        SELECT witness.media_entry_id
        FROM file_match_decision_conflicts AS settled
        JOIN file_match_conflict_sizes AS witness USING(conflict_id)
        WHERE settled.decision_id=NEW.decision_id
    '''
    return f"""
-- Reviews freeze already-published source facts, not provisional import
-- batches. Ordinary same-import matching still sees completed-batch facts.
CREATE TRIGGER shared_file_facts_review_requires_frozen_sources
BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (
    SELECT 1 FROM ({review_owners}) AS owner
    LEFT JOIN catalog_source_elements AS element
      ON element.source_element_id=owner.media_entry_id
    LEFT JOIN published_catalog_editions AS publication
      ON publication.edition_id=element.edition_id
    WHERE element.source_element_id IS NULL OR publication.edition_id IS NULL
)
BEGIN SELECT RAISE(ABORT,'file-match review requires published source owners'); END;

CREATE TRIGGER shared_file_facts_after_file_match_publication
AFTER INSERT ON file_match_decision_publications
BEGIN
    {rebuild(review_roots)}
    SELECT RAISE(ABORT,'accepted incoming hash contradicts retained file') WHERE EXISTS (
        SELECT 1 FROM file_match_decisions AS decision
        JOIN file_match_hash_decisions AS review USING(decision_id)
        JOIN candidate_qualified_file_hashes AS source USING(reported_hash_id)
        JOIN hash_values AS incoming ON incoming.hash_id=source.hash_id
        JOIN canonical_shared_file_uuids AS kept ON kept.file_uuid=decision.kept_file_uuid
        JOIN shared_file_hashes AS membership ON membership.file_uuid=kept.canonical_file_uuid
        JOIN hash_values AS retained ON retained.hash_id=membership.hash_id
        WHERE decision.decision_id=NEW.decision_id AND decision.decision='merge'
          AND review.role='incoming' AND review.disposition='accept'
          AND incoming.algorithm=retained.algorithm AND incoming.bytes<>retained.bytes
          AND NOT EXISTS (
              SELECT 1 FROM file_match_hash_decisions AS rejection
              JOIN file_match_decision_publications AS publication USING(decision_id)
              WHERE rejection.reported_hash_id=source.reported_hash_id AND rejection.disposition='reject'
          )
    );
    SELECT RAISE(ABORT,'accepted incoming size contradicts retained file') WHERE EXISTS (
        SELECT 1 FROM file_match_decisions AS decision
        JOIN file_match_size_decisions AS review USING(decision_id)
        JOIN candidate_native_file_sizes AS source
          ON source.media_entry_id=review.media_entry_id AND source.source_size_field=review.source_size_field
        JOIN candidate_native_file_byte_coverage AS qualification ON qualification.media_entry_id=source.media_entry_id
        JOIN canonical_shared_file_uuids AS kept ON kept.file_uuid=decision.kept_file_uuid
        JOIN shared_file_sizes AS retained ON retained.file_uuid=kept.canonical_file_uuid
        WHERE decision.decision_id=NEW.decision_id AND decision.decision='merge'
          AND review.role='incoming' AND review.disposition='accept' AND source.size_state='value'
          AND source.byte_length<>retained.byte_length
          AND NOT EXISTS (
              SELECT 1 FROM file_match_size_decisions AS rejection
              JOIN file_match_decision_publications AS publication USING(decision_id)
              WHERE rejection.media_entry_id=source.media_entry_id
                AND rejection.source_size_field=source.source_size_field AND rejection.disposition='reject'
          )
    );
    SELECT RAISE(ABORT,'linked incoming file is outside the merged component') WHERE EXISTS (
        SELECT 1 FROM file_match_decisions AS decision
        JOIN file_match_decision_conflicts AS settled USING(decision_id)
        JOIN file_match_conflicts AS conflict USING(conflict_id)
        JOIN catalog_media_entries AS incoming ON incoming.media_entry_id=conflict.incoming_media_entry_id
        JOIN canonical_shared_file_uuids AS incoming_root ON incoming_root.file_uuid=incoming.file_uuid
        JOIN canonical_shared_file_uuids AS kept ON kept.file_uuid=decision.kept_file_uuid
        WHERE decision.decision_id=NEW.decision_id AND decision.decision='merge'
          AND incoming_root.canonical_file_uuid<>kept.canonical_file_uuid
    );
END;

CREATE TRIGGER shared_file_facts_after_edition_publication
AFTER INSERT ON published_catalog_editions
BEGIN
    {rebuild(edition_roots)}
END;
""".strip()
