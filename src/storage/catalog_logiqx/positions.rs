use std::collections::{BTreeMap, BTreeSet};

use diesel::{QueryableByName, sql_types::BigInt};

use crate::logiqx::{AttributeLocation, AttributePosition};

use super::{
    LogiqxQueryError,
    reader::{QueryResult, location},
};

#[derive(QueryableByName)]
pub(super) struct PositionRow {
    #[diesel(sql_type = BigInt)]
    pub owner_id: i64,
    #[diesel(sql_type = BigInt)]
    pub child_order: i64,
    #[diesel(sql_type = BigInt)]
    pub field_kind: i64,
    #[diesel(sql_type = BigInt)]
    pub source_order: i64,
    #[diesel(sql_type = BigInt)]
    pub line: i64,
    #[diesel(sql_type = BigInt)]
    pub column: i64,
    #[diesel(sql_type = BigInt)]
    pub valid: i64,
}

pub(super) type PositionMap = BTreeMap<(i64, i64), Vec<PositionRow>>;

pub(super) const fn require_integer_provenance(valid: i64, owner: i64) -> QueryResult<()> {
    if valid == 1 {
        Ok(())
    } else {
        Err(LogiqxQueryError::InvalidPositions(owner))
    }
}

pub(super) fn group(rows: Vec<PositionRow>) -> PositionMap {
    let mut result = PositionMap::new();
    for row in rows {
        result
            .entry((row.owner_id, row.child_order))
            .or_default()
            .push(row);
    }
    result
}

pub(super) fn validate(rows: &[PositionRow], expected: &[bool], owner: i64) -> QueryResult<()> {
    let mut fields = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for row in rows {
        let code = usize::try_from(row.field_kind)
            .map_err(|_| LogiqxQueryError::InvalidPositions(owner))?;
        if row.valid != 1
            || row.source_order < 0
            || !expected.get(code).copied().unwrap_or(false)
            || !fields.insert(code)
            || !orders.insert(row.source_order)
        {
            return Err(LogiqxQueryError::InvalidPositions(owner));
        }
        location(row.line, row.column, owner)?;
    }
    if expected
        .iter()
        .enumerate()
        .any(|(code, present)| *present != fields.contains(&code))
    {
        return Err(LogiqxQueryError::InvalidPositions(owner));
    }
    Ok(())
}

pub(super) fn attributes<Field>(
    rows: Vec<PositionRow>,
    expected: &[bool],
    decode: fn(i64) -> Option<Field>,
    owner: i64,
) -> QueryResult<Vec<AttributePosition<Field>>> {
    validate(&rows, expected, owner)?;
    rows.into_iter()
        .map(|row| {
            Ok(AttributePosition {
                field: decode(row.field_kind).ok_or(LogiqxQueryError::InvalidPositions(owner))?,
                source_order: usize::try_from(row.source_order)
                    .map_err(|_| LogiqxQueryError::InvalidPositions(owner))?,
                location: AttributeLocation {
                    line: row.line,
                    column: row.column,
                },
            })
        })
        .collect()
}

pub(super) fn finish(map: &PositionMap) -> QueryResult<()> {
    if let Some((owner, _)) = map.keys().next() {
        Err(LogiqxQueryError::InvalidPositions(*owner))
    } else {
        Ok(())
    }
}

/// Source ordinals can contain vendor gaps; recognized children cannot share one.
pub(super) fn child_orders(orders: impl Iterator<Item = i64>, owner: i64) -> QueryResult<()> {
    let mut seen = BTreeSet::new();
    for order in orders {
        if order < 0 || !seen.insert(order) {
            return Err(LogiqxQueryError::InvalidPositions(owner));
        }
    }
    Ok(())
}
