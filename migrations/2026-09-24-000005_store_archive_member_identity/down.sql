DROP TRIGGER rom_files_archive_identity_update;
DROP TRIGGER rom_files_archive_identity_insert;
ALTER TABLE rom_files DROP COLUMN archive_member_index;
ALTER TABLE rom_files DROP COLUMN archive_backend;
