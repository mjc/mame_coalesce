//! Hydrate lexical witnesses only after the native machine owners are complete.

use std::collections::BTreeSet;

use diesel::SqliteConnection;

use crate::{
    mame::{self, AttributeLocation, AttributePosition},
    storage::mame_attributes::{Family, Positions},
};

use super::{Machine, MachineAssetKind, MachineDependency, MachineQueryError, MameDocumentFacts};

type QueryResult<T = ()> = Result<T, MachineQueryError>;

fn take<Field: Copy>(
    positions: &mut Positions,
    family: Family,
    owner: [i64; 4],
    decode: fn(i64) -> Option<Field>,
    expected: &[bool],
) -> QueryResult<Vec<AttributePosition<Field>>> {
    positions
        .take(family, owner, decode, expected)?
        .into_iter()
        .map(|position| {
            Ok(AttributePosition {
                field: position.field,
                source_order: usize::try_from(position.source_order)
                    .map_err(|_| MachineQueryError::InvalidPositions(owner[0]))?,
                location: AttributeLocation {
                    line: position.location.line,
                    column: position.location.column,
                },
            })
        })
        .collect()
}

fn order(value: usize) -> QueryResult<i64> {
    i64::try_from(value).map_err(|_| MachineQueryError::PageLimitOverflow)
}

pub(super) fn document(
    connection: &mut SqliteConnection,
    id: i64,
    header: &mut MameDocumentFacts,
) -> QueryResult {
    let mut positions = Positions::default();
    positions.load(
        connection,
        Family::Document,
        &format!("FROM __NATIVE_POSITIONS__ AS position WHERE position.document_id={id}"),
    )?;
    header.attribute_positions = take(
        &mut positions,
        Family::Document,
        [id, 0, 0, 0],
        mame::MameDocumentAttribute::from_code,
        &[header.build.is_some(), header.debug_specified, true],
    )?;
    positions.finish()?;
    Ok(())
}

pub(super) fn machines(connection: &mut SqliteConnection, machines: &mut [Machine]) -> QueryResult {
    let mut positions = Positions::default();
    for family in [
        Family::Machine,
        Family::MachineCompatibility,
        Family::Bios,
        Family::DeviceReference,
        Family::Switch,
        Family::SwitchLocation,
        Family::SwitchValue,
        Family::SwitchCondition,
        Family::SwitchValueCondition,
        Family::AdjusterCondition,
        Family::Chip,
        Family::Display,
        Family::Sound,
        Family::Input,
        Family::Control,
        Family::Port,
        Family::Analog,
        Family::Adjuster,
        Family::Driver,
        Family::Feature,
        Family::Device,
        Family::Instance,
        Family::Extension,
        Family::Slot,
        Family::SlotOption,
        Family::SoftwareList,
        Family::RamOption,
    ] {
        positions.load(connection, family,
            "FROM temp.catalog_machine_requested_owners AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.set_id=requested.owner_id")?;
    }
    positions.load(connection, Family::Sample,
        "FROM temp.catalog_machine_requested_owners AS requested CROSS JOIN asset_occurrences AS occurrence CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE occurrence.record_id=requested.owner_id AND occurrence.claim_kind='mame_sample' AND position.occurrence_id=occurrence.occurrence_id")?;

    for machine in machines {
        attach_machine(&mut positions, machine)?;
    }
    positions.finish()?;
    Ok(())
}

fn attach_machine(positions: &mut Positions, machine: &mut Machine) -> QueryResult {
    let id = machine.id.as_i64();
    let flags = machine.facts.flags;
    let has_clone = machine
        .dependencies
        .iter()
        .any(|d| matches!(d, MachineDependency::CloneOf { .. }));
    let has_rom = machine
        .dependencies
        .iter()
        .any(|d| matches!(d, MachineDependency::RomOf { .. }));
    let has_sample = machine
        .dependencies
        .iter()
        .any(|d| matches!(d, MachineDependency::SampleOf { .. }));
    machine.facts.attribute_positions = take(
        positions,
        Family::Machine,
        [id, 0, 0, 0],
        mame::MameMachineAttribute::from_code,
        &[
            true,
            machine.facts.source_file.is_some(),
            flags.is_bios_specified(),
            flags.is_device_specified(),
            flags.is_mechanical_specified(),
            flags.is_runnable_specified(),
            has_clone,
            has_rom,
            has_sample,
        ],
    )?;
    machine.facts.compatibility_attribute_positions = take(
        positions,
        Family::MachineCompatibility,
        [id, 0, 0, 0],
        mame::MameMachineCompatibilityAttribute::from_code,
        &[flags.is_consumable_specified()],
    )?;
    let mut ordinals = BTreeSet::new();
    for ordinal in machine
        .facts
        .attribute_positions
        .iter()
        .map(|p| p.source_order)
        .chain(
            machine
                .facts
                .compatibility_attribute_positions
                .iter()
                .map(|p| p.source_order),
        )
    {
        if !ordinals.insert(ordinal) {
            return Err(MachineQueryError::InvalidPositions(id));
        }
    }
    for (index, bios) in machine.bios_sets.iter_mut().enumerate() {
        bios.attribute_positions = take(
            positions,
            Family::Bios,
            [id, order(index)?, 0, 0],
            mame::MameBiosAttribute::from_code,
            &[true, true, bios.default_specified],
        )?;
    }
    let mut reference_order = 0;
    for dependency in &mut machine.dependencies {
        if let MachineDependency::DeviceReference(reference) = dependency {
            reference.attribute_positions = take(
                positions,
                Family::DeviceReference,
                [id, reference_order, 0, 0],
                mame::MameDeviceReferenceAttribute::from_code,
                &[true, true],
            )?;
            reference_order += 1;
        }
    }
    attach_switches(positions, id, &mut machine.switches)?;
    attach_specification(positions, machine)
}

