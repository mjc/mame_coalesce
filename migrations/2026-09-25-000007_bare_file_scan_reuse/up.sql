ALTER TABLE rom_files
    ADD COLUMN bare_file_cache_stamp BLOB
        CHECK (bare_file_cache_stamp IS NULL OR length(bare_file_cache_stamp) = 32);

ALTER TABLE rom_files
    ADD COLUMN cache_reused BOOLEAN NOT NULL DEFAULT FALSE
        CHECK (cache_reused IN (FALSE, TRUE));
