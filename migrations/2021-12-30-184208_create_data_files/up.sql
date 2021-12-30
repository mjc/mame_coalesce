CREATE TABLE data_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT
                        NOT NULL,
    build       TEXT  NOT NULL,
    debug       BOOLEAN,
    file_name   TEXT,
    name        TEXT  UNIQUE,
    description TEXT,
    category    TEXT,
    version     TEXT,
    author      TEXT,
    email       TEXT,
    homepage    TEXT,
    url         TEXT
);
