ALTER TABLE snapshot_extensions
    ADD COLUMN owner_set_name TEXT;

ALTER TABLE snapshot_extensions
    ADD COLUMN owner_component_order TEXT;

CREATE INDEX snapshot_extensions_asset_owner_index
    ON snapshot_extensions (snapshot_key, owner_set_name, owner_component_order);

DROP TRIGGER snapshot_extensions_are_immutable_update;

UPDATE snapshot_extensions
SET owner_set_name = (
        SELECT requirement.set_name
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.source_line = snapshot_extensions.source_line
          AND requirement.source_column = snapshot_extensions.source_column
        LIMIT 1
    ),
    owner_component_order = (
        SELECT CAST(requirement.component_order AS TEXT)
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.source_line = snapshot_extensions.source_line
          AND requirement.source_column = snapshot_extensions.source_column
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND EXISTS (
      SELECT 1
      FROM asset_requirements AS requirement
      WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
        AND requirement.source_line = snapshot_extensions.source_line
        AND requirement.source_column = snapshot_extensions.source_column
  );

CREATE TRIGGER snapshot_extensions_are_immutable_update
BEFORE UPDATE ON snapshot_extensions
BEGIN
    SELECT RAISE(ABORT, 'snapshot extensions are immutable');
END;
