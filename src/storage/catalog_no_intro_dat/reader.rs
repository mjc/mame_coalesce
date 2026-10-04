//! Transactional reader for one exact published flat-DAT snapshot.

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    NoIntroDatMode,
    database::Database,
    domain::{CatalogRegistryId, CatalogSetId, SnapshotKey},
    storage::catalog_content::registry_id,
    storage::no_intro_dat_fields::NoIntroDatGameField,
};

use super::{
    NoIntroDatCursor, NoIntroDatDocument, NoIntroDatGame, NoIntroDatGameChild, NoIntroDatPage,
    NoIntroDatParent, NoIntroDatQueryError, NoIntroDatSnapshot, header, positions, queries,
};

type QueryResult<T> = Result<T, NoIntroDatQueryError>;

#[derive(Default)]
struct ParentDeclarations {
    cloneof: Option<(i64, String)>,
    cloneofid: Option<(i64, String)>,
}

#[derive(QueryableByName)]
struct SnapshotRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    source_key: String,
    #[diesel(sql_type = Text)]
    source_name: String,
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    catalog_name: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Text)]
    interpretation_key: String,
    #[diesel(sql_type = Text)]
    format: String,
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = BigInt)]
    published: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    game_count: i64,
    #[diesel(sql_type = BigInt)]
    rom_count: i64,
    #[diesel(sql_type = BigInt)]
    category_count: i64,
    #[diesel(sql_type = BigInt)]
    identifier_count: i64,
    #[diesel(sql_type = BigInt)]
    release_count: i64,
    #[diesel(sql_type = BigInt)]
    clrmamepro_option_count: i64,
    #[diesel(sql_type = BigInt)]
    romcenter_option_count: i64,
    #[diesel(sql_type = BigInt)]
    header_field_count: i64,
    #[diesel(sql_type = BigInt)]
    clrmamepro_field_count: i64,
    #[diesel(sql_type = BigInt)]
    romcenter_field_count: i64,
    #[diesel(sql_type = BigInt)]
    game_field_count: i64,
    #[diesel(sql_type = BigInt)]
    rom_field_count: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct DocumentRow {
    #[diesel(sql_type = Nullable<Text>)]
    schema_location: Option<String>,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct GroupRow {
    #[diesel(sql_type = BigInt)]
    set_group_id: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = BigInt)]
    list_order: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct GameRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    set_group_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    list_order: i64,
    #[diesel(sql_type = Text)]
    source_element_kind: String,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    native_kind: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ParentRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = Text)]
    link_kind: String,
    #[diesel(sql_type = Text)]
    target_literal: String,
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    source_reference_kind: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    origin: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    snapshot_key: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

pub(super) fn games_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatCursor>,
    limit: super::NoIntroDatPageLimit,
) -> QueryResult<NoIntroDatPage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| read_page(connection, snapshot, cursor, limit))
}

