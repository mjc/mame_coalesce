-- DESIGN ONLY. Guard/publication fragment for relationships.sql.
-- Bounded witness scope: apply after shared.sql, all native owner/position
-- fragments, and relationships.sql. This file uses the real shared core and
-- real closed reported-kind registry; it defines no placeholder registry or
-- duplicate shared size/hash facts. A harness that substitutes minimal
-- placeholder kinds can witness only the local trigger behavior it inserts;
-- that does not establish the 22-kind native owner/position closure, edition
-- ancestry, or production import behavior.

CREATE TRIGGER reported_relationship_identity_guard
BEFORE INSERT ON reported_catalog_relationships
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS relationship
    WHERE relationship.relationship_id = NEW.relationship_id
      AND relationship.origin = 'source'
      AND relationship.edition_id IS NOT NULL
)
BEGIN
    SELECT RAISE(ABORT, 'reported relationship requires a reported identity and edition');
END;

CREATE TRIGGER reported_relationship_identity_immutable_update
BEFORE UPDATE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported relationship identity is immutable'); END;
CREATE TRIGGER reported_relationship_identity_immutable_delete
BEFORE DELETE ON reported_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'reported relationship identity is immutable'); END;
CREATE TRIGGER catalog_relationship_identity_immutable_update
BEFORE UPDATE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'relationship identity is immutable'); END;
CREATE TRIGGER catalog_relationship_identity_immutable_delete
BEFORE DELETE ON catalog_relationships
BEGIN SELECT RAISE(ABORT, 'relationship identity is immutable'); END;

CREATE TRIGGER manual_relationship_origin_guard
BEFORE INSERT ON manual_catalog_relationships
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationships
                 WHERE relationship_id = NEW.relationship_id AND origin = 'user' AND edition_id IS NULL)
BEGIN SELECT RAISE(ABORT, 'manual relationship requires a user identity'); END;
CREATE TRIGGER inferred_relationship_origin_guard
BEFORE INSERT ON inferred_catalog_relationships
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationships
                 WHERE relationship_id = NEW.relationship_id AND origin = 'derived' AND edition_id IS NULL)
BEGIN SELECT RAISE(ABORT, 'inferred relationship requires a derived identity'); END;
CREATE TRIGGER manual_relationship_single_origin_guard
BEFORE INSERT ON manual_catalog_relationships
WHEN EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id = NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id = NEW.relationship_id)
BEGIN SELECT RAISE(ABORT, 'relationship identity already has another origin payload'); END;
CREATE TRIGGER inferred_relationship_single_origin_guard
BEFORE INSERT ON inferred_catalog_relationships
WHEN EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id = NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationships WHERE relationship_id = NEW.relationship_id)
BEGIN SELECT RAISE(ABORT, 'relationship identity already has another origin payload'); END;
CREATE TRIGGER reported_relationship_single_origin_guard
BEFORE INSERT ON reported_catalog_relationships
WHEN EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id = NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id = NEW.relationship_id)
BEGIN SELECT RAISE(ABORT, 'relationship identity already has another origin payload'); END;

CREATE TRIGGER relationship_evidence_publication_guard
BEFORE INSERT ON catalog_relationship_evidence_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_relationship_evidence_publications AS existing
    WHERE existing.relationship_id = NEW.relationship_id
)
 OR NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS relationship
    WHERE relationship.relationship_id = NEW.relationship_id
      AND relationship.origin IN ('derived', 'user')
)
 OR (NEW.evidence_kind = 'rationale' AND (
       NOT EXISTS (SELECT 1 FROM catalog_relationship_rationales WHERE relationship_id = NEW.relationship_id)
       OR EXISTS (SELECT 1 FROM catalog_relationship_comparisons WHERE relationship_id = NEW.relationship_id)
    ))
 OR (NEW.evidence_kind = 'catalog_comparison' AND (
       NOT EXISTS (SELECT 1 FROM catalog_relationship_comparisons WHERE relationship_id = NEW.relationship_id)
       OR EXISTS (SELECT 1 FROM catalog_relationship_rationales WHERE relationship_id = NEW.relationship_id)
    ))
BEGIN SELECT RAISE(ABORT, 'relationship evidence publication is missing or mismatched'); END;

