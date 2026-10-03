use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    database::Database,
    domain::{CatalogRegistryId, CatalogSetId, SnapshotKey},
    logiqx::RecordLocation,
    mame::{MachineFlags, MachineSwitchKind, MameBoolean},
    storage::{catalog_content::registry_id, catalog_identity::OccurrenceId},
};

use super::{
    Machine, MachineAssetKind, MachineAssetReference, MachineCursor, MachineDependency,
    MachinePage, MachinePageLimit, MachineQueryError, MachineSnapshot, MachineSwitch,
    MachineSwitchLocation, MachineSwitchLocationKind, MachineSwitchValue, MachineSwitchValueKind,
    MameDocumentFacts,
};

type QueryResult<T> = Result<T, MachineQueryError>;

macro_rules! row {
    ($name:ident { $($field:ident: $rust_ty:ty => $sql_ty:ty),* $(,)? }) => {
        #[derive(QueryableByName)]
        struct $name {
            $(#[diesel(sql_type = $sql_ty)] $field: $rust_ty,)*
        }
    };
}

row!(SnapshotRow {
    document_id: i64 => BigInt,
    registry_uuid: Vec<u8> => Binary,
    snapshot_key: String => Text,
    source_key: String => Text,
    source_name: String => Text,
    catalog_key: String => Text,
    catalog_name: String => Text,
    document_key: String => Text,
    interpretation_key: String => Text,
    format: String => Text,
    build: Option<String> => Nullable<Text>,
    debug: i64 => BigInt,
    debug_specified: i64 => BigInt,
    config_version: String => Text,
    header_line: i64 => BigInt,
    header_column: i64 => BigInt,
});

row!(MachineRow {
    id: i64 => BigInt,
    name: String => Text,
    list_order: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
    native_id: Option<i64> => Nullable<BigInt>,
    source_file: Option<String> => Nullable<Text>,
    description: Option<String> => Nullable<Text>,
    description_source_order: Option<i64> => Nullable<BigInt>,
    description_line: Option<i64> => Nullable<BigInt>,
    description_column: Option<i64> => Nullable<BigInt>,
    year: Option<String> => Nullable<Text>,
    year_source_order: Option<i64> => Nullable<BigInt>,
    year_line: Option<i64> => Nullable<BigInt>,
    year_column: Option<i64> => Nullable<BigInt>,
    manufacturer: Option<String> => Nullable<Text>,
    manufacturer_source_order: Option<i64> => Nullable<BigInt>,
    manufacturer_line: Option<i64> => Nullable<BigInt>,
    manufacturer_column: Option<i64> => Nullable<BigInt>,
    is_device: Option<i64> => Nullable<BigInt>,
    is_device_specified: Option<i64> => Nullable<BigInt>,
    runnable: Option<i64> => Nullable<BigInt>,
    runnable_specified: Option<i64> => Nullable<BigInt>,
    is_bios: Option<i64> => Nullable<BigInt>,
    is_bios_specified: Option<i64> => Nullable<BigInt>,
    is_mechanical: Option<i64> => Nullable<BigInt>,
    is_mechanical_specified: Option<i64> => Nullable<BigInt>,
    is_consumable: Option<i64> => Nullable<BigInt>,
    is_consumable_specified: Option<i64> => Nullable<BigInt>,
    attributes_line: Option<i64> => Nullable<BigInt>,
    attributes_column: Option<i64> => Nullable<BigInt>,
    group_snapshot_ok: Option<i64> => Nullable<BigInt>,
    group_kind: Option<String> => Nullable<Text>,
    source_element_kind: String => Text,
});

row!(PositionRow {
    owner_id: i64 => BigInt,
    source_order: i64 => BigInt,
    child_kind: String => Text,
    child_order: i64 => BigInt,
});
row!(BiosRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    name: String => Text,
    description: String => Text,
    is_default: i64 => BigInt,
    default_specified: i64 => BigInt,
    source_order: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(CloneLinkRow {
    owner_id: i64 => BigInt,
    target_name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DependencyRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    dependency_kind: String => Text,
    target_name: String => Text,
    reference_tag: Option<String> => Nullable<Text>,
    source_order: Option<i64> => Nullable<BigInt>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(AssetRow {
    owner_id: i64 => BigInt,
    occurrence_id: i64 => BigInt,
    child_order: i64 => BigInt,
    source_order: Option<i64> => Nullable<BigInt>,
    line: Option<i64> => Nullable<BigInt>,
    column: Option<i64> => Nullable<BigInt>,
    claim_kind: String => Text,
});
row!(SwitchRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    kind: String => Text,
    name: String => Text,
    tag: String => Text,
    mask: String => Text,
    source_order: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(ConditionRow {
    owner_id: i64 => BigInt,
    switch_order: i64 => BigInt,
    child_order: i64 => BigInt,
    condition_order: i64 => BigInt,
    tag: String => Text,
    mask: String => Text,
    relation: String => Text,
    value: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SwitchLocationRow {
    owner_id: i64 => BigInt,
    switch_order: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    name: String => Text,
    number: String => Text,
    inverted: i64 => BigInt,
    inverted_specified: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SwitchValueRow {
    owner_id: i64 => BigInt,
    switch_order: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    name: String => Text,
    value: String => Text,
    is_default: i64 => BigInt,
    default_specified: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SampleRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(ChipRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    tag: Option<String> => Nullable<Text>,
    kind: String => Text,
    clock: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DisplayRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    tag: Option<String> => Nullable<Text>,
    kind: String => Text,
    rotation: Option<String> => Nullable<Text>,
    flip_x: i64 => BigInt,
    flip_x_specified: i64 => BigInt,
    width: Option<String> => Nullable<Text>,
    height: Option<String> => Nullable<Text>,
    refresh: String => Text,
    pixel_clock: Option<String> => Nullable<Text>,
    horizontal_total: Option<String> => Nullable<Text>,
    horizontal_blank_end: Option<String> => Nullable<Text>,
    horizontal_blank_start: Option<String> => Nullable<Text>,
    vertical_total: Option<String> => Nullable<Text>,
    vertical_blank_end: Option<String> => Nullable<Text>,
    vertical_blank_start: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SoundRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    channels: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(InputRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    service: i64 => BigInt,
    service_specified: i64 => BigInt,
    tilt: i64 => BigInt,
    tilt_specified: i64 => BigInt,
    players: String => Text,
    coins: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(ControlRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    row_order: i64 => BigInt,
    control_type: String => Text,
    player: Option<String> => Nullable<Text>,
    buttons: Option<String> => Nullable<Text>,
    minimum: Option<String> => Nullable<Text>,
    maximum: Option<String> => Nullable<Text>,
    sensitivity: Option<String> => Nullable<Text>,
    keydelta: Option<String> => Nullable<Text>,
    reverse: i64 => BigInt,
    reverse_specified: i64 => BigInt,
    ways: Option<String> => Nullable<Text>,
    ways2: Option<String> => Nullable<Text>,
    ways3: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(PortRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    tag: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(AnalogRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    row_order: i64 => BigInt,
    mask: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(AdjusterRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    default_value: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DriverRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    status: String => Text,
    emulation: String => Text,
    cocktail: Option<String> => Nullable<Text>,
    savestate: String => Text,
    requires_artwork: i64 => BigInt,
    requires_artwork_specified: i64 => BigInt,
    unofficial: i64 => BigInt,
    unofficial_specified: i64 => BigInt,
    no_sound_hardware: i64 => BigInt,
    no_sound_hardware_specified: i64 => BigInt,
    incomplete: i64 => BigInt,
    incomplete_specified: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(FeatureRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    kind: String => Text,
    status: Option<String> => Nullable<Text>,
    overall: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DeviceRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    kind: String => Text,
    tag: Option<String> => Nullable<Text>,
    fixed_image: Option<String> => Nullable<Text>,
    mandatory: Option<String> => Nullable<Text>,
    interface: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DeviceInstanceRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    brief_name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(DeviceExtensionRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    row_order: i64 => BigInt,
    name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SlotRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SlotOptionRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    row_order: i64 => BigInt,
    name: String => Text,
    devname: String => Text,
    is_default: i64 => BigInt,
    default_specified: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(SoftwareListRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    tag: String => Text,
    name: String => Text,
    status: String => Text,
    filter: Option<String> => Nullable<Text>,
    line: i64 => BigInt,
    column: i64 => BigInt,
});
row!(RamOptionRow {
    owner_id: i64 => BigInt,
    element_order: i64 => BigInt,
    name: String => Text,
    default_value: Option<String> => Nullable<Text>,
    text: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
});

pub(super) fn machines_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&MachineCursor>,
    limit: MachinePageLimit,
) -> QueryResult<MachinePage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let generation = registry_id(connection)?;
        validate_cursor(cursor, generation, snapshot)?;
        let provenance = snapshot_row(connection, snapshot, generation)?;
        let bound = limit
            .database_value()?
            .checked_add(1)
            .ok_or(MachineQueryError::PageLimitOverflow)?;
        let rows = match cursor {
            Some(cursor) => sql_query(super::queries::next_machine_page())
                .bind::<Text, _>(snapshot.as_str())
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(cursor.order)
                .bind::<BigInt, _>(cursor.owner_id.as_i64())
                .bind::<BigInt, _>(bound)
                .load::<MachineRow>(connection)?,
            None => sql_query(super::queries::first_machine_page())
                .bind::<Text, _>(snapshot.as_str())
                .bind::<Text, _>(snapshot.as_str())
                .bind::<BigInt, _>(bound)
                .load::<MachineRow>(connection)?,
        };
        let has_more = rows.len() > limit.0;
        let mut machines = rows
            .into_iter()
            .take(limit.0)
            .map(parse_machine_row)
            .collect::<QueryResult<Vec<_>>>()?;
        load_machine_children(connection, &mut machines)?;
        let next_cursor = if has_more {
            machines.last().map(|machine| MachineCursor {
                generation,
                snapshot: snapshot.clone(),
                order: machine.list_order,
                owner_id: machine.id,
            })
        } else {
            None
        };
        Ok(MachinePage {
            snapshot: provenance,
            machines,
            next_cursor,
        })
    })
}

fn snapshot_row(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    generation: CatalogRegistryId,
) -> QueryResult<MachineSnapshot> {
    let row = sql_query(super::queries::PUBLISHED_SNAPSHOT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotRow>(connection)
        .map_err(|error| match error {
            diesel::result::Error::NotFound => {
                MachineQueryError::NotPublishedMame(snapshot.clone())
            }
            other => MachineQueryError::Database(other),
        })?;
    if row.snapshot_key != snapshot.as_str() {
        return Err(MachineQueryError::NotPublishedMame(snapshot.clone()));
    }
    let registry_bytes: [u8; 16] = row
        .registry_uuid
        .try_into()
        .map_err(|bytes: Vec<u8>| invalid("registry UUID length", 0, bytes.len().to_string()))?;
    let row_generation = CatalogRegistryId::from_bytes(registry_bytes);
    if row_generation != generation {
        return Err(MachineQueryError::CursorRegistryMismatch);
    }
    let mut header = MameDocumentFacts {
        attribute_positions: Vec::new(),
        build: row.build,
        debug: stored_bool(row.debug, "debug", 0)?,
        debug_specified: stored_bool(row.debug_specified, "debug_specified", 0)?,
        config_version: row.config_version,
        location: RecordLocation {
            line: row.header_line,
            column: row.header_column,
        },
    };
    super::attributes::document(connection, row.document_id, &mut header)?;
    Ok(MachineSnapshot {
        registry_id: generation,
        snapshot_key: SnapshotKey::from_persisted(row.snapshot_key),
        source_key: crate::domain::PublishingSourceKey::new(row.source_key),
        source_name: row.source_name,
        catalog_key: crate::domain::CatalogKey::new(row.catalog_key),
        catalog_name: row.catalog_name,
        document_key: row.document_key.parse()?,
        interpretation_key: crate::domain::ParserInterpretationKey::from_persisted(
            row.interpretation_key,
        ),
        format: row.format,
        header,
    })
}

fn parse_machine_row(row: MachineRow) -> QueryResult<Machine> {
    let owner = row.id;
    if row.group_snapshot_ok != Some(1)
        || row.group_kind.as_deref() != Some("root")
        || row.source_element_kind != "mame_machine"
    {
        return Err(MachineQueryError::MismatchedOwner(owner));
    }
    require_native_owner(row.native_id, owner, "machine")?;
    let (description_source_order, description_location, year_location, manufacturer_location) =
        machine_text_facts(&row, owner)?;
    let flags = machine_flags(&row, owner)?;
    let description = required(row.description, "machine description", owner)?;
    Ok(Machine {
        id: CatalogSetId::try_from(owner)?,
        name: row.name,
        list_order: nonnegative(row.list_order, "machine list order", owner)?,
        location: RecordLocation {
            line: row.line,
            column: row.column,
        },
        facts: crate::mame::MachineFacts {
            attribute_positions: Vec::new(),
            compatibility_attribute_positions: Vec::new(),
            source_file: row.source_file,
            description,
            description_location,
            description_source_order: nonnegative(
                description_source_order,
                "description source order",
                owner,
            )?,
            year: row.year,
            year_location,
            year_source_order: row.year_source_order,
            manufacturer: row.manufacturer,
            manufacturer_location,
            manufacturer_source_order: row.manufacturer_source_order,
            flags,
            attributes_location: RecordLocation {
                line: required(row.attributes_line, "machine attributes line", owner)?,
                column: required(row.attributes_column, "machine attributes column", owner)?,
            },
        },
        dependencies: Vec::new(),
        assets: Vec::new(),
        bios_sets: Vec::new(),
        switches: Vec::new(),
        specification: Vec::new(),
    })
}

fn machine_text_facts(
    row: &MachineRow,
    owner: i64,
) -> QueryResult<(
    i64,
    RecordLocation,
    Option<RecordLocation>,
    Option<RecordLocation>,
)> {
    let description_source_order = required(
        row.description_source_order,
        "description source order",
        owner,
    )?;
    let description_location = RecordLocation {
        line: required(row.description_line, "description line", owner)?,
        column: required(row.description_column, "description column", owner)?,
    };
    let year_location = optional_location(
        row.year.as_ref(),
        row.year_line,
        row.year_column,
        "year",
        owner,
    )?;
    let manufacturer_location = optional_location(
        row.manufacturer.as_ref(),
        row.manufacturer_line,
        row.manufacturer_column,
        "manufacturer",
        owner,
    )?;
    validate_optional_source_order(
        row.year.as_ref(),
        row.year_source_order,
        "year source order",
        owner,
    )?;
    validate_optional_source_order(
        row.manufacturer.as_ref(),
        row.manufacturer_source_order,
        "manufacturer source order",
        owner,
    )?;
    Ok((
        description_source_order,
        description_location,
        year_location,
        manufacturer_location,
    ))
}

fn machine_flags(row: &MachineRow, owner: i64) -> QueryResult<MachineFlags> {
    Ok(MachineFlags::from_stored([
        stored_machine_flag(
            row.is_device,
            row.is_device_specified,
            "is_device",
            "is_device_specified",
            owner,
        )?,
        stored_machine_flag(
            row.runnable,
            row.runnable_specified,
            "runnable",
            "runnable_specified",
            owner,
        )?,
        stored_machine_flag(
            row.is_bios,
            row.is_bios_specified,
            "is_bios",
            "is_bios_specified",
            owner,
        )?,
        stored_machine_flag(
            row.is_mechanical,
            row.is_mechanical_specified,
            "is_mechanical",
            "is_mechanical_specified",
            owner,
        )?,
        stored_machine_flag(
            row.is_consumable,
            row.is_consumable_specified,
            "is_consumable",
            "is_consumable_specified",
            owner,
        )?,
    ])?)
}

fn stored_machine_flag(
    value: Option<i64>,
    specified: Option<i64>,
    field: &'static str,
    specified_field: &'static str,
    owner: i64,
) -> QueryResult<(bool, bool)> {
    let value = stored_bool(required(value, field, owner)?, field, owner)?;
    let specified = stored_bool(
        required(specified, specified_field, owner)?,
        specified_field,
        owner,
    )?;
    Ok((value, specified))
}

fn load_machine_children(
    connection: &mut SqliteConnection,
    machines: &mut [Machine],
) -> QueryResult<()> {
    if machines.is_empty() {
        return Ok(());
    }
    let owner_indexes = machines
        .iter()
        .enumerate()
        .map(|(index, machine)| (machine.id.as_i64(), index))
        .collect::<BTreeMap<_, _>>();
    connection.batch_execute(super::queries::CREATE_REQUESTED_OWNERS)?;
    for machine in machines.iter() {
        sql_query(super::queries::INSERT_REQUESTED_OWNER)
            .bind::<BigInt, _>(machine.id.as_i64())
            .execute(connection)?;
    }

    let mut expected_positions = BTreeSet::new();
    for machine in machines.iter() {
        let owner = machine.id.as_i64();
        expected_positions.insert((
            owner,
            machine.facts.description_source_order,
            "description",
            0,
        ));
        if let Some(order) = machine.facts.year_source_order {
            expected_positions.insert((owner, order, "year", 0));
        }
        if let Some(order) = machine.facts.manufacturer_source_order {
            expected_positions.insert((owner, order, "manufacturer", 0));
        }
    }
    load_bios_sets(
        connection,
        &owner_indexes,
        machines,
        &mut expected_positions,
    )?;
    load_dependencies(
        connection,
        &owner_indexes,
        machines,
        &mut expected_positions,
    )?;
    load_assets(
        connection,
        &owner_indexes,
        machines,
        &mut expected_positions,
    )?;
    load_switches(
        connection,
        &owner_indexes,
        machines,
        &mut expected_positions,
    )?;
    load_specification(
        connection,
        &owner_indexes,
        machines,
        &mut expected_positions,
    )?;
    validate_child_positions(connection, &owner_indexes, expected_positions)?;
    super::attributes::machines(connection, machines)?;
    connection.batch_execute(super::queries::DROP_REQUESTED_OWNERS)?;
    Ok(())
}

fn load_bios_sets(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    machines: &mut [Machine],
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    for row in sql_query(super::queries::BIOS_SETS).load::<BiosRow>(connection)? {
        let machine = machine_for_owner(machines, owners, row.owner_id)?;
        let order = nonnegative(row.row_order, "BIOS set order", row.owner_id)?;
        if usize::try_from(order).ok() != Some(machine.bios_sets.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let source_order = nonnegative(row.source_order, "BIOS source order", row.owner_id)?;
        positions.insert((row.owner_id, source_order, "biosset", order));
        machine.bios_sets.push(crate::mame::MachineBiosSet {
            attribute_positions: Vec::new(),
            name: row.name,
            description: row.description,
            is_default: stored_bool(row.is_default, "BIOS default", row.owner_id)?,
            default_specified: stored_bool(
                row.default_specified,
                "BIOS default presence",
                row.owner_id,
            )?,
            source_order,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }
    Ok(())
}

fn load_dependencies(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    machines: &mut [Machine],
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    for row in sql_query(super::queries::CLONE_LINKS).load::<CloneLinkRow>(connection)? {
        let machine = machine_for_owner(machines, owners, row.owner_id)?;
        machine.dependencies.push(MachineDependency::CloneOf {
            target_name: row.target_name,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }
    for row in sql_query(super::queries::DEPENDENCIES).load::<DependencyRow>(connection)? {
        let machine = machine_for_owner(machines, owners, row.owner_id)?;
        let order = nonnegative(row.row_order, "dependency order", row.owner_id)?;
        let location = RecordLocation {
            line: row.line,
            column: row.column,
        };
        let dependency = match row.dependency_kind.as_str() {
            "romof" if row.source_order.is_none() && row.reference_tag.is_none() => {
                MachineDependency::RomOf {
                    target_name: row.target_name,
                    location,
                }
            }
            "sampleof" if row.source_order.is_none() && row.reference_tag.is_none() => {
                MachineDependency::SampleOf {
                    target_name: row.target_name,
                    location,
                }
            }
            "device_ref" if row.source_order.is_some() && row.reference_tag.is_some() => {
                let source_order = nonnegative(
                    required(
                        row.source_order,
                        "device reference source order",
                        row.owner_id,
                    )?,
                    "device reference source order",
                    row.owner_id,
                )?;
                positions.insert((row.owner_id, source_order, "device_ref", order));
                MachineDependency::DeviceReference(crate::mame::DeviceReference {
                    attribute_positions: Vec::new(),
                    name: row.target_name,
                    tag: required(row.reference_tag, "device reference tag", row.owner_id)?,
                    source_order,
                    location,
                })
            }
            _ => {
                return Err(invalid(
                    "machine dependency kind/fields",
                    row.owner_id,
                    row.dependency_kind.clone(),
                ));
            }
        };
        machine.dependencies.push(dependency);
    }
    Ok(())
}

fn load_assets(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    machines: &mut [Machine],
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    let mut found = BTreeSet::new();
    for (query, expected_kind, expected_claim, child_kind) in [
        (
            super::queries::ROM_ASSETS,
            MachineAssetKind::Rom,
            "mame_rom",
            "rom",
        ),
        (
            super::queries::DISK_ASSETS,
            MachineAssetKind::Disk,
            "mame_disk",
            "disk",
        ),
        (
            super::queries::SAMPLE_ASSETS,
            MachineAssetKind::Sample,
            "mame_sample",
            "sample",
        ),
    ] {
        for row in sql_query(query).load::<AssetRow>(connection)? {
            let machine = machine_for_owner(machines, owners, row.owner_id)?;
            let occurrence_id = OccurrenceId::from_database(row.occurrence_id);
            if row.claim_kind != expected_claim {
                return Err(MachineQueryError::MismatchedOwner(row.occurrence_id));
            }
            let source_order = required(row.source_order, "asset source order", row.occurrence_id)?;
            let line = required(row.line, "asset source line", row.occurrence_id)?;
            let column = required(row.column, "asset source column", row.occurrence_id)?;
            let child_order = nonnegative(row.child_order, "asset occurrence order", row.owner_id)?;
            if !found.insert(row.occurrence_id) {
                return Err(MachineQueryError::MismatchedOwner(row.occurrence_id));
            }
            let position_order = if expected_kind == MachineAssetKind::Sample {
                source_order
            } else {
                child_order
            };
            positions.insert((row.owner_id, source_order, child_kind, position_order));
            machine.assets.push(MachineAssetReference {
                occurrence_id,
                kind: expected_kind,
                source_order: nonnegative(source_order, "asset source order", row.owner_id)?,
                location: RecordLocation { line, column },
            });
        }
    }
    for machine in machines {
        machine.assets.sort_by_key(|asset| asset.source_order);
    }
    Ok(())
}

fn load_switches(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    machines: &mut [Machine],
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    let mut switches_by_owner_order = BTreeMap::<(i64, i64), (usize, usize)>::new();
    for row in sql_query(super::queries::SWITCHES).load::<SwitchRow>(connection)? {
        let machine_index = *owners
            .get(&row.owner_id)
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let machine = machines
            .get_mut(machine_index)
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let row_order = nonnegative(row.row_order, "switch order", row.owner_id)?;
        if usize::try_from(row_order).ok() != Some(machine.switches.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let (kind, child_kind) = match row.kind.as_str() {
            "dipswitch" => (MachineSwitchKind::DipSwitch, "dipswitch"),
            "configuration" => (MachineSwitchKind::Configuration, "configuration"),
            other => return Err(invalid("switch kind", row.owner_id, other)),
        };
        let source_order = nonnegative(row.source_order, "switch source order", row.owner_id)?;
        positions.insert((row.owner_id, source_order, child_kind, row_order));
        let index = machine.switches.len();
        machine.switches.push(MachineSwitch {
            attribute_positions: Vec::new(),
            kind,
            name: row.name,
            tag: row.tag,
            mask: row.mask,
            source_order,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
            condition: None,
            locations: Vec::new(),
            values: Vec::new(),
        });
        switches_by_owner_order.insert((row.owner_id, row_order), (machine_index, index));
    }
    let mut conditions = BTreeMap::new();
    for row in sql_query(super::queries::SWITCH_CONDITIONS).load::<ConditionRow>(connection)? {
        if row.child_order != 0 || row.condition_order != 0 {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let key = (row.owner_id, row.switch_order);
        let condition = parse_condition(row, "switch condition")?;
        if conditions.insert(key, condition).is_some() {
            return Err(MachineQueryError::InvalidPositions(key.0));
        }
    }
    for (key, condition) in conditions {
        let (machine_index, switch_index) = switches_by_owner_order
            .get(&key)
            .copied()
            .ok_or(MachineQueryError::MismatchedOwner(key.0))?;
        let switch = machines
            .get_mut(machine_index)
            .and_then(|machine| machine.switches.get_mut(switch_index))
            .ok_or(MachineQueryError::MismatchedOwner(key.0))?;
        switch.condition = Some(condition);
    }
    load_switch_locations(connection, machines, &switches_by_owner_order)?;
    load_switch_values(connection, machines, &switches_by_owner_order)?;
    Ok(())
}

fn load_switch_locations(
    connection: &mut SqliteConnection,
    machines: &mut [Machine],
    switch_index: &BTreeMap<(i64, i64), (usize, usize)>,
) -> QueryResult<()> {
    for row in sql_query(super::queries::SWITCH_LOCATIONS).load::<SwitchLocationRow>(connection)? {
        let key = (row.owner_id, row.switch_order);
        let (machine_index, switch_index) = switch_index
            .get(&key)
            .copied()
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let switch = machines
            .get_mut(machine_index)
            .and_then(|machine| machine.switches.get_mut(switch_index))
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        if usize::try_from(nonnegative(
            row.row_order,
            "switch location order",
            row.owner_id,
        )?)
        .ok()
            != Some(switch.locations.len())
        {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let inverted = stored_bool(row.inverted, "switch location inverted", row.owner_id)?;
        let inverted_specified = stored_bool(
            row.inverted_specified,
            "switch location inverted presence",
            row.owner_id,
        )?;
        if !inverted_specified && inverted {
            return Err(invalid(
                "unspecified switch location default",
                row.owner_id,
                "yes",
            ));
        }
        let kind = match switch.kind {
            MachineSwitchKind::DipSwitch => MachineSwitchLocationKind::DipLocation,
            MachineSwitchKind::Configuration => MachineSwitchLocationKind::ConfigurationLocation,
        };
        switch.locations.push(MachineSwitchLocation {
            attribute_positions: Vec::new(),
            kind,
            name: row.name,
            number: row.number,
            inverted,
            inverted_specified,
            source_order: nonnegative(
                row.source_order,
                "switch location source order",
                row.owner_id,
            )?,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }
    Ok(())
}

fn load_switch_values(
    connection: &mut SqliteConnection,
    machines: &mut [Machine],
    switch_index: &BTreeMap<(i64, i64), (usize, usize)>,
) -> QueryResult<()> {
    let mut value_index = BTreeMap::<(i64, i64, i64), (usize, usize, usize)>::new();
    for row in sql_query(super::queries::SWITCH_VALUES).load::<SwitchValueRow>(connection)? {
        let key = (row.owner_id, row.switch_order);
        let (machine_index, switch_position) = switch_index
            .get(&key)
            .copied()
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let machine = machines
            .get_mut(machine_index)
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let switch = machine
            .switches
            .get_mut(switch_position)
            .ok_or(MachineQueryError::MismatchedOwner(row.owner_id))?;
        let kind = match switch.kind {
            MachineSwitchKind::DipSwitch => MachineSwitchValueKind::DipValue,
            MachineSwitchKind::Configuration => MachineSwitchValueKind::ConfigurationSetting,
        };
        if usize::try_from(nonnegative(
            row.row_order,
            "switch value order",
            row.owner_id,
        )?)
        .ok()
            != Some(switch.values.len())
        {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let is_default = stored_bool(row.is_default, "switch value default", row.owner_id)?;
        let default_specified = stored_bool(
            row.default_specified,
            "switch value default presence",
            row.owner_id,
        )?;
        if !default_specified && is_default {
            return Err(invalid(
                "unspecified switch value default",
                row.owner_id,
                "yes",
            ));
        }
        let value_position = switch.values.len();
        switch.values.push(MachineSwitchValue {
            attribute_positions: Vec::new(),
            kind,
            name: row.name,
            value: row.value,
            default: is_default,
            default_specified,
            condition: None,
            source_order: nonnegative(row.source_order, "switch value source order", row.owner_id)?,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
        value_index.insert(
            (row.owner_id, row.switch_order, row.row_order),
            (machine_index, switch_position, value_position),
        );
    }
    let mut conditions = BTreeMap::new();
    for row in
        sql_query(super::queries::SWITCH_VALUE_CONDITIONS).load::<ConditionRow>(connection)?
    {
        if row.condition_order != 0 {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let key = (row.owner_id, row.switch_order, row.child_order);
        let condition = parse_condition(row, "switch value condition")?;
        if conditions.insert(key, condition).is_some() {
            return Err(MachineQueryError::InvalidPositions(key.0));
        }
    }
    for (key, condition) in conditions {
        let (machine_index, switch_index, value_index) = value_index
            .get(&key)
            .copied()
            .ok_or(MachineQueryError::MismatchedOwner(key.0))?;
        let value = machines
            .get_mut(machine_index)
            .and_then(|machine| machine.switches.get_mut(switch_index))
            .and_then(|switch| switch.values.get_mut(value_index))
            .ok_or(MachineQueryError::MismatchedOwner(key.0))?;
        value.condition = Some(condition);
    }
    Ok(())
}

// Keep the complete, exhaustive native-family mapping together for schema review.
#[allow(clippy::too_many_lines)]
fn load_specification(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    machines: &mut [Machine],
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    use crate::mame::{
        Adjuster, Analog, Chip, ChipKind, Device, DeviceExtension, DeviceInstance, Display,
        DisplayKind, DisplayRotation, Driver, Feature, Input, InputControl, MachineSpecification,
        Port, RamOption, Sample, SaveState, Slot, SlotOption, SoftwareList, SoftwareListStatus,
        Sound,
    };

    let mut controls = BTreeMap::<(i64, i64), Vec<InputControl>>::new();
    for row in sql_query(super::queries::SPEC_CONTROLS).load::<ControlRow>(connection)? {
        let order = nonnegative(row.row_order, "input control order", row.owner_id)?;
        let values = controls
            .entry((row.owner_id, row.element_order))
            .or_default();
        if usize::try_from(order).ok() != Some(values.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let reverse = stored_bool(row.reverse, "input control reverse", row.owner_id)?;
        let reverse_specified = stored_bool(
            row.reverse_specified,
            "input control reverse presence",
            row.owner_id,
        )?;
        if !reverse_specified && reverse {
            return Err(invalid(
                "unspecified input control reverse",
                row.owner_id,
                "yes",
            ));
        }
        values.push(InputControl {
            attribute_positions: Vec::new(),
            kind: row.control_type,
            player: row.player,
            buttons: row.buttons,
            minimum: row.minimum,
            maximum: row.maximum,
            sensitivity: row.sensitivity,
            key_delta: row.keydelta,
            reverse: mame_boolean(reverse),
            reverse_specified,
            ways: row.ways,
            ways2: row.ways2,
            ways3: row.ways3,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }

    let mut analogs = BTreeMap::<(i64, i64), Vec<Analog>>::new();
    for row in sql_query(super::queries::SPEC_ANALOGS).load::<AnalogRow>(connection)? {
        let order = nonnegative(row.row_order, "analog order", row.owner_id)?;
        let values = analogs
            .entry((row.owner_id, row.element_order))
            .or_default();
        if usize::try_from(order).ok() != Some(values.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        values.push(Analog {
            attribute_positions: Vec::new(),
            mask: row.mask,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }

    let mut adjuster_conditions = BTreeMap::new();
    for row in
        sql_query(super::queries::SPEC_ADJUSTER_CONDITIONS).load::<ConditionRow>(connection)?
    {
        let owner = row.owner_id;
        if row.child_order != 0 || row.condition_order != 0 {
            return Err(MachineQueryError::InvalidPositions(owner));
        }
        let key = (owner, row.switch_order);
        let condition = parse_condition(row, "adjuster condition")?;
        if adjuster_conditions.insert(key, condition).is_some() {
            return Err(MachineQueryError::InvalidPositions(owner));
        }
    }

    let mut device_instances = BTreeMap::new();
    for row in
        sql_query(super::queries::SPEC_DEVICE_INSTANCES).load::<DeviceInstanceRow>(connection)?
    {
        let value = DeviceInstance {
            attribute_positions: Vec::new(),
            name: row.name,
            brief_name: row.brief_name,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        };
        if device_instances
            .insert((row.owner_id, row.element_order), value)
            .is_some()
        {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
    }
    let mut device_extensions = BTreeMap::<(i64, i64), Vec<DeviceExtension>>::new();
    for row in
        sql_query(super::queries::SPEC_DEVICE_EXTENSIONS).load::<DeviceExtensionRow>(connection)?
    {
        let order = nonnegative(row.row_order, "device extension order", row.owner_id)?;
        let values = device_extensions
            .entry((row.owner_id, row.element_order))
            .or_default();
        if usize::try_from(order).ok() != Some(values.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        values.push(DeviceExtension {
            attribute_positions: Vec::new(),
            name: row.name,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }

    let mut slot_options = BTreeMap::<(i64, i64), Vec<SlotOption>>::new();
    for row in sql_query(super::queries::SPEC_SLOT_OPTIONS).load::<SlotOptionRow>(connection)? {
        let order = nonnegative(row.row_order, "slot option order", row.owner_id)?;
        let values = slot_options
            .entry((row.owner_id, row.element_order))
            .or_default();
        if usize::try_from(order).ok() != Some(values.len()) {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
        let is_default = stored_bool(row.is_default, "slot option default", row.owner_id)?;
        let default_specified = stored_bool(
            row.default_specified,
            "slot option default presence",
            row.owner_id,
        )?;
        if !default_specified && is_default {
            return Err(invalid(
                "unspecified slot option default",
                row.owner_id,
                "yes",
            ));
        }
        values.push(SlotOption {
            attribute_positions: Vec::new(),
            name: row.name,
            device_name: row.devname,
            is_default: mame_boolean(is_default),
            default_specified,
            location: RecordLocation {
                line: row.line,
                column: row.column,
            },
        });
    }

    for row in sql_query(super::queries::SPEC_SAMPLES).load::<SampleRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "sample",
            MachineSpecification::Sample(Sample {
                attribute_positions: Vec::new(),
                name: row.name,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_CHIPS).load::<ChipRow>(connection)? {
        let kind = match row.kind.as_str() {
            "cpu" => ChipKind::Cpu,
            "audio" => ChipKind::Audio,
            other => return Err(invalid("chip type", row.owner_id, other)),
        };
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "chip",
            MachineSpecification::Chip(Chip {
                attribute_positions: Vec::new(),
                name: row.name,
                tag: row.tag,
                kind,
                clock: row.clock,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_DISPLAYS).load::<DisplayRow>(connection)? {
        let kind = match row.kind.as_str() {
            "raster" => DisplayKind::Raster,
            "vector" => DisplayKind::Vector,
            "lcd" => DisplayKind::Lcd,
            "svg" => DisplayKind::Svg,
            "unknown" => DisplayKind::Unknown,
            other => return Err(invalid("display type", row.owner_id, other)),
        };
        let rotation = row
            .rotation
            .map(|rotation| match rotation.as_str() {
                "0" => Ok(DisplayRotation::Deg0),
                "90" => Ok(DisplayRotation::Deg90),
                "180" => Ok(DisplayRotation::Deg180),
                "270" => Ok(DisplayRotation::Deg270),
                other => Err(invalid("display rotation", row.owner_id, other)),
            })
            .transpose()?;
        let flip_x = stored_bool(row.flip_x, "display flipx", row.owner_id)?;
        let flip_x_specified =
            stored_bool(row.flip_x_specified, "display flipx presence", row.owner_id)?;
        if !flip_x_specified && flip_x {
            return Err(invalid("unspecified display flipx", row.owner_id, "yes"));
        }
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "display",
            MachineSpecification::Display(Display {
                attribute_positions: Vec::new(),
                tag: row.tag,
                kind,
                rotation,
                flip_x: mame_boolean(flip_x),
                flip_x_specified,
                width: row.width,
                height: row.height,
                refresh: row.refresh,
                pixel_clock: row.pixel_clock,
                horizontal_total: row.horizontal_total,
                horizontal_blank_end: row.horizontal_blank_end,
                horizontal_blank_start: row.horizontal_blank_start,
                vertical_total: row.vertical_total,
                vertical_blank_end: row.vertical_blank_end,
                vertical_blank_start: row.vertical_blank_start,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_SOUNDS).load::<SoundRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "sound",
            MachineSpecification::Sound(Sound {
                attribute_positions: Vec::new(),
                channels: row.channels,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_INPUTS).load::<InputRow>(connection)? {
        let key = (row.owner_id, row.element_order);
        let service = stored_bool(row.service, "input service", row.owner_id)?;
        let service_specified = stored_bool(
            row.service_specified,
            "input service presence",
            row.owner_id,
        )?;
        let tilt = stored_bool(row.tilt, "input tilt", row.owner_id)?;
        let tilt_specified = stored_bool(row.tilt_specified, "input tilt presence", row.owner_id)?;
        if (!service_specified && service) || (!tilt_specified && tilt) {
            return Err(invalid("unspecified input default", row.owner_id, "yes"));
        }
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "input",
            MachineSpecification::Input(Input {
                attribute_positions: Vec::new(),
                service: mame_boolean(service),
                service_specified,
                tilt: mame_boolean(tilt),
                tilt_specified,
                players: row.players,
                coins: row.coins,
                controls: controls.remove(&key).unwrap_or_default(),
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_PORTS).load::<PortRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "port",
            MachineSpecification::Port(Port {
                attribute_positions: Vec::new(),
                tag: row.tag,
                analogs: analogs
                    .remove(&(row.owner_id, row.element_order))
                    .unwrap_or_default(),
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_ADJUSTERS).load::<AdjusterRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "adjuster",
            MachineSpecification::Adjuster(Adjuster {
                attribute_positions: Vec::new(),
                name: row.name,
                default: row.default_value,
                condition: adjuster_conditions.remove(&(row.owner_id, row.element_order)),
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_DRIVERS).load::<DriverRow>(connection)? {
        let status = parse_driver_quality(&row.status, row.owner_id, "driver status")?;
        let emulation = parse_driver_quality(&row.emulation, row.owner_id, "driver emulation")?;
        let cocktail = row
            .cocktail
            .as_deref()
            .map(|value| parse_driver_quality(value, row.owner_id, "driver cocktail"))
            .transpose()?;
        let savestate = match row.savestate.as_str() {
            "supported" => SaveState::Supported,
            "unsupported" => SaveState::Unsupported,
            other => return Err(invalid("driver savestate", row.owner_id, other)),
        };
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "driver",
            MachineSpecification::Driver(Driver {
                attribute_positions: Vec::new(),
                status,
                emulation,
                cocktail,
                savestate,
                requires_artwork: mame_boolean(stored_bool(
                    row.requires_artwork,
                    "driver requiresartwork",
                    row.owner_id,
                )?),
                requires_artwork_specified: stored_bool(
                    row.requires_artwork_specified,
                    "driver requiresartwork presence",
                    row.owner_id,
                )?,
                unofficial: mame_boolean(stored_bool(
                    row.unofficial,
                    "driver unofficial",
                    row.owner_id,
                )?),
                unofficial_specified: stored_bool(
                    row.unofficial_specified,
                    "driver unofficial presence",
                    row.owner_id,
                )?,
                no_sound_hardware: mame_boolean(stored_bool(
                    row.no_sound_hardware,
                    "driver nosoundhardware",
                    row.owner_id,
                )?),
                no_sound_hardware_specified: stored_bool(
                    row.no_sound_hardware_specified,
                    "driver nosoundhardware presence",
                    row.owner_id,
                )?,
                incomplete: mame_boolean(stored_bool(
                    row.incomplete,
                    "driver incomplete",
                    row.owner_id,
                )?),
                incomplete_specified: stored_bool(
                    row.incomplete_specified,
                    "driver incomplete presence",
                    row.owner_id,
                )?,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_FEATURES).load::<FeatureRow>(connection)? {
        let kind = parse_feature_kind(&row.kind, row.owner_id)?;
        let status = row
            .status
            .as_deref()
            .map(|value| parse_feature_status(value, row.owner_id))
            .transpose()?;
        let overall = row
            .overall
            .as_deref()
            .map(|value| parse_feature_status(value, row.owner_id))
            .transpose()?;
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "feature",
            MachineSpecification::Feature(Feature {
                attribute_positions: Vec::new(),
                kind,
                status,
                overall,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_DEVICES).load::<DeviceRow>(connection)? {
        let key = (row.owner_id, row.element_order);
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "device",
            MachineSpecification::Device(Device {
                attribute_positions: Vec::new(),
                kind: row.kind,
                tag: row.tag,
                fixed_image: row.fixed_image,
                mandatory: row.mandatory,
                interface: row.interface,
                instance: device_instances.remove(&key),
                extensions: device_extensions.remove(&key).unwrap_or_default(),
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_SLOTS).load::<SlotRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "slot",
            MachineSpecification::Slot(Slot {
                attribute_positions: Vec::new(),
                name: row.name,
                options: slot_options
                    .remove(&(row.owner_id, row.element_order))
                    .unwrap_or_default(),
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_SOFTWARE_LISTS).load::<SoftwareListRow>(connection)? {
        let status = match row.status.as_str() {
            "original" => SoftwareListStatus::Original,
            "compatible" => SoftwareListStatus::Compatible,
            other => return Err(invalid("software list status", row.owner_id, other)),
        };
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "softwarelist",
            MachineSpecification::SoftwareList(SoftwareList {
                attribute_positions: Vec::new(),
                tag: row.tag,
                name: row.name,
                status,
                filter: row.filter,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }
    for row in sql_query(super::queries::SPEC_RAM_OPTIONS).load::<RamOptionRow>(connection)? {
        append_spec(
            machines,
            owners,
            positions,
            row.owner_id,
            row.element_order,
            "ramoption",
            MachineSpecification::RamOption(RamOption {
                attribute_positions: Vec::new(),
                name: row.name,
                default: row.default_value,
                text: row.text,
                location: RecordLocation {
                    line: row.line,
                    column: row.column,
                },
            }),
        )?;
    }

    for (name, len) in [
        ("input controls", controls.len()),
        ("port analogs", analogs.len()),
        ("adjuster conditions", adjuster_conditions.len()),
        ("device instances", device_instances.len()),
        ("device extensions", device_extensions.len()),
        ("slot options", slot_options.len()),
    ] {
        if len != 0 {
            return Err(MachineQueryError::InvalidStoredValue {
                field: "orphaned nested specification child",
                owner: 0,
                value: name.to_owned(),
            });
        }
    }
    for machine in machines {
        machine
            .specification
            .sort_by_key(|element| element.element_order);
    }
    Ok(())
}

fn append_spec(
    machines: &mut [Machine],
    owners: &BTreeMap<i64, usize>,
    positions: &mut BTreeSet<(i64, i64, &'static str, i64)>,
    owner: i64,
    element_order: i64,
    child_kind: &'static str,
    value: crate::mame::MachineSpecification,
) -> QueryResult<()> {
    let element_order = nonnegative(element_order, "specification source order", owner)?;
    let machine = machine_for_owner(machines, owners, owner)?;
    if machine
        .specification
        .iter()
        .any(|element| element.element_order == element_order)
    {
        return Err(MachineQueryError::InvalidPositions(owner));
    }
    positions.insert((owner, element_order, child_kind, element_order));
    machine
        .specification
        .push(crate::mame::MachineSpecificationElement {
            element_order,
            value,
        });
    Ok(())
}

const fn mame_boolean(value: bool) -> MameBoolean {
    if value {
        MameBoolean::Yes
    } else {
        MameBoolean::No
    }
}

fn parse_driver_quality(
    value: &str,
    owner: i64,
    field: &'static str,
) -> QueryResult<crate::mame::DriverQuality> {
    match value {
        "good" => Ok(crate::mame::DriverQuality::Good),
        "imperfect" => Ok(crate::mame::DriverQuality::Imperfect),
        "preliminary" => Ok(crate::mame::DriverQuality::Preliminary),
        other => Err(invalid(field, owner, other)),
    }
}

fn parse_feature_status(value: &str, owner: i64) -> QueryResult<crate::mame::FeatureStatus> {
    match value {
        "unemulated" => Ok(crate::mame::FeatureStatus::Unemulated),
        "imperfect" => Ok(crate::mame::FeatureStatus::Imperfect),
        other => Err(invalid("feature status", owner, other)),
    }
}

fn parse_feature_kind(value: &str, owner: i64) -> QueryResult<crate::mame::FeatureKind> {
    use crate::mame::FeatureKind as Kind;
    match value {
        "protection" => Ok(Kind::Protection),
        "timing" => Ok(Kind::Timing),
        "graphics" => Ok(Kind::Graphics),
        "palette" => Ok(Kind::Palette),
        "sound" => Ok(Kind::Sound),
        "capture" => Ok(Kind::Capture),
        "camera" => Ok(Kind::Camera),
        "microphone" => Ok(Kind::Microphone),
        "controls" => Ok(Kind::Controls),
        "keyboard" => Ok(Kind::Keyboard),
        "mouse" => Ok(Kind::Mouse),
        "media" => Ok(Kind::Media),
        "disk" => Ok(Kind::Disk),
        "printer" => Ok(Kind::Printer),
        "tape" => Ok(Kind::Tape),
        "punch" => Ok(Kind::Punch),
        "drum" => Ok(Kind::Drum),
        "rom" => Ok(Kind::Rom),
        "comms" => Ok(Kind::Comms),
        "lan" => Ok(Kind::Lan),
        "wan" => Ok(Kind::Wan),
        other => Err(invalid("feature type", owner, other)),
    }
}

fn parse_condition(
    row: ConditionRow,
    field: &'static str,
) -> QueryResult<crate::mame::MachineCondition> {
    let relation = match row.relation.as_str() {
        "eq" => crate::mame::ConditionRelation::Eq,
        "ne" => crate::mame::ConditionRelation::Ne,
        "gt" => crate::mame::ConditionRelation::Gt,
        "le" => crate::mame::ConditionRelation::Le,
        "lt" => crate::mame::ConditionRelation::Lt,
        "ge" => crate::mame::ConditionRelation::Ge,
        other => return Err(invalid(field, row.owner_id, other)),
    };
    Ok(crate::mame::MachineCondition {
        attribute_positions: Vec::new(),
        tag: row.tag,
        mask: row.mask,
        relation,
        value: row.value,
        location: RecordLocation {
            line: row.line,
            column: row.column,
        },
    })
}

fn validate_child_positions(
    connection: &mut SqliteConnection,
    owners: &BTreeMap<i64, usize>,
    expected: BTreeSet<(i64, i64, &'static str, i64)>,
) -> QueryResult<()> {
    let mut observed = BTreeSet::new();
    let mut source_orders = BTreeSet::new();
    for row in sql_query(super::queries::CHILD_POSITIONS).load::<PositionRow>(connection)? {
        if !owners.contains_key(&row.owner_id)
            || row.source_order < 0
            || row.child_order < 0
            || !source_orders.insert((row.owner_id, row.source_order))
            || !observed.insert((
                row.owner_id,
                row.source_order,
                row.child_kind,
                row.child_order,
            ))
        {
            return Err(MachineQueryError::InvalidPositions(row.owner_id));
        }
    }
    let expected = expected
        .into_iter()
        .map(|(owner, order, kind, child)| (owner, order, kind.to_owned(), child))
        .collect::<BTreeSet<_>>();
    if observed != expected {
        let owner = observed
            .symmetric_difference(&expected)
            .next()
            .map(|position| position.0)
            .unwrap_or_default();
        return Err(MachineQueryError::InvalidPositions(owner));
    }
    Ok(())
}

fn validate_cursor(
    cursor: Option<&MachineCursor>,
    generation: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.generation != generation {
            return Err(MachineQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot != snapshot {
            return Err(MachineQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn machine_for_owner<'a>(
    machines: &'a mut [Machine],
    owners: &BTreeMap<i64, usize>,
    owner: i64,
) -> QueryResult<&'a mut Machine> {
    let index = owners
        .get(&owner)
        .copied()
        .ok_or(MachineQueryError::MismatchedOwner(owner))?;
    machines
        .get_mut(index)
        .ok_or(MachineQueryError::MismatchedOwner(owner))
}

fn required<T>(value: Option<T>, field: &'static str, owner: i64) -> QueryResult<T> {
    value.ok_or(MachineQueryError::MissingNative { field, owner })
}

const fn require_native_owner(
    value: Option<i64>,
    owner: i64,
    field: &'static str,
) -> QueryResult<()> {
    match value {
        Some(native) if native == owner => Ok(()),
        Some(_) => Err(MachineQueryError::MismatchedOwner(owner)),
        None => Err(MachineQueryError::MissingNative { field, owner }),
    }
}

fn nonnegative(value: i64, field: &'static str, owner: i64) -> QueryResult<i64> {
    if value >= 0 {
        Ok(value)
    } else {
        Err(invalid(field, owner, value.to_string()))
    }
}

fn stored_bool(value: i64, field: &'static str, owner: i64) -> QueryResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(invalid(field, owner, other.to_string())),
    }
}

fn optional_location(
    present: Option<&String>,
    line: Option<i64>,
    column: Option<i64>,
    field: &'static str,
    owner: i64,
) -> QueryResult<Option<RecordLocation>> {
    match (present, line, column) {
        (None, None, None) => Ok(None),
        (Some(_), Some(line), Some(column)) => Ok(Some(RecordLocation { line, column })),
        _ => Err(invalid(field, owner, "presence/location mismatch")),
    }
}

fn validate_optional_source_order(
    present: Option<&String>,
    source_order: Option<i64>,
    field: &'static str,
    owner: i64,
) -> QueryResult<()> {
    match (present, source_order) {
        (None, None) => Ok(()),
        (Some(_), Some(order)) if order >= 0 => Ok(()),
        _ => Err(invalid(field, owner, "presence/order mismatch")),
    }
}

fn invalid(field: &'static str, owner: i64, value: impl Into<String>) -> MachineQueryError {
    MachineQueryError::InvalidStoredValue {
        field,
        owner,
        value: value.into(),
    }
}
