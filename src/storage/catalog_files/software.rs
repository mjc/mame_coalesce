use crate::{
    mame_softwarelist::{SoftwareDiskAttribute, SoftwareRomAttribute},
    storage::software_attributes::{Family, Positions},
};
use std::collections::BTreeMap;

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use super::{
    CatalogFileOccurrence, CatalogFilesError, OccurrenceId, OccurrenceKind, SetGroupKind,
    SoftwareAreaKind, SoftwareAssetOwner, SoftwareDumpStatus, SoftwareLoadInstruction,
    SourceElementKind, SourceLocation,
};

#[cfg(test)]
#[path = "software/tests.rs"]
mod tests;

/// Native operation recorded for one ROM occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoftwareFileOperation {
    Load,
    Continue,
    Reload,
    ReloadPlain,
    Ignore,
    Fill,
}

impl SoftwareFileOperation {
    pub(crate) const fn from_instruction(instruction: Option<SoftwareLoadInstruction>) -> Self {
        use SoftwareLoadInstruction as Load;
        match instruction {
            Some(Load::Continue) => Self::Continue,
            Some(Load::Reload) => Self::Reload,
            Some(Load::ReloadPlain) => Self::ReloadPlain,
            Some(Load::Ignore) => Self::Ignore,
            Some(Load::Fill) => Self::Fill,
            Some(
                Load::Load16Byte
                | Load::Load16Word
                | Load::Load16WordSwap
                | Load::Load32Byte
                | Load::Load32Word
                | Load::Load32WordSwap
                | Load::Load32Dword
                | Load::Load64Word
                | Load::Load64WordSwap,
            )
            | None => Self::Load,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Continue => "continue",
            Self::Reload => "reload",
            Self::ReloadPlain => "reload_plain",
            Self::Ignore => "ignore",
            Self::Fill => "fill",
        }
    }
}

/// Native metadata for a ROM entry or ROM operation in a software list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareRomPayload {
    pub attribute_positions:
        Vec<super::XmlAttributePosition<crate::mame_softwarelist::SoftwareRomAttribute>>,
    pub name: Option<String>,
    pub size_text: Option<String>,
    pub size: Option<i64>,
    pub offset_text: Option<String>,
    pub offset: Option<i64>,
    pub value: Option<String>,
    pub crc_text: Option<String>,
    pub sha1_text: Option<String>,
    pub status: SoftwareDumpStatus,
    pub status_specified: bool,
    pub load_instruction: Option<SoftwareLoadInstruction>,
    pub evidence_scope: String,
    pub component_order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
    /// The declaration occurrence for this row, or its own ID when it declares the file.
    pub declaration_occurrence_id: Option<OccurrenceId>,
    pub operation: SoftwareFileOperation,
}

/// Native metadata for a disk entry in a software list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareDiskPayload {
    pub attribute_positions:
        Vec<super::XmlAttributePosition<crate::mame_softwarelist::SoftwareDiskAttribute>>,
    pub name: String,
    pub sha1_text: Option<String>,
    pub status: SoftwareDumpStatus,
    pub status_specified: bool,
    pub writeable: bool,
    pub writeable_specified: bool,
    pub evidence_scope: String,
    pub component_order: i64,
    pub source_order: i64,
    pub location: SourceLocation,
}

/// Native software-list file payload attached to one source occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareFilePayload {
    Rom(SoftwareRomPayload),
    Disk(SoftwareDiskPayload),
}

#[derive(QueryableByName)]
struct RomRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_record_id: i64,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = BigInt)]
    area_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    data_area_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_area_id: Option<i64>,
    #[diesel(sql_type = BigInt)]
    area_record_id: i64,
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    part_record_id: i64,
    #[diesel(sql_type = Text)]
    area_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    area_name: Option<String>,
    #[diesel(sql_type = BigInt)]
    area_order: i64,
    #[diesel(sql_type = Text)]
    part_name: String,
    #[diesel(sql_type = BigInt)]
    part_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    offset_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    offset: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Text)]
    dump_status: String,
    #[diesel(sql_type = BigInt)]
    status_specified: i64,
    #[diesel(sql_type = Nullable<Text>)]
    load_instruction: Option<String>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_occurrence_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    use_record_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    actual_declaration_occurrence_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_record_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_rom_record_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_rom_area_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    declaration_claim_kind: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    operation: Option<String>,
}

