use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, sql_query,
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
    #[diesel(sql_type = Nullable<Text>)]
    format_hint: Option<String>,
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
    #[diesel(sql_type = Nullable<Text>)]
    mame_region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_bios: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_offset: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_optional: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_sound_only: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_dispose: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_load_flag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_value: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_inverted: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_ovha: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_no_thread: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_disk_index: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_writable: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_writeable: Option<i64>,
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

#[derive(QueryableByName)]
struct MachineSwitchRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = BigInt)]
    mask: i64,
}

#[derive(QueryableByName)]
struct MachineSwitchLocationRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    location_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    number: String,
    #[diesel(sql_type = Bool)]
    inverted: bool,
}

#[derive(QueryableByName)]
struct MachineSwitchValueRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    value_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    value: i64,
    #[diesel(sql_type = Bool)]
    is_default: bool,
}

#[derive(QueryableByName)]
struct MachineBiosSetRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    bios_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Bool)]
    is_default: bool,
}

#[derive(QueryableByName)]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent MAME DTD flags from one SQLite row.
struct MameMachineFactsRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type = Bool)]
    is_device: bool,
    #[diesel(sql_type = Bool)]
    runnable: bool,
    #[diesel(sql_type = Bool)]
    is_bios: bool,
    #[diesel(sql_type = Bool)]
    is_mechanical: bool,
    #[diesel(sql_type = Bool)]
    is_consumable: bool,
}

#[derive(QueryableByName)]
struct NoIntroGameFactsRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    archive_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
}

#[derive(Default)]
struct CatalogRecords {
    sets: BTreeMap<String, SetRow>,
    requirements: BTreeMap<String, BTreeMap<String, Vec<serde_json::Value>>>,
    extensions: BTreeMap<(String, String), Vec<serde_json::Value>>,
    asset_extensions: BTreeMap<(String, String), Vec<serde_json::Value>>,
    machine_switches: BTreeMap<String, Vec<serde_json::Value>>,
    machine_bios_sets: BTreeMap<String, Vec<serde_json::Value>>,
    mame_machine_facts: BTreeMap<String, serde_json::Value>,
    no_intro_game_facts: BTreeMap<String, serde_json::Value>,
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
        if is_software_list_snapshot(&previous_header) || is_software_list_snapshot(&current_header)
        {
            return Err(crate::Error::InvalidPath(
                "snapshot history diff does not yet support MAME software-list catalogs".to_owned(),
            ));
        }

        let same_scope = comparable_scope(&previous_header, &current_header);
        let document_metadata_changed =
            mame_document_metadata(conn, previous)? != mame_document_metadata(conn, current)?;
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
            document_metadata_changed,
            records,
        })
    })
}

#[derive(QueryableByName, PartialEq, Eq)]
struct MameDocumentMetadataRow {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    debug: bool,
    #[diesel(sql_type = Nullable<Text>)]
    config_version: Option<String>,
}

