use std::collections::{BTreeMap, BTreeSet};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::{
    domain::AssetRole,
    domain::{CatalogKey, DatRom, RequirementKey, SetKey, SetMetadata, SetName, SnapshotKey},
};

use super::{
    catalog_reconciliation::{NativeRootRequirement, snapshot_requirements},
    db::Pool,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogSelector {
    LatestCatalog(String),
    Snapshot(SnapshotKey),
}

impl CatalogSelector {
    #[must_use]
    pub(crate) fn value(&self) -> &str {
        match self {
            Self::LatestCatalog(value) => value,
            Self::Snapshot(snapshot) => snapshot.as_str(),
        }
    }
}

pub struct BuildCatalogRepository<'pool> {
    pool: &'pool Pool,
}

impl<'pool> BuildCatalogRepository<'pool> {
    #[must_use]
    pub const fn new(pool: &'pool Pool) -> Self {
        Self { pool }
    }

    pub fn load(&self, selector: &CatalogSelector) -> crate::Result<PublishedBuildCatalog> {
        let mut conn = self.pool.get()?;
        conn.transaction::<_, crate::Error, _>(|conn| {
            let snapshot_key = match selector {
                CatalogSelector::LatestCatalog(value) => {
                    let catalog = resolve_catalog(conn, value)?;
                    latest_published_snapshot(conn, &catalog.catalog_key)?
                }
                CatalogSelector::Snapshot(snapshot) => snapshot.clone(),
            };
            load_published_catalog(conn, snapshot_key, selector.value())
        })
    }
}

fn load_published_catalog(
    conn: &mut SqliteConnection,
    snapshot_key: SnapshotKey,
    label: &str,
) -> crate::Result<PublishedBuildCatalog> {
    reject_software_lists(conn, &snapshot_key, label)?;
    let snapshot = snapshot_requirements(conn, &snapshot_key)?;
    let (root_sets, set_names) = load_root_sets(conn, &snapshot_key)?;
    let requirements = snapshot
        .root_requirements
        .into_iter()
        .filter(|requirement| requirement.role == AssetRole::Rom)
        .map(|requirement| into_build_requirement(requirement, &root_sets, &snapshot.catalog))
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(PublishedBuildCatalog {
        catalog_key: snapshot.catalog,
        snapshot_key,
        requirements,
        set_names,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedBuildCatalog {
    catalog_key: CatalogKey,
    snapshot_key: SnapshotKey,
    requirements: Vec<DatRom>,
    set_names: BTreeSet<SetName>,
}

impl PublishedBuildCatalog {
    #[must_use]
    pub fn requirements(&self) -> &[DatRom] {
        &self.requirements
    }

    #[must_use]
    pub const fn set_names(&self) -> &BTreeSet<SetName> {
        &self.set_names
    }

    #[must_use]
    pub const fn catalog_key(&self) -> &CatalogKey {
        &self.catalog_key
    }

    #[must_use]
    pub const fn snapshot_key(&self) -> &SnapshotKey {
        &self.snapshot_key
    }
}

#[derive(QueryableByName)]
struct CatalogRow {
    #[diesel(sql_type = Text)]
    catalog_key: String,
}

#[derive(QueryableByName)]
struct CatalogKeyRow {
    #[diesel(sql_type = Text)]
    catalog_key: String,
}

#[derive(QueryableByName)]
struct SnapshotKeyRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
}

#[derive(QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Bool)]
    exists: bool,
}

#[derive(QueryableByName)]
struct RootSetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    parent_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    is_bios: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_of: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sample_of: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    board: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rebuild_to: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
}

#[derive(QueryableByName)]
struct DeviceReferenceRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    target_name: String,
}

#[derive(Clone)]
struct RootSet {
    name: String,
    parent_name: Option<String>,
    metadata: SetMetadata,
}

fn resolve_catalog(conn: &mut SqliteConnection, value: &str) -> crate::Result<CatalogRow> {
    // A catalog key is identity, even when its text also names a filesystem path.
    if let Some(catalog) = catalog_by_key(conn, value)? {
        return Ok(catalog);
    }
    if let Some(catalog) = catalog_by_source_path(conn, Utf8Path::new(value))? {
        return Ok(catalog);
    }
    let candidates = sql_query(
        "SELECT catalog_key FROM catalogs WHERE display_name = ? \
         AND EXISTS (SELECT 1 FROM snapshot_publications WHERE catalog_key = catalogs.catalog_key) \
         UNION \
         SELECT publication.catalog_key FROM snapshot_publications AS publication \
         JOIN logiqx_document_facts AS headers USING (snapshot_key) \
         WHERE headers.header_name = ? AND publication.rowid = ( \
             SELECT latest.rowid FROM snapshot_publications AS latest \
             WHERE latest.catalog_key = publication.catalog_key ORDER BY latest.rowid DESC LIMIT 1)",
    )
    .bind::<Text, _>(value)
    .bind::<Text, _>(value)
    .load::<CatalogKeyRow>(conn)?
    .into_iter().map(|row| row.catalog_key).collect::<Vec<_>>();
    let key = unique_catalog_key(&candidates, value)?;
    catalog_by_key(conn, &key)?.ok_or_else(|| crate::Error::CatalogNotFound(value.into()))
}

