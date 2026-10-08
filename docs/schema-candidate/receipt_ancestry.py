"""Generate reverse guards for receipt provenance links."""


def _quote(identifier):
    return '"' + identifier.replace('"', '""') + '"'


def _message_link_tables(connection):
    """Return actual tables with a declared message_id FK to import messages."""
    tables = connection.execute("""SELECT name FROM sqlite_schema
        WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name""").fetchall()
    linked = []
    for (table,) in tables:
        foreign_keys = connection.execute(
            f'PRAGMA foreign_key_list({_quote(table)})').fetchall()
        if any(parent == 'catalog_import_messages' and child_column == 'message_id'
               and parent_column == 'message_id'
               for _, _, parent, child_column, parent_column, *_ in foreign_keys):
            linked.append(table)
    return linked


def _terminal_message_guards(connection):
    guards = []
    for table in ['catalog_import_messages', *_message_link_tables(connection)]:
        for operation, aliases in (('INSERT', ('NEW',)),
                                   ('UPDATE', ('OLD', 'NEW')),
                                   ('DELETE', ('OLD',))):
            conditions = []
            for alias in aliases:
                if table == 'catalog_import_messages':
                    lookup = ('SELECT 1 FROM catalog_imports AS run '
                              f'WHERE run.import_id={alias}.import_id')
                else:
                    lookup = ('SELECT 1 FROM catalog_import_messages AS message '
                              'JOIN catalog_imports AS run USING(import_id) '
                              f'WHERE message.message_id={alias}.message_id')
                conditions.append(f"EXISTS({lookup} AND run.status IN ('succeeded','failed'))")
            guards.append(f"""CREATE TRIGGER candidate_terminal_diagnostic_{table}_{operation.lower()}
                BEFORE {operation} ON {_quote(table)}
                WHEN {' OR '.join(conditions)}
                BEGIN SELECT RAISE(ABORT,'terminal import diagnostics are immutable'); END;""")
    return guards


def _receipt_use_condition(fetch_attempt):
    return f"""EXISTS(
        SELECT 1 FROM catalog_file_receipts AS receipt
        WHERE receipt.fetch_attempt_id={fetch_attempt}
          AND (
            EXISTS(SELECT 1 FROM catalog_editions AS edition
                JOIN published_catalog_editions AS publication USING(edition_id)
                WHERE edition.file_receipt_id=receipt.file_receipt_id)
            OR EXISTS(SELECT 1 FROM catalog_imports AS run
                WHERE run.file_receipt_id=receipt.file_receipt_id
                  AND run.status IN ('succeeded','failed'))
          ))"""