CREATE TRIGGER relationship_review_publication_guard
BEFORE INSERT ON catalog_relationship_review_publications
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationship_reviews WHERE review_id = NEW.review_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_review_publications WHERE review_id = NEW.review_id)
 OR NOT EXISTS (
     SELECT 1 FROM catalog_relationship_reviews AS review
     JOIN catalog_relationships AS relationship USING (relationship_id)
     WHERE review.review_id = NEW.review_id
       AND ((relationship.origin = 'source' AND EXISTS (
              SELECT 1 FROM published_catalog_editions AS edition
              WHERE edition.edition_id = relationship.edition_id))
         OR (relationship.origin IN ('derived','user') AND EXISTS (
              SELECT 1 FROM catalog_relationship_evidence_publications AS evidence
              WHERE evidence.relationship_id = relationship.relationship_id)))
 )
 OR EXISTS (
     SELECT 1 FROM catalog_relationship_reviews AS review
     WHERE review.review_id = NEW.review_id AND (
       (review.decision = 'superseded') <> EXISTS (
          SELECT 1 FROM replaced_catalog_relationships AS replacement
          WHERE replacement.review_id = review.review_id)
       OR EXISTS (SELECT 1 FROM replaced_catalog_relationships AS replacement
                  WHERE replacement.review_id = review.review_id
                    AND replacement.replacement_relationship_id = review.relationship_id)
     )
 )
BEGIN SELECT RAISE(ABORT, 'relationship review is incomplete or inconsistent'); END;

CREATE TRIGGER relationship_review_append_only_update
BEFORE UPDATE ON catalog_relationship_reviews
BEGIN SELECT RAISE(ABORT, 'relationship reviews are append-only'); END;
CREATE TRIGGER relationship_review_append_only_delete
BEFORE DELETE ON catalog_relationship_reviews
BEGIN SELECT RAISE(ABORT, 'relationship reviews are append-only'); END;
CREATE TRIGGER relationship_review_no_late_replacement
BEFORE INSERT ON replaced_catalog_relationships
WHEN EXISTS (SELECT 1 FROM catalog_relationship_review_publications WHERE review_id = NEW.review_id)
BEGIN SELECT RAISE(ABORT, 'published relationship review is immutable'); END;
CREATE TRIGGER relationship_replacement_append_only_update
BEFORE UPDATE ON replaced_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'relationship replacement history is append-only'); END;
CREATE TRIGGER relationship_replacement_append_only_delete
BEFORE DELETE ON replaced_catalog_relationships
BEGIN SELECT RAISE(ABORT, 'relationship replacement history is append-only'); END;
CREATE TRIGGER relationship_review_publication_append_only_update
BEFORE UPDATE ON catalog_relationship_review_publications
BEGIN SELECT RAISE(ABORT, 'relationship review publications are immutable'); END;
CREATE TRIGGER relationship_review_publication_append_only_delete
BEFORE DELETE ON catalog_relationship_review_publications
BEGIN SELECT RAISE(ABORT, 'relationship review publications are immutable'); END;

CREATE TRIGGER relationship_replacement_cycle_guard
BEFORE INSERT ON catalog_relationship_review_publications
WHEN EXISTS (
    SELECT 1 FROM catalog_relationship_reviews AS review
    JOIN replaced_catalog_relationships AS replacement USING (review_id)
    WHERE review.review_id = NEW.review_id
      AND review.decision = 'superseded'
      AND NOT EXISTS (
          SELECT 1 FROM catalog_relationship_reviews AS newer
          JOIN catalog_relationship_review_publications AS publication USING (review_id)
          WHERE newer.relationship_id = review.relationship_id
            AND newer.review_id > review.review_id
      )
      AND EXISTS (
          WITH RECURSIVE edges(predecessor_id, replacement_id) AS (
              SELECT predecessor_relationship_id, replacement_relationship_id
              FROM active_catalog_relationship_replacements
              WHERE predecessor_relationship_id <> review.relationship_id
              UNION ALL
              SELECT review.relationship_id, replacement.replacement_relationship_id
              FROM replaced_catalog_relationships AS replacement
              WHERE replacement.review_id = review.review_id
          ), walk(start_id, current_id) AS (
              SELECT predecessor_id, replacement_id FROM edges
              UNION ALL
              SELECT walk.start_id, edges.replacement_id
              FROM walk JOIN edges ON edges.predecessor_id = walk.current_id
          )
          SELECT 1 FROM walk WHERE start_id = current_id
      )
)
BEGIN SELECT RAISE(ABORT, 'published relationship replacement would create a cycle'); END;

