-- Native child cardinality not already represented by the family integrity
-- view or typed uniqueness constraints. The parent assembler includes this
-- report in the publication gate after loading native fragments.
CREATE VIEW candidate_logiqx_cmp_cardinality_problems AS
SELECT 'cmp_requires_at_least_one_set' AS problem,
       document.edition_id AS owner_id,
       document.edition_id AS edition_id
FROM clrmamepro_documents AS document
WHERE NOT EXISTS (
    SELECT 1
    FROM catalog_sets AS placement
    JOIN catalog_set_groups AS root USING (set_group_id)
    JOIN clrmamepro_sets AS native USING (set_id)
    WHERE root.set_group_id = document.set_group_id
      AND root.edition_id = document.edition_id
);
