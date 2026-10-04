//! Complete selected CMP sets with source-ordered scalar, parent and media facts.

use super::{
    ClrMameProParentKind, ClrMameProParentReference, ClrMameProQueryError,
    ClrMameProRelationshipId, ClrMameProRomReference, ClrMameProSampleReference, ClrMameProSet,
    ClrMameProSetChild, ClrMameProSetField, positions, queries,
};
use crate::{
    domain::CatalogSetId,
    storage::catalog_files::{self, ClrMameProFilePayload, OccurrenceId},
};
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use std::collections::{BTreeMap, BTreeSet};

type QueryResult<T> = Result<T, ClrMameProQueryError>;

#[derive(QueryableByName)]
pub(super) struct SetRow {
    #[diesel(sql_type=BigInt)]
    pub set_id: i64,
    #[diesel(sql_type=BigInt)]
    set_group_id: i64,
    #[diesel(sql_type=Text)]
    set_name: String,
    #[diesel(sql_type=BigInt)]
    pub list_order: i64,
    #[diesel(sql_type=Text)]
    source_element_kind: String,
    #[diesel(sql_type=BigInt)]
    line: i64,
    #[diesel(sql_type=BigInt)]
    column: i64,
    #[diesel(sql_type=Nullable<BigInt>)]
    native_id: Option<i64>,
    #[diesel(sql_type=Nullable<Text>)]
    source_block: Option<String>,
    #[diesel(sql_type=Nullable<BigInt>)]
    document_order: Option<i64>,
    #[diesel(sql_type=Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    rebuildto: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    release_year_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    release_month_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    release_day_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ParentRow {
    #[diesel(sql_type=BigInt)]
    owner_id: i64,
    #[diesel(sql_type=Text)]
    link_kind: String,
    #[diesel(sql_type=Text)]
    target_name: String,
    #[diesel(sql_type=BigInt)]
    relationship_id: i64,
    #[diesel(sql_type=Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    origin: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    source_reference_kind: Option<String>,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct MediaRow {
    #[diesel(sql_type=BigInt)]
    owner_id: i64,
    #[diesel(sql_type=BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type=BigInt)]
    occurrence_order: i64,
    #[diesel(sql_type=Text)]
    claim_kind: String,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

pub(super) fn validate_row(row: &SetRow) -> QueryResult<usize> {
    if row.valid != 1
        || row.set_id <= 0
        || row.set_group_id <= 0
        || row.list_order < 0
        || row.native_id != Some(row.set_id)
        || row.source_element_kind != "cmp_set"
        || !row.source_block.as_ref().is_some_and(|block| {
            block.eq_ignore_ascii_case("set") || block.eq_ignore_ascii_case("game")
        })
    {
        return Err(ClrMameProQueryError::MismatchedOwner(row.set_id));
    }
    positions::location(row.line, row.column, row.set_id)?;
    positions::order(
        row.document_order
            .ok_or(ClrMameProQueryError::InvalidPositions(row.set_id))?,
        row.set_id,
    )
}

pub(super) fn hydrate(
    connection: &mut SqliteConnection,
    snapshot: &str,
    rows: Vec<SetRow>,
) -> QueryResult<Vec<ClrMameProSet>> {
    let ids = rows.iter().map(|row| row.set_id).collect::<Vec<_>>();
    let mut positions = positions::set_positions(connection, &ids)?;
    let mut parents = load_parents(connection, snapshot, &ids)?;
    let mut sets = rows
        .into_iter()
        .map(|row| {
            let owner = row.set_id;
            let fields = positions
                .remove(&owner)
                .ok_or(ClrMameProQueryError::InvalidPositions(owner))?;
            let links = parents.remove(&owner).unwrap_or_default();
            hydrate_row(row, fields, links)
        })
        .collect::<QueryResult<Vec<_>>>()?;
    attach_media(connection, &ids, &mut sets)?;
    for set in &mut sets {
        validate_children(set)?;
    }
    Ok(sets)
}

type ParentMap = BTreeMap<i64, BTreeMap<i64, ParentRow>>;

fn load_parents(
    connection: &mut SqliteConnection,
    snapshot: &str,
    ids: &[i64],
) -> QueryResult<ParentMap> {
    let mut result = ParentMap::new();
    for chunk in ids.chunks(400) {
        let mut query =
            sql_query(queries::parents(chunk.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in chunk {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<ParentRow>(connection)? {
            let (code, kind) = match row.link_kind.as_str() {
                "cloneof" => (ClrMameProSetField::CloneOf as i64, "clrmamepro_cloneof"),
                "sampleof" => (ClrMameProSetField::SampleOf as i64, "clrmamepro_sampleof"),
                _ => return Err(ClrMameProQueryError::MismatchedOwner(row.owner_id)),
            };
            if row.valid != 1
                || row.relationship_id <= 0
                || row.snapshot_key.as_deref() != Some(snapshot)
                || row.origin.as_deref() != Some("source")
                || row.source_reference_kind.as_deref() != Some(kind)
            {
                return Err(ClrMameProQueryError::MismatchedOwner(row.owner_id));
            }
            let owner = row.owner_id;
            if result.entry(owner).or_default().insert(code, row).is_some() {
                return Err(ClrMameProQueryError::MismatchedOwner(owner));
            }
        }
    }
    Ok(result)
}

fn hydrate_row(
    row: SetRow,
    mut fields: BTreeMap<i64, super::ClrMameProFieldPosition>,
    parents: BTreeMap<i64, ParentRow>,
) -> QueryResult<ClrMameProSet> {
    let order = validate_row(&row)?;
    let owner = row.set_id;
    let mut children = Vec::new();
    for (field, value) in [
        (ClrMameProSetField::Name, Some(row.set_name)),
        (ClrMameProSetField::Description, row.description),
        (ClrMameProSetField::Year, row.year),
        (ClrMameProSetField::Manufacturer, row.manufacturer),
        (ClrMameProSetField::RebuildTo, row.rebuildto),
        (ClrMameProSetField::Region, row.region),
        (ClrMameProSetField::ReleaseYear, row.release_year_text),
        (ClrMameProSetField::ReleaseMonth, row.release_month_text),
        (ClrMameProSetField::ReleaseDay, row.release_day_text),
        (ClrMameProSetField::Serial, row.serial),
    ] {
        if let Some(value) = positions::declared(value, field, &mut fields, owner)? {
            children.push(ClrMameProSetChild::Field { field, value });
        }
    }
    for (_, parent) in parents {
        let (field, kind) = match parent.link_kind.as_str() {
            "cloneof" => (ClrMameProSetField::CloneOf, ClrMameProParentKind::CloneOf),
            "sampleof" => (ClrMameProSetField::SampleOf, ClrMameProParentKind::SampleOf),
            _ => return Err(ClrMameProQueryError::MismatchedOwner(owner)),
        };
        let target = positions::declared(Some(parent.target_name), field, &mut fields, owner)?
            .ok_or(ClrMameProQueryError::InvalidPositions(owner))?;
        let relationship_id = ClrMameProRelationshipId::from_database(parent.relationship_id)
            .ok_or(ClrMameProQueryError::MismatchedOwner(owner))?;
        children.push(ClrMameProSetChild::Parent(ClrMameProParentReference {
            kind,
            relationship_id,
            target,
        }));
    }
    if !fields.is_empty() {
        return Err(ClrMameProQueryError::InvalidPositions(owner));
    }
    Ok(ClrMameProSet {
        id: CatalogSetId::try_from(owner)?,
        list_order: row.list_order,
        source_block: row
            .source_block
            .ok_or(ClrMameProQueryError::InvalidMetadata(owner))?,
        source_order: order,
        location: positions::location(row.line, row.column, owner)?,
        children,
    })
}

fn attach_media(
    connection: &mut SqliteConnection,
    ids: &[i64],
    sets: &mut [ClrMameProSet],
) -> QueryResult<()> {
    let mut rows = Vec::new();
    for chunk in ids.chunks(400) {
        let mut query =
            sql_query(queries::media(chunk.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in chunk {
            query = query.bind::<BigInt, _>(*id);
        }
        rows.extend(query.load::<MediaRow>(connection)?);
    }
    let mut unique = BTreeSet::new();
    for row in &rows {
        if row.valid != 1
            || row.owner_id <= 0
            || row.occurrence_id <= 0
            || row.occurrence_order < 0
            || !matches!(row.claim_kind.as_str(), "cmp_rom" | "cmp_sample")
            || !unique.insert(row.occurrence_id)
        {
            return Err(ClrMameProQueryError::MismatchedOwner(row.occurrence_id));
        }
    }
    let occurrence_ids = rows
        .iter()
        .map(|row| OccurrenceId::from_database(row.occurrence_id))
        .collect::<Vec<_>>();
    let mut payloads = catalog_files::clrmamepro_file_payloads(connection, &occurrence_ids)?;
    let indices = sets
        .iter()
        .enumerate()
        .map(|(index, set)| (set.id.as_i64(), index))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        let index = indices
            .get(&row.owner_id)
            .ok_or(ClrMameProQueryError::MismatchedOwner(row.owner_id))?;
        let set = sets
            .get_mut(*index)
            .ok_or(ClrMameProQueryError::MismatchedOwner(row.owner_id))?;
        let occurrence_id = OccurrenceId::from_database(row.occurrence_id);
        let payload = payloads
            .remove(&occurrence_id)
            .ok_or(ClrMameProQueryError::MismatchedOwner(row.occurrence_id))?;
        let child = match (row.claim_kind.as_str(), payload) {
            ("cmp_rom", ClrMameProFilePayload::Rom(payload)) => {
                ClrMameProSetChild::Rom(Box::new(ClrMameProRomReference {
                    occurrence_id,
                    occurrence_order: row.occurrence_order,
                    payload: *payload,
                }))
            }
            ("cmp_sample", ClrMameProFilePayload::Sample(payload)) => {
                ClrMameProSetChild::Sample(ClrMameProSampleReference {
                    occurrence_id,
                    occurrence_order: row.occurrence_order,
                    payload,
                })
            }
            _ => return Err(ClrMameProQueryError::MismatchedOwner(row.occurrence_id)),
        };
        set.children.push(child);
    }
    if !payloads.is_empty() {
        return Err(ClrMameProQueryError::MismatchedOwner(0));
    }
    Ok(())
}

fn validate_children(set: &mut ClrMameProSet) -> QueryResult<()> {
    set.children.sort_by_key(ClrMameProSetChild::source_order);
    let mut previous = None;
    let mut media_order = 0_i64;
    for child in &set.children {
        let order = child.source_order();
        if previous == Some(order) {
            return Err(ClrMameProQueryError::InvalidPositions(set.id.as_i64()));
        }
        previous = Some(order);
        let rank = match child {
            ClrMameProSetChild::Rom(rom) => Some(rom.occurrence_order),
            ClrMameProSetChild::Sample(sample) => Some(sample.occurrence_order),
            ClrMameProSetChild::Field { .. } | ClrMameProSetChild::Parent(_) => None,
        };
        if let Some(rank) = rank {
            if rank != media_order {
                return Err(ClrMameProQueryError::InvalidPositions(set.id.as_i64()));
            }
            media_order = media_order
                .checked_add(1)
                .ok_or(ClrMameProQueryError::InvalidMetadata(set.id.as_i64()))?;
        }
    }
    Ok(())
}
