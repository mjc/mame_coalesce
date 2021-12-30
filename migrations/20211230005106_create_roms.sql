CREATE TABLE roms (
    id          INTEGER  PRIMARY KEY AUTOINCREMENT
                         NOT NULL,
    name        STRING   UNIQUE
                         NOT NULL,
    size        INTEGER,
    md5         BLOB,
    sha1        BLOB,
    crc         BLOB,
    date        DATE,
    updated_at  DATETIME NOT NULL,
    inserted_at DATETIME NOT NULL,
    game_id              REFERENCES games (id) 
);
