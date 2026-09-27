DROP TRIGGER rom_files_scan_provenance_update;
DROP TRIGGER rom_files_scan_provenance_insert;

CREATE TRIGGER rom_files_scan_provenance_insert
BEFORE INSERT ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan root, run, fingerprint, and provenance must be stored together');
END;

CREATE TRIGGER rom_files_scan_provenance_update
BEFORE UPDATE OF scan_root, scan_run, source_fingerprint, scan_provenance ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan root, run, fingerprint, and provenance must be stored together');
END;
