-- A review is drafted and published in one transaction. Only the final marker
-- gives it any effect; source occurrences and conflict evidence never change.
CREATE TABLE file_match_decisions (
    decision_id INTEGER PRIMARY KEY CHECK (decision_id > 0),
    decision TEXT NOT NULL CHECK (decision IN ('keep_separate', 'merge')),
    kept_content_uuid BLOB REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    rationale TEXT NOT NULL CHECK (length(trim(rationale)) > 0),
    decided_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK ((decision = 'merge') = (kept_content_uuid IS NOT NULL))
);
CREATE TABLE file_match_decision_conflicts (
    decision_id INTEGER NOT NULL REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT,
    occurrence_id INTEGER NOT NULL,
    candidate_content_uuid BLOB NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('keep_separate', 'merged')),
    PRIMARY KEY (decision_id, occurrence_id, candidate_content_uuid),
    UNIQUE (occurrence_id, candidate_content_uuid),
    FOREIGN KEY (occurrence_id, candidate_content_uuid)
        REFERENCES occurrence_content_conflicts(occurrence_id, candidate_content_uuid) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE file_match_hash_decisions (
    decision_id INTEGER NOT NULL,
    occurrence_id INTEGER NOT NULL,
    candidate_content_uuid BLOB NOT NULL,
    evidence_occurrence_id INTEGER NOT NULL,
    digest_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    provenance TEXT NOT NULL DEFAULT 'source_declared' CHECK (provenance = 'source_declared'),
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    disposition TEXT NOT NULL CHECK (disposition IN ('accept', 'reject')),
    PRIMARY KEY (decision_id, occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role),
    FOREIGN KEY (decision_id, occurrence_id, candidate_content_uuid)
        REFERENCES file_match_decision_conflicts(decision_id, occurrence_id, candidate_content_uuid) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role)
        REFERENCES occurrence_content_conflict_hashes(occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, scope, provenance, role) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE INDEX rejected_file_hash_lookup ON file_match_hash_decisions(evidence_occurrence_id, digest_id, scope, provenance, disposition, decision_id);
