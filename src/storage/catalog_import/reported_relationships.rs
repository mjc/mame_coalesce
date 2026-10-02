//! One identity issuer for source declarations; native tables own their literals.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::{MergeMediaKind, OccurrenceId, RelationshipAssertionKey, SnapshotKey};

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
        }
    }
}

/// Cannot be constructed until registry identity and reported subtype both exist.
pub(super) struct ReportedRelationshipId(i64);

impl ReportedRelationshipId {
    pub(super) const fn database_value(&self) -> i64 {
        self.0
    }
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
}

pub(super) fn register(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kind: ReportedReferenceKind,
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

    const fn table(&self) -> &'static str {
        match self {
            Self::Mame(MergeMediaKind::Rom) => "mame_rom_merges",
            Self::Mame(MergeMediaKind::Disk) => "mame_disk_merges",
            Self::Logiqx(_) => "logiqx_file_merges",
            Self::ClrMamePro => "clrmamepro_rom_merges",
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
    let mut columns = "occurrence_id,relationship_id,merge_name".to_owned();
    let mut placeholders = "?,?,?".to_owned();
    let stores_location = !matches!(owner, MergeOwner::ClrMamePro);
    if stores_location {
        columns.push_str(",source_line,source_column");
        placeholders.push_str(",?,?");
    }
    if owner.claim_kind().is_some() {
        columns.push_str(",claim_kind");
        placeholders.push_str(",?");
    }
    let query = sql_query(format!(
        "INSERT INTO {}({columns}) VALUES ({placeholders})",
        owner.table()
    ))
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(relationship.database_value())
    .bind::<Text, _>(literal);
    if stores_location {
        let query = query
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column);
        if let Some(claim_kind) = owner.claim_kind() {
            query.bind::<Text, _>(claim_kind).execute(connection)?;
        } else {
            query.execute(connection)?;
        }
    } else {
        query.execute(connection)?;
    }
    Ok(())
}
