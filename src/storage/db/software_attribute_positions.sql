-- Position-only software-list lexical witnesses. Native values stay in their owners.
CREATE TABLE software_wrapper_attribute_positions (
    wrapper_id INTEGER NOT NULL CHECK(typeof(wrapper_id)='integer' AND wrapper_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 0),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(wrapper_id,field_kind), UNIQUE(wrapper_id,source_order),
    FOREIGN KEY(wrapper_id) REFERENCES software_wrapper_headers(wrapper_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_list_attribute_positions (
    namespace_id INTEGER NOT NULL CHECK(typeof(namespace_id)='integer' AND namespace_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(namespace_id,field_kind), UNIQUE(namespace_id,source_order),
    FOREIGN KEY(namespace_id) REFERENCES software_lists(namespace_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_item_attribute_positions (
    record_id INTEGER NOT NULL CHECK(typeof(record_id)='integer' AND record_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(record_id,field_kind), UNIQUE(record_id,source_order),
    FOREIGN KEY(record_id) REFERENCES software_items(record_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_item_info_attribute_positions (
    record_id INTEGER NOT NULL CHECK(typeof(record_id)='integer' AND record_id>0),
    value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(record_id,value_order,field_kind), UNIQUE(record_id,value_order,source_order),
    FOREIGN KEY(record_id,value_order) REFERENCES software_item_info(record_id,value_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_item_shared_feature_attribute_positions (
    record_id INTEGER NOT NULL CHECK(typeof(record_id)='integer' AND record_id>0),
    value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(record_id,value_order,field_kind), UNIQUE(record_id,value_order,source_order),
    FOREIGN KEY(record_id,value_order) REFERENCES software_item_shared_features(record_id,value_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_part_attribute_positions (
    part_id INTEGER NOT NULL CHECK(typeof(part_id)='integer' AND part_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(part_id,field_kind), UNIQUE(part_id,source_order),
    FOREIGN KEY(part_id) REFERENCES software_parts(part_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_part_feature_attribute_positions (
    part_id INTEGER NOT NULL CHECK(typeof(part_id)='integer' AND part_id>0),
    value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 1),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(part_id,value_order,field_kind), UNIQUE(part_id,value_order,source_order),
    FOREIGN KEY(part_id,value_order) REFERENCES software_part_features(part_id,value_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_data_area_attribute_positions (
    area_id INTEGER NOT NULL CHECK(typeof(area_id)='integer' AND area_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(area_id,field_kind), UNIQUE(area_id,source_order),
    FOREIGN KEY(area_id) REFERENCES software_data_areas(area_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_disk_area_attribute_positions (
    area_id INTEGER NOT NULL CHECK(typeof(area_id)='integer' AND area_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 0),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(area_id,field_kind), UNIQUE(area_id,source_order),
    FOREIGN KEY(area_id) REFERENCES software_disk_areas(area_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_rom_attribute_positions (
    occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 7),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order),
    FOREIGN KEY(occurrence_id) REFERENCES software_rom_entries(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_disk_attribute_positions (
    occurrence_id INTEGER NOT NULL CHECK(typeof(occurrence_id)='integer' AND occurrence_id>0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 3),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(occurrence_id,field_kind), UNIQUE(occurrence_id,source_order),
    FOREIGN KEY(occurrence_id) REFERENCES software_disk_entries(occurrence_id) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_part_dipswitch_attribute_positions (
    part_id INTEGER NOT NULL CHECK(typeof(part_id)='integer' AND part_id>0),
    dipswitch_order INTEGER NOT NULL CHECK(typeof(dipswitch_order)='integer' AND dipswitch_order>=0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(part_id,dipswitch_order,field_kind), UNIQUE(part_id,dipswitch_order,source_order),
    FOREIGN KEY(part_id,dipswitch_order) REFERENCES software_part_dipswitches(part_id,dipswitch_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE TABLE software_part_dip_value_attribute_positions (
    part_id INTEGER NOT NULL CHECK(typeof(part_id)='integer' AND part_id>0),
    dipswitch_order INTEGER NOT NULL CHECK(typeof(dipswitch_order)='integer' AND dipswitch_order>=0),
    value_order INTEGER NOT NULL CHECK(typeof(value_order)='integer' AND value_order>=0),
    field_kind INTEGER NOT NULL CHECK(typeof(field_kind)='integer' AND field_kind BETWEEN 0 AND 2),
    source_order INTEGER NOT NULL CHECK(typeof(source_order)='integer' AND source_order>=0),
    source_line INTEGER NOT NULL CHECK(typeof(source_line)='integer' AND source_line>0),
    source_column INTEGER NOT NULL CHECK(typeof(source_column)='integer' AND source_column>0),
    PRIMARY KEY(part_id,dipswitch_order,value_order,field_kind), UNIQUE(part_id,dipswitch_order,value_order,source_order),
    FOREIGN KEY(part_id,dipswitch_order,value_order) REFERENCES software_part_dip_values(part_id,dipswitch_order,value_order) ON DELETE RESTRICT
) WITHOUT ROWID;

CREATE VIEW software_expected_attribute_positions AS
SELECT snapshot.snapshot_key AS snapshot_key,'wrapper' AS owner_kind,owner.wrapper_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_wrapper_headers AS owner JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND document.envelope_kind='plural_lists' AND CASE fields.field_kind WHEN 0 THEN (owner.build IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'list' AS owner_kind,owner.namespace_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_lists AS owner JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.description IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item' AS owner_kind,owner.record_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (sets.set_name IS NOT NULL) WHEN 1 THEN (EXISTS(SELECT 1 FROM software_clone_links AS link WHERE link.set_id=owner.record_id)) WHEN 2 THEN (owner.supported_specified=1) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item_info' AS owner_kind,owner.record_id AS owner_a,owner.value_order AS owner_b,0 AS owner_c,fields.field_kind
FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item_shared_feature' AS owner_kind,owner.record_id AS owner_a,owner.value_order AS owner_b,0 AS owner_c,fields.field_kind
FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part' AS owner_kind,owner.part_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.part_name IS NOT NULL) WHEN 1 THEN (owner.interface IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_feature' AS owner_kind,owner.part_id AS owner_a,owner.value_order AS owner_b,0 AS owner_c,fields.field_kind
FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'data_area' AS owner_kind,owner.area_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind UNION ALL SELECT 3 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND NOT EXISTS(SELECT 1 FROM software_disk_areas AS other WHERE other.area_id=owner.area_id) AND CASE fields.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) WHEN 1 THEN (owner.declared_size_text IS NOT NULL) WHEN 2 THEN (owner.width_specified=1) WHEN 3 THEN (owner.endianness_specified=1) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'disk_area' AS owner_kind,owner.area_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND NOT EXISTS(SELECT 1 FROM software_data_areas AS other WHERE other.area_id=owner.area_id) AND CASE fields.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'rom' AS owner_kind,owner.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind UNION ALL SELECT 3 AS field_kind UNION ALL SELECT 4 AS field_kind UNION ALL SELECT 5 AS field_kind UNION ALL SELECT 6 AS field_kind UNION ALL SELECT 7 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND area.record_id=owner.record_id AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation') AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.size_text IS NOT NULL) WHEN 2 THEN (owner.crc_text IS NOT NULL) WHEN 3 THEN (owner.sha1_text IS NOT NULL) WHEN 4 THEN (owner.offset_text IS NOT NULL) WHEN 5 THEN (owner.value IS NOT NULL) WHEN 6 THEN (owner.status_specified=1) WHEN 7 THEN (owner.load_instruction IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'disk' AS owner_kind,owner.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,fields.field_kind
FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind UNION ALL SELECT 3 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND area.record_id=owner.record_id AND occurrence.claim_kind='software_disk_entry' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.sha1_text IS NOT NULL) WHEN 2 THEN (owner.status_specified=1) WHEN 3 THEN (owner.writeable_specified=1) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_dipswitch' AS owner_kind,owner.part_id AS owner_a,owner.dipswitch_order AS owner_b,0 AS owner_c,fields.field_kind
FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.tag IS NOT NULL) WHEN 2 THEN (owner.mask IS NOT NULL) ELSE 0 END
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_dip_value' AS owner_kind,owner.part_id AS owner_a,owner.dipswitch_order AS owner_b,owner.value_order AS owner_c,fields.field_kind
FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
CROSS JOIN (SELECT 0 AS field_kind UNION ALL SELECT 1 AS field_kind UNION ALL SELECT 2 AS field_kind) AS fields
WHERE interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE fields.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) WHEN 2 THEN (owner.default_specified=1) ELSE 0 END;

CREATE VIEW software_actual_attribute_positions AS
SELECT snapshot.snapshot_key AS snapshot_key,'wrapper' AS owner_kind,position.wrapper_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM software_wrapper_attribute_positions AS position JOIN software_wrapper_headers AS owner ON owner.wrapper_id=position.wrapper_id JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'list' AS owner_kind,position.namespace_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM software_list_attribute_positions AS position JOIN software_lists AS owner ON owner.namespace_id=position.namespace_id JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item' AS owner_kind,position.record_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM software_item_attribute_positions AS position JOIN software_items AS owner ON owner.record_id=position.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item_info' AS owner_kind,position.record_id AS owner_a,position.value_order AS owner_b,0 AS owner_c,position.field_kind
FROM software_item_info_attribute_positions AS position JOIN software_item_info AS owner ON owner.record_id=position.record_id AND owner.value_order=position.value_order JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'item_shared_feature' AS owner_kind,position.record_id AS owner_a,position.value_order AS owner_b,0 AS owner_c,position.field_kind
FROM software_item_shared_feature_attribute_positions AS position JOIN software_item_shared_features AS owner ON owner.record_id=position.record_id AND owner.value_order=position.value_order JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part' AS owner_kind,position.part_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM software_part_attribute_positions AS position JOIN software_parts AS owner ON owner.part_id=position.part_id JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_feature' AS owner_kind,position.part_id AS owner_a,position.value_order AS owner_b,0 AS owner_c,position.field_kind
FROM software_part_feature_attribute_positions AS position JOIN software_part_features AS owner ON owner.part_id=position.part_id AND owner.value_order=position.value_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'data_area' AS owner_kind,position.area_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM software_data_area_attribute_positions AS position JOIN software_data_areas AS owner ON owner.area_id=position.area_id JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'disk_area' AS owner_kind,position.area_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN software_items AS item ON item.record_id=sets.set_id
CROSS JOIN software_parts AS part ON part.record_id=item.record_id
CROSS JOIN software_areas AS area ON area.part_id=part.part_id AND area.record_id=part.record_id
CROSS JOIN software_disk_areas AS owner ON owner.area_id=area.area_id
CROSS JOIN software_disk_area_attribute_positions AS position ON position.area_id=owner.area_id
JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'rom' AS owner_kind,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN software_items AS item ON item.record_id=sets.set_id
CROSS JOIN software_parts AS part ON part.record_id=item.record_id
CROSS JOIN software_areas AS area ON area.part_id=part.part_id AND area.record_id=part.record_id
CROSS JOIN software_rom_entries AS owner ON owner.area_id=area.area_id
CROSS JOIN software_data_areas AS detail ON detail.area_id=owner.area_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id
CROSS JOIN software_rom_attribute_positions AS position ON position.occurrence_id=owner.occurrence_id
JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'disk' AS owner_kind,position.occurrence_id AS owner_a,0 AS owner_b,0 AS owner_c,position.field_kind
FROM catalog_set_groups AS groups
CROSS JOIN catalog_sets AS sets ON sets.set_group_id=groups.set_group_id
CROSS JOIN software_items AS item ON item.record_id=sets.set_id
CROSS JOIN software_parts AS part ON part.record_id=item.record_id
CROSS JOIN software_areas AS area ON area.part_id=part.part_id AND area.record_id=part.record_id
CROSS JOIN software_disk_entries AS owner ON owner.area_id=area.area_id
CROSS JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id
CROSS JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id
CROSS JOIN software_disk_attribute_positions AS position ON position.occurrence_id=owner.occurrence_id
JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key
JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_dipswitch' AS owner_kind,position.part_id AS owner_a,position.dipswitch_order AS owner_b,0 AS owner_c,position.field_kind
FROM software_part_dipswitch_attribute_positions AS position JOIN software_part_dipswitches AS owner ON owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
UNION ALL
SELECT groups.snapshot_key AS snapshot_key,'part_dip_value' AS owner_kind,position.part_id AS owner_a,position.dipswitch_order AS owner_b,position.value_order AS owner_c,position.field_kind
FROM software_part_dip_value_attribute_positions AS position JOIN software_part_dip_values AS owner ON owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order AND owner.value_order=position.value_order JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key;

CREATE VIEW software_attribute_violations AS
SELECT expected.*, 'missing_position' AS reason FROM software_expected_attribute_positions AS expected
WHERE NOT EXISTS(SELECT 1 FROM software_actual_attribute_positions AS actual WHERE actual.snapshot_key=expected.snapshot_key AND actual.owner_kind=expected.owner_kind AND actual.owner_a=expected.owner_a AND actual.owner_b=expected.owner_b AND actual.owner_c=expected.owner_c AND actual.field_kind=expected.field_kind)
UNION ALL
SELECT actual.*, 'extraneous_or_misplaced_position' AS reason FROM software_actual_attribute_positions AS actual
WHERE NOT EXISTS(SELECT 1 FROM software_expected_attribute_positions AS expected WHERE actual.snapshot_key=expected.snapshot_key AND actual.owner_kind=expected.owner_kind AND actual.owner_a=expected.owner_a AND actual.owner_b=expected.owner_b AND actual.owner_c=expected.owner_c AND actual.field_kind=expected.field_kind)
UNION ALL
SELECT (SELECT snapshot.snapshot_key FROM software_wrapper_headers AS owner JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.wrapper_id=position.wrapper_id),'wrapper',position.wrapper_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_wrapper_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 0
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_wrapper_headers AS owner JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.wrapper_id=position.wrapper_id AND interpretation.format='mame-softwarelist-xml' AND document.envelope_kind='plural_lists')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_lists AS owner JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.namespace_id=position.namespace_id),'list',position.namespace_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_list_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 1
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_lists AS owner JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.namespace_id=position.namespace_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id),'item',position.record_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_item_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 2
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id AND owner.value_order=position.value_order),'item_info',position.record_id,position.value_order,0,position.field_kind,'orphan_or_invalid_position'
FROM software_item_info_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 1
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id AND owner.value_order=position.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id AND owner.value_order=position.value_order),'item_shared_feature',position.record_id,position.value_order,0,position.field_kind,'orphan_or_invalid_position'
FROM software_item_shared_feature_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 1
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.record_id=position.record_id AND owner.value_order=position.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id),'part',position.part_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_part_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 1
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.value_order=position.value_order),'part_feature',position.part_id,position.value_order,0,position.field_kind,'orphan_or_invalid_position'
FROM software_part_feature_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 1
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.value_order=position.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.area_id=position.area_id),'data_area',position.area_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_data_area_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 3
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.area_id=position.area_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND NOT EXISTS(SELECT 1 FROM software_disk_areas AS other WHERE other.area_id=owner.area_id))
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.area_id=position.area_id),'disk_area',position.area_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_disk_area_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 0
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.area_id=position.area_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND NOT EXISTS(SELECT 1 FROM software_data_areas AS other WHERE other.area_id=owner.area_id))
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.occurrence_id=position.occurrence_id),'rom',position.occurrence_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_rom_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 7
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.occurrence_id=position.occurrence_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND area.record_id=owner.record_id AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation'))
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.occurrence_id=position.occurrence_id),'disk',position.occurrence_id,0,0,position.field_kind,'orphan_or_invalid_position'
FROM software_disk_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 3
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.occurrence_id=position.occurrence_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND area.record_id=owner.record_id AND occurrence.claim_kind='software_disk_entry')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order),'part_dipswitch',position.part_id,position.dipswitch_order,0,position.field_kind,'orphan_or_invalid_position'
FROM software_part_dipswitch_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 2
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
UNION ALL
SELECT (SELECT groups.snapshot_key FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order AND owner.value_order=position.value_order),'part_dip_value',position.part_id,position.dipswitch_order,position.value_order,position.field_kind,'orphan_or_invalid_position'
FROM software_part_dip_value_attribute_positions AS position
WHERE typeof(position.field_kind)<>'integer' OR position.field_kind NOT BETWEEN 0 AND 2
 OR typeof(position.source_order)<>'integer' OR position.source_order<0
 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0
 OR NOT EXISTS(SELECT 1 FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key WHERE owner.part_id=position.part_id AND owner.dipswitch_order=position.dipswitch_order AND owner.value_order=position.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item')
;

CREATE TRIGGER software_wrapper_attribute_positions_insert_guard BEFORE INSERT ON software_wrapper_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_wrapper_attribute_positions AS old WHERE old.wrapper_id=NEW.wrapper_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_wrapper_headers AS owner JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.wrapper_id=NEW.wrapper_id AND interpretation.format='mame-softwarelist-xml' AND document.envelope_kind='plural_lists' AND CASE NEW.field_kind WHEN 0 THEN (owner.build IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=snapshot.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_wrapper_attribute_positions_immutable_update BEFORE UPDATE ON software_wrapper_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_wrapper_attribute_positions_immutable_delete BEFORE DELETE ON software_wrapper_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_list_attribute_positions_insert_guard BEFORE INSERT ON software_list_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_list_attribute_positions AS old WHERE old.namespace_id=NEW.namespace_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_lists AS owner JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.namespace_id=NEW.namespace_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.description IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_list_attribute_positions_immutable_update BEFORE UPDATE ON software_list_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_list_attribute_positions_immutable_delete BEFORE DELETE ON software_list_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_item_attribute_positions_insert_guard BEFORE INSERT ON software_item_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_item_attribute_positions AS old WHERE old.record_id=NEW.record_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.record_id=NEW.record_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (sets.set_name IS NOT NULL) WHEN 1 THEN (EXISTS(SELECT 1 FROM software_clone_links AS link WHERE link.set_id=owner.record_id)) WHEN 2 THEN (owner.supported_specified=1) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_item_attribute_positions_immutable_update BEFORE UPDATE ON software_item_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_item_attribute_positions_immutable_delete BEFORE DELETE ON software_item_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_item_info_attribute_positions_insert_guard BEFORE INSERT ON software_item_info_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_item_info_attribute_positions AS old WHERE old.record_id=NEW.record_id AND old.value_order=NEW.value_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.record_id=NEW.record_id AND owner.value_order=NEW.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_item_info_attribute_positions_immutable_update BEFORE UPDATE ON software_item_info_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_item_info_attribute_positions_immutable_delete BEFORE DELETE ON software_item_info_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_item_shared_feature_attribute_positions_insert_guard BEFORE INSERT ON software_item_shared_feature_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_item_shared_feature_attribute_positions AS old WHERE old.record_id=NEW.record_id AND old.value_order=NEW.value_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.record_id=NEW.record_id AND owner.value_order=NEW.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_item_shared_feature_attribute_positions_immutable_update BEFORE UPDATE ON software_item_shared_feature_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_item_shared_feature_attribute_positions_immutable_delete BEFORE DELETE ON software_item_shared_feature_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_part_attribute_positions_insert_guard BEFORE INSERT ON software_part_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_part_attribute_positions AS old WHERE old.part_id=NEW.part_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.part_id=NEW.part_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.part_name IS NOT NULL) WHEN 1 THEN (owner.interface IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_part_attribute_positions_immutable_update BEFORE UPDATE ON software_part_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_part_attribute_positions_immutable_delete BEFORE DELETE ON software_part_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_part_feature_attribute_positions_insert_guard BEFORE INSERT ON software_part_feature_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_part_feature_attribute_positions AS old WHERE old.part_id=NEW.part_id AND old.value_order=NEW.value_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.part_id=NEW.part_id AND owner.value_order=NEW.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_part_feature_attribute_positions_immutable_update BEFORE UPDATE ON software_part_feature_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_part_feature_attribute_positions_immutable_delete BEFORE DELETE ON software_part_feature_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_data_area_attribute_positions_insert_guard BEFORE INSERT ON software_data_area_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_data_area_attribute_positions AS old WHERE old.area_id=NEW.area_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.area_id=NEW.area_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND NOT EXISTS(SELECT 1 FROM software_disk_areas AS other WHERE other.area_id=owner.area_id) AND CASE NEW.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) WHEN 1 THEN (owner.declared_size_text IS NOT NULL) WHEN 2 THEN (owner.width_specified=1) WHEN 3 THEN (owner.endianness_specified=1) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_data_area_attribute_positions_immutable_update BEFORE UPDATE ON software_data_area_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_data_area_attribute_positions_immutable_delete BEFORE DELETE ON software_data_area_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_disk_area_attribute_positions_insert_guard BEFORE INSERT ON software_disk_area_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_disk_area_attribute_positions AS old WHERE old.area_id=NEW.area_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.area_id=NEW.area_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND NOT EXISTS(SELECT 1 FROM software_data_areas AS other WHERE other.area_id=owner.area_id) AND CASE NEW.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_disk_area_attribute_positions_immutable_update BEFORE UPDATE ON software_disk_area_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_disk_area_attribute_positions_immutable_delete BEFORE DELETE ON software_disk_area_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_rom_attribute_positions_insert_guard BEFORE INSERT ON software_rom_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_rom_attribute_positions AS old WHERE old.occurrence_id=NEW.occurrence_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.occurrence_id=NEW.occurrence_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND area.record_id=owner.record_id AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation') AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.size_text IS NOT NULL) WHEN 2 THEN (owner.crc_text IS NOT NULL) WHEN 3 THEN (owner.sha1_text IS NOT NULL) WHEN 4 THEN (owner.offset_text IS NOT NULL) WHEN 5 THEN (owner.value IS NOT NULL) WHEN 6 THEN (owner.status_specified=1) WHEN 7 THEN (owner.load_instruction IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_rom_attribute_positions_immutable_update BEFORE UPDATE ON software_rom_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_rom_attribute_positions_immutable_delete BEFORE DELETE ON software_rom_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_disk_attribute_positions_insert_guard BEFORE INSERT ON software_disk_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_disk_attribute_positions AS old WHERE old.occurrence_id=NEW.occurrence_id AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.occurrence_id=NEW.occurrence_id AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND area.record_id=owner.record_id AND occurrence.claim_kind='software_disk_entry' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.sha1_text IS NOT NULL) WHEN 2 THEN (owner.status_specified=1) WHEN 3 THEN (owner.writeable_specified=1) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_disk_attribute_positions_immutable_update BEFORE UPDATE ON software_disk_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_disk_attribute_positions_immutable_delete BEFORE DELETE ON software_disk_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_part_dipswitch_attribute_positions_insert_guard BEFORE INSERT ON software_part_dipswitch_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_part_dipswitch_attribute_positions AS old WHERE old.part_id=NEW.part_id AND old.dipswitch_order=NEW.dipswitch_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.part_id=NEW.part_id AND owner.dipswitch_order=NEW.dipswitch_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.tag IS NOT NULL) WHEN 2 THEN (owner.mask IS NOT NULL) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_part_dipswitch_attribute_positions_immutable_update BEFORE UPDATE ON software_part_dipswitch_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_part_dipswitch_attribute_positions_immutable_delete BEFORE DELETE ON software_part_dipswitch_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_part_dip_value_attribute_positions_insert_guard BEFORE INSERT ON software_part_dip_value_attribute_positions
WHEN EXISTS(SELECT 1 FROM software_part_dip_value_attribute_positions AS old WHERE old.part_id=NEW.part_id AND old.dipswitch_order=NEW.dipswitch_order AND old.value_order=NEW.value_order AND (old.field_kind=NEW.field_kind OR old.source_order=NEW.source_order))
 OR NOT EXISTS(SELECT 1 FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE owner.part_id=NEW.part_id AND owner.dipswitch_order=NEW.dipswitch_order AND owner.value_order=NEW.value_order AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND CASE NEW.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) WHEN 2 THEN (owner.default_specified=1) ELSE 0 END
 AND NOT EXISTS(SELECT 1 FROM snapshot_publications AS seal WHERE seal.snapshot_key=groups.snapshot_key))
BEGIN SELECT RAISE(ABORT,'software attribute position requires a present field on its unpublished native owner'); END;
CREATE TRIGGER software_part_dip_value_attribute_positions_immutable_update BEFORE UPDATE ON software_part_dip_value_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;
CREATE TRIGGER software_part_dip_value_attribute_positions_immutable_delete BEFORE DELETE ON software_part_dip_value_attribute_positions
BEGIN SELECT RAISE(ABORT,'software attribute positions are immutable'); END;

CREATE TRIGGER software_attributes_publication_guard BEFORE INSERT ON snapshot_publications
WHEN EXISTS(SELECT 1 FROM software_wrapper_headers AS owner JOIN software_documents AS document ON document.snapshot_key=owner.snapshot_key JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=document.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE snapshot.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND document.envelope_kind='plural_lists') AND (
 (SELECT COUNT(*) FROM software_wrapper_attribute_positions AS position WHERE position.wrapper_id=owner.wrapper_id)<>(CAST((owner.build IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_wrapper_attribute_positions AS position WHERE position.wrapper_id=owner.wrapper_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.build IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_lists AS owner JOIN catalog_set_groups AS groups ON groups.set_group_id=owner.namespace_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list') AND (
 (SELECT COUNT(*) FROM software_list_attribute_positions AS position WHERE position.namespace_id=owner.namespace_id)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.description IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_list_attribute_positions AS position WHERE position.namespace_id=owner.namespace_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.description IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_item_attribute_positions AS position WHERE position.record_id=owner.record_id)<>(CAST((sets.set_name IS NOT NULL) AS INTEGER)+CAST((EXISTS(SELECT 1 FROM software_clone_links AS link WHERE link.set_id=owner.record_id)) AS INTEGER)+CAST((owner.supported_specified=1) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_item_attribute_positions AS position WHERE position.record_id=owner.record_id AND (NOT (CASE position.field_kind WHEN 0 THEN (sets.set_name IS NOT NULL) WHEN 1 THEN (EXISTS(SELECT 1 FROM software_clone_links AS link WHERE link.set_id=owner.record_id)) WHEN 2 THEN (owner.supported_specified=1) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_item_info_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.value IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_item_info_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_item_shared_feature_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.value IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_item_shared_feature_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_part_attribute_positions AS position WHERE position.part_id=owner.part_id)<>(CAST((owner.part_name IS NOT NULL) AS INTEGER)+CAST((owner.interface IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_part_attribute_positions AS position WHERE position.part_id=owner.part_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.part_name IS NOT NULL) WHEN 1 THEN (owner.interface IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_part_feature_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.value_order=owner.value_order)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.value IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_part_feature_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.value_order=owner.value_order AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND NOT EXISTS(SELECT 1 FROM software_disk_areas AS other WHERE other.area_id=owner.area_id)) AND (
 (SELECT COUNT(*) FROM software_data_area_attribute_positions AS position WHERE position.area_id=owner.area_id)<>(CAST((owner.area_name IS NOT NULL) AS INTEGER)+CAST((owner.declared_size_text IS NOT NULL) AS INTEGER)+CAST((owner.width_specified=1) AS INTEGER)+CAST((owner.endianness_specified=1) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_data_area_attribute_positions AS position WHERE position.area_id=owner.area_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) WHEN 1 THEN (owner.declared_size_text IS NOT NULL) WHEN 2 THEN (owner.width_specified=1) WHEN 3 THEN (owner.endianness_specified=1) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND NOT EXISTS(SELECT 1 FROM software_data_areas AS other WHERE other.area_id=owner.area_id)) AND (
 (SELECT COUNT(*) FROM software_disk_area_attribute_positions AS position WHERE position.area_id=owner.area_id)<>(CAST((owner.area_name IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_disk_area_attribute_positions AS position WHERE position.area_id=owner.area_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.area_name IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND area.record_id=owner.record_id AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation')) AND (
 (SELECT COUNT(*) FROM software_rom_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.size_text IS NOT NULL) AS INTEGER)+CAST((owner.crc_text IS NOT NULL) AS INTEGER)+CAST((owner.sha1_text IS NOT NULL) AS INTEGER)+CAST((owner.offset_text IS NOT NULL) AS INTEGER)+CAST((owner.value IS NOT NULL) AS INTEGER)+CAST((owner.status_specified=1) AS INTEGER)+CAST((owner.load_instruction IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_rom_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.size_text IS NOT NULL) WHEN 2 THEN (owner.crc_text IS NOT NULL) WHEN 3 THEN (owner.sha1_text IS NOT NULL) WHEN 4 THEN (owner.offset_text IS NOT NULL) WHEN 5 THEN (owner.value IS NOT NULL) WHEN 6 THEN (owner.status_specified=1) WHEN 7 THEN (owner.load_instruction IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND area.record_id=owner.record_id AND occurrence.claim_kind='software_disk_entry') AND (
 (SELECT COUNT(*) FROM software_disk_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.sha1_text IS NOT NULL) AS INTEGER)+CAST((owner.status_specified=1) AS INTEGER)+CAST((owner.writeable_specified=1) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_disk_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.sha1_text IS NOT NULL) WHEN 2 THEN (owner.status_specified=1) WHEN 3 THEN (owner.writeable_specified=1) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_part_dipswitch_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.tag IS NOT NULL) AS INTEGER)+CAST((owner.mask IS NOT NULL) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_part_dipswitch_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.tag IS NOT NULL) WHEN 2 THEN (owner.mask IS NOT NULL) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
 OR EXISTS(SELECT 1 FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key
 WHERE groups.snapshot_key=NEW.snapshot_key AND (interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item') AND (
 (SELECT COUNT(*) FROM software_part_dip_value_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order AND position.value_order=owner.value_order)<>(CAST((owner.name IS NOT NULL) AS INTEGER)+CAST((owner.value IS NOT NULL) AS INTEGER)+CAST((owner.default_specified=1) AS INTEGER))
 OR EXISTS(SELECT 1 FROM software_part_dip_value_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order AND position.value_order=owner.value_order AND (NOT (CASE position.field_kind WHEN 0 THEN (owner.name IS NOT NULL) WHEN 1 THEN (owner.value IS NOT NULL) WHEN 2 THEN (owner.default_specified=1) ELSE 0 END) OR typeof(position.source_order)<>'integer' OR position.source_order<0 OR typeof(position.source_line)<>'integer' OR position.source_line<=0 OR typeof(position.source_column)<>'integer' OR position.source_column<=0))))
BEGIN SELECT RAISE(ABORT,'software publication requires exact native attribute presence and ownership'); END;