#[derive(QueryableByName)]
struct DiskRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_record_id: i64,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = BigInt)]
    area_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    data_area_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_area_id: Option<i64>,
    #[diesel(sql_type = BigInt)]
    area_record_id: i64,
    #[diesel(sql_type = BigInt)]
    part_id: i64,
    #[diesel(sql_type = BigInt)]
    part_record_id: i64,
    #[diesel(sql_type = Text)]
    area_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    area_name: Option<String>,
    #[diesel(sql_type = BigInt)]
    area_order: i64,
    #[diesel(sql_type = Text)]
    part_name: String,
    #[diesel(sql_type = BigInt)]
    part_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<Text>)]
    sha1_text: Option<String>,
    #[diesel(sql_type = Text)]
    dump_status: String,
    #[diesel(sql_type = BigInt)]
    status_specified: i64,
    #[diesel(sql_type = BigInt)]
    writeable: i64,
    #[diesel(sql_type = BigInt)]
    writeable_specified: i64,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    declaration_occurrence_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    use_record_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    operation: Option<String>,
}

pub(super) fn attach_payloads(
    connection: &mut SqliteConnection,
    occurrences: &mut [CatalogFileOccurrence],
) -> Result<(), CatalogFilesError> {
    let by_id = occurrences
        .iter()
        .enumerate()
        .map(|(index, occurrence)| (occurrence.occurrence_id.database_value(), index))
        .collect::<BTreeMap<_, _>>();

    let mut native_roms = sql_query(rom_select())
        .load::<RomRow>(connection)?
        .into_iter()
        .map(|row| {
            let occurrence_id = row.occurrence_id;
            let index = *by_id
                .get(&occurrence_id)
                .ok_or(CatalogFilesError::MissingOccurrenceOwner(occurrence_id))?;
            let occurrence = occurrences
                .get(index)
                .ok_or(CatalogFilesError::MissingOccurrenceOwner(occurrence_id))?;
            let row_owner = software_owner(
                row.area_id,
                row.area_kind.as_str(),
                super::required_native(row.area_name.clone(), "software data area name")?,
                row.area_order,
                row.part_id,
                row.part_name.clone(),
                row.part_order,
            )?;
            validate_raw_owner(occurrence, row.claim_kind.as_str(), &row_owner)?;
            Ok((occurrence_id, rom_payload(row)?))
        })
        .collect::<Result<BTreeMap<_, _>, CatalogFilesError>>()?;
    let mut native_disks = sql_query(disk_select())
        .load::<DiskRow>(connection)?
        .into_iter()
        .map(|row| {
            let occurrence_id = row.occurrence_id;
            let index = *by_id
                .get(&occurrence_id)
                .ok_or(CatalogFilesError::MissingOccurrenceOwner(occurrence_id))?;
            let occurrence = occurrences
                .get(index)
                .ok_or(CatalogFilesError::MissingOccurrenceOwner(occurrence_id))?;
            let row_owner = software_owner(
                row.area_id,
                row.area_kind.as_str(),
                super::required_native(row.area_name.clone(), "software disk area name")?,
                row.area_order,
                row.part_id,
                row.part_name.clone(),
                row.part_order,
            )?;
            validate_raw_owner(occurrence, row.claim_kind.as_str(), &row_owner)?;
            Ok((occurrence_id, disk_payload(row)?))
        })
        .collect::<Result<BTreeMap<_, _>, CatalogFilesError>>()?;

    attach_attribute_positions(connection, &mut native_roms, &mut native_disks)?;
    for occurrence in occurrences {
        let id = occurrence.occurrence_id.database_value();
        let payload = match occurrence.provenance.occurrence_kind {
            OccurrenceKind::SoftwareRomEntry | OccurrenceKind::SoftwareRomOperation => {
                Some(SoftwareFilePayload::Rom(
                    native_roms
                        .remove(&id)
                        .ok_or(CatalogFilesError::MissingSoftwareFilePayload(id))?,
                ))
            }
            OccurrenceKind::SoftwareDiskEntry => Some(SoftwareFilePayload::Disk(
                native_disks
                    .remove(&id)
                    .ok_or(CatalogFilesError::MissingSoftwareFilePayload(id))?,
            )),
            _ if native_roms.contains_key(&id) || native_disks.contains_key(&id) => {
                return Err(CatalogFilesError::MismatchedSoftwareFileOwner(id));
            }
            _ => None,
        };
        if let Some(payload) = payload {
            if let Some(owner) = &occurrence.provenance.software_owner {
                validate_payload_owner(occurrence, owner, &payload)?;
            } else {
                return Err(CatalogFilesError::MismatchedSoftwareFileOwner(id));
            }
            occurrence.software_file = Some(payload);
        }
    }

    if let Some((&id, _)) = native_roms.first_key_value() {
        return Err(CatalogFilesError::MissingOccurrenceOwner(id));
    }
    if let Some((&id, _)) = native_disks.first_key_value() {
        return Err(CatalogFilesError::MissingOccurrenceOwner(id));
    }
    Ok(())
}

