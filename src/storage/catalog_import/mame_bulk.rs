//! Bounded, dependency-ordered writes; identity decisions retain source order.
use diesel::SqliteConnection;

use super::{
    SnapshotAsset, SnapshotSet, asset_identity_input, checked_order, mame_relationships,
    mame_specification, mame_switches, reported_relationships, root_assets, root_sets,
};
use crate::{
    domain::{CatalogContentId, CatalogSetId, OccurrenceId, SnapshotKey},
    storage::{
        cached_sql::InsertBatch,
        catalog_content::{
            ContentIdentityInput, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions_bulk, record_wide_content_conflict,
            resolve_content_prefix,
        },
        mame_attributes::{Family, PositionBatch},
    },
};

pub(super) const MACHINE_BATCH_SIZE: usize = 64;
const ASSET_BATCH_SIZE: usize = 64;
const MAX_BATCH_NODES: usize = 4096;

/// Flush thresholds control queued work, never which source facts are accepted.
#[derive(Default)]
pub(super) struct MachineBatch {
    sets: Vec<SnapshotSet>,
    nodes: usize,
}

impl MachineBatch {
    pub(super) fn push(
        &mut self,
        conn: &mut SqliteConnection,
        snapshot: &SnapshotKey,
        set: SnapshotSet,
    ) -> crate::Result<()> {
        let nodes = node_count(&set);
        if !self.sets.is_empty() && self.nodes.saturating_add(nodes) > MAX_BATCH_NODES {
            self.flush(conn, snapshot)?;
        }
        self.nodes = self.nodes.saturating_add(nodes);
        self.sets.push(set);
        if self.sets.len() >= MACHINE_BATCH_SIZE || self.nodes >= MAX_BATCH_NODES {
            self.flush(conn, snapshot)?;
        }
        Ok(())
    }

    pub(super) fn flush(
        &mut self,
        conn: &mut SqliteConnection,
        snapshot: &SnapshotKey,
    ) -> crate::Result<()> {
        if !self.sets.is_empty() {
            insert(conn, snapshot, &self.sets)?;
            self.sets.clear();
            self.nodes = 0;
        }
        Ok(())
    }
}

fn node_count(set: &SnapshotSet) -> usize {
    use crate::mame::MachineSpecification as Spec;
    let mut nodes = 1_usize
        .saturating_add(set.assets.len())
        .saturating_add(set.bios_sets.len())
        .saturating_add(set.machine_dependencies.len());
    for switch in &set.switches {
        nodes = nodes
            .saturating_add(1)
            .saturating_add(switch.locations.len())
            .saturating_add(switch.values.len());
    }
    for element in &set.specification {
        let children = match &element.value {
            Spec::Input(value) => value.controls.len(),
            Spec::Port(value) => value.analogs.len(),
            Spec::Device(value) => value
                .extensions
                .len()
                .saturating_add(usize::from(value.instance.is_some())),
            Spec::Slot(value) => value.options.len(),
            Spec::Sample(_)
            | Spec::Chip(_)
            | Spec::Display(_)
            | Spec::Sound(_)
            | Spec::Adjuster(_)
            | Spec::Driver(_)
            | Spec::Feature(_)
            | Spec::SoftwareList(_)
            | Spec::RamOption(_) => 0,
        };
        nodes = nodes.saturating_add(1).saturating_add(children);
    }
    nodes
}

pub(super) fn insert(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    sets: &[SnapshotSet],
) -> crate::Result<()> {
    let owners = root_sets::insert_many(conn, snapshot, sets)?;
    let mut values = InsertBatch::default();
    for (set, owner) in sets.iter().zip(&owners) {
        let facts = set.mame_facts.as_ref().ok_or_else(|| {
            crate::Error::DatabaseSchema("MAME bulk owner has no machine facts".into())
        })?;
        super::enqueue_mame_machine_facts(conn, &mut values, owner.as_i64(), facts)?;
    }
    values.flush(conn)?;
    for (set, owner) in sets.iter().zip(&owners) {
        mame_specification::insert_values_bulk(conn, &mut values, owner.as_i64(), set)?;
        mame_switches::insert_values_bulk(conn, &mut values, owner.as_i64(), set)?;
    }
    values.flush(conn)?;
    let selected = owners.iter().copied().zip(sets).collect::<Vec<_>>();
    mame_relationships::insert_many(conn, snapshot, &selected)?;
    let mut positions = PositionBatch::default();
    for (owner, set) in &selected {
        let set_id = owner.as_i64();
        let facts = set.mame_facts.as_ref().ok_or_else(|| {
            crate::Error::DatabaseSchema("MAME bulk owner has no machine facts".into())
        })?;
        super::enqueue_mame_machine_positions(conn, &mut positions, set_id, facts)?;
        let mut reference_order = 0_i64;
        for dependency in set
            .machine_dependencies
            .iter()
            .filter(|row| row.source_field == "device_ref")
        {
            positions.queue(
                conn,
                Family::DeviceReference,
                &[set_id, reference_order],
                &dependency.attribute_positions,
                crate::mame::MameDeviceReferenceAttribute::code,
            )?;
            reference_order = reference_order.checked_add(1).ok_or_else(|| {
                crate::Error::DatabaseSchema("device reference order overflow".into())
            })?;
        }
        mame_specification::insert_positions_bulk(conn, &mut positions, set_id, set)?;
        mame_switches::insert_positions_bulk(conn, &mut positions, set_id, set)?;
    }
    positions.flush(conn)?;
    insert_assets(conn, snapshot, &selected)
}

