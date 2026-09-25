ALTER TABLE rom_files
    ADD COLUMN scan_root TEXT;

ALTER TABLE rom_files
    ADD COLUMN scan_run TEXT;

ALTER TABLE rom_files
    ADD COLUMN observed_size BIGINT
        CHECK (observed_size IS NULL OR observed_size >= 0);

ALTER TABLE rom_files
    ADD COLUMN source_fingerprint BLOB
        CHECK (source_fingerprint IS NULL OR length(source_fingerprint) = 20);

ALTER TABLE rom_files
    ADD COLUMN scan_provenance TEXT
        CHECK (scan_provenance IS NULL OR scan_provenance = 'streamed_sha1_xxh3_v1');

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

CREATE INDEX rom_files_scan_root_index ON rom_files (scan_root);
