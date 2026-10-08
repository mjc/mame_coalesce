-- DESIGN ONLY. Apply after shared.sql and native typed-owner fragments.
-- This is not production schema and is not a migration.

-- One identity issuer for reported, derived, and user assertions. Source
-- relationships belong to one edition; derived/user identities do not invent
-- source ownership. assertion_key is the stable caller-issued identity.
CREATE TABLE catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL CHECK (relationship_id > 0),
    assertion_key TEXT NOT NULL UNIQUE CHECK (length(CAST(assertion_key AS BLOB)) > 0),
    origin TEXT NOT NULL CHECK (origin IN ('source', 'derived', 'user')),
    edition_id INTEGER REFERENCES catalog_editions(edition_id),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((origin = 'source') = (edition_id IS NOT NULL)),
    UNIQUE (relationship_id, origin)
) STRICT;

-- Exactly one closed reported kind per reported identity. Typed literal
-- declarations live in their native format tables and FK to this row.
CREATE TABLE reported_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    reported_kind TEXT NOT NULL CHECK (reported_kind IN (
        'mame_cloneof', 'mame_romof', 'mame_sampleof', 'mame_device_ref',
        'mame_rom_merge', 'mame_disk_merge',
        'logiqx_cloneof', 'logiqx_romof', 'logiqx_sampleof', 'logiqx_device_ref',
        'logiqx_rom_merge', 'logiqx_disk_merge',
        'clrmamepro_cloneof', 'clrmamepro_sampleof', 'clrmamepro_rom_merge',
        'software_cloneof', 'no_intro_dat_cloneof', 'no_intro_dat_cloneofid',
        'no_intro_database_archive_clone', 'no_intro_database_archive_mergeof',
        'no_intro_pc_clone', 'no_intro_pc_mergeof'
    )),
    UNIQUE (relationship_id, reported_kind)
) STRICT;
CREATE INDEX reported_relationship_kind_lookup
    ON reported_catalog_relationships(reported_kind, relationship_id);

-- Target IDs identify endpoint rows, not endpoint values. Each closed subtype
-- below owns one real FK; names, hash bytes, and UUID facts are never copied.
CREATE TABLE catalog_relationship_targets (
    target_id INTEGER PRIMARY KEY NOT NULL CHECK (target_id > 0),
    target_kind TEXT NOT NULL CHECK (target_kind IN (
        'catalog_set', 'catalog_media_entry', 'shared_file', 'no_intro_archive',
        'declared_hash', 'observed_content_hash', 'unresolved_catalog',
        'external_record'
    )),
    UNIQUE (target_id, target_kind)
) STRICT;

CREATE TABLE catalog_set_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'catalog_set' CHECK (target_kind = 'catalog_set'),
    set_id INTEGER NOT NULL UNIQUE REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT;

CREATE TABLE catalog_media_entry_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'catalog_media_entry' CHECK (target_kind = 'catalog_media_entry'),
    media_entry_id INTEGER NOT NULL UNIQUE REFERENCES catalog_media_entries(media_entry_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT;

-- Archive descriptions are actual native records, not media files or ZIPs.
-- Repeated publisher numbers never substitute for their distinct owner IDs.
CREATE TABLE no_intro_archive_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'no_intro_archive' CHECK (target_kind = 'no_intro_archive'),
    archive_id INTEGER NOT NULL UNIQUE REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;

-- This endpoint retains the issued UUID, including when it later becomes an
-- alias. Consumers may separately follow the append-only redirect relation.
CREATE TABLE shared_file_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'shared_file' CHECK (target_kind = 'shared_file'),
    file_uuid BLOB NOT NULL UNIQUE REFERENCES shared_catalog_files(file_uuid) ON DELETE RESTRICT
        CHECK (typeof(file_uuid) = 'blob' AND length(file_uuid) = 16),
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT;

-- Equal digest bytes share hash_values but these two endpoint identities stay
-- distinct. Neither endpoint allocates or redirects a shared file UUID.
CREATE TABLE declared_hash_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'declared_hash' CHECK (target_kind = 'declared_hash'),
    hash_id INTEGER NOT NULL UNIQUE REFERENCES hash_values(hash_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT;

CREATE TABLE observed_content_hash_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'observed_content_hash' CHECK (target_kind = 'observed_content_hash'),
    hash_id INTEGER NOT NULL UNIQUE REFERENCES hash_values(hash_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT
) STRICT;

CREATE TABLE unresolved_catalog_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'unresolved_catalog' CHECK (target_kind = 'unresolved_catalog'),
    edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id) ON DELETE RESTRICT,
    record_kind TEXT NOT NULL CHECK (record_kind IN ('catalog_set', 'software_item', 'media_entry')),
    declared_name TEXT NOT NULL,
    declared_set_name TEXT,
    declared_list_name TEXT,
    declared_media_order INTEGER,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT,
    CHECK (
        (record_kind = 'catalog_set' AND declared_set_name IS NULL AND declared_list_name IS NULL AND declared_media_order IS NULL)
        OR (record_kind = 'software_item' AND declared_set_name IS NULL AND declared_list_name IS NOT NULL AND declared_media_order IS NULL)
        OR (record_kind = 'media_entry' AND declared_set_name IS NOT NULL AND declared_list_name IS NULL
            AND typeof(declared_media_order) = 'integer' AND declared_media_order >= 0)
    )
) STRICT;
CREATE UNIQUE INDEX unresolved_set_target_key
    ON unresolved_catalog_targets(edition_id, record_kind, declared_name)
    WHERE record_kind = 'catalog_set';
