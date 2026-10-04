//! Complete, selected-game-only descendant hydration.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::{
    domain::CatalogSetId,
    storage::{catalog_files::OccurrenceId, no_intro_dat_fields::NoIntroDatRomField},
    xml_reader::DeclaredText,
};

use super::{
    NoIntroDatGame, NoIntroDatGameChild, NoIntroDatQueryError, NoIntroDatRelease,
    NoIntroDatReleaseKey, NoIntroDatRepeatedText, NoIntroDatRomReference, positions, queries,
};

type QueryResult<T> = Result<T, NoIntroDatQueryError>;

#[derive(QueryableByName)]
struct RepeatedRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = BigInt)]
    family_order: i64,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ReleaseRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = BigInt)]
    family_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    region: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    name_source_order: i64,
    #[diesel(sql_type = BigInt)]
    name_source_line: i64,
    #[diesel(sql_type = BigInt)]
    name_source_column: i64,
    #[diesel(sql_type = BigInt)]
    region_source_order: i64,
    #[diesel(sql_type = BigInt)]
    region_source_line: i64,
    #[diesel(sql_type = BigInt)]
    region_source_column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct RomRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_order: i64,
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct MixedChildRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

pub(super) struct HydratedChildren {
    pub games: Vec<NoIntroDatGame>,
    pub category_count: i64,
    pub identifier_count: i64,
    pub release_count: i64,
    pub rom_count: i64,
    pub game_field_count: i64,
    pub rom_field_count: i64,
}

