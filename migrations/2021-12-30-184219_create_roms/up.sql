CREATE TABLE roms (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name        TEXT   NOT NULL
                        CONSTRAINT unique_name UNIQUE,
    size        INTEGER NOT NULL,
    md5         BLOB NOT NULL,
    sha1        BLOB NOT NULL,
    crc         BLOB NOT NULL,
    date        DATE,
    updated_at  DATETIME,
    inserted_at DATETIME,
    game_id     INTEGER REFERENCES games (id) 
);