CREATE UNIQUE INDEX unresolved_software_target_key
    ON unresolved_catalog_targets(edition_id, record_kind, declared_list_name, declared_name)
    WHERE record_kind = 'software_item';
CREATE UNIQUE INDEX unresolved_media_target_key
    ON unresolved_catalog_targets(edition_id, record_kind, declared_set_name, declared_media_order, declared_name)
    WHERE record_kind = 'media_entry';

CREATE TABLE external_catalog_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    target_kind TEXT NOT NULL DEFAULT 'external_record' CHECK (target_kind = 'external_record'),
    namespace TEXT NOT NULL CHECK (length(CAST(namespace AS BLOB)) > 0),
    declared_key TEXT NOT NULL,
    FOREIGN KEY (target_id, target_kind)
        REFERENCES catalog_relationship_targets(target_id, target_kind) ON DELETE RESTRICT,
    UNIQUE (namespace, declared_key)
) STRICT;

CREATE TABLE catalog_relationship_rules (
    rule_id INTEGER PRIMARY KEY NOT NULL CHECK (rule_id > 0),
    rule_key TEXT NOT NULL CHECK (length(CAST(rule_key AS BLOB)) > 0),
    revision TEXT NOT NULL CHECK (length(CAST(revision AS BLOB)) > 0),
    description TEXT NOT NULL CHECK (length(CAST(description AS BLOB)) > 0),
    UNIQUE (rule_key, revision)
) STRICT;

CREATE TABLE manual_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    from_target_id INTEGER NOT NULL REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    to_target_id INTEGER NOT NULL REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    CHECK (from_target_id <> to_target_id)
) STRICT;

CREATE TABLE inferred_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    from_target_id INTEGER NOT NULL REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    to_target_id INTEGER NOT NULL REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    rule_id INTEGER NOT NULL REFERENCES catalog_relationship_rules(rule_id) ON DELETE RESTRICT,
    CHECK (from_target_id <> to_target_id)
) STRICT;
CREATE INDEX inferred_relationship_from_target ON inferred_catalog_relationships(from_target_id, relationship_id);
CREATE INDEX inferred_relationship_to_target ON inferred_catalog_relationships(to_target_id, relationship_id);
CREATE INDEX manual_relationship_from_target ON manual_catalog_relationships(from_target_id, relationship_id);
CREATE INDEX manual_relationship_to_target ON manual_catalog_relationships(to_target_id, relationship_id);

CREATE TABLE catalog_relationship_rationales (
    relationship_id INTEGER PRIMARY KEY REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (length(CAST(trim(reason) AS BLOB)) > 0)
) STRICT;

CREATE TABLE catalog_relationship_comparisons (
    relationship_id INTEGER PRIMARY KEY REFERENCES inferred_catalog_relationships(relationship_id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('supports', 'contradicts', 'inconclusive'))
) STRICT;

