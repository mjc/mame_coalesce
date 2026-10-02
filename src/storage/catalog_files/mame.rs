//! Published native MAME ROM, disk and sample occurrence payloads.

use std::collections::BTreeMap;

use diesel::sql_types::{BigInt, Bool, Nullable, Text};
use diesel::{QueryableByName, RunQueryDsl, SqliteConnection, sql_query};

use crate::disk::DiskDigestScope;

use super::{CatalogFileOccurrence, CatalogFilesError, OccurrenceKind, SourceLocation};

pub use crate::mame::{MameAssetDeclarations, MameBoolean, MameDumpStatus};

/// MAME's native ROM claim, including source lexemes and normalized declarations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MameRomPayload {
    pub name: String,
    pub declarations: MameAssetDeclarations,
    pub size: Option<i64>,
    pub evidence_scope: MameRomEvidenceScope,
    pub evidence_provenance: String,
    pub merge_name: Option<String>,
    pub dump_status: MameDumpStatus,
    pub status_specified: bool,
    pub source_order: i64,
    pub location: SourceLocation,
    pub region: Option<String>,
    pub bios: Option<String>,
    pub optional: MameBoolean,
    pub optional_specified: bool,
    /// Compatibility attributes are exposed only for the exact parser rules version.
    pub compatibility: Option<MameRomCompatibility>,
}

/// MAME-specific historical ROM attributes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MameRomCompatibility {
    pub sound_only: Option<MameBoolean>,
    pub dispose: Option<MameBoolean>,
    pub load_flag: Option<String>,
    pub value: Option<String>,
    pub inverted: Option<MameBoolean>,
    pub ovha: Option<String>,
    pub no_thread: Option<MameBoolean>,
}

/// MAME's native disk claim. ROM-only fields are intentionally not present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MameDiskPayload {
    pub name: String,
    pub declarations: MameAssetDeclarations,
    pub evidence_scope: DiskDigestScope,
    pub evidence_provenance: String,
    pub merge_name: Option<String>,
    pub dump_status: MameDumpStatus,
    pub status_specified: bool,
    pub source_order: i64,
    pub location: SourceLocation,
    pub region: Option<String>,
    pub optional: MameBoolean,
    pub optional_specified: bool,
    pub disk_index: Option<String>,
    pub writable: MameBoolean,
    pub writable_specified: bool,
    /// Compatibility attributes are exposed only for the exact parser rules version.
    pub compatibility: Option<MameDiskCompatibility>,
}

/// Typed native MAME asset owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MameFilePayload {
    Rom(MameRomPayload),
    Disk(MameDiskPayload),
    Sample(MameSamplePayload),
}

/// A filename-only expected audio sample, with no inferred size or digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MameSamplePayload {
    pub name: String,
    pub source_order: i64,
    pub location: SourceLocation,
}

#[derive(QueryableByName)]
struct SamplePayloadRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    native_owner: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
}

const SAMPLE_SELECT: &str = "
SELECT occurrence.occurrence_id, occurrence.claim_kind, sample.occurrence_id AS native_owner,
       sample.name, sample.source_order, sample.source_line, sample.source_column
FROM temp.catalog_files_requested_occurrences AS requested
CROSS JOIN asset_occurrences AS occurrence
LEFT JOIN mame_samples AS sample USING(occurrence_id)
WHERE occurrence.occurrence_id=requested.occurrence_id
  AND (occurrence.claim_kind='mame_sample' OR sample.occurrence_id IS NOT NULL)
ORDER BY occurrence.occurrence_id
";

/// MAME-specific historical disk attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MameDiskCompatibility {
    pub writeable: MameBoolean,
}

/// ROM digest scope, normalized from the persisted declaration scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MameRomEvidenceScope {
    WholeFile,
    Unknown,
}

impl MameRomEvidenceScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WholeFile => "whole_asset",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(QueryableByName)]
struct MamePayloadRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    claim_kind: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_owner: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_offset_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_evidence_scope: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_evidence_provenance: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    rom_status_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_bios: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    rom_optional: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    rom_optional_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_md5_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_compatibility_owner: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_sound_only: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_dispose: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_load_flag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_value: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_inverted: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rom_ovha: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    rom_no_thread: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_owner: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_evidence_scope: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_evidence_provenance: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    disk_status_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_region: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    disk_optional: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    disk_optional_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    disk_index: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    disk_writable: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    disk_writable_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    disk_writeable: Option<i64>,
}