CREATE TABLE file_match_size_decisions (
    decision_id INTEGER NOT NULL,
    occurrence_id INTEGER NOT NULL,
    candidate_content_uuid BLOB NOT NULL,
    evidence_occurrence_id INTEGER NOT NULL,
    size_field TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('incoming', 'candidate')),
    disposition TEXT NOT NULL CHECK (disposition IN ('accept', 'reject')),
    PRIMARY KEY (decision_id, occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role),
    FOREIGN KEY (decision_id, occurrence_id, candidate_content_uuid)
        REFERENCES file_match_decision_conflicts(decision_id, occurrence_id, candidate_content_uuid) ON DELETE RESTRICT,
    FOREIGN KEY (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role)
        REFERENCES occurrence_content_conflict_sizes(occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE INDEX rejected_file_size_lookup ON file_match_size_decisions(evidence_occurrence_id, size_field, disposition, decision_id);
CREATE TABLE merged_file_ids (
    old_content_uuid BLOB PRIMARY KEY NOT NULL REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    kept_content_uuid BLOB NOT NULL REFERENCES catalog_contents(content_uuid) ON DELETE RESTRICT,
    decision_id INTEGER NOT NULL REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT,
    CHECK (old_content_uuid <> kept_content_uuid)
) WITHOUT ROWID;
CREATE INDEX merged_file_kept_lookup ON merged_file_ids(kept_content_uuid, old_content_uuid);
CREATE TABLE file_match_decision_publications (
    decision_id INTEGER PRIMARY KEY REFERENCES file_match_decisions(decision_id) ON DELETE RESTRICT
);

-- Correlated paths start from a selected occurrence, not every issued UUID.
CREATE VIEW canonical_occurrence_content AS
SELECT occurrence.occurrence_id,
    (WITH RECURSIVE path(content_uuid) AS (
        SELECT occurrence.content_uuid
        UNION ALL
        SELECT redirect.kept_content_uuid FROM path
        JOIN merged_file_ids AS redirect ON redirect.old_content_uuid = path.content_uuid
        JOIN file_match_decision_publications USING (decision_id)
    ) SELECT content_uuid FROM path WHERE NOT EXISTS (
        SELECT 1 FROM merged_file_ids AS redirect
        JOIN file_match_decision_publications USING (decision_id)
        WHERE redirect.old_content_uuid = path.content_uuid
    )) AS content_uuid
FROM asset_occurrences AS occurrence WHERE occurrence.content_uuid IS NOT NULL;

CREATE VIEW accepted_file_size_assertions AS
SELECT sizes.* FROM catalog_file_size_assertions AS sizes WHERE NOT EXISTS (
    SELECT 1 FROM file_match_size_decisions AS review
    JOIN file_match_decision_publications USING (decision_id)
    WHERE review.evidence_occurrence_id = sizes.occurrence_id
      AND review.size_field = sizes.size_field AND review.disposition = 'reject'
);

-- Source qualification is shared by linked native-format publication guards.
CREATE VIEW source_file_size_assertions AS
SELECT size.* FROM accepted_file_size_assertions AS size
JOIN asset_occurrences AS occurrence USING (occurrence_id)
WHERE CASE occurrence.claim_kind
    WHEN 'mame_rom' THEN EXISTS (SELECT 1 FROM mame_rom_claims WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'logiqx_rom' THEN EXISTS (SELECT 1 FROM logiqx_rom_claims WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'cmp_rom' THEN EXISTS (SELECT 1 FROM cmp_rom_claims WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'no_intro_pc_file' THEN EXISTS (SELECT 1 FROM no_intro_pc_file_claims WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'no_intro_dat_rom' THEN EXISTS (SELECT 1 FROM no_intro_dat_rom_claims WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'no_intro_database_source_file' THEN EXISTS (SELECT 1 FROM no_intro_dump_files WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'no_intro_database_release_file' THEN EXISTS (SELECT 1 FROM no_intro_release_files WHERE occurrence_id = size.occurrence_id AND evidence_provenance = 'source_declared')
    WHEN 'software_rom_entry' THEN 1
    ELSE 0 END;

CREATE TRIGGER file_match_conflict_review_owner BEFORE INSERT ON file_match_decision_conflicts
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    JOIN occurrence_content_conflicts AS conflict
      ON conflict.occurrence_id = NEW.occurrence_id AND conflict.candidate_content_uuid = NEW.candidate_content_uuid
    WHERE decision.decision_id = NEW.decision_id
      AND NEW.outcome = CASE decision.decision WHEN 'merge' THEN 'merged' ELSE 'keep_separate' END
) OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'terminal review requires its exact conflict and unpublished decision'); END;

CREATE TRIGGER file_match_hash_review_owner BEFORE INSERT ON file_match_hash_decisions
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_decision_conflicts AS reviewed
    JOIN occurrence_content_conflict_hashes AS evidence
      ON evidence.occurrence_id = reviewed.occurrence_id AND evidence.candidate_content_uuid = reviewed.candidate_content_uuid
    WHERE reviewed.decision_id = NEW.decision_id AND reviewed.occurrence_id = NEW.occurrence_id
      AND reviewed.candidate_content_uuid = NEW.candidate_content_uuid
      AND evidence.evidence_occurrence_id = NEW.evidence_occurrence_id AND evidence.digest_id = NEW.digest_id
      AND evidence.scope = NEW.scope AND evidence.provenance = NEW.provenance AND evidence.role = NEW.role
) OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'hash review requires its exact immutable conflict evidence'); END;

CREATE TRIGGER file_match_size_review_owner BEFORE INSERT ON file_match_size_decisions
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_decision_conflicts AS reviewed
    JOIN occurrence_content_conflict_sizes AS evidence
      ON evidence.occurrence_id = reviewed.occurrence_id AND evidence.candidate_content_uuid = reviewed.candidate_content_uuid
    WHERE reviewed.decision_id = NEW.decision_id AND reviewed.occurrence_id = NEW.occurrence_id
      AND reviewed.candidate_content_uuid = NEW.candidate_content_uuid
      AND evidence.evidence_occurrence_id = NEW.evidence_occurrence_id AND evidence.size_field = NEW.size_field AND evidence.role = NEW.role
) OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'size review requires its exact immutable conflict evidence'); END;

CREATE TRIGGER merged_file_review_owner BEFORE INSERT ON merged_file_ids
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    JOIN catalog_contents AS old ON old.content_uuid = NEW.old_content_uuid
    JOIN catalog_contents AS kept ON kept.content_uuid = NEW.kept_content_uuid AND kept.registry_id = old.registry_id
    WHERE decision.decision_id = NEW.decision_id AND decision.decision = 'merge'
      AND decision.kept_content_uuid = NEW.kept_content_uuid
      AND EXISTS (SELECT 1 FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id AND candidate_content_uuid = NEW.old_content_uuid)
      AND EXISTS (SELECT 1 FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id AND candidate_content_uuid = NEW.kept_content_uuid)
      AND EXISTS (
          SELECT 1 FROM file_match_decision_conflicts AS old_conflict
          JOIN file_match_decision_conflicts AS kept_conflict USING (decision_id, occurrence_id)
          WHERE old_conflict.decision_id = NEW.decision_id
            AND old_conflict.candidate_content_uuid = NEW.old_content_uuid
            AND kept_conflict.candidate_content_uuid = NEW.kept_content_uuid
      )
      AND NOT EXISTS (
          SELECT 1 FROM merged_file_ids JOIN file_match_decision_publications USING (decision_id)
          WHERE old_content_uuid IN (NEW.old_content_uuid, NEW.kept_content_uuid)
      )
) OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'UUID redirect requires an explicit same-registry reviewed conflict merge'); END;
CREATE TRIGGER merged_file_cycle BEFORE INSERT ON merged_file_ids
WHEN EXISTS (
    WITH RECURSIVE path(content_uuid) AS (
        SELECT NEW.kept_content_uuid
        UNION
        SELECT redirect.kept_content_uuid FROM merged_file_ids AS redirect
        JOIN path ON redirect.old_content_uuid = path.content_uuid
    ) SELECT 1 FROM path WHERE content_uuid = NEW.old_content_uuid
)
BEGIN SELECT RAISE(ABORT, 'UUID redirect cannot create a cycle'); END;

