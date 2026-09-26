use std::collections::BTreeSet;

use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{Nullable, Text},
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
}

/// Load normalized set relationships from exactly one immutable catalog snapshot.
pub fn load_catalog(
    pool: &Pool,
    snapshot: &SnapshotKey,
) -> crate::Result<MachineDependencyCatalog> {
    let mut conn = pool.get()?;
    let exists =
        sql_query("SELECT scope_kind, scope_json FROM catalog_snapshots WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<SnapshotExists>(&mut conn);
    if matches!(&exists, Err(diesel::result::Error::NotFound)) {
        return Err(crate::Error::InvalidPath(format!(
            "catalog snapshot {} does not exist",
            snapshot.as_str()
        )));
    }
    let header = exists?;
    let completeness = match header.scope_kind.as_str() {
        "complete" => SnapshotCompleteness::Complete,
        "filtered" => {
            SnapshotCompleteness::Filtered(filtered_set_scope(header.scope_json.as_deref()))
        }
        "partial" => SnapshotCompleteness::Partial,
        _ => SnapshotCompleteness::Unknown,
    };

    let rows = sql_query(
        "SELECT set_name, parent_name, metadata_json FROM snapshot_sets \
         WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SetRow>(&mut conn)?;

    let sets = rows
        .into_iter()
        .map(|row| {
            let metadata: serde_json::Value = serde_json::from_str(&row.metadata_json)?;
            let mut set = MachineSet::new(row.set_name);
            set.parent_clone = row.parent_name.map(SetName::new);
            set.is_bios = flag(&metadata, &["is_bios", "isbios"]);
            set.is_device = flag(&metadata, &["isdevice", "is_device"]);

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
            Ok(set)
        })
        .collect::<crate::Result<Vec<_>>>()?;

    Ok(MachineDependencyCatalog::with_completeness(
        snapshot.clone(),
        completeness,
        sets,
    ))
}

#[derive(QueryableByName)]
struct SnapshotExists {
    #[diesel(sql_type = Text)]
    scope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    scope_json: Option<String>,
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
