//! One identity issuer for source declarations; native tables own their literals.

use diesel::{
    RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_query,
    sql_types::{BigInt, Text},
    sqlite::Sqlite,
};
use std::collections::HashMap;

use crate::domain::{
    CatalogSetId, MergeMediaKind, NoIntroArchiveId, NoIntroArchiveReferenceField, OccurrenceId,
    RelationshipAssertionKey, SnapshotKey,
};

use super::{NativeAssetFacts, SnapshotAsset};
use crate::storage::cached_sql::{InsertBatch, InsertPhase, cached_sql};

#[derive(Clone, Copy)]
pub(super) enum XmlReferenceKind {
    CloneOf,
    RomOf,
    SampleOf,
    DeviceReference,
    RomMerge,
    DiskMerge,
}

impl XmlReferenceKind {
    pub(super) const fn field(self) -> &'static str {
        match self {
            Self::CloneOf => "cloneof",
            Self::RomOf => "romof",
            Self::SampleOf => "sampleof",
            Self::DeviceReference => "device_ref",
            Self::RomMerge | Self::DiskMerge => "merge",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum CmpReferenceKind {
    CloneOf,
    SampleOf,
    RomMerge,
}

#[derive(Clone, Copy)]
pub(super) enum ReportedReferenceKind {
    Mame(XmlReferenceKind),
    Logiqx(XmlReferenceKind),
    ClrMamePro(CmpReferenceKind),
    SoftwareClone,
    NoIntroDat(DatParentKind),
    NoIntroDatabase(NoIntroArchiveReferenceField),
    NoIntroPc(NoIntroArchiveReferenceField),
}

#[derive(Clone, Copy)]
pub(super) enum DatParentKind {
    Name,
    PublisherId,
}

impl DatParentKind {
    const fn field(self) -> &'static str {
        match self {
            Self::Name => "cloneof",
            Self::PublisherId => "cloneofid",
        }
    }
}

impl ReportedReferenceKind {
    const fn code(self) -> &'static str {
        use XmlReferenceKind as Xml;
        match self {
            Self::Mame(Xml::CloneOf) => "mame_cloneof",
            Self::Mame(Xml::RomOf) => "mame_romof",
            Self::Mame(Xml::SampleOf) => "mame_sampleof",
            Self::Mame(Xml::DeviceReference) => "mame_device_ref",
            Self::Mame(Xml::RomMerge) => "mame_rom_merge",
            Self::Mame(Xml::DiskMerge) => "mame_disk_merge",
            Self::Logiqx(Xml::CloneOf) => "logiqx_cloneof",
            Self::Logiqx(Xml::RomOf) => "logiqx_romof",
            Self::Logiqx(Xml::SampleOf) => "logiqx_sampleof",
            Self::Logiqx(Xml::DeviceReference) => "logiqx_device_ref",
            Self::Logiqx(Xml::RomMerge) => "logiqx_rom_merge",
            Self::Logiqx(Xml::DiskMerge) => "logiqx_disk_merge",
            Self::ClrMamePro(CmpReferenceKind::CloneOf) => "clrmamepro_cloneof",
            Self::ClrMamePro(CmpReferenceKind::SampleOf) => "clrmamepro_sampleof",
            Self::ClrMamePro(CmpReferenceKind::RomMerge) => "clrmamepro_rom_merge",
            Self::SoftwareClone => "software_cloneof",
            Self::NoIntroDat(DatParentKind::Name) => "no_intro_dat_cloneof",
            Self::NoIntroDat(DatParentKind::PublisherId) => "no_intro_dat_cloneofid",
            Self::NoIntroDatabase(NoIntroArchiveReferenceField::Clone) => {
                "no_intro_database_archive_clone"
            }
            Self::NoIntroDatabase(NoIntroArchiveReferenceField::MergeOf) => {
                "no_intro_database_archive_mergeof"
            }
            Self::NoIntroPc(NoIntroArchiveReferenceField::Clone) => "no_intro_pc_clone",
            Self::NoIntroPc(NoIntroArchiveReferenceField::MergeOf) => "no_intro_pc_mergeof",
        }
    }
}

