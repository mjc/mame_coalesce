mod positions;
use crate::mame_softwarelist::{
    SoftwareAreaAttributePositions, SoftwareDataAreaAttribute, SoftwareDipSwitchAttribute,
    SoftwareDipValueAttribute, SoftwareDiskAreaAttribute, SoftwareDiskAttribute,
    SoftwareItemAttribute, SoftwareListAttribute, SoftwareNamedValueAttribute,
    SoftwarePartAttribute, SoftwareRomAttribute, SoftwareWrapperAttribute,
};
use positions::PositionOwner;

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::{CatalogSetId, SnapshotKey},
    mame_softwarelist::{
        AreaKind, LoadInstruction, SoftwareArea, SoftwareComponent, SoftwareDisk,
        SoftwareDocumentHeader, SoftwareItem, SoftwareListHeader, SoftwareListMetadata,
        SoftwarePart, SoftwareRom, SoftwareTextPosition,
    },
    storage::{
        catalog_content::{
            ContentDigestAssertions, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions, resolve_content_identity,
        },
        catalog_files::SoftwareFileOperation,
        catalog_identity::{AllocatedOccurrence, OccurrenceId},
        software_rom_evidence::RomEvidence,
    },
};

#[derive(QueryableByName)]
struct NamespaceIdRow {
    #[diesel(sql_type = BigInt)]
    namespace_id: i64,
}

#[derive(QueryableByName)]
struct WrapperIdRow {
    #[diesel(sql_type = BigInt)]
    wrapper_id: i64,
}

#[derive(QueryableByName)]
struct RecordIdRow {
    #[diesel(sql_type = BigInt)]
    record_id: i64,
}

#[derive(QueryableByName)]
struct PartIdRow {
    #[diesel(sql_type = BigInt)]
    part_id: i64,
}

#[derive(QueryableByName)]
struct AreaIdRow {
    #[diesel(sql_type = BigInt)]
    area_id: i64,
}

struct PendingFileIdentity {
    occurrence: OccurrenceId,
    resolution: ContentIdentityResolution,
}

fn finish_file_identity(
    conn: &mut SqliteConnection,
    pending: Option<PendingFileIdentity>,
) -> crate::Result<()> {
    if let Some(pending) = pending {
        record_content_identity_conflict(conn, pending.occurrence, &pending.resolution)?;
    }
    Ok(())
}

/// Insert the immutable envelope owner from the source's opening element.
pub(super) fn insert_document(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &SoftwareDocumentHeader,
) -> crate::Result<()> {
    sql_query("INSERT INTO software_documents(snapshot_key, envelope_kind) VALUES (?, ?)")
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(header.root_kind.as_str())
        .execute(conn)?;
    if header.root_kind == crate::mame_softwarelist::SoftwareListRootKind::PluralLists {
        let wrapper = sql_query("INSERT INTO software_wrapper_headers(snapshot_key, build) VALUES (?, ?) RETURNING wrapper_id")
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<Nullable<Text>, _>(header.build.as_deref())
            .get_result::<WrapperIdRow>(conn)?.wrapper_id;
        positions::insert(
            conn,
            PositionOwner::Wrapper(wrapper),
            &header.attribute_positions,
            SoftwareWrapperAttribute::code,
        )?;
    }
    Ok(())
}

