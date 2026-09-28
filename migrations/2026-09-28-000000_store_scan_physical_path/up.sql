ALTER TABLE rom_files ADD COLUMN physical_path TEXT;

UPDATE rom_files
SET physical_path = path
WHERE physical_path IS NULL;