fn catalog_by_source_path(
    conn: &mut SqliteConnection,
    path: &Utf8Path,
) -> crate::Result<Option<CatalogRow>> {
    let path = retained_source_path(path)?;
    let candidates = sql_query(
        "SELECT DISTINCT catalogs.catalog_key FROM catalogs \
         JOIN catalog_snapshots USING (catalog_key) \
         JOIN snapshot_publications USING (snapshot_key, catalog_key) \
         JOIN acquisitions ON acquisitions.acquisition_key = catalog_snapshots.acquisition_key \
         WHERE acquisitions.source_uri = ?",
    )
    .bind::<Text, _>(path.as_str())
    .load::<CatalogKeyRow>(conn)?
    .into_iter()
    .map(|row| row.catalog_key)
    .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    let local_key = CatalogKey::for_local_dat(&path);
    let key = if candidates.iter().any(|key| key == local_key.as_str()) {
        local_key.as_str().to_owned()
    } else {
        unique_catalog_key(&candidates, path.as_str())?
    };
    catalog_by_key(conn, &key)
}

fn retained_source_path(path: &Utf8Path) -> crate::Result<Utf8PathBuf> {
    if let Ok(canonical) = path.canonicalize_utf8() {
        return Ok(canonical);
    }
    // Cached queries must not reopen source bytes. Resolve an existing parent when the file is gone.
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        let parent = if parent.as_str().is_empty() {
            Utf8Path::new(".")
        } else {
            parent
        };
        if let Ok(canonical) = parent.canonicalize_utf8() {
            return Ok(canonical.join(name));
        }
    }
    Utf8PathBuf::try_from(std::path::absolute(path.as_std_path())?)
        .map_err(|error| crate::Error::InvalidPath(error.to_string()))
}

fn catalog_by_key(conn: &mut SqliteConnection, key: &str) -> crate::Result<Option<CatalogRow>> {
    Ok(
        sql_query("SELECT catalog_key FROM catalogs WHERE catalog_key = ?")
            .bind::<Text, _>(key)
            .get_result::<CatalogRow>(conn)
            .optional()?,
    )
}

fn unique_catalog_key(candidates: &[String], selector: &str) -> crate::Result<String> {
    match candidates {
        [key] => Ok(key.clone()),
        [] => Err(crate::Error::CatalogNotFound(selector.to_owned())),
        _ => Err(crate::Error::InvalidPath(format!(
            "catalog selector {selector:?} matches multiple catalogs"
        ))),
    }
}

fn latest_published_snapshot(
    conn: &mut SqliteConnection,
    catalog_key: &str,
) -> crate::Result<SnapshotKey> {
    let row = sql_query(
        "SELECT snapshot_key FROM snapshot_publications \
         WHERE catalog_key = ? \
         ORDER BY rowid DESC LIMIT 1",
    )
    .bind::<Text, _>(catalog_key)
    .get_result::<SnapshotKeyRow>(conn)
    .optional()?
    .ok_or_else(|| crate::Error::CatalogNotFound(catalog_key.to_owned()))?;
    Ok(SnapshotKey::from_persisted(row.snapshot_key))
}

