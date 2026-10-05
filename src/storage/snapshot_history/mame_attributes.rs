//! Attribute order is metadata, not an additional file requirement.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    domain::SnapshotKey,
    mame,
    storage::mame_attributes::{Family, Positions},
};

const FAMILIES: [Family; 32] = [
    Family::Machine,
    Family::Bios,
    Family::Rom,
    Family::Disk,
    Family::DeviceReference,
    Family::Sample,
    Family::Chip,
    Family::Display,
    Family::Sound,
    Family::Input,
    Family::Control,
    Family::Switch,
    Family::SwitchLocation,
    Family::SwitchValue,
    Family::SwitchCondition,
    Family::SwitchValueCondition,
    Family::AdjusterCondition,
    Family::Port,
    Family::Analog,
    Family::Adjuster,
    Family::Driver,
    Family::Feature,
    Family::Device,
    Family::Instance,
    Family::Extension,
    Family::Slot,
    Family::SlotOption,
    Family::SoftwareList,
    Family::RamOption,
    Family::MachineCompatibility,
    Family::RomCompatibility,
    Family::DiskCompatibility,
];

#[derive(QueryableByName)]
struct DocumentOwner {
    #[diesel(sql_type = BigInt)]
    document_id: i64,
    #[diesel(sql_type = Bool)]
    build_present: bool,
    #[diesel(sql_type = Bool)]
    debug_specified: bool,
}

pub(super) fn document(conn: &mut SqliteConnection, key: &SnapshotKey) -> crate::Result<Vec<i64>> {
    let Some(owner) = sql_query("SELECT document_id, build IS NOT NULL AS build_present, debug_specified FROM mame_document_facts WHERE snapshot_key=?")
        .bind::<Text,_>(key.as_str()).get_result::<DocumentOwner>(conn).optional()? else {
        return Ok(Vec::new());
    };
    let mut positions = Positions::default();
    positions.load(
        conn,
        Family::Document,
        &format!(
            "FROM __NATIVE_POSITIONS__ AS position WHERE position.document_id={}",
            owner.document_id
        ),
    )?;
    let attributes = positions.take(
        Family::Document,
        [owner.document_id, 0, 0, 0],
        mame::MameDocumentAttribute::from_code,
        &[owner.build_present, owner.debug_specified, true],
    )?;
    positions.finish()?;
    Ok(attributes
        .into_iter()
        .map(|position| position.field.code())
        .collect())
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    owner_a: i64,
    #[diesel(sql_type = BigInt)]
    owner_b: i64,
    #[diesel(sql_type = BigInt)]
    owner_c: i64,
    #[diesel(sql_type = BigInt)]
    owner_d: i64,
    #[diesel(sql_type = BigInt)]
    semantic_order: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    media_name: Option<String>,
}

#[derive(QueryableByName)]
struct OwnerPresence {
    #[diesel(sql_type = Bool)]
    has_owners: bool,
}

struct OwnerOrder {
    order: [i64; 3],
    tokens: Vec<(i64, bool, i64)>,
    media_name: Option<String>,
}

type AttributeOrder = Vec<(bool, i64)>;
pub(super) type MediaOrders = BTreeMap<(Family, String), Vec<AttributeOrder>>;

#[derive(Default)]
pub(super) struct History {
    pub(super) machines: BTreeMap<i64, Vec<serde_json::Value>>,
    pub(super) media: BTreeMap<i64, MediaOrders>,
}

#[cfg(test)]
mod tests;

