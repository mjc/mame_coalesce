//! Native source literals reuse the shared relationship identity issuer.

use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::{CatalogSetId, SnapshotKey};

use super::reported_relationships::{
    CmpReferenceKind, ReportedReferenceKind, XmlReferenceKind, register,
};
use super::{SnapshotSet, checked_order};

pub(super) fn insert_logiqx(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    set: &SnapshotSet,
) -> crate::Result<()> {
    if let Some(parent) = &set.parent {
        insert_logiqx_link(
            connection,
            snapshot,
            owner,
            XmlReferenceKind::CloneOf,
            parent,
        )?;
    }
    let mut reference_order = 0;
    for dependency in &set.runtime_dependencies {
        let kind = match dependency.source_field.as_str() {
            "romof" => XmlReferenceKind::RomOf,
            "sampleof" => XmlReferenceKind::SampleOf,
            "device_ref" => {
                let relationship = register(
                    connection,
                    snapshot,
                    ReportedReferenceKind::Logiqx(XmlReferenceKind::DeviceReference),
                )?;
                sql_query(
                    "INSERT INTO logiqx_device_references \
                     (set_id,reference_order,source_order,source_line,source_column,target_name,relationship_id) \
                     VALUES (?,?,?,?,?,?,?)",
                )
                .bind::<BigInt, _>(owner.as_i64())
                .bind::<BigInt, _>(checked_order(reference_order, "Logiqx device references")?)
                .bind::<BigInt, _>(dependency.source_order.ok_or_else(|| crate::Error::DatabaseSchema("Logiqx device reference has no source child order".into()))?)
                .bind::<BigInt, _>(dependency.location.line)
                .bind::<BigInt, _>(dependency.location.column)
                .bind::<Text, _>(&dependency.target_name)
                .bind::<BigInt, _>(relationship.database_value())
                .execute(connection)?;
                reference_order += 1;
                continue;
            }
            field => {
                return Err(crate::Error::DatabaseSchema(format!(
                    "unknown Logiqx reference {field}"
                )));
            }
        };
        insert_logiqx_link(connection, snapshot, owner, kind, &dependency.target_name)?;
    }
    Ok(())
}

fn insert_logiqx_link(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    kind: XmlReferenceKind,
    literal: &str,
) -> crate::Result<()> {
    let relationship = register(connection, snapshot, ReportedReferenceKind::Logiqx(kind))?;
    sql_query(
        "INSERT INTO logiqx_set_links \
         (set_id,link_kind,target_name,relationship_id) VALUES (?,?,?,?)",
    )
    .bind::<BigInt, _>(owner.as_i64())
    .bind::<Text, _>(kind.field())
    .bind::<Text, _>(literal)
    .bind::<BigInt, _>(relationship.database_value())
    .execute(connection)?;
    Ok(())
}

pub(super) fn insert_cmp_parents(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    facts: &crate::clrmamepro::SetFacts,
) -> crate::Result<()> {
    for (kind, field, literal) in [
        (CmpReferenceKind::CloneOf, "cloneof", &facts.cloneof),
        (CmpReferenceKind::SampleOf, "sampleof", &facts.sampleof),
    ] {
        let Some(literal) = literal else {
            continue;
        };
        let relationship = register(
            connection,
            snapshot,
            ReportedReferenceKind::ClrMamePro(kind),
        )?;
        sql_query(
            "INSERT INTO clrmamepro_set_links(set_id,link_kind,target_name,relationship_id) VALUES (?,?,?,?)",
        )
        .bind::<BigInt, _>(owner.as_i64())
        .bind::<Text, _>(field)
        .bind::<Text, _>(&literal.value)
        .bind::<BigInt, _>(relationship.database_value())
        .execute(connection)?;
    }
    Ok(())
}
