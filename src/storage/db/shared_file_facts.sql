-- Accepted whole-file facts belong to a ROM, not to every list mentioning it.
-- Native declarations retain their spelling, scope, position and provenance.
-- Membership is maintained by bounded import writes and atomic review seals.
CREATE TABLE shared_file_sizes (
    content_uuid BLOB NOT NULL REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    size INTEGER NOT NULL CHECK (typeof(size) = 'integer' AND size >= 0),
    PRIMARY KEY (content_uuid, size)
) WITHOUT ROWID;
CREATE TABLE shared_file_hashes (
    content_uuid BLOB NOT NULL REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    digest_id INTEGER NOT NULL REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    PRIMARY KEY (content_uuid, digest_id)
) WITHOUT ROWID;
CREATE INDEX shared_file_hash_lookup ON shared_file_hashes(digest_id, content_uuid);

-- Seal direct SQL imports too. Only this snapshot's new declarations are read;
-- normal writers also record completed batches before the next resolution.
CREATE TRIGGER shared_file_facts_snapshot_publication AFTER INSERT ON snapshot_publications
BEGIN
    INSERT OR IGNORE INTO shared_file_sizes(content_uuid,size)
    SELECT DISTINCT canonical.content_uuid,sizes.size
    FROM catalog_set_groups AS groups
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
    CROSS JOIN asset_occurrences AS owner ON owner.record_id=sets.set_id
    CROSS JOIN canonical_occurrence_content AS canonical ON canonical.occurrence_id=owner.occurrence_id
    CROSS JOIN accepted_file_size_assertions AS sizes ON sizes.occurrence_id=owner.occurrence_id
    WHERE groups.snapshot_key=NEW.snapshot_key;
    INSERT OR IGNORE INTO shared_file_hashes(content_uuid,digest_id)
    SELECT DISTINCT assertion.content_uuid,assertion.digest_id
    FROM catalog_set_groups AS groups
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
    CROSS JOIN asset_occurrences AS owner ON owner.record_id=sets.set_id
    CROSS JOIN catalog_content_digest_assertions AS assertion ON assertion.occurrence_id=owner.occurrence_id
    WHERE groups.snapshot_key=NEW.snapshot_key;
END;
