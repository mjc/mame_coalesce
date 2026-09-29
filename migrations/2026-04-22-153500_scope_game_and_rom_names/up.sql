PRAGMA defer_foreign_keys = ON;

CREATE TEMP TABLE games_backup AS SELECT * FROM games;
CREATE TEMP TABLE roms_backup AS SELECT * FROM roms;
CREATE TEMP TABLE rom_files_backup AS SELECT * FROM rom_files;
CREATE TEMP TABLE rebuilt_sequences_backup AS
    SELECT name, seq FROM sqlite_sequence
    WHERE name IN ('games', 'roms', 'rom_files');

DROP TABLE rom_files;
DROP TABLE roms;
DROP TABLE games;

CREATE TABLE games_scoped (
    id           INTEGER PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name         TEXT  NOT NULL,
    is_bios      TEXT,
    clone_of     TEXT,
    rom_of       TEXT,
    sample_of    TEXT,
    board        TEXT,
    rebuildto    TEXT,
    year         TEXT,
    manufacturer TEXT,
    data_file_id INTEGER CONSTRAINT data_file_id_constraint REFERENCES data_files (id),
    parent_id    INTEGER CONSTRAINT parent_clone_constraint REFERENCES games_scoped (id)
);

INSERT INTO games_scoped (
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
FROM games_backup;

ALTER TABLE games_scoped RENAME TO games;

CREATE TABLE roms_scoped (
    id              INTEGER PRIMARY KEY AUTOINCREMENT
                             NOT NULL,
    name            TEXT    NOT NULL,
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

INSERT INTO roms_scoped (
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
FROM roms_backup;

ALTER TABLE roms_scoped RENAME TO roms;

CREATE TABLE rom_files (
    id              INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    parent_path     TEXT    NOT NULL,
    parent_game_name TEXT,
    path            TEXT    NOT NULL,
    name            TEXT    CONSTRAINT file_name NOT NULL,
    crc             BLOB,
    sha1            BLOB    NOT NULL,
    md5             BLOB,
    xxhash3         BLOB    NOT NULL,
    in_archive      BOOLEAN NOT NULL,
    rom_id          INTEGER CONSTRAINT file_rom REFERENCES roms (id)
);

INSERT INTO rom_files
SELECT * FROM rom_files_backup;

UPDATE sqlite_sequence
SET seq = MAX(
    seq,
    (SELECT seq FROM rebuilt_sequences_backup WHERE name = sqlite_sequence.name)
)
WHERE name IN (SELECT name FROM rebuilt_sequences_backup);

INSERT INTO sqlite_sequence (name, seq)
SELECT saved.name, saved.seq
FROM rebuilt_sequences_backup AS saved
WHERE NOT EXISTS (
    SELECT 1 FROM sqlite_sequence WHERE name = saved.name
);

CREATE INDEX game_name ON games (name);
CREATE UNIQUE INDEX games_data_file_name_unique ON games (data_file_id, name);
CREATE INDEX games_parent_id_relation_index ON games (parent_id);
CREATE INDEX games_data_file_id_relation_index ON games (data_file_id);
CREATE INDEX md5_index ON roms (md5);
CREATE INDEX sha1_index ON roms (sha1);
CREATE INDEX crc_index ON roms (crc);
CREATE UNIQUE INDEX roms_game_name_unique ON roms (game_id, name);
CREATE INDEX roms_game_id_relation_index ON roms (game_id);
CREATE INDEX rom_file_rom_id_relation_index ON rom_files (rom_id);
CREATE INDEX rom_file_sha1_index ON rom_files (sha1);
CREATE INDEX rom_file_xxhash3_index ON rom_files (xxhash3);

DROP TABLE rom_files_backup;
DROP TABLE roms_backup;
DROP TABLE games_backup;
DROP TABLE rebuilt_sequences_backup;
