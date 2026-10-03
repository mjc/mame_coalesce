use std::collections::BTreeMap;

use diesel::{
    SqliteConnection,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    domain::{OccurrenceId, SnapshotKey},
    logiqx::RecordLocation,
};

use super::{
    LogiqxGame, LogiqxMediaKind, LogiqxMediaReference, LogiqxQueryError,
    games::{family_order, game_mut},
    positions::{self, PositionMap, PositionRow},
    reader::{QueryResult, batch, location, row},
};

row!(MediaRow {
    owner_id: i64 => BigInt,
    occurrence_id: i64 => BigInt,
    occurrence_order: i64 => BigInt,
    claim_kind: String => Text,
    native_count: i64 => BigInt,
    source_order: Option<i64> => Nullable<BigInt>,
    line: Option<i64> => Nullable<BigInt>,
    column: Option<i64> => Nullable<BigInt>,
    expected_fields: i64 => BigInt,
    valid: i64 => BigInt,
    merge_present: i64 => BigInt,
    merge_kind: Option<String> => Nullable<Text>,
    merge_snapshot: Option<String> => Nullable<Text>,
    merge_reference_kind: Option<String> => Nullable<Text>,
    merge_origin: Option<String> => Nullable<Text>,
});

const COLUMNS: &str = "
SELECT native.record_id AS owner_id, native.occurrence_id, native.occurrence_order, native.claim_kind,
       (rom.occurrence_id IS NOT NULL)+(disk.occurrence_id IS NOT NULL)+(sample.occurrence_id IS NOT NULL) AS native_count,
       CASE native.claim_kind WHEN 'logiqx_rom' THEN rom.source_order WHEN 'logiqx_disk' THEN disk.source_order
         WHEN 'logiqx_sample' THEN sample.source_order END AS source_order,
       CASE native.claim_kind WHEN 'logiqx_rom' THEN rom.source_line WHEN 'logiqx_disk' THEN disk.source_line
         WHEN 'logiqx_sample' THEN sample.source_line END AS line,
       CASE native.claim_kind WHEN 'logiqx_rom' THEN rom.source_column WHEN 'logiqx_disk' THEN disk.source_column
         WHEN 'logiqx_sample' THEN sample.source_column END AS column,
       CASE native.claim_kind
         WHEN 'logiqx_rom' THEN 1+2*(rom.size_text IS NOT NULL)+4*(rom.crc_text IS NOT NULL)
           +8*(rom.sha1_text IS NOT NULL)+16*(rom.md5_text IS NOT NULL)+32*(merges.occurrence_id IS NOT NULL)
           +64*(rom.status_was_present=1)+128*(rom.date IS NOT NULL)+256*(rom.serial IS NOT NULL)
         WHEN 'logiqx_disk' THEN 1+2*(disk.sha1_text IS NOT NULL)+4*(disk.md5_text IS NOT NULL)
           +8*(merges.occurrence_id IS NOT NULL)+16*(disk.status_was_present=1)
         ELSE 1 END AS expected_fields,
       COALESCE(typeof(native.occurrence_order)='integer' AND CASE native.claim_kind
         WHEN 'logiqx_rom' THEN rom.claim_kind=native.claim_kind AND rom.dump_status IN ('good','baddump','nodump','verified')
           AND typeof(rom.status_was_present)='integer' AND rom.status_was_present IN (0,1)
           AND (rom.status_was_present=1 OR rom.dump_status='good')
           AND typeof(rom.source_order)='integer' AND typeof(rom.source_line)='integer' AND typeof(rom.source_column)='integer'
         WHEN 'logiqx_disk' THEN disk.claim_kind=native.claim_kind AND disk.dump_status IN ('good','baddump','nodump','verified')
           AND typeof(disk.status_was_present)='integer' AND disk.status_was_present IN (0,1)
           AND (disk.status_was_present=1 OR disk.dump_status='good')
           AND typeof(disk.source_order)='integer' AND typeof(disk.source_line)='integer' AND typeof(disk.source_column)='integer'
         WHEN 'logiqx_sample' THEN sample.claim_kind=native.claim_kind AND native.content_uuid IS NULL
           AND typeof(sample.source_order)='integer' AND typeof(sample.source_line)='integer' AND typeof(sample.source_column)='integer'
         ELSE 0 END,0) AS valid,
       merges.occurrence_id IS NOT NULL AS merge_present, merges.claim_kind AS merge_kind,
       registry.snapshot_key AS merge_snapshot, reported.source_reference_kind AS merge_reference_kind, registry.origin AS merge_origin
