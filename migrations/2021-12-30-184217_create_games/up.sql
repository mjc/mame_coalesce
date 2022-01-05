CREATE TABLE games (
    id           INTEGER PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name         TEXT  NOT NULL
                         CONSTRAINT unique_name UNIQUE,
    is_bios      TEXT,
    clone_of     TEXT,
    rom_of       TEXT,
    sample_of    TEXT,
    board        TEXT,
    rebuildto    TEXT,
    year         TEXT,
    manufacturer TEXT,
    data_file_id         INTEGER CONSTRAINT data_file_id_constraint REFERENCES data_files (id),
    parent_id   INTEGER CONSTRAINT parent_clone_constraint REFERENCES games (id)
);
