use diesel::{
    RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::{SnapshotSet, checked_order};

#[derive(Clone, Copy)]
enum MameConditionOwner {
    Adjuster { order: i64 },
    Switch { order: i64 },
    SwitchValue { switch_order: i64, value_order: i64 },
}

// Keep the exhaustive source-variant mapping together for schema review.
#[allow(clippy::too_many_lines)]
pub(super) fn insert(
    conn: &mut SqliteConnection,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    macro_rules! insert_record {
        ($table:literal, $order:expr, $location:expr; $( $column:literal : $sql_type:ty = $value:expr ),* $(,)?) => {{
            let columns = [$( $column ),*].join(", ");
            let placeholders = vec!["?"; [$( stringify!($column) ),*].len()].join(", ");
            let statement = format!(
                "INSERT INTO {} (set_id, element_order, source_line, source_column, {columns}) \
                 VALUES (?, ?, ?, ?, {placeholders})",
                $table
            );
            sql_query(statement)
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>($order)
                .bind::<BigInt, _>($location.line)
                .bind::<BigInt, _>($location.column)
                $(.bind::<$sql_type, _>($value))*
                .execute(conn)?;
        }};
    }

    for element in &set.specification {
        use crate::mame::MachineSpecification as Spec;
        let order = element.element_order;
        match &element.value {
            Spec::Sample(value) => insert_record!("mame_machine_samples", order, value.location;
                "name": Text = &value.name),
            Spec::Chip(value) => insert_record!("mame_machine_chips", order, value.location;
                "name": Text = &value.name,
                "tag": Nullable<Text> = value.tag.as_deref(),
                "kind": Text = value.kind.as_str(),
                "clock": Nullable<Text> = value.clock.as_deref()),
            Spec::Display(value) => insert_record!("mame_machine_displays", order, value.location;
                "tag": Nullable<Text> = value.tag.as_deref(),
                "kind": Text = value.kind.as_str(),
                "rotation": Nullable<Text> = value.rotation.map(crate::mame::DisplayRotation::as_str),
                "flip_x": diesel::sql_types::Bool = value.flip_x.as_bool(),
                "flip_x_specified": diesel::sql_types::Bool = value.flip_x_specified,
                "width": Nullable<Text> = value.width.as_deref(),
                "height": Nullable<Text> = value.height.as_deref(),
                "refresh": Text = &value.refresh,
                "pixel_clock": Nullable<Text> = value.pixel_clock.as_deref(),
                "horizontal_total": Nullable<Text> = value.horizontal_total.as_deref(),
                "horizontal_blank_end": Nullable<Text> = value.horizontal_blank_end.as_deref(),
                "horizontal_blank_start": Nullable<Text> = value.horizontal_blank_start.as_deref(),
                "vertical_total": Nullable<Text> = value.vertical_total.as_deref(),
                "vertical_blank_end": Nullable<Text> = value.vertical_blank_end.as_deref(),
                "vertical_blank_start": Nullable<Text> = value.vertical_blank_start.as_deref()),
            Spec::Sound(value) => insert_record!("mame_machine_sounds", order, value.location;
                "channels": Text = &value.channels),
            Spec::Input(value) => {
                insert_record!("mame_machine_inputs", order, value.location;
                    "service": diesel::sql_types::Bool = value.service.as_bool(),
                    "service_specified": diesel::sql_types::Bool = value.service_specified,
                    "tilt": diesel::sql_types::Bool = value.tilt.as_bool(),
                    "tilt_specified": diesel::sql_types::Bool = value.tilt_specified,
                    "players": Text = &value.players,
                    "coins": Nullable<Text> = value.coins.as_deref());
                for (control_order, control) in value.controls.iter().enumerate() {
                    sql_query(
                        "INSERT INTO mame_machine_input_controls \
                         (set_id, element_order, control_order, control_type, player, buttons, \
                          minimum, maximum, sensitivity, keydelta, reverse, reverse_specified, ways, ways2, ways3, source_line, source_column) \
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind::<BigInt, _>(set_id)
                    .bind::<BigInt, _>(order)
                    .bind::<BigInt, _>(checked_order(control_order, "MAME controls")?)
                    .bind::<Text, _>(&control.kind)
                    .bind::<Nullable<Text>, _>(control.player.as_deref())
                    .bind::<Nullable<Text>, _>(control.buttons.as_deref())
                    .bind::<Nullable<Text>, _>(control.minimum.as_deref())
                    .bind::<Nullable<Text>, _>(control.maximum.as_deref())
                    .bind::<Nullable<Text>, _>(control.sensitivity.as_deref())
                    .bind::<Nullable<Text>, _>(control.key_delta.as_deref())
                    .bind::<diesel::sql_types::Bool, _>(control.reverse.as_bool())
                    .bind::<diesel::sql_types::Bool, _>(control.reverse_specified)
                    .bind::<Nullable<Text>, _>(control.ways.as_deref())
                    .bind::<Nullable<Text>, _>(control.ways2.as_deref())
                    .bind::<Nullable<Text>, _>(control.ways3.as_deref())
                    .bind::<BigInt, _>(control.location.line)
                    .bind::<BigInt, _>(control.location.column)
                    .execute(conn)?;
                }
            }
            Spec::Port(value) => {
                insert_record!("mame_machine_ports", order, value.location;
                    "tag": Text = &value.tag);
                for (analog_order, analog) in value.analogs.iter().enumerate() {
                    sql_query(
                        "INSERT INTO mame_machine_analogs \
                         (set_id, element_order, analog_order, mask, source_line, source_column) \
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind::<BigInt, _>(set_id)
                    .bind::<BigInt, _>(order)
                    .bind::<BigInt, _>(checked_order(analog_order, "MAME analogs")?)
                    .bind::<Text, _>(&analog.mask)
                    .bind::<BigInt, _>(analog.location.line)
                    .bind::<BigInt, _>(analog.location.column)
                    .execute(conn)?;
                }
            }
            Spec::Adjuster(value) => {
                insert_record!("mame_machine_adjusters", order, value.location;
                    "name": Text = &value.name,
                    "default_value": Text = &value.default);
                if let Some(condition) = &value.condition {
                    insert_condition(
                        conn,
                        set_id,
                        MameConditionOwner::Adjuster { order },
                        condition,
                    )?;
                }
            }
            Spec::Driver(value) => insert_record!("mame_machine_drivers", order, value.location;
                "status": Text = value.status.as_str(),
                "emulation": Text = value.emulation.as_str(),
                "cocktail": Nullable<Text> = value.cocktail.map(crate::mame::DriverQuality::as_str),
                "savestate": Text = value.savestate.as_str(),
                "requires_artwork": diesel::sql_types::Bool = value.requires_artwork.as_bool(),
                "requires_artwork_specified": diesel::sql_types::Bool = value.requires_artwork_specified,
                "unofficial": diesel::sql_types::Bool = value.unofficial.as_bool(),
                "unofficial_specified": diesel::sql_types::Bool = value.unofficial_specified,
                "no_sound_hardware": diesel::sql_types::Bool = value.no_sound_hardware.as_bool(),
                "no_sound_hardware_specified": diesel::sql_types::Bool = value.no_sound_hardware_specified,
                "incomplete": diesel::sql_types::Bool = value.incomplete.as_bool(),
                "incomplete_specified": diesel::sql_types::Bool = value.incomplete_specified),
            Spec::Feature(value) => insert_record!("mame_machine_features", order, value.location;
                "kind": Text = value.kind.as_str(),
                "status": Nullable<Text> = value.status.map(crate::mame::FeatureStatus::as_str),
                "overall": Nullable<Text> = value.overall.map(crate::mame::FeatureStatus::as_str)),
            Spec::Device(value) => {
                insert_record!("mame_machine_devices", order, value.location;
                    "kind": Text = &value.kind,
                    "tag": Nullable<Text> = value.tag.as_deref(),
                    "fixed_image": Nullable<Text> = value.fixed_image.as_deref(),
                    "mandatory": Nullable<Text> = value.mandatory.as_deref(),
                    "interface": Nullable<Text> = value.interface.as_deref());
                if let Some(instance) = &value.instance {
                    sql_query(
                        "INSERT INTO mame_machine_device_instances \
                         (set_id, element_order, name, brief_name, source_line, source_column) \
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind::<BigInt, _>(set_id)
                    .bind::<BigInt, _>(order)
                    .bind::<Text, _>(&instance.name)
                    .bind::<Text, _>(&instance.brief_name)
                    .bind::<BigInt, _>(instance.location.line)
                    .bind::<BigInt, _>(instance.location.column)
                    .execute(conn)?;
                }
                for (extension_order, extension) in value.extensions.iter().enumerate() {
                    sql_query(
                        "INSERT INTO mame_machine_device_extensions \
                         (set_id, element_order, extension_order, name, source_line, source_column) \
                         VALUES (?, ?, ?, ?, ?, ?)",
                    )
                    .bind::<BigInt, _>(set_id)
                    .bind::<BigInt, _>(order)
                    .bind::<BigInt, _>(checked_order(extension_order, "MAME device extensions")?)
                    .bind::<Text, _>(&extension.name)
                    .bind::<BigInt, _>(extension.location.line)
                    .bind::<BigInt, _>(extension.location.column)
                    .execute(conn)?;
                }
            }
            Spec::Slot(value) => {
                insert_record!("mame_machine_slots", order, value.location;
                    "name": Text = &value.name);
                for (option_order, option) in value.options.iter().enumerate() {
                    sql_query(
                        "INSERT INTO mame_machine_slot_options \
                         (set_id, element_order, option_order, name, devname, is_default, default_specified, source_line, source_column) \
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind::<BigInt, _>(set_id)
                    .bind::<BigInt, _>(order)
                    .bind::<BigInt, _>(checked_order(option_order, "MAME slot options")?)
                    .bind::<Text, _>(&option.name)
                    .bind::<Text, _>(&option.device_name)
                    .bind::<diesel::sql_types::Bool, _>(option.is_default.as_bool())
                    .bind::<diesel::sql_types::Bool, _>(option.default_specified)
                    .bind::<BigInt, _>(option.location.line)
                    .bind::<BigInt, _>(option.location.column)
                    .execute(conn)?;
                }
            }
            Spec::SoftwareList(value) => {
                insert_record!("mame_machine_software_lists", order, value.location;
                "tag": Text = &value.tag,
                "name": Text = &value.name,
                "status": Text = value.status.as_str(),
                "filter": Nullable<Text> = value.filter.as_deref());
            }
            Spec::RamOption(value) => {
                insert_record!("mame_machine_ram_options", order, value.location;
                "name": Text = &value.name,
                "default_value": Nullable<Text> = value.default.as_deref(),
                "text": Text = &value.text);
            }
        }
    }
    Ok(())
}

fn insert_condition(
    conn: &mut SqliteConnection,
    set_id: i64,
    owner: MameConditionOwner,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    match owner {
        MameConditionOwner::Adjuster { order } => {
            sql_query(
                "INSERT INTO mame_machine_adjuster_conditions \
                 (set_id, element_order, condition_order, tag, mask, relation, value, source_line, source_column) \
                 VALUES (?, ?, 0, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(order)
            .bind::<Text, _>(&condition.tag)
            .bind::<Text, _>(&condition.mask)
            .bind::<Text, _>(condition.relation.as_str())
            .bind::<Text, _>(&condition.value)
            .bind::<BigInt, _>(condition.location.line)
            .bind::<BigInt, _>(condition.location.column)
            .execute(conn)?;
        }
        MameConditionOwner::Switch { order } => {
            sql_query(
                "INSERT INTO machine_switch_conditions \
                 (set_id, switch_order, condition_order, tag, mask, relation, value, source_line, source_column) \
                 VALUES (?, ?, 0, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(order)
            .bind::<Text, _>(&condition.tag)
            .bind::<Text, _>(&condition.mask)
            .bind::<Text, _>(condition.relation.as_str())
            .bind::<Text, _>(&condition.value)
            .bind::<BigInt, _>(condition.location.line)
            .bind::<BigInt, _>(condition.location.column)
            .execute(conn)?;
        }
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        } => {
            sql_query(
                "INSERT INTO machine_switch_value_conditions \
                 (set_id, switch_order, value_order, condition_order, tag, mask, relation, value, source_line, source_column) \
                 VALUES (?, ?, ?, 0, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(value_order)
            .bind::<Text, _>(&condition.tag)
            .bind::<Text, _>(&condition.mask)
            .bind::<Text, _>(condition.relation.as_str())
            .bind::<Text, _>(&condition.value)
            .bind::<BigInt, _>(condition.location.line)
            .bind::<BigInt, _>(condition.location.column)
            .execute(conn)?;
        }
    }
    Ok(())
}

pub(super) fn insert_switch_condition(
    conn: &mut SqliteConnection,
    set_id: i64,
    switch_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition(
        conn,
        set_id,
        MameConditionOwner::Switch {
            order: switch_order,
        },
        condition,
    )
}

pub(super) fn insert_switch_value_condition(
    conn: &mut SqliteConnection,
    set_id: i64,
    switch_order: i64,
    value_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition(
        conn,
        set_id,
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        },
        condition,
    )
}