fn attach_switches(
    positions: &mut Positions,
    id: i64,
    switches: &mut [super::MachineSwitch],
) -> QueryResult {
    for (index, switch) in switches.iter_mut().enumerate() {
        let switch_order = order(index)?;
        switch.attribute_positions = take(
            positions,
            Family::Switch,
            [id, switch_order, 0, 0],
            mame::MameSwitchAttribute::from_code,
            &[true, true, true],
        )?;
        condition(
            positions,
            Family::SwitchCondition,
            [id, switch_order, 0, 0],
            switch.condition.as_mut(),
        )?;
        for (index, location) in switch.locations.iter_mut().enumerate() {
            location.attribute_positions = take(
                positions,
                Family::SwitchLocation,
                [id, switch_order, order(index)?, 0],
                mame::MameSwitchLocationAttribute::from_code,
                &[true, true, location.inverted_specified],
            )?;
        }
        for (index, value) in switch.values.iter_mut().enumerate() {
            let value_order = order(index)?;
            value.attribute_positions = take(
                positions,
                Family::SwitchValue,
                [id, switch_order, value_order, 0],
                mame::MameSwitchValueAttribute::from_code,
                &[true, true, value.default_specified],
            )?;
            condition(
                positions,
                Family::SwitchValueCondition,
                [id, switch_order, value_order, 0],
                value.condition.as_mut(),
            )?;
        }
    }
    Ok(())
}

fn attach_specification(positions: &mut Positions, machine: &mut Machine) -> QueryResult {
    use mame::MachineSpecification as Spec;
    let id = machine.id.as_i64();
    for element in &mut machine.specification {
        let key = [id, element.element_order, 0, 0];
        match &mut element.value {
            Spec::Sample(value) => {
                let occurrence = machine
                    .assets
                    .iter()
                    .find(|asset| {
                        asset.kind == MachineAssetKind::Sample
                            && asset.source_order == element.element_order
                    })
                    .ok_or(MachineQueryError::InvalidPositions(id))?;
                value.attribute_positions = take(
                    positions,
                    Family::Sample,
                    [occurrence.occurrence_id.database_value(), 0, 0, 0],
                    mame::MameSampleAttribute::from_code,
                    &[true],
                )?;
            }
            Spec::Chip(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::Chip,
                    key,
                    mame::MameChipAttribute::from_code,
                    &[true, value.tag.is_some(), true, value.clock.is_some()],
                )?;
            }
            Spec::Display(value) => display(positions, key, value)?,
            Spec::Sound(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::Sound,
                    key,
                    mame::MameSoundAttribute::from_code,
                    &[true],
                )?;
            }
            Spec::Input(value) => input(positions, key, value)?,
            Spec::Port(value) => port(positions, key, value)?,
            Spec::Adjuster(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::Adjuster,
                    key,
                    mame::MameAdjusterAttribute::from_code,
                    &[true, true],
                )?;
                condition(
                    positions,
                    Family::AdjusterCondition,
                    key,
                    value.condition.as_mut(),
                )?;
            }
            Spec::Driver(value) => driver(positions, key, value)?,
            Spec::Feature(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::Feature,
                    key,
                    mame::MameFeatureAttribute::from_code,
                    &[true, value.status.is_some(), value.overall.is_some()],
                )?;
            }
            Spec::Device(value) => device(positions, key, value)?,
            Spec::Slot(value) => slot(positions, key, value)?,
            Spec::SoftwareList(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::SoftwareList,
                    key,
                    mame::MameSoftwareListAttribute::from_code,
                    &[true, true, true, value.filter.is_some()],
                )?;
            }
            Spec::RamOption(value) => {
                value.attribute_positions = take(
                    positions,
                    Family::RamOption,
                    key,
                    mame::MameRamOptionAttribute::from_code,
                    &[true, value.default.is_some()],
                )?;
            }
        }
    }
    Ok(())
}

