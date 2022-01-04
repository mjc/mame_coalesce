CREATE TABLE rom_files (
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    parent_path TEXT   NOT NULL,
    path       TEXT    NOT NULL CONSTRAINT unique_path UNIQUE,
    name       TEXT    CONSTRAINT file_name NOT NULL,
    crc        BLOB    NOT NULL,
    sha1       BLOB    NOT NULL,
    md5        BLOB    NOT NULL,
    in_archive BOOLEAN NOT NULL,
    rom_id             INTEGER CONSTRAINT file_rom REFERENCES roms (id)
);
