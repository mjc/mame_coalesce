CREATE TABLE catalog_relationship_rules (
    rule_id INTEGER PRIMARY KEY NOT NULL CHECK (typeof(rule_id) = 'integer'),
    rule_key TEXT NOT NULL CHECK (length(CAST(rule_key AS BLOB)) > 0),
    revision TEXT NOT NULL CHECK (length(CAST(revision AS BLOB)) > 0),
    description TEXT NOT NULL CHECK (length(CAST(description AS BLOB)) > 0),
    UNIQUE (rule_key, revision)
);

CREATE TABLE catalog_relationship_targets (
    target_id INTEGER PRIMARY KEY NOT NULL CHECK (typeof(target_id) = 'integer'),
    kind TEXT NOT NULL CHECK (kind IN (
        'catalog_set', 'catalog_media_entry', 'shared_file', 'no_intro_archive',
        'unresolved_catalog', 'external_record', 'declared_digest'
    )),
    UNIQUE (target_id, kind)
);

CREATE TABLE catalog_set_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'catalog_set' CHECK (kind = 'catalog_set'),
    set_id INTEGER NOT NULL UNIQUE REFERENCES catalog_sets(set_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE catalog_media_entry_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'catalog_media_entry' CHECK (kind = 'catalog_media_entry'),
    occurrence_id INTEGER NOT NULL UNIQUE REFERENCES asset_occurrences(occurrence_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE shared_file_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'shared_file' CHECK (kind = 'shared_file'),
    content_uuid BLOB NOT NULL UNIQUE
        REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT
        CHECK (typeof(content_uuid) = 'blob' AND length(content_uuid) = 16),
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE no_intro_archive_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'no_intro_archive' CHECK (kind = 'no_intro_archive'),
    archive_id INTEGER NOT NULL UNIQUE
        REFERENCES no_intro_archive_descriptions(archive_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE external_catalog_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'external_record' CHECK (kind = 'external_record'),
    namespace TEXT NOT NULL,
    declared_key TEXT NOT NULL,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE UNIQUE INDEX external_catalog_target_identity
    ON external_catalog_targets(namespace, declared_key);

CREATE TABLE declared_digest_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'declared_digest' CHECK (kind = 'declared_digest'),
    digest_id INTEGER NOT NULL UNIQUE REFERENCES digest_values(digest_id) ON DELETE RESTRICT,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE unresolved_catalog_targets (
    target_id INTEGER PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL DEFAULT 'unresolved_catalog' CHECK (kind = 'unresolved_catalog'),
    snapshot_key TEXT NOT NULL REFERENCES catalog_snapshots(snapshot_key) ON DELETE RESTRICT,
    record_kind TEXT NOT NULL CHECK (record_kind IN (
        'catalog_set', 'software_item', 'asset_requirement'
    )),
    declared_name TEXT NOT NULL,
    declared_set_name TEXT,
    declared_list_name TEXT,
    declared_media_order INTEGER,
    FOREIGN KEY (target_id, kind)
        REFERENCES catalog_relationship_targets(target_id, kind) ON DELETE RESTRICT,
    CHECK (
        (record_kind = 'catalog_set'
            AND declared_set_name IS NULL
            AND declared_list_name IS NULL
            AND declared_media_order IS NULL)
        OR (record_kind = 'software_item'
            AND declared_set_name IS NULL
            AND declared_list_name IS NOT NULL
            AND declared_media_order IS NULL)
        OR (record_kind = 'asset_requirement'
            AND declared_set_name IS NOT NULL
            AND declared_list_name IS NULL
            AND typeof(declared_media_order) = 'integer'
            AND declared_media_order >= 0)
    )
) WITHOUT ROWID;
CREATE UNIQUE INDEX unresolved_catalog_set_target_identity
    ON unresolved_catalog_targets(snapshot_key, declared_name)
    WHERE record_kind = 'catalog_set';
CREATE UNIQUE INDEX unresolved_catalog_software_target_identity
    ON unresolved_catalog_targets(snapshot_key, declared_list_name, declared_name)
    WHERE record_kind = 'software_item';
CREATE UNIQUE INDEX unresolved_catalog_media_target_identity
    ON unresolved_catalog_targets(snapshot_key, declared_set_name, declared_name, declared_media_order)
    WHERE record_kind = 'asset_requirement';
CREATE INDEX unresolved_catalog_target_edition ON unresolved_catalog_targets(snapshot_key,record_kind,target_id);

CREATE TABLE inferred_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    from_target_id INTEGER NOT NULL
        REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    to_target_id INTEGER NOT NULL
        REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    rule_id INTEGER NOT NULL REFERENCES catalog_relationship_rules(rule_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE INDEX inferred_relationship_from_target
    ON inferred_catalog_relationships(from_target_id, relationship_id);
CREATE INDEX inferred_relationship_to_target
    ON inferred_catalog_relationships(to_target_id, relationship_id);

CREATE TABLE manual_catalog_relationships (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    relation_type TEXT NOT NULL CHECK (relation_type IN (
        'exact_content_identity', 'revision_of', 'dump_of_intended_release',
        'alternate_representation_of', 'source_parent_clone', 'runtime_dependency',
        'catalog_correction', 'catalog_continuity'
    )),
    from_target_id INTEGER NOT NULL
        REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT,
    to_target_id INTEGER NOT NULL
        REFERENCES catalog_relationship_targets(target_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE INDEX manual_relationship_from_target
    ON manual_catalog_relationships(from_target_id, relationship_id);
CREATE INDEX manual_relationship_to_target
    ON manual_catalog_relationships(to_target_id, relationship_id);

-- This projection derives all display values and owning snapshots from actual
-- catalog rows. Target subtypes contain only the native identity they reference.
CREATE VIEW catalog_relationship_target_details AS
SELECT target.target_id, target.kind,
       CASE target.kind
         WHEN 'catalog_set' THEN CASE WHEN groups.kind='software_list' THEN 'software_item' ELSE 'catalog_set' END
         WHEN 'declared_digest' THEN 'content_object'
         WHEN 'unresolved_catalog' THEN unresolved.record_kind
         ELSE target.kind END AS endpoint_kind,
       COALESCE(groups.snapshot_key,unresolved.snapshot_key) AS snapshot_key,
       sets.set_id AS owner_set_id,
       CASE target.kind
         WHEN 'catalog_set' THEN CASE WHEN groups.kind='software_list' THEN lists.name ELSE sets.set_name END
         WHEN 'shared_file' THEN lower(hex(shared.content_uuid))
         WHEN 'external_record' THEN external.namespace
         WHEN 'declared_digest' THEN digest.algorithm
         WHEN 'unresolved_catalog' THEN CASE unresolved.record_kind
             WHEN 'catalog_set' THEN unresolved.declared_name
             WHEN 'software_item' THEN unresolved.declared_list_name
             ELSE unresolved.declared_set_name END END AS endpoint_a,
       CASE target.kind
         WHEN 'catalog_set' THEN CASE WHEN groups.kind='software_list' THEN sets.set_name END
         WHEN 'external_record' THEN external.declared_key
         WHEN 'declared_digest' THEN lower(hex(digest.digest))
         WHEN 'unresolved_catalog' THEN CASE WHEN unresolved.record_kind IN ('software_item','asset_requirement') THEN unresolved.declared_name END END AS endpoint_b,
       CASE target.kind
         WHEN 'catalog_media_entry' THEN occurrence.occurrence_id
         WHEN 'no_intro_archive' THEN archive.archive_id
         WHEN 'unresolved_catalog' THEN unresolved.declared_media_order END AS endpoint_c,
       COALESCE(groups.snapshot_key,unresolved.snapshot_key) IS NULL
         OR EXISTS (SELECT 1 FROM snapshot_publications AS publication
                    WHERE publication.snapshot_key=COALESCE(groups.snapshot_key,unresolved.snapshot_key)) AS is_published
FROM catalog_relationship_targets AS target
LEFT JOIN catalog_set_targets AS set_target ON set_target.target_id=target.target_id AND target.kind='catalog_set'
LEFT JOIN catalog_media_entry_targets AS media_target ON media_target.target_id=target.target_id AND target.kind='catalog_media_entry'
LEFT JOIN shared_file_targets AS shared ON shared.target_id=target.target_id AND target.kind='shared_file'
LEFT JOIN catalog_contents AS shared_owner ON shared_owner.content_uuid=shared.content_uuid
LEFT JOIN no_intro_archive_targets AS archive_target ON archive_target.target_id=target.target_id AND target.kind='no_intro_archive'
LEFT JOIN external_catalog_targets AS external ON external.target_id=target.target_id AND target.kind='external_record'
LEFT JOIN declared_digest_targets AS digest_target ON digest_target.target_id=target.target_id AND target.kind='declared_digest'
LEFT JOIN unresolved_catalog_targets AS unresolved ON unresolved.target_id=target.target_id AND target.kind='unresolved_catalog'
LEFT JOIN catalog_snapshots AS unresolved_snapshot ON unresolved_snapshot.snapshot_key=unresolved.snapshot_key
LEFT JOIN digest_values AS digest ON digest.digest_id=digest_target.digest_id
LEFT JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=media_target.occurrence_id
LEFT JOIN no_intro_archive_descriptions AS archive ON archive.archive_id=archive_target.archive_id
LEFT JOIN catalog_sets AS sets ON sets.set_id=COALESCE(set_target.set_id,occurrence.record_id,archive.set_id)
LEFT JOIN catalog_set_groups AS groups USING(set_group_id)
LEFT JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id
WHERE CASE target.kind
  WHEN 'catalog_set' THEN set_target.target_id IS NOT NULL AND groups.snapshot_key IS NOT NULL
    AND ((groups.kind='root' AND sets.source_element_kind<>'software_item')
         OR (groups.kind='software_list' AND sets.source_element_kind='software_item' AND lists.namespace_id IS NOT NULL))
  WHEN 'catalog_media_entry' THEN occurrence.occurrence_id IS NOT NULL AND groups.snapshot_key IS NOT NULL
  WHEN 'shared_file' THEN shared_owner.content_uuid IS NOT NULL
  WHEN 'no_intro_archive' THEN archive.archive_id IS NOT NULL AND groups.snapshot_key IS NOT NULL
  WHEN 'external_record' THEN external.target_id IS NOT NULL
  WHEN 'declared_digest' THEN digest.digest_id IS NOT NULL
  WHEN 'unresolved_catalog' THEN unresolved_snapshot.snapshot_key IS NOT NULL
  ELSE 0 END;

CREATE VIEW inferred_relationship_assertions AS
SELECT identity.assertion_key AS assertion_key,
       inferred.relation_type AS relation_type,
       'derived_candidate' AS origin,
       NULL AS source_snapshot_key, NULL AS source_field,
       NULL AS source_line, NULL AS source_column,
       subject.snapshot_key AS generic_subject_snapshot_key,
       subject.endpoint_kind AS subject_kind,
       subject.owner_set_id AS subject_set_id,
       subject.endpoint_a AS generic_subject_a,
       subject.endpoint_b AS generic_subject_b,
       subject.endpoint_c AS generic_subject_c,
       NULL AS source_subject_a, NULL AS source_subject_b, NULL AS source_subject_c,
       subject.snapshot_key AS subject_snapshot_key,
       target.snapshot_key AS generic_target_snapshot_key,
       target.endpoint_kind AS target_kind,
       target.owner_set_id AS target_set_id,
       target.endpoint_a AS generic_target_a,
       target.endpoint_b AS generic_target_b,
       target.endpoint_c AS generic_target_c,
       NULL AS source_target_a, NULL AS source_target_b, NULL AS source_target_c,
       target.snapshot_key AS target_snapshot_key,
       rule.revision AS rule_version
FROM catalog_relationships AS identity
JOIN inferred_catalog_relationships AS inferred USING(relationship_id)
JOIN catalog_relationship_target_details AS subject ON subject.target_id = inferred.from_target_id
JOIN catalog_relationship_target_details AS target ON target.target_id = inferred.to_target_id
JOIN catalog_relationship_rules AS rule USING(rule_id)
WHERE identity.origin = 'derived';
CREATE VIEW manual_relationship_assertions (
    assertion_key, relation_type, origin, source_snapshot_key, source_field,
    source_line, source_column, generic_subject_snapshot_key, subject_kind,
    subject_set_id, generic_subject_a, generic_subject_b, generic_subject_c,
    source_subject_a, source_subject_b, source_subject_c, subject_snapshot_key,
    generic_target_snapshot_key, target_kind, target_set_id, generic_target_a,
    generic_target_b, generic_target_c, source_target_a, source_target_b,
    source_target_c, target_snapshot_key, rule_version
) AS
SELECT identity.assertion_key, manual.relation_type, 'user_conclusion',
       NULL, NULL, NULL, NULL,
       subject.snapshot_key, subject.endpoint_kind, subject.owner_set_id,
       subject.endpoint_a, subject.endpoint_b, subject.endpoint_c,
       NULL, NULL, NULL, subject.snapshot_key,
       target.snapshot_key, target.endpoint_kind, target.owner_set_id,
       target.endpoint_a, target.endpoint_b, target.endpoint_c,
       NULL, NULL, NULL, target.snapshot_key, NULL
FROM catalog_relationships AS identity
JOIN manual_catalog_relationships AS manual USING(relationship_id)
JOIN catalog_relationship_target_details AS subject ON subject.target_id = manual.from_target_id
JOIN catalog_relationship_target_details AS target ON target.target_id = manual.to_target_id
WHERE identity.origin = 'user';

CREATE VIEW relationship_assertions AS
SELECT * FROM inferred_relationship_assertions
UNION ALL SELECT * FROM manual_relationship_assertions;

-- One closed ownership check covers every registry origin. Import EOF only
-- asks about source rows in that snapshot; evidence publication asks about a
-- derived or manual row through the same completeness result.

CREATE TRIGGER relationship_assertions_are_immutable_insert
INSTEAD OF INSERT ON relationship_assertions
BEGIN SELECT RAISE(ABORT, 'relationship assertions are immutable'); END;
CREATE TRIGGER relationship_assertions_are_immutable_update
INSTEAD OF UPDATE ON relationship_assertions
BEGIN SELECT RAISE(ABORT, 'relationship assertions are immutable'); END;
CREATE TRIGGER relationship_assertions_are_immutable_delete
INSTEAD OF DELETE ON relationship_assertions
BEGIN SELECT RAISE(ABORT, 'relationship assertions are immutable'); END;

-- Match RelationshipRule's Unicode whitespace check without modifying literals.
-- BLOB length preserves non-whitespace NULs instead of SQLite's text-length cutoff.
CREATE TRIGGER catalog_relationship_rules_require_nonempty_metadata
BEFORE INSERT ON catalog_relationship_rules
WHEN EXISTS (
    SELECT 1 FROM (
        SELECT NEW.rule_key AS value
        UNION ALL SELECT NEW.revision
        UNION ALL SELECT NEW.description
    ) WHERE typeof(value)<>'text'
         OR length(CAST(trim(value,(SELECT characters FROM relationship_unicode_whitespace)) AS BLOB)) = 0
)
BEGIN SELECT RAISE(ABORT, 'relationship rule metadata must not be whitespace-only'); END;
CREATE TRIGGER catalog_relationship_rules_reject_conflicting_description
BEFORE INSERT ON catalog_relationship_rules
WHEN EXISTS (
    SELECT 1 FROM catalog_relationship_rules
    WHERE rule_key = NEW.rule_key AND revision = NEW.revision
      AND description <> NEW.description
)
BEGIN SELECT RAISE(ABORT, 'relationship rule revision has a conflicting description'); END;
CREATE TRIGGER catalog_relationship_rules_reject_replacement
BEFORE INSERT ON catalog_relationship_rules
WHEN EXISTS (SELECT 1 FROM catalog_relationship_rules
             WHERE rule_id = NEW.rule_id OR (rule_key=NEW.rule_key AND revision=NEW.revision))
BEGIN SELECT RAISE(ABORT, 'relationship rule metadata is immutable'); END;
CREATE TRIGGER catalog_relationship_rules_immutable_update
BEFORE UPDATE ON catalog_relationship_rules
BEGIN SELECT RAISE(ABORT, 'relationship rule metadata is immutable'); END;
CREATE TRIGGER catalog_relationship_rules_immutable_delete
BEFORE DELETE ON catalog_relationship_rules
BEGIN SELECT RAISE(ABORT, 'relationship rule metadata is immutable'); END;

CREATE TRIGGER catalog_relationship_targets_reject_replacement
BEFORE INSERT ON catalog_relationship_targets
WHEN EXISTS (SELECT 1 FROM catalog_relationship_targets WHERE target_id = NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'relationship target identities are immutable'); END;
CREATE TRIGGER catalog_relationship_targets_immutable_update
BEFORE UPDATE ON catalog_relationship_targets
BEGIN SELECT RAISE(ABORT, 'relationship target identities are immutable'); END;
CREATE TRIGGER catalog_relationship_targets_immutable_delete
BEFORE DELETE ON catalog_relationship_targets
BEGIN SELECT RAISE(ABORT, 'relationship target identities are immutable'); END;

CREATE TRIGGER catalog_set_targets_validate_insert
BEFORE INSERT ON catalog_set_targets
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationship_targets AS target
    JOIN catalog_sets AS sets ON sets.set_id = NEW.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE target.target_id = NEW.target_id AND target.kind = 'catalog_set'
      AND ((groups.kind = 'root' AND sets.source_element_kind IN (
              'mame_machine','logiqx_game','cmp_set','no_intro_pc_game','no_intro_dat_game','no_intro_database_game'
           ) AND ((parser.format='mame-listxml' AND sets.source_element_kind='mame_machine')
               OR (parser.format='logiqx' AND sets.source_element_kind='logiqx_game')
               OR (parser.format='clrmamepro-dat' AND sets.source_element_kind='cmp_set')
               OR (parser.format='no-intro-pc-xml' AND sets.source_element_kind='no_intro_pc_game')
               OR (parser.format IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible','no-intro-dat-v4-strict','no-intro-dat-v4-compatible') AND sets.source_element_kind='no_intro_dat_game')
               OR (parser.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible') AND sets.source_element_kind='no_intro_database_game')))
           OR (groups.kind='software_list' AND sets.source_element_kind='software_item' AND parser.format='mame-softwarelist-xml'))
)
 OR EXISTS (SELECT 1 FROM catalog_set_targets WHERE target_id = NEW.target_id)
 OR EXISTS (SELECT 1 FROM catalog_set_targets WHERE set_id = NEW.set_id)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'catalog set target requires one valid native set owner'); END;
CREATE TRIGGER catalog_set_targets_immutable_update BEFORE UPDATE ON catalog_set_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER catalog_set_targets_immutable_delete BEFORE DELETE ON catalog_set_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER catalog_media_entry_targets_validate_insert
BEFORE INSERT ON catalog_media_entry_targets
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationship_targets AS target
    JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=NEW.occurrence_id
    JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE target.target_id=NEW.target_id AND target.kind='catalog_media_entry'
      AND ((parser.format='mame-listxml' AND occurrence.claim_kind IN ('mame_rom','mame_disk','mame_sample'))
        OR (parser.format='logiqx' AND occurrence.claim_kind IN ('logiqx_rom','logiqx_disk','logiqx_sample'))
        OR (parser.format='clrmamepro-dat' AND occurrence.claim_kind IN ('cmp_rom','cmp_sample'))
        OR (parser.format='no-intro-pc-xml' AND occurrence.claim_kind='no_intro_pc_file')
        OR (parser.format IN ('no-intro-dat-v3-strict','no-intro-dat-v3-compatible','no-intro-dat-v4-strict','no-intro-dat-v4-compatible') AND occurrence.claim_kind='no_intro_dat_rom')
        OR (parser.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible') AND occurrence.claim_kind IN ('no_intro_database_source_file','no_intro_database_release_file'))
        OR (parser.format='mame-softwarelist-xml' AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation','software_disk_entry')))
)
 OR EXISTS (SELECT 1 FROM catalog_media_entry_targets WHERE target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM catalog_media_entry_targets WHERE occurrence_id=NEW.occurrence_id)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'media target requires one valid native occurrence owner'); END;
CREATE TRIGGER catalog_media_entry_targets_immutable_update BEFORE UPDATE ON catalog_media_entry_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER catalog_media_entry_targets_immutable_delete BEFORE DELETE ON catalog_media_entry_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER shared_file_targets_validate_insert
BEFORE INSERT ON shared_file_targets
WHEN typeof(NEW.content_uuid) <> 'blob' OR length(NEW.content_uuid) <> 16
 OR NOT EXISTS (SELECT 1 FROM catalog_relationship_targets WHERE target_id=NEW.target_id AND kind='shared_file')
 OR NOT EXISTS (SELECT 1 FROM catalog_contents WHERE content_uuid=NEW.content_uuid)
 OR EXISTS (SELECT 1 FROM shared_file_targets WHERE target_id=NEW.target_id OR content_uuid=NEW.content_uuid)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'shared file target requires one interned catalog UUID'); END;
CREATE TRIGGER shared_file_targets_immutable_update BEFORE UPDATE ON shared_file_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER shared_file_targets_immutable_delete BEFORE DELETE ON shared_file_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER no_intro_archive_targets_validate_insert
BEFORE INSERT ON no_intro_archive_targets
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationship_targets AS target
    JOIN no_intro_archive_descriptions AS archive ON archive.archive_id=NEW.archive_id
    JOIN no_intro_database_games AS native ON native.set_id=archive.set_id
    JOIN catalog_sets AS sets ON sets.set_id=native.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS parser USING(interpretation_key)
    WHERE target.target_id=NEW.target_id AND target.kind='no_intro_archive'
      AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
      AND parser.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible')
)
 OR EXISTS (SELECT 1 FROM no_intro_archive_targets WHERE target_id=NEW.target_id OR archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'archive target requires one valid database-export archive'); END;
CREATE TRIGGER no_intro_archive_targets_immutable_update BEFORE UPDATE ON no_intro_archive_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER no_intro_archive_targets_immutable_delete BEFORE DELETE ON no_intro_archive_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER external_catalog_targets_validate_insert
BEFORE INSERT ON external_catalog_targets
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationship_targets WHERE target_id=NEW.target_id AND kind='external_record')
 OR EXISTS (SELECT 1 FROM external_catalog_targets WHERE target_id=NEW.target_id OR (namespace=NEW.namespace AND declared_key=NEW.declared_key))
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'external catalog target must be interned exactly once'); END;
CREATE TRIGGER external_catalog_targets_immutable_update BEFORE UPDATE ON external_catalog_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER external_catalog_targets_immutable_delete BEFORE DELETE ON external_catalog_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER declared_digest_targets_validate_insert
BEFORE INSERT ON declared_digest_targets
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationship_targets WHERE target_id=NEW.target_id AND kind='declared_digest')
 OR NOT EXISTS (SELECT 1 FROM digest_values WHERE digest_id=NEW.digest_id)
 OR EXISTS (SELECT 1 FROM declared_digest_targets WHERE target_id=NEW.target_id OR digest_id=NEW.digest_id)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'declared digest target must reference one interned digest'); END;
CREATE TRIGGER declared_digest_targets_immutable_update BEFORE UPDATE ON declared_digest_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER declared_digest_targets_immutable_delete BEFORE DELETE ON declared_digest_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER unresolved_catalog_targets_validate_insert
BEFORE INSERT ON unresolved_catalog_targets
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationship_targets WHERE target_id=NEW.target_id AND kind='unresolved_catalog')
 OR NOT EXISTS (SELECT 1 FROM catalog_snapshots WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM unresolved_catalog_targets
            WHERE target_id=NEW.target_id
               OR (snapshot_key=NEW.snapshot_key AND record_kind=NEW.record_kind
                   AND declared_name=NEW.declared_name
                   AND declared_set_name IS NEW.declared_set_name
                   AND declared_list_name IS NEW.declared_list_name
                   AND declared_media_order IS NEW.declared_media_order))
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
 OR EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE from_target_id=NEW.target_id OR to_target_id=NEW.target_id)
BEGIN SELECT RAISE(ABORT, 'unresolved catalog target requires a snapshot and closed literal shape'); END;
CREATE TRIGGER unresolved_catalog_targets_immutable_update BEFORE UPDATE ON unresolved_catalog_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;
CREATE TRIGGER unresolved_catalog_targets_immutable_delete BEFORE DELETE ON unresolved_catalog_targets
BEGIN SELECT RAISE(ABORT, 'catalog relationship targets are immutable'); END;

CREATE TRIGGER inferred_catalog_relationships_validate_insert
BEFORE INSERT ON inferred_catalog_relationships
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    JOIN catalog_relationship_target_details AS source ON source.target_id=NEW.from_target_id
    JOIN catalog_relationship_target_details AS target ON target.target_id=NEW.to_target_id
    JOIN catalog_relationship_rules AS rule ON rule.rule_id=NEW.rule_id
    WHERE identity.relationship_id=NEW.relationship_id AND identity.origin='derived'
      AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id=identity.relationship_id)
      AND NOT EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id=identity.relationship_id)
      AND NOT EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id=identity.relationship_id)
)
BEGIN SELECT RAISE(ABORT, 'inferred relationship requires an unused identity, rule, and two complete targets'); END;
CREATE TRIGGER inferred_catalog_relationships_immutable_update BEFORE UPDATE ON inferred_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'inferred catalog relationships are immutable'); END;
CREATE TRIGGER inferred_catalog_relationships_immutable_delete BEFORE DELETE ON inferred_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'inferred catalog relationships are immutable'); END;

CREATE TRIGGER manual_catalog_relationships_validate_insert
BEFORE INSERT ON manual_catalog_relationships
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    JOIN catalog_relationship_target_details AS source ON source.target_id=NEW.from_target_id
    JOIN catalog_relationship_target_details AS target ON target.target_id=NEW.to_target_id
    WHERE identity.relationship_id=NEW.relationship_id AND identity.origin='user'
      AND NOT EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id=identity.relationship_id)
      AND NOT EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id=identity.relationship_id)
      AND NOT EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id=identity.relationship_id)
)
BEGIN SELECT RAISE(ABORT, 'manual relationship requires an unused identity and two complete targets'); END;
CREATE TRIGGER manual_catalog_relationships_immutable_update BEFORE UPDATE ON manual_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'manual catalog relationships are immutable'); END;
CREATE TRIGGER manual_catalog_relationships_immutable_delete BEFORE DELETE ON manual_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'manual catalog relationships are immutable'); END;
