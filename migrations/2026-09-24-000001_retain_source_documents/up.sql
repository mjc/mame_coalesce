ALTER TABLE documents
    ADD COLUMN sha256 BLOB CHECK (sha256 IS NULL OR length(sha256) = 32);
ALTER TABLE documents ADD COLUMN payload BLOB;
ALTER TABLE documents ADD COLUMN format_hint TEXT;
ALTER TABLE documents
    ADD COLUMN retention_status TEXT NOT NULL DEFAULT 'unavailable'
        CHECK (retention_status IN ('unavailable', 'retained'));

CREATE UNIQUE INDEX documents_sha256_unique ON documents (sha256)
    WHERE sha256 IS NOT NULL;
CREATE INDEX documents_byte_length_index ON documents (byte_length);

ALTER TABLE acquisitions ADD COLUMN transport_metadata_json TEXT;
ALTER TABLE acquisitions ADD COLUMN expected_sha256 BLOB
    CHECK (expected_sha256 IS NULL OR length(expected_sha256) = 32);
ALTER TABLE acquisitions
    ADD COLUMN verification_status TEXT NOT NULL DEFAULT 'unverified'
        CHECK (verification_status IN ('verified', 'unverified', 'failed'));

CREATE UNIQUE INDEX acquisitions_key_document_source_unique
    ON acquisitions (acquisition_key, document_key, source_key);

CREATE TABLE acquisition_attempts (
    attempt_key             TEXT PRIMARY KEY NOT NULL,
    source_key              TEXT NOT NULL REFERENCES publishing_sources (source_key) ON DELETE RESTRICT,
    source_uri              TEXT,
    method                  TEXT,
    attempted_at            DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    transport_metadata_json TEXT,
    expected_sha256         BLOB CHECK (expected_sha256 IS NULL OR length(expected_sha256) = 32),
    outcome                 TEXT NOT NULL CHECK (outcome IN ('retained', 'failed')),
    verification_status     TEXT NOT NULL
                                CHECK (verification_status IN ('verified', 'unverified', 'rejected')),
    document_key            TEXT REFERENCES documents (document_key) ON DELETE RESTRICT,
    acquisition_key         TEXT,
    diagnostic              TEXT,
    FOREIGN KEY (acquisition_key, document_key, source_key)
        REFERENCES acquisitions (acquisition_key, document_key, source_key) ON DELETE RESTRICT,
    CHECK (
        (outcome = 'retained' AND verification_status IN ('verified', 'unverified')
            AND document_key IS NOT NULL AND acquisition_key IS NOT NULL AND diagnostic IS NULL)
        OR
        (outcome = 'failed' AND verification_status = 'rejected'
            AND document_key IS NULL AND acquisition_key IS NULL AND diagnostic IS NOT NULL)
    )
);

CREATE INDEX acquisition_attempts_source_key_index ON acquisition_attempts (source_key);
CREATE INDEX acquisition_attempts_document_key_index ON acquisition_attempts (document_key);
CREATE INDEX acquisition_attempts_acquisition_key_index ON acquisition_attempts (acquisition_key);

CREATE TRIGGER retained_documents_require_payload_insert
BEFORE INSERT ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.payload IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL
        OR length(NEW.payload) != NEW.byte_length)
BEGIN
    SELECT RAISE(ABORT, 'retained document payload metadata is incomplete');
END;

CREATE TRIGGER retained_documents_require_payload_update
BEFORE UPDATE ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.payload IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL
        OR length(NEW.payload) != NEW.byte_length)
BEGIN
    SELECT RAISE(ABORT, 'retained document payload metadata is incomplete');
END;

CREATE TRIGGER documents_are_immutable_update
BEFORE UPDATE ON documents
BEGIN
    SELECT RAISE(ABORT, 'source documents are immutable');
END;

CREATE TRIGGER documents_are_immutable_delete
BEFORE DELETE ON documents
BEGIN
    SELECT RAISE(ABORT, 'source documents are immutable');
END;

CREATE TRIGGER documents_are_immutable_insert
BEFORE INSERT ON documents
WHEN EXISTS (
    SELECT 1 FROM documents
    WHERE document_key = NEW.document_key
       OR (NEW.sha256 IS NOT NULL AND sha256 = NEW.sha256)
)
BEGIN
    SELECT RAISE(ABORT, 'source documents are immutable');
END;

CREATE TRIGGER acquisitions_are_immutable_update
BEFORE UPDATE ON acquisitions
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;

CREATE TRIGGER acquisitions_are_immutable_delete
BEFORE DELETE ON acquisitions
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;

CREATE TRIGGER acquisitions_are_immutable_insert
BEFORE INSERT ON acquisitions
WHEN EXISTS (SELECT 1 FROM acquisitions WHERE acquisition_key = NEW.acquisition_key)
BEGIN
    SELECT RAISE(ABORT, 'acquisition provenance is immutable');
END;

CREATE TRIGGER acquisition_attempts_are_immutable_update
BEFORE UPDATE ON acquisition_attempts
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;

CREATE TRIGGER acquisition_attempts_are_immutable_delete
BEFORE DELETE ON acquisition_attempts
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;

CREATE TRIGGER acquisition_attempts_are_immutable_insert
BEFORE INSERT ON acquisition_attempts
WHEN EXISTS (SELECT 1 FROM acquisition_attempts WHERE attempt_key = NEW.attempt_key)
BEGIN
    SELECT RAISE(ABORT, 'acquisition attempts are immutable');
END;