/// Native reference owners which derive provenance from their existing source rows.
#[derive(Clone, Copy)]
pub(super) enum ReferenceOwner {
    Software(CatalogSetId),
    NoIntroDat {
        set: CatalogSetId,
        kind: DatParentKind,
    },
    NoIntroArchive {
        archive: NoIntroArchiveId,
        field: NoIntroArchiveReferenceField,
    },
    NoIntroPc {
        set: CatalogSetId,
        field: NoIntroArchiveReferenceField,
    },
}

impl ReferenceOwner {
    const fn reference_kind(self) -> ReportedReferenceKind {
        match self {
            Self::Software(_) => ReportedReferenceKind::SoftwareClone,
            Self::NoIntroDat { kind, .. } => ReportedReferenceKind::NoIntroDat(kind),
            Self::NoIntroArchive { field, .. } => ReportedReferenceKind::NoIntroDatabase(field),
            Self::NoIntroPc { field, .. } => ReportedReferenceKind::NoIntroPc(field),
        }
    }

    const fn insert_query(self) -> &'static str {
        match self {
            Self::Software(_) => {
                "INSERT INTO software_clone_links(set_id,relationship_id,target_name) VALUES (?,?,?)"
            }
            Self::NoIntroDat { .. } => {
                "INSERT INTO no_intro_dat_set_links(set_id,relationship_id,target_literal,link_kind) VALUES (?,?,?,?)"
            }
            Self::NoIntroArchive {
                field: NoIntroArchiveReferenceField::Clone,
                ..
            } => {
                "INSERT INTO no_intro_archive_clone_links(archive_id,relationship_id,declared_target_number) VALUES (?,?,?)"
            }
            Self::NoIntroArchive {
                field: NoIntroArchiveReferenceField::MergeOf,
                ..
            } => {
                "INSERT INTO no_intro_archive_merge_links(archive_id,relationship_id,declared_mergeof) VALUES (?,?,?)"
            }
            Self::NoIntroPc {
                field: NoIntroArchiveReferenceField::Clone,
                ..
            } => {
                "INSERT INTO no_intro_pc_clone_links(set_id,relationship_id,target_archive_id) VALUES (?,?,?)"
            }
            Self::NoIntroPc {
                field: NoIntroArchiveReferenceField::MergeOf,
                ..
            } => {
                "INSERT INTO no_intro_pc_merge_links(set_id,relationship_id,target_archive_id) VALUES (?,?,?)"
            }
        }
    }

    const fn owner_id(self) -> i64 {
        match self {
            Self::Software(set) | Self::NoIntroDat { set, .. } | Self::NoIntroPc { set, .. } => {
                set.as_i64()
            }
            Self::NoIntroArchive { archive, .. } => archive.as_i64(),
        }
    }
}

pub(super) fn insert_reference(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: ReferenceOwner,
    literal: &str,
) -> crate::Result<()> {
    let relationship = register(connection, snapshot, owner.reference_kind())?;
    let query = sql_query(owner.insert_query())
        .bind::<BigInt, _>(owner.owner_id())
        .bind::<BigInt, _>(relationship.database_value())
        .bind::<Text, _>(literal);
    match owner {
        ReferenceOwner::Software(_) | ReferenceOwner::NoIntroArchive { .. } => {
            query.execute(connection)?
        }
        ReferenceOwner::NoIntroDat { kind, .. } => {
            query.bind::<Text, _>(kind.field()).execute(connection)?
        }
        ReferenceOwner::NoIntroPc { .. } => query.execute(connection)?,
    };
    Ok(())
}

/// Cannot be constructed until registry identity and reported subtype both exist.
pub(super) struct ReportedRelationshipId(i64);

impl ReportedRelationshipId {
    pub(super) const fn database_value(&self) -> i64 {
        self.0
    }
}