fn reject_software_lists(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    selector: &str,
) -> crate::Result<()> {
    let has_software_lists = sql_query(
        "SELECT EXISTS(SELECT 1 FROM catalog_set_groups \
         WHERE snapshot_key = ? AND kind = 'software_list') AS `exists`",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<ExistsRow>(conn)?
    .exists;
    if has_software_lists {
        return Err(crate::Error::InvalidPath(format!(
            "catalog {selector:?} is a software-list catalog and cannot be loaded as a flat root build catalog"
        )));
    }
    Ok(())
}

fn load_root_sets(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<(BTreeMap<i64, RootSet>, BTreeSet<SetName>)> {
    let devices = sql_query(
        "SELECT device_refs.set_id, device_refs.target_name \
         FROM (SELECT set_id, target_name, reference_order AS source_order \
               FROM logiqx_device_references \
               UNION ALL \
               SELECT set_id, target_name, dependency_order AS source_order \
               FROM mame_machine_dependencies WHERE dependency_kind = 'device_ref') AS device_refs \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY device_refs.set_id, device_refs.source_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<DeviceReferenceRow>(conn)?
    .into_iter()
    .fold(BTreeMap::<i64, Vec<String>>::new(), |mut devices, row| {
        devices.entry(row.set_id).or_default().push(row.target_name);
        devices
    });

    let rows = sql_query(
        "SELECT sets.set_id, sets.set_name, \
         COALESCE(sets.parent_name, pc_clone_parent.set_name, pc_merge_parent.set_name) AS parent_name, \
         COALESCE(games.source_file, machines.source_file) AS source_file, \
         COALESCE(games.is_bios, CASE WHEN machines.is_bios = 1 THEN 'yes' WHEN machines.is_bios = 0 THEN 'no' END, \
                  pc_games.bios_text) AS is_bios, \
         COALESCE((SELECT target_name FROM logiqx_set_links \
          WHERE set_id = sets.set_id AND link_kind = 'romof'), \
          (SELECT target_name FROM mame_machine_dependencies \
           WHERE set_id = sets.set_id AND dependency_kind = 'romof')) AS rom_of, \
         COALESCE((SELECT target_name FROM logiqx_set_links \
          WHERE set_id = sets.set_id AND link_kind = 'sampleof'), \
          (SELECT target_name FROM cmp_sample_parent_links \
           WHERE record_id = sets.set_id), \
          (SELECT target_name FROM mame_machine_dependencies \
           WHERE set_id = sets.set_id AND dependency_kind = 'sampleof')) AS sample_of, \
         games.board, COALESCE(games.rebuild_to, cmp.rebuildto) AS rebuild_to, \
         COALESCE(games.description, machines.description, cmp.description, pc_games.description) AS description, \
         COALESCE(games.year, machines.year, cmp.year) AS year, \
         COALESCE(games.manufacturer, machines.manufacturer, cmp.manufacturer) AS manufacturer \
         FROM snapshot_sets AS sets \
         LEFT JOIN logiqx_games AS games USING (set_id) \
         LEFT JOIN mame_machines AS machines USING (set_id) \
         LEFT JOIN cmp_set_facts AS cmp ON cmp.record_id = sets.set_id \
         LEFT JOIN no_intro_pc_games AS pc_games USING (set_id) \
         LEFT JOIN no_intro_pc_clone_links AS pc_clone USING (set_id) \
         LEFT JOIN no_intro_pc_merge_links AS pc_merge USING (set_id) \
         LEFT JOIN no_intro_pc_games AS pc_clone_facts ON pc_clone_facts.archive_id = pc_clone.target_archive_id \
             AND pc_clone_facts.set_id IN (SELECT set_id FROM catalog_sets \
                 WHERE set_group_id = sets.set_group_id) \
         LEFT JOIN snapshot_sets AS pc_clone_parent ON pc_clone_parent.set_id = pc_clone_facts.set_id \
             AND pc_clone_parent.snapshot_key = sets.snapshot_key \
         LEFT JOIN no_intro_pc_games AS pc_merge_facts ON pc_merge_facts.archive_id = pc_merge.target_archive_id \
             AND pc_merge_facts.set_id IN (SELECT set_id FROM catalog_sets \
                 WHERE set_group_id = sets.set_group_id) \
         LEFT JOIN snapshot_sets AS pc_merge_parent ON pc_merge_parent.set_id = pc_merge_facts.set_id \
             AND pc_merge_parent.snapshot_key = sets.snapshot_key \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_id",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<RootSetRow>(conn)?;

    let mut root_sets = BTreeMap::new();
    let mut set_names = BTreeSet::new();
    let mut distinct_names = BTreeSet::new();
    for row in rows {
        if !distinct_names.insert(row.set_name.clone()) {
            return Err(crate::Error::InvalidPath(format!(
                "catalog snapshot {} has repeated root set name {:?}; build layout requires unique names",
                snapshot.as_str(),
                row.set_name
            )));
        }
        set_names.insert(SetName::new(row.set_name.clone()));
        let metadata = SetMetadata {
            source_file: row.source_file,
            is_bios: row.is_bios,
            rom_of: row.rom_of,
            sample_of: row.sample_of,
            board: row.board,
            rebuild_to: row.rebuild_to,
            description: row.description,
            year: row.year,
            manufacturer: row.manufacturer,
            device_refs: devices.get(&row.set_id).cloned().unwrap_or_default(),
        };
        root_sets.insert(
            row.set_id,
            RootSet {
                name: row.set_name,
                parent_name: row.parent_name,
                metadata,
            },
        );
    }
    Ok((root_sets, set_names))
}

fn into_build_requirement(
    requirement: NativeRootRequirement,
    root_sets: &BTreeMap<i64, RootSet>,
    catalog_key: &CatalogKey,
) -> crate::Result<DatRom> {
    let owner_id = requirement.set_id.as_i64();
    let root_set = root_sets.get(&owner_id).ok_or_else(|| {
        crate::Error::InvalidPath(format!(
            "catalog requirement refers to non-root set owner {owner_id}"
        ))
    })?;
    if requirement.set_name != root_set.name {
        return Err(crate::Error::InvalidPath(format!(
            "catalog requirement owner name {:?} does not match native set {:?}",
            requirement.set_name, root_set.name
        )));
    }
    let component_order = u32::try_from(requirement.component_order)
        .map_err(|_| crate::Error::InvalidPath("too many catalog media components".to_owned()))?;

    Ok(DatRom {
        catalog_name: catalog_key.as_str().to_owned(),
        key: RequirementKey::new(
            SetKey::new(catalog_key.clone(), root_set.name.clone()),
            requirement.asset_name,
        ),
        parent_name: root_set.parent_name.clone(),
        set_metadata: root_set.metadata.clone(),
        role: requirement.role,
        component_order: Some(component_order),
        expected: requirement.expected,
    })
}