CREATE TRIGGER file_match_publication_owner BEFORE INSERT ON file_match_decision_publications
WHEN NOT EXISTS (SELECT 1 FROM file_match_decisions WHERE decision_id = NEW.decision_id)
 OR NOT EXISTS (SELECT 1 FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id)
 OR EXISTS (
    SELECT 1 FROM file_match_decision_conflicts AS reviewed
    WHERE reviewed.decision_id = NEW.decision_id
      AND NOT EXISTS (
          SELECT 1 FROM asset_occurrences AS occurrence
          JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
          JOIN catalog_set_groups AS groups USING (set_group_id)
          JOIN snapshot_publications USING (snapshot_key)
          WHERE occurrence.occurrence_id = reviewed.occurrence_id
      )
 )
 OR EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    WHERE decision.decision_id = NEW.decision_id AND decision.decision = 'merge'
      AND NOT EXISTS (SELECT 1 FROM merged_file_ids WHERE decision_id = NEW.decision_id)
 )
BEGIN SELECT RAISE(ABORT, 'review publication requires published source conflicts and a complete action'); END;

CREATE TRIGGER file_match_reviewed_sources_published BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (
    WITH RECURSIVE members(content_uuid) AS (
        SELECT candidate_content_uuid FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id
        UNION
        SELECT redirect.old_content_uuid FROM merged_file_ids AS redirect
        JOIN members ON redirect.kept_content_uuid = members.content_uuid
        WHERE redirect.decision_id = NEW.decision_id OR EXISTS (
            SELECT 1 FROM file_match_decision_publications WHERE decision_id = redirect.decision_id
        )
    ), owners(occurrence_id) AS (
        SELECT occurrence.occurrence_id FROM members
        CROSS JOIN asset_occurrences AS occurrence ON occurrence.content_uuid = members.content_uuid
        UNION
        SELECT evidence.evidence_occurrence_id FROM file_match_decision_conflicts AS reviewed
        JOIN occurrence_content_conflict_hashes AS evidence USING (occurrence_id, candidate_content_uuid)
        WHERE reviewed.decision_id = NEW.decision_id
        UNION
        SELECT evidence.evidence_occurrence_id FROM file_match_decision_conflicts AS reviewed
        JOIN occurrence_content_conflict_sizes AS evidence USING (occurrence_id, candidate_content_uuid)
        WHERE reviewed.decision_id = NEW.decision_id
    )
    SELECT 1 FROM owners WHERE NOT EXISTS (
        SELECT 1 FROM asset_occurrences AS occurrence
        JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
        JOIN catalog_set_groups AS groups USING (set_group_id)
        JOIN snapshot_publications USING (snapshot_key)
        WHERE occurrence.occurrence_id = owners.occurrence_id
    )
)
BEGIN SELECT RAISE(ABORT, 'all reviewed evidence and candidate component sources must be published'); END;