CREATE TABLE catalog_relationship_comparison_fields (
    relationship_id INTEGER NOT NULL REFERENCES catalog_relationship_comparisons(relationship_id) ON DELETE RESTRICT,
    disposition TEXT NOT NULL CHECK (disposition IN ('support', 'difference', 'unavailable')),
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    field_kind TEXT NOT NULL CHECK (field_kind IN (
        'whole_file_hash', 'file_size', 'set_name', 'media_name', 'source_declaration', 'other'
    )),
    PRIMARY KEY (relationship_id, disposition, list_order),
    UNIQUE (relationship_id, disposition, field_kind)
) STRICT, WITHOUT ROWID;

CREATE TABLE catalog_relationship_evidence (
    relationship_id INTEGER NOT NULL REFERENCES inferred_catalog_relationships(relationship_id) ON DELETE RESTRICT,
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    supporting_relationship_id INTEGER NOT NULL REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    PRIMARY KEY (relationship_id, list_order)
) STRICT, WITHOUT ROWID;
CREATE INDEX relationship_evidence_supporting
    ON catalog_relationship_evidence(supporting_relationship_id, relationship_id);

CREATE TABLE catalog_relationship_evidence_publications (
    relationship_id INTEGER PRIMARY KEY REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    evidence_kind TEXT NOT NULL CHECK (evidence_kind IN ('rationale', 'catalog_comparison'))
) STRICT;

-- Reviews are append-only drafts; publication is the visibility/immutability
-- boundary. Supersession is an edge, so the prior assertion/review survives.
CREATE TABLE catalog_relationship_reviews (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE CHECK (length(CAST(review_key AS BLOB)) > 0),
    relationship_id INTEGER NOT NULL REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('accepted', 'rejected', 'withdrawn', 'superseded')),
    note TEXT NOT NULL CHECK (length(CAST(note AS BLOB)) > 0),
    reviewed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;
CREATE INDEX catalog_relationship_reviews_latest
    ON catalog_relationship_reviews(relationship_id, review_id);

CREATE TABLE replaced_catalog_relationships (
    review_id INTEGER PRIMARY KEY REFERENCES catalog_relationship_reviews(review_id) ON DELETE RESTRICT,
    replacement_relationship_id INTEGER NOT NULL REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    CHECK (replacement_relationship_id > 0)
) STRICT;

CREATE TABLE catalog_relationship_review_publications (
    review_id INTEGER PRIMARY KEY REFERENCES catalog_relationship_reviews(review_id) ON DELETE RESTRICT,
    published_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

-- Select latest published review first, then expose accepted assertions. A
-- newer unsealed draft does not hide the latest published result.
CREATE VIEW active_catalog_relationship_reviews AS
WITH latest(relationship_id, review_id) AS (
    SELECT review.relationship_id, max(review.review_id)
    FROM catalog_relationship_reviews AS review
    JOIN catalog_relationship_review_publications AS publication USING (review_id)
    GROUP BY review.relationship_id
)
SELECT review.relationship_id, review.review_id
FROM latest JOIN catalog_relationship_reviews AS review USING (relationship_id, review_id)
WHERE review.decision = 'accepted';

CREATE VIEW active_catalog_relationship_replacements AS
WITH latest(relationship_id, review_id) AS (
    SELECT review.relationship_id, max(review.review_id)
    FROM catalog_relationship_reviews AS review
    JOIN catalog_relationship_review_publications AS publication USING (review_id)
    GROUP BY review.relationship_id
)
SELECT review.relationship_id AS predecessor_relationship_id,
       replacement.replacement_relationship_id, review.review_id
FROM latest JOIN catalog_relationship_reviews AS review USING (relationship_id, review_id)
JOIN replaced_catalog_relationships AS replacement USING (review_id)
WHERE review.decision = 'superseded';

-- File-match review rows identify exact native witnesses. Hash witnesses use
-- the shared reported_hash_id. Size witnesses store only the native media key
-- and a closed field selector; source values stay on their typed native owner.
CREATE TABLE file_match_conflicts (
    conflict_id INTEGER PRIMARY KEY NOT NULL CHECK (conflict_id > 0),
    incoming_media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id) ON DELETE RESTRICT,
    candidate_file_uuid BLOB NOT NULL REFERENCES shared_catalog_files(file_uuid) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (reason IN ('ambiguous_alias', 'contradictory_assertions', 'disputed_alias')),
    UNIQUE (conflict_id, incoming_media_entry_id, candidate_file_uuid)
) STRICT;
CREATE INDEX file_match_conflicts_candidate ON file_match_conflicts(candidate_file_uuid, conflict_id);

