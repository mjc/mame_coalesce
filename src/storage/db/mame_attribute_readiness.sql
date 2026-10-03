expected AS MATERIALIZED (
 SELECT snapshot_key,family,owner_a,owner_b,owner_c,owner_d,field_kind
 FROM mame_expected_attribute_positions
 WHERE snapshot_key=(SELECT snapshot_key FROM requested)
), actual AS MATERIALIZED (
 SELECT snapshot_key,family,owner_a,owner_b,owner_c,owner_d,field_kind
 FROM mame_actual_attribute_positions
 WHERE snapshot_key=(SELECT snapshot_key FROM requested)
)
SELECT 1 AS invalid
WHERE EXISTS(SELECT * FROM expected EXCEPT SELECT * FROM actual)
   OR EXISTS(SELECT * FROM actual EXCEPT SELECT * FROM expected)
   OR EXISTS(SELECT 1 FROM mame_attribute_ordinal_collisions
             WHERE snapshot_key=(SELECT snapshot_key FROM requested))
