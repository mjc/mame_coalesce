use super::{NativeAssetFacts, SnapshotAsset, sqlite_mame_boolean};
use crate::{
    domain::{CatalogContentId, CatalogSetId},
    storage::{
        cached_sql::{InsertBatch, InsertPhase, cached_sql},
        catalog_identity::OccurrenceId,
        mame_attributes::{self, Family, PositionBatch},
    },
};
use diesel::{
    RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
    sqlite::Sqlite,
};

#[derive(Clone, Copy)]
enum RootRecordKind {
    MameMachine,
    LogiqxGame,
    ClrMameProSet,
    NoIntroPcGame,
}

impl RootRecordKind {
    const fn for_asset(asset: &NativeAssetFacts) -> Self {
        match asset {
            NativeAssetFacts::Mame { .. } | NativeAssetFacts::MameSample { .. } => {
                Self::MameMachine
            }
            NativeAssetFacts::Logiqx(_) => Self::LogiqxGame,
            NativeAssetFacts::CmpRom(_) | NativeAssetFacts::CmpSample(_) => Self::ClrMameProSet,
            NativeAssetFacts::NoIntroPc { .. } => Self::NoIntroPcGame,
        }
    }

    const fn source_kind(self) -> &'static str {
        match self {
            Self::MameMachine => "mame_machine",
            Self::LogiqxGame => "logiqx_game",
            Self::ClrMameProSet => "cmp_set",
            Self::NoIntroPcGame => "no_intro_pc_game",
        }
    }
}

#[derive(Clone, Copy)]
enum RootClaimKind {
    MameRom,
    MameDisk,
    MameSample,
    LogiqxRom,
    LogiqxDisk,
    LogiqxSample,
    CmpRom,
    CmpSample,
    NoIntroPcFile,
}

impl RootClaimKind {
    fn for_record(kind: RootRecordKind, role: &str) -> crate::Result<Self> {
        match (kind, role) {
            (RootRecordKind::MameMachine, "rom") => Ok(Self::MameRom),
            (RootRecordKind::MameMachine, "disk") => Ok(Self::MameDisk),
            (RootRecordKind::MameMachine, "other") => Ok(Self::MameSample),
            (RootRecordKind::LogiqxGame, "rom") => Ok(Self::LogiqxRom),
            (RootRecordKind::LogiqxGame, "disk") => Ok(Self::LogiqxDisk),
            (RootRecordKind::LogiqxGame, "other") => Ok(Self::LogiqxSample),
            (RootRecordKind::ClrMameProSet, "rom") => Ok(Self::CmpRom),
            (RootRecordKind::ClrMameProSet, "other") => Ok(Self::CmpSample),
            (RootRecordKind::NoIntroPcGame, "rom") => Ok(Self::NoIntroPcFile),
            _ => Err(crate::Error::InvalidPath(format!(
                "invalid native claim {}/{role}",
                kind.source_kind()
            ))),
        }
    }

    const fn code(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom",
            Self::MameDisk => "mame_disk",
            Self::MameSample => "mame_sample",
            Self::LogiqxRom => "logiqx_rom",
            Self::LogiqxDisk => "logiqx_disk",
            Self::LogiqxSample => "logiqx_sample",
            Self::CmpRom => "cmp_rom",
            Self::CmpSample => "cmp_sample",
            Self::NoIntroPcFile => "no_intro_pc_file",
        }
    }

    const fn table(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom_claims",
            Self::MameDisk => "mame_disk_claims",
            Self::MameSample => "mame_samples",
            Self::LogiqxRom => "logiqx_rom_claims",
            Self::LogiqxDisk => "logiqx_disk_claims",
            Self::LogiqxSample => "logiqx_sample_claims",
            Self::CmpRom => "cmp_rom_claims",
            Self::CmpSample => "cmp_samples",
            Self::NoIntroPcFile => "no_intro_pc_file_claims",
        }
    }
}