const SELECT: &str = "SELECT occurrence.occurrence_id, occurrence.claim_kind, \
    rom.occurrence_id AS rom_owner, rom.name AS rom_name, rom.size_text AS rom_size_text, \
    rom.size AS rom_size, rom.crc_text AS rom_crc_text, rom.sha1_text AS rom_sha1_text, \
    rom.offset_text AS rom_offset_text, \
    rom.evidence_scope AS rom_evidence_scope, rom.evidence_provenance AS rom_evidence_provenance, \
    rom.merge_name AS rom_merge_name, rom.dump_status AS rom_dump_status, \
    rom.status_specified AS rom_status_specified, rom.source_order AS rom_source_order, \
    rom.source_line AS rom_line, rom.source_column AS rom_column, rom.region AS rom_region, \
    rom.bios AS rom_bios, rom.optional AS rom_optional, rom.optional_specified AS rom_optional_specified, \
    rom_compat_raw.md5_text AS rom_md5_text, rom_compat.occurrence_id AS rom_compatibility_owner, \
    rom_compat.sound_only AS rom_sound_only, \
    rom_compat.dispose AS rom_dispose, rom_compat.load_flag AS rom_load_flag, \
    rom_compat.value AS rom_value, rom_compat.inverted AS rom_inverted, \
    rom_compat.ovha AS rom_ovha, rom_compat.no_thread AS rom_no_thread, \
    disk.occurrence_id AS disk_owner, disk.name AS disk_name, disk.sha1_text AS disk_sha1_text, \
    disk.evidence_scope AS disk_evidence_scope, disk.evidence_provenance AS disk_evidence_provenance, \
    disk.merge_name AS disk_merge_name, disk.dump_status AS disk_dump_status, \
    disk.status_specified AS disk_status_specified, disk.source_order AS disk_source_order, \
    disk.source_line AS disk_line, disk.source_column AS disk_column, disk.region AS disk_region, \
    disk.optional AS disk_optional, disk.optional_specified AS disk_optional_specified, \
    disk.disk_index AS disk_index, disk.writable AS disk_writable, \
    disk.writable_specified AS disk_writable_specified, disk_compat.writeable AS disk_writeable \
FROM temp.catalog_files_requested_occurrences AS requested \
CROSS JOIN asset_occurrences AS occurrence \
JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id \
JOIN catalog_set_groups AS groups ON groups.set_group_id = sets.set_group_id \
JOIN catalog_snapshots AS snapshots ON snapshots.snapshot_key = groups.snapshot_key \
JOIN snapshot_publications AS publication ON publication.snapshot_key = snapshots.snapshot_key \
JOIN parser_interpretations AS interpretation \
  ON interpretation.interpretation_key = snapshots.interpretation_key \
LEFT JOIN mame_rom_claims AS rom ON rom.occurrence_id = occurrence.occurrence_id \
LEFT JOIN mame_disk_claims AS disk ON disk.occurrence_id = occurrence.occurrence_id \
LEFT JOIN mame_rom_compatibility AS rom_compat_raw \
  ON rom_compat_raw.occurrence_id = occurrence.occurrence_id \
LEFT JOIN mame_rom_compatibility AS rom_compat \
  ON rom_compat.occurrence_id = occurrence.occurrence_id \
 AND interpretation.rules_version = 'mame-observed-compat-declared-text-v1' \
LEFT JOIN mame_disk_compatibility AS disk_compat \
  ON disk_compat.occurrence_id = occurrence.occurrence_id \
 AND interpretation.rules_version = 'mame-observed-compat-declared-text-v1' \
WHERE occurrence.occurrence_id = requested.occurrence_id \
  AND occurrence.claim_kind IN ('mame_rom', 'mame_disk') \
ORDER BY occurrence.occurrence_id";