pub(super) fn records(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<History> {
    let has_owners = sql_query(
        "SELECT EXISTS (SELECT 1 FROM catalog_set_groups AS groups \
         JOIN catalog_sets AS sets USING (set_group_id) \
         WHERE groups.snapshot_key=? AND groups.kind='root' \
         AND sets.source_element_kind='mame_machine') AS has_owners",
    )
    .bind::<Text, _>(key.as_str())
    .get_result::<OwnerPresence>(conn)?
    .has_owners;
    if !has_owners {
        return Ok(History::default());
    }
    let mut owners = BTreeMap::<(i64, Family, [i64; 4]), OwnerOrder>::new();
    // Each family statement rematerializes its bounded owner CTE; this avoids
    // temp-schema churn but repeats owner selection instead of reusing scratch state.
    for family in FAMILIES {
        for row in sql_query(query(family))
            .bind::<Text, _>(key.as_str())
            .load::<Row>(conn)?
        {
            let field = field_code(family, row.field_kind).ok_or_else(invalid)?;
            let semantic_order = if family.keys().contains(&"element_order") {
                child_ranks
                    .get(&row.set_id)
                    .and_then(|ranks| ranks.get(&row.semantic_order))
                    .copied()
                    .ok_or_else(invalid)?
            } else {
                row.semantic_order
            };
            let identity = (
                row.set_id,
                native_family(family),
                [row.owner_a, row.owner_b, row.owner_c, row.owner_d],
            );
            let owner = owners.entry(identity).or_insert_with(|| OwnerOrder {
                order: [semantic_order, row.owner_c, row.owner_d],
                tokens: Vec::new(),
                media_name: row.media_name.clone(),
            });
            if owner.order != [semantic_order, row.owner_c, row.owner_d]
                || owner.media_name != row.media_name
            {
                return Err(invalid());
            }
            owner
                .tokens
                .push((row.source_order, family != native_family(family), field));
        }
    }
    let mut result = History::default();
    let mut media_orders = BTreeMap::<_, BTreeMap<i64, AttributeOrder>>::new();
    for ((set_id, family, _), mut owner) in owners {
        owner.tokens.sort_unstable();
        let mut ordinals = BTreeSet::new();
        let mut tokens = Vec::with_capacity(owner.tokens.len());
        for (ordinal, compatibility, field) in owner.tokens {
            if ordinal < 0 || !ordinals.insert(ordinal) {
                return Err(invalid());
            }
            tokens.push((compatibility, field));
        }
        if let Some(name) = owner.media_name {
            if media_orders
                .entry((set_id, family, name))
                .or_default()
                .insert(owner.order[0], tokens)
                .is_some()
            {
                return Err(invalid());
            }
        } else {
            result
                .machines
                .entry(set_id)
                .or_default()
                .push(serde_json::json!({
                    "family": family.code(), "owner": owner.order, "attributes": tokens,
                }));
        }
    }
    super::sort_json_groups(&mut result.machines);
    for ((set_id, family, name), orders) in media_orders {
        result
            .media
            .entry(set_id)
            .or_default()
            .insert((family, name), orders.into_values().collect());
    }
    Ok(result)
}

/// Added or removed files are requirements, not machine metadata edits.
pub(super) fn media_changed(before: Option<&MediaOrders>, after: Option<&MediaOrders>) -> bool {
    let (Some(before), Some(after)) = (before, after) else {
        return false;
    };
    before.iter().any(|(owner, orders)| {
        after
            .get(owner)
            .is_some_and(|current| orders.len() == current.len() && orders != current)
    })
}

pub(super) fn media_signature(media: Option<&MediaOrders>) -> Vec<serde_json::Value> {
    media.into_iter().flat_map(BTreeMap::iter).map(|((family, name), orders)| {
        serde_json::json!({"family": family.code(), "name": name, "attributes": orders})
    }).collect()
}

fn query(family: Family) -> String {
    let keys = family.keys();
    let columns = keys
        .iter()
        .map(|key| format!("position.{key}"))
        .chain(std::iter::repeat_n("0".to_owned(), 4 - keys.len()))
        .zip(["owner_a", "owner_b", "owner_c", "owner_d"])
        .map(|(column, alias)| format!("{column} AS {alias}"))
        .collect::<Vec<_>>()
        .join(",");
    let media_table = match native_family(family) {
        Family::Rom => Some("mame_rom_claims"),
        Family::Disk => Some("mame_disk_claims"),
        Family::Sample => Some("mame_samples"),
        _ => None,
    };
    let (owner, semantic_order, media_name, scope) = media_table.map_or_else(|| {
        let semantic_order = keys
            .get(1)
            .map_or_else(|| "0".to_owned(), |key| format!("position.{key}"));
        (
            "position.set_id",
            semantic_order,
            "NULL",
            format!(
                "FROM requested_owners AS requested CROSS JOIN {} AS position WHERE position.set_id=requested.owner_id",
                family.table()
            ),
        )
    }, |table| {
        (
            "occurrence.record_id",
            "occurrence.occurrence_order".to_owned(),
            "native.name",
            format!(
                "FROM requested_owners AS requested CROSS JOIN asset_occurrences AS occurrence CROSS JOIN {table} AS native CROSS JOIN {} AS position WHERE occurrence.record_id=requested.owner_id AND native.occurrence_id=occurrence.occurrence_id AND position.occurrence_id=occurrence.occurrence_id",
                family.table()
            ),
        )
    });
    format!(
        "WITH requested_owners(owner_id) AS MATERIALIZED ( \
         SELECT sets.set_id FROM catalog_set_groups AS groups \
         JOIN catalog_sets AS sets USING (set_group_id) \
         WHERE groups.snapshot_key=? AND groups.kind='root' \
         AND sets.source_element_kind='mame_machine' \
         ) SELECT {owner} AS set_id,{columns},{semantic_order} AS semantic_order,position.field_kind,position.source_order,{media_name} AS media_name {scope} ORDER BY owner_a,owner_b,owner_c,owner_d,position.source_order"
    )
}

const fn native_family(family: Family) -> Family {
    match family {
        Family::MachineCompatibility => Family::Machine,
        Family::RomCompatibility => Family::Rom,
        Family::DiskCompatibility => Family::Disk,
        other => other,
    }
}

fn field_code(family: Family, code: i64) -> Option<i64> {
    macro_rules! decode {
        ($field:ident) => {
            mame::$field::from_code(code).map(mame::$field::code)
        };
    }
    match family {
        Family::Document => decode!(MameDocumentAttribute),
        Family::Machine => decode!(MameMachineAttribute),
        Family::MachineCompatibility => decode!(MameMachineCompatibilityAttribute),
        Family::Bios => decode!(MameBiosAttribute),
        Family::Rom => decode!(MameRomAttribute),
        Family::RomCompatibility => decode!(MameRomCompatibilityAttribute),
        Family::Disk => decode!(MameDiskAttribute),
        Family::DiskCompatibility => decode!(MameDiskCompatibilityAttribute),
        Family::DeviceReference => decode!(MameDeviceReferenceAttribute),
        Family::Sample => decode!(MameSampleAttribute),
        Family::Chip => decode!(MameChipAttribute),
        Family::Display => decode!(MameDisplayAttribute),
        Family::Sound => decode!(MameSoundAttribute),
        Family::Input => decode!(MameInputAttribute),
        Family::Control => decode!(MameControlAttribute),
        Family::Switch => decode!(MameSwitchAttribute),
        Family::SwitchLocation => decode!(MameSwitchLocationAttribute),
        Family::SwitchValue => decode!(MameSwitchValueAttribute),
        Family::SwitchCondition | Family::SwitchValueCondition | Family::AdjusterCondition => {
            decode!(MameConditionAttribute)
        }
        Family::Port => decode!(MamePortAttribute),
        Family::Analog => decode!(MameAnalogAttribute),
        Family::Adjuster => decode!(MameAdjusterAttribute),
        Family::Driver => decode!(MameDriverAttribute),
        Family::Feature => decode!(MameFeatureAttribute),
        Family::Device => decode!(MameDeviceAttribute),
        Family::Instance => decode!(MameInstanceAttribute),
        Family::Extension => decode!(MameExtensionAttribute),
        Family::Slot => decode!(MameSlotAttribute),
        Family::SlotOption => decode!(MameSlotOptionAttribute),
        Family::SoftwareList => decode!(MameSoftwareListAttribute),
        Family::RamOption => decode!(MameRamOptionAttribute),
    }
}

fn invalid() -> crate::Error {
    crate::Error::XmlValidation("invalid native MAME attribute history witnesses".into())
}
