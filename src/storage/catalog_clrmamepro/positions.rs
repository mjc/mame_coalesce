//! Checked native set scalar positions and value/position closure.

use super::{
    ClrMameProDeclaredValue, ClrMameProFieldPosition, ClrMameProQueryError, ClrMameProSetField,
    queries,
};
use crate::storage::catalog_files::SourceLocation;
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use std::collections::{BTreeMap, BTreeSet};

type QueryResult<T> = Result<T, ClrMameProQueryError>;

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type=BigInt)]
    owner_id: i64,
    #[diesel(sql_type=BigInt)]
    field_kind: i64,
    #[diesel(sql_type=Text)]
    source_field: String,
    #[diesel(sql_type=BigInt)]
    source_order: i64,
    #[diesel(sql_type=BigInt)]
    is_quoted: i64,
    #[diesel(sql_type=BigInt)]
    line: i64,
    #[diesel(sql_type=BigInt)]
    column: i64,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

pub(super) fn set_positions(
    connection: &mut SqliteConnection,
    ids: &[i64],
) -> QueryResult<BTreeMap<i64, BTreeMap<i64, ClrMameProFieldPosition>>> {
    let mut result = ids
        .iter()
        .copied()
        .map(|id| (id, BTreeMap::new()))
        .collect::<BTreeMap<_, _>>();
    for chunk in ids.chunks(400) {
        let mut query =
            sql_query(queries::set_positions(chunk.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in chunk {
            query = query.bind::<BigInt, _>(*id);
        }
        let mut orders = BTreeSet::new();
        for row in query.load::<PositionRow>(connection)? {
            let field = ClrMameProSetField::from_code(row.field_kind)
                .ok_or(ClrMameProQueryError::InvalidPositions(row.owner_id))?;
            if row.valid != 1
                || !row.source_field.eq_ignore_ascii_case(field.keyword())
                || !matches!(row.is_quoted, 0 | 1)
                || !orders.insert((row.owner_id, row.source_order))
            {
                return Err(ClrMameProQueryError::InvalidPositions(row.owner_id));
            }
            let position = ClrMameProFieldPosition {
                source_field: row.source_field,
                source_order: order(row.source_order, row.owner_id)?,
                is_quoted: row.is_quoted == 1,
                location: location(row.line, row.column, row.owner_id)?,
            };
            let owner = result
                .get_mut(&row.owner_id)
                .ok_or(ClrMameProQueryError::MismatchedOwner(row.owner_id))?;
            if owner.insert(row.field_kind, position).is_some() {
                return Err(ClrMameProQueryError::InvalidPositions(row.owner_id));
            }
        }
    }
    Ok(result)
}

pub(super) fn declared(
    value: Option<String>,
    field: ClrMameProSetField,
    positions: &mut BTreeMap<i64, ClrMameProFieldPosition>,
    owner: i64,
) -> QueryResult<Option<ClrMameProDeclaredValue>> {
    match (value, positions.remove(&(field as i64))) {
        (None, None) => Ok(None),
        (Some(value), Some(position)) => Ok(Some(ClrMameProDeclaredValue { value, position })),
        _ => Err(ClrMameProQueryError::InvalidPositions(owner)),
    }
}

pub(super) fn order(value: i64, owner: i64) -> QueryResult<usize> {
    usize::try_from(value).map_err(|_| ClrMameProQueryError::InvalidPositions(owner))
}

pub(super) const fn location(line: i64, column: i64, owner: i64) -> QueryResult<SourceLocation> {
    if line > 0 && column > 0 {
        Ok(SourceLocation { line, column })
    } else {
        Err(ClrMameProQueryError::InvalidPositions(owner))
    }
}
