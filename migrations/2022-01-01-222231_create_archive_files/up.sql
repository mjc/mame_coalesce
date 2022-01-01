CREATE TABLE archive_files (
    id   INTEGER PRIMARY KEY AUTOINCREMENT
                 NOT NULL,
    path TEXT    NOT NULL,
    sha1 BLOB    NOT NULL
);

ALTER TABLE roms ADD archive_file_id INTEGER  REFERENCES archive_files (id);