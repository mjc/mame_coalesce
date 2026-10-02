//! One identity issuer for source declarations; native tables own their literals.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
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