def sql(connection):
    """Return (guards, audits) for receipt provenance.

    The shared schema owns every column and ordinary FK. These checks only
    enforce agreement between receipt references and their owners. Acquisition
    headers/hashes remain editable until a receipt is used by a published
    edition or terminal import. Diagnostic guards are generated from the
    composed schema's actual message FKs using the required connection; the
    base message table is handled directly. Existing shared audits already
    report edition and import receipt ancestry.
    """
    receipt_valid_for_edition = '''
        SELECT 1
        FROM catalog_editions AS edition
        JOIN catalogs AS catalog USING (catalog_id)
        JOIN catalog_fetch_attempts AS attempt ON attempt.fetch_attempt_id=NEW.fetch_attempt_id
        WHERE edition.file_receipt_id=NEW.file_receipt_id
          AND (edition.source_file_id<>NEW.source_file_id
               OR catalog.publisher_id<>attempt.publisher_id)
    '''
    receipt_valid_for_import = '''
        SELECT 1
        FROM catalog_imports AS run
        JOIN catalogs AS catalog USING (catalog_id)
        JOIN catalog_fetch_attempts AS attempt ON attempt.fetch_attempt_id=NEW.fetch_attempt_id
        WHERE run.file_receipt_id=NEW.file_receipt_id
          AND (run.source_file_id<>NEW.source_file_id
               OR catalog.publisher_id<>attempt.publisher_id
               OR attempt.outcome<>'retained')
    '''
    guards = [
        """CREATE INDEX IF NOT EXISTS catalog_editions_by_receipt
        ON catalog_editions(file_receipt_id)
        WHERE file_receipt_id IS NOT NULL;""",
        """CREATE INDEX IF NOT EXISTS catalog_imports_by_receipt
        ON catalog_imports(file_receipt_id,status)
        WHERE file_receipt_id IS NOT NULL;""",
        f"""CREATE TRIGGER candidate_receipt_reverse_ancestry_insert
        BEFORE INSERT ON catalog_file_receipts
        WHEN EXISTS ({receipt_valid_for_edition})
          OR EXISTS ({receipt_valid_for_import})
        BEGIN SELECT RAISE(ABORT,'receipt reference ancestry'); END;""",
        f"""CREATE TRIGGER candidate_receipt_reverse_ancestry_update
        BEFORE UPDATE ON catalog_file_receipts
        WHEN EXISTS ({receipt_valid_for_edition})
          OR EXISTS ({receipt_valid_for_import})
        BEGIN SELECT RAISE(ABORT,'receipt reference ancestry'); END;""",
        """CREATE TRIGGER candidate_receipt_provenance_immutable_update
        BEFORE UPDATE ON catalog_file_receipts
        WHEN EXISTS (
            SELECT 1 FROM catalog_editions AS edition
            JOIN published_catalog_editions AS publication USING (edition_id)
            WHERE edition.file_receipt_id=OLD.file_receipt_id
        ) OR EXISTS (
            SELECT 1 FROM catalog_imports AS run
            WHERE run.file_receipt_id=OLD.file_receipt_id
              AND run.status IN ('succeeded','failed')
        )
        BEGIN SELECT RAISE(ABORT,'published receipt provenance is immutable'); END;""",
        """CREATE TRIGGER candidate_terminal_import_update
        BEFORE UPDATE ON catalog_imports
        WHEN OLD.status IN ('succeeded','failed')
        BEGIN SELECT RAISE(ABORT,'terminal catalog imports are immutable'); END;""",
        """CREATE TRIGGER candidate_terminal_import_delete
        BEFORE DELETE ON catalog_imports
        WHEN OLD.status IN ('succeeded','failed')
        BEGIN SELECT RAISE(ABORT,'terminal catalog imports are immutable'); END;""",
        """CREATE TRIGGER candidate_catalog_publisher_receipt_ancestry_update
        BEFORE UPDATE OF publisher_id ON catalogs
        WHEN EXISTS (
            SELECT 1 FROM catalog_editions AS edition
            JOIN catalog_file_receipts AS receipt ON receipt.file_receipt_id=edition.file_receipt_id
            JOIN catalog_fetch_attempts AS attempt USING (fetch_attempt_id)
            WHERE edition.catalog_id=OLD.catalog_id
              AND attempt.publisher_id<>NEW.publisher_id
        ) OR EXISTS (
            SELECT 1 FROM catalog_imports AS run
            JOIN catalog_file_receipts AS receipt ON receipt.file_receipt_id=run.file_receipt_id
            JOIN catalog_fetch_attempts AS attempt USING (fetch_attempt_id)
            WHERE run.catalog_id=OLD.catalog_id
              AND attempt.publisher_id<>NEW.publisher_id
        )
        BEGIN SELECT RAISE(ABORT,'catalog publisher conflicts with receipt ancestry'); END;""",
    ]
    for table in ('catalog_fetch_headers', 'catalog_fetch_hashes'):
        for operation, aliases in (('INSERT', ('NEW',)),
                                   ('UPDATE', ('OLD', 'NEW')),
                                   ('DELETE', ('OLD',))):
            condition = ' OR '.join(
                _receipt_use_condition(f'{alias}.fetch_attempt_id') for alias in aliases)
            guards.append(f"""CREATE TRIGGER candidate_fetch_{table}_sealed_{operation.lower()}
                BEFORE {operation} ON {table}
                WHEN {condition}
                BEGIN SELECT RAISE(ABORT,'used receipt acquisition evidence is immutable'); END;""")
    guards.extend(_terminal_message_guards(connection))
    return guards, []