struct InsertCatalogRelationshipQuery;
struct InsertReportedRelationshipQuery;
struct InsertCatalogRelationshipsBulkQuery;

#[derive(diesel::Queryable)]
struct RegisteredRelationshipRow {
    assertion_key: String,
    relationship_id: i64,
}

struct InsertCatalogRelationshipsBulk<'a> {
    rows: &'a [(String, &'a str)],
}

impl QueryId for InsertCatalogRelationshipsBulk<'_> {
    type QueryId = InsertCatalogRelationshipsBulkQuery;
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl Query for InsertCatalogRelationshipsBulk<'_> {
    type SqlType = (Text, BigInt);
}

impl RunQueryDsl<SqliteConnection> for InsertCatalogRelationshipsBulk<'_> {}

impl QueryFragment<Sqlite> for InsertCatalogRelationshipsBulk<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) VALUES ",
        );
        for (index, (key, snapshot_key)) in self.rows.iter().enumerate() {
            if index != 0 {
                pass.push_sql(",");
            }
            pass.push_sql("(");
            pass.push_bind_param::<Text, _>(key)?;
            pass.push_sql(",'source',");
            pass.push_bind_param::<Text, _>(snapshot_key)?;
            pass.push_sql(")");
        }
        pass.push_sql(" RETURNING assertion_key,relationship_id");
        Ok(())
    }
}

struct InsertCatalogRelationship<'a> {
    assertion_key: &'a str,
    snapshot_key: &'a str,
}

impl QueryId for InsertCatalogRelationship<'_> {
    type QueryId = InsertCatalogRelationshipQuery;
}

impl Query for InsertCatalogRelationship<'_> {
    type SqlType = BigInt;
}

impl RunQueryDsl<SqliteConnection> for InsertCatalogRelationship<'_> {}

impl QueryFragment<Sqlite> for InsertCatalogRelationship<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) VALUES (",
        );
        pass.push_bind_param::<Text, _>(&self.assertion_key)?;
        pass.push_sql(",'source',");
        pass.push_bind_param::<Text, _>(&self.snapshot_key)?;
        pass.push_sql(") RETURNING relationship_id");
        Ok(())
    }
}

struct InsertReportedRelationship<'a> {
    relationship_id: i64,
    source_reference_kind: &'a str,
}

impl QueryId for InsertReportedRelationship<'_> {
    type QueryId = InsertReportedRelationshipQuery;
}

impl RunQueryDsl<SqliteConnection> for InsertReportedRelationship<'_> {}

impl QueryFragment<Sqlite> for InsertReportedRelationship<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (",
        );
        pass.push_bind_param::<BigInt, _>(&self.relationship_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.source_reference_kind)?;
        pass.push_sql(")");
        Ok(())
    }
}

pub(super) fn register(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kind: ReportedReferenceKind,
) -> crate::Result<ReportedRelationshipId> {
    let key = RelationshipAssertionKey::fresh();
    let row = InsertCatalogRelationship {
        assertion_key: key.as_str(),
        snapshot_key: snapshot.as_str(),
    }
    .get_result::<i64>(connection)?;
    InsertReportedRelationship {
        relationship_id: row,
        source_reference_kind: kind.code(),
    }
    .execute(connection)?;
    Ok(ReportedRelationshipId(row))
}

/// Allocate registry identities in input order without relying on SQLite's
/// `RETURNING` row order. Callers pass bounded slices to keep retained state
/// proportional to one insert batch.
pub(super) fn register_bulk(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kinds: &[ReportedReferenceKind],
) -> crate::Result<Vec<ReportedRelationshipId>> {
    const MAX_REGISTRY_ROWS: usize = 64;
    const MAX_REGISTRY_BYTES: usize = 1024 * 1024;

    let estimated_row_bytes = snapshot.as_str().len().saturating_add(64);
    let rows_by_bytes = (MAX_REGISTRY_BYTES / estimated_row_bytes.max(1)).max(1);
    let chunk_size = MAX_REGISTRY_ROWS.min(rows_by_bytes);
    let mut ids = Vec::with_capacity(kinds.len());
    for chunk in kinds.chunks(chunk_size) {
        ids.extend(register_bulk_chunk(connection, snapshot, chunk)?);
    }
    Ok(ids)
}

