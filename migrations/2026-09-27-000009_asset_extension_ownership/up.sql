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
  AND extension_id = (
      SELECT MIN(candidate.extension_id)
      FROM snapshot_extensions AS candidate
      WHERE candidate.snapshot_key = snapshot_extensions.snapshot_key
        AND candidate.record_kind = snapshot_extensions.record_kind
        AND candidate.record_name IS snapshot_extensions.record_name
        AND candidate.field_name = snapshot_extensions.field_name
        AND candidate.namespace_uri IS snapshot_extensions.namespace_uri
        AND candidate.raw_value_json = snapshot_extensions.raw_value_json
        AND candidate.source_line = snapshot_extensions.source_line
        AND candidate.source_column = snapshot_extensions.source_column
  )
  AND EXISTS (
      SELECT 1
      FROM asset_requirements AS requirement
      WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
        AND requirement.source_line = snapshot_extensions.source_line
        AND requirement.source_column = snapshot_extensions.source_column
  );

UPDATE snapshot_extensions
SET owner_set_name = record_name,
    owner_component_order = (
        SELECT CAST(requirement.component_order AS TEXT)
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.set_name = snapshot_extensions.record_name
          AND (requirement.source_line, requirement.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY requirement.source_line DESC, requirement.source_column DESC
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND extension_id = (
      SELECT MIN(candidate.extension_id)
      FROM snapshot_extensions AS candidate
      WHERE candidate.snapshot_key = snapshot_extensions.snapshot_key
        AND candidate.record_kind = snapshot_extensions.record_kind
        AND candidate.record_name IS snapshot_extensions.record_name
        AND candidate.field_name = snapshot_extensions.field_name
        AND candidate.namespace_uri IS snapshot_extensions.namespace_uri
        AND candidate.raw_value_json = snapshot_extensions.raw_value_json
        AND candidate.source_line = snapshot_extensions.source_line
        AND candidate.source_column = snapshot_extensions.source_column
  )
  AND record_kind = 'rom'
  AND (
      SELECT interpretation.format
      FROM catalog_snapshots AS snapshot
      JOIN parser_interpretations AS interpretation
        ON interpretation.interpretation_key = snapshot.interpretation_key
      WHERE snapshot.snapshot_key = snapshot_extensions.snapshot_key
  ) = 'no-intro-pc-xml'
  AND EXISTS (
      SELECT 1
      FROM asset_requirements AS requirement
      WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
        AND requirement.set_name = snapshot_extensions.record_name
        AND (requirement.source_line, requirement.source_column)
            <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
  );

UPDATE snapshot_extensions
SET owner_set_name = (
        SELECT requirement.set_name
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.asset_name = snapshot_extensions.record_name
          AND (requirement.source_line, requirement.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY requirement.source_line DESC, requirement.source_column DESC
        LIMIT 1
    ),
    owner_component_order = (
        SELECT CAST(requirement.component_order AS TEXT)
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.asset_name = snapshot_extensions.record_name
          AND (requirement.source_line, requirement.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY requirement.source_line DESC, requirement.source_column DESC
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND extension_id = (
      SELECT MIN(candidate.extension_id)
      FROM snapshot_extensions AS candidate
      WHERE candidate.snapshot_key = snapshot_extensions.snapshot_key
        AND candidate.record_kind = snapshot_extensions.record_kind
        AND candidate.record_name IS snapshot_extensions.record_name
        AND candidate.field_name = snapshot_extensions.field_name
        AND candidate.namespace_uri IS snapshot_extensions.namespace_uri
        AND candidate.raw_value_json = snapshot_extensions.raw_value_json
        AND candidate.source_line = snapshot_extensions.source_line
        AND candidate.source_column = snapshot_extensions.source_column
  )
  AND record_kind = 'rom'
  AND (
      SELECT interpretation.format
      FROM catalog_snapshots AS snapshot
      JOIN parser_interpretations AS interpretation
        ON interpretation.interpretation_key = snapshot.interpretation_key
      WHERE snapshot.snapshot_key = snapshot_extensions.snapshot_key
  ) = 'no-intro-pc-xml'
  AND EXISTS (
      SELECT 1
      FROM asset_requirements AS requirement
      WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
        AND requirement.asset_name = snapshot_extensions.record_name
        AND (requirement.source_line, requirement.source_column)
            <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
  );

UPDATE snapshot_extensions
SET owner_set_name = (
        SELECT requirement.set_name
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.asset_name = snapshot_extensions.record_name
          AND (requirement.source_line, requirement.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY requirement.source_line DESC, requirement.source_column DESC
        LIMIT 1
    ),
    owner_component_order = (
        SELECT CAST(requirement.component_order AS TEXT)
        FROM asset_requirements AS requirement
        WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
          AND requirement.asset_name = snapshot_extensions.record_name
          AND (requirement.source_line, requirement.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY requirement.source_line DESC, requirement.source_column DESC
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND extension_id = (
      SELECT MIN(candidate.extension_id)
      FROM snapshot_extensions AS candidate
      WHERE candidate.snapshot_key = snapshot_extensions.snapshot_key
        AND candidate.record_kind = snapshot_extensions.record_kind
        AND candidate.record_name IS snapshot_extensions.record_name
        AND candidate.field_name = snapshot_extensions.field_name
        AND candidate.namespace_uri IS snapshot_extensions.namespace_uri
        AND candidate.raw_value_json = snapshot_extensions.raw_value_json
        AND candidate.source_line = snapshot_extensions.source_line
        AND candidate.source_column = snapshot_extensions.source_column
  )
  AND record_kind IN ('rom', 'disk')
  AND (
      SELECT interpretation.format
      FROM catalog_snapshots AS snapshot
      JOIN parser_interpretations AS interpretation
        ON interpretation.interpretation_key = snapshot.interpretation_key
      WHERE snapshot.snapshot_key = snapshot_extensions.snapshot_key
  ) = 'mame-listxml'
  AND (
      SELECT COUNT(*)
      FROM asset_requirements AS requirement
      WHERE requirement.snapshot_key = snapshot_extensions.snapshot_key
        AND requirement.asset_name = snapshot_extensions.record_name
  ) = 1;

UPDATE snapshot_extensions
SET owner_set_name = (
        SELECT sets.set_name
        FROM snapshot_sets AS sets
        WHERE sets.snapshot_key = snapshot_extensions.snapshot_key
          AND (sets.source_line, sets.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        ORDER BY sets.source_line DESC, sets.source_column DESC
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND record_kind = 'device_ref'
  AND (
      SELECT interpretation.format
      FROM catalog_snapshots AS snapshot
      JOIN parser_interpretations AS interpretation
        ON interpretation.interpretation_key = snapshot.interpretation_key
      WHERE snapshot.snapshot_key = snapshot_extensions.snapshot_key
  ) = 'mame-listxml'
  AND EXISTS (
      SELECT 1
      FROM snapshot_sets AS sets
      WHERE sets.snapshot_key = snapshot_extensions.snapshot_key
        AND (sets.source_line, sets.source_column)
            <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
  );

UPDATE snapshot_extensions
SET owner_set_name = (
        SELECT sets.set_name
        FROM snapshot_sets AS sets
        WHERE sets.snapshot_key = snapshot_extensions.snapshot_key
          AND (sets.source_line, sets.source_column)
              <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
          AND EXISTS (
              SELECT 1
              FROM json_each(sets.metadata_json, '$.device_refs') AS device_ref
              WHERE device_ref.type = 'text'
                AND device_ref.value = snapshot_extensions.record_name
          )
        ORDER BY sets.source_line DESC, sets.source_column DESC
        LIMIT 1
    )
WHERE owner_set_name IS NULL
  AND record_kind = 'document'
  AND record_name IS NOT NULL
  AND (
      SELECT interpretation.format
      FROM catalog_snapshots AS snapshot
      JOIN parser_interpretations AS interpretation
        ON interpretation.interpretation_key = snapshot.interpretation_key
      WHERE snapshot.snapshot_key = snapshot_extensions.snapshot_key
  ) = 'logiqx'
  AND EXISTS (
      SELECT 1
      FROM snapshot_sets AS sets
      JOIN json_each(sets.metadata_json, '$.device_refs') AS device_ref
      WHERE sets.snapshot_key = snapshot_extensions.snapshot_key
        AND (sets.source_line, sets.source_column)
            <= (snapshot_extensions.source_line, snapshot_extensions.source_column)
        AND device_ref.type = 'text'
        AND device_ref.value = snapshot_extensions.record_name
  );

CREATE TRIGGER snapshot_extensions_are_immutable_update
BEFORE UPDATE ON snapshot_extensions
BEGIN
    SELECT RAISE(ABORT, 'snapshot extensions are immutable');
END;
