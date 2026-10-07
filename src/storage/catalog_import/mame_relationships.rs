//! Native MAME declarations own literals; the shared registry owns review identity.

use diesel::{
    SqliteConnection,
    query_builder::{AstPass, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_types::{BigInt, Text},
    sqlite::Sqlite,
};

use crate::domain::{CatalogSetId, SnapshotKey};
use crate::storage::cached_sql::{InsertBatch, InsertPhase, cached_sql};

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

struct PendingReference {
    owner: i64,
    kind: ReferenceKind,
    target: String,
    tag: Option<String>,
    source_order: Option<i64>,
    reference_order: Option<i64>,
    location: crate::logiqx::RecordLocation,
}

impl PendingReference {
    fn byte_size(&self) -> usize {
        self.target
            .len()
            .saturating_add(self.tag.as_ref().map_or(0, String::len))
    }
}

fn insert_reference_batch(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    references: &[PendingReference],
) -> crate::Result<()> {
    if references.is_empty() {
        return Ok(());
    }

    let kinds = references
        .iter()
        .map(|reference| ReportedReferenceKind::Mame(reference.kind))
        .collect::<Vec<_>>();
    let relationships = reported_relationships::register_bulk(connection, snapshot, &kinds)?;
    let mut native_batch = InsertBatch::new();
    for (reference, relationship) in references.iter().zip(&relationships) {
        match reference.kind {
            ReferenceKind::DeviceReference => {
                let tag = reference.tag.as_deref().ok_or_else(|| {
                    crate::Error::DatabaseSchema("native device reference requires its tag".into())
                })?;
                let source_order = reference.source_order.ok_or_else(|| {
                    crate::Error::DatabaseSchema(
                        "native device reference requires its source order".into(),
                    )
                })?;
                let reference_order = reference.reference_order.ok_or_else(|| {
                    crate::Error::DatabaseSchema(
                        "native device reference requires its reference order".into(),
                    )
                })?;
                cached_sql(
                    "INSERT INTO mame_device_references(set_id,reference_order,name,tag,source_order,relationship_id,source_line,source_column) VALUES (?,?,?,?,?,?,?,?)",
                )
                .bind::<diesel::sql_types::BigInt, _>(reference.owner)
                .bind::<diesel::sql_types::BigInt, _>(reference_order)
                .bind::<diesel::sql_types::Text, _>(&reference.target)
                .bind::<diesel::sql_types::Text, _>(tag)
                .bind::<diesel::sql_types::BigInt, _>(source_order)
                .bind::<diesel::sql_types::BigInt, _>(relationship.database_value())
                .bind::<diesel::sql_types::BigInt, _>(reference.location.line)
                .bind::<diesel::sql_types::BigInt, _>(reference.location.column)
                .enqueue(&mut native_batch, connection, InsertPhase::Children)?;
            }
            kind => {
                cached_sql(
                    "INSERT INTO mame_machine_links(set_id,link_kind,target_name,relationship_id,source_line,source_column) VALUES (?,?,?,?,?,?)",
                )
                .bind::<diesel::sql_types::BigInt, _>(reference.owner)
                .bind::<diesel::sql_types::Text, _>(kind.field())
                .bind::<diesel::sql_types::Text, _>(&reference.target)
                .bind::<diesel::sql_types::BigInt, _>(relationship.database_value())
                .bind::<diesel::sql_types::BigInt, _>(reference.location.line)
                .bind::<diesel::sql_types::BigInt, _>(reference.location.column)
                .enqueue(&mut native_batch, connection, InsertPhase::Children)?;
            }
        }
    }
    native_batch.flush(connection)?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(super) fn insert_many(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    sets: &[(CatalogSetId, &SnapshotSet)],
) -> crate::Result<()> {
    const MAX_ROWS: usize = 64;
    const MAX_BYTES: usize = 1024 * 1024;

    let mut pending = Vec::new();
    let mut bytes = 0_usize;
    for (owner, set) in sets {
        if let Some(parent) = &set.parent {
            let reference = PendingReference {
                owner: owner.as_i64(),
                kind: ReferenceKind::CloneOf,
                target: parent.clone(),
                tag: None,
                source_order: None,
                reference_order: None,
                location: set.location,
            };
            bytes = bytes.saturating_add(reference.byte_size());
            pending.push(reference);
        }
        let mut reference_order = 0_usize;
        for dependency in &set.machine_dependencies {
            let kind = match dependency.source_field.as_str() {
                "romof" => ReferenceKind::RomOf,
                "sampleof" => ReferenceKind::SampleOf,
                "device_ref" => ReferenceKind::DeviceReference,
                kind => {
                    return Err(crate::Error::DatabaseSchema(format!(
                        "unknown native MAME reference {kind}"
                    )));
                }
            };
            let is_device_reference = matches!(kind, ReferenceKind::DeviceReference);
            let reference = PendingReference {
                owner: owner.as_i64(),
                kind,
                target: dependency.target_name.clone(),
                tag: dependency.reference_tag.clone(),
                source_order: dependency.source_order,
                reference_order: if is_device_reference {
                    Some(checked_order(reference_order, "device references")?)
                } else {
                    None
                },
                location: dependency.location,
            };
            if is_device_reference {
                if reference.tag.is_none() {
                    return Err(crate::Error::DatabaseSchema(
                        "native device reference requires its tag".into(),
                    ));
                }
                if reference.source_order.is_none() {
                    return Err(crate::Error::DatabaseSchema(
                        "native device reference requires its source order".into(),
                    ));
                }
                reference_order += 1;
            }
            bytes = bytes.saturating_add(reference.byte_size());
            pending.push(reference);

            if pending.len() >= MAX_ROWS || bytes >= MAX_BYTES {
                insert_reference_batch(connection, snapshot, &pending)?;
                pending.clear();
                bytes = 0;
            }
        }
        if pending.len() >= MAX_ROWS || bytes >= MAX_BYTES {
            insert_reference_batch(connection, snapshot, &pending)?;
            pending.clear();
            bytes = 0;
        }
    }
    insert_reference_batch(connection, snapshot, &pending)
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

#[cfg(test)]
mod bulk_tests {
    use std::sync::{Arc, Mutex};

    use diesel::{
        Connection, RunQueryDsl, SqliteConnection,
        connection::{InstrumentationEvent, SimpleConnection},
        sql_query,
        sql_types::BigInt,
    };

    use super::insert_many;
    use crate::{
        domain::{CatalogSetId, SnapshotKey},
        storage::catalog_import::{SnapshotDependency, SnapshotSet},
    };

    #[derive(diesel::QueryableByName)]
    struct ReferenceCounts {
        #[diesel(sql_type = BigInt)]
        devices: i64,
        #[diesel(sql_type = BigInt)]
        links: i64,
        #[diesel(sql_type = BigInt)]
        reported: i64,
        #[diesel(sql_type = BigInt)]
        mapped_devices: i64,
        #[diesel(sql_type = BigInt)]
        mapped_links: i64,
    }

    fn set(name: &str, index: i64) -> SnapshotSet {
        let mut machine_dependencies = (0..120)
            .map(|order| SnapshotDependency {
                source_field: "device_ref".into(),
                target_name: format!("device-{index}-{order}"),
                reference_tag: Some(format!("tag-{index}-{order}")),
                source_order: Some(order),
                location: crate::logiqx::RecordLocation {
                    line: order + 1,
                    column: index + 1,
                },
                attribute_positions: Vec::new(),
            })
            .collect::<Vec<_>>();
        machine_dependencies.extend([
            SnapshotDependency {
                source_field: "romof".into(),
                target_name: format!("rom-parent-{index}"),
                reference_tag: None,
                source_order: None,
                location: crate::logiqx::RecordLocation {
                    line: 500,
                    column: index + 1,
                },
                attribute_positions: Vec::new(),
            },
            SnapshotDependency {
                source_field: "sampleof".into(),
                target_name: String::new(),
                reference_tag: None,
                source_order: None,
                location: crate::logiqx::RecordLocation {
                    line: 501,
                    column: index + 1,
                },
                attribute_positions: Vec::new(),
            },
        ]);
        SnapshotSet {
            name: name.into(),
            parent: Some(format!("clone-parent-{index}")),
            runtime_dependencies: Vec::new(),
            location: crate::logiqx::RecordLocation {
                line: index + 1,
                column: 1,
            },
            assets: Vec::new(),
            switches: Vec::new(),
            bios_sets: Vec::new(),
            specification: Vec::new(),
            mame_facts: None,
            no_intro_facts: None,
            logiqx_facts: None,
            logiqx_details: None,
            cmp_facts: None,
            machine_dependencies,
        }
    }

    #[expect(
        clippy::expect_used,
        reason = "statement-count instrumentation must retain every observed insert"
    )]
    fn connection_with_insert_counter() -> crate::Result<(SqliteConnection, Arc<Mutex<Vec<String>>>)>
    {
        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite test database");
        connection.batch_execute(
            "CREATE TABLE catalog_relationships(relationship_id INTEGER PRIMARY KEY AUTOINCREMENT, assertion_key TEXT, origin TEXT, snapshot_key TEXT);
             CREATE TABLE reported_catalog_relationships(relationship_id INTEGER PRIMARY KEY, source_reference_kind TEXT);
             CREATE TABLE mame_machine_links(set_id INTEGER, link_kind TEXT, target_name TEXT, relationship_id INTEGER, source_line INTEGER, source_column INTEGER, PRIMARY KEY(set_id,link_kind));
             CREATE TABLE mame_device_references(set_id INTEGER, reference_order INTEGER, name TEXT, tag TEXT, source_order INTEGER, relationship_id INTEGER, source_line INTEGER, source_column INTEGER, PRIMARY KEY(set_id,reference_order));",
        )?;
        let statements = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed = Arc::clone(&statements);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                let sql = query.to_string();
                if sql.starts_with("INSERT INTO catalog_relationships")
                    || sql.starts_with("INSERT INTO reported_catalog_relationships")
                    || sql.starts_with("INSERT INTO mame_machine_links")
                    || sql.starts_with("INSERT INTO mame_device_references")
                {
                    observed.lock().expect("statement count").push(sql);
                }
            }
        });
        Ok((connection, statements))
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "statement-count fixture and assertions must fail loudly"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "keep batching, owner-tail fidelity and legacy equivalence assertions in one scenario"
    )]
    fn many_references_batch_across_owners_and_keep_tail_and_native_facts() -> crate::Result<()> {
        #[derive(diesel::QueryableByName)]
        struct OwnerTail {
            #[diesel(sql_type = diesel::sql_types::Text)]
            name: String,
            #[diesel(sql_type = BigInt)]
            source_order: i64,
            #[diesel(sql_type = diesel::sql_types::Text)]
            tag: String,
            #[diesel(sql_type = BigInt)]
            source_line: i64,
            #[diesel(sql_type = BigInt)]
            source_column: i64,
        }
        #[derive(diesel::QueryableByName)]
        struct TextValue {
            #[diesel(sql_type = diesel::sql_types::Text)]
            value: String,
        }

        let (mut connection, statements) = connection_with_insert_counter()?;

        let first = set("first", 0);
        let second = set("second", 1);
        let sets = [
            (CatalogSetId::try_from(1)?, &first),
            (CatalogSetId::try_from(2)?, &second),
        ];
        insert_many(
            &mut connection,
            &SnapshotKey::from_persisted("snapshot-test".into()),
            &sets,
        )?;

        let counts = sql_query(
            "SELECT (SELECT count(*) FROM mame_device_references) AS devices,
                    (SELECT count(*) FROM mame_machine_links) AS links,
                    (SELECT count(*) FROM reported_catalog_relationships) AS reported,
                    (SELECT count(*) FROM mame_device_references AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_device_ref') AS mapped_devices,
                    (SELECT count(*) FROM mame_machine_links AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_' || native.link_kind) AS mapped_links",
        )
        .get_result::<ReferenceCounts>(&mut connection)?;
        let summarize = |counts: &ReferenceCounts| {
            (
                counts.devices,
                counts.links,
                counts.reported,
                counts.mapped_devices,
                counts.mapped_links,
            )
        };
        assert_eq!(summarize(&counts), (240, 6, 246, 240, 6));

        let tails = sql_query(
            "SELECT name,source_order,tag,source_line,source_column FROM mame_device_references
             WHERE (set_id,reference_order) IN ((1,119),(2,0)) ORDER BY set_id",
        )
        .load::<OwnerTail>(&mut connection)?;
        assert_eq!(
            tails
                .into_iter()
                .map(|row| (
                    row.name,
                    row.source_order,
                    row.tag,
                    row.source_line,
                    row.source_column,
                ))
                .collect::<Vec<_>>(),
            vec![
                ("device-0-119".into(), 119, "tag-0-119".into(), 120, 1),
                ("device-1-0".into(), 0, "tag-1-0".into(), 1, 2),
            ]
        );
        let empty_target = sql_query(
            "SELECT target_name AS value FROM mame_machine_links WHERE set_id=2 AND link_kind='sampleof'",
        )
        .get_result::<TextValue>(&mut connection)?;
        assert_eq!(empty_target.value, "");

        let statements = statements.lock().expect("statement count");
        let bulk_statement_count = statements.len();
        assert!(
            bulk_statement_count <= 20,
            "bulk references should use bounded multirow statements, got {bulk_statement_count}: {statements:?}"
        );
        assert!(
            statements
                .iter()
                .any(|sql| sql.starts_with("INSERT INTO catalog_relationships")
                    && sql.contains("RETURNING assertion_key,relationship_id")),
            "registry identities must be returned with assertion keys"
        );
        assert!(
            statements.iter().all(|sql| sql.matches('?').count() <= 999),
            "every generated multirow statement must stay within SQLite's bind limit"
        );
        let rows_per_registry_insert = statements
            .iter()
            .filter(|sql| sql.starts_with("INSERT INTO catalog_relationships"))
            .map(|sql| sql.matches("),(").count() + 1)
            .collect::<Vec<_>>();
        assert!(rows_per_registry_insert.iter().all(|rows| *rows <= 64));

        drop(statements);
        let (mut legacy_connection, legacy_statements) = connection_with_insert_counter()?;
        let snapshot = SnapshotKey::from_persisted("snapshot-test".into());
        for (owner, set) in &sets {
            super::insert(&mut legacy_connection, &snapshot, *owner, set)?;
        }
        let legacy_counts = sql_query(
            "SELECT (SELECT count(*) FROM mame_device_references) AS devices,
                    (SELECT count(*) FROM mame_machine_links) AS links,
                    (SELECT count(*) FROM reported_catalog_relationships) AS reported,
                    (SELECT count(*) FROM mame_device_references AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_device_ref') AS mapped_devices,
                    (SELECT count(*) FROM mame_machine_links AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_' || native.link_kind) AS mapped_links",
        )
        .get_result::<ReferenceCounts>(&mut legacy_connection)?;
        assert_eq!(summarize(&legacy_counts), summarize(&counts));

        let legacy_statement_count = legacy_statements.lock().expect("statement count").len();
        assert_eq!(
            legacy_statement_count,
            usize::try_from(counts.reported).expect("reported count fits in usize") * 3
        );
        assert!(
            bulk_statement_count < legacy_statement_count,
            "bulk path ({bulk_statement_count}) must execute fewer insert statements than the legacy path ({legacy_statement_count})"
        );
        Ok(())
    }
}
