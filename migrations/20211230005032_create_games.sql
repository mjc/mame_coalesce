CREATE TABLE games (
    id           INTEGER PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name         STRING  UNIQUE NOT NULL,
    is_bios      BOOLEAN,
    clone_of     INTEGER,
    rom_of       INTEGER,
    sample_of    INTEGER,
    board        STRING,
    rebuildto    STRING,
    year         DATE,
    manufacturer STRING,
    data_file_id         REFERENCES data_files (id) 
);
