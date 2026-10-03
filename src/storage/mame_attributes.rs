//! Closed MAME attribute position families and checked source-free hydration.
use crate::{
    domain::SnapshotKey,
    storage::catalog_files::{SourceLocation, XmlAttributePosition},
};
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Text},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Family {
    Document,
    Machine,
    Bios,
    Rom,
    Disk,
    DeviceReference,
    Sample,
    Chip,
    Display,
    Sound,
    Input,
    Control,
    Switch,
    SwitchLocation,
    SwitchValue,
    SwitchCondition,
    SwitchValueCondition,
    AdjusterCondition,
    Port,
    Analog,
    Adjuster,
    Driver,
    Feature,
    Device,
    Instance,
    Extension,
    Slot,
    SlotOption,
    SoftwareList,
    RamOption,
    MachineCompatibility,
    RomCompatibility,
    DiskCompatibility,
}

impl Family {
    pub(super) const fn code(self) -> i64 {
        match self {
            Self::Document => 0,
            Self::Machine => 1,
            Self::Bios => 2,
            Self::Rom => 3,
            Self::Disk => 4,
            Self::DeviceReference => 5,
            Self::Sample => 6,
            Self::Chip => 7,
            Self::Display => 8,
            Self::Sound => 9,
            Self::Input => 10,
            Self::Control => 11,
            Self::Switch => 12,
            Self::SwitchLocation => 13,
            Self::SwitchValue => 14,
            Self::SwitchCondition => 15,
            Self::SwitchValueCondition => 16,
            Self::AdjusterCondition => 17,
            Self::Port => 18,
            Self::Analog => 19,
            Self::Adjuster => 20,
            Self::Driver => 21,
            Self::Feature => 22,
            Self::Device => 23,
            Self::Instance => 24,
            Self::Extension => 25,
            Self::Slot => 26,
            Self::SlotOption => 27,
            Self::SoftwareList => 28,
            Self::RamOption => 29,
            Self::MachineCompatibility => 30,
            Self::RomCompatibility => 31,
            Self::DiskCompatibility => 32,
        }
    }

    pub(super) const fn table(self) -> &'static str {
        match self {
            Self::Document => "mame_document_facts_attribute_positions",
            Self::Machine => "mame_machines_attribute_positions",
            Self::Bios => "mame_bios_sets_attribute_positions",
            Self::Rom => "mame_rom_claims_attribute_positions",
            Self::Disk => "mame_disk_claims_attribute_positions",
            Self::DeviceReference => "mame_device_references_attribute_positions",
            Self::Sample => "mame_samples_attribute_positions",
            Self::Chip => "mame_machine_chips_attribute_positions",
            Self::Display => "mame_machine_displays_attribute_positions",
            Self::Sound => "mame_machine_sounds_attribute_positions",
            Self::Input => "mame_machine_inputs_attribute_positions",
            Self::Control => "mame_machine_input_controls_attribute_positions",
            Self::Switch => "machine_switches_attribute_positions",
            Self::SwitchLocation => "machine_switch_locations_attribute_positions",
            Self::SwitchValue => "machine_switch_values_attribute_positions",
            Self::SwitchCondition => "machine_switch_conditions_attribute_positions",
            Self::SwitchValueCondition => "machine_switch_value_conditions_attribute_positions",
            Self::AdjusterCondition => "mame_machine_adjuster_conditions_attribute_positions",
            Self::Port => "mame_machine_ports_attribute_positions",
            Self::Analog => "mame_machine_analogs_attribute_positions",
            Self::Adjuster => "mame_machine_adjusters_attribute_positions",
            Self::Driver => "mame_machine_drivers_attribute_positions",
            Self::Feature => "mame_machine_features_attribute_positions",
            Self::Device => "mame_machine_devices_attribute_positions",
            Self::Instance => "mame_machine_device_instances_attribute_positions",
            Self::Extension => "mame_machine_device_extensions_attribute_positions",
            Self::Slot => "mame_machine_slots_attribute_positions",
            Self::SlotOption => "mame_machine_slot_options_attribute_positions",
            Self::SoftwareList => "mame_machine_software_lists_attribute_positions",
            Self::RamOption => "mame_machine_ram_options_attribute_positions",
            Self::MachineCompatibility => "mame_machine_compatibility_attribute_positions",
            Self::RomCompatibility => "mame_rom_compatibility_attribute_positions",
            Self::DiskCompatibility => "mame_disk_compatibility_attribute_positions",
        }
    }

    pub(super) const fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Document => &["document_id"],
            Self::Machine | Self::MachineCompatibility => &["set_id"],
            Self::Bios => &["set_id", "bios_order"],
            Self::Rom
            | Self::Disk
            | Self::RomCompatibility
            | Self::DiskCompatibility
            | Self::Sample => &["occurrence_id"],
            Self::DeviceReference => &["set_id", "reference_order"],
            Self::Chip
            | Self::Display
            | Self::Sound
            | Self::Input
            | Self::Port
            | Self::Adjuster
            | Self::Driver
            | Self::Feature
            | Self::Device
            | Self::Slot
            | Self::SoftwareList
            | Self::RamOption
            | Self::Instance => &["set_id", "element_order"],
            Self::Control => &["set_id", "element_order", "control_order"],
            Self::Switch => &["set_id", "switch_order"],
            Self::SwitchLocation => &["set_id", "switch_order", "location_order"],
            Self::SwitchValue => &["set_id", "switch_order", "value_order"],
            Self::SwitchCondition => &["set_id", "switch_order", "condition_order"],
            Self::SwitchValueCondition => {
                &["set_id", "switch_order", "value_order", "condition_order"]
            }
            Self::AdjusterCondition => &["set_id", "element_order", "condition_order"],
            Self::Analog => &["set_id", "element_order", "analog_order"],
            Self::Extension => &["set_id", "element_order", "extension_order"],
            Self::SlotOption => &["set_id", "element_order", "option_order"],
        }
    }
}

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = Bool)]
    valid_types: bool,
    #[diesel(sql_type = BigInt)]
    owner_a: i64,
    #[diesel(sql_type = BigInt)]
    owner_b: i64,
    #[diesel(sql_type = BigInt)]
    owner_c: i64,
    #[diesel(sql_type = BigInt)]
    owner_d: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(Default)]
