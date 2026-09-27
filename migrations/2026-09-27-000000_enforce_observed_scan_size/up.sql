DROP TRIGGER rom_files_scan_provenance_insert;
DROP TRIGGER rom_files_scan_provenance_update;

UPDATE rom_files
SET scan_root = NULL,
    scan_run = NULL,
    observed_size = NULL,
    source_fingerprint = NULL,
    scan_provenance = NULL
WHERE NOT (
    (scan_root IS NULL AND scan_run IS NULL AND observed_size IS NULL
        AND source_fingerprint IS NULL AND scan_provenance IS NULL)
    OR
    (scan_root IS NOT NULL AND scan_run IS NOT NULL AND observed_size IS NOT NULL
        AND source_fingerprint IS NOT NULL AND scan_provenance IS NOT NULL)
);

CREATE TRIGGER rom_files_scan_provenance_insert
BEFORE INSERT ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL
        AND NEW.observed_size IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL
        AND NEW.observed_size IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan metadata must be stored together');
END;

CREATE TRIGGER rom_files_scan_provenance_update
BEFORE UPDATE OF scan_root, scan_run, observed_size, source_fingerprint, scan_provenance ON rom_files
WHEN NOT (
    (NEW.scan_root IS NULL AND NEW.scan_run IS NULL
        AND NEW.source_fingerprint IS NULL AND NEW.scan_provenance IS NULL
        AND NEW.observed_size IS NULL)
    OR
    (NEW.scan_root IS NOT NULL AND NEW.scan_run IS NOT NULL
        AND NEW.source_fingerprint IS NOT NULL AND NEW.scan_provenance IS NOT NULL
        AND NEW.observed_size IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'scan metadata must be stored together');
END;
