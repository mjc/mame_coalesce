ALTER TABLE rom_files
    ADD COLUMN archive_backend TEXT
        CHECK (archive_backend IS NULL OR archive_backend IN ('zip', '7z', 'rar'));

ALTER TABLE rom_files
    ADD COLUMN archive_member_index BIGINT
        CHECK (archive_member_index IS NULL OR archive_member_index >= 0);

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
