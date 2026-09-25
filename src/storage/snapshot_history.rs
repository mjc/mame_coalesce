use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::domain::{
    CatalogKey, CatalogRecordKind, CatalogScope, CatalogSnapshotDiff, CatalogSnapshotEntry,
    RelationshipEndpoint, RelationshipExplanation, SnapshotKey, SnapshotRecordDiff,
    SnapshotRecordStatus, SnapshotRequirementChange,
};

use super::db::Pool;

#[derive(QueryableByName)]
struct SnapshotRow {
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    scope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    scope_json: Option<String>,
}

#[derive(QueryableByName)]
struct HistoryRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = Text)]
    scope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    scope_json: Option<String>,
}

#[derive(QueryableByName)]
struct SetRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    parent_name: Option<String>,
    #[diesel(sql_type = Text)]
    metadata_json: String,
}

#[derive(QueryableByName)]
struct RequirementRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    asset_name: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    md5: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Text)]
    metadata_json: String,
}

#[derive(QueryableByName)]
struct ExtensionRow {
    #[diesel(sql_type = Text)]
    record_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    record_name: Option<String>,
    #[diesel(sql_type = Text)]
    field_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    namespace_uri: Option<String>,
    #[diesel(sql_type = Text)]
    raw_value_json: String,
}

#[derive(Default)]
struct CatalogRecords {
    sets: BTreeMap<String, SetRow>,
    requirements: BTreeMap<String, BTreeMap<String, Vec<serde_json::Value>>>,
    extensions: BTreeMap<(String, String), Vec<serde_json::Value>>,
}

