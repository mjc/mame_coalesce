-- A registry generation is database-wide, not one namespace per ROM list.
-- A fresh database starts a new generation; paired backups preserve this row.
CREATE TABLE file_id_registries (
    registry_id INTEGER PRIMARY KEY CHECK (registry_id = 1),
    registry_uuid BLOB NOT NULL UNIQUE CHECK (typeof(registry_uuid) = 'blob' AND length(registry_uuid) = 16),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO file_id_registries (registry_id, registry_uuid) VALUES (1, randomblob(16));
CREATE TABLE catalog_contents (
    content_uuid BLOB PRIMARY KEY NOT NULL CHECK (typeof(content_uuid) = 'blob' AND length(content_uuid) = 16),
    registry_id INTEGER NOT NULL DEFAULT 1 REFERENCES file_id_registries(registry_id) ON DELETE RESTRICT
) WITHOUT ROWID;

-- Aliases are supported by actual source occurrences. Never copy a digest or
-- size onto the shared identity, or keep an alias after its evidence disappears.
-- Staged occurrences are visible inside the importing transaction; public
-- reports independently require a published snapshot.
CREATE VIEW catalog_content_digest_assertions AS
SELECT occurrence.content_uuid, assertion.occurrence_id, assertion.digest_id,
       assertion.scope, assertion.provenance, groups.snapshot_key, snapshots.interpretation_key
FROM occurrence_digest_assertions AS assertion
JOIN asset_occurrences AS occurrence USING (occurrence_id)
JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
JOIN catalog_set_groups AS groups USING (set_group_id)
JOIN catalog_snapshots AS snapshots USING (snapshot_key)
WHERE occurrence.content_uuid IS NOT NULL
  AND assertion.provenance = 'source_declared'
  AND assertion.scope IN ('whole_asset', 'whole_file')
  AND occurrence.claim_kind IN ('mame_rom', 'logiqx_rom', 'cmp_rom', 'no_intro_pc_file',
      'no_intro_dat_rom', 'no_intro_database_file', 'software_rom_entry');

-- Only lengths whose native meaning is a complete file participate. Software
-- loading-step lengths and logical disks do not become whole-file lengths.
CREATE VIEW catalog_file_size_assertions AS
SELECT occurrence_id, 'mame_rom_size' AS size_field, size
FROM mame_rom_claims WHERE size IS NOT NULL AND evidence_scope IN ('whole_asset', 'whole_file')
UNION ALL
SELECT occurrence_id, 'logiqx_rom_size', size
FROM logiqx_rom_claims WHERE size IS NOT NULL AND evidence_scope IN ('whole_asset', 'whole_file')
UNION ALL
SELECT occurrence_id, 'cmp_rom_size', size
FROM cmp_rom_claims WHERE size IS NOT NULL AND evidence_scope IN ('whole_asset', 'whole_file')
UNION ALL
SELECT occurrence_id, 'no_intro_pc_file_size', size
FROM no_intro_pc_file_claims WHERE size IS NOT NULL AND evidence_scope IN ('whole_asset', 'whole_file');

CREATE TABLE occurrence_content_conflict_hashes (
    occurrence_id INTEGER NOT NULL,
    candidate_content_uuid BLOB NOT NULL,
    evidence_occurrence_id INTEGER NOT NULL,
    digest_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    provenance TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    PRIMARY KEY (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role),
    FOREIGN KEY (occurrence_id, candidate_content_uuid)
        REFERENCES occurrence_content_conflicts(occurrence_id, candidate_content_uuid) ON DELETE RESTRICT,
    FOREIGN KEY (evidence_occurrence_id, digest_id, scope, provenance)
        REFERENCES occurrence_digest_assertions(occurrence_id, digest_id, scope, provenance) ON DELETE RESTRICT,
    CHECK (role <> 'incoming' OR evidence_occurrence_id = occurrence_id)
) WITHOUT ROWID;
CREATE TABLE occurrence_content_conflict_sizes (
    occurrence_id INTEGER NOT NULL,
    candidate_content_uuid BLOB NOT NULL,
    evidence_occurrence_id INTEGER NOT NULL REFERENCES asset_occurrences(occurrence_id) ON DELETE RESTRICT,
    size_field TEXT NOT NULL CHECK (size_field IN ('mame_rom_size', 'logiqx_rom_size', 'cmp_rom_size', 'no_intro_pc_file_size')),
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    PRIMARY KEY (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role),
    FOREIGN KEY (occurrence_id, candidate_content_uuid)
        REFERENCES occurrence_content_conflicts(occurrence_id, candidate_content_uuid) ON DELETE RESTRICT,
    CHECK (role <> 'incoming' OR evidence_occurrence_id = occurrence_id)
) WITHOUT ROWID;

-- Every recorded conflict is unresolved until an explicit reviewed settlement
-- is implemented. A later weaker claim must not bypass an active dispute.
CREATE VIEW disputed_file_hashes AS
SELECT evidence.digest_id, evidence.scope, evidence.occurrence_id,
       evidence.candidate_content_uuid
FROM occurrence_content_conflict_hashes AS evidence
JOIN digest_values USING (digest_id)
WHERE algorithm IN ('sha1', 'sha256') AND evidence.scope IN ('whole_asset', 'whole_file');
CREATE INDEX disputed_file_hash_lookup
    ON occurrence_content_conflict_hashes(digest_id, scope, candidate_content_uuid);

CREATE TRIGGER registry_generation_immutable_update BEFORE UPDATE ON file_id_registries
BEGIN SELECT RAISE(ABORT, 'identity registry generation is immutable'); END;
CREATE TRIGGER registry_generation_immutable_insert BEFORE INSERT ON file_id_registries
WHEN EXISTS (SELECT 1 FROM file_id_registries WHERE registry_id = NEW.registry_id)
BEGIN SELECT RAISE(ABORT, 'identity registry generation is immutable'); END;
CREATE TRIGGER registry_generation_immutable_delete BEFORE DELETE ON file_id_registries
BEGIN SELECT RAISE(ABORT, 'identity registry generation is immutable'); END;
CREATE TRIGGER catalog_content_identity_immutable_update BEFORE UPDATE ON catalog_contents
BEGIN SELECT RAISE(ABORT, 'issued catalog UUIDs are immutable'); END;
CREATE TRIGGER catalog_content_identity_immutable_delete BEFORE DELETE ON catalog_contents
BEGIN SELECT RAISE(ABORT, 'issued catalog UUIDs are immutable'); END;
CREATE TRIGGER catalog_content_identity_immutable_insert BEFORE INSERT ON catalog_contents
WHEN EXISTS (SELECT 1 FROM catalog_contents WHERE content_uuid = NEW.content_uuid)
BEGIN SELECT RAISE(ABORT, 'issued catalog UUIDs are immutable'); END;
CREATE TRIGGER digest_identity_immutable_update BEFORE UPDATE ON digest_values
BEGIN SELECT RAISE(ABORT, 'interned digest identities are immutable'); END;
CREATE TRIGGER digest_identity_immutable_delete BEFORE DELETE ON digest_values
BEGIN SELECT RAISE(ABORT, 'interned digest identities are immutable'); END;
CREATE TRIGGER digest_identity_immutable_insert BEFORE INSERT ON digest_values
WHEN EXISTS (SELECT 1 FROM digest_values WHERE digest_id = NEW.digest_id)
BEGIN SELECT RAISE(ABORT, 'interned digest identities are immutable'); END;
CREATE TRIGGER published_source_digest_insert BEFORE INSERT ON occurrence_digest_assertions
WHEN NEW.provenance = 'source_declared' AND EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence
    JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
    JOIN catalog_set_groups AS groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence.occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published source digest assertions are immutable'); END;
CREATE TRIGGER occurrence_digest_immutable_update BEFORE UPDATE ON occurrence_digest_assertions
BEGIN SELECT RAISE(ABORT, 'occurrence digest assertions are immutable'); END;
CREATE TRIGGER occurrence_digest_immutable_delete BEFORE DELETE ON occurrence_digest_assertions
BEGIN SELECT RAISE(ABORT, 'occurrence digest assertions are immutable'); END;
CREATE TRIGGER content_conflict_immutable_update BEFORE UPDATE ON occurrence_content_conflicts
BEGIN SELECT RAISE(ABORT, 'source content conflicts are immutable'); END;
CREATE TRIGGER content_conflict_immutable_delete BEFORE DELETE ON occurrence_content_conflicts
BEGIN SELECT RAISE(ABORT, 'source content conflicts are immutable'); END;
CREATE TRIGGER content_conflict_reason_immutable_insert BEFORE INSERT ON occurrence_content_conflicts
WHEN EXISTS (
    SELECT 1 FROM occurrence_content_conflicts
    WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid
      AND reason <> NEW.reason
)
BEGIN SELECT RAISE(ABORT, 'source content conflict reasons are immutable'); END;
CREATE TRIGGER conflict_hash_evidence_owner_insert BEFORE INSERT ON occurrence_content_conflict_hashes
WHEN (NEW.role = 'candidate' AND NOT EXISTS (
    SELECT 1 FROM asset_occurrences
    WHERE occurrence_id = NEW.evidence_occurrence_id AND content_uuid = NEW.candidate_content_uuid
)) OR NEW.provenance <> 'source_declared' OR NEW.scope NOT IN ('whole_asset', 'whole_file')
BEGIN SELECT RAISE(ABORT, 'conflict hashes require qualified source evidence from the named owner'); END;
CREATE TRIGGER conflict_size_evidence_owner_insert BEFORE INSERT ON occurrence_content_conflict_sizes
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_file_size_assertions
    WHERE occurrence_id = NEW.evidence_occurrence_id AND size_field = NEW.size_field
) OR (NEW.role = 'candidate' AND NOT EXISTS (
    SELECT 1 FROM asset_occurrences
    WHERE occurrence_id = NEW.evidence_occurrence_id AND content_uuid = NEW.candidate_content_uuid
))
BEGIN SELECT RAISE(ABORT, 'conflict sizes require a qualified native size from the named owner'); END;
CREATE TRIGGER conflict_hash_evidence_immutable_update BEFORE UPDATE ON occurrence_content_conflict_hashes
BEGIN SELECT RAISE(ABORT, 'conflict hash evidence is immutable'); END;
CREATE TRIGGER conflict_hash_evidence_immutable_delete BEFORE DELETE ON occurrence_content_conflict_hashes
BEGIN SELECT RAISE(ABORT, 'conflict hash evidence is immutable'); END;
CREATE TRIGGER conflict_size_evidence_immutable_update BEFORE UPDATE ON occurrence_content_conflict_sizes
BEGIN SELECT RAISE(ABORT, 'conflict size evidence is immutable'); END;
CREATE TRIGGER conflict_size_evidence_immutable_delete BEFORE DELETE ON occurrence_content_conflict_sizes
BEGIN SELECT RAISE(ABORT, 'conflict size evidence is immutable'); END;
