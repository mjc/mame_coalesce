//! Checked conversion for native attribute-position ledgers.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt};

use crate::{
    logiqx::{AttributePosition, RecordLocation},
    xml_reader::{AttributeLocation, DeclaredText},
};

use super::{NoIntroDatQueryError, queries};

type QueryResult<T> = Result<T, NoIntroDatQueryError>;

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    position_domain: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(Clone)]
pub(super) struct OwnerPositions<Field> {
    pub attributes: Vec<AttributePosition<Field>>,
    codes: BTreeSet<i64>,
}

impl<Field> Default for OwnerPositions<Field> {
    fn default() -> Self {
        Self {
            attributes: Vec::new(),
            codes: BTreeSet::new(),
        }
    }
}

pub(super) fn load<Field: Copy>(
    connection: &mut SqliteConnection,
    table: &str,
    owner_column: &str,
    owner_ids: &[i64],
    decode: fn(i64) -> Option<Field>,
) -> QueryResult<BTreeMap<i64, OwnerPositions<Field>>> {
    let game_fields = table == "no_intro_dat_game_field_positions";
    let mut result = owner_ids
        .iter()
        .copied()
        .map(|owner| (owner, OwnerPositions::default()))
        .collect::<BTreeMap<_, _>>();
    for ids in owner_ids.chunks(400) {
        let mut query = sql_query(queries::position_rows(table, owner_column, ids.len()))
            .into_boxed::<diesel::sqlite::Sqlite>();
        for id in ids {
            query = query.bind::<BigInt, _>(*id);
        }
        let mut unique_positions = BTreeSet::new();
        for row in query.load::<PositionRow>(connection)? {
            if row.valid != 1 || row.source_order < 0 || row.line <= 0 || row.column <= 0 {
                return Err(NoIntroDatQueryError::InvalidPositions(row.owner_id));
            }
            if row.position_domain != i64::from(game_fields && row.field_kind == 4)
                || !unique_positions.insert((row.owner_id, row.position_domain, row.source_order))
            {
                return Err(NoIntroDatQueryError::InvalidPositions(row.owner_id));
            }
            let owner = result
                .get_mut(&row.owner_id)
                .ok_or(NoIntroDatQueryError::InvalidPositions(row.owner_id))?;
            let field = decode(row.field_kind)
                .ok_or(NoIntroDatQueryError::InvalidPositions(row.owner_id))?;
            let source_order = usize::try_from(row.source_order)
                .map_err(|_| NoIntroDatQueryError::InvalidPositions(row.owner_id))?;
            if !owner.codes.insert(row.field_kind) {
                return Err(NoIntroDatQueryError::InvalidPositions(row.owner_id));
            }
            owner.attributes.push(AttributePosition {
                field,
                source_order,
                location: AttributeLocation {
                    line: row.line,
                    column: row.column,
                },
            });
        }
    }
    for positions in result.values_mut() {
        positions
            .attributes
            .sort_by_key(|position| position.source_order);
    }
    Ok(result)
}

pub(super) fn validate_presence<Field>(
    positions: &OwnerPositions<Field>,
    expected: &[bool],
    owner: i64,
) -> QueryResult<()> {
    if positions.codes.len() != expected.iter().filter(|present| **present).count() {
        return Err(NoIntroDatQueryError::InvalidPositions(owner));
    }
    for (code, present) in expected.iter().enumerate() {
        let code =
            i64::try_from(code).map_err(|_| NoIntroDatQueryError::InvalidPositions(owner))?;
        if *present != positions.codes.contains(&code) {
            return Err(NoIntroDatQueryError::InvalidPositions(owner));
        }
    }
    Ok(())
}

pub(super) fn declared<Field: Copy + PartialEq>(
    value: Option<String>,
    positions: &OwnerPositions<Field>,
    code: i64,
    owner: i64,
    decode: fn(i64) -> Option<Field>,
) -> QueryResult<Option<DeclaredText>> {
    let wanted = decode(code).ok_or(NoIntroDatQueryError::InvalidPositions(owner))?;
    let position = positions
        .attributes
        .iter()
        .find(|position| position.field == wanted);
    match (value, position) {
        (None, None) => Ok(None),
        (Some(value), Some(position)) => Ok(Some(DeclaredText {
            value,
            source_order: position.source_order,
            location: RecordLocation {
                line: position.location.line,
                column: position.location.column,
            },
        })),
        _ => Err(NoIntroDatQueryError::InvalidPositions(owner)),
    }
}

pub(super) const fn location(line: i64, column: i64, owner: i64) -> QueryResult<RecordLocation> {
    if line > 0 && column > 0 {
        Ok(RecordLocation { line, column })
    } else {
        Err(NoIntroDatQueryError::InvalidPositions(owner))
    }
}

pub(super) const fn require_integer(valid: i64, owner: i64) -> QueryResult<()> {
    if valid == 1 {
        Ok(())
    } else {
        Err(NoIntroDatQueryError::InvalidPositions(owner))
    }
}
