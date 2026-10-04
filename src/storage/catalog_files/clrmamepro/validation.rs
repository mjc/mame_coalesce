use std::collections::{BTreeMap, BTreeSet};

use super::super::{
    DigestAlgorithm, DigestProvenance, OccurrenceDigest, parse_algorithm, parse_provenance,
    required_native,
};
use super::{
    CatalogFilesError, ClrMameProDumpStatus, ClrMameProEvidenceScope, ClrMameProFieldPosition,
    ClrMameProFilePayload, ClrMameProPositionedField, ClrMameProRomField, ClrMameProRomPayload,
    ClrMameProSamplePayload, OccurrenceId, Payloads, SourceLocation, invalid_value,
    rows::{DigestRow, OwnerRow, PositionRow, RomRow, SampleRow},
};

pub(super) fn owners(rows: &[OwnerRow]) -> Result<(), CatalogFilesError> {
    let mut ids = BTreeSet::new();
    for row in rows {
        let matches = match row.claim_kind.as_deref() {
            Some("cmp_rom") => row.rom_id == Some(row.occurrence_id) && row.sample_id.is_none(),
            Some("cmp_sample") => row.sample_id == Some(row.occurrence_id) && row.rom_id.is_none(),
            _ => false,
        };
        if !row.valid_owner || !row.valid_identity || !matches || !ids.insert(row.occurrence_id) {
            return Err(CatalogFilesError::MismatchedClrMameProFileOwner(
                row.occurrence_id,
            ));
        }
    }
    Ok(())
}

pub(super) fn rom(row: RomRow) -> Result<(OccurrenceId, ClrMameProFilePayload), CatalogFilesError> {
    if !row.valid_storage {
        return Err(invalid_value(
            "CMP ROM storage",
            row.occurrence_id.to_string(),
        ));
    }
    validate_size(row.size_text.as_deref(), row.size)?;
    for (field, value, algorithm) in [
        ("CMP CRC", row.crc_text.as_deref(), DigestAlgorithm::Crc32),
        (
            "CMP CRC32",
            row.crc32_text.as_deref(),
            DigestAlgorithm::Crc32,
        ),
        ("CMP MD5", row.md5_text.as_deref(), DigestAlgorithm::Md5),
        ("CMP SHA1", row.sha1_text.as_deref(), DigestAlgorithm::Sha1),
    ] {
        if let Some(value) = value {
            decode_digest(value, algorithm, field)?;
        }
    }
    let effective = effective_status(&row);
    if effective != row.dump_status.as_deref() {
        return Err(invalid_value(
            "CMP virtual dump status",
            row.occurrence_id.to_string(),
        ));
    }
    let dump_status = typed_status(effective);
    let evidence_scope = match row.evidence_scope.as_str() {
        "whole_file" => ClrMameProEvidenceScope::WholeFile,
        "whole_asset" => ClrMameProEvidenceScope::WholeAsset,
        _ => return Err(invalid_value("CMP evidence scope", row.evidence_scope)),
    };
    let payload = ClrMameProRomPayload {
        name: row.name,
        size_text: row.size_text,
        size: row.size,
        crc_text: row.crc_text,
        crc32_text: row.crc32_text,
        md5_text: row.md5_text,
        sha1_text: row.sha1_text,
        date: row.date,
        serial: row.serial,
        status_text: row.status_text,
        nodump_present: row.nodump_present,
        baddump_present: row.baddump_present,
        dump_status,
        evidence_scope,
        merge: None,
        source_order: checked_order(row.source_order)?,
        location: SourceLocation {
            line: row.source_line,
            column: row.source_column,
        },
        field_positions: Vec::new(),
    };
    Ok((
        OccurrenceId::from_database(row.occurrence_id),
        ClrMameProFilePayload::Rom(Box::new(payload)),
    ))
}

fn validate_size(text: Option<&str>, size: Option<i64>) -> Result<(), CatalogFilesError> {
    let expected = text
        .map(|value| {
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid_value("CMP size declaration", value.to_owned()));
            }
            value
                .parse::<i64>()
                .map_err(|_| invalid_value("CMP size declaration", value.to_owned()))
        })
        .transpose()?;
    if expected != size {
        return Err(invalid_value(
            "CMP virtual size",
            format!("{text:?} => {size:?}"),
        ));
    }
    Ok(())
}

fn effective_status(row: &RomRow) -> Option<&str> {
    if (row.nodump_present && row.baddump_present)
        || (row.status_text.is_some() && (row.nodump_present || row.baddump_present))
    {
        None
    } else if let Some(status) = row.status_text.as_deref() {
        Some(status)
    } else if row.nodump_present {
        Some("nodump")
    } else if row.baddump_present {
        Some("baddump")
    } else {
        None
    }
}

