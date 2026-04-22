PRAGMA foreign_keys = OFF;

DROP INDEX roms_game_name_unique;

CREATE TABLE roms_global_name (
    id              INTEGER PRIMARY KEY AUTOINCREMENT
                             NOT NULL,
    name            TEXT    NOT NULL
                            CONSTRAINT unique_name UNIQUE,
    size            INTEGER NOT NULL,
    md5             BLOB    NOT NULL,
    sha1            BLOB    NOT NULL,
    crc             BLOB    NOT NULL,
    date            DATE,
    updated_at      DATETIME,
    inserted_at     DATETIME,
    game_id         INTEGER REFERENCES games (id),
    archive_file_id INTEGER REFERENCES archive_files (id)
);

INSERT INTO roms_global_name (
    id,
    name,
    size,
    md5,
    sha1,
    crc,
    date,
    updated_at,
    inserted_at,
    game_id,
    archive_file_id
)
SELECT
    id,
    name,
    size,
    md5,
    sha1,
    crc,
    date,
    updated_at,
    inserted_at,
    game_id,
    archive_file_id
FROM roms;

DROP TABLE roms;
ALTER TABLE roms_global_name RENAME TO roms;

DROP INDEX games_data_file_name_unique;

CREATE TABLE games_global_name (
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
    data_file_id INTEGER CONSTRAINT data_file_id_constraint REFERENCES data_files (id),
    parent_id    INTEGER CONSTRAINT parent_clone_constraint REFERENCES games (id)
);

INSERT INTO games_global_name (
    id,
    name,
    is_bios,
    clone_of,
    rom_of,
    sample_of,
    board,
    rebuildto,
    year,
    manufacturer,
    data_file_id,
    parent_id
)
SELECT
    id,
    name,
    is_bios,
    clone_of,
    rom_of,
    sample_of,
    board,
    rebuildto,
    year,
    manufacturer,
    data_file_id,
    parent_id
FROM games;

DROP TABLE games;
ALTER TABLE games_global_name RENAME TO games;

CREATE INDEX game_name ON games (
    name
);

CREATE INDEX games_parent_id_relation_index ON games (
    parent_id
);

CREATE INDEX games_data_file_id_relation_index ON games (
    data_file_id
);

CREATE INDEX md5_index ON roms (
    md5
);

CREATE INDEX sha1_index ON roms (
    sha1
);

CREATE INDEX crc_index ON roms (
    crc
);

CREATE INDEX roms_game_id_relation_index ON roms (
    game_id
);

PRAGMA foreign_keys = ON;
