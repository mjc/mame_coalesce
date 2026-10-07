//! Native MAME declarations own literals; the shared registry owns review identity.

use diesel::{
    SqliteConnection,
    query_builder::{AstPass, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_types::{BigInt, Text},
    sqlite::Sqlite,
};

use crate::domain::{CatalogSetId, SnapshotKey};

use super::reported_relationships::{
    self, ReportedReferenceKind, XmlReferenceKind as ReferenceKind,
};
use super::{SnapshotSet, checked_order};

struct MachineLinkQuery;

struct MachineLinkInsert<'a> {
    set_id: i64,
    kind: &'a str,
    target_name: &'a str,
    relationship_id: i64,
    source_line: i64,
    source_column: i64,
}

impl QueryId for MachineLinkInsert<'_> {
    type QueryId = MachineLinkQuery;
}

impl QueryFragment<Sqlite> for MachineLinkInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO mame_machine_links \
             (set_id,link_kind,target_name,relationship_id,source_line,source_column) VALUES (",
        );
        pass.push_bind_param::<BigInt, _>(&self.set_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.kind)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.target_name)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.relationship_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_line)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_column)?;
        pass.push_sql(")");
        Ok(())
    }
}

struct DeviceReferenceQuery;

struct DeviceReferenceInsert<'a> {
    set_id: i64,
    reference_order: i64,
    name: &'a str,
    tag: &'a str,
    source_order: i64,
    relationship_id: i64,
    source_line: i64,
    source_column: i64,
}

impl QueryId for DeviceReferenceInsert<'_> {
    type QueryId = DeviceReferenceQuery;
}

impl QueryFragment<Sqlite> for DeviceReferenceInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO mame_device_references \
             (set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) VALUES (",
        );
        pass.push_bind_param::<BigInt, _>(&self.set_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.reference_order)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.name)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.tag)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_order)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.relationship_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_line)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_column)?;
        pass.push_sql(")");
        Ok(())
    }
}

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
    ExecuteDsl::execute(
        MachineLinkInsert {
            set_id: owner.as_i64(),
            kind: kind.field(),
            target_name: target,
            relationship_id: relationship.database_value(),
            source_line: location.line,
            source_column: location.column,
        },
        connection,
    )?;
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
                ExecuteDsl::execute(
                    DeviceReferenceInsert {
                        set_id: owner.as_i64(),
                        reference_order: checked_order(reference_order, "device references")?,
                        name: &dependency.target_name,
                        tag,
                        source_order,
                        relationship_id: relationship.database_value(),
                        source_line: dependency.location.line,
                        source_column: dependency.location.column,
                    },
                    connection,
                )?;
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