fn register_bulk_chunk(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kinds: &[ReportedReferenceKind],
) -> crate::Result<Vec<ReportedRelationshipId>> {
    let keys = kinds
        .iter()
        .map(|_| RelationshipAssertionKey::fresh())
        .collect::<Vec<_>>();
    let rows = keys
        .iter()
        .map(|key| (key.as_str().to_owned(), snapshot.as_str()))
        .collect::<Vec<_>>();
    let returned = InsertCatalogRelationshipsBulk { rows: &rows }
        .load::<RegisteredRelationshipRow>(connection)?;
    let ids_by_key = returned
        .into_iter()
        .map(|row| (row.assertion_key, row.relationship_id))
        .collect::<HashMap<_, _>>();
    if ids_by_key.len() != keys.len() {
        return Err(crate::Error::DatabaseSchema(
            "bulk relationship registration did not return every assertion key".into(),
        ));
    }

    let mut ids = Vec::with_capacity(kinds.len());
    let mut subtype_batch = InsertBatch::new();
    for (key, kind) in keys.iter().zip(kinds) {
        let id = ids_by_key.get(key.as_str()).copied().ok_or_else(|| {
            crate::Error::DatabaseSchema(
                "bulk relationship registration returned an unknown assertion key".into(),
            )
        })?;
        cached_sql(
            "INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (?,?)",
        )
        .bind::<BigInt, _>(id)
        .bind::<Text, _>(kind.code())
        .enqueue(&mut subtype_batch, connection, InsertPhase::Children)?;
        ids.push(ReportedRelationshipId(id));
    }
    subtype_batch.flush(connection)?;
    Ok(ids)
}

enum MergeOwner {
    Mame(MergeMediaKind),
    Logiqx(MergeMediaKind),
    ClrMamePro,
}

impl MergeOwner {
    fn for_asset(asset: &SnapshotAsset) -> crate::Result<Option<Self>> {
        let xml_kind = || match asset.role {
            "rom" => Ok(MergeMediaKind::Rom),
            "disk" => Ok(MergeMediaKind::Disk),
            role => Err(crate::Error::DatabaseSchema(format!(
                "invalid XML merge media kind {role}"
            ))),
        };
        match &asset.native {
            NativeAssetFacts::Mame { .. } => Ok(Some(Self::Mame(xml_kind()?))),
            NativeAssetFacts::Logiqx(_) => Ok(Some(Self::Logiqx(xml_kind()?))),
            NativeAssetFacts::CmpRom(_) => Ok(Some(Self::ClrMamePro)),
            _ => Ok(None),
        }
    }

    const fn reference_kind(&self) -> ReportedReferenceKind {
        match self {
            Self::Mame(MergeMediaKind::Rom) => {
                ReportedReferenceKind::Mame(XmlReferenceKind::RomMerge)
            }
            Self::Mame(MergeMediaKind::Disk) => {
                ReportedReferenceKind::Mame(XmlReferenceKind::DiskMerge)
            }
            Self::Logiqx(MergeMediaKind::Rom) => {
                ReportedReferenceKind::Logiqx(XmlReferenceKind::RomMerge)
            }
            Self::Logiqx(MergeMediaKind::Disk) => {
                ReportedReferenceKind::Logiqx(XmlReferenceKind::DiskMerge)
            }
            Self::ClrMamePro => ReportedReferenceKind::ClrMamePro(CmpReferenceKind::RomMerge),
        }
    }

    const fn claim_kind(&self) -> Option<&'static str> {
        match self {
            Self::Logiqx(MergeMediaKind::Rom) => Some("logiqx_rom"),
            Self::Logiqx(MergeMediaKind::Disk) => Some("logiqx_disk"),
            _ => None,
        }
    }
}