CREATE TABLE file_match_conflict_hashes (
    conflict_id INTEGER NOT NULL REFERENCES file_match_conflicts(conflict_id) ON DELETE RESTRICT,
    reported_hash_id INTEGER NOT NULL REFERENCES catalog_entry_hashes(reported_hash_id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    PRIMARY KEY (conflict_id, reported_hash_id, role)
) STRICT, WITHOUT ROWID;

CREATE TABLE file_match_conflict_sizes (
    conflict_id INTEGER NOT NULL REFERENCES file_match_conflicts(conflict_id) ON DELETE RESTRICT,
    media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id) ON DELETE RESTRICT,
    source_size_field TEXT NOT NULL CHECK (source_size_field IN ('size', 'file_length')),
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    PRIMARY KEY (conflict_id, media_entry_id, source_size_field, role)
) STRICT, WITHOUT ROWID;

CREATE TABLE file_match_decisions (
    decision_id INTEGER PRIMARY KEY NOT NULL CHECK (decision_id > 0),
    decision TEXT NOT NULL CHECK (decision IN ('keep_separate', 'merge')),
    kept_file_uuid BLOB REFERENCES shared_catalog_files(file_uuid) ON DELETE RESTRICT,
    rationale TEXT NOT NULL CHECK (length(CAST(trim(rationale) AS BLOB)) > 0),
    decided_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'merge') = (kept_file_uuid IS NOT NULL))
) STRICT;