fn mame_document_metadata(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Option<MameDocumentMetadataRow>> {
    Ok(
        sql_query("SELECT debug, config_version FROM mame_document_facts WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<MameDocumentMetadataRow>(conn)
            .optional()?,
    )
}

fn is_software_list_snapshot(snapshot: &SnapshotRow) -> bool {
    snapshot
        .format_hint
        .as_deref()
        .is_some_and(|hint| hint.split('+').next() == Some("mame-softwarelist-xml"))
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
        "SELECT snapshot.catalog_key, snapshot.scope_kind, snapshot.scope_json, documents.format_hint, \
                EXISTS (SELECT 1 FROM snapshot_publications AS publication \
                        WHERE publication.snapshot_key = snapshot.snapshot_key) AS published \
         FROM catalog_snapshots AS snapshot \
         JOIN documents USING (document_key) WHERE snapshot.snapshot_key = ?",
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
        "SELECT asset.set_name, asset.component_order, asset.asset_name, asset.role, asset.size, \
         asset.crc, asset.md5, asset.sha1, asset.evidence_scope, asset.evidence_provenance, \
         asset.merge_name, asset.dump_status, asset.serial, asset.date, asset.metadata_json, \
         facts.region AS mame_region, facts.bios AS mame_bios, facts.offset AS mame_offset, \
         facts.optional AS mame_optional, facts.sound_only AS mame_sound_only, \
         facts.dispose AS mame_dispose, facts.load_flag AS mame_load_flag, facts.value AS mame_value, \
         facts.inverted AS mame_inverted, facts.ovha AS mame_ovha, facts.no_thread AS mame_no_thread, \
         facts.disk_index AS mame_disk_index, facts.writable AS mame_writable, \
         facts.writeable AS mame_writeable \
         FROM asset_requirements AS asset LEFT JOIN mame_asset_facts AS facts \
         USING (snapshot_key, set_name, component_order) \
         WHERE asset.snapshot_key = ? ORDER BY asset.set_name, asset.asset_name, asset.component_order",
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
    let machine_switches = mame_switch_facts(conn, key)?;
    let machine_bios_sets = mame_bios_set_facts(conn, key)?;
    let mame_machine_facts = load_mame_machine_facts(conn, key)?;
    let no_intro_game_facts = sql_query(
        "SELECT set_name, archive_id, description FROM no_intro_game_facts \
         WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroGameFactsRow>(conn)?
    .into_iter()
    .map(|row| {
        (
            row.set_name,
            serde_json::json!({
                "archive_id": row.archive_id,
                "description": row.description,
            }),
        )
    })
    .collect();

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
    result.machine_switches = machine_switches;
    result.machine_bios_sets = machine_bios_sets;
    result.mame_machine_facts = mame_machine_facts;
    result.no_intro_game_facts = no_intro_game_facts;
    for extensions in result.extensions.values_mut() {
        extensions.sort_by_key(serde_json::Value::to_string);
    }
    assemble_requirements(&mut result, requirements);
    Ok(result)
}

fn assemble_requirements(result: &mut CatalogRecords, requirements: Vec<RequirementRow>) {
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
            "mame_attributes": {
                "region": row.mame_region,
                "bios": row.mame_bios,
                "offset": row.mame_offset,
                "optional": row.mame_optional.map(|value| value != 0),
                "sound_only": row.mame_sound_only.map(|value| value != 0),
                "dispose": row.mame_dispose.map(|value| value != 0),
                "load_flag": row.mame_load_flag,
                "value": row.mame_value,
                "inverted": row.mame_inverted.map(|value| value != 0),
                "ovha": row.mame_ovha,
                "no_thread": row.mame_no_thread.map(|value| value != 0),
                "disk_index": row.mame_disk_index,
                "writable": row.mame_writable.map(|value| value != 0),
                "writeable": row.mame_writeable.map(|value| value != 0),
            },
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
}

fn mame_switch_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<String, Vec<serde_json::Value>>> {
    let switches = sql_query(
        "SELECT set_name, switch_order, kind, name, tag, mask FROM machine_switches \
         WHERE snapshot_key = ? ORDER BY set_name, switch_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchRow>(conn)?;
    let switch_locations = sql_query(
        "SELECT set_name, switch_order, location_order, name, number, inverted \
         FROM machine_switch_locations WHERE snapshot_key = ? ORDER BY set_name, switch_order, location_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchLocationRow>(conn)?;
    let switch_values = sql_query(
        "SELECT set_name, switch_order, value_order, name, value, is_default \
         FROM machine_switch_values WHERE snapshot_key = ? ORDER BY set_name, switch_order, value_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchValueRow>(conn)?;

    let mut locations = BTreeMap::<(String, i64), Vec<serde_json::Value>>::new();
    for location in switch_locations {
        locations
            .entry((location.set_name, location.switch_order))
            .or_default()
            .push(serde_json::json!({
                "order": location.location_order,
                "name": location.name,
                "number": location.number,
                "inverted": location.inverted,
            }));
    }
    let mut values = BTreeMap::<(String, i64), Vec<serde_json::Value>>::new();
    for value in switch_values {
        values
            .entry((value.set_name, value.switch_order))
            .or_default()
            .push(serde_json::json!({
                "order": value.value_order,
                "name": value.name,
                "value": value.value,
                "default": value.is_default,
            }));
    }
    let mut result = BTreeMap::<String, Vec<serde_json::Value>>::new();
    for switch in switches {
        let key = (switch.set_name.clone(), switch.switch_order);
        result
            .entry(switch.set_name)
            .or_default()
            .push(serde_json::json!({
                "kind": switch.kind,
                "name": switch.name,
                "tag": switch.tag,
                "mask": switch.mask,
                "locations": locations.remove(&key).unwrap_or_default(),
                "values": values.remove(&key).unwrap_or_default(),
            }));
    }
    Ok(result)
}

fn mame_bios_set_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<String, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT set_name, bios_order, name, description, is_default FROM machine_bios_sets \
         WHERE snapshot_key = ? ORDER BY set_name, bios_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineBiosSetRow>(conn)?;
    let mut facts = BTreeMap::<String, Vec<serde_json::Value>>::new();
    for row in rows {
        facts
            .entry(row.set_name)
            .or_default()
            .push(serde_json::json!({
                "order": row.bios_order,
                "name": row.name,
                "description": row.description,
                "default": row.is_default,
            }));
    }
    Ok(facts)
}

fn load_mame_machine_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<String, serde_json::Value>> {
    let rows = sql_query(
        "SELECT set_name, source_file, description, year, manufacturer, is_device, runnable, \
         is_bios, is_mechanical, is_consumable FROM mame_machine_facts \
         WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MameMachineFactsRow>(conn)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.set_name,
                serde_json::json!({
                    "source_file": row.source_file,
                    "description": row.description,
                    "year": row.year,
                    "manufacturer": row.manufacturer,
                    "is_device": row.is_device,
                    "runnable": row.runnable,
                    "is_bios": row.is_bios,
                    "is_mechanical": row.is_mechanical,
                    "is_consumable": row.is_consumable,
                }),
            )
        })
        .collect())
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
        "mame_machine_facts": records.mame_machine_facts.get(name),
        "no_intro_game_facts": records.no_intro_game_facts.get(name),
        "machine_switches": records.machine_switches.get(name),
        "machine_bios_sets": records.machine_bios_sets.get(name),
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
