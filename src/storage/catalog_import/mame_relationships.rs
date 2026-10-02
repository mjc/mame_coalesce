//! Native MAME declarations own literals; the shared registry owns review identity.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::{CatalogSetId, OccurrenceId, RelationshipAssertionKey, SnapshotKey};

use super::{NativeAssetFacts, SnapshotAsset, SnapshotSet, checked_order};

#[derive(Clone, Copy)]
enum ReferenceKind {
    CloneOf,
    RomOf,
    SampleOf,
    DeviceReference,
    RomMerge,
    DiskMerge,
}

impl ReferenceKind {
    const fn code(self) -> &'static str {
        match self {
            Self::CloneOf => "mame_cloneof",
            Self::RomOf => "mame_romof",
            Self::SampleOf => "mame_sampleof",
            Self::DeviceReference => "mame_device_ref",
            Self::RomMerge => "mame_rom_merge",
            Self::DiskMerge => "mame_disk_merge",
        }
    }

    const fn field(self) -> &'static str {
        match self {
            Self::CloneOf => "cloneof",
            Self::RomOf => "romof",
            Self::SampleOf => "sampleof",
            Self::DeviceReference => "device_ref",
            Self::RomMerge | Self::DiskMerge => "merge",
        }
    }
}

pub(super) fn insert_asset_merge(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    occurrence: OccurrenceId,
    asset: &SnapshotAsset,
) -> crate::Result<()> {
    if !matches!(asset.native, NativeAssetFacts::Mame { .. }) {
        return Ok(());
    }
    let Some(literal) = asset.merge.as_deref() else {
        return Ok(());
    };
    let (kind, table) = match asset.role {
        "rom" => (ReferenceKind::RomMerge, "mame_rom_merges"),
        "disk" => (ReferenceKind::DiskMerge, "mame_disk_merges"),
        role => {
            return Err(crate::Error::DatabaseSchema(format!(
                "invalid MAME merge media kind {role}"
            )));
        }
    };
    let relationship = register(connection, snapshot, kind)?;
    sql_query(format!(
        "INSERT INTO {table}(occurrence_id,relationship_id,merge_name,source_line,source_column) \
         VALUES (?,?,?,?,?)",
    ))
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(relationship.0)
    .bind::<Text, _>(literal)
    .bind::<BigInt, _>(asset.location.line)
    .bind::<BigInt, _>(asset.location.column)
    .execute(connection)?;
    Ok(())
}

/// Only issued after both registry identity and its reported subtype are stored.
struct ReportedRelationshipId(i64);

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
}

fn register(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kind: ReferenceKind,
) -> crate::Result<ReportedRelationshipId> {
    let key = RelationshipAssertionKey::fresh();
    let row = sql_query(
        "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) \
         VALUES (?,'source',?) RETURNING relationship_id",
    )
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<IdRow>(connection)?;
    sql_query(
        "INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) \
         VALUES (?,?)",
    )
    .bind::<BigInt, _>(row.relationship_id)
    .bind::<Text, _>(kind.code())
    .execute(connection)?;
    Ok(ReportedRelationshipId(row.relationship_id))
}

fn insert_link(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    kind: ReferenceKind,
    target: &str,
    location: crate::logiqx::RecordLocation,
) -> crate::Result<()> {
    let relationship = register(connection, snapshot, kind)?;
    sql_query(
        "INSERT INTO mame_machine_links \
         (set_id,link_kind,target_name,relationship_id,source_line,source_column) \
         VALUES (?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(owner.as_i64())
    .bind::<Text, _>(kind.field())
    .bind::<Text, _>(target)
    .bind::<BigInt, _>(relationship.0)
    .bind::<BigInt, _>(location.line)
    .bind::<BigInt, _>(location.column)
    .execute(connection)?;
    Ok(())
}

pub(super) fn insert(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    set: &SnapshotSet,
) -> crate::Result<()> {
    if let Some(parent) = &set.parent {
        insert_link(
            connection,
            snapshot,
            owner,
            ReferenceKind::CloneOf,
            parent,
            set.location,
        )?;
    }
    let mut reference_order = 0_usize;
    for dependency in &set.machine_dependencies {
        match dependency.source_field.as_str() {
            "romof" | "sampleof" => {
                let kind = if dependency.source_field == "romof" {
                    ReferenceKind::RomOf
                } else {
                    ReferenceKind::SampleOf
                };
                insert_link(
                    connection,
                    snapshot,
                    owner,
                    kind,
                    &dependency.target_name,
                    dependency.location,
                )?;
            }
            "device_ref" => {
                let tag = dependency.reference_tag.as_deref().ok_or_else(|| {
                    crate::Error::DatabaseSchema("native device reference requires its tag".into())
                })?;
                let source_order = dependency.source_order.ok_or_else(|| {
                    crate::Error::DatabaseSchema(
                        "native device reference requires its source order".into(),
                    )
                })?;
                let relationship = register(connection, snapshot, ReferenceKind::DeviceReference)?;
                sql_query(
                    "INSERT INTO mame_device_references \
                     (set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) \
                     VALUES (?,?,?,?,?,?,?,?)",
                )
                .bind::<BigInt, _>(owner.as_i64())
                .bind::<BigInt, _>(checked_order(reference_order, "device references")?)
                .bind::<Text, _>(&dependency.target_name)
                .bind::<Text, _>(tag)
                .bind::<BigInt, _>(source_order)
                .bind::<BigInt, _>(relationship.0)
                .bind::<BigInt, _>(dependency.location.line)
                .bind::<BigInt, _>(dependency.location.column)
                .execute(connection)?;
                reference_order += 1;
            }
            kind => {
                return Err(crate::Error::DatabaseSchema(format!(
                    "unknown native MAME reference {kind}"
                )));
            }
        }
    }
    Ok(())
}
