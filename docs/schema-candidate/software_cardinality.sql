-- Native software child/PCDATA cardinalities not expressible by row keys.
-- Parser-compatible cardinalities are intentional: plural wrappers may be
-- empty; lists need a title; title description/year/publisher are required
-- but may be empty; notes are optional singletons; parts, areas, switches,
-- and their recognized children may be empty. The importer retains other
-- children as extensions, so this view does not reject them as DTD violations.
CREATE VIEW candidate_software_cardinality_problems AS
SELECT 'software_list_without_title' AS problem,
       list.set_group_id AS owner_id,
       groups.edition_id AS edition_id
FROM software_lists AS list
JOIN catalog_set_groups AS groups USING (set_group_id)
WHERE NOT EXISTS (
    SELECT 1
    FROM catalog_sets AS set_entry
    JOIN software_titles AS title ON title.set_id = set_entry.set_id
    WHERE set_entry.set_group_id = list.set_group_id
)
UNION ALL
SELECT 'software_title_required_text_missing',
       title.set_id,
       groups.edition_id
FROM software_titles AS title
JOIN catalog_sets AS set_entry ON set_entry.set_id = title.set_id
JOIN catalog_set_groups AS groups USING (set_group_id)
WHERE (SELECT count(*) FROM software_title_text_elements AS text
       WHERE text.set_id = title.set_id AND text.field_kind = 0) <> 1
   OR (SELECT count(*) FROM software_title_text_elements AS text
       WHERE text.set_id = title.set_id AND text.field_kind = 1) <> 1
   OR (SELECT count(*) FROM software_title_text_elements AS text
       WHERE text.set_id = title.set_id AND text.field_kind = 2) <> 1
UNION ALL
SELECT 'software_area_subtype_closure',
       area.area_id,
       coalesce(parent_element.edition_id, area_element.edition_id)
FROM software_areas AS area
LEFT JOIN catalog_source_elements AS parent_element
  ON parent_element.source_element_id = area.part_id
LEFT JOIN catalog_source_elements AS area_element
  ON area_element.source_element_id = area.area_id
WHERE (area.kind = 'data'
       AND ((SELECT count(*) FROM software_data_areas AS data
             WHERE data.area_id = area.area_id) <> 1
         OR (SELECT count(*) FROM software_disk_areas AS disk
             WHERE disk.area_id = area.area_id) <> 0))
   OR (area.kind = 'disk'
       AND ((SELECT count(*) FROM software_disk_areas AS disk
             WHERE disk.area_id = area.area_id) <> 1
         OR (SELECT count(*) FROM software_data_areas AS data
             WHERE data.area_id = area.area_id) <> 0));