pub(super) struct Positions(BTreeMap<(Family, [i64; 4]), Vec<PositionRow>>);

impl Positions {
    pub(super) fn load(
        &mut self,
        conn: &mut SqliteConnection,
        family: Family,
        owner_scope: &str,
    ) -> crate::Result<()> {
        let columns = family
            .keys()
            .iter()
            .map(|key| {
                format!("CASE WHEN typeof(position.{key})='integer' THEN position.{key} ELSE 0 END")
            })
            .chain(std::iter::repeat_n("0".to_owned(), 4 - family.keys().len()))
            .zip(["owner_a", "owner_b", "owner_c", "owner_d"])
            .map(|(key, alias)| format!("{key} AS {alias}"))
            .collect::<Vec<_>>()
            .join(",");
        let valid = family
            .keys()
            .iter()
            .copied()
            .chain(["field_kind", "source_order", "source_line", "source_column"])
            .map(|key| format!("typeof(position.{key})='integer'"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let scope = owner_scope.replace("__NATIVE_POSITIONS__", family.table());
        let query = format!(
            "SELECT {columns}, ({valid}) AS valid_types, CASE WHEN typeof(position.field_kind)='integer' THEN position.field_kind ELSE 0 END AS field_kind, CASE WHEN typeof(position.source_order)='integer' THEN position.source_order ELSE 0 END AS source_order, CASE WHEN typeof(position.source_line)='integer' THEN position.source_line ELSE 0 END AS source_line, CASE WHEN typeof(position.source_column)='integer' THEN position.source_column ELSE 0 END AS source_column {scope} ORDER BY owner_a,owner_b,owner_c,owner_d,position.source_order"
        );
        for row in sql_query(query).load::<PositionRow>(conn)? {
            self.0
                .entry((family, [row.owner_a, row.owner_b, row.owner_c, row.owner_d]))
                .or_default()
                .push(row);
        }
        Ok(())
    }

    pub(super) fn take<Field: Copy>(
        &mut self,
        family: Family,
        key: [i64; 4],
        decode: fn(i64) -> Option<Field>,
        expected: &[bool],
    ) -> crate::Result<Vec<XmlAttributePosition<Field>>> {
        let key_len = family.keys().len();
        if key_len == 0
            || key_len > key.len()
            || key[0] <= 0
            || key[1..key_len].iter().any(|part| *part < 0)
            || key[key_len..].iter().any(|part| *part != 0)
        {
            return Err(invalid());
        }
        let rows = self.0.remove(&(family, key)).unwrap_or_default();
        let mut seen = BTreeSet::new();
        let mut previous = None;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let present = usize::try_from(row.field_kind)
                .ok()
                .and_then(|code| expected.get(code))
                .copied()
                == Some(true);
            if !row.valid_types
                || !present
                || !seen.insert(row.field_kind)
                || row.source_order < 0
                || previous.is_some_and(|order| order >= row.source_order)
                || row.source_line <= 0
                || row.source_column <= 0
            {
                return Err(invalid());
            }
            previous = Some(row.source_order);
            result.push(XmlAttributePosition {
                field: decode(row.field_kind).ok_or_else(invalid)?,
                source_order: row.source_order,
                location: SourceLocation {
                    line: row.source_line,
                    column: row.source_column,
                },
            });
        }
        if seen.len() != expected.iter().filter(|&&present| present).count() {
            return Err(invalid());
        }
        Ok(result)
    }

    pub(super) fn finish(self) -> crate::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        }
    }
}

