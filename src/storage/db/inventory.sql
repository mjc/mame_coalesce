-- Rebuildable physical scan and archive inventory, independent of catalog identity.
CREATE TABLE rom_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    parent_path     TEXT    NOT NULL,
    parent_game_name TEXT,
    path            TEXT    NOT NULL,
    name            TEXT    CONSTRAINT file_name NOT NULL,
    crc             BLOB,
    sha1            BLOB    NOT NULL,
    md5             BLOB,
    xxhash3         BLOB    NOT NULL,
    in_archive      BOOLEAN NOT NULL
, archive_backend TEXT
        CHECK (archive_backend IS NULL OR archive_backend IN ('zip', '7z', 'rar')), archive_member_index BIGINT
        CHECK (archive_member_index IS NULL OR archive_member_index >= 0), scan_root TEXT, scan_run TEXT, observed_size BIGINT
        CHECK (observed_size IS NULL OR observed_size >= 0), source_fingerprint BLOB
        CHECK (source_fingerprint IS NULL OR length(source_fingerprint) = 20), scan_provenance TEXT
        CHECK (scan_provenance IS NULL OR scan_provenance = 'streamed_sha1_xxh3_v1'), bare_file_cache_stamp BLOB
        CHECK (bare_file_cache_stamp IS NULL OR length(bare_file_cache_stamp) = 32), cache_reused BOOLEAN NOT NULL DEFAULT FALSE
        CHECK (cache_reused IN (FALSE, TRUE)), physical_path TEXT);

CREATE INDEX rom_file_sha1_index ON rom_files (sha1);
CREATE INDEX rom_file_xxhash3_index ON rom_files (xxhash3);
CREATE INDEX rom_files_scan_root_index ON rom_files (scan_root);

CREATE TRIGGER rom_files_archive_identity_insert
BEFORE INSERT ON rom_files
WHEN (NEW.archive_backend IS NULL) != (NEW.archive_member_index IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'archive backend and member index must be stored together');
END;
CREATE TRIGGER rom_files_archive_identity_update
BEFORE UPDATE OF archive_backend, archive_member_index ON rom_files
WHEN (NEW.archive_backend IS NULL) != (NEW.archive_member_index IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'archive backend and member index must be stored together');
END;
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
