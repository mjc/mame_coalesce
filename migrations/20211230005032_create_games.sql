CREATE TABLE games (
    id           INTEGER PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name         STRING  NOT NULL
                         CONSTRAINT unique_name UNIQUE,
    is_bios      BOOLEAN,
    clone_of     INTEGER,
    rom_of       INTEGER,
    sample_of    INTEGER,
    board        STRING,
    rebuildto    STRING,
    year         DATE,
    manufacturer STRING,
    data_file_id         CONSTRAINT data_file_id_constraint REFERENCES data_files (id) 
);
