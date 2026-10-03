//! Bounded native Logiqx payloads, independent of retained source documents.
use std::collections::BTreeMap;

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};

use crate::logiqx::{DiskAttribute, NameAttribute, RomAttribute};

use super::{
    CatalogFileOccurrence, CatalogFilesError, OccurrenceKind, SetGroupKind, SourceElementKind,
    SourceLocation, XmlAttributePosition, XmlAttributeRow, invalid_value, required_native,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxDumpStatus {
    Good,
    BadDump,
    NoDump,
    CompatibilityVerified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxRomPayload {
    pub size_text: Option<String>,
    pub crc_text: Option<String>,
    pub sha1_text: Option<String>,
    pub md5_text: Option<String>,
    pub merge_name: Option<String>,
    pub status: LogiqxDumpStatus,
    pub status_was_present: bool,
    pub date: Option<String>,
    pub compatibility_serial: Option<String>,
    pub source_order: i64,
    pub attribute_positions: Vec<XmlAttributePosition<RomAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxDiskPayload {
    pub sha1_text: Option<String>,
    pub md5_text: Option<String>,
    pub merge_name: Option<String>,
    pub status: LogiqxDumpStatus,
    pub status_was_present: bool,
    pub source_order: i64,
    pub attribute_positions: Vec<XmlAttributePosition<DiskAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogiqxSamplePayload {
    pub source_order: i64,
    pub attribute_positions: Vec<XmlAttributePosition<NameAttribute>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogiqxFilePayload {
    Rom(LogiqxRomPayload),
    Disk(LogiqxDiskPayload),
    Sample(LogiqxSamplePayload),
}

#[derive(QueryableByName)]
struct PayloadRow {
    #[diesel(sql_type=BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type=Text)]
    claim_kind: String,
    #[diesel(sql_type=BigInt)]
    native_owner_count: i64,
    #[diesel(sql_type=Bool)]
    native_game_exists: bool,
    #[diesel(sql_type=Nullable<Text>)]
    native_kind: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    md5_text: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type=Nullable<Bool>)]
    status_was_present: Option<bool>,
    #[diesel(sql_type=Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type=Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type=Nullable<BigInt>)]
    source_order: Option<i64>,
}

pub(super) const PAYLOAD_SELECT: &str = "
SELECT occurrence.occurrence_id,occurrence.claim_kind,
       game.set_id IS NOT NULL AS native_game_exists,
       (rom.occurrence_id IS NOT NULL)+(disk.occurrence_id IS NOT NULL)+(sample.occurrence_id IS NOT NULL) AS native_owner_count,
       CASE WHEN rom.occurrence_id IS NOT NULL THEN 'logiqx_rom'
            WHEN disk.occurrence_id IS NOT NULL THEN 'logiqx_disk'
            WHEN sample.occurrence_id IS NOT NULL THEN 'logiqx_sample' END AS native_kind,
       rom.size_text,rom.crc_text,COALESCE(rom.sha1_text,disk.sha1_text) AS sha1_text,
       COALESCE(rom.md5_text,disk.md5_text) AS md5_text,merges.merge_name,
       COALESCE(rom.dump_status,disk.dump_status) AS dump_status,
       COALESCE(rom.status_was_present,disk.status_was_present) AS status_was_present,
       rom.date,rom.serial,COALESCE(rom.source_order,disk.source_order,sample.source_order) AS source_order
FROM temp.catalog_files_requested_occurrences AS requested
CROSS JOIN asset_occurrences AS occurrence
LEFT JOIN logiqx_games AS game ON game.set_id=occurrence.record_id
LEFT JOIN logiqx_rom_claims AS rom USING(occurrence_id)
LEFT JOIN logiqx_disk_claims AS disk USING(occurrence_id)
LEFT JOIN logiqx_sample_claims AS sample USING(occurrence_id)
LEFT JOIN logiqx_file_merges AS merges USING(occurrence_id)
WHERE occurrence.occurrence_id=requested.occurrence_id
  AND (occurrence.claim_kind IN ('logiqx_rom','logiqx_disk','logiqx_sample')
       OR rom.occurrence_id IS NOT NULL OR disk.occurrence_id IS NOT NULL OR sample.occurrence_id IS NOT NULL)
ORDER BY occurrence.occurrence_id
";

fn status(value: Option<String>) -> Result<LogiqxDumpStatus, CatalogFilesError> {
    let value = required_native(value, "Logiqx dump status")?;
    match value.as_str() {
        "good" => Ok(LogiqxDumpStatus::Good),
        "baddump" => Ok(LogiqxDumpStatus::BadDump),
        "nodump" => Ok(LogiqxDumpStatus::NoDump),
        "verified" => Ok(LogiqxDumpStatus::CompatibilityVerified),
        _ => Err(invalid_value("Logiqx dump status", value)),
    }
}

pub(super) fn attribute_select(table: &'static str) -> String {
    format!(
        "SELECT positions.occurrence_id,positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM temp.catalog_files_requested_occurrences AS requested CROSS JOIN {table} AS positions WHERE positions.occurrence_id=requested.occurrence_id ORDER BY positions.occurrence_id,positions.source_order"
    )
}

fn positions<Field: Copy + PartialEq>(
    conn: &mut SqliteConnection,
    table: &'static str,
    decode: impl Fn(i64) -> Option<Field>,
    name: Field,
) -> Result<BTreeMap<i64, Vec<XmlAttributePosition<Field>>>, CatalogFilesError> {
    let rows = sql_query(attribute_select(table)).load::<XmlAttributeRow>(conn)?;
    let mut result = BTreeMap::<i64, Vec<XmlAttributePosition<Field>>>::new();
    for row in rows {
        let field = decode(row.field_kind)
            .ok_or_else(|| invalid_value("Logiqx attribute kind", row.field_kind.to_string()))?;
        if row.source_order < 0 || row.source_line <= 0 || row.source_column <= 0 {
            return Err(invalid_value(
                "Logiqx attribute position",
                row.occurrence_id.to_string(),
            ));
        }
        result
            .entry(row.occurrence_id)
            .or_default()
            .push(XmlAttributePosition {
                field,
                source_order: row.source_order,
                location: SourceLocation {
                    line: row.source_line,
                    column: row.source_column,
                },
            });
    }
    for (owner, fields) in &result {
        if !fields.iter().any(|position| position.field == name) {
            return Err(invalid_value(
                "missing Logiqx name position",
                owner.to_string(),
            ));
        }
    }
    Ok(result)
}

pub(super) fn attach_payloads(
    conn: &mut SqliteConnection,
    occurrences: &mut [CatalogFileOccurrence],
) -> Result<(), CatalogFilesError> {
    let mut rows = sql_query(PAYLOAD_SELECT)
        .load::<PayloadRow>(conn)?
        .into_iter()
        .map(|row| (row.occurrence_id, row))
        .collect::<BTreeMap<_, _>>();
    let mut rom_positions = positions(
        conn,
        "logiqx_rom_attribute_positions",
        RomAttribute::from_code,
        RomAttribute::Name,
    )?;
    let mut disk_positions = positions(
        conn,
        "logiqx_disk_attribute_positions",
        DiskAttribute::from_code,
        DiskAttribute::Name,
    )?;
    let mut sample_positions = positions(
        conn,
        "logiqx_sample_attribute_positions",
        NameAttribute::from_code,
        NameAttribute::Name,
    )?;
    for occurrence in occurrences {
        let id = occurrence.occurrence_id.database_value();
        let Some(row) = rows.remove(&id) else {
            continue;
        };
        if row.native_owner_count != 1
            || !row.native_game_exists
            || row.native_kind.as_deref() != Some(&row.claim_kind)
            || occurrence.provenance.source_element_kind != SourceElementKind::LogiqxGame
            || occurrence.provenance.format != "logiqx"
            || occurrence.provenance.set_group_kind != SetGroupKind::Root
        {
            return Err(invalid_value("Logiqx native owner", id.to_string()));
        }
        let source_order = required_native(row.source_order, "Logiqx child order")?;
        let missing = || invalid_value("missing Logiqx attribute positions", id.to_string());
        occurrence.logiqx_file = Some(match occurrence.provenance.occurrence_kind {
            OccurrenceKind::LogiqxRom => LogiqxFilePayload::Rom(LogiqxRomPayload {
                size_text: row.size_text,
                crc_text: row.crc_text,
                sha1_text: row.sha1_text,
                md5_text: row.md5_text,
                merge_name: row.merge_name,
                status: status(row.dump_status)?,
                status_was_present: required_native(
                    row.status_was_present,
                    "Logiqx explicit status",
                )?,
                date: row.date,
                compatibility_serial: row.serial,
                source_order,
                attribute_positions: rom_positions.remove(&id).ok_or_else(missing)?,
            }),
            OccurrenceKind::LogiqxDisk => LogiqxFilePayload::Disk(LogiqxDiskPayload {
                sha1_text: row.sha1_text,
                md5_text: row.md5_text,
                merge_name: row.merge_name,
                status: status(row.dump_status)?,
                status_was_present: required_native(
                    row.status_was_present,
                    "Logiqx explicit status",
                )?,
                source_order,
                attribute_positions: disk_positions.remove(&id).ok_or_else(missing)?,
            }),
            OccurrenceKind::LogiqxSample => LogiqxFilePayload::Sample(LogiqxSamplePayload {
                source_order,
                attribute_positions: sample_positions.remove(&id).ok_or_else(missing)?,
            }),
            _ => return Err(invalid_value("Logiqx occurrence kind", id.to_string())),
        });
    }
    if let Some(id) = rows
        .keys()
        .chain(rom_positions.keys())
        .chain(disk_positions.keys())
        .chain(sample_positions.keys())
        .next()
    {
        return Err(CatalogFilesError::MissingOccurrenceOwner(*id));
    }
    Ok(())
}