/// Insert a source claim and its matching native payload together. Publication
/// never exposes an occurrence without a typed field owner.
pub(super) fn insert(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    order: i64,
    asset: &SnapshotAsset,
    content_uuid: Option<&[u8]>,
) -> crate::Result<OccurrenceId> {
    let record_kind = RootRecordKind::for_asset(&asset.native);
    let kind = RootClaimKind::for_record(record_kind, asset.role)?;
    let id = insert_occurrence(conn, record, order, kind, content_uuid)?;
    match kind {
        RootClaimKind::MameRom | RootClaimKind::MameDisk => {
            insert_mame(conn, id, kind, asset)?;
        }
        RootClaimKind::MameSample => {
            let NativeAssetFacts::MameSample {
                source_order,
                attribute_positions,
            } = &asset.native
            else {
                return Err(crate::Error::InvalidPath(
                    "MAME sample has no native declaration".into(),
                ));
            };
            sql_query("INSERT INTO mame_samples(occurrence_id,name,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
                .bind::<BigInt, _>(id.database_value()).bind::<Text, _>(&asset.name)
                .bind::<BigInt, _>(*source_order).bind::<BigInt, _>(asset.location.line)
                .bind::<BigInt, _>(asset.location.column).execute(conn)?;
            mame_attributes::insert(
                conn,
                Family::Sample,
                &[id.database_value()],
                attribute_positions,
                crate::mame::MameSampleAttribute::code,
            )?;
        }
        RootClaimKind::LogiqxRom | RootClaimKind::LogiqxDisk | RootClaimKind::LogiqxSample => {
            insert_logiqx(conn, id, kind, asset)?;
        }
        RootClaimKind::CmpRom => {
            super::cmp_native::insert_rom_claim(conn, id.database_value(), asset)?;
        }
        RootClaimKind::CmpSample => {
            let NativeAssetFacts::CmpSample(sample) = &asset.native else {
                return Err(crate::Error::InvalidPath(
                    "CMP sample has no scalar declaration".into(),
                ));
            };
            super::cmp_native::insert_sample(conn, id, sample)?;
        }
        RootClaimKind::NoIntroPcFile => {
            let NativeAssetFacts::NoIntroPc {
                size_text,
                source_order,
                ..
            } = &asset.native
            else {
                return Err(crate::Error::InvalidPath(
                    "P/C ROM has no native declaration".into(),
                ));
            };
            sql_query("INSERT INTO no_intro_pc_file_claims (occurrence_id,name,size_text,source_order,evidence_scope,evidence_provenance,source_line,source_column) VALUES (?,?,?,?,?,'source_declared',?,?)")
            .bind::<BigInt, _>(id.database_value())
            .bind::<Text, _>(&asset.name)
            .bind::<Nullable<Text>, _>(size_text.as_deref())
            .bind::<BigInt, _>(super::checked_order(*source_order, "P/C ROM children")?)
            .bind::<Text, _>(asset.evidence_scope)
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column)
            .execute(conn)?;
        }
    }
    Ok(id)
}

struct InsertOccurrenceQuery;

struct InsertOccurrence<'a> {
    record_id: i64,
    order: i64,
    claim_kind: &'static str,
    content_uuid: Option<&'a [u8]>,
}

impl QueryId for InsertOccurrence<'_> {
    type QueryId = InsertOccurrenceQuery;
}

impl Query for InsertOccurrence<'_> {
    type SqlType = BigInt;
}

impl RunQueryDsl<SqliteConnection> for InsertOccurrence<'_> {}

impl QueryFragment<Sqlite> for InsertOccurrence<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) VALUES (",
        );
        pass.push_bind_param::<BigInt, _>(&self.record_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.order)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.claim_kind)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Binary>, _>(&self.content_uuid)?;
        pass.push_sql(") RETURNING occurrence_id");
        Ok(())
    }
}

fn insert_occurrence(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    order: i64,
    kind: RootClaimKind,
    content_uuid: Option<&[u8]>,
) -> crate::Result<OccurrenceId> {
    let occurrence_id = InsertOccurrence {
        record_id: record.as_i64(),
        order,
        claim_kind: kind.code(),
        content_uuid,
    }
    .get_result::<i64>(conn)?;
    Ok(OccurrenceId::from_database(occurrence_id))
}

struct MameRomClaimQuery;
struct MameDiskClaimQuery;
struct MameDiskCompatibilityQuery;

struct MameRomClaimInsert<'a> {
    occurrence_id: i64,
    name: &'a str,
    size_text: Option<&'a str>,
    crc_text: Option<&'a str>,
    sha1_text: Option<&'a str>,
    evidence_scope: &'a str,
    dump_status: Option<&'a str>,
    source_line: i64,
    source_column: i64,
    region: Option<&'a str>,
    bios: Option<&'a str>,
    offset_text: Option<&'a str>,
    optional: bool,
    source_order: i64,
    status_specified: bool,
    optional_specified: bool,
}