fn typed_status(value: Option<&str>) -> Option<ClrMameProDumpStatus> {
    match value {
        Some("good") => Some(ClrMameProDumpStatus::Good),
        Some("baddump") => Some(ClrMameProDumpStatus::BadDump),
        Some("nodump") => Some(ClrMameProDumpStatus::NoDump),
        Some("verified") => Some(ClrMameProDumpStatus::Verified),
        _ => None,
    }
}

pub(super) fn sample(
    row: SampleRow,
) -> Result<(OccurrenceId, ClrMameProFilePayload), CatalogFilesError> {
    if !row.valid_storage || !row.source_field.eq_ignore_ascii_case("sample") {
        return Err(invalid_value(
            "CMP sample storage",
            row.occurrence_id.to_string(),
        ));
    }
    let position = ClrMameProFieldPosition {
        source_field: row.source_field,
        source_order: checked_order(row.source_order)?,
        is_quoted: row.is_quoted,
        location: SourceLocation {
            line: row.source_line,
            column: row.source_column,
        },
    };
    Ok((
        OccurrenceId::from_database(row.occurrence_id),
        ClrMameProFilePayload::Sample(ClrMameProSamplePayload {
            name: row.sample_name,
            position,
        }),
    ))
}

fn checked_order(value: i64) -> Result<usize, CatalogFilesError> {
    usize::try_from(value).map_err(|_| invalid_value("CMP source order", value.to_string()))
}

pub(super) fn positions(
    payloads: &mut Payloads,
    rows: Vec<PositionRow>,
) -> Result<(), CatalogFilesError> {
    let mut seen = BTreeMap::<OccurrenceId, (BTreeSet<i64>, BTreeSet<usize>)>::new();
    for row in rows {
        let id = OccurrenceId::from_database(row.occurrence_id);
        let Some(ClrMameProFilePayload::Rom(payload)) = payloads.get_mut(&id) else {
            return Err(CatalogFilesError::MissingClrMameProFilePayload(
                row.occurrence_id,
            ));
        };
        let field = ClrMameProRomField::from_code(row.field_kind)
            .ok_or_else(|| invalid_value("CMP ROM field code", row.field_kind.to_string()))?;
        if !row.valid_storage
            || !row.source_field.eq_ignore_ascii_case(field.keyword())
            || (matches!(
                field,
                ClrMameProRomField::NoDump | ClrMameProRomField::BadDump
            ) && row.is_quoted)
        {
            return Err(invalid_value(
                "CMP ROM field position",
                row.occurrence_id.to_string(),
            ));
        }
        let source_order = checked_order(row.source_order)?;
        let (fields, orders) = seen.entry(id).or_default();
        if !fields.insert(row.field_kind) || !orders.insert(source_order) {
            return Err(invalid_value(
                "duplicate CMP ROM field position",
                row.occurrence_id.to_string(),
            ));
        }
        payload.field_positions.push(ClrMameProPositionedField {
            field,
            position: ClrMameProFieldPosition {
                source_field: row.source_field,
                source_order,
                is_quoted: row.is_quoted,
                location: SourceLocation {
                    line: row.source_line,
                    column: row.source_column,
                },
            },
        });
    }
    for (id, payload) in payloads {
        if let ClrMameProFilePayload::Rom(rom) = payload
            && seen.remove(id).map(|(fields, _)| fields) != Some(present_fields(rom))
        {
            return Err(invalid_value(
                "CMP ROM field presence",
                id.database_value().to_string(),
            ));
        }
    }
    Ok(())
}

fn present_fields(payload: &ClrMameProRomPayload) -> BTreeSet<i64> {
    use ClrMameProRomField as Field;
    [
        (Field::Name, true),
        (Field::Size, payload.size_text.is_some()),
        (Field::Crc, payload.crc_text.is_some()),
        (Field::Crc32, payload.crc32_text.is_some()),
        (Field::Md5, payload.md5_text.is_some()),
        (Field::Sha1, payload.sha1_text.is_some()),
        (Field::Merge, payload.merge.is_some()),
        (Field::Date, payload.date.is_some()),
        (Field::Serial, payload.serial.is_some()),
        (Field::Status, payload.status_text.is_some()),
        (Field::NoDump, payload.nodump_present),
        (Field::BadDump, payload.baddump_present),
    ]
    .into_iter()
    .filter_map(|(field, present)| present.then_some(field as i64))
    .collect()
}

fn decode_digest(
    text: &str,
    algorithm: DigestAlgorithm,
    field: &'static str,
) -> Result<Vec<u8>, CatalogFilesError> {
    if text.len() != algorithm.byte_length() * 2
        || !text.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(invalid_value(field, text.to_owned()));
    }
    hex::decode(text).map_err(|_| invalid_value(field, text.to_owned()))
}

