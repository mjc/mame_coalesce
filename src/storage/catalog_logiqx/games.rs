use std::collections::BTreeMap;

use diesel::{
    OptionalExtension, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    domain::{CatalogSetId, SnapshotKey},
    logiqx::{BiosSetAttribute, GameAttribute, NameAttribute, ReleaseAttribute},
};

use super::{
    LogiqxArchiveReference, LogiqxBiosSet, LogiqxComment, LogiqxCursor, LogiqxDeviceReference,
    LogiqxGame, LogiqxParentReference, LogiqxQueryError, LogiqxRelease, LogiqxTextValue,
    positions::{self, PositionMap, PositionRow},
    queries,
    reader::{QueryResult, batch, default_presence, invalid, location, row, yes_no},
};

row!(GameRow {
    set_id: i64 => BigInt,
    set_name: String => Text,
    list_order: i64 => BigInt,
    source_element_kind: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
    native_id: Option<i64> => Nullable<BigInt>,
    source_file: Option<String> => Nullable<Text>,
    is_bios: Option<String> => Nullable<Text>,
    is_bios_was_present: Option<i64> => Nullable<BigInt>,
    board: Option<String> => Nullable<Text>,
    rebuild_to: Option<String> => Nullable<Text>,
    description: Option<String> => Nullable<Text>,
    year: Option<String> => Nullable<Text>,
    manufacturer: Option<String> => Nullable<Text>,
    valid: i64 => BigInt,
});

row!(LinkRow {
    owner_id: i64 => BigInt,
    link_kind: String => Text,
    target_name: String => Text,
    snapshot_key: Option<String> => Nullable<Text>,
    source_reference_kind: Option<String> => Nullable<Text>,
    origin: Option<String> => Nullable<Text>,
});