FROM requested CROSS JOIN asset_occurrences AS native
LEFT JOIN logiqx_rom_claims AS rom ON rom.occurrence_id=native.occurrence_id
LEFT JOIN logiqx_disk_claims AS disk ON disk.occurrence_id=native.occurrence_id
LEFT JOIN logiqx_sample_claims AS sample ON sample.occurrence_id=native.occurrence_id
LEFT JOIN logiqx_file_merges AS merges ON merges.occurrence_id=native.occurrence_id
LEFT JOIN catalog_relationships AS registry ON registry.relationship_id=merges.relationship_id
LEFT JOIN reported_catalog_relationships AS reported ON reported.relationship_id=merges.relationship_id
WHERE native.record_id=requested.owner_id ORDER BY native.record_id, native.occurrence_order";

fn requested(count: usize) -> String {
    let values = std::iter::repeat_n("(?)", count)
        .collect::<Vec<_>>()
        .join(",");
    format!("WITH requested(owner_id) AS (VALUES {values})")
}

pub(super) fn select(count: usize) -> String {
    format!("{} {COLUMNS}", requested(count))
}

pub(super) fn attribute_select(table: &str, count: usize) -> String {
    format!(
        "{} SELECT native.occurrence_id AS owner_id, 0 AS child_order, native.field_kind, native.source_order, \
         native.source_line AS line, native.source_column AS column, \
         typeof(native.field_kind)='integer' AND typeof(native.source_order)='integer' \
         AND typeof(native.source_line)='integer' AND typeof(native.source_column)='integer' AS valid \
         FROM requested CROSS JOIN asset_occurrences AS occurrences CROSS JOIN {table} AS native \
         WHERE occurrences.record_id=requested.owner_id AND native.occurrence_id=occurrences.occurrence_id \
         ORDER BY native.occurrence_id, native.source_order",
        requested(count)
    )
}

fn position_map(
    connection: &mut SqliteConnection,
    table: &str,
    ids: &[i64],
) -> QueryResult<PositionMap> {
    Ok(positions::group(batch::<PositionRow>(
        connection,
        &attribute_select(table, ids.len()),
        ids,
    )?))
}

pub(super) fn load(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    ids: &[i64],
    indexes: &BTreeMap<i64, usize>,
    games: &mut [LogiqxGame],
) -> QueryResult<()> {
    let rows = batch::<MediaRow>(connection, &select(ids.len()), ids)?;
    let mut rom_positions = position_map(connection, "logiqx_rom_attribute_positions", ids)?;
    let mut disk_positions = position_map(connection, "logiqx_disk_attribute_positions", ids)?;
    let mut sample_positions = position_map(connection, "logiqx_sample_attribute_positions", ids)?;
    for row in rows {
        let owner = row.occurrence_id;
        if owner <= 0 || row.native_count != 1 || row.valid != 1 {
            return Err(LogiqxQueryError::MismatchedOwner(owner));
        }
        let (kind, fields, map) = match row.claim_kind.as_str() {
            "logiqx_rom" => (LogiqxMediaKind::Rom, 9, &mut rom_positions),
            "logiqx_disk" => (LogiqxMediaKind::Disk, 5, &mut disk_positions),
            "logiqx_sample" => (LogiqxMediaKind::Sample, 1, &mut sample_positions),
            _ => return Err(LogiqxQueryError::MismatchedOwner(owner)),
        };
        if row.merge_present == 1
            && (row.merge_kind.as_deref() != Some(row.claim_kind.as_str())
                || row.merge_snapshot.as_deref() != Some(snapshot.as_str())
                || row.merge_origin.as_deref() != Some("source")
                || row.merge_reference_kind.as_deref()
                    != Some(format!("{}_merge", row.claim_kind).as_str()))
        {
            return Err(LogiqxQueryError::MismatchedOwner(owner));
        }
        let expected = (0..fields)
            .map(|code| row.expected_fields & (1 << code) != 0)
            .collect::<Vec<_>>();
        positions::validate(
            &map.remove(&(owner, 0)).unwrap_or_default(),
            &expected,
            owner,
        )?;
        let source_order = row.source_order.ok_or(LogiqxQueryError::MissingNative {
            field: "media source order",
            owner,
        })?;
        let media_location = media_location(&row)?;
        let game = game_mut(games, indexes, row.owner_id)?;
        family_order(
            row.occurrence_order,
            game.media.len(),
            source_order,
            game.media.last().map(|media| media.source_order),
            row.owner_id,
        )?;
        game.media.push(LogiqxMediaReference {
            occurrence_id: OccurrenceId::try_from(owner)?,
            occurrence_order: row.occurrence_order,
            kind,
            source_order,
            location: media_location,
        });
    }
    positions::finish(&rom_positions)?;
    positions::finish(&disk_positions)?;
    positions::finish(&sample_positions)
}

fn media_location(row: &MediaRow) -> QueryResult<RecordLocation> {
    let owner = row.occurrence_id;
    location(
        row.line.ok_or(LogiqxQueryError::MissingNative {
            field: "media line",
            owner,
        })?,
        row.column.ok_or(LogiqxQueryError::MissingNative {
            field: "media column",
            owner,
        })?,
        owner,
    )
}
