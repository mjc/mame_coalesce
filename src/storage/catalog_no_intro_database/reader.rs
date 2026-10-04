use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    database::Database,
    domain::{CatalogRegistryId, SnapshotKey},
    logiqx::RecordLocation,
    no_intro_db_xml::{
        DatabaseHeader, EnvelopeKind, HeaderField, HeaderFieldKind, NoIntroDatabaseDocument,
        NoIntroDatabaseMode, XmlSourceExtent,
    },
    xml_reader::DeclaredText,
};

use super::positions::{location, require_integer};
use super::{
    NoIntroDatabaseCursor, NoIntroDatabaseGame, NoIntroDatabasePage, NoIntroDatabasePageLimit,
    NoIntroDatabaseQueryError, NoIntroDatabaseSnapshot, queries,
};

type QueryResult<T> = Result<T, NoIntroDatabaseQueryError>;

macro_rules! row {
    ($name:ident { $($field:ident: $rust_ty:ty => $sql_ty:ty),* $(,)? }) => {
        #[derive(QueryableByName)]
        struct $name {
            $(#[diesel(sql_type = $sql_ty)] $field: $rust_ty,)*
        }
    };
}

row!(SnapshotRow {
    snapshot_key: String => Text,
    source_key: String => Text,
    source_name: String => Text,
    catalog_key: String => Text,
    catalog_name: String => Text,
    document_key: String => Text,
    interpretation_key: String => Text,
    format: String => Text,
    declared_version: Option<String> => Nullable<Text>,
    published: i64 => BigInt,
});

row!(ExportRow {
    envelope_kind: String => Text,
    header_present: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
    end_line: i64 => BigInt,
    end_column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(HeaderRow {
    line: i64 => BigInt,
    column: i64 => BigInt,
    end_line: i64 => BigInt,
    end_column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(HeaderFieldRow {
    source_order: i64 => BigInt,
    field_kind: i64 => BigInt,
    value: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
    end_line: i64 => BigInt,
    end_column: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(GroupRow {
    set_group_id: i64 => BigInt,
    kind: String => Text,
    list_order: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(GameRow {
    set_id: i64 => BigInt,
    set_name: String => Text,
    list_order: i64 => BigInt,
    source_element_kind: String => Text,
    line: i64 => BigInt,
    column: i64 => BigInt,
    native_id: Option<i64> => Nullable<BigInt>,
    name_source_order: Option<i64> => Nullable<BigInt>,
    name_source_line: Option<i64> => Nullable<BigInt>,
    name_source_column: Option<i64> => Nullable<BigInt>,
    valid: i64 => BigInt,
});

row!(ParseCountRow {
    game_count: i64 => BigInt,
    header_field_count: i64 => BigInt,
    valid: i64 => BigInt,
});

pub(super) fn games_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&NoIntroDatabaseCursor>,
    limit: NoIntroDatabasePageLimit,
) -> QueryResult<NoIntroDatabasePage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let registry_id = crate::storage::catalog_content::registry_id(connection)?;
        validate_cursor(cursor, registry_id, snapshot)?;
        let snapshot_row = snapshot_row(connection, snapshot, registry_id)?;
        let counts = parse_counts(connection, snapshot)?;
        let document = load_document(connection, snapshot, &snapshot_row.format)?;
        let header_fields = document
            .header
            .as_ref()
            .map_or(0, |header| header.fields.len());
        if i64::try_from(header_fields).ok() != Some(counts.header_field_count) {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
        }
        let groups = sql_query(queries::GROUPS)
            .bind::<Text, _>(snapshot.as_str())
            .load::<GroupRow>(connection)?;
        if groups.len() > 1
            || groups.iter().any(|group| {
                group.valid != 1
                    || group.kind != "root"
                    || group.list_order != 0
                    || group.set_group_id <= 0
            })
        {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(
                groups.first().map_or(0, |group| group.set_group_id),
            ));
        }
        let bound = i64::try_from(limit.0)
            .map_err(|_| NoIntroDatabaseQueryError::PageLimitOverflow)?
            .checked_add(1)
            .ok_or(NoIntroDatabaseQueryError::PageLimitOverflow)?;
        let mut games = match groups.first() {
            Some(group) => load_games(
                connection,
                group.set_group_id,
                cursor,
                bound,
                document.extent,
            )?,
            None if cursor.is_some() => {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(0));
            }
            None => {
                if counts.game_count != 0 {
                    return Err(NoIntroDatabaseQueryError::MismatchedOwner(0));
                }
                Vec::new()
            }
        };
        validate_game_orders(&games, cursor, counts.game_count, bound)?;
        let has_more = games.len() > limit.0;
        games.truncate(limit.0);
        let game_extents = games
            .iter()
            .map(|(game, extent)| (game.id.as_i64(), *extent))
            .collect();
        let games = games.into_iter().map(|(game, _)| game).collect();
        let games = super::children::hydrate(connection, games, &game_extents)?;
        let next_cursor = if has_more {
            games.last().map(|game| NoIntroDatabaseCursor {
                registry_id,
                snapshot_key: snapshot.clone(),
                list_order: game.list_order,
                owner_id: game.id,
            })
        } else {
            None
        };
        Ok(NoIntroDatabasePage {
            snapshot: snapshot_row,
            document,
            games,
            next_cursor,
        })
    })
}

fn parse_counts(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> QueryResult<ParseCountRow> {
    let counts = sql_query("SELECT game_count, header_field_count, typeof(game_count)='integer' AND typeof(header_field_count)='integer' AS valid FROM no_intro_database_parse_counts WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<ParseCountRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatabaseQueryError::MissingDocument(snapshot.clone()))?;
    require_integer(counts.valid, 0)?;
    if counts.game_count < 0 || counts.header_field_count < 0 {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
    }
    Ok(counts)
}

fn validate_game_orders(
    games: &[(NoIntroDatabaseGame, XmlSourceExtent)],
    cursor: Option<&NoIntroDatabaseCursor>,
    total: i64,
    bound: i64,
) -> QueryResult<()> {
    let mut expected = match cursor {
        Some(cursor) if cursor.list_order < total => cursor.list_order.checked_add(1),
        Some(_) => None,
        None => Some(0),
    }
    .ok_or(NoIntroDatabaseQueryError::InvalidMetadata(0))?;
    for (game, _) in games {
        if game.list_order != expected || game.list_order >= total {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(game.id.as_i64()));
        }
        expected = expected
            .checked_add(1)
            .ok_or(NoIntroDatabaseQueryError::InvalidMetadata(game.id.as_i64()))?;
    }
    if i64::try_from(games.len()).ok() != Some(bound) && expected != total {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
    }
    Ok(())
}

fn validate_cursor(
    cursor: Option<&NoIntroDatabaseCursor>,
    registry_id: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.registry_id != registry_id {
            return Err(NoIntroDatabaseQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot_key != snapshot {
            return Err(NoIntroDatabaseQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn snapshot_row(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    registry_id: CatalogRegistryId,
) -> QueryResult<NoIntroDatabaseSnapshot> {
    let row = sql_query(queries::SNAPSHOT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatabaseQueryError::NotPublished(snapshot.clone()))?;
    if !matches!(
        row.format.as_str(),
        "no-intro-database-xml-compatible" | "no-intro-database-xml-nul-compatible"
    ) {
        return Err(NoIntroDatabaseQueryError::NotPublished(snapshot.clone()));
    }
    if row.published != 1 || row.snapshot_key != snapshot.as_str() {
        return Err(NoIntroDatabaseQueryError::NotPublished(snapshot.clone()));
    }
    Ok(NoIntroDatabaseSnapshot {
        registry_id,
        snapshot_key: snapshot.clone(),
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

fn load_document(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    format: &str,
) -> QueryResult<NoIntroDatabaseDocument> {
    let row = sql_query("SELECT envelope_kind, header_present, source_line AS line, source_column AS column, document_end_line AS end_line, document_end_column AS end_column, typeof(header_present)='integer' AND typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(document_end_line)='integer' AND typeof(document_end_column)='integer' AS valid FROM no_intro_exports WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<ExportRow>(connection)
        .optional()?
        .ok_or_else(|| NoIntroDatabaseQueryError::MissingDocument(snapshot.clone()))?;
    require_integer(row.valid, 0)?;
    let header_present = boolean(row.header_present, "header presence", 0)?;
    let envelope = match row.envelope_kind.as_str() {
        "single_datafile" => EnvelopeKind::SingleDatafile,
        "sibling_header_datafile" => EnvelopeKind::SiblingHeaderDatafile,
        _ => return Err(invalid("envelope kind", 0, row.envelope_kind)),
    };
    if envelope == EnvelopeKind::SiblingHeaderDatafile && !header_present {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
    }
    let mode = match format {
        "no-intro-database-xml-compatible" => NoIntroDatabaseMode::ObservedCompatible,
        "no-intro-database-xml-nul-compatible" => NoIntroDatabaseMode::NullRecoveryCompatible,
        _ => return Err(NoIntroDatabaseQueryError::NotPublished(snapshot.clone())),
    };
    let document_extent = checked_extent(
        RecordLocation { line: 1, column: 1 },
        location(row.end_line, row.end_column, 0)?,
        0,
    )?;
    let document_location = location(row.line, row.column, 0)?;
    if !document_extent.contains(document_location) {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(0));
    }
    let header = load_header(connection, snapshot, header_present, document_extent)?;
    if header.is_some() != header_present
        || (envelope == EnvelopeKind::SingleDatafile && header_present && header.is_none())
    {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
    }
    Ok(NoIntroDatabaseDocument {
        envelope,
        mode,
        location: document_location,
        extent: document_extent,
        header,
    })
}

fn load_header(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    expected: bool,
    document_extent: XmlSourceExtent,
) -> QueryResult<Option<DatabaseHeader>> {
    let row = sql_query("SELECT source_line AS line, source_column AS column, source_end_line AS end_line, source_end_column AS end_column, typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(source_end_line)='integer' AND typeof(source_end_column)='integer' AS valid FROM no_intro_export_headers WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<HeaderRow>(connection)
        .optional()?;
    let Some(row) = row else {
        return if expected {
            Err(NoIntroDatabaseQueryError::InvalidMetadata(0))
        } else {
            let fields = header_fields(connection, snapshot, None)?;
            if fields.is_empty() {
                Ok(None)
            } else {
                Err(NoIntroDatabaseQueryError::InvalidMetadata(0))
            }
        };
    };
    if !expected {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(0));
    }
    require_integer(row.valid, 0)?;
    let extent =
        checked_extent_from_columns(row.line, row.column, row.end_line, row.end_column, 0)?;
    if !contains_extent(document_extent, extent) {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(0));
    }
    Ok(Some(DatabaseHeader {
        extent,
        fields: header_fields(connection, snapshot, Some(extent))?,
    }))
}

fn header_fields(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    parent_extent: Option<XmlSourceExtent>,
) -> QueryResult<Vec<HeaderField>> {
    let rows = sql_query("SELECT source_order, field_kind, value, source_line AS line, source_column AS column, source_end_line AS end_line, source_end_column AS end_column, typeof(source_order)='integer' AND typeof(field_kind)='integer' AND typeof(source_line)='integer' AND typeof(source_column)='integer' AND typeof(source_end_line)='integer' AND typeof(source_end_column)='integer' AS valid FROM no_intro_header_fields WHERE snapshot_key=? ORDER BY source_order")
        .bind::<Text, _>(snapshot.as_str())
        .load::<HeaderFieldRow>(connection)?;
    let mut fields = Vec::with_capacity(rows.len());
    let mut previous_end = None;
    for row in rows {
        require_integer(row.valid, 0)?;
        if i64::try_from(fields.len()).ok() != Some(row.source_order) {
            return Err(NoIntroDatabaseQueryError::InvalidPositions(0));
        }
        let kind = match row.field_kind {
            0 => HeaderFieldKind::Author,
            1 => HeaderFieldKind::Piracy,
            2 => HeaderFieldKind::Trademarks,
            3 => HeaderFieldKind::Url,
            4 => HeaderFieldKind::Version,
            _ => return Err(NoIntroDatabaseQueryError::InvalidPositions(0)),
        };
        let source_order = usize::try_from(row.source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidPositions(0))?;
        let extent =
            checked_extent_from_columns(row.line, row.column, row.end_line, row.end_column, 0)?;
        if parent_extent.is_some_and(|parent| !contains_extent(parent, extent)) {
            return Err(NoIntroDatabaseQueryError::InvalidPositions(0));
        }
        advance_extent(&mut previous_end, extent, 0)?;
        fields.push(HeaderField {
            kind,
            value: row.value,
            source_order,
            extent,
        });
    }
    Ok(fields)
}

fn load_games(
    connection: &mut SqliteConnection,
    group_id: i64,
    cursor: Option<&NoIntroDatabaseCursor>,
    bound: i64,
    document_extent: XmlSourceExtent,
) -> QueryResult<Vec<(NoIntroDatabaseGame, XmlSourceExtent)>> {
    let mut previous_end = None;
    if let Some(cursor) = cursor {
        let anchor = sql_query(queries::cursor_anchor())
            .bind::<BigInt, _>(group_id)
            .bind::<BigInt, _>(cursor.owner_id.as_i64())
            .bind::<BigInt, _>(cursor.list_order)
            .get_result::<GameRow>(connection)
            .optional()?
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ))?;
        let anchor_extent = game_extents(connection, &[cursor.owner_id.as_i64()])?
            .into_iter()
            .next()
            .map(|(_, extent)| extent)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ))?;
        if !contains_extent(document_extent, anchor_extent) {
            return Err(NoIntroDatabaseQueryError::InvalidPositions(
                cursor.owner_id.as_i64(),
            ));
        }
        advance_extent(&mut previous_end, anchor_extent, cursor.owner_id.as_i64())?;
        let anchor = game(anchor, anchor_extent)?;
        if (anchor.0.id, anchor.0.list_order) != (cursor.owner_id, cursor.list_order) {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(
                cursor.owner_id.as_i64(),
            ));
        }
    }
    let rows = if let Some(cursor) = cursor {
        sql_query(queries::games(true))
            .bind::<BigInt, _>(group_id)
            .bind::<BigInt, _>(cursor.list_order)
            .bind::<BigInt, _>(cursor.owner_id.as_i64())
            .bind::<BigInt, _>(bound)
            .load::<GameRow>(connection)?
    } else {
        sql_query(queries::games(false))
            .bind::<BigInt, _>(group_id)
            .bind::<BigInt, _>(bound)
            .load::<GameRow>(connection)?
    };
    let ids = rows.iter().map(|row| row.set_id).collect::<Vec<_>>();
    let extents = game_extents(connection, &ids)?;
    let mut extents = extents
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    rows.into_iter()
        .map(|row| {
            let extent = extents
                .remove(&row.set_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.set_id))?;
            if !contains_extent(document_extent, extent) {
                return Err(NoIntroDatabaseQueryError::InvalidPositions(row.set_id));
            }
            advance_extent(&mut previous_end, extent, row.set_id)?;
            game(row, extent)
        })
        .collect()
}

fn game(
    row: GameRow,
    extent: XmlSourceExtent,
) -> QueryResult<(NoIntroDatabaseGame, XmlSourceExtent)> {
    require_integer(row.valid, row.set_id)?;
    if row.source_element_kind != "no_intro_database_game"
        || row.native_id != Some(row.set_id)
        || row.list_order < 0
    {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.set_id));
    }
    let name_source_order = row
        .name_source_order
        .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.set_id))?;
    let name_source_line = row
        .name_source_line
        .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.set_id))?;
    let name_source_column = row
        .name_source_column
        .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.set_id))?;
    if name_source_order < 0 {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.set_id));
    }
    let id = crate::domain::CatalogSetId::try_from(row.set_id)?;
    let record_location = location(row.line, row.column, row.set_id)?;
    let name = DeclaredText {
        value: row.set_name,
        source_order: usize::try_from(name_source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.set_id))?,
        location: location(name_source_line, name_source_column, row.set_id)?,
    };
    if extent.start() != record_location {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(row.set_id));
    }
    Ok((
        NoIntroDatabaseGame {
            id,
            list_order: row.list_order,
            location: record_location,
            name,
            children: Vec::new(),
        },
        extent,
    ))
}