trait MergeShape: 'static {
    const TABLE: &'static str;
    const STORES_LOCATION: bool;
    const STORES_CLAIM_KIND: bool;
}

struct MameRomMergeShape;
struct MameDiskMergeShape;
struct LogiqxMergeShape;
struct ClrMameProMergeShape;

macro_rules! merge_shapes {
    ($($shape:ty => ($table:literal, $location:literal, $claim:literal)),+ $(,)?) => {
        $(
            impl MergeShape for $shape {
                const TABLE: &'static str = $table;
                const STORES_LOCATION: bool = $location;
                const STORES_CLAIM_KIND: bool = $claim;
            }
        )+
    };
}

merge_shapes!(
    MameRomMergeShape => ("mame_rom_merges", true, false),
    MameDiskMergeShape => ("mame_disk_merges", true, false),
    LogiqxMergeShape => ("logiqx_file_merges", false, true),
    ClrMameProMergeShape => ("clrmamepro_rom_merges", false, false),
);

struct MergeInsert<'a, Shape: MergeShape> {
    occurrence_id: i64,
    relationship_id: i64,
    merge_name: &'a str,
    source_line: i64,
    source_column: i64,
    claim_kind: &'a str,
    shape: std::marker::PhantomData<Shape>,
}

impl<Shape: MergeShape> QueryId for MergeInsert<'_, Shape> {
    type QueryId = Shape;
}

impl<Shape: MergeShape> QueryFragment<Sqlite> for MergeInsert<'_, Shape> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("INSERT INTO ");
        pass.push_sql(Shape::TABLE);
        pass.push_sql("(occurrence_id,relationship_id,merge_name");
        if Shape::STORES_LOCATION {
            pass.push_sql(",source_line,source_column");
        }
        if Shape::STORES_CLAIM_KIND {
            pass.push_sql(",claim_kind");
        }
        pass.push_sql(") VALUES (");
        pass.push_bind_param::<BigInt, _>(&self.occurrence_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.relationship_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.merge_name)?;
        if Shape::STORES_LOCATION {
            pass.push_sql(",");
            pass.push_bind_param::<BigInt, _>(&self.source_line)?;
            pass.push_sql(",");
            pass.push_bind_param::<BigInt, _>(&self.source_column)?;
        }
        if Shape::STORES_CLAIM_KIND {
            pass.push_sql(",");
            pass.push_bind_param::<Text, _>(&self.claim_kind)?;
        }
        pass.push_sql(")");
        Ok(())
    }
}

fn insert_merge<Shape: MergeShape>(
    connection: &mut SqliteConnection,
    occurrence: OccurrenceId,
    relationship: &ReportedRelationshipId,
    literal: &str,
    location: crate::logiqx::RecordLocation,
    claim_kind: Option<&str>,
) -> crate::Result<()> {
    let claim_kind = claim_kind.unwrap_or_default();
    ExecuteDsl::execute(
        MergeInsert::<Shape> {
            occurrence_id: occurrence.database_value(),
            relationship_id: relationship.database_value(),
            merge_name: literal,
            source_line: location.line,
            source_column: location.column,
            claim_kind,
            shape: std::marker::PhantomData,
        },
        connection,
    )?;
    Ok(())
}

pub(super) fn insert_asset_merge(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    occurrence: OccurrenceId,
    asset: &SnapshotAsset,
) -> crate::Result<()> {
    let Some(literal) = asset.merge.as_deref() else {
        return Ok(());
    };
    let Some(owner) = MergeOwner::for_asset(asset)? else {
        return Ok(());
    };
    let relationship = register(connection, snapshot, owner.reference_kind())?;
    match owner {
        MergeOwner::Mame(MergeMediaKind::Rom) => insert_merge::<MameRomMergeShape>(
            connection,
            occurrence,
            &relationship,
            literal,
            asset.location,
            None,
        ),
        MergeOwner::Mame(MergeMediaKind::Disk) => insert_merge::<MameDiskMergeShape>(
            connection,
            occurrence,
            &relationship,
            literal,
            asset.location,
            None,
        ),
        logiqx @ MergeOwner::Logiqx(_) => insert_merge::<LogiqxMergeShape>(
            connection,
            occurrence,
            &relationship,
            literal,
            asset.location,
            logiqx.claim_kind(),
        ),
        MergeOwner::ClrMamePro => insert_merge::<ClrMameProMergeShape>(
            connection,
            occurrence,
            &relationship,
            literal,
            asset.location,
            None,
        ),
    }
}

