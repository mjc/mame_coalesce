CREATE TABLE catalog_coverage (
    coverage_id INTEGER PRIMARY KEY NOT NULL CHECK (coverage_id > 0),
    kind TEXT NOT NULL CHECK (kind IN ('unknown', 'complete', 'filtered', 'partial'))
);

CREATE TABLE catalog_covered_sets (
    coverage_id INTEGER NOT NULL
        REFERENCES catalog_coverage (coverage_id) ON DELETE RESTRICT,
    list_order INTEGER NOT NULL CHECK (list_order >= 0),
    set_kind TEXT NOT NULL CHECK (set_kind IN ('root', 'software_item')),
    set_group_kind TEXT NOT NULL CHECK (set_group_kind IN ('root', 'software_list')),
    software_list_name TEXT,
    set_name TEXT NOT NULL,
    coverage TEXT NOT NULL CHECK (coverage IN ('covered', 'unknown')),
    PRIMARY KEY (coverage_id, list_order),
    CHECK (
        (set_kind = 'root' AND set_group_kind = 'root' AND software_list_name IS NULL) OR
        (set_kind = 'software_item' AND set_group_kind = 'software_list' AND software_list_name IS NOT NULL)
    )
);

CREATE UNIQUE INDEX catalog_covered_sets_root_unique
    ON catalog_covered_sets (coverage_id, set_kind, set_name)
    WHERE set_kind = 'root';

CREATE UNIQUE INDEX catalog_covered_sets_software_item_unique
    ON catalog_covered_sets (coverage_id, set_kind, software_list_name, set_name)
    WHERE set_kind = 'software_item';

CREATE TRIGGER catalog_coverage_validate_kind_update
BEFORE UPDATE OF kind ON catalog_coverage
WHEN (
    NEW.kind IN ('unknown', 'complete') AND EXISTS (
        SELECT 1 FROM catalog_covered_sets
        WHERE coverage_id = OLD.coverage_id
    )
) OR (
    NEW.kind = 'filtered' AND EXISTS (
        SELECT 1 FROM catalog_covered_sets
        WHERE coverage_id = OLD.coverage_id AND coverage = 'unknown'
    )
)
BEGIN
    SELECT RAISE(ABORT, 'catalog coverage kind is incompatible with its members');
END;

CREATE TRIGGER catalog_coverage_immutable_update
BEFORE UPDATE ON catalog_coverage
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots
    WHERE coverage_id = OLD.coverage_id
)
BEGIN
    SELECT RAISE(ABORT, 'referenced catalog coverage is immutable');
END;

CREATE TRIGGER catalog_coverage_immutable_delete
BEFORE DELETE ON catalog_coverage
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots
    WHERE coverage_id = OLD.coverage_id
)
BEGIN
    SELECT RAISE(ABORT, 'referenced catalog coverage is immutable');
END;

CREATE TRIGGER catalog_covered_sets_validate_insert
BEFORE INSERT ON catalog_covered_sets
WHEN EXISTS (
    SELECT 1 FROM catalog_coverage
    WHERE coverage_id = NEW.coverage_id AND (
        kind IN ('unknown', 'complete')
        OR (kind = 'filtered' AND NEW.coverage = 'unknown')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'catalog coverage kind is incompatible with its member');
END;

CREATE TRIGGER catalog_covered_sets_validate_update
BEFORE UPDATE ON catalog_covered_sets
WHEN EXISTS (
    SELECT 1 FROM catalog_coverage
    WHERE coverage_id = NEW.coverage_id AND (
        kind IN ('unknown', 'complete')
        OR (kind = 'filtered' AND NEW.coverage = 'unknown')
    )
)
BEGIN
    SELECT RAISE(ABORT, 'catalog coverage kind is incompatible with its member');
END;

CREATE TRIGGER catalog_covered_sets_immutable_insert
BEFORE INSERT ON catalog_covered_sets
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots
    WHERE coverage_id = NEW.coverage_id
)
BEGIN
    SELECT RAISE(ABORT, 'members of referenced catalog coverage are immutable');
END;

CREATE TRIGGER catalog_covered_sets_immutable_update
BEFORE UPDATE ON catalog_covered_sets
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots
    WHERE coverage_id IN (OLD.coverage_id, NEW.coverage_id)
)
BEGIN
    SELECT RAISE(ABORT, 'members of referenced catalog coverage are immutable');
END;

CREATE TRIGGER catalog_covered_sets_immutable_delete
BEFORE DELETE ON catalog_covered_sets
WHEN EXISTS (
    SELECT 1 FROM catalog_snapshots
    WHERE coverage_id = OLD.coverage_id
)
BEGIN
    SELECT RAISE(ABORT, 'members of referenced catalog coverage are immutable');
END;