/// Allocate a list group before any item callbacks for that source list.
pub(super) fn insert_list_group(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    list_order: usize,
) -> crate::Result<i64> {
    let namespace = sql_query(
        "INSERT INTO catalog_set_groups (snapshot_key, kind, list_order) \
         VALUES (?, 'software_list', ?) RETURNING set_group_id AS namespace_id",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<BigInt, _>(checked_order(list_order, "software lists")?)
    .get_result::<NamespaceIdRow>(conn)?
    .namespace_id;
    Ok(namespace)
}

/// Insert final list facts and positions once late notes are known.
pub(super) fn insert_list_details(
    conn: &mut SqliteConnection,
    namespace: i64,
    header: &SoftwareListHeader,
    metadata: &SoftwareListMetadata,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_lists \
         (namespace_id, source_order, name, description, notes, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(namespace)
    .bind::<BigInt, _>(checked_order(header.source_order, "software lists")?)
    .bind::<Text, _>(header.name.as_str())
    .bind::<Nullable<Text>, _>(header.description.as_deref())
    .bind::<Nullable<Text>, _>(metadata.notes.as_deref())
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column)
    .execute(conn)?;
    positions::insert(
        conn,
        PositionOwner::List(namespace),
        &metadata.attribute_positions,
        SoftwareListAttribute::code,
    )?;
    for position in &metadata.text_positions {
        sql_query(
            "INSERT INTO software_list_text_positions \
                   (namespace_id, field_kind, source_order, source_line, source_column) \
                   VALUES (?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(namespace)
        .bind::<BigInt, _>(i64::from(position.field.as_code()))
        .bind::<BigInt, _>(checked_order(
            position.source_order,
            "software list text positions",
        )?)
        .bind::<BigInt, _>(position.location.line)
        .bind::<BigInt, _>(position.location.column)
        .execute(conn)?;
    }
    Ok(())
}

pub(super) fn insert_item(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    namespace_id: i64,
    item: &SoftwareItem,
    item_order: usize,
) -> crate::Result<()> {
    let record = sql_query(
        "INSERT INTO catalog_sets (set_group_id, source_element_kind, list_order, set_name, source_line, source_column) \
         VALUES (?, 'software_item', ?, ?, ?, ?) RETURNING set_id AS record_id",
    )
    .bind::<BigInt, _>(namespace_id)
    .bind::<BigInt, _>(checked_order(item_order, "software items")?)
    .bind::<Text, _>(item.name.as_str())
    .bind::<BigInt, _>(item.location.line)
    .bind::<BigInt, _>(item.location.column)
    .get_result::<RecordIdRow>(conn)?
    .record_id;
    let record = CatalogSetId::from_database(record);

    sql_query(
        "INSERT INTO software_items \
         (record_id, source_order, supported, supported_specified, description, year, publisher, notes) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(checked_order(item.source_order, "software items")?)
    .bind::<Nullable<Text>, _>(
        Some(item.supported.unwrap_or_default().as_str()),
    )
    .bind::<BigInt, _>(i64::from(item.supported_specified))
    .bind::<Text, _>(&item.description)
    .bind::<Text, _>(&item.year)
    .bind::<Text, _>(&item.publisher)
    .bind::<Nullable<Text>, _>(item.notes.as_deref())
    .execute(conn)?;

    insert_item_text_positions(conn, record, &item.text_positions)?;

    if let Some(parent) = &item.clone_of {
        super::reported_relationships::insert_reference(
            conn,
            snapshot_key,
            super::reported_relationships::ReferenceOwner::Software(record),
            parent.as_str(),
        )?;
    }

    positions::insert(
        conn,
        PositionOwner::Item(record.as_i64()),
        &item.attribute_positions,
        SoftwareItemAttribute::code,
    )?;
    insert_named_values(
        conn,
        "software_item_info",
        record,
        &item.info,
        "software item info",
    )?;
    insert_named_values(
        conn,
        "software_item_shared_features",
        record,
        &item.shared_features,
        "software item shared features",
    )?;

    let mut occurrence_order = 0_i64;
    for (part_order, part) in item.parts.iter().enumerate() {
        insert_part(conn, record, part, part_order, &mut occurrence_order)?;
    }
    Ok(())
}

fn insert_item_text_positions(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    positions: &[SoftwareTextPosition],
) -> crate::Result<()> {
    for position in positions {
        sql_query("INSERT INTO software_item_text_positions \
            (record_id, field_kind, source_order, source_line, source_column) VALUES (?, ?, ?, ?, ?)")
            .bind::<BigInt, _>(record.as_i64())
            .bind::<BigInt, _>(i64::from(position.field.as_code()))
            .bind::<BigInt, _>(checked_order(position.source_order, "software item text positions")?)
            .bind::<BigInt, _>(position.location.line)
            .bind::<BigInt, _>(position.location.column)
            .execute(conn)?;
    }
    Ok(())
}

fn insert_named_values(
    conn: &mut SqliteConnection,
    table: &str,
    record: CatalogSetId,
    values: &[crate::mame_softwarelist::NamedValue],
    kind: &str,
) -> crate::Result<()> {
    for (order, value) in values.iter().enumerate() {
        sql_query(format!(
            "INSERT INTO {table} (record_id, value_order, source_order, name, value, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        ))
        .bind::<BigInt, _>(record.as_i64())
        .bind::<BigInt, _>(checked_order(order, kind)?)
        .bind::<BigInt, _>(checked_order(value.source_order, kind)?)
        .bind::<Text, _>(&value.name)
        .bind::<Nullable<Text>, _>(value.value.as_deref())
        .bind::<BigInt, _>(value.location.line)
        .bind::<BigInt, _>(value.location.column)
        .execute(conn)?;
        let owner = match table {
            "software_item_info" => {
                PositionOwner::Info(record.as_i64(), checked_order(order, kind)?)
            }
            "software_item_shared_features" => {
                PositionOwner::SharedFeature(record.as_i64(), checked_order(order, kind)?)
            }
            _ => {
                return Err(crate::Error::XmlValidation(
                    "unknown software named-value owner".into(),
                ));
            }
        };
        positions::insert(
            conn,
            owner,
            &value.attribute_positions,
            SoftwareNamedValueAttribute::code,
        )?;
    }
    Ok(())
}

fn insert_part(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    part: &SoftwarePart,
    part_order: usize,
    occurrence_order: &mut i64,
) -> crate::Result<()> {
    let part_id = sql_query(
        "INSERT INTO software_parts \
         (record_id, part_name, part_order, source_order, interface, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING part_id",
    )
    .bind::<BigInt, _>(record.as_i64())
    .bind::<Text, _>(part.name.as_str())
    .bind::<BigInt, _>(checked_order(part_order, "software parts")?)
    .bind::<BigInt, _>(checked_order(part.source_order, "software parts")?)
    .bind::<Text, _>(&part.interface)
    .bind::<BigInt, _>(part.location.line)
    .bind::<BigInt, _>(part.location.column)
    .get_result::<PartIdRow>(conn)?
    .part_id;
    positions::insert(
        conn,
        PositionOwner::Part(part_id),
        &part.attribute_positions,
        SoftwarePartAttribute::code,
    )?;

    for (value_order, value) in part.features.iter().enumerate() {
        sql_query(
            "INSERT INTO software_part_features \
             (part_id, value_order, source_order, name, value, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(part_id)
        .bind::<BigInt, _>(checked_order(value_order, "software part features")?)
        .bind::<BigInt, _>(checked_order(value.source_order, "software part features")?)
        .bind::<Text, _>(&value.name)
        .bind::<Nullable<Text>, _>(value.value.as_deref())
        .bind::<BigInt, _>(value.location.line)
        .bind::<BigInt, _>(value.location.column)
        .execute(conn)?;
        positions::insert(
            conn,
            PositionOwner::Feature(
                part_id,
                checked_order(value_order, "software part features")?,
            ),
            &value.attribute_positions,
            SoftwareNamedValueAttribute::code,
        )?;
    }
    for (switch_order, switch) in part.dipswitches.iter().enumerate() {
        let switch_order = checked_order(switch_order, "software part DIP switches")?;
        sql_query(
            "INSERT INTO software_part_dipswitches \
             (part_id, dipswitch_order, source_order, name, tag, mask, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(part_id)
        .bind::<BigInt, _>(switch_order)
        .bind::<BigInt, _>(checked_order(switch.source_order, "software DIP switches")?)
        .bind::<Text, _>(switch.name.as_str())
        .bind::<Text, _>(&switch.tag)
        .bind::<Text, _>(&switch.mask)
        .bind::<BigInt, _>(switch.location.line)
        .bind::<BigInt, _>(switch.location.column)
        .execute(conn)?;
        positions::insert(
            conn,
            PositionOwner::DipSwitch(part_id, switch_order),
            &switch.attribute_positions,
            SoftwareDipSwitchAttribute::code,
        )?;
        for (value_order, value) in switch.values.iter().enumerate() {
            sql_query(
                "INSERT INTO software_part_dip_values \
                 (part_id, dipswitch_order, value_order, source_order, name, value, is_default, default_specified, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(part_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(value_order, "software DIP values")?)
            .bind::<BigInt, _>(checked_order(value.source_order, "software DIP values")?)
            .bind::<Text, _>(&value.name)
            .bind::<Text, _>(&value.value)
            .bind::<BigInt, _>(i64::from(value.is_default))
            .bind::<BigInt, _>(i64::from(value.default_specified))
            .bind::<BigInt, _>(value.location.line)
            .bind::<BigInt, _>(value.location.column)
            .execute(conn)?;
            positions::insert(
                conn,
                PositionOwner::DipValue(
                    part_id,
                    switch_order,
                    checked_order(value_order, "software DIP values")?,
                ),
                &value.attribute_positions,
                SoftwareDipValueAttribute::code,
            )?;
        }
    }

    for (area_order, area) in part.areas.iter().enumerate() {
        insert_area(conn, record, part_id, area, area_order, occurrence_order)?;
    }
    Ok(())
}

fn insert_area(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    part_id: i64,
    area: &SoftwareArea,
    area_order: usize,
    occurrence_order: &mut i64,
) -> crate::Result<()> {
    let area_id = sql_query(
        "INSERT INTO software_areas \
         (part_id, record_id, area_kind, area_order) VALUES (?, ?, ?, ?) RETURNING area_id",
    )
    .bind::<BigInt, _>(part_id)
    .bind::<BigInt, _>(record.as_i64())
    .bind::<Text, _>(area.kind.as_str())
    .bind::<BigInt, _>(checked_order(area_order, "software areas")?)
    .get_result::<AreaIdRow>(conn)?
    .area_id;

    match area.kind {
        AreaKind::Data => {
            let size = area.declared_size_text.as_deref().ok_or_else(|| {
                crate::Error::InvalidPath("data area lacks its declared size".into())
            })?;
            let width = area.width.ok_or_else(|| {
                crate::Error::InvalidPath("data area lacks its effective width".into())
            })?;
            let endianness = area.endianness.ok_or_else(|| {
                crate::Error::InvalidPath("data area lacks its effective endianness".into())
            })?;
            sql_query(
                "INSERT INTO software_data_areas \
                (area_id, area_name, source_order, declared_size_text, width, width_specified, \
                 endianness, endianness_specified, source_line, source_column) \
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(area_id)
            .bind::<Text, _>(area.name.as_str())
            .bind::<BigInt, _>(checked_order(area.source_order, "software areas")?)
            .bind::<Text, _>(size)
            .bind::<BigInt, _>(i64::from(width))
            .bind::<BigInt, _>(i64::from(area.width_specified))
            .bind::<Text, _>(endianness.as_str())
            .bind::<BigInt, _>(i64::from(area.endianness_specified))
            .bind::<BigInt, _>(area.location.line)
            .bind::<BigInt, _>(area.location.column)
            .execute(conn)?;
        }
        AreaKind::Disk => {
            sql_query("INSERT INTO software_disk_areas \
                (area_id, area_name, source_order, source_line, source_column) VALUES (?, ?, ?, ?, ?)")
                .bind::<BigInt, _>(area_id)
                .bind::<Text, _>(area.name.as_str())
                .bind::<BigInt, _>(checked_order(area.source_order, "software areas")?)
                .bind::<BigInt, _>(area.location.line)
                .bind::<BigInt, _>(area.location.column)
                .execute(conn)?;
        }
    }

    match &area.attribute_positions {
        SoftwareAreaAttributePositions::Data(values) if area.kind == AreaKind::Data => {
            positions::insert(
                conn,
                PositionOwner::DataArea(area_id),
                values,
                SoftwareDataAreaAttribute::code,
            )?;
        }
        SoftwareAreaAttributePositions::Disk(values) if area.kind == AreaKind::Disk => {
            positions::insert(
                conn,
                PositionOwner::DiskArea(area_id),
                values,
                SoftwareDiskAreaAttribute::code,
            )?;
        }
        _ => {
            return Err(crate::Error::XmlValidation(
                "software attribute area subtype mismatch".into(),
            ));
        }
    }
    insert_area_components(conn, record, area_id, area, occurrence_order)
}

struct AreaImport<'a> {
    record: CatalogSetId,
    area_id: i64,
    occurrence_order: &'a mut i64,
    declaration: Option<OccurrenceId>,
}

fn insert_area_components(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    area_id: i64,
    area: &SoftwareArea,
    occurrence_order: &mut i64,
) -> crate::Result<()> {
    let mut context = AreaImport {
        record,
        area_id,
        occurrence_order,
        declaration: None,
    };
    let mut pending = None;
    for (component_order, component) in area.components.iter().enumerate() {
        if !matches!(component, SoftwareComponent::Rom(rom) if !matches!(
            SoftwareFileOperation::from_instruction(rom.load),
            SoftwareFileOperation::Load | SoftwareFileOperation::Fill
        )) {
            // The previous declaration's complete native first run must exist
            // before capturing its evidence or resolving another candidate.
            finish_file_identity(conn, pending.take())?;
        }
        let file_size = crate::software_loading::file_verification_length(
            area.components
                .iter()
                .skip(component_order)
                .map(|component| match component {
                    SoftwareComponent::Rom(rom) => (
                        rom.load,
                        rom.size_text
                            .as_deref()
                            .and_then(|text| crate::mame_softwarelist::parse_number(text).ok()),
                    ),
                    SoftwareComponent::Disk(_) => (Some(LoadInstruction::Fill), None),
                }),
        )
        .and_then(|size| i64::try_from(size).ok());
        let identity = insert_component(
            conn,
            &mut context,
            component,
            checked_order(component_order, "software area components")?,
            file_size,
        )?;
        if identity.is_some() {
            pending = identity;
        }
    }
    finish_file_identity(conn, pending)?;
    Ok(())
}

fn insert_component(
    conn: &mut SqliteConnection,
    context: &mut AreaImport<'_>,
    component: &SoftwareComponent,
    component_order: i64,
    file_size: Option<i64>,
) -> crate::Result<Option<PendingFileIdentity>> {
    let current_order = *context.occurrence_order;
    *context.occurrence_order = context
        .occurrence_order
        .checked_add(1)
        .ok_or_else(|| crate::Error::InvalidPath("too many software occurrences".into()))?;

    match component {
        SoftwareComponent::Rom(rom) => insert_rom_component(
            conn,
            context,
            rom,
            component_order,
            current_order,
            file_size,
        ),
        SoftwareComponent::Disk(disk) => {
            insert_disk_component(
                conn,
                context.record,
                context.area_id,
                disk,
                component_order,
                current_order,
            )?;
            Ok(None)
        }
    }
}

#[derive(Clone, Copy)]
enum SoftwareClaim {
    RomDeclaration,
    RomOperation,
    DiskEntry,
}

impl SoftwareClaim {
    const fn code(self) -> &'static str {
        match self {
            Self::RomDeclaration => "software_rom_entry",
            Self::RomOperation => "software_rom_operation",
            Self::DiskEntry => "software_disk_entry",
        }
    }

    const fn identity_eligible(self) -> bool {
        match self {
            Self::RomDeclaration | Self::DiskEntry => true,
            Self::RomOperation => false,
        }
    }

    const fn is_rom_declaration(self) -> bool {
        match self {
            Self::RomDeclaration => true,
            Self::RomOperation | Self::DiskEntry => false,
        }
    }
}

fn allocate_occurrence(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    occurrence_order: i64,
    claim: SoftwareClaim,
    source_hashes_usable: bool,
    digests: ContentDigestAssertions<'_>,
    file_size: Option<i64>,
) -> crate::Result<(OccurrenceId, ContentIdentityResolution)> {
    let resolution = if claim.identity_eligible() && source_hashes_usable {
        resolve_content_identity(conn, file_size, digests)?
    } else {
        ContentIdentityResolution::NoEligibleEvidence
    };
    let content_uuid = resolution
        .content_id()
        .map(|content_id| content_id.as_bytes().to_vec());
    let allocated = sql_query(
        "INSERT INTO asset_occurrences (record_id, occurrence_order, claim_kind, content_uuid) \
         VALUES (?, ?, ?, ?) RETURNING occurrence_id",
    )
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(occurrence_order)
    .bind::<Text, _>(claim.code())
    .bind::<Nullable<Binary>, _>(content_uuid)
    .get_result::<AllocatedOccurrence>(conn)?;
    let occurrence = OccurrenceId::from_database(allocated.occurrence_id);

    Ok((occurrence, resolution))
}

fn insert_rom_component(
    conn: &mut SqliteConnection,
    context: &mut AreaImport<'_>,
    rom: &SoftwareRom,
    component_order: i64,
    occurrence_order: i64,
    file_size: Option<i64>,
) -> crate::Result<Option<PendingFileIdentity>> {
    let record = context.record;
    let area_id = context.area_id;
    let declaration = &mut context.declaration;
    let claim = if SoftwareFileOperation::from_instruction(rom.load) == SoftwareFileOperation::Load
    {
        SoftwareClaim::RomDeclaration
    } else {
        SoftwareClaim::RomOperation
    };
    let is_declaration = claim.is_rom_declaration();
    let evidence = RomEvidence::classify(
        is_declaration,
        rom.name
            .as_ref()
            .map(crate::mame_softwarelist::ComponentName::as_str),
        rom.status == Some(crate::mame_softwarelist::DumpStatus::NoDump),
    );
    let digests = ContentDigestAssertions::new(
        evidence.scope(),
        rom.crc.as_ref().map(<[u8; 4]>::as_slice),
        None,
        rom.sha1.as_ref().map(<[u8; 20]>::as_slice),
        None,
    );
    let source_hashes_usable = evidence == RomEvidence::WholeFile
        && rom.crc_text.as_ref().is_none_or(|_| rom.crc.is_some())
        && rom.sha1_text.as_ref().is_none_or(|_| rom.sha1.is_some());
    let (occurrence, resolution) = allocate_occurrence(
        conn,
        record,
        occurrence_order,
        claim,
        source_hashes_usable,
        digests,
        file_size,
    )?;
    insert_rom_entry(
        conn,
        record,
        area_id,
        component_order,
        occurrence,
        rom,
        evidence,
    )?;
    update_rom_declaration(conn, record, occurrence, is_declaration, declaration)?;

    let use_declaration = if matches!(
        rom.load,
        Some(crate::mame_softwarelist::LoadInstruction::Fill)
    ) {
        *declaration = None;
        None
    } else if is_declaration {
        Some(occurrence)
    } else {
        *declaration
    };
    insert_rom_use(conn, record, occurrence, rom, use_declaration)?;

    record_occurrence_digest_assertions(conn, occurrence, digests, "source_declared")?;
    Ok(is_declaration.then_some(PendingFileIdentity {
        occurrence,
        resolution,
    }))
}

fn record_occurrence_identity_evidence(
    conn: &mut SqliteConnection,
    occurrence: OccurrenceId,
    digests: ContentDigestAssertions<'_>,
    resolution: &ContentIdentityResolution,
) -> crate::Result<()> {
    record_occurrence_digest_assertions(conn, occurrence, digests, "source_declared")?;
    record_content_identity_conflict(conn, occurrence, resolution)
}

fn insert_rom_entry(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    area_id: i64,
    component_order: i64,
    occurrence: OccurrenceId,
    rom: &SoftwareRom,
    evidence: RomEvidence,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_rom_entries \
         (occurrence_id, record_id, area_id, component_order, source_order, name, evidence_scope, \
          size_text, offset_text, value, crc_text, sha1_text, dump_status, status_specified, \
          load_instruction, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(area_id)
    .bind::<BigInt, _>(component_order)
    .bind::<BigInt, _>(checked_order(rom.source_order, "software ROM entries")?)
    .bind::<Nullable<Text>, _>(
        rom.name
            .as_ref()
            .map(crate::mame_softwarelist::ComponentName::as_str),
    )
    .bind::<Text, _>(evidence.scope())
    .bind::<Nullable<Text>, _>(rom.size_text.as_deref())
    .bind::<Nullable<Text>, _>(rom.offset_text.as_deref())
    .bind::<Nullable<Text>, _>(rom.value.as_deref())
    .bind::<Nullable<Text>, _>(rom.crc_text.as_deref())
    .bind::<Nullable<Text>, _>(rom.sha1_text.as_deref())
    .bind::<Nullable<Text>, _>(Some(rom.status.unwrap_or_default().as_str()))
    .bind::<BigInt, _>(i64::from(rom.status_specified))
    .bind::<Nullable<Text>, _>(rom.load.as_ref().map(LoadInstruction::as_str))
    .bind::<BigInt, _>(rom.location.line)
    .bind::<BigInt, _>(rom.location.column)
    .execute(conn)?;
    positions::insert(
        conn,
        PositionOwner::Rom(occurrence.database_value()),
        &rom.attribute_positions,
        SoftwareRomAttribute::code,
    )?;
    Ok(())
}

fn update_rom_declaration(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    occurrence: OccurrenceId,
    is_declaration: bool,
    declaration: &mut Option<OccurrenceId>,
) -> crate::Result<()> {
    if !is_declaration {
        return Ok(());
    }
    sql_query(
        "INSERT INTO software_file_declarations (occurrence_id, record_id) \
         VALUES (?, ?)",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(record.as_i64())
    .execute(conn)?;
    *declaration = Some(occurrence);
    Ok(())
}

fn insert_rom_use(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    occurrence: OccurrenceId,
    rom: &SoftwareRom,
    declaration: Option<OccurrenceId>,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_file_uses \
         (occurrence_id, record_id, declaration_occurrence_id, operation) \
         VALUES (?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(record.as_i64())
    .bind::<Nullable<BigInt>, _>(declaration.map(OccurrenceId::database_value))
    .bind::<Text, _>(SoftwareFileOperation::from_instruction(rom.load).as_str())
    .execute(conn)?;
    Ok(())
}

fn insert_disk_component(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    area_id: i64,
    disk: &SoftwareDisk,
    component_order: i64,
    occurrence_order: i64,
) -> crate::Result<()> {
    let sha1 = disk
        .requirement
        .expected_sha1()
        .map(|digest| *digest.as_bytes());
    let digests = ContentDigestAssertions::new(
        disk.requirement.digest_scope().as_str(),
        None,
        None,
        sha1.as_ref().map(<[u8; 20]>::as_slice),
        None,
    );
    let source_hashes_usable = disk.sha1_text.as_ref().is_none_or(|_| sha1.is_some());
    let (occurrence, resolution) = allocate_occurrence(
        conn,
        record,
        occurrence_order,
        SoftwareClaim::DiskEntry,
        source_hashes_usable,
        digests,
        None,
    )?;

    sql_query(
        "INSERT INTO software_disk_entries \
         (occurrence_id, record_id, area_id, component_order, source_order, name, evidence_scope, \
          sha1_text, dump_status, status_specified, writeable, writeable_specified, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(area_id)
    .bind::<BigInt, _>(component_order)
    .bind::<BigInt, _>(checked_order(disk.source_order, "software disk entries")?)
    .bind::<Text, _>(disk.requirement.name().as_str())
    .bind::<Text, _>(disk.requirement.digest_scope().as_str())
    .bind::<Nullable<Text>, _>(disk.sha1_text.as_deref())
    .bind::<Nullable<Text>, _>(Some(disk.status.unwrap_or_default().as_str()))
    .bind::<BigInt, _>(i64::from(disk.status_specified))
    .bind::<Nullable<BigInt>, _>(Some(i64::from(disk.writeable.unwrap_or(false))))
    .bind::<BigInt, _>(i64::from(disk.writeable_specified))
    .bind::<BigInt, _>(disk.location.line)
    .bind::<BigInt, _>(disk.location.column)
    .execute(conn)?;
    positions::insert(
        conn,
        PositionOwner::Disk(occurrence.database_value()),
        &disk.attribute_positions,
        SoftwareDiskAttribute::code,
    )?;
    sql_query(
        "INSERT INTO software_file_uses \
         (occurrence_id, record_id, declaration_occurrence_id, operation) \
         VALUES (?, ?, NULL, 'disk')",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .bind::<BigInt, _>(record.as_i64())
    .execute(conn)?;

    record_occurrence_identity_evidence(conn, occurrence, digests, &resolution)?;
    Ok(())
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
