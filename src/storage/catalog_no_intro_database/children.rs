use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    domain::NoIntroArchiveId,
    logiqx::RecordLocation,
    no_intro_db_xml::{
        ArchiveClone, ArchiveDescription, ReleaseSerials, SourceDetails, SourceSerials,
        XmlSourceExtent,
    },
    storage::catalog_files::{NoIntroDumpSourceId, NoIntroReleaseId},
};

use super::positions::{location, require_integer};
use super::reader::{advance_extent, checked_extent, contains_extent};
use super::{
    NoIntroDatabaseArchive, NoIntroDatabaseArchiveField, NoIntroDatabaseDumpDetailsField,
    NoIntroDatabaseDumpFileField, NoIntroDatabaseDumpSerialsField, NoIntroDatabaseDumpSource,
    NoIntroDatabaseFileReference, NoIntroDatabaseGame, NoIntroDatabaseGameChild,
    NoIntroDatabaseQueryError, NoIntroDatabaseRelease, NoIntroDatabaseReleaseDetailsField,
    NoIntroDatabaseReleaseFileField, NoIntroDatabaseReleaseSerialsField, positions,
};

type QueryResult<T> = Result<T, NoIntroDatabaseQueryError>;

#[derive(QueryableByName)]
struct NativeExtentRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    parent_id: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    end_line: i64,
    #[diesel(sql_type = BigInt)]
    end_column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

fn native_extents(
    connection: &mut SqliteConnection,
    table: &'static str,
    owner_column: &'static str,
    parent_column: &'static str,
    parent_ids: &[i64],
) -> QueryResult<BTreeMap<i64, (i64, XmlSourceExtent)>> {
    let mut result = BTreeMap::new();
    for batch in parent_ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let values = std::iter::repeat_n("(?)", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "WITH requested(parent_id) AS (VALUES {values}) \
             SELECT native.{owner_column} AS owner_id, native.{parent_column} AS parent_id, \
                    CASE WHEN typeof(native.source_line)='integer' THEN native.source_line ELSE 0 END AS line, \
                    CASE WHEN typeof(native.source_column)='integer' THEN native.source_column ELSE 0 END AS column, \
                    CASE WHEN typeof(native.source_end_line)='integer' THEN native.source_end_line ELSE 0 END AS end_line, \
                    CASE WHEN typeof(native.source_end_column)='integer' THEN native.source_end_column ELSE 0 END AS end_column, \
                    typeof(native.{owner_column})='integer' AND typeof(native.{parent_column})='integer' \
                    AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' \
                    AND typeof(native.source_end_line)='integer' AND typeof(native.source_end_column)='integer' AS valid \
             FROM requested CROSS JOIN {table} AS native \
             WHERE native.{parent_column}=requested.parent_id ORDER BY native.{owner_column}"
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<NativeExtentRow>(connection)? {
            require_integer(row.valid, row.owner_id)?;
            let extent = checked_extent(
                location(row.line, row.column, row.owner_id)?,
                location(row.end_line, row.end_column, row.owner_id)?,
                row.owner_id,
            )?;
            if result
                .insert(row.owner_id, (row.parent_id, extent))
                .is_some()
            {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
            }
        }
    }
    Ok(result)
}

fn validate_parent_extents(
    children: &BTreeMap<i64, (i64, XmlSourceExtent)>,
    parents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<()> {
    for (owner, (parent_id, extent)) in children {
        let parent = parents
            .get(parent_id)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(*owner))?;
        if !contains_extent(*parent, *extent) {
            return Err(NoIntroDatabaseQueryError::InvalidPositions(*owner));
        }
    }
    Ok(())
}

fn take_child_extent(
    extents: &mut BTreeMap<i64, (i64, XmlSourceExtent)>,
    owner: i64,
    start: RecordLocation,
    opening_end: Option<RecordLocation>,
) -> QueryResult<XmlSourceExtent> {
    let (_, extent) = extents
        .remove(&owner)
        .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(owner))?;
    if extent.start() != start
        || opening_end.is_some_and(|point| !extent_contains_point_inclusive_end(extent, point))
    {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(owner));
    }
    Ok(extent)
}

fn extent_contains_point_inclusive_end(extent: XmlSourceExtent, point: RecordLocation) -> bool {
    let start = extent.start();
    let end = extent.end();
    point.line > 0
        && point.column > 0
        && (point.line, point.column) > (start.line, start.column)
        && (point.line, point.column) <= (end.line, end.column)
}

fn ensure_consumed_extents(extents: &BTreeMap<i64, (i64, XmlSourceExtent)>) -> QueryResult<()> {
    if let Some(owner) = extents.keys().next() {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(*owner));
    }
    Ok(())
}

// Every declaration uses the same checked presence/position conversion. The
// explicit field-to-enum pairs remain visible beside each native row shape.
macro_rules! declared_record {
    ($model:ident, $row:ident, $positions:ident, $owner:expr, $field_type:ident;
     $($field:ident => $kind:ident),+;
     $($extra:ident: $value:expr),+ $(,)?) => {
        $model {
            $($field: declared($row.$field, $positions, $field_type::$kind, $owner)?,)+
            $($extra: $value,)+
        }
    };
}

#[derive(QueryableByName)]
struct ChildRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = Text)]
    child_kind: String,
    #[diesel(sql_type = BigInt)]
    child_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    end_line: i64,
    #[diesel(sql_type = BigInt)]
    end_column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ArchiveRow {
    #[diesel(sql_type = BigInt)]
    archive_id: i64,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    additional: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    adult: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    aftermarket: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    bios: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    categories: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    complete: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dat: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    datter_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    devstatus: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    gameid1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    gameid2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    langchecked: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    languages: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    licensed: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    listed: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mergename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name_alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    number: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    physical: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    regparent: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    showlang: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    special1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    special2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sticky_note: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    clone_marker: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    clone_target: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    clone_relationship_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mergeof: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    merge_relationship_id: Option<i64>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
    #[diesel(sql_type = BigInt)]
    registry_valid: i64,
}

