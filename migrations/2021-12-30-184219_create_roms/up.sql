CREATE TABLE roms (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name        TEXT   NOT NULL
                        CONSTRAINT unique_name UNIQUE,
    size        INTEGER,
    md5         BLOB,
    sha1        BLOB,
    crc         BLOB,
    date        DATE,
    updated_at  DATETIME,
    inserted_at DATETIME,
    game_id     INTEGER REFERENCES games (id) 
);