fn insert_assets(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    selected: &[(CatalogSetId, &SnapshotSet)],
) -> crate::Result<()> {
    let mut assets = selected.iter().flat_map(|(owner, set)| {
        set.assets
            .iter()
            .enumerate()
            .map(move |(order, asset)| Ok((*owner, checked_order(order, "catalog assets")?, asset)))
    });
    loop {
        let page = assets
            .by_ref()
            .take(ASSET_BATCH_SIZE)
            .collect::<crate::Result<Vec<_>>>()?;
        if page.is_empty() {
            break;
        }
        let inputs = page
            .iter()
            .map(|(_, _, asset)| asset_identity_input(asset))
            .collect::<crate::Result<Vec<_>>>()?;
        let mut offset = 0;
        while offset < page.len() {
            let prefix = resolve_content_prefix(conn, &inputs[offset..])?;
            let resolutions = &prefix.resolutions;
            let mut start = 0;
            while start < resolutions.len() {
                // A conflict records exactly its already-seen evidence. Never write
                // a later linked occurrence before recording that prefix's conflict.
                let end = resolutions[start..]
                    .iter()
                    .position(|row| matches!(row, ContentIdentityResolution::Conflict { .. }))
                    .map_or(resolutions.len(), |position| start + position + 1);
                let occurrences = insert_claims(
                    conn,
                    snapshot,
                    &page[offset + start..offset + end],
                    &inputs[offset + start..offset + end],
                    resolutions[start..end]
                        .iter()
                        .map(ContentIdentityResolution::content_id),
                )?;
                for (occurrence, resolution) in occurrences.iter().zip(&resolutions[start..end]) {
                    record_content_identity_conflict(conn, *occurrence, resolution)?;
                }
                start = end;
            }
            offset += resolutions.len();
            if prefix.has_wide_conflict {
                let occurrences = insert_claims(
                    conn,
                    snapshot,
                    &page[offset..=offset],
                    &inputs[offset..=offset],
                    std::iter::once(None),
                )?;
                record_wide_content_conflict(conn, occurrences[0], inputs[offset].assertions)?;
                offset += 1;
            } else if offset != page.len() {
                return Err(crate::Error::DatabaseSchema(
                    "incomplete bulk identity correspondence".into(),
                ));
            }
        }
    }
    Ok(())
}

fn insert_claims(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    page: &[(CatalogSetId, i64, &SnapshotAsset)],
    inputs: &[ContentIdentityInput<'_>],
    contents: impl Iterator<Item = Option<CatalogContentId>>,
) -> crate::Result<Vec<OccurrenceId>> {
    let native = page
        .iter()
        .zip(contents)
        .map(
            |((record, order, asset), content)| root_assets::MameAssetInput {
                record: *record,
                order: *order,
                asset,
                content,
            },
        )
        .collect::<Vec<_>>();
    if native.len() != page.len() || inputs.len() != page.len() {
        return Err(crate::Error::DatabaseSchema(
            "incomplete bulk media correspondence".into(),
        ));
    }
    let occurrences = root_assets::insert_mame_bulk(conn, &native)?;
    let merges = occurrences
        .iter()
        .copied()
        .zip(page.iter().map(|(_, _, asset)| *asset))
        .collect::<Vec<_>>();
    reported_relationships::insert_asset_merges_bulk(conn, snapshot, &merges)?;
    let assertions = occurrences
        .iter()
        .copied()
        .zip(inputs.iter().map(|input| input.assertions))
        .collect::<Vec<_>>();
    record_occurrence_digest_assertions_bulk(conn, &assertions, "source_declared")?;
    Ok(occurrences)
}