impl QueryId for MameRomClaimInsert<'_> {
    type QueryId = MameRomClaimQuery;
}

impl RunQueryDsl<SqliteConnection> for MameRomClaimInsert<'_> {}

trait MameClaimRow {
    const PREFIX: &'static str;
    fn walk_values<'b>(&'b self, pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()>;
}

impl MameClaimRow for MameRomClaimInsert<'_> {
    const PREFIX: &'static str = "INSERT INTO mame_rom_claims \
             (occurrence_id,name,size_text,crc_text,sha1_text,evidence_scope,evidence_provenance, \
              dump_status,source_line,source_column,region,bios,offset_text,optional,source_order, \
              status_specified,optional_specified) \
             VALUES ";
    fn walk_values<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("(");
        pass.push_bind_param::<BigInt, _>(&self.occurrence_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.name)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.size_text)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.crc_text)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.sha1_text)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.evidence_scope)?;
        pass.push_sql(",'source_declared',");
        pass.push_bind_param::<Nullable<Text>, _>(&self.dump_status)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_line)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_column)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.region)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.bios)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.offset_text)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.optional)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_order)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.status_specified)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.optional_specified)?;
        pass.push_sql(")");
        Ok(())
    }
}

impl QueryFragment<Sqlite> for MameRomClaimInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(Self::PREFIX);
        self.walk_values(pass)
    }
}

#[allow(clippy::struct_excessive_bools)] // independent source flags map directly to stored columns
struct MameDiskClaimInsert<'a> {
    occurrence_id: i64,
    name: &'a str,
    sha1_text: Option<&'a str>,
    evidence_scope: &'a str,
    dump_status: Option<&'a str>,
    source_line: i64,
    source_column: i64,
    region: Option<&'a str>,
    disk_index: Option<&'a str>,
    writable: bool,
    optional: bool,
    source_order: i64,
    status_specified: bool,
    optional_specified: bool,
    writable_specified: bool,
}

impl QueryId for MameDiskClaimInsert<'_> {
    type QueryId = MameDiskClaimQuery;
}

impl RunQueryDsl<SqliteConnection> for MameDiskClaimInsert<'_> {}

impl MameClaimRow for MameDiskClaimInsert<'_> {
    const PREFIX: &'static str = "INSERT INTO mame_disk_claims \
             (occurrence_id,name,sha1_text,evidence_scope,evidence_provenance,dump_status, \
              source_line,source_column,region,disk_index,writable,optional,source_order, \
              status_specified,optional_specified,writable_specified) VALUES ";
    fn walk_values<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("(");
        pass.push_bind_param::<BigInt, _>(&self.occurrence_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.name)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.sha1_text)?;
        pass.push_sql(",");
        pass.push_bind_param::<Text, _>(&self.evidence_scope)?;
        pass.push_sql(",'source_declared',");
        pass.push_bind_param::<Nullable<Text>, _>(&self.dump_status)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_line)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_column)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.region)?;
        pass.push_sql(",");
        pass.push_bind_param::<Nullable<Text>, _>(&self.disk_index)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.writable)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.optional)?;
        pass.push_sql(",");
        pass.push_bind_param::<BigInt, _>(&self.source_order)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.status_specified)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.optional_specified)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.writable_specified)?;
        pass.push_sql(")");
        Ok(())
    }
}

impl QueryFragment<Sqlite> for MameDiskClaimInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(Self::PREFIX);
        self.walk_values(pass)
    }
}

struct ClaimRows<'a, Row>(&'a [Row]);

impl<Row> QueryId for ClaimRows<'_, Row> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<Row> RunQueryDsl<SqliteConnection> for ClaimRows<'_, Row> {}

impl<Row: MameClaimRow> QueryFragment<Sqlite> for ClaimRows<'_, Row> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql(Row::PREFIX);
        for (index, row) in self.0.iter().enumerate() {
            if index > 0 {
                pass.push_sql(",");
            }
            row.walk_values(pass.reborrow())?;
        }
        Ok(())
    }
}

struct MameDiskCompatibilityInsert {
    occurrence_id: i64,
    writeable: bool,
}

impl QueryId for MameDiskCompatibilityInsert {
    type QueryId = MameDiskCompatibilityQuery;
}

impl RunQueryDsl<SqliteConnection> for MameDiskCompatibilityInsert {}

