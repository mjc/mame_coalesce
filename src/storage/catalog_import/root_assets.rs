use super::{SnapshotAsset, sqlite_mame_boolean, sqlite_mame_offset};
use crate::{
    domain::CatalogSetId,
    storage::catalog_identity::{AllocatedOccurrence, OccurrenceId},
};
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

#[derive(QueryableByName)]
struct NativeRecordKind {
    #[diesel(sql_type = Text)]
    kind: String,
}

#[derive(Clone, Copy)]
enum RootClaimKind {
    MameRom,
    MameDisk,
    LogiqxRom,
    LogiqxDisk,
    CmpRom,
    NoIntroPcFile,
}

impl RootClaimKind {
    fn for_record(kind: &str, role: &str) -> crate::Result<Self> {
        match (kind, role) {
            ("mame_machine", "rom") => Ok(Self::MameRom),
            ("mame_machine", "disk") => Ok(Self::MameDisk),
            ("logiqx_game", "rom") => Ok(Self::LogiqxRom),
            ("logiqx_game", "disk") => Ok(Self::LogiqxDisk),
            ("cmp_set", "rom") => Ok(Self::CmpRom),
            ("no_intro_pc_game", "rom") => Ok(Self::NoIntroPcFile),
            _ => Err(crate::Error::InvalidPath(format!(
                "invalid native claim {kind}/{role}"
            ))),
        }
    }

    const fn code(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom",
            Self::MameDisk => "mame_disk",
            Self::LogiqxRom => "logiqx_rom",
            Self::LogiqxDisk => "logiqx_disk",
            Self::CmpRom => "cmp_rom",
            Self::NoIntroPcFile => "no_intro_pc_file",
        }
    }

    const fn table(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom_claims",
            Self::MameDisk => "mame_disk_claims",
            Self::LogiqxRom => "logiqx_rom_claims",
            Self::LogiqxDisk => "logiqx_disk_claims",
            Self::CmpRom => "cmp_rom_claims",
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
    size: Option<i64>,
    content_uuid: Option<Vec<u8>>,
) -> crate::Result<OccurrenceId> {
    let record_kind =
        sql_query("SELECT source_element_kind AS kind FROM catalog_sets WHERE set_id = ?")
            .bind::<BigInt, _>(record.as_i64())
            .get_result::<NativeRecordKind>(conn)?;
    let kind = RootClaimKind::for_record(&record_kind.kind, asset.role)?;
    let occurrence = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES (?, ?, ?, ?) RETURNING occurrence_id",
    )
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(order)
    .bind::<Text, _>(kind.code())
    .bind::<Nullable<Binary>, _>(content_uuid)
    .get_result::<AllocatedOccurrence>(conn)?;
    let id = OccurrenceId::from_database(occurrence.occurrence_id);
    match kind {
        RootClaimKind::MameRom | RootClaimKind::MameDisk => {
            insert_mame(conn, id, kind, asset, size)?;
        }
        RootClaimKind::LogiqxRom | RootClaimKind::LogiqxDisk => {
            sql_query(format!(
                "INSERT INTO {} (occurrence_id,name,size,evidence_scope,evidence_provenance, \
                 merge_name,dump_status,source_line,source_column,serial,date) \
                 VALUES (?, ?, ?, ?, 'source_declared', ?, ?, ?, ?, ?, ?)",
                kind.table()
            ))
            .bind::<BigInt, _>(id.database_value())
            .bind::<Text, _>(&asset.name)
            .bind::<Nullable<BigInt>, _>(size)
            .bind::<Text, _>(asset.evidence_scope)
            .bind::<Nullable<Text>, _>(asset.merge.as_deref())
            .bind::<Nullable<Text>, _>(asset.dump_status.as_deref())
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column)
            .bind::<Nullable<Text>, _>(asset.serial.as_deref())
            .bind::<Nullable<Text>, _>(asset.date.as_deref())
            .execute(conn)?;
        }
        RootClaimKind::CmpRom | RootClaimKind::NoIntroPcFile => {
            sql_query(format!(
                "INSERT INTO {} (occurrence_id,name,size,evidence_scope,evidence_provenance, \
                 merge_name,dump_status,source_line,source_column) \
                 VALUES (?, ?, ?, ?, 'source_declared', ?, ?, ?, ?)",
                kind.table()
            ))
            .bind::<BigInt, _>(id.database_value())
            .bind::<Text, _>(&asset.name)
            .bind::<Nullable<BigInt>, _>(size)
            .bind::<Text, _>(asset.evidence_scope)
            .bind::<Nullable<Text>, _>(asset.merge.as_deref())
            .bind::<Nullable<Text>, _>(asset.dump_status.as_deref())
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column)
            .execute(conn)?;
        }
    }
    Ok(id)
}

fn insert_mame(
    conn: &mut SqliteConnection,
    id: OccurrenceId,
    kind: RootClaimKind,
    asset: &SnapshotAsset,
    size: Option<i64>,
) -> crate::Result<()> {
    let attributes = asset.mame_attributes.as_ref();
    let offset = sqlite_mame_offset(attributes)?;
    sql_query(format!(
        "INSERT INTO {} (occurrence_id,name,size,evidence_scope,evidence_provenance,merge_name, \
         dump_status,source_line,source_column,region,bios,offset,optional,sound_only,dispose, \
         load_flag,value,inverted,ovha,no_thread,disk_index,writable,writeable) \
         VALUES (?, ?, ?, ?, 'source_declared', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        kind.table()
    ))
    .bind::<BigInt, _>(id.database_value())
    .bind::<Text, _>(&asset.name)
    .bind::<Nullable<BigInt>, _>(size)
    .bind::<Text, _>(asset.evidence_scope)
    .bind::<Nullable<Text>, _>(asset.merge.as_deref())
    .bind::<Nullable<Text>, _>(asset.dump_status.as_deref())
    .bind::<BigInt, _>(asset.location.line)
    .bind::<BigInt, _>(asset.location.column)
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.region.as_deref()))
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.bios.as_deref()))
    .bind::<Nullable<BigInt>, _>(offset)
    .bind::<Nullable<BigInt>, _>(attributes.map(|value| i64::from(value.optional.as_bool())))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.sound_only).map(sqlite_mame_boolean))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.dispose).map(sqlite_mame_boolean))
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.load_flag.as_deref()))
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.value.as_deref()))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.inverted).map(sqlite_mame_boolean))
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.ovha.as_deref()))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.no_thread).map(sqlite_mame_boolean))
    .bind::<Nullable<Text>, _>(attributes.and_then(|value| value.disk_index.as_deref()))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.writable).map(sqlite_mame_boolean))
    .bind::<Nullable<BigInt>, _>(attributes.and_then(|value| value.writeable).map(sqlite_mame_boolean))
    .execute(conn)?;
    Ok(())
}
