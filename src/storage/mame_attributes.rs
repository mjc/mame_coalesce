//! Closed MAME attribute position families and checked source-free hydration.
use crate::{
    domain::SnapshotKey,
    storage::catalog_files::{SourceLocation, XmlAttributePosition},
};
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, QueryFragment, QueryId},
    query_dsl::methods::ExecuteDsl,
    sql_query,
    sql_types::{BigInt, Bool, Text},
    sqlite::Sqlite,
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

const ATTRIBUTE_POSITION_BATCH_SIZE: usize = 64;

/// Numeric positions spanning owners, bounded to 64 rows per closed family
/// (132 KiB of row buffers across all 33 families). Flush before publication,
/// inside the caller's transaction; discard the collector if that transaction fails.
#[derive(Default)]
pub(super) struct PositionBatch(BTreeMap<Family, Vec<PositionBindValues>>);

impl PositionBatch {
    pub(super) fn queue<Field: Copy>(
        &mut self,
        conn: &mut SqliteConnection,
        family: Family,
        key: &[i64],
        positions: &[crate::xml_reader::AttributePosition<Field>],
        code: fn(Field) -> i64,
    ) -> crate::Result<()> {
        let key = owner_key(family, key)?;
        if positions.is_empty() {
            return Ok(());
        }
        let rows = self
            .0
            .entry(family)
            .or_insert_with(|| Vec::with_capacity(ATTRIBUTE_POSITION_BATCH_SIZE));
        for position in positions {
            let row = PositionBindValues::new(key, position, code)?;
            // A failed execution retains its rows and never grows the buffer.
            if rows.len() == ATTRIBUTE_POSITION_BATCH_SIZE {
                flush_position_rows(conn, family, rows)?;
            }
            rows.push(row);
            if rows.len() == ATTRIBUTE_POSITION_BATCH_SIZE {
                flush_position_rows(conn, family, rows)?;
            }
        }
        Ok(())
    }

    pub(super) fn flush(&mut self, conn: &mut SqliteConnection) -> crate::Result<()> {
        for (&family, rows) in &mut self.0 {
            flush_position_rows(conn, family, rows)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct PositionBindValues {
    key: [i64; 4],
    field_kind: i64,
    source_order: i64,
    source_line: i64,
    source_column: i64,
}

impl PositionBindValues {
    fn new<Field: Copy>(
        key: [i64; 4],
        position: &crate::xml_reader::AttributePosition<Field>,
        code: fn(Field) -> i64,
    ) -> crate::Result<Self> {
        Ok(Self {
            key,
            field_kind: code(position.field),
            source_order: i64::try_from(position.source_order).map_err(|_| invalid())?,
            source_line: position.location.line,
            source_column: position.location.column,
        })
    }
}

struct AttributePositionInsert<'a> {
    family: Family,
    rows: &'a [PositionBindValues],
}

impl QueryId for AttributePositionInsert<'_> {
    type QueryId = ();

    const HAS_STATIC_QUERY_ID: bool = false;
}

impl QueryFragment<Sqlite> for AttributePositionInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        if !self.rows.len().is_power_of_two() {
            pass.unsafe_to_cache_prepared();
        }
        pass.push_sql("INSERT INTO ");
        pass.push_sql(self.family.table());
        pass.push_sql(" (");
        for (index, key) in self.family.keys().iter().enumerate() {
            if index != 0 {
                pass.push_sql(",");
            }
            pass.push_sql(key);
        }
        pass.push_sql(",field_kind,source_order,source_line,source_column) VALUES ");
        for (position_index, row) in self.rows.iter().enumerate() {
            if position_index != 0 {
                pass.push_sql(", ");
            }
            pass.push_sql("(");
            for (key_index, key) in row.key[..self.family.keys().len()].iter().enumerate() {
                if key_index != 0 {
                    pass.push_sql(", ");
                }
                pass.push_bind_param::<BigInt, _>(key)?;
            }
            pass.push_sql(", ");
            pass.push_bind_param::<BigInt, _>(&row.field_kind)?;
            pass.push_sql(", ");
            pass.push_bind_param::<BigInt, _>(&row.source_order)?;
            pass.push_sql(", ");
            pass.push_bind_param::<BigInt, _>(&row.source_line)?;
            pass.push_sql(", ");
            pass.push_bind_param::<BigInt, _>(&row.source_column)?;
            pass.push_sql(")");
        }
        Ok(())
    }
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
    let key = owner_key(family, key)?;
    for batch in positions.chunks(ATTRIBUTE_POSITION_BATCH_SIZE) {
        let mut bind_values = [PositionBindValues::default(); ATTRIBUTE_POSITION_BATCH_SIZE];
        for (position, bind_values) in batch.iter().zip(&mut bind_values) {
            *bind_values = PositionBindValues::new(key, position, code)?;
        }
        ExecuteDsl::execute(
            AttributePositionInsert {
                family,
                rows: &bind_values[..batch.len()],
            },
            conn,
        )?;
    }
    Ok(())
}

fn owner_key(family: Family, key: &[i64]) -> crate::Result<[i64; 4]> {
    if key.len() != family.keys().len() {
        return Err(invalid());
    }
    let mut owner = [0; 4];
    owner[..key.len()].copy_from_slice(key);
    Ok(owner)
}

fn flush_position_rows(
    conn: &mut SqliteConnection,
    family: Family,
    rows: &mut Vec<PositionBindValues>,
) -> crate::Result<()> {
    if !rows.is_empty() {
        ExecuteDsl::execute(AttributePositionInsert { family, rows }, conn)?;
        rows.clear();
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
