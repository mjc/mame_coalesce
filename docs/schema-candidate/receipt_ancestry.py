"""Generate reverse guards for receipt provenance links."""


def sql():
    """Return (guards, audits) for receipt provenance.

    The shared schema owns every column and ordinary FK. These checks only
    enforce agreement between receipt references and their owners. Existing
    shared audits already report edition and import receipt ancestry.
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
    return guards, []