fn game_extents(
    connection: &mut SqliteConnection,
    ids: &[i64],
) -> QueryResult<Vec<(i64, XmlSourceExtent)>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let values = std::iter::repeat_n("(?)", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "WITH requested(set_id) AS (VALUES {values}) \
         SELECT game.set_id AS owner_id, \
                CASE WHEN typeof(sets.source_line)='integer' THEN sets.source_line ELSE 0 END AS line, \
                CASE WHEN typeof(sets.source_column)='integer' THEN sets.source_column ELSE 0 END AS column, \
                CASE WHEN typeof(game.source_end_line)='integer' THEN game.source_end_line ELSE 0 END AS end_line, \
                CASE WHEN typeof(game.source_end_column)='integer' THEN game.source_end_column ELSE 0 END AS end_column, \
                typeof(game.set_id)='integer' AND typeof(sets.set_id)='integer' \
                AND typeof(sets.source_line)='integer' AND typeof(sets.source_column)='integer' \
                AND typeof(game.source_end_line)='integer' AND typeof(game.source_end_column)='integer' AS valid \
         FROM requested CROSS JOIN no_intro_database_games AS game \
         CROSS JOIN catalog_sets AS sets \
         WHERE game.set_id=requested.set_id AND sets.set_id=game.set_id ORDER BY game.set_id"
    );
    let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
    for id in ids {
        query = query.bind::<BigInt, _>(*id);
    }
    let rows = query.load::<ExtentRow>(connection)?;
    let mut result = std::collections::BTreeMap::new();
    for row in rows {
        require_integer(row.valid, row.owner_id)?;
        let extent = checked_extent_from_columns(
            row.line,
            row.column,
            row.end_line,
            row.end_column,
            row.owner_id,
        )?;
        if result.insert(row.owner_id, extent).is_some() {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
        }
    }
    if result.len() != ids.len() {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(0));
    }
    Ok(result.into_iter().collect())
}