pub(super) fn attach_payloads(
    connection: &mut SqliteConnection,
    occurrences: &mut [CatalogFileOccurrence],
) -> Result<(), CatalogFilesError> {
    let mut payloads = sql_query(SELECT)
        .load::<MamePayloadRow>(connection)?
        .into_iter()
        .map(|row| Ok((row.occurrence_id, payload(row)?)))
        .collect::<Result<BTreeMap<_, _>, CatalogFilesError>>()?;

    for row in sql_query(SAMPLE_SELECT).load::<SamplePayloadRow>(connection)? {
        let id = row.occurrence_id;
        if row.claim_kind != "mame_sample" {
            return Err(CatalogFilesError::MismatchedMameFileOwner(id));
        }
        if row.native_owner != Some(id) {
            return Err(CatalogFilesError::MissingMameFilePayload(id));
        }
        let sample = MameSamplePayload {
            name: required(row.name, "sample name", id)?,
            source_order: required(row.source_order, "sample source order", id)?,
            location: SourceLocation {
                line: required(row.source_line, "sample source line", id)?,
                column: required(row.source_column, "sample source column", id)?,
            },
        };
        if payloads
            .insert(id, MameFilePayload::Sample(sample))
            .is_some()
        {
            return Err(CatalogFilesError::MismatchedMameFileOwner(id));
        }
    }

    for occurrence in occurrences {
        let id = occurrence.occurrence_id.database_value();
        let is_mame = matches!(
            occurrence.provenance.occurrence_kind,
            OccurrenceKind::MameRom | OccurrenceKind::MameDisk | OccurrenceKind::MameSample
        );
        let Some(payload) = payloads.remove(&id) else {
            if is_mame {
                return Err(CatalogFilesError::MissingMameFilePayload(id));
            }
            continue;
        };
        let expected = matches!(
            (&occurrence.provenance.occurrence_kind, &payload),
            (OccurrenceKind::MameRom, MameFilePayload::Rom(_))
                | (OccurrenceKind::MameDisk, MameFilePayload::Disk(_))
                | (OccurrenceKind::MameSample, MameFilePayload::Sample(_))
        );
        if !is_mame
            || !expected
            || occurrence.provenance.format != "mame-listxml"
            || occurrence.provenance.source_element_kind != super::SourceElementKind::MameMachine
            || occurrence.provenance.set_group_kind != super::SetGroupKind::Root
        {
            return Err(CatalogFilesError::MismatchedMameFileOwner(id));
        }
        let (name, location) = match &payload {
            MameFilePayload::Rom(rom) => (&rom.name, rom.location),
            MameFilePayload::Disk(disk) => (&disk.name, disk.location),
            MameFilePayload::Sample(sample) => (&sample.name, sample.location),
        };
        occurrence.provenance.asset_name = Some(name.clone());
        occurrence.provenance.native_occurrence_location = Some(location);
        occurrence.mame_file = Some(payload);
    }
    if let Some((id, _)) = payloads.first_key_value() {
        return Err(CatalogFilesError::MissingOccurrenceOwner(*id));
    }
    Ok(())
}

fn payload(row: MamePayloadRow) -> Result<MameFilePayload, CatalogFilesError> {
    let expected_kind = match row.claim_kind.as_str() {
        "mame_rom" => OccurrenceKind::MameRom,
        "mame_disk" => OccurrenceKind::MameDisk,
        _ => {
            return Err(CatalogFilesError::MismatchedMameFileOwner(
                row.occurrence_id,
            ));
        }
    };
    match (row.rom_owner, row.disk_owner) {
        (Some(owner), None) if expected_kind == OccurrenceKind::MameRom => {
            rom_payload(row, owner).map(MameFilePayload::Rom)
        }
        (None, Some(owner)) if expected_kind == OccurrenceKind::MameDisk => {
            disk_payload(row, owner).map(MameFilePayload::Disk)
        }
        (None, None) => Err(CatalogFilesError::MissingMameFilePayload(row.occurrence_id)),
        _ => Err(CatalogFilesError::MismatchedMameFileOwner(
            row.occurrence_id,
        )),
    }
}

