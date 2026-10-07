presence AS (
 SELECT family,owner_a,owner_b,owner_c,owner_d,field_kind,0 AS origin
 FROM mame_expected_attribute_positions
 WHERE snapshot_key=(SELECT snapshot_key FROM requested)
 UNION ALL
 SELECT family,owner_a,owner_b,owner_c,owner_d,field_kind,1 AS origin
 FROM mame_actual_attribute_positions
 WHERE snapshot_key=(SELECT snapshot_key FROM requested)
)
SELECT 1 AS invalid
WHERE EXISTS(
 SELECT 1 FROM presence
 GROUP BY family,owner_a,owner_b,owner_c,owner_d,field_kind
 HAVING min(origin)=max(origin)
)
   OR EXISTS(SELECT 1 FROM mame_attribute_ordinal_collisions
             WHERE snapshot_key=(SELECT snapshot_key FROM requested))