CREATE TRIGGER file_match_merge_complete BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    WHERE decision.decision_id = NEW.decision_id AND decision.decision = 'merge' AND (
        EXISTS (SELECT 1 FROM merged_file_ids AS redirect JOIN file_match_decision_publications USING (decision_id)
                WHERE redirect.old_content_uuid = decision.kept_content_uuid)
        OR EXISTS (
            SELECT 1 FROM file_match_decision_conflicts AS reviewed
            WHERE reviewed.decision_id = NEW.decision_id
              AND reviewed.candidate_content_uuid <> decision.kept_content_uuid
              AND NOT EXISTS (
                  SELECT 1 FROM merged_file_ids AS redirect
                  WHERE redirect.old_content_uuid = reviewed.candidate_content_uuid AND redirect.decision_id = NEW.decision_id
                    AND redirect.kept_content_uuid = decision.kept_content_uuid
              )
        )
    )
)
BEGIN SELECT RAISE(ABORT, 'merge must redirect every named current candidate to its kept UUID'); END;

CREATE TRIGGER content_conflict_actual_owner BEFORE INSERT ON occurrence_content_conflicts
WHEN NOT EXISTS (SELECT 1 FROM asset_occurrences WHERE occurrence_id = NEW.occurrence_id AND content_uuid IS NULL)
  OR NOT EXISTS (SELECT 1 FROM catalog_contents WHERE content_uuid = NEW.candidate_content_uuid)
BEGIN SELECT RAISE(ABORT, 'content conflict requires an actual unlinked incoming entry and issued candidate UUID'); END;

CREATE TRIGGER conflict_hash_actual_evidence BEFORE INSERT ON occurrence_content_conflict_hashes
WHEN NOT EXISTS (SELECT 1 FROM occurrence_content_conflicts WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid)
  OR NOT EXISTS (
      SELECT 1 FROM occurrence_digest_assertions WHERE occurrence_id = NEW.evidence_occurrence_id AND digest_id = NEW.digest_id
        AND scope = NEW.scope AND provenance = NEW.provenance
  )
  OR EXISTS (
      SELECT 1 FROM occurrence_content_conflict_hashes WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid
        AND evidence_occurrence_id = NEW.evidence_occurrence_id AND digest_id = NEW.digest_id AND scope = NEW.scope AND provenance = NEW.provenance AND role = NEW.role
  )
BEGIN SELECT RAISE(ABORT, 'conflict hash requires exact source evidence without replacement'); END;

CREATE TRIGGER conflict_size_actual_evidence BEFORE INSERT ON occurrence_content_conflict_sizes
WHEN NOT EXISTS (SELECT 1 FROM occurrence_content_conflicts WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid)
  OR EXISTS (
      SELECT 1 FROM occurrence_content_conflict_sizes WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid
        AND evidence_occurrence_id = NEW.evidence_occurrence_id AND size_field = NEW.size_field AND role = NEW.role
  )