row!(ExtentRow {
    owner_id: i64 => BigInt,
    line: i64 => BigInt,
    column: i64 => BigInt,
    end_line: i64 => BigInt,
    end_column: i64 => BigInt,
    valid: i64 => BigInt,
});

fn checked_extent_from_columns(
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
    owner: i64,
) -> QueryResult<XmlSourceExtent> {
    checked_extent(
        location(line, column, owner)?,
        location(end_line, end_column, owner)?,
        owner,
    )
}

pub(super) fn checked_extent(
    start: RecordLocation,
    end: RecordLocation,
    owner: i64,
) -> QueryResult<XmlSourceExtent> {
    XmlSourceExtent::new(start, end).ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner))
}

pub(super) fn advance_extent(
    previous_end: &mut Option<RecordLocation>,
    extent: XmlSourceExtent,
    owner: i64,
) -> QueryResult<()> {
    let start = extent.start();
    if previous_end.is_some_and(|end| (end.line, end.column) > (start.line, start.column)) {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(owner));
    }
    *previous_end = Some(extent.end());
    Ok(())
}

pub(super) const fn contains_extent(parent: XmlSourceExtent, child: XmlSourceExtent) -> bool {
    let parent_end = parent.end();
    let child_end = child.end();
    parent.contains(child.start())
        && (child_end.line < parent_end.line
            || (child_end.line == parent_end.line && child_end.column <= parent_end.column))
}

fn boolean(value: i64, field: &'static str, owner: i64) -> QueryResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid(field, owner, value.to_string())),
    }
}

const fn invalid(field: &'static str, owner: i64, value: String) -> NoIntroDatabaseQueryError {
    NoIntroDatabaseQueryError::InvalidStoredValue {
        field,
        owner,
        value,
    }
}
