DROP TRIGGER documents_are_immutable_insert;
DROP TRIGGER documents_are_immutable_delete;
DROP TRIGGER documents_are_immutable_update;
DROP TRIGGER acquisitions_are_immutable_insert;
DROP TRIGGER acquisitions_are_immutable_delete;
DROP TRIGGER acquisitions_are_immutable_update;
DROP TRIGGER acquisition_attempts_are_immutable_insert;
DROP TRIGGER acquisition_attempts_are_immutable_delete;
DROP TRIGGER acquisition_attempts_are_immutable_update;
DROP TRIGGER retained_documents_require_payload_update;
DROP TRIGGER retained_documents_require_payload_insert;

DROP INDEX acquisition_attempts_acquisition_key_index;
DROP INDEX acquisition_attempts_document_key_index;
DROP INDEX acquisition_attempts_source_key_index;
DROP INDEX acquisitions_key_document_source_unique;
DROP INDEX documents_byte_length_index;
DROP INDEX documents_sha256_unique;

DROP TABLE acquisition_attempts;

ALTER TABLE acquisitions DROP COLUMN verification_status;
ALTER TABLE acquisitions DROP COLUMN expected_sha256;
ALTER TABLE acquisitions DROP COLUMN transport_metadata_json;

ALTER TABLE documents DROP COLUMN retention_status;
ALTER TABLE documents DROP COLUMN format_hint;
ALTER TABLE documents DROP COLUMN payload;
ALTER TABLE documents DROP COLUMN sha256;
