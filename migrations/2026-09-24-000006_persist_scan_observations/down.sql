DROP INDEX rom_files_scan_root_index;
DROP TRIGGER rom_files_scan_provenance_update;
DROP TRIGGER rom_files_scan_provenance_insert;
ALTER TABLE rom_files DROP COLUMN scan_provenance;
ALTER TABLE rom_files DROP COLUMN source_fingerprint;
ALTER TABLE rom_files DROP COLUMN observed_size;
ALTER TABLE rom_files DROP COLUMN scan_run;
ALTER TABLE rom_files DROP COLUMN scan_root;
