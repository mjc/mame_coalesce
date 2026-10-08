-- One review's components, including source owners and historical aliases.
-- NEW is the publication seal. Filter seeds before recursive traversal.
WITH RECURSIVE affected(content_uuid) AS (
    SELECT candidate_content_uuid FROM file_match_decision_conflicts WHERE decision_id = NEW.decision_id
    UNION
    SELECT owner.content_uuid FROM file_match_hash_decisions AS review
    JOIN asset_occurrences AS owner ON owner.occurrence_id = review.evidence_occurrence_id
    WHERE review.decision_id = NEW.decision_id AND owner.content_uuid IS NOT NULL
    UNION
    SELECT owner.content_uuid FROM file_match_size_decisions AS review
    JOIN asset_occurrences AS owner ON owner.occurrence_id = review.evidence_occurrence_id
    WHERE review.decision_id = NEW.decision_id AND owner.content_uuid IS NOT NULL
    UNION
    SELECT kept_content_uuid FROM file_match_decisions
    WHERE decision_id = NEW.decision_id AND kept_content_uuid IS NOT NULL
    UNION
    SELECT redirect.old_content_uuid FROM affected
    JOIN merged_file_ids AS redirect ON redirect.kept_content_uuid = affected.content_uuid
    JOIN file_match_decision_publications USING (decision_id)
    UNION
    SELECT redirect.kept_content_uuid FROM affected
    JOIN merged_file_ids AS redirect ON redirect.old_content_uuid = affected.content_uuid
    JOIN file_match_decision_publications USING (decision_id)
)
SELECT content_uuid FROM affected