const EDITION_VALIDATION_QUERY: &str = concat!(
    "WITH requested(snapshot_key) AS (SELECT snapshot_key FROM mame_document_facts WHERE snapshot_key=?1), ",
    include_str!("db/mame_attribute_readiness.sql")
);

pub(super) fn validate_edition(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<()> {
    #[derive(QueryableByName)]
    struct Validation {
        #[diesel(sql_type = Bool)]
        invalid: bool,
    }
    let row = sql_query(EDITION_VALIDATION_QUERY)
        .bind::<Text, _>(key.as_str())
        .get_result::<Validation>(conn)
        .optional()?;
    if row.is_some_and(|validation| validation.invalid) {
        Err(invalid())
    } else {
        Ok(())
    }
}

pub(super) fn insert<Field: Copy>(
    conn: &mut SqliteConnection,
    family: Family,
    key: &[i64],
    positions: &[crate::xml_reader::AttributePosition<Field>],
    code: fn(Field) -> i64,
) -> crate::Result<()> {
    if key.len() != family.keys().len() {
        return Err(invalid());
    }
    let statement = format!(
        "INSERT INTO {} ({},field_kind,source_order,source_line,source_column) VALUES ({})",
        family.table(),
        family.keys().join(","),
        std::iter::repeat_n("?", key.len() + 4)
            .collect::<Vec<_>>()
            .join(",")
    );
    for position in positions {
        let source_order = i64::try_from(position.source_order).map_err(|_| invalid())?;
        let query = sql_query(&statement);
        match key {
            [a] => query
                .bind::<BigInt, _>(*a)
                .bind::<BigInt, _>(code(position.field))
                .bind::<BigInt, _>(source_order)
                .bind::<BigInt, _>(position.location.line)
                .bind::<BigInt, _>(position.location.column)
                .execute(conn)?,
            [a, b] => query
                .bind::<BigInt, _>(*a)
                .bind::<BigInt, _>(*b)
                .bind::<BigInt, _>(code(position.field))
                .bind::<BigInt, _>(source_order)
                .bind::<BigInt, _>(position.location.line)
                .bind::<BigInt, _>(position.location.column)
                .execute(conn)?,
            [a, b, c] => query
                .bind::<BigInt, _>(*a)
                .bind::<BigInt, _>(*b)
                .bind::<BigInt, _>(*c)
                .bind::<BigInt, _>(code(position.field))
                .bind::<BigInt, _>(source_order)
                .bind::<BigInt, _>(position.location.line)
                .bind::<BigInt, _>(position.location.column)
                .execute(conn)?,
            [a, b, c, d] => query
                .bind::<BigInt, _>(*a)
                .bind::<BigInt, _>(*b)
                .bind::<BigInt, _>(*c)
                .bind::<BigInt, _>(*d)
                .bind::<BigInt, _>(code(position.field))
                .bind::<BigInt, _>(source_order)
                .bind::<BigInt, _>(position.location.line)
                .bind::<BigInt, _>(position.location.column)
                .execute(conn)?,
            _ => return Err(invalid()),
        };
    }
    Ok(())
}

fn invalid() -> crate::Error {
    crate::Error::XmlValidation(
        "invalid native MAME attribute presence, ownership or positions".into(),
    )
}

#[cfg(test)]
mod tests;