CREATE TRIGGER relationship_evidence_after_seal_guard
BEFORE INSERT ON catalog_relationship_evidence
WHEN EXISTS (SELECT 1 FROM catalog_relationship_evidence_publications WHERE relationship_id = NEW.relationship_id)
BEGIN SELECT RAISE(ABORT, 'published relationship evidence is immutable'); END;
CREATE TRIGGER relationship_evidence_append_only_update
BEFORE UPDATE ON catalog_relationship_evidence
BEGIN SELECT RAISE(ABORT, 'relationship evidence is append-only'); END;
CREATE TRIGGER relationship_evidence_append_only_delete
BEFORE DELETE ON catalog_relationship_evidence
BEGIN SELECT RAISE(ABORT, 'relationship evidence is append-only'); END;
CREATE TRIGGER relationship_evidence_publication_append_only_update
BEFORE UPDATE ON catalog_relationship_evidence_publications
BEGIN SELECT RAISE(ABORT, 'relationship evidence publications are immutable'); END;
CREATE TRIGGER relationship_evidence_publication_append_only_delete
BEFORE DELETE ON catalog_relationship_evidence_publications
BEGIN SELECT RAISE(ABORT, 'relationship evidence publications are immutable'); END;

CREATE TRIGGER file_match_conflict_append_only_update
BEFORE UPDATE ON file_match_conflicts
BEGIN SELECT RAISE(ABORT, 'file-match conflict evidence is immutable'); END;
CREATE TRIGGER file_match_conflict_append_only_delete
BEFORE DELETE ON file_match_conflicts
BEGIN SELECT RAISE(ABORT, 'file-match conflict evidence is immutable'); END;
CREATE TRIGGER file_match_conflict_hash_append_only_update
BEFORE UPDATE ON file_match_conflict_hashes
BEGIN SELECT RAISE(ABORT, 'file-match hash evidence is immutable'); END;
CREATE TRIGGER file_match_conflict_hash_append_only_delete
BEFORE DELETE ON file_match_conflict_hashes
BEGIN SELECT RAISE(ABORT, 'file-match hash evidence is immutable'); END;
CREATE TRIGGER file_match_conflict_size_append_only_update
BEFORE UPDATE ON file_match_conflict_sizes
BEGIN SELECT RAISE(ABORT, 'file-match size evidence is immutable'); END;
CREATE TRIGGER file_match_conflict_size_append_only_delete
BEFORE DELETE ON file_match_conflict_sizes
BEGIN SELECT RAISE(ABORT, 'file-match size evidence is immutable'); END;
CREATE TRIGGER file_match_decision_append_only_update
BEFORE UPDATE ON file_match_decisions
BEGIN SELECT RAISE(ABORT, 'file-match decisions are append-only'); END;
CREATE TRIGGER file_match_decision_append_only_delete
BEFORE DELETE ON file_match_decisions
BEGIN SELECT RAISE(ABORT, 'file-match decisions are append-only'); END;
CREATE TRIGGER file_match_decision_conflict_append_only_update
BEFORE UPDATE ON file_match_decision_conflicts
BEGIN SELECT RAISE(ABORT, 'file-match outcomes are append-only'); END;
CREATE TRIGGER file_match_decision_conflict_append_only_delete
BEFORE DELETE ON file_match_decision_conflicts
BEGIN SELECT RAISE(ABORT, 'file-match outcomes are append-only'); END;

