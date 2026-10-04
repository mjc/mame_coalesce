//! Required header and optional directive owners.

use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    logiqx::RecordLocation,
    storage::no_intro_dat_fields::{
        NoIntroDatClrMameProField as ClrMameProField, NoIntroDatHeaderField as HeaderField,
        NoIntroDatRomCenterField as RomCenterField,
    },
    xml_reader::DeclaredText,
};

use super::{
    NoIntroDatClrMameProDirective, NoIntroDatForceNoDump, NoIntroDatHeader, NoIntroDatHeaderChild,
    NoIntroDatQueryError, NoIntroDatRomCenterDirective, positions, queries,
};

type QueryResult<T> = Result<T, NoIntroDatQueryError>;

#[derive(QueryableByName)]
struct HeaderRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    trademarks: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    piracy: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    subset: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct ClrRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct RomCenterRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    plugin_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct PositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
    #[diesel(sql_type = BigInt)]
    valid: i64,
}

#[derive(Clone, Copy)]
struct Position {
    source_order: usize,
    line: i64,
    column: i64,
}

pub(super) struct LoadedHeader {
    pub header: NoIntroDatHeader,
    pub text_count: i64,
    pub clrmamepro_count: i64,
    pub romcenter_count: i64,
    pub clrmamepro_field_count: i64,
    pub romcenter_field_count: i64,
}

pub(super) fn load(connection: &mut SqliteConnection, snapshot: &str) -> QueryResult<LoadedHeader> {
    let owner = sql_query(queries::HEADER)
        .bind::<Text, _>(snapshot)
        .get_result::<HeaderRow>(connection)
        .optional()?
        .ok_or(NoIntroDatQueryError::MissingNative {
            field: "required header",
            owner: 0,
        })?;
    require_core_header_fields(&owner)?;
    positions::require_integer(owner.valid, 0)?;
    if owner.source_order < 0 {
        return Err(NoIntroDatQueryError::InvalidMetadata(0));
    }
    let source_order = order(owner.source_order, 0)?;
    let location = positions::location(owner.line, owner.column, 0)?;
    let text_positions = load_positions(
        connection,
        "no_intro_dat_header_field_positions",
        snapshot,
        HeaderField::from_code,
    )?;
    let clr_positions = load_positions(
        connection,
        "no_intro_dat_clrmamepro_field_positions",
        snapshot,
        ClrMameProField::from_code,
    )?;
    let rom_positions = load_positions(
        connection,
        "no_intro_dat_romcenter_field_positions",
        snapshot,
        RomCenterField::from_code,
    )?;
    let clrmamepro_field_count =
        i64::try_from(clr_positions.len()).map_err(|_| NoIntroDatQueryError::InvalidMetadata(0))?;
    let romcenter_field_count =
        i64::try_from(rom_positions.len()).map_err(|_| NoIntroDatQueryError::InvalidMetadata(0))?;
    let clr_row = sql_query(queries::CLRMAMEPRO)
        .bind::<Text, _>(snapshot)
        .get_result::<ClrRow>(connection)
        .optional()?;
    let rom_row = sql_query(queries::ROMCENTER)
        .bind::<Text, _>(snapshot)
        .get_result::<RomCenterRow>(connection)
        .optional()?;
    let clrmamepro_count = i64::from(clr_row.is_some());
    let romcenter_count = i64::from(rom_row.is_some());
    if clr_row.is_none() && !clr_positions.is_empty() {
        return Err(NoIntroDatQueryError::InvalidPositions(0));
    }
    if rom_row.is_none() && !rom_positions.is_empty() {
        return Err(NoIntroDatQueryError::InvalidPositions(0));
    }

    let mut children = header_text_children(owner, text_positions)?;
    let text_count =
        i64::try_from(children.len()).map_err(|_| NoIntroDatQueryError::InvalidMetadata(0))?;
    if let Some(row) = clr_row {
        children.push(NoIntroDatHeaderChild::ClrMamePro(clrmamepro_directive(
            row,
            clr_positions,
        )?));
    }
    if let Some(row) = rom_row {
        children.push(NoIntroDatHeaderChild::RomCenter(romcenter_directive(
            row,
            rom_positions,
        )?));
    }
    children.sort_by_key(NoIntroDatHeaderChild::source_order);
    ensure_unique_orders(children.iter().map(NoIntroDatHeaderChild::source_order), 0)?;
    Ok(LoadedHeader {
        header: NoIntroDatHeader {
            source_order,
            location,
            children,
        },
        text_count,
        clrmamepro_count,
        romcenter_count,
        clrmamepro_field_count,
        romcenter_field_count,
    })
}

fn require_core_header_fields(owner: &HeaderRow) -> QueryResult<()> {
    for (field, value) in [
        ("header id", &owner.id_text),
        ("header name", &owner.name),
        ("header description", &owner.description),
        ("header version", &owner.version_text),
    ] {
        if value.is_none() {
            return Err(NoIntroDatQueryError::MissingNative { field, owner: 0 });
        }
    }
    Ok(())
}