impl QueryFragment<Sqlite> for MameDiskCompatibilityInsert {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("INSERT INTO mame_disk_compatibility(occurrence_id,writeable) VALUES (");
        pass.push_bind_param::<BigInt, _>(&self.occurrence_id)?;
        pass.push_sql(",");
        pass.push_bind_param::<Bool, _>(&self.writeable)?;
        pass.push_sql(")");
        Ok(())
    }
}

fn insert_logiqx(
    conn: &mut SqliteConnection,
    id: OccurrenceId,
    kind: RootClaimKind,
    asset: &SnapshotAsset,
) -> crate::Result<()> {
    let NativeAssetFacts::Logiqx(facts) = &asset.native else {
        return Err(crate::Error::InvalidPath(
            "Logiqx media entry has no native attributes".into(),
        ));
    };
    if matches!(kind, RootClaimKind::LogiqxSample) {
        sql_query("INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
            .bind::<BigInt, _>(id.database_value()).bind::<Text, _>(&asset.name)
            .bind::<BigInt, _>(facts.source_order)
            .bind::<BigInt, _>(asset.location.line).bind::<BigInt, _>(asset.location.column)
            .execute(conn)?;
        return Ok(());
    }
    let statement = format!(
        "INSERT INTO {} (occurrence_id,name,evidence_scope,evidence_provenance, \
         md5_text,sha1_text, \
         dump_status,status_was_present,source_order,source_line,source_column{}) \
         VALUES (?,?,?,'source_declared',?,?,?,?,?,?,?{})",
        kind.table(),
        if matches!(kind, RootClaimKind::LogiqxRom) {
            ",size_text,crc_text,serial,date"
        } else {
            ""
        },
        if matches!(kind, RootClaimKind::LogiqxRom) {
            ",?,?,?,?"
        } else {
            ""
        },
    );
    let query = sql_query(statement)
        .bind::<BigInt, _>(id.database_value())
        .bind::<Text, _>(&asset.name)
        .bind::<Text, _>(asset.evidence_scope)
        .bind::<Nullable<Text>, _>(facts.md5_text.as_deref())
        .bind::<Nullable<Text>, _>(facts.sha1_text.as_deref())
        .bind::<Text, _>(asset.dump_status.as_deref().unwrap_or("good"))
        .bind::<diesel::sql_types::Bool, _>(facts.status_was_present)
        .bind::<BigInt, _>(facts.source_order)
        .bind::<BigInt, _>(asset.location.line)
        .bind::<BigInt, _>(asset.location.column);
    if matches!(kind, RootClaimKind::LogiqxRom) {
        query
            .bind::<Nullable<Text>, _>(facts.size_text.as_deref())
            .bind::<Nullable<Text>, _>(facts.crc_text.as_deref())
            .bind::<Nullable<Text>, _>(asset.serial.as_deref())
            .bind::<Nullable<Text>, _>(asset.date.as_deref())
            .execute(conn)?;
    } else {
        query.execute(conn)?;
    }
    Ok(())
}

fn insert_mame(
    conn: &mut SqliteConnection,
    id: OccurrenceId,
    kind: RootClaimKind,
    asset: &SnapshotAsset,
) -> crate::Result<()> {
    let NativeAssetFacts::Mame {
        attributes,
        declarations,
        source_order,
        attribute_positions,
    } = &asset.native
    else {
        return Err(crate::Error::InvalidPath(
            "MAME media entry has no native attributes".into(),
        ));
    };
    match kind {
        RootClaimKind::MameRom => {
            ExecuteDsl::execute(
                rom_claim(id, asset, attributes, declarations, *source_order),
                conn,
            )?;
            insert_rom_compatibility(conn, id, attributes, declarations)?;
        }
        RootClaimKind::MameDisk => {
            ExecuteDsl::execute(
                disk_claim(id, asset, attributes, declarations, *source_order),
                conn,
            )?;
            if let Some(writeable) = attributes.writeable {
                ExecuteDsl::execute(
                    MameDiskCompatibilityInsert {
                        occurrence_id: id.database_value(),
                        writeable: writeable.as_bool(),
                    },
                    conn,
                )?;
            }
        }
        _ => {
            return Err(crate::Error::InvalidPath(
                "non-MAME native media owner".into(),
            ));
        }
    }
    insert_mame_positions(conn, id, kind, attribute_positions)
}

fn rom_claim<'a>(
    id: OccurrenceId,
    asset: &'a SnapshotAsset,
    attributes: &'a crate::mame::MameAssetAttributes,
    declarations: &'a crate::mame::MameAssetDeclarations,
    source_order: i64,
) -> MameRomClaimInsert<'a> {
    MameRomClaimInsert {
        occurrence_id: id.database_value(),
        name: &asset.name,
        size_text: declarations.size_text.as_deref(),
        crc_text: declarations.crc_text.as_deref(),
        sha1_text: declarations.sha1_text.as_deref(),
        evidence_scope: asset.evidence_scope,
        dump_status: asset.dump_status.as_deref(),
        source_line: asset.location.line,
        source_column: asset.location.column,
        region: attributes.region.as_deref(),
        bios: attributes.bios.as_deref(),
        offset_text: declarations.offset_text.as_deref(),
        optional: attributes.optional.as_bool(),
        source_order,
        status_specified: attributes.status_specified,
        optional_specified: attributes.optional_specified,
    }
}