BEGIN SELECT RAISE(ABORT, 'conflict size requires its actual source conflict without replacement'); END;

-- Check a merge against all retained candidate facts, including aliases from
-- earlier merges. Only exact rejected fields are excluded. Draft rejections in
-- this decision are effective for this validation, never for ordinary lookup.
CREATE TRIGGER file_match_merge_hash_consistency BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (
    WITH RECURSIVE members(content_uuid) AS (
        SELECT kept_content_uuid FROM file_match_decisions WHERE decision_id = NEW.decision_id AND decision = 'merge'
        UNION
        SELECT redirect.old_content_uuid FROM merged_file_ids AS redirect JOIN members ON redirect.kept_content_uuid = members.content_uuid
        WHERE redirect.decision_id = NEW.decision_id OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = redirect.decision_id)
    ), facts(occurrence_id, digest_id, scope, provenance) AS (
        SELECT assertion.occurrence_id, assertion.digest_id, assertion.scope, assertion.provenance
        FROM members CROSS JOIN asset_occurrences AS occurrence ON occurrence.content_uuid = members.content_uuid
        CROSS JOIN occurrence_digest_assertions AS assertion USING (occurrence_id)
        WHERE assertion.provenance = 'source_declared' AND assertion.scope IN ('whole_asset', 'whole_file')
        UNION
        SELECT evidence_occurrence_id, digest_id, scope, provenance FROM file_match_hash_decisions
        WHERE decision_id = NEW.decision_id AND role = 'incoming' AND disposition = 'accept'
    )
    SELECT 1 FROM facts AS assertion
    CROSS JOIN digest_values USING (digest_id)
    WHERE assertion.provenance = 'source_declared' AND assertion.scope IN ('whole_asset', 'whole_file')
      AND NOT EXISTS (
        SELECT 1 FROM file_match_hash_decisions AS review WHERE review.evidence_occurrence_id = assertion.occurrence_id
          AND review.digest_id = assertion.digest_id AND review.scope = assertion.scope AND review.provenance = assertion.provenance
          AND review.disposition = 'reject' AND (review.decision_id = NEW.decision_id OR EXISTS (
              SELECT 1 FROM file_match_decision_publications WHERE decision_id = review.decision_id
          ))
      )
    GROUP BY algorithm HAVING count(DISTINCT digest_id) > 1
)
BEGIN SELECT RAISE(ABORT, 'merge retains contradictory whole-file hash evidence'); END;
CREATE TRIGGER file_match_merge_size_consistency BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (
    WITH RECURSIVE members(content_uuid) AS (
        SELECT kept_content_uuid FROM file_match_decisions WHERE decision_id = NEW.decision_id AND decision = 'merge'
        UNION
        SELECT redirect.old_content_uuid FROM merged_file_ids AS redirect JOIN members ON redirect.kept_content_uuid = members.content_uuid
        WHERE redirect.decision_id = NEW.decision_id OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = redirect.decision_id)
    ), facts(occurrence_id, size_field, size) AS (
        SELECT sizes.occurrence_id, sizes.size_field, sizes.size FROM members
        CROSS JOIN asset_occurrences AS occurrence ON occurrence.content_uuid = members.content_uuid
        CROSS JOIN catalog_file_size_assertions AS sizes USING (occurrence_id)
        UNION
        SELECT sizes.occurrence_id, sizes.size_field, sizes.size FROM file_match_size_decisions AS accepted
        CROSS JOIN catalog_file_size_assertions AS sizes
          ON sizes.occurrence_id = accepted.evidence_occurrence_id AND sizes.size_field = accepted.size_field
        WHERE accepted.decision_id = NEW.decision_id AND accepted.role = 'incoming' AND accepted.disposition = 'accept'
    )
    SELECT count(DISTINCT size) FROM facts AS sizes
    WHERE NOT EXISTS (
        SELECT 1 FROM file_match_size_decisions AS review WHERE review.evidence_occurrence_id = sizes.occurrence_id
          AND review.size_field = sizes.size_field AND review.disposition = 'reject'
          AND (review.decision_id = NEW.decision_id OR EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = review.decision_id))
    )
    HAVING count(DISTINCT size) > 1
)
BEGIN SELECT RAISE(ABORT, 'merge retains contradictory qualified file sizes'); END;

