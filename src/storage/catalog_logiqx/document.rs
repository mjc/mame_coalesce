use diesel::{
    OptionalExtension, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::SnapshotKey,
    logiqx::{ClrMameProAttribute, DocumentAttribute, RomCenterAttribute},
};

use super::{
    LogiqxClrMameProOptions, LogiqxDocument, LogiqxForceMerging, LogiqxForceNoDump,
    LogiqxForcePacking, LogiqxHeader, LogiqxHeaderTextField, LogiqxHeaderTextPosition,
    LogiqxOptionValue, LogiqxQueryError, LogiqxRomCenterOptions, LogiqxRomMode, LogiqxSampleMode,
    positions::{self, PositionRow},
    reader::{QueryResult, default_presence, invalid, location, row, stored_bool, yes_no},
};

row!(DocumentRow {
    build: Option<String> => Nullable<Text>,
    debug: String => Text,
    debug_was_present: i64 => BigInt,
    file_name: Option<String> => Nullable<Text>,
    sha1: Option<Vec<u8>> => Nullable<Binary>,
    header_name: Option<String> => Nullable<Text>,
    header_description: Option<String> => Nullable<Text>,
    header_category: Option<String> => Nullable<Text>,
    header_version: Option<String> => Nullable<Text>,
    header_date: Option<String> => Nullable<Text>,
    header_author: Option<String> => Nullable<Text>,
    header_email: Option<String> => Nullable<Text>,
    header_homepage: Option<String> => Nullable<Text>,
    header_url: Option<String> => Nullable<Text>,
    header_comment: Option<String> => Nullable<Text>,
    valid: i64 => BigInt,
});

row!(ClrRow {
    source_order: i64 => BigInt,
    source_line: i64 => BigInt,
    source_column: i64 => BigInt,
    header: Option<String> => Nullable<Text>,
    header_was_present: i64 => BigInt,
    forcemerging: String => Text,
    forcemerging_was_present: i64 => BigInt,
    forcenodump: String => Text,
    forcenodump_was_present: i64 => BigInt,
    forcepacking: String => Text,
    forcepacking_was_present: i64 => BigInt,
    valid: i64 => BigInt,
});

row!(RomCenterRow {
    source_order: i64 => BigInt,
    source_line: i64 => BigInt,
    source_column: i64 => BigInt,
    plugin: Option<String> => Nullable<Text>,
    plugin_was_present: i64 => BigInt,
    rommode: String => Text,
    rommode_was_present: i64 => BigInt,
    biosmode: String => Text,
    biosmode_was_present: i64 => BigInt,
    samplemode: String => Text,
    samplemode_was_present: i64 => BigInt,
    lockrommode: String => Text,
    lockrommode_was_present: i64 => BigInt,
    lockbiosmode: String => Text,
    lockbiosmode_was_present: i64 => BigInt,
    locksamplemode: String => Text,
    locksamplemode_was_present: i64 => BigInt,
    valid: i64 => BigInt,
});