pub fn diff(
    pool: &Pool,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<CatalogSnapshotDiff> {
    let mut conn = pool.get()?;
    let previous_header = snapshot(&mut conn, previous)?;
    let current_header = snapshot(&mut conn, current)?;
    if previous_header.catalog_key != current_header.catalog_key {
        return Err(crate::Error::InvalidPath(
            "snapshot history can only compare snapshots from the same catalog".to_owned(),
        ));
    }

    let same_scope = comparable_scope(&previous_header, &current_header);
    let previous_records = records(&mut conn, previous)?;
    let current_records = records(&mut conn, current)?;
    let explanations = super::relationships::explain_all(pool)?;
    let mut names = BTreeSet::new();
    names.extend(previous_records.sets.keys().cloned());
    names.extend(current_records.sets.keys().cloned());

    let records = names
        .into_iter()
        .map(|name| {
            let before = previous_records.sets.get(&name);
            let after = current_records.sets.get(&name);
            let relationship_evidence = explanations
                .iter()
                .filter(|explanation| touches_set(explanation, previous, current, &name))
                .cloned()
                .collect();
            let (status, metadata_changed, regrouped, requirement_changes) = match (before, after) {
                (None, None) => unreachable!("name came from one of the set maps"),
                (None, Some(_)) if scopes_cover_set(&previous_header, &current_header, &name) => (
                    SnapshotRecordStatus::AddedWithinScope,
                    false,
                    false,
                    Vec::new(),
                ),
                (Some(_), None) if scopes_cover_set(&previous_header, &current_header, &name) => (
                    SnapshotRecordStatus::RemovedWithinScope,
                    false,
                    false,
                    Vec::new(),
                ),
                (None, _) | (_, None) => (
                    absence_status(&previous_header, &current_header, &name),
                    false,
                    false,
                    Vec::new(),
                ),
                (Some(before), Some(after)) => {
                    let metadata_changed = set_metadata(&previous_records, &name, before)
                        != set_metadata(&current_records, &name, after);
                    let regrouped = before.parent_name != after.parent_name;
                    let requirement_changes = requirement_changes(
                        previous_records.requirements.get(&name),
                        current_records.requirements.get(&name),
                    );
                    let status = if metadata_changed || regrouped || !requirement_changes.is_empty()
                    {
                        SnapshotRecordStatus::Changed
                    } else {
                        SnapshotRecordStatus::Unchanged
                    };
                    (status, metadata_changed, regrouped, requirement_changes)
                }
            };
            SnapshotRecordDiff {
                set_name: name,
                status,
                metadata_changed,
                regrouped,
                requirement_changes,
                relationship_evidence,
            }
        })
        .collect();

    Ok(CatalogSnapshotDiff {
        previous: previous.clone(),
        current: current.clone(),
        same_scope,
        records,
    })
}

pub fn history(pool: &Pool, catalog: &CatalogKey) -> crate::Result<Vec<CatalogSnapshotEntry>> {
    let mut conn = pool.get()?;
    let rows = sql_query(
        "SELECT snapshot_key, document_key, declared_version, scope_kind, scope_json \
         FROM catalog_snapshots WHERE catalog_key = ? \
         ORDER BY document_key, interpretation_key, snapshot_key",
    )
    .bind::<Text, _>(catalog.as_str())
    .load::<HistoryRow>(&mut conn)?;
    rows.into_iter()
        .map(|row| {
            let scope = match row.scope_kind.as_str() {
                "unknown" => CatalogScope::Unknown,
                "complete" => CatalogScope::Complete,
                "filtered" => CatalogScope::Filtered(parse_scope(row.scope_json)?),
                "partial" => CatalogScope::Partial(parse_scope(row.scope_json)?),
                kind => {
                    return Err(crate::Error::InvalidPath(format!(
                        "invalid persisted catalog scope: {kind}"
                    )));
                }
            };
            Ok(CatalogSnapshotEntry {
                snapshot: SnapshotKey::from_persisted(row.snapshot_key),
                document_key: row.document_key,
                declared_version: row.declared_version,
                scope,
            })
        })
        .collect()
}

fn parse_scope(value: Option<String>) -> crate::Result<serde_json::Value> {
    value
        .ok_or_else(|| crate::Error::InvalidPath("snapshot scope details are missing".to_owned()))
        .and_then(|json| serde_json::from_str(&json).map_err(Into::into))
}

fn snapshot(conn: &mut diesel::SqliteConnection, key: &SnapshotKey) -> crate::Result<SnapshotRow> {
    sql_query(
        "SELECT catalog_key, scope_kind, scope_json FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .get_result(conn)
    .map_err(|error| match error {
        diesel::result::Error::NotFound => {
            crate::Error::InvalidPath(format!("catalog snapshot {} does not exist", key.as_str()))
        }
        error => error.into(),
    })
}

fn comparable_scope(previous: &SnapshotRow, current: &SnapshotRow) -> bool {
    if previous.scope_kind == "complete" && current.scope_kind == "complete" {
        return true;
    }
    previous.scope_kind == "filtered"
        && current.scope_kind == "filtered"
        && scope_policy(previous).is_some()
        && scope_set_names(previous) == scope_set_names(current)
        && scope_policy(previous) == scope_policy(current)
}

fn absence_status(
    previous: &SnapshotRow,
    current: &SnapshotRow,
    name: &str,
) -> SnapshotRecordStatus {
    let explicitly_excluded = [previous, current].into_iter().any(|snapshot| {
        snapshot.scope_kind == "filtered"
            && scope_set_names(snapshot).is_some_and(|names| !names.contains(name))
    });
    if explicitly_excluded {
        SnapshotRecordStatus::OutOfScope
    } else {
        SnapshotRecordStatus::Unknown
    }
}

fn scopes_cover_set(previous: &SnapshotRow, current: &SnapshotRow, name: &str) -> bool {
    snapshot_covers_set(previous, name)
        && snapshot_covers_set(current, name)
        && (previous.scope_kind != "filtered"
            || current.scope_kind != "filtered"
            || scope_policy(previous) == scope_policy(current))
}

fn snapshot_covers_set(snapshot: &SnapshotRow, name: &str) -> bool {
    match snapshot.scope_kind.as_str() {
        "complete" => true,
        "filtered" => scope_set_names(snapshot).is_some_and(|names| names.contains(name)),
        _ => false,
    }
}

fn scope_policy(snapshot: &SnapshotRow) -> Option<serde_json::Value> {
    let mut details =
        serde_json::from_str::<serde_json::Value>(snapshot.scope_json.as_deref()?).ok()?;
    details.as_object_mut()?.remove("sets");
    Some(details)
}

fn scope_set_names(snapshot: &SnapshotRow) -> Option<BTreeSet<String>> {
    serde_json::from_str::<serde_json::Value>(snapshot.scope_json.as_deref()?)
        .ok()?
        .get("sets")?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn records(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<CatalogRecords> {
    let sets = sql_query(
        "SELECT set_name, parent_name, metadata_json FROM snapshot_sets \
         WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(key.as_str())
    .load::<SetRow>(conn)?;
    let requirements = sql_query(
        "SELECT set_name, asset_name, role, size, crc, md5, sha1, evidence_scope, \
         evidence_provenance, merge_name, dump_status, serial, date, metadata_json \
         FROM asset_requirements WHERE snapshot_key = ? ORDER BY set_name, asset_name, component_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<RequirementRow>(conn)?;
    let extensions = sql_query(
        "SELECT record_kind, record_name, field_name, namespace_uri, raw_value_json \
         FROM snapshot_extensions WHERE snapshot_key = ? \
         ORDER BY record_kind, record_name, field_name, namespace_uri, raw_value_json",
    )
    .bind::<Text, _>(key.as_str())
    .load::<ExtensionRow>(conn)?;

    let mut result = CatalogRecords::default();
    for set in sets {
        result.sets.insert(set.set_name.clone(), set);
    }
    for extension in extensions {
        if let Some(record_name) = extension.record_name {
            result
                .extensions
                .entry((extension.record_kind, record_name))
                .or_default()
                .push(serde_json::json!({
                    "field": extension.field_name,
                    "namespace": extension.namespace_uri,
                    "value": json(&extension.raw_value_json),
                }));
        }
    }
    for extensions in result.extensions.values_mut() {
        extensions.sort_by_key(serde_json::Value::to_string);
    }
    for row in requirements {
        let source_extensions = result
            .extensions
            .get(&(row.role.clone(), row.asset_name.clone()))
            .cloned()
            .unwrap_or_default();
        let value = serde_json::json!({
            "role": row.role,
            "size": row.size,
            "crc": row.crc.map(hex::encode),
            "md5": row.md5.map(hex::encode),
            "sha1": row.sha1.map(hex::encode),
            "evidence_scope": row.evidence_scope,
            "evidence_provenance": row.evidence_provenance,
            "merge_name": row.merge_name,
            "dump_status": row.dump_status,
            "serial": row.serial,
            "date": row.date,
            "metadata": json(&row.metadata_json),
            "extensions": source_extensions,
        });
        result
            .requirements
            .entry(row.set_name)
            .or_default()
            .entry(row.asset_name)
            .or_default()
            .push(value);
    }
    for assets in result.requirements.values_mut() {
        for evidence in assets.values_mut() {
            evidence.sort_by_key(serde_json::Value::to_string);
        }
    }
    Ok(result)
}

fn requirement_changes(
    previous: Option<&BTreeMap<String, Vec<serde_json::Value>>>,
    current: Option<&BTreeMap<String, Vec<serde_json::Value>>>,
) -> Vec<SnapshotRequirementChange> {
    let mut names = BTreeSet::new();
    if let Some(previous) = previous {
        names.extend(previous.keys().cloned());
    }
    if let Some(current) = current {
        names.extend(current.keys().cloned());
    }
    names
        .into_iter()
        .filter_map(|asset_name| {
            let before = previous.and_then(|values| values.get(&asset_name));
            let after = current.and_then(|values| values.get(&asset_name));
            if before == after {
                return None;
            }
            let previous_value = before.map(|values| serde_json::Value::Array(values.clone()));
            let current_value = after.map(|values| serde_json::Value::Array(values.clone()));
            let size_changed = field_changed(before, after, "size");
            let hash_changed = ["crc", "md5", "sha1"]
                .into_iter()
                .any(|field| field_changed(before, after, field));
            let other_evidence_changed = [
                "role",
                "evidence_scope",
                "evidence_provenance",
                "merge_name",
                "dump_status",
                "serial",
                "date",
                "metadata",
                "extensions",
            ]
            .into_iter()
            .any(|field| field_changed(before, after, field))
                || !(size_changed || hash_changed);
            Some(SnapshotRequirementChange {
                asset_name,
                size_changed,
                hash_changed,
                other_evidence_changed,
                previous: previous_value,
                current: current_value,
            })
        })
        .collect()
}

fn field_changed(
    previous: Option<&Vec<serde_json::Value>>,
    current: Option<&Vec<serde_json::Value>>,
    field: &str,
) -> bool {
    field_values(previous, field) != field_values(current, field)
}

fn field_values(rows: Option<&Vec<serde_json::Value>>, field: &str) -> Vec<serde_json::Value> {
    let mut values = rows
        .into_iter()
        .flatten()
        .filter_map(|row| row.get(field).filter(|value| !value.is_null()).cloned())
        .collect::<Vec<_>>();
    values.sort_by_key(serde_json::Value::to_string);
    values
}

fn json(value: &str) -> serde_json::Value {
    serde_json::from_str(value).unwrap_or_else(|_| serde_json::Value::String(value.to_owned()))
}

fn set_metadata(records: &CatalogRecords, name: &str, set: &SetRow) -> serde_json::Value {
    serde_json::json!({
        "metadata": json(&set.metadata_json),
        "source_extensions": records.extensions.iter()
            .filter(|((kind, record_name), _)| {
                record_name == name && matches!(kind.as_str(), "game" | "machine" | "set")
            })
            .flat_map(|((kind, _), extensions)| {
                extensions.iter().map(move |extension| {
                    serde_json::json!({"record_kind": kind, "extension": extension})
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn touches_set(
    explanation: &RelationshipExplanation,
    previous: &SnapshotKey,
    current: &SnapshotKey,
    name: &str,
) -> bool {
    let endpoint_matches = |endpoint: &RelationshipEndpoint| match endpoint {
        RelationshipEndpoint::CatalogRecord(record) => {
            (&record.snapshot == previous || &record.snapshot == current)
                && record.kind == CatalogRecordKind::Set
                && record.key.as_str() == name
        }
        _ => false,
    };
    endpoint_matches(&explanation.claim.subject) || endpoint_matches(&explanation.claim.target)
}