CREATE TRIGGER file_match_decisions_immutable_update BEFORE UPDATE ON file_match_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decisions_immutable_delete BEFORE DELETE ON file_match_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decisions_immutable_replace BEFORE INSERT ON file_match_decisions
WHEN EXISTS (SELECT 1 FROM file_match_decisions WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER file_match_decision_conflicts_immutable_update BEFORE UPDATE ON file_match_decision_conflicts
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decision_conflicts_immutable_delete BEFORE DELETE ON file_match_decision_conflicts
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decision_conflicts_immutable_replace BEFORE INSERT ON file_match_decision_conflicts
WHEN EXISTS (SELECT 1 FROM file_match_decision_conflicts WHERE occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER file_match_hash_decisions_immutable_update BEFORE UPDATE ON file_match_hash_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_hash_decisions_immutable_delete BEFORE DELETE ON file_match_hash_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_hash_decisions_immutable_replace BEFORE INSERT ON file_match_hash_decisions
WHEN EXISTS (SELECT 1 FROM file_match_hash_decisions WHERE decision_id = NEW.decision_id AND occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid AND evidence_occurrence_id = NEW.evidence_occurrence_id AND digest_id = NEW.digest_id AND scope = NEW.scope AND provenance = NEW.provenance AND role = NEW.role)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER file_match_size_decisions_immutable_update BEFORE UPDATE ON file_match_size_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_size_decisions_immutable_delete BEFORE DELETE ON file_match_size_decisions
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_size_decisions_immutable_replace BEFORE INSERT ON file_match_size_decisions
WHEN EXISTS (SELECT 1 FROM file_match_size_decisions WHERE decision_id = NEW.decision_id AND occurrence_id = NEW.occurrence_id AND candidate_content_uuid = NEW.candidate_content_uuid AND evidence_occurrence_id = NEW.evidence_occurrence_id AND size_field = NEW.size_field AND role = NEW.role)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER merged_file_ids_immutable_update BEFORE UPDATE ON merged_file_ids
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER merged_file_ids_immutable_delete BEFORE DELETE ON merged_file_ids
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER merged_file_ids_immutable_replace BEFORE INSERT ON merged_file_ids
WHEN EXISTS (SELECT 1 FROM merged_file_ids WHERE old_content_uuid = NEW.old_content_uuid)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER file_match_decision_publications_immutable_update BEFORE UPDATE ON file_match_decision_publications
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decision_publications_immutable_delete BEFORE DELETE ON file_match_decision_publications
BEGIN SELECT RAISE(ABORT, 'file identity reviews are append-only'); END;
CREATE TRIGGER file_match_decision_publications_immutable_replace BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
BEGIN SELECT RAISE(ABORT, 'file identity review replacement is forbidden'); END;

CREATE TRIGGER occurrence_content_conflicts_published_insert BEFORE INSERT ON occurrence_content_conflicts
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence
    JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
    JOIN catalog_set_groups AS groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence.occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published source conflict evidence is immutable'); END;

CREATE TRIGGER occurrence_content_conflict_hashes_published_insert BEFORE INSERT ON occurrence_content_conflict_hashes
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence
    JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
    JOIN catalog_set_groups AS groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence.occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published source conflict evidence is immutable'); END;

CREATE TRIGGER occurrence_content_conflict_sizes_published_insert BEFORE INSERT ON occurrence_content_conflict_sizes
WHEN EXISTS (
    SELECT 1 FROM asset_occurrences AS occurrence
    JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id
    JOIN catalog_set_groups AS groups USING (set_group_id)
    JOIN snapshot_publications USING (snapshot_key)
    WHERE occurrence.occurrence_id = NEW.occurrence_id
)
BEGIN SELECT RAISE(ABORT, 'published source conflict evidence is immutable'); END;
