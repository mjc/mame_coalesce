-- Native MAME child/PCDATA cardinality and strict content-sequence audit.
-- This fragment is included explicitly by the schema-candidate assembler.
-- It adds diagnostics only; publication integration belongs to the assembler.
-- Empty #PCDATA is valid for description/year/manufacturer/ramoption text.

CREATE VIEW candidate_mame_cardinality_problems AS
WITH strict_machines AS (
    SELECT machine.set_id AS owner_id
    FROM mame_machines AS machine
    JOIN catalog_source_elements AS element
      ON element.source_element_id=machine.set_id
    JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id
    JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
    WHERE rules.format_family='mame' AND rules.dialect='strict-dtd'
),
machine_children AS (
    SELECT text.machine_id AS owner_id,text.source_order,
           CASE text.field_kind
             WHEN 'description' THEN 0
             WHEN 'year' THEN 1
             WHEN 'manufacturer' THEN 2
           END AS content_rank
    FROM mame_machine_text_elements AS text
    JOIN strict_machines AS strict ON strict.owner_id=text.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,3 FROM mame_bios_sets AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,4 FROM mame_roms AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,5 FROM mame_disks AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,6 FROM mame_device_references AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,7 FROM mame_samples AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,8 FROM mame_chips AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,9 FROM mame_displays AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,10 FROM mame_sound AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,11 FROM mame_inputs AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,12 FROM mame_switches AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id WHERE child.kind='dipswitch'
    UNION ALL SELECT child.machine_id,child.source_order,13 FROM mame_switches AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id WHERE child.kind='configuration'
    UNION ALL SELECT child.machine_id,child.source_order,14 FROM mame_ports AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,15 FROM mame_adjusters AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,16 FROM mame_drivers AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,17 FROM mame_features AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,18 FROM mame_devices AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,19 FROM mame_slots AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,20 FROM mame_softwarelist_references AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
    UNION ALL SELECT child.machine_id,child.source_order,21 FROM mame_ram_options AS child JOIN strict_machines AS strict ON strict.owner_id=child.machine_id
),
strict_machine_sequence AS (
    SELECT owner_id
    FROM (
        SELECT owner_id,content_rank,
               max(content_rank) OVER (
                   PARTITION BY owner_id ORDER BY source_order
                   ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
               ) AS prior_max_rank
        FROM machine_children
    ) AS ordered
    WHERE content_rank<prior_max_rank
    GROUP BY owner_id
),
strict_switches AS (
    SELECT switch.source_element_id AS owner_id
    FROM mame_switches AS switch
    JOIN catalog_source_elements AS element
      ON element.source_element_id=switch.source_element_id
    JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id
    JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
    WHERE rules.format_family='mame' AND rules.dialect='strict-dtd'
),
switch_children AS (
    SELECT condition.switch_id AS owner_id,condition.source_order,0 AS content_rank
    FROM mame_switch_conditions AS condition
    JOIN strict_switches AS strict ON strict.owner_id=condition.switch_id
    UNION ALL
    SELECT location.switch_id,location.source_order,1
    FROM mame_switch_locations AS location
    JOIN strict_switches AS strict ON strict.owner_id=location.switch_id
    UNION ALL
    SELECT value.switch_id,value.source_order,2
    FROM mame_switch_values AS value
    JOIN strict_switches AS strict ON strict.owner_id=value.switch_id
),
strict_switch_sequence AS (
    SELECT owner_id
    FROM (
        SELECT owner_id,content_rank,
               max(content_rank) OVER (
                   PARTITION BY owner_id ORDER BY source_order
                   ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
               ) AS prior_max_rank
        FROM switch_children
    ) AS ordered
    WHERE content_rank<prior_max_rank
    GROUP BY owner_id
),
strict_devices AS (
    SELECT device.source_element_id AS owner_id,element.edition_id
    FROM mame_devices AS device
    JOIN catalog_source_elements AS element
      ON element.source_element_id=device.source_element_id
    JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id
    JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
    WHERE rules.format_family='mame' AND rules.dialect='strict-dtd'
)
SELECT 'mame_root_requires_machine' AS problem,
       document.edition_id AS owner_id,
       document.edition_id AS edition_id
FROM mame_documents AS document
LEFT JOIN catalog_set_groups AS root_group
  ON root_group.edition_id=document.edition_id
 AND root_group.group_kind='root'
LEFT JOIN catalog_sets AS machine_set
  ON machine_set.set_group_id=root_group.set_group_id
LEFT JOIN mame_machines AS machine
  ON machine.set_id=machine_set.set_id
GROUP BY document.edition_id
HAVING count(machine.set_id)=0

UNION ALL

-- ROM size is required by the pinned DTD but intentionally nullable in the
-- shared projection because observed compatibility accepts its absence.
SELECT 'strict_rom_requires_size',
       rom.media_entry_id,
       element.edition_id
FROM mame_roms AS rom
JOIN catalog_source_elements AS element
  ON element.source_element_id=rom.media_entry_id
JOIN catalog_editions AS edition ON edition.edition_id=element.edition_id
JOIN catalog_reading_rules AS rules ON rules.reading_rules_id=edition.reading_rules_id
WHERE rules.format_family='mame'
  AND rules.dialect='strict-dtd'
  AND rom.size_text IS NULL

UNION ALL

SELECT 'machine_requires_description',
       machine.set_id,
       element.edition_id
FROM mame_machines AS machine
JOIN catalog_source_elements AS element
  ON element.source_element_id=machine.set_id
LEFT JOIN mame_machine_text_elements AS description
  ON description.machine_id=machine.set_id
 AND description.field_kind='description'
GROUP BY machine.set_id,element.edition_id
HAVING count(description.source_element_id)=0

UNION ALL

-- The DTD's sequence is dialect-specific; observed compatibility retains the
-- parser's accepted source order.
SELECT 'strict_machine_child_sequence',
       machine.set_id,
       element.edition_id
FROM strict_machine_sequence AS violation
JOIN mame_machines AS machine ON machine.set_id=violation.owner_id
JOIN catalog_source_elements AS element
  ON element.source_element_id=machine.set_id

UNION ALL

SELECT 'strict_switch_child_sequence',
       switch.source_element_id,
       element.edition_id
FROM strict_switch_sequence AS violation
JOIN mame_switches AS switch ON switch.source_element_id=violation.owner_id
JOIN catalog_source_elements AS element
  ON element.source_element_id=switch.source_element_id

UNION ALL

SELECT 'strict_device_child_sequence',
       strict.owner_id,
       strict.edition_id
FROM strict_devices AS strict
WHERE EXISTS (
      SELECT 1
      FROM mame_device_instances AS instance
      JOIN mame_device_extensions AS extension
        ON extension.device_id=strict.owner_id
       AND extension.source_order<instance.source_order
      WHERE instance.device_id=strict.owner_id
  );
