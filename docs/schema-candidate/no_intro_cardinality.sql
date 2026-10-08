-- Additional No-Intro child/field cardinality checks not already covered by
-- candidate_no_intro_integrity_problems. Main's assembler discovers this view
-- by its _cardinality_problems suffix and adds it to candidate publication.
-- Parent edition is resolved through the actual game/set-group ancestry.
CREATE VIEW candidate_no_intro_cardinality_problems AS
WITH required(field_kind) AS (VALUES ('size'),('crc'),('md5'),('sha1')),
first_games AS (
    SELECT set_row.set_group_id, MIN(set_row.source_order) AS first_game_order
    FROM catalog_sets AS set_row
    JOIN no_intro_dat_games AS game ON game.set_id=set_row.set_id
    GROUP BY set_row.set_group_id
),
strict_header_events AS (
    SELECT header.header_id,
           field.source_order,
           CASE field.field_kind
               WHEN 'id' THEN 0
               WHEN 'name' THEN 1
               WHEN 'description' THEN 2
               WHEN 'version' THEN 3
               WHEN 'date' THEN 4
               WHEN 'author' THEN 5
               WHEN 'homepage' THEN 6
               WHEN 'url' THEN 7
               WHEN 'trademarks' THEN CASE rules.dialect
                   WHEN 'no-intro-dat-v4-strict' THEN 8 END
               WHEN 'piracy' THEN CASE rules.dialect
                   WHEN 'no-intro-dat-v4-strict' THEN 9 END
               WHEN 'subset' THEN CASE rules.dialect
                   WHEN 'no-intro-dat-v3-strict' THEN 8 ELSE 10 END
           END AS child_rank
    FROM no_intro_dat_headers AS header
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    JOIN no_intro_dat_header_text_children AS field USING(header_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
    UNION ALL
    SELECT header.header_id,
           option_row.source_order,
           CASE rules.dialect
               WHEN 'no-intro-dat-v3-strict' THEN 9
               ELSE 11
           END AS child_rank
    FROM no_intro_dat_headers AS header
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    JOIN no_intro_dat_clrmamepro_options AS option_row USING(header_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
    UNION ALL
    SELECT header.header_id,
           option_row.source_order,
           CASE rules.dialect
               WHEN 'no-intro-dat-v3-strict' THEN 10
               ELSE 12
           END AS child_rank
    FROM no_intro_dat_headers AS header
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    JOIN no_intro_dat_romcenter_options AS option_row USING(header_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
),
strict_header_order AS (
    SELECT header_id, source_order, child_rank,
           MAX(child_rank) OVER (
               PARTITION BY header_id ORDER BY source_order
               ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
           ) AS previous_max_rank
    FROM strict_header_events
    WHERE child_rank IS NOT NULL
),
strict_game_events AS (
    SELECT game.set_id, description.source_order, 1 AS child_rank
    FROM no_intro_dat_games AS game
    JOIN no_intro_dat_game_descriptions AS description USING(set_id)
    JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
    UNION ALL
    SELECT game.set_id, rom.source_order, 2 AS child_rank
    FROM no_intro_dat_games AS game
    JOIN no_intro_dat_rom_claims AS rom USING(set_id)
    JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
    UNION ALL
    SELECT game.set_id, category.source_order, 0 AS child_rank
    FROM no_intro_dat_games AS game
    JOIN no_intro_dat_categories AS category USING(set_id)
    JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
    UNION ALL
    SELECT game.set_id, release.source_order, 3 AS child_rank
    FROM no_intro_dat_games AS game
    JOIN no_intro_dat_releases AS release USING(set_id)
    JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
    JOIN catalog_set_groups AS group_row USING(set_group_id)
    JOIN catalog_editions AS edition USING(edition_id)
    JOIN catalog_reading_rules AS rules USING(reading_rules_id)
    WHERE rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
),
strict_game_order AS (
    SELECT set_id, source_order, child_rank,
           MAX(child_rank) OVER (
               PARTITION BY set_id ORDER BY source_order
               ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
           ) AS previous_max_rank
    FROM strict_game_events
)
SELECT 'dat_strict_rom_field_missing:' || required.field_kind AS problem,
       rom.media_entry_id AS owner_id,
       group_row.edition_id AS edition_id
FROM no_intro_dat_rom_claims AS rom
JOIN no_intro_dat_games AS game ON game.set_id=rom.set_id
JOIN catalog_sets AS set_row ON set_row.set_id=game.set_id
JOIN catalog_set_groups AS group_row ON group_row.set_group_id=set_row.set_group_id
JOIN no_intro_dat_documents AS document ON document.edition_id=group_row.edition_id
JOIN catalog_editions AS edition ON edition.edition_id=document.edition_id
JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
CROSS JOIN required
WHERE rules.format_family='no_intro_dat'
  AND rules.dialect IN ('no-intro-dat-v3-strict','no-intro-dat-v4-strict')
  AND NOT EXISTS (
      SELECT 1
      FROM no_intro_dat_rom_field_positions AS position
      WHERE position.media_entry_id=rom.media_entry_id
        AND position.field_kind=required.field_kind
  )
UNION ALL
SELECT 'dat_header_after_game', header.header_id, group_row.edition_id
FROM no_intro_dat_headers AS header
JOIN catalog_set_groups AS group_row USING(set_group_id)
JOIN first_games USING(set_group_id)
WHERE header.source_order >= first_games.first_game_order
UNION ALL
SELECT 'dat_strict_header_child_order', order_row.header_id, group_row.edition_id
FROM strict_header_order AS order_row
JOIN no_intro_dat_headers AS header USING(header_id)
JOIN catalog_set_groups AS group_row USING(set_group_id)
WHERE order_row.previous_max_rank > order_row.child_rank
UNION ALL
SELECT 'dat_strict_game_child_order', order_row.set_id, group_row.edition_id
FROM strict_game_order AS order_row
JOIN catalog_sets AS set_row ON set_row.set_id=order_row.set_id
JOIN catalog_set_groups AS group_row USING(set_group_id)
WHERE order_row.previous_max_rank > order_row.child_rank;