fn header_text_children(
    owner: HeaderRow,
    mut positions: BTreeMap<i64, Position>,
) -> QueryResult<Vec<NoIntroDatHeaderChild>> {
    let mut children = Vec::new();
    for (field, value) in [
        (HeaderField::Id, owner.id_text),
        (HeaderField::Name, owner.name),
        (HeaderField::Description, owner.description),
        (HeaderField::Version, owner.version_text),
        (HeaderField::Date, owner.date),
        (HeaderField::Author, owner.author),
        (HeaderField::Homepage, owner.homepage),
        (HeaderField::Url, owner.url),
        (HeaderField::Trademarks, owner.trademarks),
        (HeaderField::Piracy, owner.piracy),
        (HeaderField::Subset, owner.subset),
        (HeaderField::Comment, owner.comment),
    ] {
        if let Some(value) = positioned_text(value, &mut positions, field as i64)? {
            children.push(NoIntroDatHeaderChild::Text { field, value });
        }
    }
    if !positions.is_empty() {
        return Err(NoIntroDatQueryError::InvalidPositions(0));
    }
    Ok(children)
}

fn clrmamepro_directive(
    row: ClrRow,
    mut field_positions: BTreeMap<i64, Position>,
) -> QueryResult<NoIntroDatClrMameProDirective> {
    positions::require_integer(row.valid, 0)?;
    let source_order = order(row.source_order, 0)?;
    let location = positions::location(row.line, row.column, 0)?;
    let forcenodump = positioned_text(
        row.forcenodump_text,
        &mut field_positions,
        ClrMameProField::ForceNoDump as i64,
    )?;
    let header = positioned_text(
        row.header_text,
        &mut field_positions,
        ClrMameProField::Header as i64,
    )?;
    if !field_positions.is_empty() {
        return Err(NoIntroDatQueryError::InvalidPositions(0));
    }
    Ok(NoIntroDatClrMameProDirective {
        source_order,
        location,
        forcenodump,
        header,
        forcenodump_effective: force_no_dump(row.forcenodump_effective)?,
    })
}

fn romcenter_directive(
    row: RomCenterRow,
    mut field_positions: BTreeMap<i64, Position>,
) -> QueryResult<NoIntroDatRomCenterDirective> {
    positions::require_integer(row.valid, 0)?;
    let source_order = order(row.source_order, 0)?;
    let location = positions::location(row.line, row.column, 0)?;
    let plugin = positioned_text(
        row.plugin_text,
        &mut field_positions,
        RomCenterField::Plugin as i64,
    )?;
    if !field_positions.is_empty() {
        return Err(NoIntroDatQueryError::InvalidPositions(0));
    }
    Ok(NoIntroDatRomCenterDirective {
        source_order,
        location,
        plugin,
    })
}

fn force_no_dump(value: Option<String>) -> QueryResult<Option<NoIntroDatForceNoDump>> {
    value
        .map(|value| match value.as_str() {
            "obsolete" => Ok(NoIntroDatForceNoDump::Obsolete),
            "required" => Ok(NoIntroDatForceNoDump::Required),
            "ignore" => Ok(NoIntroDatForceNoDump::Ignore),
            _ => Err(NoIntroDatQueryError::InvalidStoredValue {
                field: "forcenodump effective value",
                owner: 0,
                value,
            }),
        })
        .transpose()
}

fn load_positions<Field>(
    connection: &mut SqliteConnection,
    table: &'static str,
    snapshot: &str,
    decode: impl Fn(i64) -> Option<Field>,
) -> QueryResult<BTreeMap<i64, Position>> {
    let query = format!(
        "SELECT field_kind, source_order, source_line AS line, source_column AS column, \
         typeof(field_kind)='integer' AND typeof(source_order)='integer' \
           AND typeof(source_line)='integer' AND typeof(source_column)='integer' AS valid \
         FROM {table} WHERE snapshot_key=? ORDER BY source_order"
    );
    let rows = sql_query(query)
        .bind::<Text, _>(snapshot)
        .load::<PositionRow>(connection)?;
    let mut result = BTreeMap::new();
    let mut orders = BTreeSet::new();
    for row in rows {
        if row.valid != 1
            || decode(row.field_kind).is_none()
            || row.source_order < 0
            || row.line <= 0
            || row.column <= 0
            || !orders.insert(row.source_order)
        {
            return Err(NoIntroDatQueryError::InvalidPositions(0));
        }
        let position = Position {
            source_order: usize::try_from(row.source_order)
                .map_err(|_| NoIntroDatQueryError::InvalidPositions(0))?,
            line: row.line,
            column: row.column,
        };
        if result.insert(row.field_kind, position).is_some() {
            return Err(NoIntroDatQueryError::InvalidPositions(0));
        }
    }
    Ok(result)
}

fn positioned_text(
    value: Option<String>,
    positions: &mut BTreeMap<i64, Position>,
    code: i64,
) -> QueryResult<Option<DeclaredText>> {
    match (value, positions.remove(&code)) {
        (None, None) => Ok(None),
        (Some(value), Some(position)) => Ok(Some(declared(value, position))),
        _ => Err(NoIntroDatQueryError::InvalidPositions(0)),
    }
}

const fn declared(value: String, position: Position) -> DeclaredText {
    DeclaredText {
        value,
        source_order: position.source_order,
        location: RecordLocation {
            line: position.line,
            column: position.column,
        },
    }
}

fn order(value: i64, owner: i64) -> QueryResult<usize> {
    usize::try_from(value).map_err(|_| NoIntroDatQueryError::InvalidPositions(owner))
}

fn ensure_unique_orders(orders: impl Iterator<Item = usize>, owner: i64) -> QueryResult<()> {
    let mut seen = BTreeSet::new();
    if orders.into_iter().all(|order| seen.insert(order)) {
        Ok(())
    } else {
        Err(NoIntroDatQueryError::InvalidPositions(owner))
    }
}
