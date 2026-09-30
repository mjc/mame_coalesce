use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{Bool, Nullable, Text},
};

use crate::{
    domain::{SetName, SnapshotKey},
    machine_dependencies::{
        MachineDependency, MachineDependencyCatalog, MachineDependencyKind, MachineSet,
        SnapshotCompleteness,
    },
};

use super::db::Pool;

#[derive(QueryableByName)]
struct SetRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    parent_name: Option<String>,
    #[diesel(sql_type = Text)]
    metadata_json: String,
    #[diesel(sql_type = Nullable<Bool>)]
    is_bios: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    is_device: Option<bool>,
}

#[derive(QueryableByName)]
struct MameDependencyRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    dependency_kind: String,
    #[diesel(sql_type = Text)]
    target_name: String,
}

/// Load normalized set relationships from exactly one immutable catalog snapshot.
pub fn load_catalog(
    pool: &Pool,
    snapshot: &SnapshotKey,
) -> crate::Result<MachineDependencyCatalog> {
    let mut conn = pool.get()?;
    let exists = sql_query(
        "SELECT s.scope_kind, s.scope_json, pi.format AS parser_format \
         FROM catalog_snapshots s \
         JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         WHERE s.snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SnapshotExists>(&mut conn);
    if matches!(&exists, Err(diesel::result::Error::NotFound)) {
        return Err(crate::Error::InvalidPath(format!(
            "catalog snapshot {} does not exist",
            snapshot.as_str()
        )));
    }
    let header = exists?;
    if !matches!(header.parser_format.as_str(), "logiqx" | "mame-listxml") {
        return Err(crate::Error::UnsupportedMachineDependencyFormat(
            header.parser_format,
        ));
    }
    let completeness = match header.scope_kind.as_str() {
        "complete" => SnapshotCompleteness::Complete,
        "filtered" => {
            SnapshotCompleteness::Filtered(filtered_set_scope(header.scope_json.as_deref()))
        }
        "partial" => SnapshotCompleteness::Partial,
        _ => SnapshotCompleteness::Unknown,
    };

    let is_mame = header.parser_format == "mame-listxml";
    let rows = sql_query(
        "SELECT s.set_name, s.parent_name, s.metadata_json, mf.is_bios, mf.is_device \
         FROM snapshot_sets AS s LEFT JOIN mame_machine_facts AS mf \
           ON mf.snapshot_key = s.snapshot_key AND mf.set_name = s.set_name \
         WHERE s.snapshot_key = ? ORDER BY s.set_name",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SetRow>(&mut conn)?;

    let mame_dependencies = load_mame_dependencies(&mut conn, snapshot, is_mame)?;

    let sets = rows
        .into_iter()
        .map(|row| machine_set_from_row(row, is_mame, &mame_dependencies))
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(MachineDependencyCatalog::with_completeness(
        snapshot.clone(),
        completeness,
        sets,
    ))
}

fn load_mame_dependencies(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
    is_mame: bool,
) -> crate::Result<BTreeMap<String, Vec<MameDependencyRow>>> {
    if !is_mame {
        return Ok(BTreeMap::new());
    }
    Ok(sql_query(
        "SELECT set_name, dependency_kind, target_name FROM mame_machine_dependencies \
         WHERE snapshot_key = ? ORDER BY set_name, dependency_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<MameDependencyRow>(conn)?
    .into_iter()
    .fold(
        BTreeMap::<String, Vec<MameDependencyRow>>::new(),
        |mut by_set, row| {
            by_set.entry(row.set_name.clone()).or_default().push(row);
            by_set
        },
    ))
}

fn machine_set_from_row(
    row: SetRow,
    is_mame: bool,
    mame_dependencies: &BTreeMap<String, Vec<MameDependencyRow>>,
) -> crate::Result<MachineSet> {
    let metadata: serde_json::Value = serde_json::from_str(&row.metadata_json)?;
    let mut set = MachineSet::new(row.set_name);
    set.parent_clone = row.parent_name.map(SetName::new);
    set.is_bios = row
        .is_bios
        .unwrap_or_else(|| flag(&metadata, &["is_bios", "isbios"]));
    set.is_device = row
        .is_device
        .unwrap_or_else(|| flag(&metadata, &["isdevice", "is_device"]));

    if is_mame {
        for dependency in mame_dependencies
            .get(set.name.as_str())
            .into_iter()
            .flatten()
        {
            match dependency.dependency_kind.as_str() {
                "romof" => set.dependencies.push(MachineDependency {
                    kind: MachineDependencyKind::RomOf,
                    target: SetName::new(&dependency.target_name),
                }),
                "device_ref" => set.dependencies.push(MachineDependency {
                    kind: MachineDependencyKind::DeviceReference,
                    target: SetName::new(&dependency.target_name),
                }),
                "sampleof" => set
                    .unsupported_relationships
                    .push(("sampleof".to_owned(), SetName::new(&dependency.target_name))),
                _ => unreachable!("MAME dependency kind is schema constrained"),
            }
        }
    } else {
        if let Some(target) = string(&metadata, &["romof", "rom_of"]) {
            set.dependencies.push(MachineDependency {
                kind: MachineDependencyKind::RomOf,
                target: SetName::new(target),
            });
        }
        for target in device_references(&metadata) {
            set.dependencies.push(MachineDependency {
                kind: MachineDependencyKind::DeviceReference,
                target: SetName::new(target),
            });
        }
        if let Some(target) = string(&metadata, &["sampleof", "sample_of"]) {
            set.unsupported_relationships
                .push(("sampleof".to_owned(), SetName::new(target)));
        }
    }
    Ok(set)
}

#[derive(QueryableByName)]
struct SnapshotExists {
    #[diesel(sql_type = Text)]
    scope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    scope_json: Option<String>,
    #[diesel(sql_type = Text)]
    parser_format: String,
}

fn filtered_set_scope(scope_json: Option<&str>) -> Option<BTreeSet<SetName>> {
    let details = serde_json::from_str::<serde_json::Value>(scope_json?).ok()?;
    details
        .get("sets")?
        .as_array()?
        .iter()
        .map(|name| name.as_str().map(SetName::new))
        .collect()
}

fn flag(metadata: &serde_json::Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        metadata.get(*key).is_some_and(|value| {
            value.as_bool() == Some(true)
                || value.as_str().is_some_and(|text| {
                    matches!(text.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
                })
        })
    })
}

fn string<'a>(metadata: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(serde_json::Value::as_str))
}

fn device_references(metadata: &serde_json::Value) -> impl Iterator<Item = &str> {
    metadata
        .get("device_refs")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reference| {
            reference
                .as_str()
                .or_else(|| reference.get("name").and_then(serde_json::Value::as_str))
        })
}