pub(super) fn hydrate(
    connection: &mut SqliteConnection,
    snapshot: &str,
    mut games: Vec<NoIntroDatGame>,
    total_roms: i64,
    global_header_filter: bool,
) -> QueryResult<HydratedChildren> {
    if games.is_empty() {
        return Ok(HydratedChildren {
            games,
            category_count: 0,
            identifier_count: 0,
            release_count: 0,
            rom_count: 0,
            game_field_count: 0,
            rom_field_count: 0,
        });
    }
    let game_ids = games
        .iter()
        .map(|game| game.id.as_i64())
        .collect::<Vec<_>>();
    let mut by_game = game_ids
        .iter()
        .copied()
        .map(|id| (id, Vec::<NoIntroDatGameChild>::new()))
        .collect::<BTreeMap<_, _>>();
    let mut category_count = 0_i64;
    let mut identifier_count = 0_i64;
    let mut release_count = 0_i64;
    let mut category_orders = BTreeMap::<i64, i64>::new();
    let mut identifier_orders = BTreeMap::<i64, i64>::new();
    let mut release_orders = BTreeMap::<i64, i64>::new();

    for ids in game_ids.chunks(400) {
        for row in load_repeated(connection, &queries::category_rows(ids.len()), ids)? {
            checked_family(&row, &mut category_orders, &mut category_count)?;
            let game_id = row.game_id;
            let text = repeated_text(row)?;
            by_game
                .get_mut(&game_id)
                .ok_or(NoIntroDatQueryError::MismatchedOwner(game_id))?
                .push(NoIntroDatGameChild::Category(text));
        }
        for row in load_repeated(connection, &queries::identifier_rows(ids.len()), ids)? {
            checked_family(&row, &mut identifier_orders, &mut identifier_count)?;
            let game_id = row.game_id;
            let text = repeated_text(row)?;
            by_game
                .get_mut(&game_id)
                .ok_or(NoIntroDatQueryError::MismatchedOwner(game_id))?
                .push(NoIntroDatGameChild::Identifier(text));
        }
        let mut query =
            sql_query(queries::release_rows(ids.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in ids {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<ReleaseRow>(connection)? {
            require_row(row.valid, row.game_id)?;
            let release_order =
                next_family_order(&mut release_orders, row.game_id, row.family_order)?;
            let game_id = row.game_id;
            let release = release(row, release_order)?;
            by_game
                .get_mut(&game_id)
                .ok_or(NoIntroDatQueryError::MismatchedOwner(game_id))?
                .push(NoIntroDatGameChild::Release(release));
            release_count = release_count
                .checked_add(1)
                .ok_or(NoIntroDatQueryError::InvalidMetadata(game_id))?;
        }
    }

    let references = load_rom_references(
        connection,
        snapshot,
        &game_ids,
        total_roms,
        global_header_filter,
    )?;
    let rom_count =
        i64::try_from(references.len()).map_err(|_| NoIntroDatQueryError::InvalidMetadata(0))?;
    let mut rom_field_count = 0_i64;
    for (game_id, reference) in references {
        rom_field_count = rom_field_count
            .checked_add(
                i64::try_from(reference.attribute_positions.len())
                    .map_err(|_| NoIntroDatQueryError::InvalidMetadata(game_id))?,
            )
            .ok_or(NoIntroDatQueryError::InvalidMetadata(game_id))?;
        by_game
            .get_mut(&game_id)
            .ok_or(NoIntroDatQueryError::MismatchedOwner(game_id))?
            .push(NoIntroDatGameChild::Rom(Box::new(reference)));
    }

    validate_mixed_child_rows(connection, &game_ids)?;
    let game_field_count = attach_children(&mut games, by_game)?;
    Ok(HydratedChildren {
        games,
        category_count,
        identifier_count,
        release_count,
        rom_count,
        game_field_count,
        rom_field_count,
    })
}

fn attach_children(
    games: &mut [NoIntroDatGame],
    mut by_game: BTreeMap<i64, Vec<NoIntroDatGameChild>>,
) -> QueryResult<i64> {
    let mut game_field_count = 0_i64;
    for game in games {
        let mut children = by_game
            .remove(&game.id.as_i64())
            .ok_or(NoIntroDatQueryError::MismatchedOwner(game.id.as_i64()))?;
        game.children.append(&mut children);
        game.children.sort_by_key(NoIntroDatGameChild::source_order);
        ensure_child_order(&game.children, game.id.as_i64())?;
        game_field_count = game_field_count
            .checked_add(
                1 + i64::from(game.publisher_id.is_some())
                    + i64::from(game.cloneof.is_some())
                    + i64::from(game.cloneofid.is_some())
                    + i64::from(
                        game.children
                            .iter()
                            .any(|child| matches!(child, NoIntroDatGameChild::Description(_))),
                    ),
            )
            .ok_or(NoIntroDatQueryError::InvalidMetadata(game.id.as_i64()))?;
    }
    Ok(game_field_count)
}

fn repeated_text(row: RepeatedRow) -> QueryResult<NoIntroDatRepeatedText> {
    Ok(NoIntroDatRepeatedText {
        family_order: row.family_order,
        value: DeclaredText {
            source_order: source_order(row.source_order, row.game_id)?,
            location: positions::location(row.line, row.column, row.game_id)?,
            value: row.value,
        },
    })
}

fn validate_mixed_child_rows(
    connection: &mut SqliteConnection,
    game_ids: &[i64],
) -> QueryResult<()> {
    for ids in game_ids.chunks(400) {
        let mut query =
            sql_query(queries::mixed_child_rows(ids.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in ids {
            query = query.bind::<BigInt, _>(*id);
        }
        let rows = query.load::<MixedChildRow>(connection)?;
        let mut last = BTreeMap::<i64, i64>::new();
        for row in rows {
            require_row(row.valid, row.game_id)?;
            if row.source_order < 0
                || last
                    .insert(row.game_id, row.source_order)
                    .is_some_and(|prior| prior >= row.source_order)
            {
                return Err(NoIntroDatQueryError::InvalidPositions(row.game_id));
            }
        }
    }

    Ok(())
}

fn load_rom_references(
    connection: &mut SqliteConnection,
    snapshot: &str,
    game_ids: &[i64],
    total_roms: i64,
    global_header_filter: bool,
) -> QueryResult<Vec<(i64, NoIntroDatRomReference)>> {
    let mut declarations = Vec::new();
    for batch in game_ids.chunks(400) {
        let mut query =
            sql_query(queries::rom_rows(batch.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        declarations.extend(query.bind::<Text, _>(snapshot).load::<RomRow>(connection)?);
    }
    declarations.sort_by_key(|row| (row.game_id, row.source_order, row.occurrence_id));
    let ids = declarations
        .iter()
        .map(|row| OccurrenceId::try_from(row.occurrence_id))
        .collect::<crate::Result<Vec<_>>>()?;
    let mut payloads = crate::storage::catalog_files::no_intro_dat_rom_payloads(connection, &ids)?;
    let raw_ids = ids.iter().map(|id| id.database_value()).collect::<Vec<_>>();
    let mut field_positions = positions::load(
        connection,
        "no_intro_dat_rom_field_positions",
        "occurrence_id",
        &raw_ids,
        NoIntroDatRomField::from_code,
    )?;
    let mut next_orders = BTreeMap::new();
    let result = declarations
        .into_iter()
        .map(|row| {
            require_row(row.valid, row.occurrence_id)?;
            if row.occurrence_order < 0 || row.occurrence_order >= total_roms {
                return Err(NoIntroDatQueryError::InvalidMetadata(row.occurrence_id));
            }
            next_family_order(&mut next_orders, row.game_id, row.occurrence_order)?;
            let id = OccurrenceId::try_from(row.occurrence_id)?;
            let payload = payloads
                .remove(&id)
                .ok_or(NoIntroDatQueryError::MissingNative {
                    field: "ROM payload",
                    owner: row.occurrence_id,
                })?;
            let positions = field_positions
                .remove(&row.occurrence_id)
                .ok_or(NoIntroDatQueryError::InvalidPositions(row.occurrence_id))?;
            let game_id = row.game_id;
            Ok((
                game_id,
                rom_reference(&row, payload, positions, global_header_filter)?,
            ))
        })
        .collect::<QueryResult<Vec<_>>>()?;
    if !field_positions.is_empty() || !payloads.is_empty() {
        return Err(NoIntroDatQueryError::MismatchedOwner(0));
    }
    Ok(result)
}

fn rom_reference(
    row: &RomRow,
    payload: crate::storage::catalog_files::NoIntroDatRomPayload,
    field_positions: positions::OwnerPositions<NoIntroDatRomField>,
    global_header_filter: bool,
) -> QueryResult<NoIntroDatRomReference> {
    if payload.name != row.name
        || payload.source_order != row.source_order
        || payload.size.is_some_and(|size| size < 0)
        || payload.location.line != row.line
        || payload.location.column != row.column
        || payload.evidence_scope.as_str() != row.evidence_scope
        || row.evidence_provenance != "source_declared"
        || (payload.header_text.is_some() || global_header_filter)
            != (row.evidence_scope == "unknown")
    {
        return Err(NoIntroDatQueryError::MismatchedOwner(row.occurrence_id));
    }
    positions::validate_presence(
        &field_positions,
        &[
            true,
            payload.size_text.is_some(),
            payload.crc_text.is_some(),
            payload.md5_text.is_some(),
            payload.sha1_text.is_some(),
            payload.sha256_text.is_some(),
            payload.status_text.is_some(),
            payload.serial_text.is_some(),
            payload.header_text.is_some(),
            payload.date_text.is_some(),
            payload.mia_text.is_some(),
        ],
        row.occurrence_id,
    )?;
    Ok(NoIntroDatRomReference {
        occurrence_id: OccurrenceId::try_from(row.occurrence_id)?,
        occurrence_order: row.occurrence_order,
        source_order: source_order(row.source_order, row.occurrence_id)?,
        location: positions::location(row.line, row.column, row.occurrence_id)?,
        attribute_positions: field_positions.attributes,
        payload,
    })
}

fn release(row: ReleaseRow, release_order: i64) -> QueryResult<NoIntroDatRelease> {
    if row.name_source_order < 0
        || row.region_source_order < 0
        || row.name_source_order == row.region_source_order
    {
        return Err(NoIntroDatQueryError::InvalidPositions(row.game_id));
    }
    Ok(NoIntroDatRelease {
        key: NoIntroDatReleaseKey {
            game_id: CatalogSetId::try_from(row.game_id)?,
            release_order,
        },
        source_order: source_order(row.source_order, row.game_id)?,
        location: positions::location(row.line, row.column, row.game_id)?,
        name: declared(
            row.name,
            row.name_source_order,
            row.name_source_line,
            row.name_source_column,
            row.game_id,
        )?,
        region: declared(
            row.region,
            row.region_source_order,
            row.region_source_line,
            row.region_source_column,
            row.game_id,
        )?,
    })
}

fn load_repeated(
    connection: &mut SqliteConnection,
    sql: &str,
    ids: &[i64],
) -> QueryResult<Vec<RepeatedRow>> {
    let mut query = sql_query(sql.to_owned()).into_boxed::<diesel::sqlite::Sqlite>();
    for id in ids {
        query = query.bind::<BigInt, _>(*id);
    }
    let rows = query.load::<RepeatedRow>(connection)?;
    for row in &rows {
        require_row(row.valid, row.game_id)?;
    }
    Ok(rows)
}

fn checked_family(
    row: &RepeatedRow,
    next_orders: &mut BTreeMap<i64, i64>,
    count: &mut i64,
) -> QueryResult<()> {
    next_family_order(next_orders, row.game_id, row.family_order)?;
    *count = count
        .checked_add(1)
        .ok_or(NoIntroDatQueryError::InvalidMetadata(row.game_id))?;
    Ok(())
}

fn next_family_order(
    next_orders: &mut BTreeMap<i64, i64>,
    owner: i64,
    order: i64,
) -> QueryResult<i64> {
    let expected = next_orders.entry(owner).or_default();
    if order != *expected {
        return Err(NoIntroDatQueryError::InvalidMetadata(owner));
    }
    *expected = expected
        .checked_add(1)
        .ok_or(NoIntroDatQueryError::InvalidMetadata(owner))?;
    Ok(order)
}

fn source_order(value: i64, owner: i64) -> QueryResult<usize> {
    usize::try_from(value).map_err(|_| NoIntroDatQueryError::InvalidPositions(owner))
}

fn declared(
    value: String,
    order: i64,
    line: i64,
    column: i64,
    owner: i64,
) -> QueryResult<DeclaredText> {
    Ok(DeclaredText {
        value,
        source_order: source_order(order, owner)?,
        location: positions::location(line, column, owner)?,
    })
}

const fn require_row(valid: i64, owner: i64) -> QueryResult<()> {
    positions::require_integer(valid, owner)
}

fn ensure_child_order(children: &[NoIntroDatGameChild], owner: i64) -> QueryResult<()> {
    let mut seen = BTreeSet::new();
    if children
        .iter()
        .all(|child| seen.insert(child.source_order()))
    {
        Ok(())
    } else {
        Err(NoIntroDatQueryError::InvalidPositions(owner))
    }
}