#[derive(QueryableByName)]
struct DumpDetailsRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    opening_end_line: i64,
    #[diesel(sql_type = BigInt)]
    opening_end_column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    comment1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    d_date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    d_date_info: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dumper: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    link1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    link2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    link3: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    media_title: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    nodump: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    origin: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    originalformat: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    project: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    r_date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    r_date_info: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rominfo: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    section: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    tool: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct DumpSerialsRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    box_barcode: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    box_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    chip_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    digital_serial1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    digital_serial2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    lockout_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    media_serial1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    media_serial2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    media_serial3: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mediastamp: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pcb_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    romchip_serial1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    romchip_serial2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    savechip_serial: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ReleaseDetailsRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    opening_end_line: i64,
    #[diesel(sql_type = BigInt)]
    opening_end_column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    archivename: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    category: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dirname: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    group_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    nfo_size: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    nfoname: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    nfosize: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    origin: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    originalformat: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rominfo: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    tool: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ReleaseSerialsRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    box_barcode: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    box_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    media_serial1: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mediastamp: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pcb_serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    romchip_serial1: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct NfoHashRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = Text)]
    source_hash_field: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    hash_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    algorithm: Option<String>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    digest: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    presence: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    scope: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    invalid_literal: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct FileRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    expected_set_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    actual_set_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    occurrence_set_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    occurrence_order: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Text)]
    presence: String,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct SelectedFileOwnerRow {
    #[diesel(sql_type = BigInt)]
    game_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

pub(super) fn validate_selected_file_owners(
    connection: &mut SqliteConnection,
    game_ids: &[i64],
) -> QueryResult<()> {
    for batch in game_ids.chunks(400) {
        let mut query = sql_query(super::queries::selected_file_owners(batch.len()))
            .into_boxed::<diesel::sqlite::Sqlite>();
        for game_id in batch {
            query = query.bind::<BigInt, _>(*game_id);
        }
        for row in query.load::<SelectedFileOwnerRow>(connection)? {
            if row.valid != 1 || !batch.contains(&row.game_id) {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(
                    row.occurrence_id,
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn hydrate(
    connection: &mut SqliteConnection,
    mut games: Vec<NoIntroDatabaseGame>,
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<Vec<NoIntroDatabaseGame>> {
    let indexes = games
        .iter()
        .enumerate()
        .map(|(index, game)| (game.id.as_i64(), index))
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::new();
    for ids in indexes.keys().copied().collect::<Vec<_>>().chunks(400) {
        validate_selected_file_owners(connection, ids)?;
        rows.extend(load_child_rows(connection, ids)?);
    }
    validate_sibling_rows(&rows)?;
    for row in &rows {
        if !indexes.contains_key(&row.game_id) {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id));
        }
    }
    let archive_ids = rows
        .iter()
        .filter(|row| row.child_kind == "archive")
        .map(|row| row.child_id)
        .collect::<Vec<_>>();
    let mut archives = load_archives(connection, &archive_ids, game_extents)?;
    let dump_ids = rows
        .iter()
        .filter(|row| row.child_kind == "dump_source")
        .map(|row| row.child_id)
        .collect::<Vec<_>>();
    let mut dump_sources = load_dump_sources(connection, &dump_ids, game_extents)?;
    let release_ids = rows
        .iter()
        .filter(|row| row.child_kind == "release")
        .map(|row| row.child_id)
        .collect::<Vec<_>>();
    let mut releases = load_releases(connection, &release_ids, game_extents)?;
    let mut file_orders = BTreeMap::new();
    for row in rows {
        let game_index = *indexes
            .get(&row.game_id)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id))?;
        let child = match row.child_kind.as_str() {
            "archive" => NoIntroDatabaseGameChild::Archive(
                archives
                    .remove(&row.child_id)
                    .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id))?,
            ),
            "dump_source" => NoIntroDatabaseGameChild::DumpSource(
                dump_sources
                    .remove(&row.child_id)
                    .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id))?,
            ),
            "release" => NoIntroDatabaseGameChild::Release(
                releases
                    .remove(&row.child_id)
                    .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id))?,
            ),
            _ => return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.child_id)),
        };
        let child_location = match &child {
            NoIntroDatabaseGameChild::Archive(archive) => archive.location,
            NoIntroDatabaseGameChild::DumpSource(source) => source.location,
            NoIntroDatabaseGameChild::Release(release) => release.location,
        };
        if child.source_order() != row.source_order
            || child_location != location(row.line, row.column, row.child_id)?
        {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.child_id));
        }
        child_files(&child, row.game_id, &mut file_orders)?;
        games[game_index].children.push(child);
    }
    Ok(games)
}

fn child_files(
    child: &NoIntroDatabaseGameChild,
    game: i64,
    orders: &mut BTreeMap<i64, i64>,
) -> QueryResult<()> {
    match child {
        NoIntroDatabaseGameChild::Archive(_) => Ok(()),
        NoIntroDatabaseGameChild::DumpSource(source) => {
            let owner = source.id.as_i64();
            history_orders(
                owner,
                source
                    .details
                    .as_ref()
                    .map(|value| stored_order(value.source_order, owner))
                    .transpose()?,
                source
                    .serials
                    .as_ref()
                    .map(|value| stored_order(value.source_order, owner))
                    .transpose()?,
                &source.files,
            )?;
            file_orders(&source.files, game, orders)
        }
        NoIntroDatabaseGameChild::Release(release) => {
            let owner = release.id.as_i64();
            history_orders(
                owner,
                release.details.as_ref().map(|value| value.source_order),
                release
                    .serials
                    .as_ref()
                    .map(|value| stored_order(value.source_order, owner))
                    .transpose()?,
                &release.files,
            )?;
            file_orders(&release.files, game, orders)
        }
    }
}

fn stored_order(value: usize, owner: i64) -> QueryResult<i64> {
    i64::try_from(value).map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(owner))
}

fn advance_order(actual: i64, expected: &mut i64, owner: i64) -> QueryResult<()> {
    if actual != *expected {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(owner));
    }
    *expected = expected
        .checked_add(1)
        .ok_or(NoIntroDatabaseQueryError::InvalidMetadata(owner))?;
    Ok(())
}

fn file_orders<Field>(
    files: &[NoIntroDatabaseFileReference<Field>],
    game: i64,
    orders: &mut BTreeMap<i64, i64>,
) -> QueryResult<()> {
    let expected = orders.entry(game).or_insert(0);
    for file in files {
        advance_order(
            file.occurrence_order,
            expected,
            file.occurrence_id.database_value(),
        )?;
    }
    Ok(())
}

