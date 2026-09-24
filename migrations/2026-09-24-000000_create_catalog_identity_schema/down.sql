DROP TRIGGER catalog_snapshots_are_immutable_delete;
DROP TRIGGER catalog_snapshots_are_immutable_update;
DROP TRIGGER catalog_snapshots_are_immutable_insert;

DROP INDEX import_runs_snapshot_key_index;
DROP INDEX import_runs_document_key_index;
DROP INDEX import_runs_catalog_key_index;
DROP INDEX snapshots_parent_key_index;
DROP INDEX snapshots_interpretation_key_index;
DROP INDEX snapshots_document_key_index;
DROP INDEX snapshots_catalog_key_index;
DROP INDEX acquisitions_document_key_index;
DROP INDEX acquisitions_source_key_index;
DROP INDEX documents_sha1_index;
DROP INDEX catalogs_source_key_index;

DROP TABLE import_runs;
DROP TABLE catalog_snapshots;
DROP TABLE parser_interpretations;
DROP TABLE acquisitions;
DROP TABLE documents;
DROP TABLE catalogs;
DROP TABLE publishing_sources;
