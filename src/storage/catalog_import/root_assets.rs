use super::{NativeAssetFacts, SnapshotAsset, sqlite_mame_boolean};
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
    LogiqxSample,
    CmpRom,
    CmpSample,
    NoIntroPcFile,
}

impl RootClaimKind {
    fn for_record(kind: &str, role: &str) -> crate::Result<Self> {
        match (kind, role) {
            ("mame_machine", "rom") => Ok(Self::MameRom),
            ("mame_machine", "disk") => Ok(Self::MameDisk),
            ("logiqx_game", "rom") => Ok(Self::LogiqxRom),
            ("logiqx_game", "disk") => Ok(Self::LogiqxDisk),
            ("logiqx_game", "other") => Ok(Self::LogiqxSample),
            ("cmp_set", "rom") => Ok(Self::CmpRom),
            ("cmp_set", "other") => Ok(Self::CmpSample),
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
            insert_mame(conn, id, kind, asset)?;
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
        "INSERT INTO {} (occurrence_id,name,evidence_scope,evidence_provenance,merge_name, \
         md5_text,sha1_text, \
         dump_status,status_was_present,source_order,source_line,source_column{}) \
         VALUES (?,?,?,'source_declared',?,?,?,?,?,?,?,?{})",
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
        .bind::<Nullable<Text>, _>(asset.merge.as_deref())
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
    } = &asset.native
    else {
        return Err(crate::Error::InvalidPath(
            "MAME media entry has no native attributes".into(),
        ));
    };
    match kind {
        RootClaimKind::MameRom => {
            sql_query(
                "INSERT INTO mame_rom_claims
                (occurrence_id,name,size_text,crc_text,sha1_text,evidence_scope,evidence_provenance,
                 merge_name,dump_status,source_line,source_column,region,bios,offset_text,optional,
                 source_order,status_specified,optional_specified)
                 VALUES (?,?,?,?,?,?,'source_declared',?,?,?,?,?,?,?,?,?,?,?)",
            )
            .bind::<BigInt, _>(id.database_value())
            .bind::<Text, _>(&asset.name)
            .bind::<Nullable<Text>, _>(declarations.size_text.as_deref())
            .bind::<Nullable<Text>, _>(declarations.crc_text.as_deref())
            .bind::<Nullable<Text>, _>(declarations.sha1_text.as_deref())
            .bind::<Text, _>(asset.evidence_scope)
            .bind::<Nullable<Text>, _>(asset.merge.as_deref())
            .bind::<Nullable<Text>, _>(asset.dump_status.as_deref())
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column)
            .bind::<Nullable<Text>, _>(attributes.region.as_deref())
            .bind::<Nullable<Text>, _>(attributes.bios.as_deref())
            .bind::<Nullable<Text>, _>(declarations.offset_text.as_deref())
            .bind::<diesel::sql_types::Bool, _>(attributes.optional.as_bool())
            .bind::<BigInt, _>(*source_order)
            .bind::<diesel::sql_types::Bool, _>(attributes.status_specified)
            .bind::<diesel::sql_types::Bool, _>(attributes.optional_specified)
            .execute(conn)?;
            insert_rom_compatibility(conn, id, attributes, declarations)?;
        }
        RootClaimKind::MameDisk => {
            sql_query("INSERT INTO mame_disk_claims
                (occurrence_id,name,sha1_text,evidence_scope,evidence_provenance,merge_name,dump_status,
                 source_line,source_column,region,disk_index,writable,optional,source_order,
                 status_specified,optional_specified,writable_specified)
                 VALUES (?,?,?,?,'source_declared',?,?,?,?,?,?,?,?,?,?,?,?)")
                .bind::<BigInt,_>(id.database_value())
                .bind::<Text,_>(&asset.name)
                .bind::<Nullable<Text>,_>(declarations.sha1_text.as_deref())
                .bind::<Text,_>(asset.evidence_scope)
                .bind::<Nullable<Text>,_>(asset.merge.as_deref())
                .bind::<Nullable<Text>,_>(asset.dump_status.as_deref())
                .bind::<BigInt,_>(asset.location.line)
                .bind::<BigInt,_>(asset.location.column)
                .bind::<Nullable<Text>,_>(attributes.region.as_deref())
                .bind::<Nullable<Text>,_>(attributes.disk_index.as_deref())
                .bind::<diesel::sql_types::Bool,_>(attributes.writable.unwrap_or_default().as_bool())
                .bind::<diesel::sql_types::Bool,_>(attributes.optional.as_bool())
                .bind::<BigInt,_>(*source_order)
                .bind::<diesel::sql_types::Bool,_>(attributes.status_specified)
                .bind::<diesel::sql_types::Bool,_>(attributes.optional_specified)
                .bind::<diesel::sql_types::Bool,_>(attributes.writable_specified)
                .execute(conn)?;
            if let Some(writeable) = attributes.writeable {
                sql_query(
                    "INSERT INTO mame_disk_compatibility(occurrence_id,writeable) VALUES (?,?)",
                )
                .bind::<BigInt, _>(id.database_value())
                .bind::<diesel::sql_types::Bool, _>(writeable.as_bool())
                .execute(conn)?;
            }
        }
        _ => {
            return Err(crate::Error::InvalidPath(
                "non-MAME native media owner".into(),
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
    sql_query(
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