fn history_orders<Field>(
    owner: i64,
    details: Option<i64>,
    serials: Option<i64>,
    files: &[NoIntroDatabaseFileReference<Field>],
) -> QueryResult<()> {
    let scalars = [details, serials];
    let total = files
        .len()
        .checked_add(scalars.iter().flatten().count())
        .and_then(|count| i64::try_from(count).ok())
        .ok_or(NoIntroDatabaseQueryError::InvalidMetadata(owner))?;
    if details.is_some() && details == serials
        || scalars
            .into_iter()
            .flatten()
            .any(|order| order < 0 || order >= total)
    {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(owner));
    }
    let mut previous = None;
    for file in files {
        let order = file.source_order;
        if order < 0
            || order >= total
            || previous.is_some_and(|value| value >= order)
            || scalars.contains(&Some(order))
        {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(owner));
        }
        previous = Some(order);
    }
    Ok(())
}

fn load_archives(
    connection: &mut SqliteConnection,
    ids: &[i64],
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, NoIntroDatabaseArchive>> {
    let mut extents = native_extents(
        connection,
        "no_intro_archive_descriptions",
        "archive_id",
        "set_id",
        &game_extents.keys().copied().collect::<Vec<_>>(),
    )?;
    validate_parent_extents(&extents, game_extents)?;
    let mut result = BTreeMap::new();
    for batch in ids.chunks(400) {
        let mut query = sql_query(super::queries::archive_rows(batch.len()))
            .into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        let rows = query.load::<ArchiveRow>(connection)?;
        let owner_ids = rows.iter().map(|row| row.archive_id).collect::<Vec<_>>();
        let mut positions = positions::load(
            connection,
            "no_intro_archive_field_positions",
            "archive_id",
            &owner_ids,
            NoIntroDatabaseArchiveField::from_code,
        )?;
        for row in rows {
            if row.registry_valid != 1 {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.archive_id));
            }
            let position = positions
                .remove(&row.archive_id)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(row.archive_id))?;
            let archive_id = row.archive_id;
            let (_, extent) = extents
                .remove(&archive_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(archive_id))?;
            let record = archive(row, position, extent)?;
            if result.insert(archive_id, record).is_some() {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(archive_id));
            }
        }
    }
    Ok(result)
}

const fn archive_presence(row: &ArchiveRow) -> [bool; 32] {
    [
        row.additional.is_some(),
        row.adult.is_some(),
        row.aftermarket.is_some(),
        row.alt.is_some(),
        row.bios.is_some(),
        row.categories.is_some(),
        row.complete.is_some(),
        row.dat.is_some(),
        row.datter_note.is_some(),
        row.description.is_some(),
        row.devstatus.is_some(),
        row.gameid1.is_some(),
        row.gameid2.is_some(),
        row.langchecked.is_some(),
        row.languages.is_some(),
        row.licensed.is_some(),
        row.listed.is_some(),
        row.mergename.is_some(),
        row.name.is_some(),
        row.name_alt.is_some(),
        row.number.is_some(),
        row.physical.is_some(),
        row.region.is_some(),
        row.regparent.is_some(),
        row.showlang.is_some(),
        row.special1.is_some(),
        row.special2.is_some(),
        row.sticky_note.is_some(),
        row.version1.is_some(),
        row.version2.is_some(),
        row.clone_marker.is_some() || row.clone_target.is_some(),
        row.mergeof.is_some(),
    ]
}

fn archive_clone(
    marker: Option<String>,
    target: Option<String>,
    position: &positions::OwnerPositions<NoIntroDatabaseArchiveField>,
    owner: i64,
) -> QueryResult<Option<ArchiveClone>> {
    Ok(match (marker, target) {
        (Some(marker), None) if marker == "P" => Some(ArchiveClone::ParentMarker(
            declared(
                Some(marker),
                position,
                NoIntroDatabaseArchiveField::Clone,
                owner,
            )?
            .ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner))?,
        )),
        (None, Some(value)) if value != "P" => Some(ArchiveClone::OtherValue(
            declared(
                Some(value),
                position,
                NoIntroDatabaseArchiveField::Clone,
                owner,
            )?
            .ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner))?,
        )),
        (None, None) => None,
        _ => return Err(NoIntroDatabaseQueryError::InvalidMetadata(owner)),
    })
}

fn archive(
    row: ArchiveRow,
    positions: positions::OwnerPositions<NoIntroDatabaseArchiveField>,
    extent: XmlSourceExtent,
) -> QueryResult<NoIntroDatabaseArchive> {
    let position = &positions;
    if row.set_id <= 0 {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.archive_id));
    }
    require_integer(row.valid, row.archive_id)?;
    if row.source_order < 0 {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.archive_id));
    }
    if row.clone_marker.is_some() && row.clone_target.is_some()
        || row.clone_relationship_id.is_some() != row.clone_target.is_some()
        || row.merge_relationship_id.is_some() != row.mergeof.is_some()
    {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.archive_id));
    }
    let expected = archive_presence(&row);
    positions::validate_presence(position, &expected, row.archive_id)?;
    let clone = archive_clone(row.clone_marker, row.clone_target, position, row.archive_id)?;
    let location = location(row.line, row.column, row.archive_id)?;
    if extent.start() != location {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(row.archive_id));
    }
    let description = declared_record!(ArchiveDescription, row, position, row.archive_id, NoIntroDatabaseArchiveField;
        additional => Additional,
        adult => Adult,
        aftermarket => Aftermarket,
        alt => Alt,
        bios => Bios,
        categories => Categories,
        complete => Complete,
        dat => Dat,
        datter_note => DatterNote,
        description => Description,
        devstatus => Devstatus,
        gameid1 => GameId1,
        gameid2 => GameId2,
        langchecked => Langchecked,
        languages => Languages,
        licensed => Licensed,
        listed => Listed,
        mergename => Mergename,
        name => Name,
        name_alt => NameAlt,
        number => Number,
        physical => Physical,
        region => Region,
        regparent => Regparent,
        showlang => Showlang,
        special1 => Special1,
        special2 => Special2,
        sticky_note => StickyNote,
        version1 => Version1,
        version2 => Version2,
        mergeof => MergeOf;
        source_order: usize::try_from(row.source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.archive_id))?,
        extent: extent,
        clone: clone
    );
    Ok(NoIntroDatabaseArchive {
        id: NoIntroArchiveId::try_from(row.archive_id)?,
        source_order: row.source_order,
        location,
        description,
        clone_relationship_id: row.clone_relationship_id,
        merge_relationship_id: row.merge_relationship_id,
        attribute_positions: positions.attributes,
    })
}