pub(super) fn insert_asset_merges_bulk(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    occurrences: &[(OccurrenceId, &SnapshotAsset)],
) -> crate::Result<()> {
    const MAX_ROWS: usize = 64;
    const MAX_BYTES: usize = 1024 * 1024;

    let mut offset = 0;
    while offset < occurrences.len() {
        let mut end = offset;
        let mut bytes = 0_usize;
        let mut pending = Vec::new();
        while end < occurrences.len() && pending.len() < MAX_ROWS {
            let (occurrence, asset) = occurrences[end];
            let Some(literal) = asset.merge.as_deref() else {
                end += 1;
                continue;
            };
            let Some(owner) = MergeOwner::for_asset(asset)? else {
                end += 1;
                continue;
            };
            let row_bytes = literal.len().saturating_add(asset.role.len());
            if !pending.is_empty() && bytes.saturating_add(row_bytes) > MAX_BYTES {
                break;
            }
            bytes = bytes.saturating_add(row_bytes);
            pending.push((occurrence, asset, owner));
            end += 1;
        }

        if pending.is_empty() {
            offset = end;
            continue;
        }
        let kinds = pending
            .iter()
            .map(|(_, _, owner)| owner.reference_kind())
            .collect::<Vec<_>>();
        let relationships = register_bulk(connection, snapshot, &kinds)?;
        let mut merge_batch = InsertBatch::new();
        for ((occurrence, asset, owner), relationship) in pending.iter().zip(&relationships) {
            let literal = asset.merge.as_deref().ok_or_else(|| {
                crate::Error::DatabaseSchema(
                    "bulk merge owner is missing its declared merge literal".into(),
                )
            })?;
            match owner {
                MergeOwner::Mame(MergeMediaKind::Rom) => {
                    cached_sql("INSERT INTO mame_rom_merges(occurrence_id,relationship_id,merge_name,source_line,source_column) VALUES (?,?,?,?,?)")
                        .bind::<BigInt, _>(occurrence.database_value())
                        .bind::<BigInt, _>(relationship.database_value())
                        .bind::<Text, _>(literal)
                        .bind::<BigInt, _>(asset.location.line)
                        .bind::<BigInt, _>(asset.location.column)
                        .enqueue(&mut merge_batch, connection, InsertPhase::Children)?;
                }
                MergeOwner::Mame(MergeMediaKind::Disk) => {
                    cached_sql("INSERT INTO mame_disk_merges(occurrence_id,relationship_id,merge_name,source_line,source_column) VALUES (?,?,?,?,?)")
                        .bind::<BigInt, _>(occurrence.database_value())
                        .bind::<BigInt, _>(relationship.database_value())
                        .bind::<Text, _>(literal)
                        .bind::<BigInt, _>(asset.location.line)
                        .bind::<BigInt, _>(asset.location.column)
                        .enqueue(&mut merge_batch, connection, InsertPhase::Children)?;
                }
                MergeOwner::Logiqx(_) => {
                    let claim_kind = owner.claim_kind().ok_or_else(|| {
                        crate::Error::DatabaseSchema("XML merge requires a claim kind".into())
                    })?;
                    cached_sql("INSERT INTO logiqx_file_merges(occurrence_id,relationship_id,merge_name,claim_kind) VALUES (?,?,?,?)")
                        .bind::<BigInt, _>(occurrence.database_value())
                        .bind::<BigInt, _>(relationship.database_value())
                        .bind::<Text, _>(literal)
                        .bind::<Text, _>(claim_kind)
                        .enqueue(&mut merge_batch, connection, InsertPhase::Children)?;
                }
                MergeOwner::ClrMamePro => {
                    cached_sql("INSERT INTO clrmamepro_rom_merges(occurrence_id,relationship_id,merge_name) VALUES (?,?,?)")
                        .bind::<BigInt, _>(occurrence.database_value())
                        .bind::<BigInt, _>(relationship.database_value())
                        .bind::<Text, _>(literal)
                        .enqueue(&mut merge_batch, connection, InsertPhase::Children)?;
                }
            }
        }
        merge_batch.flush(connection)?;
        offset = end;
    }
    Ok(())
}