CREATE TRIGGER file_match_conflict_hash_owner_guard
BEFORE INSERT ON file_match_conflict_hashes
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_conflicts AS conflict
    JOIN candidate_qualified_file_hashes AS hash ON hash.reported_hash_id=NEW.reported_hash_id
    JOIN catalog_media_entries AS media ON media.media_entry_id=hash.media_entry_id
    WHERE conflict.conflict_id = NEW.conflict_id
      AND ((NEW.role = 'incoming' AND media.media_entry_id = conflict.incoming_media_entry_id)
        OR (NEW.role = 'candidate' AND EXISTS (
            SELECT 1 FROM canonical_shared_file_uuids AS source
            JOIN canonical_shared_file_uuids AS candidate ON candidate.file_uuid=conflict.candidate_file_uuid
            WHERE source.file_uuid=media.file_uuid AND source.canonical_file_uuid=candidate.canonical_file_uuid
        )))
)
 OR EXISTS (SELECT 1 FROM file_match_decision_conflicts AS settled
            JOIN file_match_decision_publications AS publication USING(decision_id)
            WHERE settled.conflict_id=NEW.conflict_id)
BEGIN SELECT RAISE(ABORT, 'file-match hash witness is not owned by its conflict side'); END;

CREATE TRIGGER file_match_conflict_size_owner_guard
BEFORE INSERT ON file_match_conflict_sizes
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_conflicts AS conflict
    JOIN candidate_native_file_sizes AS size ON size.media_entry_id=NEW.media_entry_id
    JOIN candidate_native_file_byte_coverage AS qualification ON qualification.media_entry_id=size.media_entry_id
    JOIN catalog_media_entries AS media ON media.media_entry_id=size.media_entry_id
    WHERE conflict.conflict_id = NEW.conflict_id
      AND ((NEW.role = 'incoming' AND media.media_entry_id = conflict.incoming_media_entry_id)
        OR (NEW.role = 'candidate' AND EXISTS (
            SELECT 1 FROM canonical_shared_file_uuids AS source
            JOIN canonical_shared_file_uuids AS candidate ON candidate.file_uuid=conflict.candidate_file_uuid
            WHERE source.file_uuid=media.file_uuid AND source.canonical_file_uuid=candidate.canonical_file_uuid
        )))
      AND size.source_size_field=NEW.source_size_field AND size.size_state='value'
)
 OR EXISTS (SELECT 1 FROM file_match_decision_conflicts AS settled
            JOIN file_match_decision_publications AS publication USING(decision_id)
            WHERE settled.conflict_id=NEW.conflict_id)
BEGIN SELECT RAISE(ABORT, 'file-match size witness is not owned by its conflict side'); END;

CREATE TRIGGER file_match_decision_conflict_guard
BEFORE INSERT ON file_match_decision_conflicts
WHEN EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
 OR NOT EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    WHERE decision.decision_id = NEW.decision_id
      AND ((decision.decision = 'merge' AND NEW.outcome = 'merged')
        OR (decision.decision = 'keep_separate' AND NEW.outcome = 'keep_separate'))
 )
BEGIN SELECT RAISE(ABORT, 'file-match outcome disagrees with its decision'); END;

CREATE TRIGGER file_match_hash_decision_guard
BEFORE INSERT ON file_match_hash_decisions
WHEN EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
 OR (NEW.disposition='accept' AND EXISTS (
    SELECT 1 FROM file_match_hash_decisions AS previous
    JOIN file_match_decision_publications AS publication USING (decision_id)
    WHERE previous.reported_hash_id = NEW.reported_hash_id
      AND previous.disposition = 'reject'
 ))
BEGIN SELECT RAISE(ABORT, 'published file-match hash decision cannot be changed or restored'); END;
CREATE TRIGGER file_match_hash_decision_append_only_update
BEFORE UPDATE ON file_match_hash_decisions
BEGIN SELECT RAISE(ABORT, 'file-match hash decisions are append-only'); END;
CREATE TRIGGER file_match_hash_decision_append_only_delete
BEFORE DELETE ON file_match_hash_decisions
BEGIN SELECT RAISE(ABORT, 'file-match hash decisions are append-only'); END;

CREATE TRIGGER file_match_size_decision_guard
BEFORE INSERT ON file_match_size_decisions
WHEN EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
 OR (NEW.disposition='accept' AND EXISTS (
    SELECT 1 FROM file_match_size_decisions AS previous
    JOIN file_match_decision_publications AS publication USING (decision_id)
    WHERE previous.media_entry_id = NEW.media_entry_id
      AND previous.source_size_field = NEW.source_size_field
      AND previous.disposition = 'reject'
 ))