fn attach_attribute_positions(
    connection: &mut SqliteConnection,
    native_roms: &mut BTreeMap<i64, SoftwareRomPayload>,
    native_disks: &mut BTreeMap<i64, SoftwareDiskPayload>,
) -> Result<(), CatalogFilesError> {
    let mut positions = Positions::default();
    for family in [Family::Rom, Family::Disk] {
        positions.load(connection,family,"FROM temp.catalog_files_requested_occurrences AS requested CROSS JOIN __NATIVE_POSITIONS__ AS position WHERE position.occurrence_id=requested.occurrence_id")?;
    }
    for (&id, payload) in native_roms {
        payload.attribute_positions = positions.take(
            Family::Rom,
            (id, 0, 0),
            SoftwareRomAttribute::from_code,
            &[
                payload.name.is_some(),
                payload.size_text.is_some(),
                payload.crc_text.is_some(),
                payload.sha1_text.is_some(),
                payload.offset_text.is_some(),
                payload.value.is_some(),
                payload.status_specified,
                payload.load_instruction.is_some(),
            ],
        )?;
    }
    for (&id, payload) in native_disks {
        payload.attribute_positions = positions.take(
            Family::Disk,
            (id, 0, 0),
            SoftwareDiskAttribute::from_code,
            &[
                true,
                payload.sha1_text.is_some(),
                payload.status_specified,
                payload.writeable_specified,
            ],
        )?;
    }
    positions.finish()?;
    Ok(())
}

fn rom_select() -> String {
    "SELECT rom.occurrence_id, occurrence.record_id AS occurrence_record_id, occurrence.claim_kind, \
            rom.record_id, rom.area_id, data_area.area_id AS data_area_id, disk_area.area_id AS disk_area_id, \
            area.record_id AS area_record_id, area.part_id, \
            part.record_id AS part_record_id, area.area_kind, data_area.area_name, area.area_order, \
            part.part_name, part.part_order, rom.name, rom.size_text, rom.size, \
            rom.offset_text, rom.offset, rom.value, rom.crc_text, rom.sha1_text, rom.dump_status, \
            rom.status_specified, rom.load_instruction, rom.evidence_scope, rom.component_order, \
            rom.source_order, rom.source_line, rom.source_column, file_use.declaration_occurrence_id, \
            file_use.record_id AS use_record_id, declaration.occurrence_id AS actual_declaration_occurrence_id, \
            declaration.record_id AS declaration_record_id, declared_rom.record_id AS declaration_rom_record_id, \
            declared_rom.area_id AS declaration_rom_area_id, declared_occurrence.claim_kind AS declaration_claim_kind, \
            file_use.operation \
     FROM temp.catalog_files_requested_occurrences AS requested \
     CROSS JOIN asset_occurrences AS occurrence \
     JOIN software_rom_entries AS rom ON rom.occurrence_id = requested.occurrence_id \
     JOIN software_areas AS area ON area.area_id = rom.area_id \
     LEFT JOIN software_data_areas AS data_area ON data_area.area_id = area.area_id \
     LEFT JOIN software_disk_areas AS disk_area ON disk_area.area_id = area.area_id \
     JOIN software_parts AS part ON part.part_id = area.part_id \
     LEFT JOIN software_file_uses AS file_use ON file_use.occurrence_id = rom.occurrence_id \
     LEFT JOIN software_file_declarations AS declaration \
       ON declaration.occurrence_id = file_use.declaration_occurrence_id \
     LEFT JOIN software_rom_entries AS declared_rom \
       ON declared_rom.occurrence_id = declaration.occurrence_id \
     LEFT JOIN asset_occurrences AS declared_occurrence \
       ON declared_occurrence.occurrence_id = declaration.occurrence_id \
     WHERE occurrence.occurrence_id = requested.occurrence_id \
       AND occurrence.occurrence_id = rom.occurrence_id \
     ORDER BY rom.occurrence_id"
        .to_owned()
}