row!(CommentRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    text: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(ReleaseRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    name: String => Text,
    region: String => Text,
    language: Option<String> => Nullable<Text>,
    date: Option<String> => Nullable<Text>,
    is_default: String => Text,
    default_was_present: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(BiosRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    name: String => Text,
    description: String => Text,
    is_default: String => Text,
    default_was_present: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(ArchiveRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    source_order: i64 => BigInt,
    name: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(DeviceRow {
    owner_id: i64 => BigInt,
    row_order: i64 => BigInt,
    name: String => Text,
    snapshot_key: Option<String> => Nullable<Text>,
    source_reference_kind: Option<String> => Nullable<Text>,
    origin: Option<String> => Nullable<Text>,
    valid: i64 => BigInt,
});

pub(super) fn load(
    connection: &mut SqliteConnection,
    group: i64,
    cursor: Option<&LogiqxCursor>,
    bound: i64,
) -> QueryResult<Vec<GameRow>> {
    if let Some(cursor) = cursor {
        let row = sql_query(format!("{} AND sets.set_id = ?", queries::GAME_COLUMNS))
            .bind::<BigInt, _>(group)
            .bind::<BigInt, _>(cursor.owner_id.as_i64())
            .get_result::<GameRow>(connection)
            .optional()?
            .ok_or(LogiqxQueryError::MismatchedOwner(cursor.owner_id.as_i64()))?;
        validate_game(&row)?;
        if row.list_order != cursor.order {
            return Err(LogiqxQueryError::InvalidPositions(row.set_id));
        }
    }
    let mut query = sql_query(queries::games(cursor.is_some()))
        .into_boxed::<diesel::sqlite::Sqlite>()
        .bind::<BigInt, _>(group);
    if let Some(cursor) = cursor {
        query = query
            .bind::<BigInt, _>(cursor.order)
            .bind::<BigInt, _>(cursor.owner_id.as_i64());
    }
    let rows = query.bind::<BigInt, _>(bound).load::<GameRow>(connection)?;
    for row in &rows {
        validate_game(row)?;
    }
    Ok(rows)
}

fn validate_game(row: &GameRow) -> QueryResult<()> {
    if row.set_id <= 0 || row.source_element_kind != "logiqx_game" {
        return Err(LogiqxQueryError::MismatchedOwner(row.set_id));
    }
    if row.native_id != Some(row.set_id) {
        return Err(LogiqxQueryError::MissingNative {
            field: "game",
            owner: row.set_id,
        });
    }
    if row.valid != 1 || row.list_order < 0 {
        return Err(LogiqxQueryError::InvalidPositions(row.set_id));
    }
    location(row.line, row.column, row.set_id)?;
    Ok(())
}

pub(super) fn hydrate(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    rows: Vec<GameRow>,
) -> QueryResult<Vec<LogiqxGame>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids = rows.iter().map(|row| row.set_id).collect::<Vec<_>>();
    let attributes = position_map(connection, "logiqx_game_attribute_positions", None, &ids)?;
    let texts = position_map(connection, "logiqx_game_text_positions", None, &ids)?;
    let links = batch::<LinkRow>(
        connection,
        &queries::owners(
            "native.link_kind, native.target_name, registry.snapshot_key, reported.source_reference_kind, registry.origin",
            "logiqx_set_links AS native LEFT JOIN catalog_relationships AS registry USING(relationship_id) \
         LEFT JOIN reported_catalog_relationships AS reported USING(relationship_id)",
            ids.len(),
            "native.link_kind",
        ),
        &ids,
    )?;
    let mut parents = BTreeMap::<i64, Vec<LinkRow>>::new();
    for link in links {
        if link.snapshot_key.as_deref() != Some(snapshot.as_str())
            || link.origin.as_deref() != Some("source")
            || link.source_reference_kind.as_deref()
                != Some(format!("logiqx_{}", link.link_kind).as_str())
        {
            return Err(LogiqxQueryError::MismatchedOwner(link.owner_id));
        }
        parents.entry(link.owner_id).or_default().push(link);
    }
    let games = hydrate_rows(rows, attributes, texts, parents)?;
    hydrate_children(connection, snapshot, &ids, games)
}

fn hydrate_rows(
    rows: Vec<GameRow>,
    mut attributes: PositionMap,
    mut texts: PositionMap,
    mut parents: BTreeMap<i64, Vec<LinkRow>>,
) -> QueryResult<Vec<LogiqxGame>> {
    let mut games = Vec::with_capacity(rows.len());
    for row in rows {
        let owner = row.set_id;
        let is_bios = row.is_bios.ok_or(LogiqxQueryError::MissingNative {
            field: "isbios",
            owner,
        })?;
        let bios_presence = row
            .is_bios_was_present
            .ok_or(LogiqxQueryError::MissingNative {
                field: "isbios presence",
                owner,
            })?;
        let is_bios_was_present = default_presence(&is_bios, bios_presence, "no", "isbios", owner)?;
        let text_rows = texts.remove(&(owner, 0)).unwrap_or_default();
        positions::validate(
            &text_rows,
            &[
                row.description.is_some(),
                row.year.is_some(),
                row.manufacturer.is_some(),
            ],
            owner,
        )?;
        let game_links = parents.remove(&owner).unwrap_or_default();
        let parent_presence = |kind| game_links.iter().any(|link| link.link_kind == kind);
        let attribute_positions = positions::attributes(
            attributes.remove(&(owner, 0)).unwrap_or_default(),
            &[
                true,
                row.source_file.is_some(),
                is_bios_was_present,
                parent_presence("cloneof"),
                parent_presence("romof"),
                parent_presence("sampleof"),
                row.board.is_some(),
                row.rebuild_to.is_some(),
            ],
            GameAttribute::from_code,
            owner,
        )?;
        let mut game = LogiqxGame {
            id: CatalogSetId::try_from(owner)?,
            name: row.set_name,
            list_order: row.list_order,
            location: location(row.line, row.column, owner)?,
            sourcefile: row.source_file,
            is_bios: yes_no(&is_bios, "isbios", owner)?,
            is_bios_was_present,
            cloneof: None,
            romof: None,
            sampleof: None,
            board: row.board,
            rebuildto: row.rebuild_to,
            description: text_value(row.description, 0, &text_rows, owner)?,
            year: text_value(row.year, 1, &text_rows, owner)?,
            manufacturer: text_value(row.manufacturer, 2, &text_rows, owner)?,
            attribute_positions,
            comments: Vec::new(),
            releases: Vec::new(),
            bios_sets: Vec::new(),
            media: Vec::new(),
            archives: Vec::new(),
            device_references: Vec::new(),
        };
        hydrate_parents(&mut game, game_links)?;
        games.push(game);
    }
    positions::finish(&attributes)?;
    positions::finish(&texts)?;
    if let Some(owner) = parents.keys().next() {
        return Err(LogiqxQueryError::MismatchedOwner(*owner));
    }
    Ok(games)
}

fn hydrate_parents(game: &mut LogiqxGame, links: Vec<LinkRow>) -> QueryResult<()> {
    let owner = game.id.as_i64();
    for link in links {
        let (field, target) = match link.link_kind.as_str() {
            "cloneof" => (GameAttribute::CloneOf, &mut game.cloneof),
            "romof" => (GameAttribute::RomOf, &mut game.romof),
            "sampleof" => (GameAttribute::SampleOf, &mut game.sampleof),
            _ => return Err(invalid("game link kind", owner, link.link_kind)),
        };
        if target.is_some() {
            return Err(LogiqxQueryError::MismatchedOwner(owner));
        }
        let position = game
            .attribute_positions
            .iter()
            .find(|position| position.field == field)
            .copied()
            .ok_or(LogiqxQueryError::InvalidPositions(owner))?;
        *target = Some(LogiqxParentReference {
            target_name: link.target_name,
            position,
        });
    }
    Ok(())
}

fn hydrate_children(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    ids: &[i64],
    mut games: Vec<LogiqxGame>,
) -> QueryResult<Vec<LogiqxGame>> {
    let indexes = games
        .iter()
        .enumerate()
        .map(|(index, game)| (game.id.as_i64(), index))
        .collect::<BTreeMap<_, _>>();
    load_comments(connection, ids, &indexes, &mut games)?;
    load_releases(connection, ids, &indexes, &mut games)?;
    load_bios(connection, ids, &indexes, &mut games)?;
    load_archives(connection, ids, &indexes, &mut games)?;
    load_devices(connection, snapshot, ids, &indexes, &mut games)?;
    super::media::load(connection, snapshot, ids, &indexes, &mut games)?;
    for game in &games {
        positions::child_orders(
            game.description
                .iter()
                .chain(game.year.iter())
                .chain(game.manufacturer.iter())
                .map(|text| text.source_order)
                .chain(game.comments.iter().map(|child| child.source_order))
                .chain(game.releases.iter().map(|child| child.source_order))
                .chain(game.bios_sets.iter().map(|child| child.source_order))
                .chain(game.media.iter().map(|child| child.source_order))
                .chain(game.archives.iter().map(|child| child.source_order)),
            game.id.as_i64(),
        )?;
    }
    Ok(games)
}

fn text_value(
    value: Option<String>,
    code: i64,
    rows: &[PositionRow],
    owner: i64,
) -> QueryResult<Option<LogiqxTextValue>> {
    value
        .map(|value| {
            let position = rows
                .iter()
                .find(|row| row.field_kind == code)
                .ok_or(LogiqxQueryError::InvalidPositions(owner))?;
            Ok(LogiqxTextValue {
                value,
                source_order: position.source_order,
                location: location(position.line, position.column, owner)?,
            })
        })
        .transpose()
}

fn position_map(
    connection: &mut SqliteConnection,
    table: &str,
    child: Option<&str>,
    ids: &[i64],
) -> QueryResult<PositionMap> {
    Ok(positions::group(batch(
        connection,
        &queries::positions(table, child, ids.len()),
        ids,
    )?))
}

pub(super) fn game_mut<'a>(
    games: &'a mut [LogiqxGame],
    indexes: &BTreeMap<i64, usize>,
    owner: i64,
) -> QueryResult<&'a mut LogiqxGame> {
    indexes
        .get(&owner)
        .and_then(|index| games.get_mut(*index))
        .ok_or(LogiqxQueryError::MismatchedOwner(owner))
}

pub(super) fn family_order(
    order: i64,
    count: usize,
    source_order: i64,
    previous: Option<i64>,
    owner: i64,
) -> QueryResult<()> {
    if usize::try_from(order).ok() != Some(count)
        || source_order < 0
        || previous.is_some_and(|previous| source_order <= previous)
    {
        return Err(LogiqxQueryError::InvalidPositions(owner));
    }
    Ok(())
}

fn load_comments(
    connection: &mut SqliteConnection,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<CommentRow>(
        connection,
        &queries::children(
            "native.comment_order AS row_order, native.source_order, native.comment_text AS text, native.source_line AS line, native.source_column AS column",
            "logiqx_game_comments",
            "native.comment_order",
            None,
            ids.len(),
        ),
        ids,
    )?;
    for row in rows {
        positions::require_integer_provenance(row.valid, row.owner_id)?;
        let game = game_mut(games, indexes, row.owner_id)?;
        family_order(
            row.row_order,
            game.comments.len(),
            row.source_order,
            game.comments.last().map(|child| child.source_order),
            row.owner_id,
        )?;
        game.comments.push(LogiqxComment {
            comment_order: row.row_order,
            text: row.text,
            source_order: row.source_order,
            location: location(row.line, row.column, row.owner_id)?,
        });
    }
    Ok(())
}

fn load_releases(
    connection: &mut SqliteConnection,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<ReleaseRow>(
        connection,
        &queries::children(
            "native.release_order AS row_order, native.source_order, native.name, native.region, native.language, native.date, \
         native.\"default\" AS is_default, native.default_was_present, native.source_line AS line, native.source_column AS column",
            "logiqx_releases",
            "native.release_order",
            Some("native.default_was_present"),
            ids.len(),
        ),
        ids,
    )?;
    let mut attributes = position_map(
        connection,
        "logiqx_release_attribute_positions",
        Some("native.release_order"),
        ids,
    )?;
    for row in rows {
        positions::require_integer_provenance(row.valid, row.owner_id)?;
        let game = game_mut(games, indexes, row.owner_id)?;
        family_order(
            row.row_order,
            game.releases.len(),
            row.source_order,
            game.releases.last().map(|child| child.source_order),
            row.owner_id,
        )?;
        let default_was_present = default_presence(
            &row.is_default,
            row.default_was_present,
            "no",
            "release default",
            row.owner_id,
        )?;
        let attribute_positions = positions::attributes(
            attributes
                .remove(&(row.owner_id, row.row_order))
                .unwrap_or_default(),
            &[
                true,
                true,
                row.language.is_some(),
                row.date.is_some(),
                default_was_present,
            ],
            ReleaseAttribute::from_code,
            row.owner_id,
        )?;
        game.releases.push(LogiqxRelease {
            release_order: row.row_order,
            name: row.name,
            region: row.region,
            language: row.language,
            date: row.date,
            is_default: yes_no(&row.is_default, "release default", row.owner_id)?,
            default_was_present,
            source_order: row.source_order,
            location: location(row.line, row.column, row.owner_id)?,
            attribute_positions,
        });
    }
    positions::finish(&attributes)
}

fn load_bios(
    connection: &mut SqliteConnection,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<BiosRow>(
        connection,
        &queries::children(
            "native.bios_order AS row_order, native.source_order, native.name, native.description, native.is_default, \
         native.default_was_present, native.source_line AS line, native.source_column AS column",
            "logiqx_bios_sets",
            "native.bios_order",
            Some("native.default_was_present"),
            ids.len(),
        ),
        ids,
    )?;
    let mut attributes = position_map(
        connection,
        "logiqx_bios_attribute_positions",
        Some("native.bios_order"),
        ids,
    )?;
    for row in rows {
        positions::require_integer_provenance(row.valid, row.owner_id)?;
        let game = game_mut(games, indexes, row.owner_id)?;
        family_order(
            row.row_order,
            game.bios_sets.len(),
            row.source_order,
            game.bios_sets.last().map(|child| child.source_order),
            row.owner_id,
        )?;
        let default_was_present = default_presence(
            &row.is_default,
            row.default_was_present,
            "no",
            "BIOS default",
            row.owner_id,
        )?;
        let attribute_positions = positions::attributes(
            attributes
                .remove(&(row.owner_id, row.row_order))
                .unwrap_or_default(),
            &[true, true, default_was_present],
            BiosSetAttribute::from_code,
            row.owner_id,
        )?;
        game.bios_sets.push(LogiqxBiosSet {
            bios_order: row.row_order,
            name: row.name,
            description: row.description,
            is_default: yes_no(&row.is_default, "BIOS default", row.owner_id)?,
            default_was_present,
            source_order: row.source_order,
            location: location(row.line, row.column, row.owner_id)?,
            attribute_positions,
        });
    }
    positions::finish(&attributes)
}

fn load_archives(
    connection: &mut SqliteConnection,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<ArchiveRow>(
        connection,
        &queries::children(
            "native.archive_order AS row_order, native.source_order, native.archive_name AS name, native.source_line AS line, native.source_column AS column",
            "logiqx_archive_references",
            "native.archive_order",
            None,
            ids.len(),
        ),
        ids,
    )?;
    let mut attributes = position_map(
        connection,
        "logiqx_archive_attribute_positions",
        Some("native.archive_order"),
        ids,
    )?;
    for row in rows {
        positions::require_integer_provenance(row.valid, row.owner_id)?;
        let game = game_mut(games, indexes, row.owner_id)?;
        family_order(
            row.row_order,
            game.archives.len(),
            row.source_order,
            game.archives.last().map(|child| child.source_order),
            row.owner_id,
        )?;
        let attribute_positions = positions::attributes(
            attributes
                .remove(&(row.owner_id, row.row_order))
                .unwrap_or_default(),
            &[true],
            NameAttribute::from_code,
            row.owner_id,
        )?;
        game.archives.push(LogiqxArchiveReference {
            archive_order: row.row_order,
            name: row.name,
            source_order: row.source_order,
            location: location(row.line, row.column, row.owner_id)?,
            attribute_positions,
        });
    }
    positions::finish(&attributes)
}

fn load_devices(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<DeviceRow>(
        connection,
        &queries::owners(
            "native.reference_order AS row_order, native.target_name AS name, registry.snapshot_key, reported.source_reference_kind, registry.origin, typeof(native.reference_order)='integer' AS valid",
            "logiqx_device_references AS native LEFT JOIN catalog_relationships AS registry USING(relationship_id) \
         LEFT JOIN reported_catalog_relationships AS reported USING(relationship_id)",
            ids.len(),
            "native.reference_order",
        ),
        ids,
    )?;
    let mut attributes = position_map(
        connection,
        "logiqx_device_reference_attribute_positions",
        Some("native.reference_order"),
        ids,
    )?;
    for row in rows {
        positions::require_integer_provenance(row.valid, row.owner_id)?;
        if row.snapshot_key.as_deref() != Some(snapshot.as_str())
            || row.source_reference_kind.as_deref() != Some("logiqx_device_ref")
            || row.origin.as_deref() != Some("source")
        {
            return Err(LogiqxQueryError::MismatchedOwner(row.owner_id));
        }
        let game = game_mut(games, indexes, row.owner_id)?;
        if usize::try_from(row.row_order).ok() != Some(game.device_references.len()) {
            return Err(LogiqxQueryError::InvalidPositions(row.owner_id));
        }
        let attribute_positions = positions::attributes(
            attributes
                .remove(&(row.owner_id, row.row_order))
                .unwrap_or_default(),
            &[true],
            NameAttribute::from_code,
            row.owner_id,
        )?;
        game.device_references.push(LogiqxDeviceReference {
            reference_order: row.row_order,
            name: row.name,
            attribute_positions,
        });
    }
    positions::finish(&attributes)
}