pub(super) fn load(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> QueryResult<LogiqxDocument> {
    let row = sql_query(super::queries::document("build, debug, debug_was_present, file_name, sha1, header_name, \
                        header_description, header_category, header_version, header_date, header_author, \
                        header_email, header_homepage, header_url, header_comment",
                        "logiqx_document_facts", &["debug_was_present"]))
        .bind::<Text, _>(snapshot.as_str()).get_result::<DocumentRow>(connection).optional()?
        .ok_or_else(|| LogiqxQueryError::MissingDocument(snapshot.clone()))?;
    positions::require_integer_provenance(row.valid, 0)?;
    let debug_was_present = default_presence(&row.debug, row.debug_was_present, "no", "debug", 0)?;
    let debug = yes_no(&row.debug, "debug", 0)?;
    if row.sha1.as_ref().is_some_and(|sha1| sha1.len() != 20) {
        return Err(invalid("document SHA1", 0, "expected 20 bytes".to_owned()));
    }
    let attribute_positions = positions::attributes(
        positioned(connection, snapshot, "logiqx_document_attribute_positions")?,
        &[row.build.is_some(), debug_was_present],
        DocumentAttribute::from_code,
        0,
    )?;
    let clrmamepro = load_clrmamepro(connection, snapshot)?;
    let romcenter = load_romcenter(connection, snapshot)?;
    let text_rows = positioned(connection, snapshot, "logiqx_header_text_positions")?;
    let header_present = row.header_name.is_some();
    let present = [
        header_present,
        row.header_description.is_some(),
        row.header_category.is_some(),
        row.header_version.is_some(),
        row.header_date.is_some(),
        row.header_author.is_some(),
        row.header_email.is_some(),
        row.header_homepage.is_some(),
        row.header_url.is_some(),
        row.header_comment.is_some(),
    ];
    positions::validate(&text_rows, &present, 0)?;
    if !header_present
        && (present.iter().any(|present| *present) || clrmamepro.is_some() || romcenter.is_some())
    {
        return Err(LogiqxQueryError::InvalidPositions(0));
    }
    positions::child_orders(
        text_rows
            .iter()
            .map(|row| row.source_order)
            .chain(clrmamepro.iter().map(|options| options.source_order))
            .chain(romcenter.iter().map(|options| options.source_order)),
        0,
    )?;
    let text_positions = text_rows
        .into_iter()
        .map(|position| {
            Ok(LogiqxHeaderTextPosition {
                field: LogiqxHeaderTextField::from_code(position.field_kind)
                    .ok_or(LogiqxQueryError::InvalidPositions(0))?,
                source_order: usize::try_from(position.source_order)
                    .map_err(|_| LogiqxQueryError::InvalidPositions(0))?,
                location: location(position.line, position.column, 0)?,
            })
        })
        .collect::<QueryResult<Vec<_>>>()?;
    let header = header_present.then_some(LogiqxHeader {
        name: row.header_name,
        description: row.header_description,
        category: row.header_category,
        version: row.header_version,
        date: row.header_date,
        author: row.header_author,
        email: row.header_email,
        homepage: row.header_homepage,
        url: row.header_url,
        comment: row.header_comment,
        text_positions,
    });
    Ok(LogiqxDocument {
        build: row.build,
        debug,
        debug_was_present,
        file_name: row.file_name,
        sha1: row.sha1,
        header,
        clrmamepro,
        romcenter,
        attribute_positions,
    })
}

fn positioned(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    table: &str,
) -> QueryResult<Vec<PositionRow>> {
    Ok(sql_query(super::queries::document_positions(table))
        .bind::<Text, _>(snapshot.as_str())
        .load(connection)?)
}

fn load_clrmamepro(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> QueryResult<Option<LogiqxClrMameProOptions>> {
    let row = sql_query(super::queries::document("source_order, source_line, source_column, header, header_was_present, \
                        forcemerging, forcemerging_was_present, forcenodump, forcenodump_was_present, \
                        forcepacking, forcepacking_was_present", "logiqx_clrmamepro_options",
                        &["source_order", "source_line", "source_column", "header_was_present",
                          "forcemerging_was_present", "forcenodump_was_present", "forcepacking_was_present"]))
        .bind::<Text, _>(snapshot.as_str()).get_result::<ClrRow>(connection).optional()?;
    let rows = positioned(
        connection,
        snapshot,
        "logiqx_clrmamepro_attribute_positions",
    )?;
    let Some(row) = row else {
        positions::validate(&rows, &[], 0)?;
        return Ok(None);
    };
    positions::require_integer_provenance(row.valid, 0)?;
    if stored_bool(row.header_was_present, "clrmamepro header presence", 0)? != row.header.is_some()
    {
        return Err(LogiqxQueryError::InvalidPositions(0));
    }
    let forcemerging = match row.forcemerging.as_str() {
        "split" => LogiqxForceMerging::Split,
        "none" => LogiqxForceMerging::None,
        "full" => LogiqxForceMerging::Full,
        _ => return Err(invalid("forcemerging", 0, row.forcemerging)),
    };
    let forcenodump = match row.forcenodump.as_str() {
        "obsolete" => LogiqxForceNoDump::Obsolete,
        "required" => LogiqxForceNoDump::Required,
        "ignore" => LogiqxForceNoDump::Ignore,
        _ => return Err(invalid("forcenodump", 0, row.forcenodump)),
    };
    let forcepacking = match row.forcepacking.as_str() {
        "zip" => LogiqxForcePacking::Zip,
        "unzip" => LogiqxForcePacking::Unzip,
        _ => return Err(invalid("forcepacking", 0, row.forcepacking)),
    };
    let forcemerging_was_present = default_presence(
        &row.forcemerging,
        row.forcemerging_was_present,
        "split",
        "forcemerging",
        0,
    )?;
    let forcenodump_was_present = default_presence(
        &row.forcenodump,
        row.forcenodump_was_present,
        "obsolete",
        "forcenodump",
        0,
    )?;
    let forcepacking_was_present = default_presence(
        &row.forcepacking,
        row.forcepacking_was_present,
        "zip",
        "forcepacking",
        0,
    )?;
    let attribute_positions = positions::attributes(
        rows,
        &[
            row.header.is_some(),
            forcemerging_was_present,
            forcenodump_was_present,
            forcepacking_was_present,
        ],
        ClrMameProAttribute::from_code,
        0,
    )?;
    Ok(Some(LogiqxClrMameProOptions {
        source_order: row.source_order,
        location: location(row.source_line, row.source_column, 0)?,
        header: row.header,
        forcemerging: LogiqxOptionValue::new(forcemerging, forcemerging_was_present),
        forcenodump: LogiqxOptionValue::new(forcenodump, forcenodump_was_present),
        forcepacking: LogiqxOptionValue::new(forcepacking, forcepacking_was_present),
        attribute_positions,
    }))
}

fn rom_mode(value: &str, field: &'static str) -> QueryResult<LogiqxRomMode> {
    match value {
        "split" => Ok(LogiqxRomMode::Split),
        "merged" => Ok(LogiqxRomMode::Merged),
        "unmerged" => Ok(LogiqxRomMode::Unmerged),
        _ => Err(invalid(field, 0, value.to_owned())),
    }
}

fn load_romcenter(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> QueryResult<Option<LogiqxRomCenterOptions>> {
    let row = sql_query(super::queries::document("source_order, source_line, source_column, plugin, plugin_was_present, \
                        rommode, rommode_was_present, biosmode, biosmode_was_present, samplemode, samplemode_was_present, \
                        lockrommode, lockrommode_was_present, lockbiosmode, lockbiosmode_was_present, \
                        locksamplemode, locksamplemode_was_present", "logiqx_romcenter_options",
                        &["source_order", "source_line", "source_column", "plugin_was_present", "rommode_was_present",
                          "biosmode_was_present", "samplemode_was_present", "lockrommode_was_present",
                          "lockbiosmode_was_present", "locksamplemode_was_present"]))
        .bind::<Text, _>(snapshot.as_str()).get_result::<RomCenterRow>(connection).optional()?;
    let rows = positioned(connection, snapshot, "logiqx_romcenter_attribute_positions")?;
    let Some(row) = row else {
        positions::validate(&rows, &[], 0)?;
        return Ok(None);
    };
    positions::require_integer_provenance(row.valid, 0)?;
    if stored_bool(row.plugin_was_present, "romcenter plugin presence", 0)? != row.plugin.is_some()
    {
        return Err(LogiqxQueryError::InvalidPositions(0));
    }
    let rommode = rom_mode(&row.rommode, "rommode")?;
    let biosmode = rom_mode(&row.biosmode, "biosmode")?;
    let samplemode = match row.samplemode.as_str() {
        "merged" => LogiqxSampleMode::Merged,
        "unmerged" => LogiqxSampleMode::Unmerged,
        _ => return Err(invalid("samplemode", 0, row.samplemode)),
    };
    let rommode_was_present =
        default_presence(&row.rommode, row.rommode_was_present, "split", "rommode", 0)?;
    let biosmode_was_present = default_presence(
        &row.biosmode,
        row.biosmode_was_present,
        "split",
        "biosmode",
        0,
    )?;
    let samplemode_was_present = default_presence(
        &row.samplemode,
        row.samplemode_was_present,
        "merged",
        "samplemode",
        0,
    )?;
    let lockrommode_was_present = default_presence(
        &row.lockrommode,
        row.lockrommode_was_present,
        "no",
        "lockrommode",
        0,
    )?;
    let lockbiosmode_was_present = default_presence(
        &row.lockbiosmode,
        row.lockbiosmode_was_present,
        "no",
        "lockbiosmode",
        0,
    )?;
    let locksamplemode_was_present = default_presence(
        &row.locksamplemode,
        row.locksamplemode_was_present,
        "no",
        "locksamplemode",
        0,
    )?;
    let attribute_positions = positions::attributes(
        rows,
        &[
            row.plugin.is_some(),
            rommode_was_present,
            biosmode_was_present,
            samplemode_was_present,
            lockrommode_was_present,
            lockbiosmode_was_present,
            locksamplemode_was_present,
        ],
        RomCenterAttribute::from_code,
        0,
    )?;
    Ok(Some(LogiqxRomCenterOptions {
        source_order: row.source_order,
        location: location(row.source_line, row.source_column, 0)?,
        plugin: row.plugin,
        rommode: LogiqxOptionValue::new(rommode, rommode_was_present),
        biosmode: LogiqxOptionValue::new(biosmode, biosmode_was_present),
        samplemode: LogiqxOptionValue::new(samplemode, samplemode_was_present),
        lockrommode: LogiqxOptionValue::new(
            yes_no(&row.lockrommode, "lockrommode", 0)?,
            lockrommode_was_present,
        ),
        lockbiosmode: LogiqxOptionValue::new(
            yes_no(&row.lockbiosmode, "lockbiosmode", 0)?,
            lockbiosmode_was_present,
        ),
        locksamplemode: LogiqxOptionValue::new(
            yes_no(&row.locksamplemode, "locksamplemode", 0)?,
            locksamplemode_was_present,
        ),
        attribute_positions,
    }))
}
