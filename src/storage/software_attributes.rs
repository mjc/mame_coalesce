//! Closed software attribute ownership and checked position-only query hydration.
use crate::{
    domain::SnapshotKey,
    storage::catalog_files::{SourceLocation, XmlAttributePosition},
};
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Text},
};
use std::collections::{BTreeMap, BTreeSet};

// Materialize only the requested edition before comparing the two native views.
// The global violations view also audits orphans, and must not be used here.
const EDITION_VALIDATION_QUERY: &str = "
WITH expected AS MATERIALIZED (
    SELECT * FROM software_expected_attribute_positions WHERE snapshot_key=?1
), actual AS MATERIALIZED (
    SELECT * FROM software_actual_attribute_positions WHERE snapshot_key=?1
)
SELECT EXISTS(SELECT * FROM expected EXCEPT SELECT * FROM actual)
    OR EXISTS(SELECT * FROM actual EXCEPT SELECT * FROM expected)
    OR EXISTS(
        SELECT 1 FROM software_wrapper_headers AS wrapper
        CROSS JOIN software_wrapper_attribute_positions AS position
        WHERE wrapper.snapshot_key=?1 AND position.wrapper_id=wrapper.wrapper_id
          AND (typeof(position.wrapper_id)<>'integer' OR position.wrapper_id<=0
            OR typeof(position.field_kind)<>'integer' OR position.field_kind<>0
            OR typeof(position.source_order)<>'integer' OR position.source_order<0
            OR typeof(position.source_line)<>'integer' OR position.source_line<=0
            OR typeof(position.source_column)<>'integer' OR position.source_column<=0)
    ) AS invalid";

#[derive(QueryableByName)]
struct EditionValidation {
    #[diesel(sql_type = Bool)]
    invalid: bool,
}

pub(super) fn validate_edition(
    conn: &mut SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<()> {
    let validation = sql_query(EDITION_VALIDATION_QUERY)
        .bind::<Text, _>(key.as_str())
        .get_result::<EditionValidation>(conn)?;
    if validation.invalid {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Family {
    Wrapper,
    List,
    Item,
    Info,
    SharedFeature,
    Part,
    Feature,
    DataArea,
    DiskArea,
    Rom,
    Disk,
    DipSwitch,
    DipValue,
}

impl Family {
    const fn table(self) -> &'static str {
        match self {
            Self::Wrapper => "software_wrapper_attribute_positions",
            Self::List => "software_list_attribute_positions",
            Self::Item => "software_item_attribute_positions",
            Self::Info => "software_item_info_attribute_positions",
            Self::SharedFeature => "software_item_shared_feature_attribute_positions",
            Self::Part => "software_part_attribute_positions",
            Self::Feature => "software_part_feature_attribute_positions",
            Self::DataArea => "software_data_area_attribute_positions",
            Self::DiskArea => "software_disk_area_attribute_positions",
            Self::Rom => "software_rom_attribute_positions",
            Self::Disk => "software_disk_attribute_positions",
            Self::DipSwitch => "software_part_dipswitch_attribute_positions",
            Self::DipValue => "software_part_dip_value_attribute_positions",
        }
    }
    const fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Wrapper => &["wrapper_id"],
            Self::List => &["namespace_id"],
            Self::Item => &["record_id"],
            Self::Info | Self::SharedFeature => &["record_id", "value_order"],
            Self::Part => &["part_id"],
            Self::Feature => &["part_id", "value_order"],
            Self::DataArea | Self::DiskArea => &["area_id"],
            Self::Rom | Self::Disk => &["occurrence_id"],
            Self::DipSwitch => &["part_id", "dipswitch_order"],
            Self::DipValue => &["part_id", "dipswitch_order", "value_order"],
        }
    }
}

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Bool)]
    valid_types: bool,
    #[diesel(sql_type=BigInt)]
    owner_a: i64,
    #[diesel(sql_type=BigInt)]
    owner_b: i64,
    #[diesel(sql_type=BigInt)]
    owner_c: i64,
    #[diesel(sql_type=BigInt)]
    field_kind: i64,
    #[diesel(sql_type=BigInt)]
    source_order: i64,
    #[diesel(sql_type=BigInt)]
    source_line: i64,
    #[diesel(sql_type=BigInt)]
    source_column: i64,
}

#[derive(Default)]
pub(super) struct Positions(BTreeMap<(Family, i64, i64, i64), Vec<Row>>);
impl Positions {
    pub(super) fn load(
        &mut self,
        conn: &mut SqliteConnection,
        family: Family,
        owner_scope: &str,
    ) -> crate::Result<()> {
        let keys = family.keys();
        let columns = keys
            .iter()
            .map(|key| format!("position.{key}"))
            .chain(std::iter::repeat_n("0".into(), 3 - keys.len()))
            .zip(["owner_a", "owner_b", "owner_c"])
            .map(|(key, alias)| format!("{key} AS {alias}"))
            .collect::<Vec<_>>()
            .join(",");
        let scope = owner_scope.replace("__NATIVE_POSITIONS__", family.table());
        let valid_types = keys
            .iter()
            .chain(["field_kind", "source_order", "source_line", "source_column"].iter())
            .map(|key| format!("typeof(position.{key})='integer'"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let query = format!(
            "SELECT {columns}, ({valid_types}) AS valid_types, position.field_kind,position.source_order,position.source_line,position.source_column {scope} ORDER BY owner_a,owner_b,owner_c,position.source_order"
        );
        for row in sql_query(query).load::<Row>(conn)? {
            self.0
                .entry((family, row.owner_a, row.owner_b, row.owner_c))
                .or_default()
                .push(row);
        }
        Ok(())
    }
    pub(super) fn take<Field: Copy>(
        &mut self,
        family: Family,
        key: (i64, i64, i64),
        decode: fn(i64) -> Option<Field>,
        expected: &[bool],
    ) -> crate::Result<Vec<XmlAttributePosition<Field>>> {
        let rows = self
            .0
            .remove(&(family, key.0, key.1, key.2))
            .unwrap_or_default();
        let mut fields = BTreeSet::new();
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
                || !fields.insert(row.field_kind)
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
        if fields.len() != expected.iter().filter(|&&present| present).count() {
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
fn invalid() -> crate::Error {
    crate::Error::XmlValidation(
        "invalid native software attribute presence, ownership or positions".into(),
    )
}

#[cfg(test)]
mod tests;
