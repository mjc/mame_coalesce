//! Native MAME declarations own literals; the shared registry owns review identity.

use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::{CatalogSetId, SnapshotKey};

use super::reported_relationships::{
    self, ReportedReferenceKind, XmlReferenceKind as ReferenceKind,
};
use super::{SnapshotSet, checked_order};

fn register(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kind: ReferenceKind,
) -> crate::Result<reported_relationships::ReportedRelationshipId> {
    reported_relationships::register(connection, snapshot, ReportedReferenceKind::Mame(kind))
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
    .bind::<BigInt, _>(relationship.database_value())
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
                .bind::<BigInt, _>(relationship.database_value())
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