fn load_dump_sources(
    connection: &mut SqliteConnection,
    ids: &[i64],
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, NoIntroDatabaseDumpSource>> {
    validate_history_siblings(connection, ids, super::queries::SiblingParent::DumpSource)?;
    let base_rows = load_dump_source_rows(connection, ids, game_extents)?;
    let parent_extents = base_rows
        .iter()
        .map(|(id, base)| (*id, base.extent))
        .collect::<BTreeMap<_, _>>();
    let mut details = load_dump_details(connection, ids, &parent_extents)?;
    let mut serials = load_dump_serials(connection, ids, &parent_extents)?;
    let mut files = load_file_refs(
        connection,
        ids,
        super::queries::FileOwner::DumpSource,
        NoIntroDatabaseDumpFileField::from_code,
        &parent_extents,
    )?;
    let mut result = BTreeMap::new();
    for id in ids {
        let base = base_rows
            .get(id)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(*id))?;
        let (detail, details_attribute_positions) = details.remove(id).map_or_else(
            || (None, Vec::new()),
            |(value, positions)| (Some(value), positions.attributes),
        );
        let (serial, serials_attribute_positions) = serials.remove(id).map_or_else(
            || (None, Vec::new()),
            |(value, positions)| (Some(value), positions.attributes),
        );
        let source = NoIntroDatabaseDumpSource {
            id: NoIntroDumpSourceId::try_from(*id)?,
            source_order: base.source_order,
            location: base.location,
            details: detail,
            serials: serial,
            details_attribute_positions,
            serials_attribute_positions,
            files: files.remove(id).unwrap_or_default(),
        };
        result.insert(*id, source);
    }
    Ok(result)
}

#[derive(QueryableByName)]
struct OwnerRow {
    #[diesel(sql_type = BigInt)]
    owner_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

struct OwnerBase {
    source_order: i64,
    location: RecordLocation,
    extent: XmlSourceExtent,
}

fn load_dump_source_rows(
    connection: &mut SqliteConnection,
    ids: &[i64],
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, OwnerBase>> {
    let mut result = BTreeMap::new();
    let mut extents = native_extents(
        connection,
        "no_intro_dump_sources",
        "dump_source_id",
        "set_id",
        &game_extents.keys().copied().collect::<Vec<_>>(),
    )?;
    validate_parent_extents(&extents, game_extents)?;
    for batch in ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let sql = super::queries::owners(
            "native.source_order, native.source_line AS line, native.source_column AS column, \
             typeof(native.dump_source_id)='integer' AND typeof(native.set_id)='integer' \
             AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
             AND typeof(native.source_column)='integer' AS valid",
            "no_intro_dump_sources",
            "dump_source_id",
            batch.len(),
            "native.source_order",
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<OwnerRow>(connection)? {
            require_integer(row.valid, row.owner_id)?;
            let (_, extent) = extents
                .remove(&row.owner_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id))?;
            let record_location = location(row.line, row.column, row.owner_id)?;
            if extent.start() != record_location {
                return Err(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id));
            }
            if row.source_order < 0
                || result
                    .insert(
                        row.owner_id,
                        OwnerBase {
                            source_order: row.source_order,
                            location: record_location,
                            extent,
                        },
                    )
                    .is_some()
            {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
            }
        }
    }
    if let Some(id) = ids.iter().find(|id| !result.contains_key(id)) {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(*id));
    }
    Ok(result)
}