fn disk_select() -> String {
    "SELECT disk.occurrence_id, occurrence.record_id AS occurrence_record_id, occurrence.claim_kind, \
            disk.record_id, disk.area_id, data_area.area_id AS data_area_id, disk_area.area_id AS disk_area_id, \
            area.record_id AS area_record_id, area.part_id, \
            part.record_id AS part_record_id, area.area_kind, disk_area.area_name, area.area_order, \
            part.part_name, part.part_order, disk.name, disk.sha1_text, \
            disk.dump_status, disk.status_specified, disk.writeable, disk.writeable_specified, \
            disk.evidence_scope, disk.component_order, disk.source_order, disk.source_line, \
            disk.source_column, file_use.declaration_occurrence_id, \
            file_use.record_id AS use_record_id, file_use.operation \
     FROM temp.catalog_files_requested_occurrences AS requested \
     CROSS JOIN asset_occurrences AS occurrence \
     JOIN software_disk_entries AS disk ON disk.occurrence_id = requested.occurrence_id \
     JOIN software_areas AS area ON area.area_id = disk.area_id \
     LEFT JOIN software_disk_areas AS disk_area ON disk_area.area_id = area.area_id \
     LEFT JOIN software_data_areas AS data_area ON data_area.area_id = area.area_id \
     JOIN software_parts AS part ON part.part_id = area.part_id \
     LEFT JOIN software_file_uses AS file_use ON file_use.occurrence_id = disk.occurrence_id \
     WHERE occurrence.occurrence_id = requested.occurrence_id \
       AND occurrence.occurrence_id = disk.occurrence_id \
     ORDER BY disk.occurrence_id"
        .to_owned()
}

fn rom_payload(row: RomRow) -> Result<SoftwareRomPayload, CatalogFilesError> {
    validate_record_links(
        row.occurrence_id,
        row.occurrence_record_id,
        row.record_id,
        row.area_record_id,
        row.part_record_id,
        row.claim_kind.as_str(),
        SoftwareAreaKind::Data,
    )?;
    if row.area_kind != "data"
        || super::super::software_area::native_kind(row.area_id, row.data_area_id, row.disk_area_id)
            != Some(crate::mame_softwarelist::AreaKind::Data)
    {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            row.occurrence_id,
        ));
    }
    if row.use_record_id != Some(row.record_id) {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            row.occurrence_id,
        ));
    }
    let operation = parse_operation(row.operation.as_deref().ok_or_else(|| {
        invalid_value(
            "software file operation",
            "NULL for ROM occurrence".to_owned(),
        )
    })?)?;
    let load_instruction = row
        .load_instruction
        .as_deref()
        .map(parse_load_instruction)
        .transpose()?;
    let instruction_operation = SoftwareFileOperation::from_instruction(load_instruction);
    if operation != instruction_operation {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            row.occurrence_id,
        ));
    }
    validate_rom_declaration_link(&row, operation)?;
    let status = parse_dump_status(&row.dump_status)?;
    let evidence = super::super::software_rom_evidence::RomEvidence::classify(
        row.claim_kind == "software_rom_entry",
        row.name.as_deref(),
        status == SoftwareDumpStatus::NoDump,
    );
    if !evidence.accepts_scope(&row.evidence_scope) {
        return Err(invalid_value(
            "software ROM evidence scope",
            row.evidence_scope,
        ));
    }

    Ok(SoftwareRomPayload {
        attribute_positions: Vec::new(),
        name: row.name,
        size_text: row.size_text,
        size: row.size,
        offset_text: row.offset_text,
        offset: row.offset,
        value: row.value,
        crc_text: row.crc_text,
        sha1_text: row.sha1_text,
        status,
        status_specified: stored_bool(row.status_specified, "software ROM status_specified")?,
        load_instruction,
        evidence_scope: row.evidence_scope,
        component_order: row.component_order,
        source_order: row.source_order,
        location: SourceLocation {
            line: row.source_line,
            column: row.source_column,
        },
        declaration_occurrence_id: row
            .declaration_occurrence_id
            .map(OccurrenceId::from_database),
        operation,
    })
}