BEGIN SELECT RAISE(ABORT, 'published file-match size decision cannot be changed or restored'); END;
CREATE TRIGGER file_match_size_decision_append_only_update
BEFORE UPDATE ON file_match_size_decisions
BEGIN SELECT RAISE(ABORT, 'file-match size decisions are append-only'); END;
CREATE TRIGGER file_match_size_decision_append_only_delete
BEFORE DELETE ON file_match_size_decisions
BEGIN SELECT RAISE(ABORT, 'file-match size decisions are append-only'); END;

CREATE TRIGGER file_match_redirect_guard
BEFORE INSERT ON file_match_uuid_redirects
WHEN NOT EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    JOIN shared_catalog_files AS old_file ON old_file.file_uuid = NEW.old_file_uuid
    JOIN shared_catalog_files AS kept_file ON kept_file.file_uuid = NEW.kept_file_uuid
    WHERE decision.decision_id = NEW.decision_id AND decision.decision = 'merge'
      AND decision.kept_file_uuid = NEW.kept_file_uuid
      AND old_file.registry_id = kept_file.registry_id
      AND EXISTS (
          SELECT 1 FROM file_match_decision_conflicts AS settled
          JOIN file_match_conflicts AS conflict USING (conflict_id)
          WHERE settled.decision_id = decision.decision_id
            AND settled.outcome = 'merged'
            AND conflict.candidate_file_uuid = NEW.old_file_uuid
      )
)
 OR EXISTS (
    WITH RECURSIVE reachable(file_uuid) AS (
        SELECT NEW.kept_file_uuid
        UNION
        SELECT redirect.kept_file_uuid FROM file_match_uuid_redirects AS redirect
        JOIN file_match_decision_publications AS publication USING (decision_id)
        JOIN reachable ON redirect.old_file_uuid = reachable.file_uuid
    )
    SELECT 1 FROM reachable WHERE file_uuid = NEW.old_file_uuid
 )
BEGIN SELECT RAISE(ABORT, 'UUID redirect lacks a same-registry reviewed conflict or creates a cycle'); END;

CREATE TRIGGER file_match_redirect_append_only_update
BEFORE UPDATE ON file_match_uuid_redirects
BEGIN SELECT RAISE(ABORT, 'issued UUID redirects are append-only'); END;
CREATE TRIGGER file_match_redirect_append_only_delete
BEFORE DELETE ON file_match_uuid_redirects
BEGIN SELECT RAISE(ABORT, 'issued UUID redirects are append-only'); END;