fn rom_payload(row: MamePayloadRow, owner: i64) -> Result<MameRomPayload, CatalogFilesError> {
    let qualified = row.rom_compatibility_owner.is_some();
    let compatibility = MameRomCompatibility {
        sound_only: optional_bool(row.rom_sound_only, "sound_only", owner)?,
        dispose: optional_bool(row.rom_dispose, "dispose", owner)?,
        load_flag: row.rom_load_flag,
        value: row.rom_value,
        inverted: optional_bool(row.rom_inverted, "inverted", owner)?,
        ovha: row.rom_ovha,
        no_thread: optional_bool(row.rom_no_thread, "no_thread", owner)?,
    };
    Ok(MameRomPayload {
        name: required(row.rom_name, "MAME ROM name", owner)?,
        declarations: MameAssetDeclarations {
            size_text: row.rom_size_text,
            crc_text: row.rom_crc_text,
            md5_text: row.rom_md5_text,
            sha1_text: row.rom_sha1_text,
            offset_text: row.rom_offset_text,
        },
        size: row.rom_size,
        evidence_scope: parse_rom_evidence_scope(required(
            row.rom_evidence_scope,
            "MAME ROM evidence scope",
            owner,
        )?)?,
        evidence_provenance: required(
            row.rom_evidence_provenance,
            "MAME ROM evidence provenance",
            owner,
        )?,
        merge_name: row.rom_merge_name,
        dump_status: parse_dump_status(required(
            row.rom_dump_status,
            "MAME ROM dump status",
            owner,
        )?)?,
        status_specified: required(row.rom_status_specified, "MAME ROM status presence", owner)?,
        source_order: required(row.rom_source_order, "MAME ROM source order", owner)?,
        location: SourceLocation {
            line: required(row.rom_line, "MAME ROM line", owner)?,
            column: required(row.rom_column, "MAME ROM column", owner)?,
        },
        region: row.rom_region,
        bios: row.rom_bios,
        optional: mame_boolean(row.rom_optional, "MAME ROM optional", owner)?,
        optional_specified: required(
            row.rom_optional_specified,
            "MAME ROM optional presence",
            owner,
        )?,
        compatibility: qualified.then_some(compatibility),
    })
}

fn disk_payload(row: MamePayloadRow, owner: i64) -> Result<MameDiskPayload, CatalogFilesError> {
    let compatibility = optional_bool(row.disk_writeable, "writeable", owner)?
        .map(|writeable| MameDiskCompatibility { writeable });
    Ok(MameDiskPayload {
        name: required(row.disk_name, "MAME disk name", owner)?,
        declarations: MameAssetDeclarations {
            sha1_text: row.disk_sha1_text,
            ..MameAssetDeclarations::default()
        },
        evidence_scope: parse_disk_digest_scope(required(
            row.disk_evidence_scope,
            "MAME disk evidence scope",
            owner,
        )?)?,
        evidence_provenance: required(
            row.disk_evidence_provenance,
            "MAME disk evidence provenance",
            owner,
        )?,
        merge_name: row.disk_merge_name,
        dump_status: parse_dump_status(required(
            row.disk_dump_status,
            "MAME disk dump status",
            owner,
        )?)?,
        status_specified: required(
            row.disk_status_specified,
            "MAME disk status presence",
            owner,
        )?,
        source_order: required(row.disk_source_order, "MAME disk source order", owner)?,
        location: SourceLocation {
            line: required(row.disk_line, "MAME disk line", owner)?,
            column: required(row.disk_column, "MAME disk column", owner)?,
        },
        region: row.disk_region,
        optional: mame_boolean(row.disk_optional, "MAME disk optional", owner)?,
        optional_specified: required(
            row.disk_optional_specified,
            "MAME disk optional presence",
            owner,
        )?,
        disk_index: row.disk_index,
        writable: mame_boolean(row.disk_writable, "MAME disk writable", owner)?,
        writable_specified: required(
            row.disk_writable_specified,
            "MAME disk writable presence",
            owner,
        )?,
        compatibility,
    })
}

fn required<T>(value: Option<T>, field: &'static str, owner: i64) -> Result<T, CatalogFilesError> {
    value.ok_or_else(|| CatalogFilesError::InvalidStoredValue {
        field,
        value: format!("NULL for MAME owner {owner}"),
    })
}

