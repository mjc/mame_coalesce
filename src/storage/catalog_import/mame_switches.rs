//! Cross-owner BIOS and switch writes using the shared bounded collectors.

use diesel::{
    SqliteConnection,
    sql_types::{BigInt, Bool, Text},
};

use super::{SnapshotSet, checked_order, mame_specification};
use crate::{
    mame,
    storage::{
        cached_sql::{InsertBatch, InsertPhase, cached_sql},
        mame_attributes::{Family, PositionBatch},
    },
};

pub(super) fn insert_values_bulk(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (bios_order, bios) in set.bios_sets.iter().enumerate() {
        cached_sql(
            "INSERT INTO mame_bios_sets \
             (set_id, bios_order, name, description, is_default, default_specified, source_order, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(checked_order(bios_order, "MAME BIOS sets")?)
        .bind::<Text, _>(&bios.name)
        .bind::<Text, _>(&bios.description)
        .bind::<Bool, _>(bios.is_default)
        .bind::<Bool, _>(bios.default_specified)
        .bind::<BigInt, _>(bios.source_order)
        .bind::<BigInt, _>(bios.location.line)
        .bind::<BigInt, _>(bios.location.column)
        .enqueue(batch, conn, InsertPhase::Parents)?;
    }
    for (switch_order, switch) in set.switches.iter().enumerate() {
        let switch_order = checked_order(switch_order, "machine switches")?;
        insert_switch_values(conn, batch, set_id, switch_order, switch)?;
    }
    Ok(())
}

fn insert_switch_values(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    switch_order: i64,
    switch: &mame::MachineSwitch,
) -> crate::Result<()> {
    cached_sql(
        "INSERT INTO machine_switches \
         (set_id, switch_order, kind, name, tag, mask, source_order, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(switch_order)
    .bind::<Text, _>(switch.kind.as_str())
    .bind::<Text, _>(&switch.name)
    .bind::<Text, _>(&switch.tag)
    .bind::<Text, _>(&switch.mask)
    .bind::<BigInt, _>(switch.source_order)
    .bind::<BigInt, _>(switch.location.line)
    .bind::<BigInt, _>(switch.location.column)
    .enqueue(batch, conn, InsertPhase::Parents)?;
    if let Some(condition) = &switch.condition {
        mame_specification::insert_switch_condition_values(
            conn,
            batch,
            set_id,
            switch_order,
            condition,
        )?;
    }
    for (location_order, location) in switch.locations.iter().enumerate() {
        cached_sql(
            "INSERT INTO machine_switch_locations \
             (set_id, switch_order, location_order, source_order, name, number, inverted, inverted_specified, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(switch_order)
        .bind::<BigInt, _>(checked_order(location_order, "machine switch locations")?)
        .bind::<BigInt, _>(location.source_order)
        .bind::<Text, _>(&location.name)
        .bind::<Text, _>(&location.number)
        .bind::<Bool, _>(location.inverted)
        .bind::<Bool, _>(location.inverted_specified)
        .bind::<BigInt, _>(location.location.line)
        .bind::<BigInt, _>(location.location.column)
        .enqueue(batch, conn, InsertPhase::Children)?;
    }
    for (value_order, value) in switch.values.iter().enumerate() {
        let value_order = checked_order(value_order, "machine switch values")?;
        cached_sql(
            "INSERT INTO machine_switch_values \
             (set_id, switch_order, value_order, source_order, name, value, is_default, default_specified, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(switch_order)
        .bind::<BigInt, _>(value_order)
        .bind::<BigInt, _>(value.source_order)
        .bind::<Text, _>(&value.name)
        .bind::<Text, _>(&value.value)
        .bind::<Bool, _>(value.default)
        .bind::<Bool, _>(value.default_specified)
        .bind::<BigInt, _>(value.location.line)
        .bind::<BigInt, _>(value.location.column)
        .enqueue(batch, conn, InsertPhase::Children)?;
        if let Some(condition) = &value.condition {
            mame_specification::insert_switch_value_condition_values(
                conn,
                batch,
                set_id,
                switch_order,
                value_order,
                condition,
            )?;
        }
    }
    Ok(())
}

pub(super) fn insert_positions_bulk(
    conn: &mut SqliteConnection,
    batch: &mut PositionBatch,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (bios_order, bios) in set.bios_sets.iter().enumerate() {
        batch.queue(
            conn,
            Family::Bios,
            &[set_id, checked_order(bios_order, "MAME BIOS sets")?],
            &bios.attribute_positions,
            mame::MameBiosAttribute::code,
        )?;
    }
    for (switch_order, switch) in set.switches.iter().enumerate() {
        let switch_order = checked_order(switch_order, "machine switches")?;
        batch.queue(
            conn,
            Family::Switch,
            &[set_id, switch_order],
            &switch.attribute_positions,
            mame::MameSwitchAttribute::code,
        )?;
        if let Some(condition) = &switch.condition {
            mame_specification::insert_switch_condition_positions(
                conn,
                batch,
                set_id,
                switch_order,
                condition,
            )?;
        }
        for (location_order, location) in switch.locations.iter().enumerate() {
            batch.queue(
                conn,
                Family::SwitchLocation,
                &[
                    set_id,
                    switch_order,
                    checked_order(location_order, "machine switch locations")?,
                ],
                &location.attribute_positions,
                mame::MameSwitchLocationAttribute::code,
            )?;
        }
        for (value_order, value) in switch.values.iter().enumerate() {
            let value_order = checked_order(value_order, "machine switch values")?;
            batch.queue(
                conn,
                Family::SwitchValue,
                &[set_id, switch_order, value_order],
                &value.attribute_positions,
                mame::MameSwitchValueAttribute::code,
            )?;
            if let Some(condition) = &value.condition {
                mame_specification::insert_switch_value_condition_positions(
                    conn,
                    batch,
                    set_id,
                    switch_order,
                    value_order,
                    condition,
                )?;
            }
        }
    }
    Ok(())
}