fn read_page(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatCursor>,
    limit: super::NoIntroDatPageLimit,
) -> QueryResult<NoIntroDatPage> {
    let registry_id = registry_id(connection)?;
    validate_cursor(cursor, registry_id, snapshot)?;
    let snapshot_row = load_snapshot(connection, snapshot, registry_id)?;
    let counts = load_counts(connection, snapshot)?;
    let (document, header_filter) =
        load_document(connection, snapshot, &snapshot_row.format, &counts)?;
    let strict = matches!(
        document.mode,
        NoIntroDatMode::V3Strict | NoIntroDatMode::V4Strict
    );

    let group_id = load_root_group(connection, snapshot)?;
    let bound = i64::try_from(limit.0)
        .map_err(|_| NoIntroDatQueryError::PageLimitOverflow)?
        .checked_add(1)
        .ok_or(NoIntroDatQueryError::PageLimitOverflow)?;
    let anchor_source_order = load_cursor_anchor(
        connection,
        snapshot,
        cursor,
        group_id,
        document.header.source_order,
    )?;
    let rows = if let Some(cursor) = cursor {
        let mut query = sql_query(queries::games(true)).into_boxed::<diesel::sqlite::Sqlite>();
        query = query
            .bind::<BigInt, _>(group_id)
            .bind::<BigInt, _>(cursor.list_order)
            .bind::<BigInt, _>(cursor.owner_id.as_i64())
            .bind::<BigInt, _>(bound);
        query.load::<GameRow>(connection)?
    } else {
        let mut query = sql_query(queries::games(false)).into_boxed::<diesel::sqlite::Sqlite>();
        query = query.bind::<BigInt, _>(group_id).bind::<BigInt, _>(bound);
        query.load::<GameRow>(connection)?
    };
    let mut games = load_games(connection, rows, snapshot.as_str())?;
    validate_game_orders(
        &games,
        cursor,
        counts.game_count,
        bound,
        anchor_source_order,
        document.header.source_order,
    )?;
    let has_more = games.len() > limit.0;
    games.truncate(limit.0);
    let hydrated = super::children::hydrate(
        connection,
        snapshot.as_str(),
        games,
        counts.rom_count,
        header_filter,
    )?;
    for game in &hydrated.games {
        let file_count = game
            .children
            .iter()
            .filter(|child| matches!(child, NoIntroDatGameChild::Rom(_)))
            .count();
        if file_count == 0 || (strict && file_count != 1) {
            return Err(NoIntroDatQueryError::MissingNative {
                field: "required ROM cardinality",
                owner: game.id.as_i64(),
            });
        }
    }
    if !has_more
        && cursor.is_none()
        && i64::try_from(hydrated.games.len()).ok() == Some(counts.game_count)
        && (hydrated.category_count != counts.category_count
            || hydrated.identifier_count != counts.identifier_count
            || hydrated.release_count != counts.release_count
            || hydrated.rom_count != counts.rom_count
            || hydrated.game_field_count != counts.game_field_count
            || hydrated.rom_field_count != counts.rom_field_count)
    {
        return Err(NoIntroDatQueryError::InvalidMetadata(0));
    }
    let next_cursor = if has_more {
        hydrated.games.last().map(|game| NoIntroDatCursor {
            registry_id,
            snapshot_key: snapshot.clone(),
            list_order: game.list_order,
            owner_id: game.id,
        })
    } else {
        None
    };
    Ok(NoIntroDatPage {
        snapshot: snapshot_row,
        document,
        games: hydrated.games,
        next_cursor,
    })
}

fn load_document(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    format: &str,
    counts: &CountRow,
) -> QueryResult<(NoIntroDatDocument, bool)> {
    let document_row = sql_query(queries::DOCUMENT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<DocumentRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatQueryError::MissingDocument(snapshot.clone()))?;
    positions::require_integer(document_row.valid, 0)?;
    let document_location = positions::location(document_row.line, document_row.column, 0)?;
    let header = header::load(connection, snapshot.as_str())?;
    if header.text_count != counts.header_field_count
        || header.clrmamepro_count != counts.clrmamepro_option_count
        || header.romcenter_count != counts.romcenter_option_count
        || header.clrmamepro_field_count != counts.clrmamepro_field_count
        || header.romcenter_field_count != counts.romcenter_field_count
    {
        return Err(NoIntroDatQueryError::InvalidMetadata(0));
    }
    let header_filter = header.header.children.iter().any(|child| {
        matches!(
            child,
            super::NoIntroDatHeaderChild::ClrMamePro(options) if options.header.is_some()
        )
    });
    let document = NoIntroDatDocument {
        mode: mode(format).ok_or_else(|| NoIntroDatQueryError::NotPublished(snapshot.clone()))?,
        schema_location: document_row.schema_location,
        location: document_location,
        header: header.header,
    };
    let strict = matches!(
        document.mode,
        NoIntroDatMode::V3Strict | NoIntroDatMode::V4Strict
    );
    if strict
        && !document.header.children.iter().any(|child| {
            matches!(
                child,
                super::NoIntroDatHeaderChild::Text {
                    field: super::NoIntroDatHeaderField::Author,
                    ..
                }
            )
        })
    {
        return Err(NoIntroDatQueryError::MissingNative {
            field: "header author",
            owner: 0,
        });
    }

    Ok((document, header_filter))
}

