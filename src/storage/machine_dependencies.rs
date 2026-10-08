use std::collections::BTreeMap;

use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    domain::{CatalogScope, QualifiedCatalogSet, SetName, SnapshotKey},
    machine_dependencies::{
        MachineDependency, MachineDependencyCatalog, MachineDependencyKind, MachineSet,
        SnapshotCompleteness,
    },
};

use super::{
    catalog_coverage::{self, CoverageId},
    db::Pool,
};

#[derive(QueryableByName)]
struct SetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    parent_name: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    is_bios: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    is_device: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    logiqx_is_bios: Option<String>,
}

#[derive(QueryableByName)]
struct MameDependencyRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
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
        "SELECT s.coverage_id, pi.format AS parser_format \
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
    let scope = catalog_coverage::load(&mut conn, CoverageId::try_from(header.coverage_id)?)?;
    let completeness = match scope {
        CatalogScope::Unknown => SnapshotCompleteness::Unknown,
        CatalogScope::Complete => SnapshotCompleteness::Complete,
        CatalogScope::Filtered(sets) => SnapshotCompleteness::Filtered(Some(
            sets.into_iter()
                .filter_map(|set| match set {
                    QualifiedCatalogSet::RootSet(name) => Some(name),
                    QualifiedCatalogSet::SoftwareItem { .. } => None,
                })
                .collect(),
        )),
        CatalogScope::Partial(_) => SnapshotCompleteness::Partial,
    };

    let is_mame = header.parser_format == "mame-listxml";
    let rows = sql_query(
        "SELECT s.set_id, s.set_name, s.parent_name, mf.is_bios, mf.is_device, lf.is_bios AS logiqx_is_bios \
         FROM snapshot_sets AS s LEFT JOIN mame_machine_facts AS mf \
           ON mf.set_id = s.set_id \
         LEFT JOIN logiqx_set_facts AS lf \
           ON lf.set_id = s.set_id \
         WHERE s.snapshot_key = ? ORDER BY s.set_name, s.set_id",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SetRow>(&mut conn)?;

    let dependencies = load_dependencies(&mut conn, snapshot, is_mame)?;

    let sets = rows
        .into_iter()
        .map(|row| machine_set_from_row(row, &dependencies))
        .collect();

    Ok(MachineDependencyCatalog::with_completeness(
        snapshot.clone(),
        completeness,
        sets,
    ))
}

pub(super) const MAME_DEPENDENCIES: &str = concat!(
    "WITH requested_mame_machines(set_id) AS (\
       SELECT set_id FROM snapshot_sets WHERE snapshot_key = ?\
     ), dependencies AS (",
    include_str!("db/mame_dependencies.sql"),
    ") SELECT dependency.* FROM dependencies AS dependency \
       JOIN catalog_sets AS sets USING (set_id) \
       ORDER BY sets.set_name, dependency.dependency_order"
);

fn load_dependencies(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
    is_mame: bool,
) -> crate::Result<BTreeMap<i64, Vec<MameDependencyRow>>> {
    let query = if is_mame {
        MAME_DEPENDENCIES
    } else {
        "SELECT links.set_id, links.link_kind AS dependency_kind, links.target_name \
         FROM (SELECT sets.set_id, sets.set_name, link.link_kind, link.target_name, position.source_line, position.source_column, 0 AS reference_order \
               FROM logiqx_set_links AS link JOIN snapshot_sets AS sets USING (set_id) \
               JOIN logiqx_game_attribute_positions AS position ON position.set_id=link.set_id \
                AND position.field_kind=CASE link.link_kind WHEN 'romof' THEN 4 WHEN 'sampleof' THEN 5 END \
               WHERE sets.snapshot_key = ? AND link.link_kind IN ('romof', 'sampleof') \
               UNION ALL \
               SELECT sets.set_id, sets.set_name, 'device_ref', reference.target_name, position.source_line, position.source_column, reference.reference_order \
               FROM logiqx_device_references AS reference JOIN snapshot_sets AS sets USING (set_id) \
               JOIN logiqx_device_reference_attribute_positions AS position \
                ON position.set_id=reference.set_id AND position.reference_order=reference.reference_order AND position.field_kind=0 \
               WHERE sets.snapshot_key = ?) AS links \
         ORDER BY links.set_name, links.source_line, links.source_column, links.reference_order"
    };
    let query = sql_query(query).bind::<Text, _>(snapshot.as_str());
    let rows = if is_mame {
        query.load::<MameDependencyRow>(conn)?
    } else {
        query
            .bind::<Text, _>(snapshot.as_str())
            .load::<MameDependencyRow>(conn)?
    };
    Ok(rows.into_iter().fold(
        BTreeMap::<i64, Vec<MameDependencyRow>>::new(),
        |mut by_set, row| {
            by_set.entry(row.set_id).or_default().push(row);
            by_set
        },
    ))
}

fn machine_set_from_row(
    row: SetRow,
    dependencies: &BTreeMap<i64, Vec<MameDependencyRow>>,
) -> MachineSet {
    let mut set = MachineSet::new(row.set_name);
    set.parent_clone = row.parent_name.map(SetName::new);
    set.is_bios = row
        .is_bios
        .or_else(|| row.logiqx_is_bios.as_deref().map(is_true))
        .unwrap_or(false);
    set.is_device = row.is_device.unwrap_or(false);

    for dependency in dependencies.get(&row.set_id).into_iter().flatten() {
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
            _ => unreachable!("machine dependency kind is source constrained"),
        }
    }
    set
}

#[derive(QueryableByName)]
struct SnapshotExists {
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
    #[diesel(sql_type = Text)]
    parser_format: String,
}

fn is_true(value: &str) -> bool {
    matches!(value.to_ascii_lowercase().as_str(), "yes" | "true" | "1")
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use diesel::{connection::SimpleConnection, sql_types::Text};

    use super::*;
    use crate::database::Database;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    #[test]
    fn mame_snapshot_dependencies_seek_native_owner_keys() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        let mut connection = database.pool().get()?;
        connection.batch_execute("ANALYZE")?;
        let plan = sql_query(format!("EXPLAIN QUERY PLAN {MAME_DEPENDENCIES}"))
            .bind::<Text, _>("requested-snapshot")
            .load::<PlanRow>(&mut connection)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        for alias in ["native", "reference", "rom"] {
            assert!(
                !plan
                    .iter()
                    .any(|row| row.starts_with(&format!("SCAN {alias}"))),
                "snapshot dependencies scan {alias}: {plan:?}"
            );
            assert!(
                plan.iter()
                    .any(|row| row.starts_with(&format!("SEARCH {alias} USING PRIMARY KEY"))),
                "snapshot dependencies do not seek {alias}: {plan:?}"
            );
        }
        Ok(())
    }
}
