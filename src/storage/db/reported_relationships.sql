CREATE TABLE catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL CHECK (typeof(relationship_id) = 'integer'),
    assertion_key TEXT NOT NULL UNIQUE,
    origin TEXT NOT NULL CHECK (origin IN ('source', 'derived', 'user')),
    snapshot_key TEXT REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (origin <> 'source' OR snapshot_key IS NOT NULL)
);

CREATE TABLE reported_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    source_reference_kind TEXT NOT NULL CHECK (source_reference_kind IN (
        'mame_cloneof', 'mame_romof', 'mame_sampleof', 'mame_device_ref',
        'mame_rom_merge', 'mame_disk_merge',
        'logiqx_cloneof', 'logiqx_romof', 'logiqx_sampleof', 'logiqx_device_ref',
        'logiqx_rom_merge', 'logiqx_disk_merge',
        'clrmamepro_cloneof', 'clrmamepro_sampleof', 'clrmamepro_rom_merge',
        'software_cloneof', 'no_intro_dat_cloneof', 'no_intro_dat_cloneofid',
        'no_intro_database_archive_clone', 'no_intro_database_archive_mergeof'
    )),
    UNIQUE (relationship_id, source_reference_kind)
);

CREATE INDEX catalog_relationships_snapshot_origin_index
    ON catalog_relationships(snapshot_key, origin, relationship_id);

CREATE TRIGGER catalog_relationships_insert_guard
BEFORE INSERT ON catalog_relationships
WHEN EXISTS (
        SELECT 1 FROM catalog_relationships
        WHERE relationship_id = NEW.relationship_id OR assertion_key = NEW.assertion_key
    )
 OR EXISTS (SELECT 1 FROM relationship_assertions WHERE assertion_key = NEW.assertion_key)
 OR (NEW.snapshot_key IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM catalog_snapshots WHERE snapshot_key = NEW.snapshot_key
    ))
 OR (NEW.origin = 'source' AND (
        NEW.snapshot_key IS NULL
        OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key = NEW.snapshot_key)
    ))
BEGIN
    SELECT RAISE(ABORT, 'catalog relationship identity is invalid, reused, or published');
END;

CREATE TRIGGER catalog_relationships_immutable_update
BEFORE UPDATE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'catalog relationship identities are immutable'); END;

CREATE TRIGGER catalog_relationships_immutable_delete
BEFORE DELETE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'catalog relationship identities are immutable'); END;

CREATE TRIGGER reported_catalog_relationships_insert_guard
BEFORE INSERT ON reported_catalog_relationships
WHEN EXISTS (
        SELECT 1 FROM reported_catalog_relationships
        WHERE relationship_id = NEW.relationship_id
    )
 OR NOT EXISTS (
        SELECT 1 FROM catalog_relationships AS identity
        WHERE identity.relationship_id = NEW.relationship_id
          AND identity.origin = 'source'
          AND identity.snapshot_key IS NOT NULL
          AND NOT EXISTS (
              SELECT 1 FROM snapshot_publications AS publication
              WHERE publication.snapshot_key = identity.snapshot_key
          )
    )
BEGIN
    SELECT RAISE(ABORT, 'reported relationship requires an unused unpublished source identity');
END;

CREATE TRIGGER reported_catalog_relationships_immutable_update
BEFORE UPDATE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported catalog relationships are immutable'); END;

CREATE TRIGGER reported_catalog_relationships_immutable_delete
BEFORE DELETE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported catalog relationships are immutable'); END;
