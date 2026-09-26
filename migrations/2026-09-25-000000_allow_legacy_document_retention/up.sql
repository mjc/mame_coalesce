-- Legacy identity rows remain immutable. Their retained bytes live separately,
-- so hydrating a row never requires weakening the document immutability trigger.
CREATE TABLE legacy_document_payloads (
    document_key TEXT PRIMARY KEY NOT NULL
        REFERENCES documents (document_key) ON DELETE RESTRICT,
    sha256       BLOB NOT NULL CHECK (length(sha256) = 32),
    sha1         BLOB NOT NULL CHECK (length(sha1) = 20),
    byte_length  INTEGER NOT NULL CHECK (byte_length >= 0),
    payload      BLOB NOT NULL,
    format_hint  TEXT NOT NULL,
    CHECK (document_key = 'sha256:' || lower(hex(sha256))),
    CHECK (byte_length = length(payload))
);

CREATE TRIGGER legacy_document_payloads_require_legacy_document
BEFORE INSERT ON legacy_document_payloads
WHEN NOT EXISTS (
    SELECT 1 FROM documents
    WHERE document_key = NEW.document_key
      AND retention_status = 'unavailable'
      AND sha256 IS NULL
      AND payload IS NULL
      AND (sha1 IS NULL OR sha1 = NEW.sha1)
      AND (byte_length IS NULL OR byte_length = NEW.byte_length)
)
BEGIN
    SELECT RAISE(ABORT, 'legacy payload does not match an immutable document');
END;

CREATE TRIGGER legacy_document_payloads_reject_existing_key
BEFORE INSERT ON legacy_document_payloads
WHEN EXISTS (
    SELECT 1 FROM legacy_document_payloads
    WHERE document_key = NEW.document_key
)
BEGIN
    SELECT RAISE(ABORT, 'legacy document payloads are immutable');
END;

CREATE TRIGGER legacy_document_payloads_are_immutable_update
BEFORE UPDATE ON legacy_document_payloads
BEGIN
    SELECT RAISE(ABORT, 'legacy document payloads are immutable');
END;

CREATE TRIGGER legacy_document_payloads_are_immutable_delete
BEFORE DELETE ON legacy_document_payloads
BEGIN
    SELECT RAISE(ABORT, 'legacy document payloads are immutable');
END;