#[cfg(test)]
mod prepared_statement_tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use diesel::{
        Connection,
        connection::{InstrumentationEvent, SimpleConnection},
        sqlite::SqliteConnection,
    };

    use super::{ReportedReferenceKind, XmlReferenceKind, register};
    use crate::domain::SnapshotKey;

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "SQLite fixture setup and prepared statement assertions must fail loudly"
    )]
    fn relationship_registration_reuses_both_statements_for_changed_values() -> crate::Result<()> {
        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
        connection.batch_execute(
            "CREATE TABLE catalog_relationships (
                relationship_id INTEGER PRIMARY KEY AUTOINCREMENT,
                assertion_key TEXT NOT NULL,
                origin TEXT NOT NULL,
                snapshot_key TEXT NOT NULL
            );
            CREATE TABLE reported_catalog_relationships (
                relationship_id INTEGER PRIMARY KEY,
                source_reference_kind TEXT NOT NULL
            )",
        )?;
        let cached_queries = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cached_queries);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if matches!(event, InstrumentationEvent::CacheQuery { .. }) {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let first = register(
            &mut connection,
            &SnapshotKey::from_persisted("snapshot-a".into()),
            ReportedReferenceKind::Mame(XmlReferenceKind::CloneOf),
        )?;
        let second = register(
            &mut connection,
            &SnapshotKey::from_persisted("snapshot-b".into()),
            ReportedReferenceKind::Mame(XmlReferenceKind::RomOf),
        )?;

        assert_ne!(first.database_value(), second.database_value());
        assert_eq!(cached_queries.load(Ordering::Relaxed), 2);
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod bulk_merge_tests {
    use std::{
        fmt::Write as _,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use diesel::{
        Connection, RunQueryDsl, SqliteConnection,
        connection::{InstrumentationEvent, SimpleConnection},
        sql_query,
        sql_types::BigInt,
    };

    use super::{insert_asset_merge, insert_asset_merges_bulk};
    use crate::{
        domain::{OccurrenceId, SnapshotKey},
        storage::catalog_import::{SnapshotAsset, machine_contents},
    };

    #[derive(diesel::QueryableByName)]
    struct MergeCounts {
        #[diesel(sql_type = BigInt)]
        merges: i64,
        #[diesel(sql_type = BigInt)]
        reported: i64,
        #[diesel(sql_type = BigInt)]
        mapped: i64,
    }

    #[derive(diesel::QueryableByName)]
    struct TextValue {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    #[expect(
        clippy::expect_used,
        reason = "in-memory SQLite fixture creation must fail loudly"
    )]
    fn connection_with_insert_counter() -> crate::Result<(SqliteConnection, Arc<AtomicUsize>)> {
        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite test database");
        connection.batch_execute(
            "CREATE TABLE catalog_relationships(relationship_id INTEGER PRIMARY KEY AUTOINCREMENT, assertion_key TEXT, origin TEXT, snapshot_key TEXT);
             CREATE TABLE reported_catalog_relationships(relationship_id INTEGER PRIMARY KEY, source_reference_kind TEXT);
             CREATE TABLE mame_rom_merges(occurrence_id INTEGER PRIMARY KEY, relationship_id INTEGER UNIQUE, merge_name TEXT, source_line INTEGER, source_column INTEGER);",
        )?;
        let statements = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&statements);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                let sql = query.to_string();
                if sql.starts_with("INSERT INTO catalog_relationships")
                    || sql.starts_with("INSERT INTO reported_catalog_relationships")
                    || sql.starts_with("INSERT INTO mame_rom_merges")
                {
                    observed.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        Ok((connection, statements))
    }

    fn assets() -> crate::Result<Vec<SnapshotAsset>> {
        let mut xml = String::from(
            r#"<mame mameconfig="10"><machine name="test"><description>Test</description>"#,
        );
        for index in 0..130 {
            write!(
                xml,
                "<rom name=\"asset-{index}\" size=\"1\" crc=\"00000000\" merge=\"{}\"/>",
                if index == 129 {
                    String::new()
                } else {
                    format!("parent-{index}")
                }
            )
            .expect("writing XML to a String succeeds");
        }
        xml.push_str("</machine></mame>");
        let catalog = crate::mame::MameCatalog::parse(xml.as_bytes())?;
        let machine = catalog
            .machines
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::DatabaseSchema("missing MAME test machine".into()))?;
        Ok(machine_contents(machine).assets)
    }

    #[test]
    fn asset_merge_bulk_matches_legacy_rows_and_reduces_insert_statements() -> crate::Result<()> {
        let assets = assets()?;
        let occurrences = assets
            .iter()
            .enumerate()
            .map(|(index, asset)| {
                Ok((
                    OccurrenceId::try_from(
                        i64::try_from(index).expect("asset index fits in i64") + 1,
                    )?,
                    asset,
                ))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let snapshot = SnapshotKey::from_persisted("merge-snapshot".into());

        let (mut bulk_connection, bulk_statements) = connection_with_insert_counter()?;
        insert_asset_merges_bulk(&mut bulk_connection, &snapshot, &occurrences)?;
        let bulk_counts = sql_query(
            "SELECT (SELECT count(*) FROM mame_rom_merges) AS merges,
                    (SELECT count(*) FROM reported_catalog_relationships) AS reported,
                    (SELECT count(*) FROM mame_rom_merges AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_rom_merge') AS mapped",
        )
        .get_result::<MergeCounts>(&mut bulk_connection)?;
        assert_eq!(
            (bulk_counts.merges, bulk_counts.reported, bulk_counts.mapped),
            (130, 130, 130)
        );
        let empty_merge =
            sql_query("SELECT merge_name AS value FROM mame_rom_merges WHERE occurrence_id=130")
                .get_result::<TextValue>(&mut bulk_connection)?;
        assert_eq!(empty_merge.value, "");
        let bulk_statement_count = bulk_statements.load(Ordering::Relaxed);

        let (mut legacy_connection, legacy_statements) = connection_with_insert_counter()?;
        for (occurrence, asset) in &occurrences {
            insert_asset_merge(&mut legacy_connection, &snapshot, *occurrence, asset)?;
        }
        let legacy_counts = sql_query(
            "SELECT (SELECT count(*) FROM mame_rom_merges) AS merges,
                    (SELECT count(*) FROM reported_catalog_relationships) AS reported,
                    (SELECT count(*) FROM mame_rom_merges AS native
                     JOIN reported_catalog_relationships AS reported USING(relationship_id)
                     WHERE reported.source_reference_kind='mame_rom_merge') AS mapped",
        )
        .get_result::<MergeCounts>(&mut legacy_connection)?;
        assert_eq!(
            (
                legacy_counts.merges,
                legacy_counts.reported,
                legacy_counts.mapped
            ),
            (bulk_counts.merges, bulk_counts.reported, bulk_counts.mapped)
        );

        let legacy_statement_count = legacy_statements.load(Ordering::Relaxed);
        assert_eq!(legacy_statement_count, 130 * 3);
        assert!(bulk_statement_count <= 9);
        assert!(bulk_statement_count < legacy_statement_count);
        Ok(())
    }
}
