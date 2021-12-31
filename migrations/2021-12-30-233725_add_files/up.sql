CREATE TABLE files (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    path       TEXT    CONSTRAINT file_path NOT NULL
                       CONSTRAINT file_path UNIQUE,
    name       TEXT    CONSTRAINT file_name NOT NULL,
    crc        BLOB    NOT NULL,
    sha1       BLOB    NOT NULL,
    md5        BLOB    NOT NULL,
    in_archive BOOLEAN NOT NULL,
    rom_id             INTEGER CONSTRAINT file_rom REFERENCES roms (id)
);
