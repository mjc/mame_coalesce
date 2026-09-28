use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};

use crate::domain::{
    CatalogKey, CatalogScope, CatalogSnapshotDiff, CatalogSnapshotEntry, RelationshipEndpoint,
    RelationshipExplanation, SnapshotKey, SnapshotRecordDiff, SnapshotRecordStatus,
    SnapshotRequirementChange,
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
    #[diesel(sql_type = Bool)]
    published: bool,
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
    #[diesel(sql_type = BigInt)]
    component_order: i64,
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
    #[diesel(sql_type = Nullable<Text>)]
    owner_set_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    owner_component_order: Option<String>,
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
    asset_extensions: BTreeMap<(String, String), Vec<serde_json::Value>>,
}

pub fn diff(
    pool: &Pool,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<CatalogSnapshotDiff> {
    let mut conn = pool.get()?;
    conn.transaction::<_, crate::Error, _>(|conn| {
        let previous_header = snapshot(conn, previous)?;
        let current_header = snapshot(conn, current)?;
        if previous_header.catalog_key != current_header.catalog_key {
            return Err(crate::Error::InvalidPath(
                "snapshot history can only compare snapshots from the same catalog".to_owned(),
            ));
        }

        let same_scope = comparable_scope(&previous_header, &current_header);
        let previous_records = records(conn, previous)?;
        let current_records = records(conn, current)?;
        let explanations =
            super::relationships::explain_catalog_sets_for_snapshots(conn, previous, current)?;
        let mut evidence_by_set = relationship_evidence_by_set(explanations, previous, current);
        let mut names = BTreeSet::new();
        names.extend(previous_records.sets.keys().cloned());
        names.extend(current_records.sets.keys().cloned());

        let records = names
            .into_iter()
            .map(|name| {
                let before = previous_records.sets.get(&name);
                let after = current_records.sets.get(&name);
                let relationship_evidence = evidence_by_set.remove(&name).unwrap_or_default();
                let (status, metadata_changed, regrouped, requirement_changes) =
                    match (before, after) {
                        (None, None) => unreachable!("name came from one of the set maps"),
                        (None, Some(_))
                            if scopes_cover_set(&previous_header, &current_header, &name) =>
                        {
                            (
                                SnapshotRecordStatus::AddedWithinScope,
                                false,
                                false,
                                Vec::new(),
                            )
                        }
                        (Some(_), None)
                            if scopes_cover_set(&previous_header, &current_header, &name) =>
                        {
                            (
                                SnapshotRecordStatus::RemovedWithinScope,
                                false,
                                false,
                                Vec::new(),
                            )
                        }
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
                            let status =
                                if metadata_changed || regrouped || !requirement_changes.is_empty()
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
    })
}

fn relationship_evidence_by_set(
    explanations: Vec<RelationshipExplanation>,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> BTreeMap<String, Vec<RelationshipExplanation>> {
    let mut evidence_by_set = BTreeMap::<String, Vec<RelationshipExplanation>>::new();
    for explanation in explanations {
        let mut names = BTreeSet::new();
        for endpoint in [&explanation.claim.subject, &explanation.claim.target] {
            if let RelationshipEndpoint::CatalogRecord(record) = endpoint
                && (&record.snapshot == previous || &record.snapshot == current)
                && record.kind == crate::domain::CatalogRecordKind::Set
            {
                names.insert(record.key.as_str().to_owned());
            }
        }
        for name in names {
            evidence_by_set
                .entry(name)
                .or_default()
                .push(explanation.clone());
        }
    }
    evidence_by_set
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
        "SELECT snapshot.catalog_key, snapshot.scope_kind, snapshot.scope_json, \
                EXISTS (SELECT 1 FROM snapshot_publications AS publication \
                        WHERE publication.snapshot_key = snapshot.snapshot_key) AS published \
         FROM catalog_snapshots AS snapshot WHERE snapshot.snapshot_key = ?",
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
    if !previous.published || !current.published {
        return false;
    }
    if previous.scope_kind == "complete" && current.scope_kind == "complete" {
        return true;
    }
    if previous.scope_kind != "filtered" || current.scope_kind != "filtered" {
        return false;
    }
    match (
        scope_set_names(previous),
        scope_set_names(current),
        scope_policy(previous),
        scope_policy(current),
    ) {
        (
            Some(previous_names),
            Some(current_names),
            Some(previous_policy),
            Some(current_policy),
        ) => previous_names == current_names && previous_policy == current_policy,
        _ => false,
    }
}

fn absence_status(
    previous: &SnapshotRow,
    current: &SnapshotRow,
    name: &str,
) -> SnapshotRecordStatus {
    let explicitly_excluded = [previous, current].into_iter().any(|snapshot| {
        snapshot.published
            && snapshot.scope_kind == "filtered"
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
    snapshot.published
        && match snapshot.scope_kind.as_str() {
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
        "SELECT set_name, component_order, asset_name, role, size, crc, md5, sha1, evidence_scope, \
         evidence_provenance, merge_name, dump_status, serial, date, metadata_json \
         FROM asset_requirements WHERE snapshot_key = ? ORDER BY set_name, asset_name, component_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<RequirementRow>(conn)?;
    let extensions = sql_query(
        "SELECT record_kind, record_name, owner_set_name, owner_component_order, field_name, namespace_uri, raw_value_json \
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
        let record_kind = extension.record_kind.clone();
        let record_name = extension.record_name.clone();
        let value = serde_json::json!({
            "record_kind": record_kind,
            "record_name": record_name,
            "field": extension.field_name,
            "namespace": extension.namespace_uri,
            "value": json(&extension.raw_value_json),
        });
        match (extension.owner_set_name, extension.owner_component_order) {
            (Some(set), Some(component_order)) => result
                .asset_extensions
                .entry((set, component_order))
                .or_default()
                .push(value),
            (Some(set), None) => result
                .extensions
                .entry(("set".to_owned(), set))
                .or_default()
                .push(value),
            (None, _) => {
                if let Some(record_name) = record_name {
                    result
                        .extensions
                        .entry((record_kind, record_name))
                        .or_default()
                        .push(value);
                }
            }
        }
    }
    for extensions in result.extensions.values_mut() {
        extensions.sort_by_key(serde_json::Value::to_string);
    }
    for row in requirements {
        let source_extensions = result
            .asset_extensions
            .remove(&(row.set_name.clone(), row.component_order.to_string()))
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