fn load_cursor_anchor(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatCursor>,
    group_id: i64,
    header_source_order: usize,
) -> QueryResult<Option<usize>> {
    if let Some(cursor) = cursor {
        let anchor_row = sql_query(queries::CURSOR_ANCHOR)
            .bind::<BigInt, _>(group_id)
            .bind::<BigInt, _>(cursor.owner_id.as_i64())
            .bind::<BigInt, _>(cursor.list_order)
            .get_result::<GameRow>(connection)
            .optional()?
            .ok_or(NoIntroDatQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ))?;
        let anchor = load_games(connection, vec![anchor_row], snapshot.as_str())?
            .pop()
            .ok_or(NoIntroDatQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ))?;
        if (anchor.id, anchor.list_order) != (cursor.owner_id, cursor.list_order) {
            return Err(NoIntroDatQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ));
        }
        if anchor.source_order <= header_source_order {
            return Err(NoIntroDatQueryError::InvalidPositions(anchor.id.as_i64()));
        }
        Ok(Some(anchor.source_order))
    } else {
        Ok(None)
    }
}

fn load_root_group(connection: &mut SqliteConnection, snapshot: &SnapshotKey) -> QueryResult<i64> {
    let groups = sql_query(queries::ROOT_GROUPS)
        .bind::<Text, _>(snapshot.as_str())
        .load::<GroupRow>(connection)?;
    let [group] = groups.as_slice() else {
        return Err(NoIntroDatQueryError::MismatchedOwner(
            groups.first().map_or(0, |group| group.set_group_id),
        ));
    };
    if group.valid != 1 || group.kind != "root" || group.list_order != 0 || group.set_group_id <= 0
    {
        return Err(NoIntroDatQueryError::MismatchedOwner(group.set_group_id));
    }
    Ok(group.set_group_id)
}

fn load_snapshot(
    connection: &mut SqliteConnection,
    requested: &SnapshotKey,
    registry_id: CatalogRegistryId,
) -> QueryResult<NoIntroDatSnapshot> {
    let row = sql_query(queries::SNAPSHOT)
        .bind::<Text, _>(requested.as_str())
        .get_result::<SnapshotRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatQueryError::NotPublished(requested.clone()))?;
    if row.valid != 1
        || row.published != 1
        || row.snapshot_key != requested.as_str()
        || mode(&row.format).is_none()
    {
        return Err(NoIntroDatQueryError::NotPublished(requested.clone()));
    }
    Ok(NoIntroDatSnapshot {
        registry_id,
        snapshot_key: requested.clone(),
        source_key: crate::domain::PublishingSourceKey::new(row.source_key),
        source_name: row.source_name,
        catalog_key: crate::domain::CatalogKey::new(row.catalog_key),
        catalog_name: row.catalog_name,
        document_key: row.document_key.parse()?,
        interpretation_key: crate::domain::ParserInterpretationKey::from_persisted(
            row.interpretation_key,
        ),
        format: row.format,
        declared_version: row.declared_version,
    })
}

fn load_counts(connection: &mut SqliteConnection, snapshot: &SnapshotKey) -> QueryResult<CountRow> {
    let row = sql_query(queries::COUNTS)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<CountRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatQueryError::MissingDocument(snapshot.clone()))?;
    if row.valid != 1
        || [
            row.game_count,
            row.rom_count,
            row.category_count,
            row.identifier_count,
            row.release_count,
            row.clrmamepro_option_count,
            row.romcenter_option_count,
            row.header_field_count,
            row.clrmamepro_field_count,
            row.romcenter_field_count,
            row.game_field_count,
            row.rom_field_count,
        ]
        .into_iter()
        .any(|count| count < 0)
        || row.clrmamepro_option_count > 1
        || row.romcenter_option_count > 1
    {
        return Err(NoIntroDatQueryError::InvalidMetadata(0));
    }
    Ok(row)
}

