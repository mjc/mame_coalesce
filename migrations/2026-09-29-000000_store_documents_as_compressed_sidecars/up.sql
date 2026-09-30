DROP TRIGGER retained_documents_require_payload_update;
DROP TRIGGER retained_documents_require_payload_insert;
DROP TRIGGER documents_are_immutable_update;

-- Existing developer caches have no supported migration contract. Keep catalog facts,
-- but discard retained byte copies rather than silently rebuilding a sidecar here.
UPDATE documents
SET retention_status = 'unavailable', sha256 = NULL, format_hint = NULL
WHERE retention_status = 'retained';

ALTER TABLE documents ADD COLUMN object_key TEXT;
DROP TABLE legacy_document_payloads;
ALTER TABLE documents DROP COLUMN payload;

CREATE TRIGGER retained_documents_require_object_insert
BEFORE INSERT ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.object_key IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'retained document object metadata is incomplete');
END;

CREATE TRIGGER retained_documents_require_object_update
BEFORE UPDATE ON documents
WHEN NEW.retention_status = 'retained'
    AND (NEW.object_key IS NULL OR NEW.sha256 IS NULL OR NEW.byte_length IS NULL)
BEGIN
    SELECT RAISE(ABORT, 'retained document object metadata is incomplete');
END;

CREATE TRIGGER documents_are_immutable_update
BEFORE UPDATE ON documents
WHEN OLD.retention_status = 'retained'
BEGIN
    SELECT RAISE(ABORT, 'retained source documents are immutable');
END;
