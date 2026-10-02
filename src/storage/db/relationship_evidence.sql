CREATE VIEW relationship_unicode_whitespace AS
SELECT char(9,10,11,12,13,32,133,160,5760,8192,8193,8194,8195,8196,8197,
            8198,8199,8200,8201,8202,8232,8233,8239,8287,12288) AS characters;

CREATE TABLE catalog_relationship_rationales (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    reason TEXT NOT NULL CHECK (typeof(reason) = 'text')
) WITHOUT ROWID;
CREATE TABLE catalog_relationship_comparisons (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (status IN ('compatible','candidate','contradictory','ambiguous','unknown'))
) WITHOUT ROWID;
CREATE TABLE catalog_relationship_comparison_fields (
    relationship_id INTEGER NOT NULL
        REFERENCES catalog_relationship_comparisons(relationship_id) ON DELETE RESTRICT,
    disposition TEXT NOT NULL CHECK (disposition IN ('agreement','contradiction')),
    list_order INTEGER NOT NULL CHECK (typeof(list_order)='integer' AND list_order >= 0),
    field TEXT NOT NULL CHECK (field IN ('sha1','md5','crc','size')),
    PRIMARY KEY (relationship_id, disposition, list_order),
    UNIQUE (relationship_id, field)
) WITHOUT ROWID;
CREATE TABLE catalog_relationship_evidence_publications (
    relationship_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    evidence_kind TEXT NOT NULL CHECK (evidence_kind IN ('rationale','catalog_comparison'))
) WITHOUT ROWID;
CREATE TABLE catalog_relationship_evidence (
    relationship_id INTEGER NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    list_order INTEGER NOT NULL CHECK (typeof(list_order)='integer' AND list_order >= 0),
    supporting_relationship_id INTEGER NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    PRIMARY KEY (relationship_id, list_order)
) WITHOUT ROWID;

CREATE TRIGGER catalog_relationship_rationales_insert_guard
BEFORE INSERT ON catalog_relationship_rationales
WHEN length(CAST(trim(NEW.reason,(SELECT characters FROM relationship_unicode_whitespace)) AS BLOB))=0
 OR NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    WHERE identity.relationship_id=NEW.relationship_id AND identity.origin IN ('derived','user')
      AND ((identity.origin='derived' AND EXISTS (SELECT 1 FROM inferred_catalog_relationships WHERE relationship_id=identity.relationship_id))
        OR (identity.origin='user' AND EXISTS (SELECT 1 FROM manual_catalog_relationships WHERE relationship_id=identity.relationship_id)))
 )
 OR EXISTS (SELECT 1 FROM catalog_relationship_comparisons WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_rationales WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_evidence_publications WHERE relationship_id=NEW.relationship_id)
BEGIN SELECT RAISE(ABORT,'rationale requires nonblank text and one unsealed actual non-source owner'); END;
CREATE TRIGGER catalog_relationship_comparisons_insert_guard
BEFORE INSERT ON catalog_relationship_comparisons
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    JOIN inferred_catalog_relationships AS inferred USING(relationship_id)
    WHERE identity.relationship_id=NEW.relationship_id AND identity.origin='derived'
 )
 OR EXISTS (SELECT 1 FROM catalog_relationship_rationales WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_comparisons WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_evidence_publications WHERE relationship_id=NEW.relationship_id)
BEGIN SELECT RAISE(ABORT,'comparison requires one unsealed actual derived owner'); END;
CREATE TRIGGER catalog_relationship_comparison_fields_insert_guard
BEFORE INSERT ON catalog_relationship_comparison_fields
WHEN NOT EXISTS (SELECT 1 FROM catalog_relationship_comparisons WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_evidence_publications WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_comparison_fields
            WHERE relationship_id=NEW.relationship_id AND (field=NEW.field OR
                  (disposition=NEW.disposition AND list_order=NEW.list_order)))
 OR NEW.list_order<>(SELECT COUNT(*) FROM catalog_relationship_comparison_fields
                     WHERE relationship_id=NEW.relationship_id AND disposition=NEW.disposition)
BEGIN SELECT RAISE(ABORT,'comparison field requires an unsealed owner and dense unique order'); END;
CREATE TRIGGER catalog_relationship_evidence_insert_guard
BEFORE INSERT ON catalog_relationship_evidence
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_relationships AS identity
    JOIN inferred_catalog_relationships AS inferred USING(relationship_id)
    WHERE identity.relationship_id=NEW.relationship_id AND identity.origin='derived'
)
 OR EXISTS (SELECT 1 FROM catalog_relationship_evidence_publications WHERE relationship_id=NEW.relationship_id)
 OR EXISTS (SELECT 1 FROM catalog_relationship_evidence WHERE relationship_id=NEW.relationship_id AND list_order=NEW.list_order)
 OR NEW.list_order<>(SELECT COUNT(*) FROM catalog_relationship_evidence WHERE relationship_id=NEW.relationship_id)
BEGIN SELECT RAISE(ABORT,'support requires an unsealed derived owner and dense order'); END;

CREATE TRIGGER catalog_relationship_rationales_immutable_update BEFORE UPDATE ON catalog_relationship_rationales
BEGIN SELECT RAISE(ABORT,'relationship rationale is immutable'); END;
CREATE TRIGGER catalog_relationship_rationales_immutable_delete BEFORE DELETE ON catalog_relationship_rationales
BEGIN SELECT RAISE(ABORT,'relationship rationale is immutable'); END;
CREATE TRIGGER catalog_relationship_comparisons_immutable_update BEFORE UPDATE ON catalog_relationship_comparisons
BEGIN SELECT RAISE(ABORT,'relationship comparison is immutable'); END;
CREATE TRIGGER catalog_relationship_comparisons_immutable_delete BEFORE DELETE ON catalog_relationship_comparisons
BEGIN SELECT RAISE(ABORT,'relationship comparison is immutable'); END;
CREATE TRIGGER catalog_relationship_comparison_fields_immutable_update BEFORE UPDATE ON catalog_relationship_comparison_fields
BEGIN SELECT RAISE(ABORT,'relationship comparison field is immutable'); END;
CREATE TRIGGER catalog_relationship_comparison_fields_immutable_delete BEFORE DELETE ON catalog_relationship_comparison_fields
BEGIN SELECT RAISE(ABORT,'relationship comparison field is immutable'); END;
CREATE TRIGGER catalog_relationship_evidence_publications_immutable_update BEFORE UPDATE ON catalog_relationship_evidence_publications
BEGIN SELECT RAISE(ABORT,'relationship evidence publication is immutable'); END;
CREATE TRIGGER catalog_relationship_evidence_publications_immutable_delete BEFORE DELETE ON catalog_relationship_evidence_publications
BEGIN SELECT RAISE(ABORT,'relationship evidence publication is immutable'); END;
CREATE TRIGGER catalog_relationship_evidence_immutable_update BEFORE UPDATE ON catalog_relationship_evidence
BEGIN SELECT RAISE(ABORT,'relationship evidence is immutable'); END;
CREATE TRIGGER catalog_relationship_evidence_immutable_delete BEFORE DELETE ON catalog_relationship_evidence
BEGIN SELECT RAISE(ABORT,'relationship evidence is immutable'); END;
