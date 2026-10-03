//! Recognized attribute order from native owners; coordinates and vendor gaps are neutral.
use std::collections::BTreeMap;

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::{
    domain::SnapshotKey,
    logiqx::{
        BiosSetAttribute, ClrMameProAttribute, DiskAttribute, DocumentAttribute, GameAttribute,
        NameAttribute, ReleaseAttribute, RomAttribute, RomCenterAttribute,
    },
};

#[derive(Clone, Copy)]
enum Family {
    Document,
    ClrMamePro,
    RomCenter,
    Game,
    Release,
    Bios,
    Archive,
    Device,
    Rom,
    Disk,
    Sample,
}

impl Family {
    const fn table(self) -> &'static str {
        match self {
            Self::Document => "logiqx_document_attribute_positions",
            Self::ClrMamePro => "logiqx_clrmamepro_attribute_positions",
            Self::RomCenter => "logiqx_romcenter_attribute_positions",
            Self::Game => "logiqx_game_attribute_positions",
            Self::Release => "logiqx_release_attribute_positions",
            Self::Bios => "logiqx_bios_attribute_positions",
            Self::Archive => "logiqx_archive_attribute_positions",
            Self::Device => "logiqx_device_reference_attribute_positions",
            Self::Rom => "logiqx_rom_attribute_positions",
            Self::Disk => "logiqx_disk_attribute_positions",
            Self::Sample => "logiqx_sample_attribute_positions",
        }
    }

    fn field(self, code: i64) -> crate::Result<&'static str> {
        let field = match self {
            Self::Document => DocumentAttribute::from_code(code).map(DocumentAttribute::as_str),
            Self::ClrMamePro => {
                ClrMameProAttribute::from_code(code).map(ClrMameProAttribute::as_str)
            }
            Self::RomCenter => RomCenterAttribute::from_code(code).map(RomCenterAttribute::as_str),
            Self::Game => GameAttribute::from_code(code).map(GameAttribute::as_str),
            Self::Release => ReleaseAttribute::from_code(code).map(ReleaseAttribute::as_str),
            Self::Bios => BiosSetAttribute::from_code(code).map(BiosSetAttribute::as_str),
            Self::Rom => RomAttribute::from_code(code).map(RomAttribute::as_str),
            Self::Disk => DiskAttribute::from_code(code).map(DiskAttribute::as_str),
            Self::Archive | Self::Device | Self::Sample => {
                NameAttribute::from_code(code).map(NameAttribute::as_str)
            }
        };
        field.ok_or_else(|| {
            crate::Error::DatabaseSchema("unknown native Logiqx attribute kind".into())
        })
    }

    const fn ordinal(self) -> &'static str {
        match self {
            Self::Release => "positions.release_order",
            Self::Bios => "positions.bios_order",
            Self::Archive => "positions.archive_order",
            Self::Device => "positions.reference_order",
            _ => "0",
        }
    }
}

#[derive(QueryableByName)]
struct AttributeRow {
    #[diesel(sql_type=BigInt)]
    owner_id: i64,
    #[diesel(sql_type=BigInt)]
    family_order: i64,
    #[diesel(sql_type=BigInt)]
    field_kind: i64,
}

fn layouts(
    rows: Vec<AttributeRow>,
    family: Family,
) -> crate::Result<BTreeMap<(i64, i64), Vec<&'static str>>> {
    let mut layouts = BTreeMap::<(i64, i64), Vec<&'static str>>::new();
    for row in rows {
        layouts
            .entry((row.owner_id, row.family_order))
            .or_default()
            .push(family.field(row.field_kind)?);
    }
    Ok(layouts)
}

#[derive(PartialEq, Eq)]
pub(super) struct DocumentAttributes {
    root: Vec<&'static str>,
    clrmamepro: Vec<&'static str>,
    romcenter: Vec<&'static str>,
}

pub(super) fn load_document(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<DocumentAttributes> {
    let mut load = |family: Family| -> crate::Result<Vec<&'static str>> {
        let rows=sql_query(format!("SELECT 0 AS owner_id,0 AS family_order,field_kind FROM {} WHERE snapshot_key=? ORDER BY source_order",family.table()))
            .bind::<Text,_>(snapshot.as_str()).load::<AttributeRow>(conn)?;
        Ok(layouts(rows, family)?.remove(&(0, 0)).unwrap_or_default())
    };
    Ok(DocumentAttributes {
        root: load(Family::Document)?,
        clrmamepro: load(Family::ClrMamePro)?,
        romcenter: load(Family::RomCenter)?,
    })
}

pub(super) fn load_games(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut result = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for (family, name) in [
        (Family::Game, "game"),
        (Family::Release, "release"),
        (Family::Bios, "biosset"),
        (Family::Archive, "archive"),
        (Family::Device, "device_ref"),
    ] {
        let rows=sql_query(format!("SELECT sets.set_id AS owner_id,{} AS family_order,positions.field_kind FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) JOIN {} AS positions ON positions.set_id=sets.set_id WHERE groups.snapshot_key=? ORDER BY owner_id,family_order,positions.source_order",family.ordinal(),family.table()))
            .bind::<Text,_>(snapshot.as_str()).load::<AttributeRow>(conn)?;
        for ((owner, order), fields) in layouts(rows, family)? {
            result
                .entry(owner)
                .or_default()
                .push(serde_json::json!({"owner":name,"family_order":order,"attributes":fields}));
        }
    }
    Ok(result)
}

pub(super) fn load_media(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut result = BTreeMap::new();
    for family in [Family::Rom, Family::Disk, Family::Sample] {
        let rows=sql_query(format!("SELECT occurrence.occurrence_id AS owner_id,0 AS family_order,positions.field_kind FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id JOIN {} AS positions USING(occurrence_id) WHERE groups.snapshot_key=? ORDER BY owner_id,positions.source_order",family.table()))
            .bind::<Text,_>(snapshot.as_str()).load::<AttributeRow>(conn)?;
        for ((owner, _), fields) in layouts(rows, family)? {
            result.insert(owner, fields.into_iter().map(Into::into).collect());
        }
    }
    Ok(result)
}
