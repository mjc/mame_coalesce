use diesel::{
    SqliteConnection,
    sql_types::{BigInt, Nullable, Text},
};

use super::{SnapshotSet, checked_order};
use crate::{
    mame,
    storage::{
        cached_sql::{InsertBatch, InsertPhase, cached_sql},
        mame_attributes::{Family, PositionBatch},
    },
};

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
    let mut batch = InsertBatch::new();
    insert_values_bulk(conn, &mut batch, set_id, set)?;
    batch.flush(conn)?;
    let mut positions = PositionBatch::default();
    insert_positions_bulk(conn, &mut positions, set_id, set)?;
    positions.flush(conn)
}

#[allow(clippy::too_many_lines)]
pub(super) fn insert_values_bulk(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    macro_rules! placeholder {
        ($column:literal) => {
            "?"
        };
    }
    macro_rules! insert_record {
        ($table:literal, $order:expr, $location:expr; $( $column:literal : $sql_type:ty = $value:expr ),* $(,)?) => {{
            cached_sql(concat!(
                "INSERT INTO ", $table,
                " (set_id, element_order, source_line, source_column",
                $( ", ", $column, )*
                ") VALUES (?, ?, ?, ?",
                $( ", ", placeholder!($column), )*
                ")"
            ))
                .bind::<BigInt, _>(set_id)
                .bind::<BigInt, _>($order)
                .bind::<BigInt, _>($location.line)
                .bind::<BigInt, _>($location.column)
                $(.bind::<$sql_type, _>($value))*
                .enqueue(batch, conn, InsertPhase::Parents)?;
        }};
    }

    for element in &set.specification {
        use crate::mame::MachineSpecification as Spec;
        let order = element.element_order;
        match &element.value {
            Spec::Sample(_) => {
                return Err(crate::Error::InvalidPath(
                    "MAME samples require actual media owners".into(),
                ));
            }
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
                    cached_sql(
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
                    .enqueue(batch, conn, InsertPhase::Children)?;
                }
            }
            Spec::Port(value) => {
                insert_record!("mame_machine_ports", order, value.location;
                    "tag": Text = &value.tag);
                for (analog_order, analog) in value.analogs.iter().enumerate() {
                    cached_sql(
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
                    .enqueue(batch, conn, InsertPhase::Children)?;
                }
            }
            Spec::Adjuster(value) => {
                insert_record!("mame_machine_adjusters", order, value.location;
                    "name": Text = &value.name,
                    "default_value": Text = &value.default);
                if let Some(condition) = &value.condition {
                    insert_condition_values(
                        batch,
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
                ensure_legacy_device_instance_cardinality(&value.instances)?;
                insert_record!("mame_machine_devices", order, value.location;
                    "kind": Text = &value.kind,
                    "tag": Nullable<Text> = value.tag.as_deref(),
                    "fixed_image": Nullable<Text> = value.fixed_image.as_deref(),
                    "mandatory": Nullable<Text> = value.mandatory.as_deref(),
                    "interface": Nullable<Text> = value.interface.as_deref());
                for instance in &value.instances {
                    cached_sql(
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
                    .enqueue(batch, conn, InsertPhase::Children)?;
                }
                for (extension_order, extension) in value.extensions.iter().enumerate() {
                    cached_sql(
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
                    .enqueue(batch, conn, InsertPhase::Children)?;
                }
            }
            Spec::Slot(value) => {
                insert_record!("mame_machine_slots", order, value.location;
                    "name": Text = &value.name);
                for (option_order, option) in value.options.iter().enumerate() {
                    cached_sql(
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
                    .enqueue(batch, conn, InsertPhase::Children)?;
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

#[allow(clippy::too_many_lines)]
pub(super) fn insert_positions_bulk(
    conn: &mut SqliteConnection,
    batch: &mut PositionBatch,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    macro_rules! queue {
        ($family:expr, $key:expr, $positions:expr, $code:path) => {
            batch.queue(conn, $family, $key, $positions, $code)?
        };
    }

    for element in &set.specification {
        use mame::MachineSpecification as Spec;
        let order = element.element_order;
        match &element.value {
            Spec::Sample(_) => {}
            Spec::Chip(value) => queue!(
                Family::Chip,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameChipAttribute::code
            ),
            Spec::Display(value) => queue!(
                Family::Display,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameDisplayAttribute::code
            ),
            Spec::Sound(value) => queue!(
                Family::Sound,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameSoundAttribute::code
            ),
            Spec::Input(value) => {
                queue!(
                    Family::Input,
                    &[set_id, order],
                    &value.attribute_positions,
                    mame::MameInputAttribute::code
                );
                for (index, control) in value.controls.iter().enumerate() {
                    queue!(
                        Family::Control,
                        &[set_id, order, checked_order(index, "MAME controls")?],
                        &control.attribute_positions,
                        mame::MameControlAttribute::code
                    );
                }
            }
            Spec::Port(value) => {
                queue!(
                    Family::Port,
                    &[set_id, order],
                    &value.attribute_positions,
                    mame::MamePortAttribute::code
                );
                for (index, analog) in value.analogs.iter().enumerate() {
                    queue!(
                        Family::Analog,
                        &[set_id, order, checked_order(index, "MAME analogs")?],
                        &analog.attribute_positions,
                        mame::MameAnalogAttribute::code
                    );
                }
            }
            Spec::Adjuster(value) => {
                queue!(
                    Family::Adjuster,
                    &[set_id, order],
                    &value.attribute_positions,
                    mame::MameAdjusterAttribute::code
                );
                if let Some(condition) = &value.condition {
                    insert_condition_positions(
                        conn,
                        batch,
                        set_id,
                        MameConditionOwner::Adjuster { order },
                        condition,
                    )?;
                }
            }
            Spec::Driver(value) => queue!(
                Family::Driver,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameDriverAttribute::code
            ),
            Spec::Feature(value) => queue!(
                Family::Feature,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameFeatureAttribute::code
            ),
            Spec::Device(value) => {
                ensure_legacy_device_instance_cardinality(&value.instances)?;
                queue!(
                    Family::Device,
                    &[set_id, order],
                    &value.attribute_positions,
                    mame::MameDeviceAttribute::code
                );
                for instance in &value.instances {
                    queue!(
                        Family::Instance,
                        &[set_id, order],
                        &instance.attribute_positions,
                        mame::MameInstanceAttribute::code
                    );
                }
                for (index, extension) in value.extensions.iter().enumerate() {
                    queue!(
                        Family::Extension,
                        &[
                            set_id,
                            order,
                            checked_order(index, "MAME device extensions")?
                        ],
                        &extension.attribute_positions,
                        mame::MameExtensionAttribute::code
                    );
                }
            }
            Spec::Slot(value) => {
                queue!(
                    Family::Slot,
                    &[set_id, order],
                    &value.attribute_positions,
                    mame::MameSlotAttribute::code
                );
                for (index, option) in value.options.iter().enumerate() {
                    queue!(
                        Family::SlotOption,
                        &[set_id, order, checked_order(index, "MAME slot options")?],
                        &option.attribute_positions,
                        mame::MameSlotOptionAttribute::code
                    );
                }
            }
            Spec::SoftwareList(value) => queue!(
                Family::SoftwareList,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameSoftwareListAttribute::code
            ),
            Spec::RamOption(value) => queue!(
                Family::RamOption,
                &[set_id, order],
                &value.attribute_positions,
                mame::MameRamOptionAttribute::code
            ),
        }
    }
    Ok(())
}

fn ensure_legacy_device_instance_cardinality(
    instances: &[mame::DeviceInstance],
) -> crate::Result<()> {
    if instances.len() > 1 {
        return Err(crate::Error::DatabaseSchema(
            "legacy MAME storage supports only one device instance per device".to_owned(),
        ));
    }
    Ok(())
}

fn insert_condition_values(
    batch: &mut InsertBatch,
    conn: &mut SqliteConnection,
    set_id: i64,
    owner: MameConditionOwner,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    match owner {
        MameConditionOwner::Adjuster { order } => {
            cached_sql(
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
            .enqueue(batch, conn, InsertPhase::Children)?;
        }
        MameConditionOwner::Switch { order } => {
            cached_sql(
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
            .enqueue(batch, conn, InsertPhase::Children)?;
        }
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        } => {
            cached_sql(
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
            .enqueue(batch, conn, InsertPhase::Children)?;
        }
    }
    Ok(())
}

fn insert_condition_positions(
    conn: &mut SqliteConnection,
    batch: &mut PositionBatch,
    set_id: i64,
    owner: MameConditionOwner,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    let (family, key): (Family, Vec<i64>) = match owner {
        MameConditionOwner::Adjuster { order } => {
            (Family::AdjusterCondition, vec![set_id, order, 0])
        }
        MameConditionOwner::Switch { order } => (Family::SwitchCondition, vec![set_id, order, 0]),
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        } => (
            Family::SwitchValueCondition,
            vec![set_id, switch_order, value_order, 0],
        ),
    };
    batch.queue(
        conn,
        family,
        &key,
        &condition.attribute_positions,
        mame::MameConditionAttribute::code,
    )
}

pub(super) fn insert_switch_condition_values(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    switch_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition_values(
        batch,
        conn,
        set_id,
        MameConditionOwner::Switch {
            order: switch_order,
        },
        condition,
    )
}

pub(super) fn insert_switch_condition_positions(
    conn: &mut SqliteConnection,
    batch: &mut PositionBatch,
    set_id: i64,
    switch_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition_positions(
        conn,
        batch,
        set_id,
        MameConditionOwner::Switch {
            order: switch_order,
        },
        condition,
    )
}

pub(super) fn insert_switch_value_condition_values(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    switch_order: i64,
    value_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition_values(
        batch,
        conn,
        set_id,
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        },
        condition,
    )
}

pub(super) fn insert_switch_value_condition_positions(
    conn: &mut SqliteConnection,
    batch: &mut PositionBatch,
    set_id: i64,
    switch_order: i64,
    value_order: i64,
    condition: &crate::mame::MachineCondition,
) -> crate::Result<()> {
    insert_condition_positions(
        conn,
        batch,
        set_id,
        MameConditionOwner::SwitchValue {
            switch_order,
            value_order,
        },
        condition,
    )
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use crate::storage::cached_sql::InsertBatch;
    use diesel::{
        Connection, RunQueryDsl, SqliteConnection,
        connection::{InstrumentationEvent, SimpleConnection},
        sql_query,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(diesel::QueryableByName)]
    struct OptionRow {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        option_order: i64,
        #[diesel(sql_type = diesel::sql_types::Text)]
        name: String,
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        default_specified: i64,
    }

    #[test]
    fn slot_options_reuse_one_statement_without_losing_order_or_presence()
    -> Result<(), Box<dyn std::error::Error>> {
        let catalog = crate::mame::MameCatalog::parse(br#"<mame mameconfig="10"><machine name="test"><description>Test</description><slot name="cart"><slotoption name="first" devname="device1"/><slotoption name="second" devname="device2" default="no"/><slotoption name="third" devname="device3" default="yes"/><slotoption name="fourth" devname="device4"/></slot></machine></mame>"#)?;
        let machine = catalog
            .machines
            .into_iter()
            .next()
            .ok_or("missing machine")?;
        let set = super::super::machine_contents(machine);
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("CREATE TABLE mame_machine_slots(set_id INTEGER,element_order INTEGER,source_line INTEGER,source_column INTEGER,name TEXT);
            CREATE TABLE mame_machine_slot_options(set_id INTEGER,element_order INTEGER,option_order INTEGER,name TEXT,devname TEXT,is_default INTEGER,default_specified INTEGER,source_line INTEGER,source_column INTEGER);
            CREATE TABLE mame_machine_slots_attribute_positions(set_id INTEGER,element_order INTEGER,field_kind INTEGER,source_order INTEGER,source_line INTEGER,source_column INTEGER);
            CREATE TABLE mame_machine_slot_options_attribute_positions(set_id INTEGER,element_order INTEGER,option_order INTEGER,field_kind INTEGER,source_order INTEGER,source_line INTEGER,source_column INTEGER);")?;
        let preparations = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&preparations);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.starts_with("INSERT INTO mame_machine_slot_options ")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });
        super::insert(&mut conn, 1, &set)?;
        super::insert(&mut conn, 2, &set)?;
        let rows = sql_query("SELECT option_order,name,default_specified FROM mame_machine_slot_options WHERE set_id=2 ORDER BY option_order").load::<OptionRow>(&mut conn)?;
        assert_eq!(
            rows.into_iter()
                .map(|row| (row.option_order, row.name, row.default_specified))
                .collect::<Vec<_>>(),
            vec![
                (0, "first".into(), 0),
                (1, "second".into(), 1),
                (2, "third".into(), 1),
                (3, "fourth".into(), 0),
            ]
        );
        assert_eq!(preparations.load(Ordering::Relaxed), 1);
        Ok(())
    }

    #[test]
    fn bulk_slot_options_use_bounded_values_inserts_and_keep_the_tail()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut xml = String::from(
            r#"<mame mameconfig="10"><machine name="test"><description>Test</description><slot name="cart">"#,
        );
        for order in 0..220 {
            write!(
                xml,
                "<slotoption name=\"option-{order}\" devname=\"device-{order}\"{}/>",
                if order == 219 { " default=\"yes\"" } else { "" }
            )?;
        }
        xml.push_str("</slot></machine></mame>");
        let catalog = crate::mame::MameCatalog::parse(xml.as_bytes())?;
        let machine = catalog
            .machines
            .into_iter()
            .next()
            .ok_or("missing machine")?;
        let set = super::super::machine_contents(machine);
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys=ON;
            CREATE TABLE mame_machine_slots(set_id INTEGER,element_order INTEGER,source_line INTEGER,source_column INTEGER,name TEXT,PRIMARY KEY(set_id,element_order));
            CREATE TABLE mame_machine_slot_options(set_id INTEGER,element_order INTEGER,option_order INTEGER,name TEXT,devname TEXT,is_default INTEGER,default_specified INTEGER,source_line INTEGER,source_column INTEGER,FOREIGN KEY(set_id,element_order) REFERENCES mame_machine_slots(set_id,element_order));")?;
        let executions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&executions);
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { sql, .. } = event
                && sql.starts_with("INSERT INTO mame_machine_slot_options")
            {
                observed.fetch_add(1, Ordering::Relaxed);
            }
        });

        let mut batch = InsertBatch::new();
        super::insert_values_bulk(&mut conn, &mut batch, 1, &set)?;
        batch.flush(&mut conn)?;

        let rows = sql_query("SELECT option_order,name,default_specified FROM mame_machine_slot_options WHERE set_id=1 ORDER BY option_order").load::<OptionRow>(&mut conn)?;
        assert_eq!(rows.len(), 220);
        assert_eq!(
            (rows[0].option_order, rows[0].name.as_str()),
            (0, "option-0")
        );
        assert_eq!(
            (
                rows[219].option_order,
                rows[219].name.as_str(),
                rows[219].default_specified
            ),
            (219, "option-219", 1)
        );
        assert!(executions.load(Ordering::Relaxed) <= 3);
        Ok(())
    }
}