pub(super) fn digests_and_identity(
    payloads: &Payloads,
    owners: &[OwnerRow],
    rows: Vec<DigestRow>,
) -> Result<(), CatalogFilesError> {
    let mut by_id = BTreeMap::<OccurrenceId, Vec<OccurrenceDigest>>::new();
    for row in rows {
        let id = OccurrenceId::from_database(row.occurrence_id);
        // The general file API accepts mixed IDs. Non-CMP assertions belong to its other loaders.
        if !payloads.contains_key(&id) {
            continue;
        }
        by_id.entry(id).or_default().push(digest(row)?);
    }
    for owner in owners {
        let id = OccurrenceId::from_database(owner.occurrence_id);
        let payload = payloads
            .get(&id)
            .ok_or(CatalogFilesError::MissingClrMameProFilePayload(
                owner.occurrence_id,
            ))?;
        let rows = by_id.remove(&id).unwrap_or_default();
        let source = rows
            .iter()
            .filter(|row| row.provenance == DigestProvenance::SourceDeclared)
            .collect::<Vec<_>>();
        match payload {
            ClrMameProFilePayload::Rom(rom) => {
                validate_source_digests(rom, &source)?;
                if owner.content_uuid.is_some() && !identity_eligible(rom, &source) {
                    return Err(invalid_value(
                        "CMP UUID source evidence",
                        owner.occurrence_id.to_string(),
                    ));
                }
            }
            ClrMameProFilePayload::Sample(_)
                if owner.content_uuid.is_some() || !source.is_empty() =>
            {
                return Err(invalid_value(
                    "CMP filename-only sample evidence",
                    owner.occurrence_id.to_string(),
                ));
            }
            ClrMameProFilePayload::Sample(_) => {}
        }
    }
    Ok(())
}

fn digest(row: DigestRow) -> Result<OccurrenceDigest, CatalogFilesError> {
    if !row.valid_storage {
        return Err(invalid_value(
            "CMP digest assertion storage",
            row.occurrence_id.to_string(),
        ));
    }
    let algorithm = parse_algorithm(required_native(row.algorithm, "CMP digest algorithm")?)?;
    let value = required_native(row.digest, "CMP digest dictionary owner")?;
    if value.len() != algorithm.byte_length() {
        return Err(CatalogFilesError::DigestLengthMismatch {
            occurrence_id: row.occurrence_id,
            algorithm,
            actual: value.len(),
            expected: algorithm.byte_length(),
        });
    }
    Ok(OccurrenceDigest {
        algorithm,
        value,
        scope: row.scope,
        provenance: parse_provenance(row.provenance)?,
    })
}

fn validate_source_digests(
    rom: &ClrMameProRomPayload,
    source: &[&OccurrenceDigest],
) -> Result<(), CatalogFilesError> {
    let expected = native_digests(rom)?;
    let matches = |row: &OccurrenceDigest, algorithm: DigestAlgorithm, value: &[u8]| {
        row.scope == rom.evidence_scope.as_str() && row.algorithm == algorithm && row.value == value
    };
    if expected
        .iter()
        .any(|(algorithm, value)| !source.iter().any(|row| matches(row, *algorithm, value)))
        || source.iter().any(|row| {
            !expected
                .iter()
                .any(|(algorithm, value)| matches(row, *algorithm, value))
        })
    {
        return Err(invalid_value(
            "CMP source digest agreement",
            rom.name.clone(),
        ));
    }
    Ok(())
}

fn native_digests(
    rom: &ClrMameProRomPayload,
) -> Result<Vec<(DigestAlgorithm, Vec<u8>)>, CatalogFilesError> {
    [
        (DigestAlgorithm::Crc32, &rom.crc_text),
        (DigestAlgorithm::Crc32, &rom.crc32_text),
        (DigestAlgorithm::Md5, &rom.md5_text),
        (DigestAlgorithm::Sha1, &rom.sha1_text),
    ]
    .into_iter()
    .filter_map(|(algorithm, text)| {
        text.as_deref().map(|value| {
            decode_digest(value, algorithm, "CMP native digest").map(|bytes| (algorithm, bytes))
        })
    })
    .collect()
}

fn identity_eligible(rom: &ClrMameProRomPayload, source: &[&OccurrenceDigest]) -> bool {
    let crc_conflict = rom
        .crc_text
        .as_ref()
        .zip(rom.crc32_text.as_ref())
        .is_some_and(|(crc, crc32)| !crc.eq_ignore_ascii_case(crc32));
    let dump_conflict = (rom.nodump_present && rom.baddump_present)
        || (rom.status_text.is_some() && (rom.nodump_present || rom.baddump_present));
    !crc_conflict
        && !dump_conflict
        && rom.sha1_text.is_some()
        && source.iter().any(|row| {
            row.algorithm == DigestAlgorithm::Sha1 && row.scope == rom.evidence_scope.as_str()
        })
}