CREATE TRIGGER file_match_publication_guard
BEFORE INSERT ON file_match_decision_publications
WHEN EXISTS (SELECT 1 FROM file_match_decision_publications WHERE decision_id = NEW.decision_id)
 OR NOT EXISTS (SELECT 1 FROM file_match_decisions WHERE decision_id = NEW.decision_id)
 OR NOT EXISTS (SELECT 1 FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id)
 OR EXISTS (
    SELECT 1 FROM file_match_decision_conflicts AS settled
    JOIN file_match_decisions AS decision USING (decision_id)
    WHERE settled.decision_id = NEW.decision_id
      AND ((decision.decision = 'merge' AND settled.outcome <> 'merged')
        OR (decision.decision = 'keep_separate' AND settled.outcome <> 'keep_separate'))
 )
 OR EXISTS (
    SELECT 1
    FROM file_match_decision_conflicts AS settled
    JOIN file_match_conflict_hashes AS witness USING (conflict_id)
    WHERE settled.decision_id = NEW.decision_id
      AND NOT EXISTS (
          SELECT 1 FROM file_match_hash_decisions AS review
          WHERE review.decision_id = settled.decision_id
            AND review.conflict_id = settled.conflict_id
            AND review.reported_hash_id = witness.reported_hash_id
            AND review.role = witness.role
      )
 )
 OR EXISTS (
    SELECT 1
    FROM file_match_decision_conflicts AS settled
    JOIN file_match_conflict_sizes AS witness USING (conflict_id)
    WHERE settled.decision_id = NEW.decision_id
      AND NOT EXISTS (
          SELECT 1 FROM file_match_size_decisions AS review
          WHERE review.decision_id = settled.decision_id
            AND review.conflict_id = settled.conflict_id
            AND review.media_entry_id = witness.media_entry_id
            AND review.source_size_field = witness.source_size_field
            AND review.role = witness.role
      )
 )
 OR EXISTS (
    SELECT 1 FROM file_match_decisions AS decision
    WHERE decision.decision_id = NEW.decision_id AND decision.decision = 'merge'
      AND NOT EXISTS (
          SELECT 1 FROM file_match_decision_conflicts AS settled
          JOIN file_match_conflicts AS conflict USING (conflict_id)
          WHERE settled.decision_id = decision.decision_id
            AND conflict.candidate_file_uuid = decision.kept_file_uuid
      )
 )
 OR EXISTS (
    SELECT 1 FROM file_match_uuid_redirects AS redirect
    WHERE redirect.decision_id = NEW.decision_id
      AND NOT EXISTS (
          SELECT 1 FROM file_match_decision_conflicts AS settled
          JOIN file_match_conflicts AS conflict USING (conflict_id)
          WHERE settled.decision_id = NEW.decision_id
            AND settled.outcome = 'merged'
            AND conflict.candidate_file_uuid = redirect.old_file_uuid
      )
 )
 OR EXISTS (
     SELECT 1 FROM file_match_decisions AS decision
     JOIN file_match_decision_conflicts AS settled USING (decision_id)
     JOIN file_match_conflicts AS conflict USING (conflict_id)
     WHERE decision.decision_id = NEW.decision_id
       AND decision.decision = 'merge'
       AND conflict.candidate_file_uuid <> decision.kept_file_uuid
       AND NOT EXISTS (
           SELECT 1 FROM file_match_uuid_redirects AS redirect
           WHERE redirect.decision_id = NEW.decision_id
             AND redirect.old_file_uuid = conflict.candidate_file_uuid
             AND redirect.kept_file_uuid = decision.kept_file_uuid
       )
 )
 OR EXISTS (
     WITH RECURSIVE walk(issued_file_uuid, current_file_uuid) AS (
         SELECT old_file_uuid,kept_file_uuid FROM file_match_uuid_redirects
         WHERE decision_id=NEW.decision_id
         UNION
         SELECT walk.issued_file_uuid,redirect.kept_file_uuid
         FROM walk JOIN file_match_uuid_redirects AS redirect ON redirect.old_file_uuid=walk.current_file_uuid
         WHERE redirect.decision_id=NEW.decision_id OR EXISTS (
             SELECT 1 FROM file_match_decision_publications WHERE decision_id=redirect.decision_id
         )
     )
     SELECT 1 FROM walk WHERE issued_file_uuid = current_file_uuid
 )
 OR EXISTS (
     SELECT 1 FROM file_match_uuid_redirects AS redirect
     LEFT JOIN canonical_shared_file_uuids AS destination
       ON destination.file_uuid=redirect.kept_file_uuid
     WHERE redirect.decision_id=NEW.decision_id
       AND destination.canonical_file_uuid IS NULL
 )
BEGIN SELECT RAISE(ABORT, 'file-match decision is incomplete or has an unreviewed redirect'); END;

CREATE TRIGGER file_match_publication_append_only_update
BEFORE UPDATE ON file_match_decision_publications
BEGIN SELECT RAISE(ABORT, 'file-match publications are immutable'); END;
CREATE TRIGGER file_match_publication_append_only_delete
BEFORE DELETE ON file_match_decision_publications
BEGIN SELECT RAISE(ABORT, 'file-match publications are immutable'); END;

-- shared_file_facts.sql() emits the same affected-component maintenance for
-- hash and size membership at review and edition publication. Keeping one
-- generator prevents one lane or an old issued alias being forgotten.

-- Typed native literal/position closure is deliberately left to the main
-- candidate harness. Native declarations point to reported_catalog_relationships;
-- canonical position rows must point to canonical_relationship_id and must
-- not copy their literal. The harness must prove one typed owner and one
-- canonical position per reported identity in both directions, including
-- kind, field-key, and edition ancestry. No polymorphic source-owner row is
-- persisted. Relationships.sql provides candidate_relationship_integrity_problems
-- for registry/declaration/evidence/shared-hash checks; it does not substitute
-- for this native position closure.