fn load_file_refs<Field: Copy>(
    connection: &mut SqliteConnection,
    owner_ids: &[i64],
    owner: super::queries::FileOwner,
    decode: fn(i64) -> Option<Field>,
    parent_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, Vec<NoIntroDatabaseFileReference<Field>>>> {
    let shape = owner.shape();
    let mut extents = native_extents(
        connection,
        shape.table,
        "occurrence_id",
        shape.owner_column,
        owner_ids,
    )?;
    validate_parent_extents(&extents, parent_extents)?;
    let mut result = owner_ids
        .iter()
        .copied()
        .map(|id| (id, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for batch in owner_ids.chunks(350) {
        if batch.is_empty() {
            continue;
        }
        let sql = super::queries::file_rows(owner, batch.len());
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        let rows = query.load::<FileRow>(connection)?;
        let occurrence_ids = rows.iter().map(|row| row.occurrence_id).collect::<Vec<_>>();
        let position_map = positions::load(
            connection,
            shape.position_table,
            "occurrence_id",
            &occurrence_ids,
            decode,
        )?;
        let mut local_orders = BTreeSet::new();
        for row in rows {
            require_integer(row.valid, row.occurrence_id)?;
            if row.source_order < 0
                || row.actual_set_id.is_none()
                || row.actual_set_id != Some(row.expected_set_id)
                || row.actual_set_id != row.occurrence_set_id
                || row.occurrence_order.is_none_or(|order| order < 0)
                || !local_orders.insert((row.owner_id, row.source_order))
            {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(
                    row.occurrence_id,
                ));
            }
            let positions = position_map.get(&row.occurrence_id).ok_or(
                NoIntroDatabaseQueryError::InvalidPositions(row.occurrence_id),
            )?;
            if row.presence.len() != shape.fields.len()
                || !row
                    .presence
                    .bytes()
                    .all(|byte| byte == b'0' || byte == b'1')
            {
                return Err(NoIntroDatabaseQueryError::InvalidPositions(
                    row.occurrence_id,
                ));
            }
            let expected = row
                .presence
                .bytes()
                .map(|byte| byte == b'1')
                .collect::<Vec<_>>();
            positions::validate_presence(positions, &expected, row.occurrence_id)?;
            let location = positions::location(row.line, row.column, row.occurrence_id)?;
            let (_, extent) = extents.remove(&row.occurrence_id).ok_or(
                NoIntroDatabaseQueryError::MismatchedOwner(row.occurrence_id),
            )?;
            if extent.start() != location {
                return Err(NoIntroDatabaseQueryError::InvalidPositions(
                    row.occurrence_id,
                ));
            }
            let occurrence_id = crate::domain::OccurrenceId::try_from(row.occurrence_id)?;
            let reference = NoIntroDatabaseFileReference {
                occurrence_id,
                occurrence_order: row.occurrence_order.ok_or(
                    NoIntroDatabaseQueryError::MismatchedOwner(row.occurrence_id),
                )?,
                source_order: row.source_order,
                location,
                attribute_positions: positions.attributes.clone(),
            };
            result
                .get_mut(&row.owner_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id))?
                .push(reference);
        }
    }
    for references in result.values_mut() {
        references.sort_by_key(|reference| reference.source_order);
    }
    ensure_consumed_extents(&extents)?;
    Ok(result)
}

fn load_dump_details(
    connection: &mut SqliteConnection,
    ids: &[i64],
    parent_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<
    BTreeMap<
        i64,
        (
            SourceDetails,
            positions::OwnerPositions<NoIntroDatabaseDumpDetailsField>,
        ),
    >,
> {
    let mut extents = native_extents(
        connection,
        "no_intro_dump_details",
        "dump_source_id",
        "dump_source_id",
        ids,
    )?;
    validate_parent_extents(&extents, parent_extents)?;
    let mut rows = Vec::new();
    for batch in ids.chunks(400) {
        let select = "native.source_order, native.source_line AS line, native.source_column AS column, \
            native.opening_end_line, native.opening_end_column, native.comment1, native.comment2, \
            native.d_date, native.d_date_info, native.dumper, native.id, native.link1, native.link2, \
            native.link3, native.media_title, native.nodump, native.origin, native.originalformat, \
            native.project, native.r_date, native.r_date_info, native.region, native.rominfo, \
            native.section, native.tool, \
            typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
            AND typeof(native.source_column)='integer' AND typeof(native.opening_end_line)='integer' \
            AND typeof(native.opening_end_column)='integer' AS valid";
        let sql = super::queries::owners(
            select,
            "no_intro_dump_details",
            "dump_source_id",
            batch.len(),
            "native.source_order",
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        rows.extend(query.load::<DumpDetailsRow>(connection)?);
    }
    let positions_map = positions::load(
        connection,
        "no_intro_dump_details_field_positions",
        "dump_source_id",
        ids,
        NoIntroDatabaseDumpDetailsField::from_code,
    )?;
    let mut result = BTreeMap::new();
    for row in rows {
        require_integer(row.valid, row.owner_id)?;
        let source_order = usize::try_from(row.source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id))?;
        let attribute_positions = positions_map
            .get(&row.owner_id)
            .ok_or(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id))?;
        let expected = dump_details_presence(&row);
        positions::validate_presence(attribute_positions, &expected, row.owner_id)?;
        let source_location = location(row.line, row.column, row.owner_id)?;
        let opening_end = location(row.opening_end_line, row.opening_end_column, row.owner_id)?;
        let extent = take_child_extent(
            &mut extents,
            row.owner_id,
            source_location,
            Some(opening_end),
        )?;
        let details = declared_record!(SourceDetails, row, attribute_positions, row.owner_id, NoIntroDatabaseDumpDetailsField;
            comment1 => Comment1,
            comment2 => Comment2,
            d_date => DumpDate,
            d_date_info => DumpDateInfo,
            dumper => Dumper,
            id => Id,
            link1 => Link1,
            link2 => Link2,
            link3 => Link3,
            media_title => MediaTitle,
            nodump => NoDump,
            origin => Origin,
            originalformat => OriginalFormat,
            project => Project,
            r_date => ReleaseDate,
            r_date_info => ReleaseDateInfo,
            region => Region,
            rominfo => RomInfo,
            section => Section,
            tool => Tool;
            source_order: source_order,
            extent: extent,
            opening_end: opening_end
        );
        if result
            .insert(row.owner_id, (details, attribute_positions.clone()))
            .is_some()
        {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
        }
    }
    for id in ids {
        if !result.contains_key(id) {
            let positions = positions_map
                .get(id)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(*id))?;
            positions::validate_presence(positions, &[], *id)?;
        }
    }
    ensure_consumed_extents(&extents)?;
    Ok(result)
}

const fn dump_details_presence(row: &DumpDetailsRow) -> [bool; 20] {
    [
        row.comment1.is_some(),
        row.comment2.is_some(),
        row.d_date.is_some(),
        row.d_date_info.is_some(),
        row.dumper.is_some(),
        row.id.is_some(),
        row.link1.is_some(),
        row.link2.is_some(),
        row.link3.is_some(),
        row.media_title.is_some(),
        row.nodump.is_some(),
        row.origin.is_some(),
        row.originalformat.is_some(),
        row.project.is_some(),
        row.r_date.is_some(),
        row.r_date_info.is_some(),
        row.region.is_some(),
        row.rominfo.is_some(),
        row.section.is_some(),
        row.tool.is_some(),
    ]
}