fn optional_bool(
    value: Option<i64>,
    field: &'static str,
    owner: i64,
) -> Result<Option<MameBoolean>, CatalogFilesError> {
    match value {
        Some(0) => Ok(Some(MameBoolean::No)),
        Some(1) => Ok(Some(MameBoolean::Yes)),
        Some(value) => Err(CatalogFilesError::InvalidStoredValue {
            field,
            value: format!("{value} for MAME owner {owner}"),
        }),
        None => Ok(None),
    }
}

fn mame_boolean(
    value: Option<bool>,
    field: &'static str,
    owner: i64,
) -> Result<MameBoolean, CatalogFilesError> {
    Ok(if required(value, field, owner)? {
        MameBoolean::Yes
    } else {
        MameBoolean::No
    })
}

fn parse_rom_evidence_scope(value: String) -> Result<MameRomEvidenceScope, CatalogFilesError> {
    match value.as_str() {
        "whole_asset" | "whole_file" => Ok(MameRomEvidenceScope::WholeFile),
        "unknown" => Ok(MameRomEvidenceScope::Unknown),
        _ => Err(CatalogFilesError::InvalidStoredValue {
            field: "MAME ROM evidence scope",
            value,
        }),
    }
}

fn parse_disk_digest_scope(value: String) -> Result<DiskDigestScope, CatalogFilesError> {
    match value.as_str() {
        "chd_header_sha1" => Ok(DiskDigestScope::ChdHeaderSha1),
        "unknown" => Ok(DiskDigestScope::Unknown),
        _ => Err(CatalogFilesError::InvalidStoredValue {
            field: "MAME disk digest scope",
            value,
        }),
    }
}

fn parse_dump_status(value: String) -> Result<MameDumpStatus, CatalogFilesError> {
    match value.as_str() {
        "good" => Ok(MameDumpStatus::Good),
        "baddump" => Ok(MameDumpStatus::BadDump),
        "nodump" => Ok(MameDumpStatus::NoDump),
        _ => Err(CatalogFilesError::InvalidStoredValue {
            field: "MAME dump status",
            value,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::sql_types::Text;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    #[test]
    fn native_payload_select_seeks_requested_owners() -> Result<(), Box<dyn std::error::Error>> {
        let pool = crate::storage::db::create_db_pool(":memory:")?;
        let mut connection = pool.get()?;
        super::super::create_request_table(&mut connection)?;
        let plan =
            sql_query(format!("EXPLAIN QUERY PLAN {SELECT}")).load::<PlanRow>(&mut connection)?;
        for owner in [
            "occurrence",
            "rom",
            "disk",
            "rom_compat_raw",
            "rom_compat",
            "disk_compat",
        ] {
            assert!(
                plan.iter().any(
                    |step| step.detail.starts_with(&format!("SEARCH {owner} USING "))
                        && step.detail.contains("PRIMARY KEY")
                ),
                "no bounded lookup for {owner}"
            );
            assert!(
                !plan
                    .iter()
                    .any(|step| step.detail.starts_with(&format!("SCAN {owner} "))),
                "unrelated native rows scanned for {owner}"
            );
        }
        Ok(())
    }

    #[test]
    fn sample_payload_select_seeks_requested_occurrence_keys()
    -> Result<(), Box<dyn std::error::Error>> {
        let pool = crate::storage::db::create_db_pool(":memory:")?;
        let mut connection = pool.get()?;
        super::super::create_request_table(&mut connection)?;
        let plan = sql_query(format!("EXPLAIN QUERY PLAN {SAMPLE_SELECT}"))
            .load::<PlanRow>(&mut connection)?;
        for owner in ["occurrence", "sample"] {
            assert!(
                plan.iter().any(
                    |step| step.detail.starts_with(&format!("SEARCH {owner} USING "))
                        && step.detail.contains("PRIMARY KEY")
                ),
                "no occurrence key lookup for {owner}"
            );
            assert!(
                !plan
                    .iter()
                    .any(|step| step.detail.starts_with(&format!("SCAN {owner} "))),
                "unrelated rows scanned for {owner}"
            );
        }
        Ok(())
    }
}
