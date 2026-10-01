use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::{
        CatalogRecordKind, CatalogRecordRef, CatalogSetId, DocumentLocation, RelationshipType,
        SnapshotKey,
    },
    mame_softwarelist::{
        LoadInstruction, SoftwareArea, SoftwareComponent, SoftwareDisk, SoftwareItem, SoftwareList,
        SoftwareListCatalog, SoftwarePart, SoftwareRom, SoftwareTextPosition,
    },
    storage::{
        catalog_content::{
            ContentDigestAssertions, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions, resolve_content_identity,
        },
        catalog_identity::{AllocatedOccurrence, OccurrenceId},
        relationships::{SourceRelationshipDraft, insert_source_assertion},
    },
};

#[derive(QueryableByName)]
struct NamespaceIdRow {
    #[diesel(sql_type = BigInt)]
    namespace_id: i64,
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

/// Import software list records and their source occurrences into the native catalog model.
pub(super) fn insert(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    catalog: &SoftwareListCatalog,
) -> crate::Result<()> {
    sql_query("INSERT INTO software_documents(snapshot_key, envelope_kind) VALUES (?, ?)")
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(catalog.root_kind.as_str())
        .execute(conn)?;
    if catalog.root_kind.as_str() == "plural_lists" {
        sql_query("INSERT INTO software_wrapper_headers(snapshot_key, build) VALUES (?, ?)")
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<Nullable<Text>, _>(catalog.build.as_deref())
            .execute(conn)?;
    }
    for (list_order, list) in catalog.lists.iter().enumerate() {
        insert_list(conn, snapshot_key, list, list_order)?;
    }
    Ok(())
}

fn insert_list(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    list: &SoftwareList,
    list_order: usize,
) -> crate::Result<()> {
    let namespace = sql_query(
        "INSERT INTO catalog_set_groups (snapshot_key, kind, list_order) \
         VALUES (?, 'software_list', ?) RETURNING set_group_id AS namespace_id",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<BigInt, _>(checked_order(list_order, "software lists")?)
    .get_result::<NamespaceIdRow>(conn)?
    .namespace_id;

    sql_query(
        "INSERT INTO software_lists \
         (namespace_id, source_order, name, description, notes, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(namespace)
    .bind::<BigInt, _>(checked_order(list.source_order, "software lists")?)
    .bind::<Text, _>(list.name.as_str())
    .bind::<Nullable<Text>, _>(list.description.as_deref())
    .bind::<Nullable<Text>, _>(list.notes.as_deref())
    .bind::<BigInt, _>(list.location.line)
    .bind::<BigInt, _>(list.location.column)
    .execute(conn)?;
    for position in &list.text_positions {
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

    for (item_order, item) in list.items.iter().enumerate() {
        insert_item(conn, snapshot_key, namespace, list, item, item_order)?;
    }
    Ok(())
}

fn insert_item(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    namespace_id: i64,
    list: &SoftwareList,
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
         (record_id, source_order, clone_of, supported, supported_specified, description, year, publisher, notes) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(record.as_i64())
    .bind::<BigInt, _>(checked_order(item.source_order, "software items")?)
    .bind::<Nullable<Text>, _>(
        item.clone_of
            .as_ref()
            .map(crate::mame_softwarelist::SoftwareItemName::as_str),
    )
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
        insert_source_assertion(
            conn,
            SourceRelationshipDraft {
                relation_type: RelationshipType::SourceParentClone,
                subject: CatalogRecordRef::new(
                    snapshot_key.clone(),
                    CatalogRecordKind::SoftwareItem,
                    super::super::catalog_reconciliation::record_key(&(
                        list.name.as_str(),
                        item.name.as_str(),
                    ))?,
                )
                .with_owner(record),
                target: CatalogRecordRef::new(
                    snapshot_key.clone(),
                    CatalogRecordKind::SoftwareItem,
                    super::super::catalog_reconciliation::record_key(&(
                        list.name.as_str(),
                        parent.as_str(),
                    ))?,
                ),
                source_field: "cloneof".to_owned(),
                source_location: Some(DocumentLocation {
                    line: item.location.line,
                    column: item.location.column,
                }),
                evidence: serde_json::json!({
                    "list_name": list.name.as_str(),
                    "target_item_name": parent.as_str()
                }),
            },
        )?;
    }

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
         (part_id, record_id, area_name, area_kind, area_order, source_order, declared_size_text, \
          width, width_specified, endianness, endianness_specified, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING area_id",
    )
    .bind::<BigInt, _>(part_id)
    .bind::<BigInt, _>(record.as_i64())
    .bind::<Text, _>(area.name.as_str())
    .bind::<Text, _>(area.kind.as_str())
    .bind::<BigInt, _>(checked_order(area_order, "software areas")?)
    .bind::<BigInt, _>(checked_order(area.source_order, "software areas")?)
    .bind::<Nullable<Text>, _>(area.declared_size_text.as_deref())
    .bind::<Nullable<BigInt>, _>(area.width.map(i64::from))
    .bind::<BigInt, _>(i64::from(area.width_specified))
    .bind::<Nullable<Text>, _>(
        area.endianness
            .map(crate::mame_softwarelist::Endianness::as_str),
    )
    .bind::<BigInt, _>(i64::from(area.endianness_specified))
    .bind::<BigInt, _>(area.location.line)
    .bind::<BigInt, _>(area.location.column)
    .get_result::<AreaIdRow>(conn)?
    .area_id;

    let mut declaration = None;
    for (component_order, component) in area.components.iter().enumerate() {
        insert_component(
            conn,
            record,
            area_id,
            component,
            checked_order(component_order, "software area components")?,
            occurrence_order,
            &mut declaration,
        )?;
    }
    Ok(())
}

fn insert_component(
    conn: &mut SqliteConnection,
    record: CatalogSetId,
    area_id: i64,
    component: &SoftwareComponent,
    component_order: i64,
    occurrence_order: &mut i64,
    declaration: &mut Option<OccurrenceId>,
) -> crate::Result<()> {
    let current_order = *occurrence_order;
    *occurrence_order = occurrence_order
        .checked_add(1)
        .ok_or_else(|| crate::Error::InvalidPath("too many software occurrences".into()))?;

    match component {
        SoftwareComponent::Rom(rom) => insert_rom_component(
            conn,
            record,
            area_id,
            rom,
            component_order,
            current_order,
            declaration,
        ),
        SoftwareComponent::Disk(disk) => {
            insert_disk_component(conn, record, area_id, disk, component_order, current_order)
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
) -> crate::Result<(OccurrenceId, ContentIdentityResolution)> {
    let resolution = if claim.identity_eligible() && source_hashes_usable {
        resolve_content_identity(conn, None, digests)?
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
    record: CatalogSetId,
    area_id: i64,
    rom: &SoftwareRom,
    component_order: i64,
    occurrence_order: i64,
    declaration: &mut Option<OccurrenceId>,
) -> crate::Result<()> {
    let claim = if matches!(
        rom.load,
        Some(
            crate::mame_softwarelist::LoadInstruction::Reload
                | crate::mame_softwarelist::LoadInstruction::Fill
                | crate::mame_softwarelist::LoadInstruction::Continue
                | crate::mame_softwarelist::LoadInstruction::ReloadPlain
                | crate::mame_softwarelist::LoadInstruction::Ignore
        )
    ) {
        SoftwareClaim::RomOperation
    } else {
        SoftwareClaim::RomDeclaration
    };
    let is_declaration = claim.is_rom_declaration();
    let digests = ContentDigestAssertions::new(
        "whole_asset",
        rom.crc.as_ref().map(<[u8; 4]>::as_slice),
        None,
        rom.sha1.as_ref().map(<[u8; 20]>::as_slice),
        None,
    );
    let source_hashes_usable = rom.name.is_some()
        && rom.crc_text.as_ref().is_none_or(|_| rom.crc.is_some())
        && rom.sha1_text.as_ref().is_none_or(|_| rom.sha1.is_some());
    let (occurrence, resolution) = allocate_occurrence(
        conn,
        record,
        occurrence_order,
        claim,
        source_hashes_usable,
        digests,
    )?;
    insert_rom_entry(conn, record, area_id, component_order, occurrence, rom)?;
    update_rom_declaration(conn, record, occurrence, is_declaration, declaration)?;

    let use_declaration = if matches!(
        rom.load,
        Some(crate::mame_softwarelist::LoadInstruction::Fill)
    ) {
        None
    } else if is_declaration {
        Some(occurrence)
    } else {
        *declaration
    };
    insert_rom_use(conn, record, occurrence, rom, use_declaration)?;

    record_occurrence_identity_evidence(conn, occurrence, digests, &resolution)?;
    Ok(())
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
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_rom_entries \
         (occurrence_id, record_id, area_id, component_order, source_order, name, evidence_scope, \
          size_text, offset_text, value, crc_text, sha1_text, dump_status, status_specified, \
          load_instruction, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, 'whole_asset', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        "INSERT INTO software_file_declarations (occurrence_id, record_id, declared_size) \
         VALUES (?, ?, NULL)",
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
    .bind::<Text, _>(rom_operation(rom.load))
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

const fn rom_operation(
    instruction: Option<crate::mame_softwarelist::LoadInstruction>,
) -> &'static str {
    use crate::mame_softwarelist::LoadInstruction as Load;
    match instruction {
        Some(Load::Continue) => "continue",
        Some(Load::Reload) => "reload",
        Some(Load::ReloadPlain) => "reload_plain",
        Some(Load::Ignore) => "ignore",
        Some(Load::Fill) => "fill",
        Some(
            Load::Load16Byte
            | Load::Load16Word
            | Load::Load16WordSwap
            | Load::Load32Byte
            | Load::Load32Word
            | Load::Load32WordSwap
            | Load::Load32Dword
            | Load::Load64Word
            | Load::Load64WordSwap,
        )
        | None => "load",
    }
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
