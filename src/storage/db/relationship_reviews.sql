CREATE TABLE catalog_relationship_reviews (
    review_id INTEGER PRIMARY KEY AUTOINCREMENT,
    review_key TEXT NOT NULL UNIQUE CHECK (typeof(review_key)='text'),
    relationship_id INTEGER NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT,
    decision TEXT NOT NULL CHECK (decision IN ('accepted','rejected','withdrawn','superseded')),
    note TEXT NOT NULL CHECK (typeof(note)='text'),
    reviewed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP CHECK (typeof(reviewed_at)='text')
);
CREATE INDEX catalog_relationship_reviews_owner_latest
    ON catalog_relationship_reviews(relationship_id, review_id);
CREATE TABLE replaced_catalog_relationships (
    review_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationship_reviews(review_id) ON DELETE RESTRICT,
    replacement_relationship_id INTEGER NOT NULL
        REFERENCES catalog_relationships(relationship_id) ON DELETE RESTRICT
) WITHOUT ROWID;
CREATE TABLE catalog_relationship_review_publications (
    review_id INTEGER PRIMARY KEY NOT NULL
        REFERENCES catalog_relationship_reviews(review_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TRIGGER catalog_relationship_reviews_immutable_update BEFORE UPDATE ON catalog_relationship_reviews
BEGIN SELECT RAISE(ABORT,'relationship reviews are append-only'); END;
CREATE TRIGGER catalog_relationship_reviews_immutable_delete BEFORE DELETE ON catalog_relationship_reviews
BEGIN SELECT RAISE(ABORT,'relationship reviews are append-only'); END;
CREATE TRIGGER replaced_catalog_relationships_immutable_update BEFORE UPDATE ON replaced_catalog_relationships
BEGIN SELECT RAISE(ABORT,'relationship review replacements are immutable'); END;
CREATE TRIGGER replaced_catalog_relationships_immutable_delete BEFORE DELETE ON replaced_catalog_relationships
BEGIN SELECT RAISE(ABORT,'relationship review replacements are immutable'); END;
CREATE TRIGGER catalog_relationship_review_publications_immutable_update BEFORE UPDATE ON catalog_relationship_review_publications
BEGIN SELECT RAISE(ABORT,'relationship review publication is immutable'); END;
CREATE TRIGGER catalog_relationship_review_publications_immutable_delete BEFORE DELETE ON catalog_relationship_review_publications
BEGIN SELECT RAISE(ABORT,'relationship review publication is immutable'); END;