fn validate_rom_declaration_link(
    row: &RomRow,
    operation: SoftwareFileOperation,
) -> Result<(), CatalogFilesError> {
    let declaration_id = row.declaration_occurrence_id;
    match row.claim_kind.as_str() {
        "software_rom_entry"
            if operation != SoftwareFileOperation::Load
                || declaration_id != Some(row.occurrence_id) =>
        {
            return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
                row.occurrence_id,
            ));
        }
        "software_rom_operation"
            if declaration_id == Some(row.occurrence_id)
                || (matches!(
                    operation,
                    SoftwareFileOperation::Load | SoftwareFileOperation::Fill
                ) && declaration_id.is_some()) =>
        {
            return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
                row.occurrence_id,
            ));
        }
        _ => {}
    }
    if let Some(id) = row.declaration_occurrence_id {
        if row.actual_declaration_occurrence_id != Some(id)
            || row.declaration_record_id != Some(row.record_id)
            || row.declaration_rom_record_id != Some(row.record_id)
            || row.declaration_rom_area_id != Some(row.area_id)
            || row.declaration_claim_kind.as_deref() != Some("software_rom_entry")
        {
            return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
                row.occurrence_id,
            ));
        }
    } else if row.actual_declaration_occurrence_id.is_some()
        || row.declaration_record_id.is_some()
        || row.declaration_rom_record_id.is_some()
        || row.declaration_rom_area_id.is_some()
        || row.declaration_claim_kind.is_some()
    {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            row.occurrence_id,
        ));
    }

    Ok(())
}

fn disk_payload(row: DiskRow) -> Result<SoftwareDiskPayload, CatalogFilesError> {
    validate_record_links(
        row.occurrence_id,
        row.occurrence_record_id,
        row.record_id,
        row.area_record_id,
        row.part_record_id,
        row.claim_kind.as_str(),
        SoftwareAreaKind::Disk,
    )?;
    if row.area_kind != "disk"
        || super::super::software_area::native_kind(row.area_id, row.data_area_id, row.disk_area_id)
            != Some(crate::mame_softwarelist::AreaKind::Disk)
        || row.operation.as_deref() != Some("disk")
        || row.use_record_id != Some(row.record_id)
        || row.declaration_occurrence_id.is_some()
    {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            row.occurrence_id,
        ));
    }
    Ok(SoftwareDiskPayload {
        attribute_positions: Vec::new(),
        name: row.name,
        sha1_text: row.sha1_text,
        status: parse_dump_status(&row.dump_status)?,
        status_specified: stored_bool(row.status_specified, "software disk status_specified")?,
        writeable: stored_bool(row.writeable, "software disk writeable")?,
        writeable_specified: stored_bool(
            row.writeable_specified,
            "software disk writeable_specified",
        )?,
        evidence_scope: row.evidence_scope,
        component_order: row.component_order,
        source_order: row.source_order,
        location: SourceLocation {
            line: row.source_line,
            column: row.source_column,
        },
    })
}

fn validate_record_links(
    occurrence_id: i64,
    occurrence_record_id: i64,
    record_id: i64,
    area_record_id: i64,
    part_record_id: i64,
    claim_kind: &str,
    expected_area_kind: SoftwareAreaKind,
) -> Result<(), CatalogFilesError> {
    let claim_matches = match expected_area_kind {
        SoftwareAreaKind::Data => {
            matches!(claim_kind, "software_rom_entry" | "software_rom_operation")
        }
        SoftwareAreaKind::Disk => claim_kind == "software_disk_entry",
    };
    if !claim_matches
        || occurrence_record_id != record_id
        || record_id != area_record_id
        || area_record_id != part_record_id
    {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            occurrence_id,
        ));
    }
    Ok(())
}

