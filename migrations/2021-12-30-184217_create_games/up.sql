CREATE TABLE games (
    id           INTEGER PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name         TEXT  NOT NULL
                         CONSTRAINT unique_name UNIQUE,
    is_bios      TEXT,
    clone_of     INTEGER,
    rom_of       INTEGER,
    sample_of    INTEGER,
    board        TEXT,
    rebuildto    TEXT,
    year         DATE,
    manufacturer TEXT,
    data_file_id         INTEGER CONSTRAINT data_file_id_constraint REFERENCES data_files (id) 
);