CREATE TABLE file_match_decision_conflicts (
    decision_id INTEGER NOT NULL REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT,
    conflict_id INTEGER NOT NULL UNIQUE REFERENCES file_match_conflicts(conflict_id) ON DELETE RESTRICT,
    outcome TEXT NOT NULL CHECK (outcome IN ('keep_separate', 'merged')),
    PRIMARY KEY (decision_id, conflict_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE file_match_hash_decisions (
    decision_id INTEGER NOT NULL,
    conflict_id INTEGER NOT NULL,
    reported_hash_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    disposition TEXT NOT NULL CHECK (disposition IN ('accept', 'reject')),
    PRIMARY KEY (decision_id, conflict_id, reported_hash_id, role),
    FOREIGN KEY (decision_id, conflict_id)
        REFERENCES file_match_decision_conflicts(decision_id, conflict_id) ON DELETE RESTRICT,
    FOREIGN KEY (conflict_id, reported_hash_id, role)
        REFERENCES file_match_conflict_hashes(conflict_id, reported_hash_id, role) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
CREATE INDEX file_match_hash_review_lookup
    ON file_match_hash_decisions(reported_hash_id, disposition, decision_id);

CREATE TABLE file_match_size_decisions (
    decision_id INTEGER NOT NULL,
    conflict_id INTEGER NOT NULL,
    media_entry_id INTEGER NOT NULL,
    source_size_field TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    disposition TEXT NOT NULL CHECK (disposition IN ('accept', 'reject')),
    PRIMARY KEY (decision_id, conflict_id, media_entry_id, source_size_field, role),
    FOREIGN KEY (decision_id, conflict_id)
        REFERENCES file_match_decision_conflicts(decision_id, conflict_id) ON DELETE RESTRICT,
    FOREIGN KEY (conflict_id, media_entry_id, source_size_field, role)
        REFERENCES file_match_conflict_sizes(conflict_id, media_entry_id, source_size_field, role) ON DELETE RESTRICT
) STRICT, WITHOUT ROWID;
CREATE INDEX file_match_size_review_lookup
    ON file_match_size_decisions(media_entry_id, source_size_field, disposition, decision_id);

CREATE TABLE file_match_uuid_redirects (
    old_file_uuid BLOB PRIMARY KEY NOT NULL REFERENCES shared_catalog_files(file_uuid) ON DELETE RESTRICT,
    kept_file_uuid BLOB NOT NULL REFERENCES shared_catalog_files(file_uuid) ON DELETE RESTRICT,
    decision_id INTEGER NOT NULL REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT,
    CHECK (typeof(old_file_uuid) = 'blob' AND length(old_file_uuid) = 16),
    CHECK (typeof(kept_file_uuid) = 'blob' AND length(kept_file_uuid) = 16),
    CHECK (old_file_uuid <> kept_file_uuid)
) STRICT, WITHOUT ROWID;
CREATE INDEX file_match_uuid_redirect_destination
    ON file_match_uuid_redirects(kept_file_uuid, old_file_uuid);

CREATE TABLE file_match_decision_publications (
    decision_id INTEGER PRIMARY KEY REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT,
    published_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
) STRICT;

CREATE VIEW canonical_shared_file_uuids AS
SELECT issued.file_uuid,
       (WITH RECURSIVE walk(file_uuid) AS (
            SELECT issued.file_uuid
            UNION
            SELECT redirect.kept_file_uuid FROM walk
            JOIN file_match_uuid_redirects AS redirect ON redirect.old_file_uuid=walk.file_uuid
            JOIN file_match_decision_publications AS publication USING(decision_id)
        )
        SELECT walk.file_uuid FROM walk
        WHERE NOT EXISTS (
            SELECT 1 FROM file_match_uuid_redirects AS redirect
            JOIN file_match_decision_publications AS publication USING(decision_id)
            WHERE redirect.old_file_uuid=walk.file_uuid
        )) AS canonical_file_uuid
FROM shared_catalog_files AS issued;

-- Reverse traversal starts at a selected canonical root, not at every issued
-- UUID. Both endpoints remain issued identities; no source assignment changes.
CREATE VIEW candidate_file_component_members AS
SELECT root.file_uuid AS canonical_file_uuid, member.file_uuid
FROM shared_catalog_files AS root
JOIN shared_catalog_files AS member ON member.file_uuid IN (
    WITH RECURSIVE aliases(file_uuid) AS (
        SELECT root.file_uuid
        UNION
        SELECT redirect.old_file_uuid FROM aliases
        JOIN file_match_uuid_redirects AS redirect ON redirect.kept_file_uuid=aliases.file_uuid
        JOIN file_match_decision_publications AS publication USING(decision_id)
    ) SELECT file_uuid FROM aliases
)
WHERE NOT EXISTS (
    SELECT 1 FROM file_match_uuid_redirects AS redirect
    JOIN file_match_decision_publications AS publication USING(decision_id)
    WHERE redirect.old_file_uuid=root.file_uuid
);

-- Source evidence views never copy hash bytes or size values. The rejection
-- ledger excludes only an exact published witness; a rejected witness cannot
-- be restored by a later acceptance.
CREATE VIEW accepted_catalog_file_hash_evidence AS
SELECT declaration.reported_hash_id, media.file_uuid, declaration.hash_id,
       declaration.hash_scope, declaration.source_hash_field,
       declaration.media_entry_id
FROM catalog_media_entries AS media
CROSS JOIN catalog_entry_hashes AS declaration
  ON declaration.media_entry_id=media.media_entry_id
WHERE media.file_uuid IS NOT NULL
  AND EXISTS (
      SELECT 1 FROM candidate_qualified_file_hashes AS qualified
      WHERE qualified.reported_hash_id=declaration.reported_hash_id
  )
  AND NOT EXISTS (
      SELECT 1 FROM file_match_hash_decisions AS review
      JOIN file_match_decision_publications AS publication USING (decision_id)
      WHERE review.reported_hash_id = declaration.reported_hash_id
        AND review.disposition = 'reject'
  );

CREATE VIEW accepted_catalog_file_size_evidence AS
SELECT size.media_entry_id, size.source_size_field, size.byte_length, media.file_uuid
FROM catalog_media_entries AS media
CROSS JOIN candidate_native_file_sizes AS size
  ON size.media_entry_id=media.media_entry_id
WHERE media.file_uuid IS NOT NULL AND size.size_state='value'
  AND EXISTS (
      SELECT 1 FROM candidate_native_file_byte_coverage AS qualification
      WHERE qualification.media_entry_id=media.media_entry_id
  )
  AND NOT EXISTS (
      SELECT 1 FROM file_match_size_decisions AS review
      JOIN file_match_decision_publications AS publication USING(decision_id)
      WHERE review.media_entry_id=size.media_entry_id
        AND review.source_size_field=size.source_size_field
        AND review.disposition='reject'
  );

CREATE VIEW accepted_catalog_file_identity_evidence AS
SELECT evidence.* FROM accepted_catalog_file_hash_evidence AS evidence
JOIN hash_values AS value USING (hash_id)
WHERE value.algorithm IN ('sha1', 'sha256');

CREATE VIEW canonical_catalog_file_hash_evidence AS
SELECT member.canonical_file_uuid AS file_uuid, evidence.hash_id
FROM candidate_file_component_members AS member
JOIN accepted_catalog_file_hash_evidence AS evidence ON evidence.file_uuid=member.file_uuid
GROUP BY member.canonical_file_uuid, evidence.hash_id;

CREATE VIEW canonical_catalog_file_size_evidence AS
SELECT member.canonical_file_uuid AS file_uuid, evidence.byte_length
FROM candidate_file_component_members AS member
JOIN accepted_catalog_file_size_evidence AS evidence ON evidence.file_uuid=member.file_uuid
GROUP BY member.canonical_file_uuid, evidence.byte_length;

CREATE VIEW catalog_shared_fact_mismatches AS
SELECT 'hash_missing' AS problem, evidence.file_uuid, evidence.hash_id, NULL AS byte_length
FROM canonical_catalog_file_hash_evidence AS evidence
LEFT JOIN shared_file_hashes AS stored
  ON stored.file_uuid = evidence.file_uuid AND stored.hash_id = evidence.hash_id
WHERE stored.file_uuid IS NULL
UNION ALL
SELECT 'hash_unsupported', stored.file_uuid, stored.hash_id, NULL
FROM shared_file_hashes AS stored
LEFT JOIN canonical_catalog_file_hash_evidence AS evidence
  ON evidence.file_uuid = stored.file_uuid AND evidence.hash_id = stored.hash_id
WHERE evidence.file_uuid IS NULL
UNION ALL
SELECT 'size_missing', evidence.file_uuid, NULL, evidence.byte_length
FROM canonical_catalog_file_size_evidence AS evidence
LEFT JOIN shared_file_sizes AS stored USING(file_uuid,byte_length)
WHERE stored.file_uuid IS NULL
UNION ALL
SELECT 'size_unsupported', stored.file_uuid, NULL, stored.byte_length
FROM shared_file_sizes AS stored
LEFT JOIN canonical_catalog_file_size_evidence AS evidence USING(file_uuid,byte_length)
WHERE evidence.file_uuid IS NULL;

CREATE VIEW candidate_shared_file_contradictions AS
SELECT 'size_contradiction' AS problem, file_uuid
FROM shared_file_sizes GROUP BY file_uuid HAVING count(*)>1
UNION ALL
SELECT 'hash_contradiction', membership.file_uuid
FROM shared_file_hashes AS membership JOIN hash_values AS value USING(hash_id)
GROUP BY membership.file_uuid,value.algorithm HAVING count(*)>1;

CREATE VIEW candidate_review_file_components AS
SELECT settled.decision_id, canonical.canonical_file_uuid
FROM file_match_decision_conflicts AS settled
JOIN file_match_conflicts AS conflict USING(conflict_id)
JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=conflict.candidate_file_uuid
UNION ALL
SELECT settled.decision_id,canonical.canonical_file_uuid
FROM file_match_decision_conflicts AS settled
JOIN file_match_conflicts AS conflict USING(conflict_id)
JOIN catalog_media_entries AS incoming ON incoming.media_entry_id=conflict.incoming_media_entry_id
JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=incoming.file_uuid
UNION ALL
SELECT decision.decision_id,canonical.canonical_file_uuid
FROM file_match_decisions AS decision
JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=decision.kept_file_uuid;

CREATE VIEW candidate_shared_identity_problems AS
SELECT 'linked_file_redirect_cycle_or_missing_identity' AS problem,
       media.media_entry_id AS owner_id, element.edition_id
FROM catalog_media_entries AS media
JOIN catalog_source_elements AS element ON element.source_element_id=media.media_entry_id
LEFT JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=media.file_uuid
WHERE media.file_uuid IS NOT NULL AND canonical.canonical_file_uuid IS NULL
UNION ALL
SELECT 'linked_file_cross_registry_redirect',media.media_entry_id,element.edition_id
FROM catalog_media_entries AS media
JOIN catalog_source_elements AS element ON element.source_element_id=media.media_entry_id
JOIN canonical_shared_file_uuids AS canonical ON canonical.file_uuid=media.file_uuid
JOIN shared_catalog_files AS issued ON issued.file_uuid=media.file_uuid
JOIN shared_catalog_files AS kept ON kept.file_uuid=canonical.canonical_file_uuid
WHERE issued.registry_id<>kept.registry_id
UNION ALL
SELECT 'issued_file_redirect_cycle:' || hex(canonical.file_uuid),NULL,NULL
FROM canonical_shared_file_uuids AS canonical WHERE canonical.canonical_file_uuid IS NULL
UNION ALL
SELECT 'issued_file_cross_registry_redirect:' || hex(redirect.old_file_uuid),NULL,NULL
FROM file_match_uuid_redirects AS redirect
JOIN file_match_decision_publications AS publication USING(decision_id)
JOIN shared_catalog_files AS issued ON issued.file_uuid=redirect.old_file_uuid
JOIN shared_catalog_files AS kept ON kept.file_uuid=redirect.kept_file_uuid
WHERE issued.registry_id<>kept.registry_id;

-- Relationship-owned, query-only audit surface. Native declarations retain
-- their own closed typed tables. The assembler supplies the closed DOC-23
-- declaration projection and bidirectional canonical-position checks; neither
-- query surface is a persisted polymorphic source-owner table.
CREATE VIEW candidate_relationship_integrity_problems AS
WITH typed_declarations(relationship_id, reported_kind, owner_edition_id) AS (
    SELECT relationship_id,reported_kind,edition_id
    FROM candidate_relationship_declarations
), declaration_counts AS (
    SELECT relationship_id, count(*) AS declaration_count,
           min(reported_kind) AS sole_kind,
           min(owner_edition_id) AS sole_edition_id
    FROM typed_declarations
    GROUP BY relationship_id
), problems(problem, owner_id, edition_id) AS (
    SELECT 'source_identity_missing_kind', relationship.relationship_id, relationship.edition_id
    FROM catalog_relationships AS relationship
    LEFT JOIN reported_catalog_relationships AS reported USING (relationship_id)
    WHERE relationship.origin = 'source' AND reported.relationship_id IS NULL
    UNION ALL
    SELECT 'kind_without_source_identity:' || reported.reported_kind,
           reported.relationship_id, relationship.edition_id
    FROM reported_catalog_relationships AS reported
    LEFT JOIN catalog_relationships AS relationship USING (relationship_id)
    WHERE relationship.relationship_id IS NULL OR relationship.origin <> 'source'
       OR relationship.edition_id IS NULL
    UNION ALL
    SELECT 'reported_identity_declaration_count:' || reported.reported_kind,
           reported.relationship_id, relationship.edition_id
    FROM reported_catalog_relationships AS reported
    LEFT JOIN catalog_relationships AS relationship USING (relationship_id)
    LEFT JOIN declaration_counts AS counts USING (relationship_id)
    WHERE coalesce(counts.declaration_count, 0) <> 1
    UNION ALL
    SELECT 'reported_identity_declaration_kind:' || reported.reported_kind,
           reported.relationship_id, relationship.edition_id
    FROM reported_catalog_relationships AS reported
    JOIN catalog_relationships AS relationship USING (relationship_id)
    JOIN declaration_counts AS counts USING (relationship_id)
    WHERE counts.sole_kind <> reported.reported_kind
    UNION ALL
    SELECT 'reported_identity_owner_edition:' || reported.reported_kind,
           reported.relationship_id, relationship.edition_id
    FROM reported_catalog_relationships AS reported
    JOIN catalog_relationships AS relationship USING (relationship_id)
    JOIN declaration_counts AS counts USING (relationship_id)
    WHERE counts.sole_edition_id <> relationship.edition_id
    UNION ALL
    SELECT 'orphan_typed_declaration:' || declaration.reported_kind,
           declaration.relationship_id, declaration.owner_edition_id
    FROM typed_declarations AS declaration
    LEFT JOIN reported_catalog_relationships AS reported
      ON reported.relationship_id = declaration.relationship_id
     AND reported.reported_kind = declaration.reported_kind
    WHERE reported.relationship_id IS NULL
    UNION ALL
    SELECT 'derived_or_user_evidence_not_ready', relationship.relationship_id, relationship.edition_id
    FROM catalog_relationships AS relationship
    WHERE relationship.origin IN ('derived', 'user')
      AND NOT EXISTS (
          SELECT 1 FROM catalog_relationship_evidence_publications AS publication
          WHERE publication.relationship_id = relationship.relationship_id
      )
    UNION ALL
    SELECT 'published_evidence_payload_mismatch:' || publication.evidence_kind,
           publication.relationship_id, relationship.edition_id
    FROM catalog_relationship_evidence_publications AS publication
    JOIN catalog_relationships AS relationship USING (relationship_id)
    WHERE NOT EXISTS (
              SELECT 1 FROM catalog_relationships AS relationship
              WHERE relationship.relationship_id = publication.relationship_id
                AND relationship.origin IN ('derived', 'user')
          )
       OR (publication.evidence_kind = 'rationale' AND NOT EXISTS (
               SELECT 1 FROM catalog_relationship_rationales AS rationale
               WHERE rationale.relationship_id = publication.relationship_id
           ))
       OR (publication.evidence_kind = 'rationale' AND EXISTS (
               SELECT 1 FROM catalog_relationship_comparisons AS comparison
               WHERE comparison.relationship_id = publication.relationship_id
           ))
       OR (publication.evidence_kind = 'catalog_comparison' AND NOT EXISTS (
               SELECT 1 FROM catalog_relationship_comparisons AS comparison
               WHERE comparison.relationship_id = publication.relationship_id
           ))
       OR (publication.evidence_kind = 'catalog_comparison' AND EXISTS (
               SELECT 1 FROM catalog_relationship_rationales AS rationale
               WHERE rationale.relationship_id = publication.relationship_id
           ))
    UNION ALL
    SELECT 'published_file_match_hash_witness_unreviewed:decision=' || publication.decision_id
           || ':hash=' || witness.reported_hash_id, NULL, NULL
    FROM file_match_decision_publications AS publication
    JOIN file_match_decision_conflicts AS settled USING (decision_id)
    JOIN file_match_conflict_hashes AS witness USING (conflict_id)
    WHERE NOT EXISTS (
        SELECT 1 FROM file_match_hash_decisions AS review
        WHERE review.decision_id = settled.decision_id
          AND review.conflict_id = settled.conflict_id
          AND review.reported_hash_id = witness.reported_hash_id
          AND review.role = witness.role
    )
    UNION ALL
    SELECT 'published_file_match_size_witness_unreviewed:decision=' || publication.decision_id
           || ':media=' || witness.media_entry_id, NULL, NULL
    FROM file_match_decision_publications AS publication
    JOIN file_match_decision_conflicts AS settled USING (decision_id)
    JOIN file_match_conflict_sizes AS witness USING (conflict_id)
    WHERE NOT EXISTS (
        SELECT 1 FROM file_match_size_decisions AS review
        WHERE review.decision_id = settled.decision_id
          AND review.conflict_id = settled.conflict_id
          AND review.media_entry_id = witness.media_entry_id
          AND review.source_size_field = witness.source_size_field
          AND review.role = witness.role
    )
    UNION ALL
    SELECT 'shared_membership_mismatch:' || mismatch.problem || ':'
           || hex(mismatch.file_uuid) || ':'
           || coalesce('hash=' || mismatch.hash_id,'size=' || mismatch.byte_length), NULL, NULL
    FROM catalog_shared_fact_mismatches AS mismatch
)
SELECT problem, owner_id, edition_id FROM problems;