fn load_dump_serials(
    connection: &mut SqliteConnection,
    ids: &[i64],
    parent_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<
    BTreeMap<
        i64,
        (
            SourceSerials,
            positions::OwnerPositions<NoIntroDatabaseDumpSerialsField>,
        ),
    >,
> {
    let mut extents = native_extents(
        connection,
        "no_intro_dump_serials",
        "dump_source_id",
        "dump_source_id",
        ids,
    )?;
    validate_parent_extents(&extents, parent_extents)?;
    let mut rows = Vec::new();
    for batch in ids.chunks(400) {
        let select = "native.source_order, native.source_line AS line, native.source_column AS column, \
            native.box_barcode, native.box_serial, native.chip_serial, native.digital_serial1, \
            native.digital_serial2, native.lockout_serial, native.media_serial1, native.media_serial2, \
            native.media_serial3, native.mediastamp, native.pcb_serial, native.romchip_serial1, \
            native.romchip_serial2, native.savechip_serial, \
            typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
            AND typeof(native.source_column)='integer' AS valid";
        let sql = super::queries::owners(
            select,
            "no_intro_dump_serials",
            "dump_source_id",
            batch.len(),
            "native.source_order",
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        rows.extend(query.load::<DumpSerialsRow>(connection)?);
    }
    let positions_map = positions::load(
        connection,
        "no_intro_dump_serials_field_positions",
        "dump_source_id",
        ids,
        NoIntroDatabaseDumpSerialsField::from_code,
    )?;
    let mut result = BTreeMap::new();
    for row in rows {
        require_integer(row.valid, row.owner_id)?;
        if row.source_order < 0 {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
        }
        let attribute_positions = positions_map
            .get(&row.owner_id)
            .ok_or(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id))?;
        let expected = dump_serials_presence(&row);
        positions::validate_presence(attribute_positions, &expected, row.owner_id)?;
        let (_, extent) = extents
            .remove(&row.owner_id)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id))?;
        let record_location = location(row.line, row.column, row.owner_id)?;
        if extent.start() != record_location {
            return Err(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id));
        }
        let serials = declared_record!(SourceSerials, row, attribute_positions, row.owner_id, NoIntroDatabaseDumpSerialsField;
            box_barcode => BoxBarcode,
            box_serial => BoxSerial,
            chip_serial => ChipSerial,
            digital_serial1 => DigitalSerial1,
            digital_serial2 => DigitalSerial2,
            lockout_serial => LockoutSerial,
            media_serial1 => MediaSerial1,
            media_serial2 => MediaSerial2,
            media_serial3 => MediaSerial3,
            mediastamp => MediaStamp,
            pcb_serial => PcbSerial,
            romchip_serial1 => RomChipSerial1,
            romchip_serial2 => RomChipSerial2,
            savechip_serial => SaveChipSerial;
            source_order: usize::try_from(row.source_order)
                .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id))?,
            extent: extent
        );
        if result
            .insert(row.owner_id, (serials, attribute_positions.clone()))
            .is_some()
        {
            return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
        }
    }
    for id in ids {
        if !result.contains_key(id) {
            let positions = positions_map
                .get(id)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(*id))?;
            positions::validate_presence(positions, &[], *id)?;
        }
    }
    ensure_consumed_extents(&extents)?;
    Ok(result)
}

const fn dump_serials_presence(row: &DumpSerialsRow) -> [bool; 14] {
    [
        row.box_barcode.is_some(),
        row.box_serial.is_some(),
        row.chip_serial.is_some(),
        row.digital_serial1.is_some(),
        row.digital_serial2.is_some(),
        row.lockout_serial.is_some(),
        row.media_serial1.is_some(),
        row.media_serial2.is_some(),
        row.media_serial3.is_some(),
        row.mediastamp.is_some(),
        row.pcb_serial.is_some(),
        row.romchip_serial1.is_some(),
        row.romchip_serial2.is_some(),
        row.savechip_serial.is_some(),
    ]
}

fn load_releases(
    connection: &mut SqliteConnection,
    ids: &[i64],
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, NoIntroDatabaseRelease>> {
    validate_history_siblings(connection, ids, super::queries::SiblingParent::Release)?;
    let bases = load_release_rows(connection, ids, game_extents)?;
    let parent_extents = bases
        .iter()
        .map(|(id, base)| (*id, base.extent))
        .collect::<BTreeMap<_, _>>();
    let mut details = load_release_details(connection, ids, &parent_extents)?;
    let mut serials = load_release_serials(connection, ids, &parent_extents)?;
    let mut files = load_file_refs(
        connection,
        ids,
        super::queries::FileOwner::Release,
        NoIntroDatabaseReleaseFileField::from_code,
        &parent_extents,
    )?;
    let mut result = BTreeMap::new();
    for id in ids {
        let base = bases
            .get(id)
            .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(*id))?;
        let detail = details.remove(id);
        let (serial, serials_attribute_positions) = serials.remove(id).map_or_else(
            || (None, Vec::new()),
            |(value, positions)| (Some(value), positions.attributes),
        );
        result.insert(
            *id,
            NoIntroDatabaseRelease {
                id: NoIntroReleaseId::try_from(*id)?,
                source_order: base.source_order,
                location: base.location,
                details: detail,
                serials: serial,
                serials_attribute_positions,
                files: files.remove(id).unwrap_or_default(),
            },
        );
    }
    Ok(result)
}

fn load_release_rows(
    connection: &mut SqliteConnection,
    ids: &[i64],
    game_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, OwnerBase>> {
    let mut result = BTreeMap::new();
    let mut extents = native_extents(
        connection,
        "no_intro_releases",
        "release_id",
        "set_id",
        &game_extents.keys().copied().collect::<Vec<_>>(),
    )?;
    validate_parent_extents(&extents, game_extents)?;
    for batch in ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let sql = super::queries::owners(
            "native.source_order, native.source_line AS line, native.source_column AS column, \
             typeof(native.release_id)='integer' AND typeof(native.set_id)='integer' \
             AND typeof(native.source_order)='integer' AND typeof(native.source_line)='integer' \
             AND typeof(native.source_column)='integer' AS valid",
            "no_intro_releases",
            "release_id",
            batch.len(),
            "native.source_order",
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<OwnerRow>(connection)? {
            require_integer(row.valid, row.owner_id)?;
            let (_, extent) = extents
                .remove(&row.owner_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id))?;
            let record_location = location(row.line, row.column, row.owner_id)?;
            if extent.start() != record_location {
                return Err(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id));
            }
            if row.source_order < 0
                || result
                    .insert(
                        row.owner_id,
                        OwnerBase {
                            source_order: row.source_order,
                            location: record_location,
                            extent,
                        },
                    )
                    .is_some()
            {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(row.owner_id));
            }
        }
    }
    if let Some(id) = ids.iter().find(|id| !result.contains_key(id)) {
        return Err(NoIntroDatabaseQueryError::MismatchedOwner(*id));
    }
    Ok(result)
}