fn disk_claim<'a>(
    id: OccurrenceId,
    asset: &'a SnapshotAsset,
    attributes: &'a crate::mame::MameAssetAttributes,
    declarations: &'a crate::mame::MameAssetDeclarations,
    source_order: i64,
) -> MameDiskClaimInsert<'a> {
    MameDiskClaimInsert {
        occurrence_id: id.database_value(),
        name: &asset.name,
        sha1_text: declarations.sha1_text.as_deref(),
        evidence_scope: asset.evidence_scope,
        dump_status: asset.dump_status.as_deref(),
        source_line: asset.location.line,
        source_column: asset.location.column,
        region: attributes.region.as_deref(),
        disk_index: attributes.disk_index.as_deref(),
        writable: attributes.writable.unwrap_or_default().as_bool(),
        optional: attributes.optional.as_bool(),
        source_order,
        status_specified: attributes.status_specified,
        optional_specified: attributes.optional_specified,
        writable_specified: attributes.writable_specified,
    }
}

pub(super) struct MameAssetInput<'a> {
    pub record: CatalogSetId,
    pub order: i64,
    pub asset: &'a SnapshotAsset,
    pub content: Option<CatalogContentId>,
}

/// Allocate occurrence IDs under the import's IMMEDIATE transaction, then write
/// parents, native claims and positions in dependency order.
pub(super) fn insert_mame_bulk(
    conn: &mut SqliteConnection,
    inputs: &[MameAssetInput<'_>],
) -> crate::Result<Vec<OccurrenceId>> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let ids = insert_mame_occurrences(conn, inputs)?;
    insert_mame_claims(conn, inputs, &ids)?;
    insert_mame_bulk_positions(conn, inputs, &ids)?;
    Ok(ids)
}

fn insert_mame_occurrences(
    conn: &mut SqliteConnection,
    inputs: &[MameAssetInput<'_>],
) -> crate::Result<Vec<OccurrenceId>> {
    #[derive(diesel::QueryableByName)]
    struct LastId {
        #[diesel(sql_type = BigInt)]
        value: i64,
    }
    let last = cached_sql("SELECT COALESCE(MAX(occurrence_id),0) AS value FROM asset_occurrences")
        .get_result::<LastId>(conn)?
        .value;
    let mut batch = InsertBatch::default();
    let mut ids = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        let offset = super::checked_order(index, "media entries")?;
        let value = last
            .checked_add(offset)
            .and_then(|id| id.checked_add(1))
            .ok_or_else(|| crate::Error::DatabaseSchema("occurrence ID overflow".into()))?;
        let kind = RootClaimKind::for_record(RootRecordKind::MameMachine, input.asset.role)?;
        cached_sql("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind,content_uuid) VALUES (?,?,?,?,?)")
            .bind::<BigInt,_>(value).bind::<BigInt,_>(input.record.as_i64())
            .bind::<BigInt,_>(input.order).bind::<Text,_>(kind.code())
            .bind::<Nullable<Binary>,_>(input.content.map(|id| id.as_bytes().to_vec()))
            .enqueue(&mut batch, conn, InsertPhase::Parents)?;
        ids.push(OccurrenceId::from_database(value));
    }
    batch.flush(conn)?;
    Ok(ids)
}

