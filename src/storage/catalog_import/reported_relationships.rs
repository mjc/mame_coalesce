//! One identity issuer for source declarations; native tables own their literals.

use diesel::{
    RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_query,
    sql_types::{BigInt, Text},
    sqlite::Sqlite,
};

use crate::domain::{
    CatalogSetId, MergeMediaKind, NoIntroArchiveId, NoIntroArchiveReferenceField, OccurrenceId,
    RelationshipAssertionKey, SnapshotKey,
};

use super::{NativeAssetFacts, SnapshotAsset};

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
