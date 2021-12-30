CREATE TABLE data_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT
                        NOT NULL,
    build       STRING  NOT NULL,
    debug       BOOLEAN,
    file_name   STRING,
    name        STRING  UNIQUE,
    description STRING,
    category    STRING,
    version     STRING,
    author      STRING,
    email       STRING,
    homepage    STRING,
    url         STRING
);