fn insert_mame_claims(
    conn: &mut SqliteConnection,
    inputs: &[MameAssetInput<'_>],
    ids: &[OccurrenceId],
) -> crate::Result<()> {
    let mut batch = InsertBatch::default();
    let mut roms = Vec::new();
    let mut disks = Vec::new();
    for (input, id) in inputs.iter().zip(ids) {
        match &input.asset.native {
            NativeAssetFacts::Mame {
                attributes,
                declarations,
                source_order,
                ..
            } => match RootClaimKind::for_record(RootRecordKind::MameMachine, input.asset.role)? {
                RootClaimKind::MameRom => roms.push(rom_claim(
                    *id,
                    input.asset,
                    attributes,
                    declarations,
                    *source_order,
                )),
                RootClaimKind::MameDisk => disks.push(disk_claim(
                    *id,
                    input.asset,
                    attributes,
                    declarations,
                    *source_order,
                )),
                _ => {
                    return Err(crate::Error::DatabaseSchema(
                        "invalid MAME media family".into(),
                    ));
                }
            },
            NativeAssetFacts::MameSample { source_order, .. } => {
                cached_sql("INSERT INTO mame_samples(occurrence_id,name,source_order,source_line,source_column) VALUES (?,?,?,?,?)")
                    .bind::<BigInt,_>(id.database_value()).bind::<Text,_>(&input.asset.name)
                    .bind::<BigInt,_>(*source_order).bind::<BigInt,_>(input.asset.location.line)
                    .bind::<BigInt,_>(input.asset.location.column)
                    .enqueue(&mut batch,conn,InsertPhase::Parents)?;
            }
            _ => {
                return Err(crate::Error::DatabaseSchema(
                    "non-MAME asset in MAME bulk writer".into(),
                ));
            }
        }
    }
    for page in roms.chunks(56) {
        ExecuteDsl::execute(ClaimRows(page), conn)?;
    }
    for page in disks.chunks(56) {
        ExecuteDsl::execute(ClaimRows(page), conn)?;
    }
    batch.flush(conn)?;
    Ok(())
}

fn insert_mame_bulk_positions(
    conn: &mut SqliteConnection,
    inputs: &[MameAssetInput<'_>],
    ids: &[OccurrenceId],
) -> crate::Result<()> {
    let mut positions = PositionBatch::default();
    for (input, id) in inputs.iter().zip(ids) {
        match &input.asset.native {
            NativeAssetFacts::Mame {
                attributes,
                declarations,
                attribute_positions,
                ..
            } => {
                let kind =
                    RootClaimKind::for_record(RootRecordKind::MameMachine, input.asset.role)?;
                match (kind, attribute_positions) {
                    (
                        RootClaimKind::MameRom,
                        crate::mame::MameAssetAttributePositions::Rom {
                            native,
                            compatibility,
                        },
                    ) => {
                        insert_rom_compatibility(conn, *id, attributes, declarations)?;
                        positions.queue(
                            conn,
                            Family::Rom,
                            &[id.database_value()],
                            native,
                            crate::mame::MameRomAttribute::code,
                        )?;
                        positions.queue(
                            conn,
                            Family::RomCompatibility,
                            &[id.database_value()],
                            compatibility,
                            crate::mame::MameRomCompatibilityAttribute::code,
                        )?;
                    }
                    (
                        RootClaimKind::MameDisk,
                        crate::mame::MameAssetAttributePositions::Disk {
                            native,
                            compatibility,
                        },
                    ) => {
                        if let Some(writeable) = attributes.writeable {
                            ExecuteDsl::execute(
                                MameDiskCompatibilityInsert {
                                    occurrence_id: id.database_value(),
                                    writeable: writeable.as_bool(),
                                },
                                conn,
                            )?;
                        }
                        positions.queue(
                            conn,
                            Family::Disk,
                            &[id.database_value()],
                            native,
                            crate::mame::MameDiskAttribute::code,
                        )?;
                        positions.queue(
                            conn,
                            Family::DiskCompatibility,
                            &[id.database_value()],
                            compatibility,
                            crate::mame::MameDiskCompatibilityAttribute::code,
                        )?;
                    }
                    _ => {
                        return Err(crate::Error::InvalidPath(
                            "MAME attribute positions have the wrong asset family".into(),
                        ));
                    }
                }
            }
            NativeAssetFacts::MameSample {
                attribute_positions,
                ..
            } => positions.queue(
                conn,
                Family::Sample,
                &[id.database_value()],
                attribute_positions,
                crate::mame::MameSampleAttribute::code,
            )?,
            _ => {
                return Err(crate::Error::DatabaseSchema(
                    "non-MAME asset in MAME bulk positions".into(),
                ));
            }
        }
    }
    positions.flush(conn)?;
    Ok(())
}

fn insert_mame_positions(
    conn: &mut SqliteConnection,
    id: OccurrenceId,
    kind: RootClaimKind,
    positions: &crate::mame::MameAssetAttributePositions,
) -> crate::Result<()> {
    match (kind, positions) {
        (
            RootClaimKind::MameRom,
            crate::mame::MameAssetAttributePositions::Rom {
                native,
                compatibility,
            },
        ) => {
            mame_attributes::insert(
                conn,
                Family::Rom,
                &[id.database_value()],
                native,
                crate::mame::MameRomAttribute::code,
            )?;
            if !compatibility.is_empty() {
                mame_attributes::insert(
                    conn,
                    Family::RomCompatibility,
                    &[id.database_value()],
                    compatibility,
                    crate::mame::MameRomCompatibilityAttribute::code,
                )?;
            }
        }
        (
            RootClaimKind::MameDisk,
            crate::mame::MameAssetAttributePositions::Disk {
                native,
                compatibility,
            },
        ) => {
            mame_attributes::insert(
                conn,
                Family::Disk,
                &[id.database_value()],
                native,
                crate::mame::MameDiskAttribute::code,
            )?;
            if !compatibility.is_empty() {
                mame_attributes::insert(
                    conn,
                    Family::DiskCompatibility,
                    &[id.database_value()],
                    compatibility,
                    crate::mame::MameDiskCompatibilityAttribute::code,
                )?;
            }
        }
        _ => {
            return Err(crate::Error::InvalidPath(
                "MAME attribute positions have the wrong asset family".into(),
            ));
        }
    }
    Ok(())
}

fn insert_rom_compatibility(
    conn: &mut SqliteConnection,
    id: OccurrenceId,
    attributes: &crate::mame::MameAssetAttributes,
    declarations: &crate::mame::MameAssetDeclarations,
) -> crate::Result<()> {
    if declarations.md5_text.is_none()
        && attributes.sound_only.is_none()
        && attributes.dispose.is_none()
        && attributes.load_flag.is_none()
        && attributes.value.is_none()
        && attributes.inverted.is_none()
        && attributes.ovha.is_none()
        && attributes.no_thread.is_none()
    {
        return Ok(());
    }
    cached_sql(
        "INSERT INTO mame_rom_compatibility
        (occurrence_id,md5_text,sound_only,dispose,load_flag,value,inverted,ovha,no_thread)
        VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind::<BigInt, _>(id.database_value())
    .bind::<Nullable<Text>, _>(declarations.md5_text.as_deref())
    .bind::<Nullable<BigInt>, _>(attributes.sound_only.map(sqlite_mame_boolean))
    .bind::<Nullable<BigInt>, _>(attributes.dispose.map(sqlite_mame_boolean))
    .bind::<Nullable<Text>, _>(attributes.load_flag.as_deref())
    .bind::<Nullable<Text>, _>(attributes.value.as_deref())
    .bind::<Nullable<BigInt>, _>(attributes.inverted.map(sqlite_mame_boolean))
    .bind::<Nullable<Text>, _>(attributes.ovha.as_deref())
    .bind::<Nullable<BigInt>, _>(attributes.no_thread.map(sqlite_mame_boolean))
    .execute(conn)?;
    Ok(())
}

#[cfg(test)]
mod root_asset_dispatch_tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    use diesel::{
        Connection,
        connection::{InstrumentationEvent, SimpleConnection},
        sqlite::SqliteConnection,
    };

    use super::{
        MameDiskClaimInsert, MameDiskCompatibilityInsert, MameRomClaimInsert, NativeAssetFacts,
        RootClaimKind, SnapshotAsset, insert, insert_occurrence,
    };
    use crate::{domain::CatalogSetId, logiqx::RecordLocation};

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "SQLite fixture setup and dispatch assertions must fail loudly"
    )]
    fn root_asset_dispatch_uses_its_typed_source_without_a_database_lookup() -> crate::Result<()> {
        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
        connection.batch_execute(
            "CREATE TABLE catalog_sets (set_id INTEGER, source_element_kind TEXT);
            CREATE TABLE asset_occurrences (
                occurrence_id INTEGER PRIMARY KEY AUTOINCREMENT,
                record_id INTEGER, occurrence_order INTEGER, claim_kind TEXT, content_uuid BLOB
            );
            INSERT INTO catalog_sets(set_id, source_element_kind) VALUES (1, 'mame_machine');
            CREATE TABLE mame_samples (
                occurrence_id INTEGER, name TEXT, source_order INTEGER,
                source_line INTEGER, source_column INTEGER
            )",
        )?;
        let queried_record_kind = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&queried_record_kind);
        let cache_events = Arc::new(AtomicUsize::new(0));
        let observed_cache_events = Arc::clone(&cache_events);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| match event {
            InstrumentationEvent::StartQuery { query, .. }
                if query.to_string().contains("source_element_kind") =>
            {
                observed.store(true, Ordering::Relaxed);
            }
            InstrumentationEvent::CacheQuery { .. } => {
                observed_cache_events.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        });

        let asset = SnapshotAsset::filename_only(
            "sample.wav".into(),
            RecordLocation { line: 3, column: 8 },
            NativeAssetFacts::MameSample {
                source_order: 0,
                attribute_positions: Vec::new(),
            },
        );
        let occurrence = insert(
            &mut connection,
            CatalogSetId::try_from(1).expect("valid fixture set id"),
            0,
            &asset,
            None,
        )?;

        assert_eq!(occurrence.database_value(), 1);
        assert!(!queried_record_kind.load(Ordering::Relaxed));

        let next_id = insert_occurrence(
            &mut connection,
            CatalogSetId::try_from(1).expect("valid fixture set id"),
            1,
            RootClaimKind::MameSample,
            None,
        )?;
        assert_eq!(next_id.database_value(), 2);
        assert_eq!(cache_events.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[test]
    fn mame_media_claim_statements_reuse_prepared_queries() -> crate::Result<()> {
        let mut connection = SqliteConnection::establish(":memory:")
            .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
        connection.batch_execute(
            "CREATE TABLE mame_rom_claims (
                occurrence_id INTEGER, name TEXT, size_text TEXT, crc_text TEXT, sha1_text TEXT,
                evidence_scope TEXT, evidence_provenance TEXT, dump_status TEXT,
                source_line INTEGER, source_column INTEGER, region TEXT, bios TEXT,
                offset_text TEXT, optional INTEGER, source_order INTEGER,
                status_specified INTEGER, optional_specified INTEGER
            );
            CREATE TABLE mame_disk_claims (
                occurrence_id INTEGER, name TEXT, sha1_text TEXT, evidence_scope TEXT,
                evidence_provenance TEXT, dump_status TEXT, source_line INTEGER,
                source_column INTEGER, region TEXT, disk_index TEXT, writable INTEGER,
                optional INTEGER, source_order INTEGER, status_specified INTEGER,
                optional_specified INTEGER, writable_specified INTEGER
            );
            CREATE TABLE mame_disk_compatibility (occurrence_id INTEGER, writeable INTEGER);",
        )?;
        let cache_events = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&cache_events);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if matches!(event, InstrumentationEvent::CacheQuery { .. }) {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        for (occurrence_id, name) in [(1, "first"), (2, "second")] {
            diesel::RunQueryDsl::execute(
                MameRomClaimInsert {
                    occurrence_id,
                    name,
                    size_text: None,
                    crc_text: None,
                    sha1_text: None,
                    evidence_scope: "whole_asset",
                    dump_status: None,
                    source_line: occurrence_id,
                    source_column: 1,
                    region: None,
                    bios: None,
                    offset_text: None,
                    optional: false,
                    source_order: occurrence_id,
                    status_specified: false,
                    optional_specified: false,
                },
                &mut connection,
            )?;
            diesel::RunQueryDsl::execute(
                MameDiskClaimInsert {
                    occurrence_id,
                    name,
                    sha1_text: None,
                    evidence_scope: "whole_asset",
                    dump_status: None,
                    source_line: occurrence_id,
                    source_column: 1,
                    region: None,
                    disk_index: None,
                    writable: false,
                    optional: false,
                    source_order: occurrence_id,
                    status_specified: false,
                    optional_specified: false,
                    writable_specified: false,
                },
                &mut connection,
            )?;
            diesel::RunQueryDsl::execute(
                MameDiskCompatibilityInsert {
                    occurrence_id,
                    writeable: occurrence_id == 2,
                },
                &mut connection,
            )?;
        }

        assert_eq!(cache_events.load(Ordering::Relaxed), 3);
        Ok(())
    }
}
