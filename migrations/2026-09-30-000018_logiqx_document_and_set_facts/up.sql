CREATE TABLE logiqx_document_facts (
    snapshot_key TEXT PRIMARY KEY NOT NULL,
    build TEXT,
    debug TEXT,
    file_name TEXT,
    sha1 BLOB,
    header_name TEXT NOT NULL,
    header_description TEXT,
    header_version TEXT,
    header_date TEXT,
    header_author TEXT,
    header_email TEXT,
    header_homepage TEXT,
    header_url TEXT,
    header_comment TEXT,
    header_category TEXT,
    FOREIGN KEY (snapshot_key) REFERENCES catalog_snapshots (snapshot_key) ON DELETE RESTRICT,
    CHECK (sha1 IS NULL OR length(sha1) = 20)
);

CREATE TABLE logiqx_set_facts (
    snapshot_key TEXT NOT NULL,
    set_name TEXT NOT NULL,
    source_file TEXT,
    is_bios TEXT,
    board TEXT,
    rebuild_to TEXT,
    description TEXT,
    year TEXT,
    manufacturer TEXT,
    PRIMARY KEY (snapshot_key, set_name),
    FOREIGN KEY (snapshot_key, set_name)
        REFERENCES snapshot_sets (snapshot_key, set_name) ON DELETE RESTRICT
);