fn validate_game_orders(
    games: &[NoIntroDatGame],
    cursor: Option<&NoIntroDatCursor>,
    total: i64,
    bound: i64,
    anchor_source_order: Option<usize>,
    header_source_order: usize,
) -> QueryResult<()> {
    let mut expected = match cursor {
        Some(cursor) if cursor.list_order < total => cursor.list_order.checked_add(1),
        Some(_) => None,
        None => Some(0),
    }
    .ok_or(NoIntroDatQueryError::InvalidMetadata(0))?;
    let mut previous_source_order = anchor_source_order;
    for game in games {
        if game.list_order != expected
            || game.list_order >= total
            || previous_source_order.is_some_and(|previous| game.source_order <= previous)
            || game.source_order <= header_source_order
        {
            return Err(NoIntroDatQueryError::InvalidMetadata(game.id.as_i64()));
        }
        previous_source_order = Some(game.source_order);
        expected = expected
            .checked_add(1)
            .ok_or(NoIntroDatQueryError::InvalidMetadata(game.id.as_i64()))?;
    }
    if i64::try_from(games.len()).ok() != Some(bound) && expected != total {
        return Err(NoIntroDatQueryError::InvalidMetadata(0));
    }
    Ok(())
}

fn validate_cursor(
    cursor: Option<&NoIntroDatCursor>,
    registry_id: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.registry_id != registry_id {
            return Err(NoIntroDatQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot_key != snapshot {
            return Err(NoIntroDatQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn mode(format: &str) -> Option<NoIntroDatMode> {
    match format {
        "no-intro-dat-v3-strict" => Some(NoIntroDatMode::V3Strict),
        "no-intro-dat-v3-compatible" => Some(NoIntroDatMode::V3Compatible),
        "no-intro-dat-v4-strict" => Some(NoIntroDatMode::V4Strict),
        "no-intro-dat-v4-compatible" => Some(NoIntroDatMode::V4Compatible),
        _ => None,
    }
}

fn source_order(value: i64, owner: i64) -> QueryResult<usize> {
    usize::try_from(value).map_err(|_| NoIntroDatQueryError::InvalidPositions(owner))
}

fn game_row_to_game(
    row: GameRow,
    positions_map: &super::positions::OwnerPositions<NoIntroDatGameField>,
    parents: ParentDeclarations,
) -> QueryResult<NoIntroDatGame> {
    let owner = row.set_id;
    if row.valid != 1
        || row.source_element_kind != "no_intro_dat_game"
        || row.native_id != Some(owner)
        || row.native_kind.as_deref() != Some("no_intro_dat_game")
        || row.list_order < 0
        || row.set_group_id <= 0
    {
        return Err(NoIntroDatQueryError::MismatchedOwner(owner));
    }
    if row.description_text.is_none() {
        return Err(NoIntroDatQueryError::MissingNative {
            field: "game description",
            owner,
        });
    }
    let source_order = source_order(
        row.source_order
            .ok_or(NoIntroDatQueryError::MissingNative {
                field: "game owner",
                owner,
            })?,
        owner,
    )?;
    let present = [
        true,
        row.id_text.is_some(),
        parents.cloneof.is_some(),
        parents.cloneofid.is_some(),
        row.description_text.is_some(),
    ];
    positions::validate_presence(positions_map, &present, owner)?;
    let name = positions::declared(
        Some(row.set_name),
        positions_map,
        NoIntroDatGameField::Name as i64,
        owner,
        NoIntroDatGameField::from_code,
    )?
    .ok_or(NoIntroDatQueryError::InvalidPositions(owner))?;
    let publisher_id = positions::declared(
        row.id_text,
        positions_map,
        NoIntroDatGameField::Id as i64,
        owner,
        NoIntroDatGameField::from_code,
    )?;
    let description = positions::declared(
        row.description_text,
        positions_map,
        NoIntroDatGameField::Description as i64,
        owner,
        NoIntroDatGameField::from_code,
    )?;
    let children = description
        .into_iter()
        .map(NoIntroDatGameChild::Description)
        .collect::<Vec<_>>();
    let cloneof = parent(
        NoIntroDatGameField::CloneOf,
        parents.cloneof,
        positions_map,
        owner,
    )?;
    let cloneofid = parent(
        NoIntroDatGameField::CloneOfId,
        parents.cloneofid,
        positions_map,
        owner,
    )?;
    Ok(NoIntroDatGame {
        id: CatalogSetId::try_from(owner)?,
        list_order: row.list_order,
        source_order,
        location: positions::location(row.line, row.column, owner)?,
        name,
        publisher_id,
        cloneof,
        cloneofid,
        children,
    })
}

fn load_games(
    connection: &mut SqliteConnection,
    rows: Vec<GameRow>,
    snapshot: &str,
) -> QueryResult<Vec<NoIntroDatGame>> {
    use std::collections::BTreeMap;
    let ids = rows.iter().map(|row| row.set_id).collect::<Vec<_>>();
    let mut field_positions = positions::load(
        connection,
        "no_intro_dat_game_field_positions",
        "set_id",
        &ids,
        NoIntroDatGameField::from_code,
    )?;
    let mut parents = ids
        .iter()
        .map(|id| (*id, ParentDeclarations::default()))
        .collect::<BTreeMap<_, _>>();
    for batch in ids.chunks(400) {
        let mut query =
            sql_query(queries::parent_rows(batch.len())).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<ParentRow>(connection)? {
            let expected_kind = match row.link_kind.as_str() {
                "cloneof" => "no_intro_dat_cloneof",
                "cloneofid" => "no_intro_dat_cloneofid",
                _ => return Err(NoIntroDatQueryError::MismatchedOwner(row.game_id)),
            };
            if row.valid != 1
                || row.relationship_id <= 0
                || row.source_reference_kind.as_deref() != Some(expected_kind)
                || row.origin.as_deref() != Some("source")
                || row.snapshot_key.as_deref() != Some(snapshot)
            {
                return Err(NoIntroDatQueryError::MismatchedOwner(row.game_id));
            }
            let owners = parents
                .get_mut(&row.game_id)
                .ok_or(NoIntroDatQueryError::MismatchedOwner(row.game_id))?;
            let slot = match row.link_kind.as_str() {
                "cloneof" => &mut owners.cloneof,
                "cloneofid" => &mut owners.cloneofid,
                _ => return Err(NoIntroDatQueryError::MismatchedOwner(row.game_id)),
            };
            if slot
                .replace((row.relationship_id, row.target_literal))
                .is_some()
            {
                return Err(NoIntroDatQueryError::MismatchedOwner(row.game_id));
            }
        }
    }
    rows.into_iter()
        .map(|row| {
            let owner = row.set_id;
            let positions = field_positions
                .remove(&owner)
                .ok_or(NoIntroDatQueryError::InvalidPositions(owner))?;
            let links = parents
                .remove(&owner)
                .ok_or(NoIntroDatQueryError::MismatchedOwner(owner))?;
            game_row_to_game(row, &positions, links)
        })
        .collect()
}

fn parent(
    field: NoIntroDatGameField,
    value: Option<(i64, String)>,
    positions: &super::positions::OwnerPositions<NoIntroDatGameField>,
    owner: i64,
) -> QueryResult<Option<NoIntroDatParent>> {
    match value {
        Some((relationship_id, literal)) => {
            let target = positions::declared(
                Some(literal),
                positions,
                field as i64,
                owner,
                NoIntroDatGameField::from_code,
            )?
            .ok_or(NoIntroDatQueryError::InvalidPositions(owner))?;
            Ok(Some(NoIntroDatParent {
                relationship_id,
                target,
            }))
        }
        None => Ok(None),
    }
}