fn load_release_details(
    connection: &mut SqliteConnection,
    ids: &[i64],
    parent_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<BTreeMap<i64, super::NoIntroDatabaseReleaseDetails>> {
    let mut extents = native_extents(
        connection,
        "no_intro_release_details",
        "release_id",
        "release_id",
        ids,
    )?;
    validate_parent_extents(&extents, parent_extents)?;
    let mut result = BTreeMap::new();
    for batch in ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let values = std::iter::repeat_n("(?)", batch.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "WITH requested(owner_id) AS (VALUES {values}) \
             SELECT native.release_id AS owner_id, native.source_order, native.source_line AS line, \
                    native.source_column AS column, native.opening_end_line, native.opening_end_column, \
                    native.archivename, native.category, native.comment, native.date, native.dirname, \
                    native.\"group\" AS group_name, native.id, native.nfo_size, native.nfoname, native.nfosize, \
                    native.origin, native.originalformat, native.region, native.rominfo, native.tool, \
                    typeof(native.release_id)='integer' AND typeof(native.source_order)='integer' \
                    AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' \
                    AND typeof(native.opening_end_line)='integer' AND typeof(native.opening_end_column)='integer' AS valid \
             FROM requested CROSS JOIN no_intro_release_details AS native \
             WHERE native.release_id=requested.owner_id ORDER BY native.release_id"
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        let rows = query.load::<ReleaseDetailsRow>(connection)?;
        let mut hashes = load_nfo_hashes(connection, batch)?;
        let mut positions_map = positions::load(
            connection,
            "no_intro_release_details_field_positions",
            "release_id",
            batch,
            NoIntroDatabaseReleaseDetailsField::from_code,
        )?;
        for row in rows {
            let owner_id = row.owner_id;
            let attribute_positions = positions_map
                .remove(&owner_id)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner_id))?;
            let hash_rows = hashes.remove(&owner_id).unwrap_or_default();
            let details = release_details_from_row(
                row,
                attribute_positions,
                hash_rows,
                extents
                    .remove(&owner_id)
                    .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(owner_id))?
                    .1,
            )?;
            if result.insert(owner_id, details).is_some() {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(owner_id));
            }
        }
        for (owner_id, position) in positions_map {
            positions::validate_presence(&position, &[], owner_id)?;
        }
        if let Some((&owner_id, _)) = hashes.first_key_value() {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(owner_id));
        }
    }
    ensure_consumed_extents(&extents)?;
    Ok(result)
}

fn release_details_from_row(
    row: ReleaseDetailsRow,
    attribute_positions: positions::OwnerPositions<NoIntroDatabaseReleaseDetailsField>,
    mut hash_rows: BTreeMap<String, crate::storage::catalog_files::NoIntroDatabaseDigestValue>,
    extent: XmlSourceExtent,
) -> QueryResult<super::NoIntroDatabaseReleaseDetails> {
    require_integer(row.valid, row.owner_id)?;
    if row.source_order < 0 {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
    }
    let nfo_crc32 = hash_rows.remove("nfo_crc32");
    let nfocrc = hash_rows.remove("nfocrc");
    let expected = [
        row.archivename.is_some(),
        row.category.is_some(),
        row.comment.is_some(),
        row.date.is_some(),
        row.dirname.is_some(),
        row.group_name.is_some(),
        row.id.is_some(),
        nfo_crc32.is_some(),
        row.nfo_size.is_some(),
        nfocrc.is_some(),
        row.nfoname.is_some(),
        row.nfosize.is_some(),
        row.origin.is_some(),
        row.originalformat.is_some(),
        row.region.is_some(),
        row.rominfo.is_some(),
        row.tool.is_some(),
    ];
    positions::validate_presence(&attribute_positions, &expected, row.owner_id)?;
    let source_order = usize::try_from(row.source_order)
        .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id))?;
    let source_location = location(row.line, row.column, row.owner_id)?;
    let opening_end = location(row.opening_end_line, row.opening_end_column, row.owner_id)?;
    if extent.start() != source_location
        || !extent_contains_point_inclusive_end(extent, opening_end)
    {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id));
    }
    Ok(super::NoIntroDatabaseReleaseDetails {
        source_order: i64::try_from(source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id))?,
        location: source_location,
        opening_end,
        archivename: row.archivename,
        category: row.category,
        comment: row.comment,
        date: row.date,
        dirname: row.dirname,
        group: row.group_name,
        id: row.id,
        nfo_crc32,
        nfo_size: row.nfo_size,
        nfocrc,
        nfoname: row.nfoname,
        nfosize: row.nfosize,
        origin: row.origin,
        originalformat: row.originalformat,
        region: row.region,
        rominfo: row.rominfo,
        tool: row.tool,
        attribute_positions: attribute_positions.attributes,
    })
}

fn load_nfo_hashes(
    connection: &mut SqliteConnection,
    ids: &[i64],
) -> QueryResult<
    BTreeMap<i64, BTreeMap<String, crate::storage::catalog_files::NoIntroDatabaseDigestValue>>,
> {
    let mut result = BTreeMap::<
        i64,
        BTreeMap<String, crate::storage::catalog_files::NoIntroDatabaseDigestValue>,
    >::new();
    for batch in ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let mut query = sql_query(super::queries::nfo_hashes(batch.len()))
            .into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        for row in query.load::<NfoHashRow>(connection)? {
            require_integer(row.valid, row.owner_id)?;
            if row.presence.as_deref() != Some("present")
                || row.scope.as_deref() != Some("nfo_companion")
            {
                return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
            }
            let value = match (
                row.hash_id,
                row.algorithm.as_deref(),
                row.digest,
                row.invalid_literal,
            ) {
                (Some(_), Some("crc32"), Some(bytes), None) if bytes.len() == 4 => {
                    crate::storage::catalog_files::NoIntroDatabaseDigestValue::Valid(bytes)
                }
                (None, None, None, Some(literal))
                    if literal.len() != 8
                        || !literal.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
                {
                    crate::storage::catalog_files::NoIntroDatabaseDigestValue::Invalid(literal)
                }
                _ => return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id)),
            };
            let field = row.source_hash_field;
            if field != "nfo_crc32" && field != "nfocrc" {
                return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
            }
            if result
                .entry(row.owner_id)
                .or_default()
                .insert(field, value)
                .is_some()
            {
                return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
            }
        }
    }
    Ok(result)
}

fn load_release_serials(
    connection: &mut SqliteConnection,
    ids: &[i64],
    parent_extents: &BTreeMap<i64, XmlSourceExtent>,
) -> QueryResult<
    BTreeMap<
        i64,
        (
            ReleaseSerials,
            positions::OwnerPositions<NoIntroDatabaseReleaseSerialsField>,
        ),
    >,
