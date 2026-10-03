//! Attribute rank witnesses, without physical offsets or generated owner IDs.
use crate::domain::SnapshotKey;
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Text},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Bool)]
    valid_types: bool,
    #[diesel(sql_type=BigInt)]
    record_id: i64,
    #[diesel(sql_type=BigInt)]
    path_a: i64,
    #[diesel(sql_type=BigInt)]
    path_b: i64,
    #[diesel(sql_type=BigInt)]
    path_c: i64,
    #[diesel(sql_type=BigInt)]
    field_kind: i64,
    #[diesel(sql_type=BigInt)]
    source_order: i64,
}

pub(super) fn lists(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<i64>>> {
    let rows=sql_query("SELECT position.namespace_id AS record_id,0 AS path_a,0 AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM catalog_set_groups AS groups CROSS JOIN software_list_attribute_positions AS position WHERE groups.snapshot_key=? AND groups.kind='software_list' AND position.namespace_id=groups.set_group_id ORDER BY position.source_order").bind::<Text,_>(key.as_str()).load::<Row>(conn)?;
    let mut result = BTreeMap::<i64, Vec<i64>>::new();
    for row in rows {
        if !row.valid_types || !(0..2).contains(&row.field_kind) {
            return Err(crate::Error::XmlValidation(
                "invalid software list attribute history position".into(),
            ));
        }
        result
            .entry(row.record_id)
            .or_default()
            .push(row.field_kind);
    }
    Ok(result)
}

pub(super) fn items(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Value>> {
    let mut grouped = BTreeMap::<i64, Vec<Value>>::new();
    append(
        conn,
        key,
        &mut grouped,
        "item",
        3,
        "SELECT sets.set_id AS record_id,0 AS path_a,0 AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_items AS owner JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_item_attribute_positions AS position WHERE position.record_id=owner.record_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "item_info",
        2,
        "SELECT sets.set_id AS record_id,owner.value_order AS path_a,0 AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_item_info AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_item_info_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "item_shared_feature",
        2,
        "SELECT sets.set_id AS record_id,owner.value_order AS path_a,0 AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_item_shared_features AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_item_shared_feature_attribute_positions AS position WHERE position.record_id=owner.record_id AND position.value_order=owner.value_order AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "part",
        2,
        "SELECT sets.set_id AS record_id,owner.part_order AS path_a,0 AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_parts AS owner JOIN software_items AS item ON item.record_id=owner.record_id JOIN catalog_sets AS sets ON sets.set_id=owner.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_part_attribute_positions AS position WHERE position.part_id=owner.part_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "part_feature",
        2,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,owner.value_order AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_part_features AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_part_feature_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.value_order=owner.value_order AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "data_area",
        4,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,area.area_order AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_data_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_data_area_attribute_positions AS position WHERE position.area_id=owner.area_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND NOT EXISTS(SELECT 1 FROM software_disk_areas AS other WHERE other.area_id=owner.area_id) ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "disk_area",
        1,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,area.area_order AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_disk_areas AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_disk_area_attribute_positions AS position WHERE position.area_id=owner.area_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND NOT EXISTS(SELECT 1 FROM software_data_areas AS other WHERE other.area_id=owner.area_id) ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "rom",
        8,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,area.area_order AS path_b,owner.component_order AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_rom_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_data_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_rom_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='data' AND area.record_id=owner.record_id AND occurrence.claim_kind IN ('software_rom_entry','software_rom_operation') ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "disk",
        4,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,area.area_order AS path_b,owner.component_order AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_disk_entries AS owner JOIN software_areas AS area ON area.area_id=owner.area_id JOIN software_parts AS part ON part.part_id=area.part_id AND part.record_id=area.record_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN software_disk_areas AS detail ON detail.area_id=owner.area_id JOIN asset_occurrences AS occurrence ON occurrence.occurrence_id=owner.occurrence_id AND occurrence.record_id=owner.record_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_disk_attribute_positions AS position WHERE position.occurrence_id=owner.occurrence_id AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' AND area.area_kind='disk' AND area.record_id=owner.record_id AND occurrence.claim_kind='software_disk_entry' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "part_dipswitch",
        3,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,owner.dipswitch_order AS path_b,0 AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_part_dipswitches AS owner JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_part_dipswitch_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    append(
        conn,
        key,
        &mut grouped,
        "part_dip_value",
        3,
        "SELECT sets.set_id AS record_id,part.part_order AS path_a,owner.dipswitch_order AS path_b,owner.value_order AS path_c,position.field_kind,position.source_order,(typeof(position.source_order)='integer' AND position.source_order>=0 AND typeof(position.field_kind)='integer' AND typeof(position.source_line)='integer' AND position.source_line>0 AND typeof(position.source_column)='integer' AND position.source_column>0) AS valid_types FROM software_part_dip_values AS owner JOIN software_part_dipswitches AS switch ON switch.part_id=owner.part_id AND switch.dipswitch_order=owner.dipswitch_order JOIN software_parts AS part ON part.part_id=owner.part_id JOIN software_items AS item ON item.record_id=part.record_id JOIN catalog_sets AS sets ON sets.set_id=item.record_id JOIN catalog_set_groups AS groups ON groups.set_group_id=sets.set_group_id JOIN catalog_snapshots AS snapshot ON snapshot.snapshot_key=groups.snapshot_key JOIN parser_interpretations AS interpretation ON interpretation.interpretation_key=snapshot.interpretation_key CROSS JOIN software_part_dip_value_attribute_positions AS position WHERE position.part_id=owner.part_id AND position.dipswitch_order=owner.dipswitch_order AND position.value_order=owner.value_order AND groups.snapshot_key=? AND interpretation.format='mame-softwarelist-xml' AND groups.kind='software_list' AND sets.source_element_kind='software_item' ORDER BY position.source_order",
    )?;
    Ok(grouped
        .into_iter()
        .map(|(owner, values)| (owner, Value::Array(values)))
        .collect())
}

fn append(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    result: &mut BTreeMap<i64, Vec<Value>>,
    kind: &str,
    field_count: i64,
    query: &str,
) -> crate::Result<()> {
    let rows = sql_query(query)
        .bind::<Text, _>(key.as_str())
        .load::<Row>(conn)?;
    let mut owners = BTreeMap::<(i64, i64, i64, i64), Vec<i64>>::new();
    for row in rows {
        if !row.valid_types || row.source_order < 0 || !(0..field_count).contains(&row.field_kind) {
            return Err(crate::Error::XmlValidation(
                "invalid software attribute history position".into(),
            ));
        }
        let fields = owners
            .entry((row.record_id, row.path_a, row.path_b, row.path_c))
            .or_default();
        if fields.contains(&row.field_kind) {
            return Err(crate::Error::XmlValidation(
                "duplicate software attribute history field".into(),
            ));
        }
        fields.push(row.field_kind);
    }
    for ((owner, a, b, c), fields) in owners {
        result
            .entry(owner)
            .or_default()
            .push(json!({"owner":kind,"path":[a,b,c],"fields":fields}));
    }
    Ok(())
}
