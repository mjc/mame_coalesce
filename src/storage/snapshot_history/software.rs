//! Software-list history reads actual native owners; generated keys and physical
//! coordinates do not establish continuity between catalog editions.

use std::collections::BTreeMap;

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};
use serde::Serialize;
use serde_json::{Value, json};

use super::{CatalogRecords, SetRow};
use crate::domain::SnapshotKey;

type OrderedChildren = BTreeMap<i64, Vec<(i64, Value)>>;

macro_rules! native_row {
    ($name:ident { $( $(#[$attribute:meta])* $field:ident: $sql:ty => $rust:ty ),* $(,)? }) => {
        #[derive(QueryableByName, Serialize)]
        struct $name {
            #[diesel(sql_type = BigInt)]
            #[serde(skip)]
            parent_id: i64,
            #[diesel(sql_type = BigInt)]
            #[serde(skip)]
            source_order: i64,
            $( $(#[$attribute])* #[diesel(sql_type = $sql)] $field: $rust, )*
        }
    };
}

#[derive(QueryableByName, Serialize)]
struct ListRow {
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    namespace_id: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    notes: Option<String>,
}
native_row!(ItemRow {
    #[serde(skip)] record_id: BigInt => i64,
    #[serde(skip)] set_name: Text => String,
    #[serde(skip)] list_name: Text => String,
    clone_of: Nullable<Text> => Option<String>,
    supported: Nullable<Text> => Option<String>,
    supported_specified: Bool => bool,
    description: Text => String,
    year: Text => String,
    publisher: Text => String,
    notes: Nullable<Text> => Option<String>,
});
native_row!(TextPositionRow { field_kind: BigInt => i64 });
native_row!(NamedValueRow {
    name: Text => String,
    value: Nullable<Text> => Option<String>,
});
native_row!(PartRow {
    #[serde(skip)] part_id: BigInt => i64,
    part_name: Text => String,
    interface: Text => String,
});
native_row!(AreaRow {
    #[serde(skip)] area_id: BigInt => i64,
    #[serde(skip)] data_area_id: Nullable<BigInt> => Option<i64>,
    #[serde(skip)] disk_area_id: Nullable<BigInt> => Option<i64>,
    area_name: Text => String,
    area_kind: Text => String,
    declared_size_text: Nullable<Text> => Option<String>,
    width: Nullable<BigInt> => Option<i64>,
    width_specified: Bool => bool,
    endianness: Nullable<Text> => Option<String>,
    endianness_specified: Bool => bool,
});
native_row!(RomRow {
    #[serde(skip)] occurrence_id: BigInt => i64,
    name: Nullable<Text> => Option<String>,
    size_text: Nullable<Text> => Option<String>,
    offset_text: Nullable<Text> => Option<String>,
    crc_text: Nullable<Text> => Option<String>,
    sha1_text: Nullable<Text> => Option<String>,
    value: Nullable<Text> => Option<String>,
    dump_status: Nullable<Text> => Option<String>,
    status_specified: Bool => bool,
    load_instruction: Nullable<Text> => Option<String>,
    evidence_scope: Text => String,
    is_declaration: Bool => bool,
    operation: Text => String,
    #[serde(skip)] declaration_id: Nullable<BigInt> => Option<i64>,
});
native_row!(DiskRow {
    name: Text => String,
    sha1_text: Nullable<Text> => Option<String>,
    dump_status: Nullable<Text> => Option<String>,
    status_specified: Bool => bool,
    writeable: Nullable<Bool> => Option<bool>,
    writeable_specified: Bool => bool,
    evidence_scope: Text => String,
});
native_row!(SwitchRow {
    #[serde(skip)] dipswitch_order: BigInt => i64,
    name: Text => String,
    tag: Text => String,
    mask: Text => String,
});
native_row!(SwitchValueRow {
    #[serde(skip)] dipswitch_order: BigInt => i64,
    name: Text => String,
    value: Text => String,
    is_default: Bool => bool,
    default_specified: Bool => bool,
});

#[derive(QueryableByName, Serialize)]
struct EnvelopeRow {
    #[diesel(sql_type = Text)]
    envelope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
}

// Force the selected edition to be the outer owner before native PK/index
// seeks. Each query reads one native family without a multiplying sibling join.
const SETS: &str = " FROM catalog_set_groups AS groups \
    CROSS JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id ";
const PARTS: &str = " CROSS JOIN software_parts AS parts ON parts.record_id = sets.set_id ";
const AREAS: &str = " CROSS JOIN software_areas AS areas ON areas.part_id = parts.part_id ";
const AREA_DETAILS: &str = " LEFT JOIN software_data_areas AS data_area ON data_area.area_id = areas.area_id \
    LEFT JOIN software_disk_areas AS disk_area ON disk_area.area_id = areas.area_id ";
const SELECTED: &str = " WHERE groups.snapshot_key = ? AND groups.kind = 'software_list' ";

pub(super) fn load_document(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<Option<Value>> {
    let envelope = sql_query("SELECT document.envelope_kind, wrapper.build FROM software_documents AS document \
        LEFT JOIN software_wrapper_headers AS wrapper USING (snapshot_key) WHERE document.snapshot_key = ?")
        .bind::<Text,_>(key.as_str()).get_result::<EnvelopeRow>(conn).optional()?;
    let Some(envelope) = envelope else {
        return Ok(None);
    };
    let items = load_items(conn, key)?;
    let facts = load_item_facts(conn, key, &items)?;
    let lists = load_list_facts(conn, key, &items, &facts)?
        .into_iter()
        .map(|(_, facts)| facts)
        .collect::<Vec<_>>();
    Ok(Some(json!({"envelope": envelope, "lists": lists})))
}

fn load_list_facts(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    items: &[ItemRow],
    facts: &BTreeMap<i64, Value>,
) -> crate::Result<Vec<(i64, Value)>> {
    let lists = load_lists(conn, key)?;
    let positions = sql_query("SELECT positions.namespace_id AS parent_id, positions.source_order, positions.field_kind \
        FROM catalog_set_groups AS groups CROSS JOIN software_list_text_positions AS positions \
        ON positions.namespace_id = groups.set_group_id WHERE groups.snapshot_key = ? ORDER BY positions.source_order")
        .bind::<Text,_>(key.as_str()).load::<TextPositionRow>(conn)?;
    let mut children = OrderedChildren::new();
    for position in positions {
        push(
            &mut children,
            position.parent_id,
            position.source_order,
            "text",
            &position,
        )?;
    }
    for item in items {
        push(
            &mut children,
            item.parent_id,
            item.source_order,
            "software",
            &json!({"name": item.set_name, "facts": facts.get(&item.record_id)}),
        )?;
    }
    lists
        .into_iter()
        .map(|list| {
            let mut fact = serde_json::to_value(&list)?;
            fact["children"] = ordered(children.remove(&list.namespace_id).unwrap_or_default());
            Ok((list.namespace_id, fact))
        })
        .collect()
}

fn load_lists(conn: &mut SqliteConnection, key: &SnapshotKey) -> crate::Result<Vec<ListRow>> {
    sql_query("SELECT lists.namespace_id, lists.name, lists.description, lists.notes \
        FROM catalog_set_groups AS groups CROSS JOIN software_lists AS lists \
        ON lists.namespace_id = groups.set_group_id WHERE groups.snapshot_key = ? ORDER BY groups.list_order")
        .bind::<Text,_>(key.as_str()).load::<ListRow>(conn).map_err(Into::into)
}

fn load_items(conn: &mut SqliteConnection, key: &SnapshotKey) -> crate::Result<Vec<ItemRow>> {
    sql_query(format!("SELECT groups.set_group_id AS parent_id, items.source_order, sets.set_id AS record_id, \
        sets.set_name, lists.name AS list_name, items.clone_of, items.supported, items.supported_specified, \
        items.description, items.year, items.publisher, items.notes {SETS} \
        JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id \
        JOIN software_items AS items ON items.record_id = sets.set_id {SELECTED} ORDER BY items.source_order"))
        .bind::<Text,_>(key.as_str()).load::<ItemRow>(conn).map_err(Into::into)
}

pub(super) fn load_records(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    result: &mut CatalogRecords,
) -> crate::Result<()> {
    let items = load_items(conn, key)?;
    if items.is_empty() {
        return Ok(());
    }
    let mut facts = load_item_facts(conn, key, &items)?;
    result.software_list_contexts = ambiguous_list_contexts(conn, key, &items, &facts)?;
    for item in items {
        let name = serde_json::to_string(&(&item.list_name, &item.set_name))?;
        let value = facts.remove(&item.record_id).ok_or_else(|| {
            crate::Error::InvalidPath("software title has no native history facts".into())
        })?;
        result
            .software_item_lists
            .insert(item.record_id, item.parent_id);
        result.software_items.insert(item.record_id, vec![value]);
        result.sets.entry(name.clone()).or_default().push(SetRow {
            set_id: item.record_id,
            set_name: name,
            parent_name: item.clone_of,
            source_order: item.source_order,
        });
    }
    load_requirements(conn, key, result)?;
    Ok(())
}

fn load_item_facts(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    items: &[ItemRow],
) -> crate::Result<BTreeMap<i64, Value>> {
    let mut children = load_item_children(conn, key)?;
    let parts = load_parts(conn, key)?;
    for part in parts {
        push(
            &mut children,
            part.parent_id,
            part.source_order,
            "part",
            &part.value,
        )?;
    }
    let mut facts = BTreeMap::new();
    for item in items {
        let mut value = serde_json::to_value(item)?;
        value["children"] = ordered(children.remove(&item.record_id).unwrap_or_default());
        facts.insert(item.record_id, value);
    }
    Ok(facts)
}

// Repeated list names do not identify their native owners. Include their whole
// source-owned context once per parent, never once per title. Generated namespace
// IDs and list positions are lookup keys only, not comparison facts.
fn ambiguous_list_contexts(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    items: &[ItemRow],
    facts: &BTreeMap<i64, Value>,
) -> crate::Result<BTreeMap<i64, String>> {
    let lists = load_list_facts(conn, key, items, facts)?;
    let mut counts = BTreeMap::<&str, usize>::new();
    for (_, list) in &lists {
        if let Some(name) = list["name"].as_str() {
            *counts.entry(name).or_default() += 1;
        }
    }
    let mut result = BTreeMap::new();
    for (owner, list) in &lists {
        if list["name"]
            .as_str()
            .and_then(|name| counts.get(name))
            .copied()
            .unwrap_or_default()
            > 1
        {
            result.insert(*owner, serde_json::to_string(list)?);
        }
    }
    Ok(result)
}

// Intern exact canonical structural values across this one comparison. The
// ordinal is not a persisted identity, hash, source position or public key.
// Sorting references avoids copying or serializing a parent subtree per title.
pub(super) fn classify_parent_contexts(
    previous: &mut CatalogRecords,
    current: &mut CatalogRecords,
) {
    let contexts = previous
        .software_list_contexts
        .values()
        .chain(current.software_list_contexts.values())
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let classes = contexts
        .into_iter()
        .enumerate()
        .map(|(class, facts)| (facts, class))
        .collect::<BTreeMap<_, _>>();
    let [previous_classes, current_classes] = [&*previous, &*current].map(|records| {
        // A long exact-content comparison happens once per parent, not once
        // per child. Equal repeated-name lists otherwise still cost N² time.
        let parents = records
            .software_list_contexts
            .iter()
            .filter_map(|(parent, context)| {
                classes.get(context.as_str()).map(|class| (*parent, *class))
            })
            .collect::<BTreeMap<_, _>>();
        records
            .software_item_lists
            .iter()
            .filter_map(|(item, parent)| parents.get(parent).map(|class| (*item, *class)))
            .collect()
    });
    previous.software_context_classes = previous_classes;
    current.software_context_classes = current_classes;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
        database::Database,
        domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    };
    use camino::Utf8PathBuf;
    use std::fmt::Write;

    #[test]
    fn repeated_list_history_storage_grows_linearly_with_titles() -> crate::Result<()> {
        let database = Database::in_memory()?;
        let directory = tempfile::tempdir()?;
        let mut sizes = Vec::new();
        for count in [8, 64] {
            let mut titles = String::new();
            for index in 0..count {
                write!(titles, "<software name=\"game{index}\"><description>Game</description><year>2000</year><publisher>P</publisher></software>")
                    .map_err(|_| crate::Error::InvalidPath("cannot format test title".into()))?;
            }
            let list = format!("<softwarelist name=\"same\">{titles}</softwarelist>");
            let xml = format!("<softwarelists>{list}{list}</softwarelists>");
            let path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{count}.xml")))
                .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
            std::fs::write(&path, xml)?;
            let report = app::import_catalog(
                &database,
                &CatalogImportRequest {
                    document_path: path,
                    format: CatalogDocumentFormat::MameSoftwareListXml,
                    source_key: PublishingSourceKey::new("linear-history"),
                    source_display_name: "Linear history".into(),
                    catalog_key: CatalogKey::new("linear-history"),
                    catalog_display_name: "Linear history".into(),
                    scope: CatalogScope::Complete,
                },
            )?;
            assert_eq!(report.status, CatalogImportStatus::Succeeded);
            let mut connection = database.pool().get()?;
            let mut records = CatalogRecords::default();
            let snapshot = report.snapshot_key.ok_or_else(|| {
                crate::Error::InvalidPath("successful import has no snapshot".into())
            })?;
            load_records(&mut connection, &snapshot, &mut records)?;
            assert_eq!(records.software_items.len(), count * 2);
            classify_parent_contexts(&mut records, &mut CatalogRecords::default());
            assert_eq!(records.software_context_classes.len(), count * 2);
            let title_bytes = records
                .software_items
                .values()
                .map(|facts| serde_json::to_vec(facts).map(|bytes| bytes.len()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum::<usize>();
            sizes.push(
                title_bytes
                    + records
                        .software_list_contexts
                        .values()
                        .map(String::len)
                        .sum::<usize>(),
            );
        }
        assert!(
            sizes[1] <= sizes[0] * 10,
            "eight times the titles must not copy eight times as many parent subtrees per title: {sizes:?}"
        );
        Ok(())
    }

    #[test]
    fn parent_context_classes_compare_facts_not_generated_owner_keys() {
        let mut previous = CatalogRecords {
            software_list_contexts: BTreeMap::from([
                (10, "same facts".into()),
                (20, "different facts".into()),
            ]),
            software_item_lists: BTreeMap::from([(1, 10), (2, 20), (3, 10), (4, 30)]),
            ..CatalogRecords::default()
        };
        let mut current = CatalogRecords {
            software_list_contexts: BTreeMap::from([
                (100, "different facts".into()),
                (200, "same facts".into()),
            ]),
            software_item_lists: BTreeMap::from([(11, 200), (22, 100), (33, 200), (44, 300)]),
            ..CatalogRecords::default()
        };
        classify_parent_contexts(&mut previous, &mut current);
        assert_eq!(
            previous.software_context_classes.get(&1),
            current.software_context_classes.get(&11)
        );
        assert_eq!(
            previous.software_context_classes.get(&2),
            current.software_context_classes.get(&22)
        );
        assert_eq!(
            previous.software_context_classes.get(&1),
            previous.software_context_classes.get(&3)
        );
        assert_ne!(
            previous.software_context_classes.get(&1),
            previous.software_context_classes.get(&2)
        );
        assert!(!previous.software_context_classes.contains_key(&4));
        assert!(!current.software_context_classes.contains_key(&44));
    }
}

fn load_item_children(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<OrderedChildren> {
    let mut children = OrderedChildren::new();
    let positions = sql_query(format!("SELECT positions.record_id AS parent_id, positions.source_order, positions.field_kind \
        {SETS} CROSS JOIN software_item_text_positions AS positions ON positions.record_id = sets.set_id {SELECTED}"))
        .bind::<Text,_>(key.as_str()).load::<TextPositionRow>(conn)?;
    for position in positions {
        push(
            &mut children,
            position.parent_id,
            position.source_order,
            "text",
            &position,
        )?;
    }
    for (table, kind) in [
        ("software_item_info", "info"),
        ("software_item_shared_features", "sharedfeat"),
    ] {
        let rows = sql_query(format!("SELECT child.record_id AS parent_id, child.source_order, child.name, child.value \
            {SETS} CROSS JOIN {table} AS child ON child.record_id = sets.set_id {SELECTED} ORDER BY child.value_order"))
            .bind::<Text,_>(key.as_str()).load::<NamedValueRow>(conn)?;
        for row in rows {
            push(&mut children, row.parent_id, row.source_order, kind, &row)?;
        }
    }
    Ok(children)
}

struct OwnedPart {
    parent_id: i64,
    source_order: i64,
    value: Value,
}

fn load_parts(conn: &mut SqliteConnection, key: &SnapshotKey) -> crate::Result<Vec<OwnedPart>> {
    let parts = sql_query(format!("SELECT parts.record_id AS parent_id, parts.source_order, parts.part_id, parts.part_name, parts.interface \
        {SETS} {PARTS} {SELECTED} ORDER BY parts.part_order"))
        .bind::<Text,_>(key.as_str()).load::<PartRow>(conn)?;
    let mut children = load_part_children(conn, key)?;
    parts
        .into_iter()
        .map(|part| {
            let mut value = serde_json::to_value(&part)?;
            value["children"] = ordered(children.remove(&part.part_id).unwrap_or_default());
            Ok(OwnedPart {
                parent_id: part.parent_id,
                source_order: part.source_order,
                value,
            })
        })
        .collect()
}

fn load_part_children(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<OrderedChildren> {
    let mut children = OrderedChildren::new();
    let features = sql_query(format!("SELECT features.part_id AS parent_id, features.source_order, features.name, features.value \
        {SETS} {PARTS} CROSS JOIN software_part_features AS features ON features.part_id = parts.part_id {SELECTED} ORDER BY features.value_order"))
        .bind::<Text,_>(key.as_str()).load::<NamedValueRow>(conn)?;
    for feature in features {
        push(
            &mut children,
            feature.parent_id,
            feature.source_order,
            "feature",
            &feature,
        )?;
    }
    let areas = sql_query(format!("SELECT areas.part_id AS parent_id, COALESCE(data_area.source_order, disk_area.source_order) AS source_order, \
        areas.area_id, COALESCE(data_area.area_name, disk_area.area_name) AS area_name, areas.area_kind, \
        data_area.area_id AS data_area_id, disk_area.area_id AS disk_area_id, \
        data_area.declared_size_text, data_area.width, COALESCE(data_area.width_specified, 0) AS width_specified, \
        data_area.endianness, COALESCE(data_area.endianness_specified, 0) AS endianness_specified \
        {SETS} {PARTS} {AREAS} {AREA_DETAILS} {SELECTED} ORDER BY areas.area_order"))
        .bind::<Text,_>(key.as_str()).load::<AreaRow>(conn)?;
    let mut entries = load_entries(conn, key)?;
    for area in areas {
        if super::super::software_area::native_kind(
            area.area_id,
            area.data_area_id,
            area.disk_area_id,
        )
        .map(crate::mame_softwarelist::AreaKind::as_str)
            != Some(area.area_kind.as_str())
        {
            return Err(crate::Error::InvalidPath(format!(
                "software area {} lacks exactly one matching native detail",
                area.area_id
            )));
        }
        let mut value = serde_json::to_value(&area)?;
        value["entries"] = ordered(entries.remove(&area.area_id).unwrap_or_default());
        push(
            &mut children,
            area.parent_id,
            area.source_order,
            "area",
            &value,
        )?;
    }
    load_switches(conn, key, &mut children)?;
    Ok(children)
}

fn load_entries(conn: &mut SqliteConnection, key: &SnapshotKey) -> crate::Result<OrderedChildren> {
    let roms = sql_query(format!("SELECT rom.area_id AS parent_id, rom.source_order, rom.occurrence_id, rom.name, \
        rom.size_text, rom.offset_text, rom.crc_text, rom.sha1_text, rom.value, rom.dump_status, rom.status_specified, \
        rom.load_instruction, rom.evidence_scope, declaration.occurrence_id IS NOT NULL AS is_declaration, \
        file_use.operation, file_use.declaration_occurrence_id AS declaration_id \
        {SETS} {PARTS} {AREAS} CROSS JOIN software_rom_entries AS rom ON rom.area_id = areas.area_id \
        LEFT JOIN software_file_declarations AS declaration ON declaration.occurrence_id = rom.occurrence_id \
        JOIN software_file_uses AS file_use ON file_use.occurrence_id = rom.occurrence_id {SELECTED} ORDER BY rom.component_order"))
        .bind::<Text,_>(key.as_str()).load::<RomRow>(conn)?;
    let ordinals = roms
        .iter()
        .map(|rom| (rom.occurrence_id, (rom.parent_id, rom.source_order)))
        .collect::<BTreeMap<_, _>>();
    // A file-use reference is expressed by its owner's relative native rank,
    // never by a database-generated occurrence ID or a physical line number.
    let mut ranks = BTreeMap::<i64, BTreeMap<i64, usize>>::new();
    for rom in &roms {
        ranks
            .entry(rom.parent_id)
            .or_default()
            .insert(rom.source_order, 0);
    }
    for area in ranks.values_mut() {
        for (rank, value) in area.values_mut().enumerate() {
            *value = rank;
        }
    }
    let mut entries = OrderedChildren::new();
    for rom in roms {
        let mut value = serde_json::to_value(&rom)?;
        value["required_file_entry"] = match rom.declaration_id {
            None => Value::Null,
            Some(id) => {
                let (area, order) = ordinals.get(&id).ok_or_else(|| {
                    crate::Error::InvalidPath(
                        "software file use has no native declaration owner".into(),
                    )
                })?;
                if *area != rom.parent_id {
                    return Err(crate::Error::InvalidPath(
                        "software file use crosses areas".into(),
                    ));
                }
                json!(ranks.get(area).and_then(|ranks| ranks.get(order)))
            }
        };
        push(&mut entries, rom.parent_id, rom.source_order, "rom", &value)?;
    }
    let disks = sql_query(format!("SELECT disk.area_id AS parent_id, disk.source_order, disk.name, disk.sha1_text, \
        disk.dump_status, disk.status_specified, disk.writeable, disk.writeable_specified, disk.evidence_scope \
        {SETS} {PARTS} {AREAS} CROSS JOIN software_disk_entries AS disk ON disk.area_id = areas.area_id {SELECTED} ORDER BY disk.component_order"))
        .bind::<Text,_>(key.as_str()).load::<DiskRow>(conn)?;
    for disk in disks {
        push(
            &mut entries,
            disk.parent_id,
            disk.source_order,
            "disk",
            &disk,
        )?;
    }
    Ok(entries)
}

fn load_switches(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    children: &mut OrderedChildren,
) -> crate::Result<()> {
    let switches = sql_query(format!("SELECT switches.part_id AS parent_id, switches.source_order, switches.dipswitch_order, switches.name, switches.tag, switches.mask \
        {SETS} {PARTS} CROSS JOIN software_part_dipswitches AS switches ON switches.part_id = parts.part_id {SELECTED} ORDER BY switches.dipswitch_order"))
        .bind::<Text,_>(key.as_str()).load::<SwitchRow>(conn)?;
    let values = sql_query(format!("SELECT values_row.part_id AS parent_id, values_row.source_order, values_row.dipswitch_order, \
        values_row.name, values_row.value, values_row.is_default, values_row.default_specified \
        {SETS} {PARTS} CROSS JOIN software_part_dip_values AS values_row ON values_row.part_id = parts.part_id {SELECTED} ORDER BY values_row.value_order"))
        .bind::<Text,_>(key.as_str()).load::<SwitchValueRow>(conn)?;
    let mut by_switch = BTreeMap::<(i64, i64), Vec<(i64, Value)>>::new();
    for value in values {
        by_switch
            .entry((value.parent_id, value.dipswitch_order))
            .or_default()
            .push((value.source_order, serde_json::to_value(&value)?));
    }
    for switch in switches {
        let mut value = serde_json::to_value(&switch)?;
        value["values"] = ordered(
            by_switch
                .remove(&(switch.parent_id, switch.dipswitch_order))
                .unwrap_or_default(),
        );
        push(
            children,
            switch.parent_id,
            switch.source_order,
            "dipswitch",
            &value,
        )?;
    }
    Ok(())
}

fn push<T: Serialize>(
    children: &mut OrderedChildren,
    owner: i64,
    source_order: i64,
    kind: &str,
    value: &T,
) -> crate::Result<()> {
    children.entry(owner).or_default().push((
        source_order,
        json!({"kind": kind, "facts": serde_json::to_value(value)?}),
    ));
    Ok(())
}

#[derive(QueryableByName)]
struct RequiredFileRow {
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = Text)]
    part_name: String,
    #[diesel(sql_type = Text)]
    area_name: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
}

fn load_requirements(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    result: &mut CatalogRecords,
) -> crate::Result<()> {
    for (table, role, crc, declaration) in [
        (
            "software_rom_entries",
            "software_rom",
            "entry.crc_text",
            "JOIN software_file_declarations AS declaration ON declaration.occurrence_id = entry.occurrence_id",
        ),
        ("software_disk_entries", "software_disk", "NULL", ""),
    ] {
        let rows = sql_query(format!(
            "SELECT sets.set_id AS record_id, parts.part_name, COALESCE(data_area.area_name, disk_area.area_name) AS area_name, entry.name, \
            '{role}' AS role, entry.evidence_scope, {crc} AS crc_text, entry.sha1_text \
            {SETS} {PARTS} {AREAS} {AREA_DETAILS} CROSS JOIN {table} AS entry ON entry.area_id = areas.area_id \
            {declaration} {SELECTED} AND entry.name IS NOT NULL ORDER BY entry.component_order"
        ))
        .bind::<Text, _>(key.as_str())
        .load::<RequiredFileRow>(conn)?;
        for row in rows {
            let name = serde_json::to_string(&(&row.part_name, &row.area_name, &row.name))?;
            result
                .requirements
                .entry(row.record_id)
                .or_default()
                .entry(name)
                .or_default()
                .push(json!({
                    "role": row.role, "size": null,
                    "crc": usable_hash::<4>(row.crc_text.as_deref()),
                    "sha1": usable_hash::<20>(row.sha1_text.as_deref()),
                    "evidence_scope": row.evidence_scope, "evidence_provenance": "source_declared",
                }));
        }
    }
    for rows in result.requirements.values_mut() {
        for values in rows.values_mut() {
            values.sort_by_key(Value::to_string);
        }
    }
    Ok(())
}

fn usable_hash<const N: usize>(text: Option<&str>) -> Option<String> {
    let text = text?;
    if text.len() != N * 2 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(text.to_ascii_lowercase())
}

fn ordered(mut children: Vec<(i64, Value)>) -> Value {
    children.sort_by_key(|(source_order, _)| *source_order);
    Value::Array(children.into_iter().map(|(_, value)| value).collect())
}