> {
    let mut extents = native_extents(
        connection,
        "no_intro_release_serials",
        "release_id",
        "release_id",
        ids,
    )?;
    validate_parent_extents(&extents, parent_extents)?;
    let mut result = BTreeMap::new();
    for batch in ids.chunks(400) {
        if batch.is_empty() {
            continue;
        }
        let select = "native.source_order, native.source_line AS line, native.source_column AS column, \
            native.box_barcode, native.box_serial, native.media_serial1, native.mediastamp, \
            native.pcb_serial, native.romchip_serial1, \
            typeof(native.release_id)='integer' AND typeof(native.source_order)='integer' \
            AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' AS valid";
        let sql = super::queries::owners(
            select,
            "no_intro_release_serials",
            "release_id",
            batch.len(),
            "native.source_order",
        );
        let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        let rows = query.load::<ReleaseSerialsRow>(connection)?;
        let mut positions_map = positions::load(
            connection,
            "no_intro_release_serials_field_positions",
            "release_id",
            batch,
            NoIntroDatabaseReleaseSerialsField::from_code,
        )?;
        for row in rows {
            let owner_id = row.owner_id;
            let position = positions_map
                .remove(&owner_id)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner_id))?;
            let extent = extents
                .remove(&owner_id)
                .ok_or(NoIntroDatabaseQueryError::MismatchedOwner(owner_id))?
                .1;
            let serials = release_serials_from_row(row, &position, extent)?;
            if result.insert(owner_id, (serials, position)).is_some() {
                return Err(NoIntroDatabaseQueryError::MismatchedOwner(owner_id));
            }
        }
        for (owner_id, position) in positions_map {
            positions::validate_presence(&position, &[], owner_id)?;
        }
    }
    ensure_consumed_extents(&extents)?;
    Ok(result)
}

fn release_serials_from_row(
    row: ReleaseSerialsRow,
    position: &positions::OwnerPositions<NoIntroDatabaseReleaseSerialsField>,
    extent: XmlSourceExtent,
) -> QueryResult<ReleaseSerials> {
    require_integer(row.valid, row.owner_id)?;
    if row.source_order < 0 {
        return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id));
    }
    positions::validate_presence(
        position,
        &[
            row.box_barcode.is_some(),
            row.box_serial.is_some(),
            row.media_serial1.is_some(),
            row.mediastamp.is_some(),
            row.pcb_serial.is_some(),
            row.romchip_serial1.is_some(),
        ],
        row.owner_id,
    )?;
    let record_location = location(row.line, row.column, row.owner_id)?;
    if extent.start() != record_location {
        return Err(NoIntroDatabaseQueryError::InvalidPositions(row.owner_id));
    }
    Ok(ReleaseSerials {
        source_order: usize::try_from(row.source_order)
            .map_err(|_| NoIntroDatabaseQueryError::InvalidMetadata(row.owner_id))?,
        extent,
        box_barcode: declared(
            row.box_barcode,
            position,
            NoIntroDatabaseReleaseSerialsField::BoxBarcode,
            row.owner_id,
        )?,
        box_serial: declared(
            row.box_serial,
            position,
            NoIntroDatabaseReleaseSerialsField::BoxSerial,
            row.owner_id,
        )?,
        media_serial1: declared(
            row.media_serial1,
            position,
            NoIntroDatabaseReleaseSerialsField::MediaSerial1,
            row.owner_id,
        )?,
        mediastamp: declared(
            row.mediastamp,
            position,
            NoIntroDatabaseReleaseSerialsField::MediaStamp,
            row.owner_id,
        )?,
        pcb_serial: declared(
            row.pcb_serial,
            position,
            NoIntroDatabaseReleaseSerialsField::PcbSerial,
            row.owner_id,
        )?,
        romchip_serial1: declared(
            row.romchip_serial1,
            position,
            NoIntroDatabaseReleaseSerialsField::RomChipSerial1,
            row.owner_id,
        )?,
    })
}

fn declared<Field: Copy + PartialEq>(
    value: Option<String>,
    positions: &positions::OwnerPositions<Field>,
    field: Field,
    owner: i64,
) -> QueryResult<Option<crate::xml_reader::DeclaredText>> {
    value
        .map(|value| {
            let position = positions
                .attributes
                .iter()
                .find(|position| position.field == field)
                .ok_or(NoIntroDatabaseQueryError::InvalidPositions(owner))?;
            Ok(crate::xml_reader::DeclaredText {
                value,
                source_order: position.source_order,
                location: RecordLocation {
                    line: position.location.line,
                    column: position.location.column,
                },
            })
        })
        .transpose()
}

fn validate_sibling_rows(rows: &[ChildRow]) -> QueryResult<()> {
    let mut parents = BTreeMap::new();
    for row in rows {
        require_integer(row.valid, row.child_id)?;
        if row.child_id <= 0 || row.game_id <= 0 {
            return Err(NoIntroDatabaseQueryError::InvalidMetadata(row.child_id));
        }
        let (order, previous_end) = parents.entry(row.game_id).or_insert((0, None));
        advance_order(row.source_order, order, row.child_id)?;
        let extent = checked_extent(
            location(row.line, row.column, row.child_id)?,
            location(row.end_line, row.end_column, row.child_id)?,
            row.child_id,
        )?;
        advance_extent(previous_end, extent, row.child_id)?;
    }
    Ok(())
}

fn validate_history_siblings(
    connection: &mut SqliteConnection,
    ids: &[i64],
    parent: super::queries::SiblingParent,
) -> QueryResult<()> {
    for batch in ids.chunks(400) {
        let mut query = sql_query(super::queries::sibling_rows(batch.len(), parent))
            .into_boxed::<diesel::sqlite::Sqlite>();
        for id in batch {
            query = query.bind::<BigInt, _>(*id);
        }
        validate_sibling_rows(&query.load::<ChildRow>(connection)?)?;
    }
    Ok(())
}

fn load_child_rows(connection: &mut SqliteConnection, ids: &[i64]) -> QueryResult<Vec<ChildRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query =
        sql_query(super::queries::child_rows(ids.len())).into_boxed::<diesel::sqlite::Sqlite>();
    for id in ids {
        query = query.bind::<BigInt, _>(*id);
    }
    Ok(query.load(connection)?)
}
