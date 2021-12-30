CREATE TABLE data_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT
                        NOT NULL,
    build       TEXT,
    debug       TEXT,
    file_name   TEXT,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    category    TEXT,
    version     TEXT,
    author      TEXT,
    email       TEXT,
    homepage    TEXT,
    url         TEXT
);
