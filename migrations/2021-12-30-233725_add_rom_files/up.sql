CREATE TABLE rom_files (
    id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    parent_path TEXT   NOT NULL,
    parent_game_name TEXT,
    path       TEXT    NOT NULL,
    name       TEXT    CONSTRAINT file_name NOT NULL,
    crc        BLOB    ,
    sha1       BLOB    NOT NULL,
    md5        BLOB    ,
    in_archive BOOLEAN NOT NULL,
    rom_id             INTEGER CONSTRAINT file_rom REFERENCES roms (id)
);