fn display(positions: &mut Positions, key: [i64; 4], value: &mut mame::Display) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Display,
        key,
        mame::MameDisplayAttribute::from_code,
        &[
            value.tag.is_some(),
            true,
            value.rotation.is_some(),
            value.flip_x_specified,
            value.width.is_some(),
            value.height.is_some(),
            true,
            value.pixel_clock.is_some(),
            value.horizontal_total.is_some(),
            value.horizontal_blank_end.is_some(),
            value.horizontal_blank_start.is_some(),
            value.vertical_total.is_some(),
            value.vertical_blank_end.is_some(),
            value.vertical_blank_start.is_some(),
        ],
    )?;
    Ok(())
}

fn input(positions: &mut Positions, key: [i64; 4], value: &mut mame::Input) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Input,
        key,
        mame::MameInputAttribute::from_code,
        &[
            value.service_specified,
            value.tilt_specified,
            true,
            value.coins.is_some(),
        ],
    )?;
    for (index, control) in value.controls.iter_mut().enumerate() {
        control.attribute_positions = take(
            positions,
            Family::Control,
            [key[0], key[1], order(index)?, 0],
            mame::MameControlAttribute::from_code,
            &[
                true,
                control.player.is_some(),
                control.buttons.is_some(),
                control.minimum.is_some(),
                control.maximum.is_some(),
                control.sensitivity.is_some(),
                control.key_delta.is_some(),
                control.reverse_specified,
                control.ways.is_some(),
                control.ways2.is_some(),
                control.ways3.is_some(),
            ],
        )?;
    }
    Ok(())
}

fn port(positions: &mut Positions, key: [i64; 4], value: &mut mame::Port) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Port,
        key,
        mame::MamePortAttribute::from_code,
        &[true],
    )?;
    for (index, analog) in value.analogs.iter_mut().enumerate() {
        analog.attribute_positions = take(
            positions,
            Family::Analog,
            [key[0], key[1], order(index)?, 0],
            mame::MameAnalogAttribute::from_code,
            &[true],
        )?;
    }
    Ok(())
}

fn driver(positions: &mut Positions, key: [i64; 4], value: &mut mame::Driver) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Driver,
        key,
        mame::MameDriverAttribute::from_code,
        &[
            true,
            true,
            value.cocktail.is_some(),
            true,
            value.requires_artwork_specified,
            value.unofficial_specified,
            value.no_sound_hardware_specified,
            value.incomplete_specified,
        ],
    )?;
    Ok(())
}

fn device(positions: &mut Positions, key: [i64; 4], value: &mut mame::Device) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Device,
        key,
        mame::MameDeviceAttribute::from_code,
        &[
            true,
            value.tag.is_some(),
            value.fixed_image.is_some(),
            value.mandatory.is_some(),
            value.interface.is_some(),
        ],
    )?;
    if let Some(instance) = &mut value.instance {
        instance.attribute_positions = take(
            positions,
            Family::Instance,
            key,
            mame::MameInstanceAttribute::from_code,
            &[true, true],
        )?;
    }
    for (index, extension) in value.extensions.iter_mut().enumerate() {
        extension.attribute_positions = take(
            positions,
            Family::Extension,
            [key[0], key[1], order(index)?, 0],
            mame::MameExtensionAttribute::from_code,
            &[true],
        )?;
    }
    Ok(())
}

fn slot(positions: &mut Positions, key: [i64; 4], value: &mut mame::Slot) -> QueryResult {
    value.attribute_positions = take(
        positions,
        Family::Slot,
        key,
        mame::MameSlotAttribute::from_code,
        &[true],
    )?;
    for (index, option) in value.options.iter_mut().enumerate() {
        option.attribute_positions = take(
            positions,
            Family::SlotOption,
            [key[0], key[1], order(index)?, 0],
            mame::MameSlotOptionAttribute::from_code,
            &[true, true, option.default_specified],
        )?;
    }
    Ok(())
}

fn condition(
    positions: &mut Positions,
    family: Family,
    key: [i64; 4],
    condition: Option<&mut mame::MachineCondition>,
) -> QueryResult {
    if let Some(condition) = condition {
        condition.attribute_positions = take(
            positions,
            family,
            key,
            mame::MameConditionAttribute::from_code,
            &[true, true, true, true],
        )?;
    }
    Ok(())
}