fn validate_payload_owner(
    occurrence: &CatalogFileOccurrence,
    owner: &SoftwareAssetOwner,
    payload: &SoftwareFilePayload,
) -> Result<(), CatalogFilesError> {
    let occurrence_id = occurrence.occurrence_id.database_value();
    let expected_area_kind = match payload {
        SoftwareFilePayload::Rom(_) => SoftwareAreaKind::Data,
        SoftwareFilePayload::Disk(_) => SoftwareAreaKind::Disk,
    };
    let expected_occurrence_kind = match payload {
        SoftwareFilePayload::Rom(_) => matches!(
            occurrence.provenance.occurrence_kind,
            OccurrenceKind::SoftwareRomEntry | OccurrenceKind::SoftwareRomOperation
        ),
        SoftwareFilePayload::Disk(_) => {
            occurrence.provenance.occurrence_kind == OccurrenceKind::SoftwareDiskEntry
        }
    };
    if !matches!(
        &occurrence.provenance.set_group_kind,
        SetGroupKind::SoftwareList { .. }
    ) || occurrence.provenance.source_element_kind != SourceElementKind::SoftwareItem
        || !expected_occurrence_kind
        || owner.area_kind != expected_area_kind
        || owner.part_id <= 0
        || owner.area_id <= 0
        || owner.part_order < 0
        || owner.area_order < 0
    {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(
            occurrence_id,
        ));
    }
    Ok(())
}

fn validate_raw_owner(
    occurrence: &CatalogFileOccurrence,
    claim_kind: &str,
    row_owner: &SoftwareAssetOwner,
) -> Result<(), CatalogFilesError> {
    let id = occurrence.occurrence_id.database_value();
    let Some(owner) = &occurrence.provenance.software_owner else {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(id));
    };
    let claim_matches = match occurrence.provenance.occurrence_kind {
        OccurrenceKind::SoftwareRomEntry => claim_kind == "software_rom_entry",
        OccurrenceKind::SoftwareRomOperation => claim_kind == "software_rom_operation",
        OccurrenceKind::SoftwareDiskEntry => claim_kind == "software_disk_entry",
        _ => false,
    };
    if owner != row_owner || !claim_matches {
        return Err(CatalogFilesError::MismatchedSoftwareFileOwner(id));
    }
    Ok(())
}

fn software_owner(
    area_id: i64,
    area_kind: &str,
    area_name: String,
    area_order: i64,
    part_id: i64,
    part_name: String,
    part_order: i64,
) -> Result<SoftwareAssetOwner, CatalogFilesError> {
    Ok(SoftwareAssetOwner {
        area_id,
        area_kind: super::parse_software_area_kind(area_kind.to_owned())?,
        area_name,
        area_order,
        part_id,
        part_name,
        part_order,
    })
}

fn parse_dump_status(value: &str) -> Result<SoftwareDumpStatus, CatalogFilesError> {
    match value {
        "good" => Ok(SoftwareDumpStatus::Good),
        "baddump" => Ok(SoftwareDumpStatus::BadDump),
        "nodump" => Ok(SoftwareDumpStatus::NoDump),
        _ => Err(invalid_value("software dump status", value.to_owned())),
    }
}

fn parse_load_instruction(value: &str) -> Result<SoftwareLoadInstruction, CatalogFilesError> {
    SoftwareLoadInstruction::from_source_name(value)
        .ok_or_else(|| invalid_value("software load instruction", value.to_owned()))
}

fn parse_operation(value: &str) -> Result<SoftwareFileOperation, CatalogFilesError> {
    match value {
        "load" => Ok(SoftwareFileOperation::Load),
        "continue" => Ok(SoftwareFileOperation::Continue),
        "reload" => Ok(SoftwareFileOperation::Reload),
        "reload_plain" => Ok(SoftwareFileOperation::ReloadPlain),
        "ignore" => Ok(SoftwareFileOperation::Ignore),
        "fill" => Ok(SoftwareFileOperation::Fill),
        _ => Err(invalid_value("software file operation", value.to_owned())),
    }
}

fn stored_bool(value: i64, field: &'static str) -> Result<bool, CatalogFilesError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_value(field, value.to_string())),
    }
}

const fn invalid_value(field: &'static str, value: String) -> CatalogFilesError {
    CatalogFilesError::InvalidStoredValue { field, value }
}
