//! Complete native CMP media hydration on the caller's existing read transaction.
use std::collections::{BTreeMap, BTreeSet};

use diesel::{RunQueryDsl, SqliteConnection, sql_query};

use super::{
    CatalogFileOccurrence, CatalogFilesError, ClrMameProEvidenceScope, ClrMameProFieldPosition,
    ClrMameProPositionedField, ClrMameProRelationshipId, ClrMameProRomField, OccurrenceId,
    OccurrenceKind, SourceLocation, invalid_value,
};

mod queries;
mod rows;
#[cfg(test)]
mod tests;
mod validation;

use rows::{DigestRow, MergeRow, OwnerRow, PositionRow, RomRow, SampleRow};

const BATCH_SIZE: usize = 400;
type Payloads = BTreeMap<OccurrenceId, ClrMameProFilePayload>;

/// A recognized effective CMP dump status; unknown explicit text remains in `status_text`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProDumpStatus {
    Good,
    BadDump,
    NoDump,
    Verified,
}

/// Unresolved native merge declaration and its actual reported registry identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProMergeReference {
    pub relationship_id: ClrMameProRelationshipId,
    pub merge_name: String,
}

/// Native ROM values; positions retain spelling, quotation and separate field ordinals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProRomPayload {
    pub name: String,
    pub size_text: Option<String>,
    pub size: Option<i64>,
    pub crc_text: Option<String>,
    pub crc32_text: Option<String>,
    pub md5_text: Option<String>,
    pub sha1_text: Option<String>,
    pub date: Option<String>,
    pub serial: Option<String>,
    pub status_text: Option<String>,
    pub nodump_present: bool,
    pub baddump_present: bool,
    pub dump_status: Option<ClrMameProDumpStatus>,
    pub evidence_scope: ClrMameProEvidenceScope,
    pub merge: Option<ClrMameProMergeReference>,
    /// Raw set-item ordinal of the ROM form, not dense media occurrence order.
    pub source_order: usize,
    pub location: SourceLocation,
    pub field_positions: Vec<ClrMameProPositionedField<ClrMameProRomField>>,
}

/// Filename-only scalar sample. It declares no size, scope or shared identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProSamplePayload {
    pub name: String,
    pub position: ClrMameProFieldPosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClrMameProFilePayload {
    Rom(Box<ClrMameProRomPayload>),
    Sample(ClrMameProSamplePayload),
}

pub(super) fn attach_payloads(
    connection: &mut SqliteConnection,
    occurrences: &mut [CatalogFileOccurrence],
) -> Result<(), CatalogFilesError> {
    let ids = occurrences
        .iter()
        .map(|row| row.occurrence_id)
        .collect::<Vec<_>>();
    let mut payloads = clrmamepro_file_payloads(connection, &ids)?;
    for occurrence in occurrences {
        let id = occurrence.occurrence_id;
        let payload = payloads.remove(&id);
        let expected_rom = occurrence.provenance.occurrence_kind == OccurrenceKind::ClrMameProRom;
        let expected_sample =
            occurrence.provenance.occurrence_kind == OccurrenceKind::ClrMameProSample;
        match (expected_rom, expected_sample, payload) {
            (true, false, Some(ClrMameProFilePayload::Rom(rom))) => {
                occurrence.provenance.asset_name = Some(rom.name.clone());
                occurrence.provenance.native_occurrence_location = Some(rom.location);
                occurrence.clrmamepro_file = Some(ClrMameProFilePayload::Rom(rom));
            }
            (false, true, Some(ClrMameProFilePayload::Sample(sample))) => {
                occurrence.provenance.asset_name = Some(sample.name.clone());
                occurrence.provenance.native_occurrence_location = Some(sample.position.location);
                occurrence.clrmamepro_file = Some(ClrMameProFilePayload::Sample(sample));
            }
            (false, false, None) => {}
            (true, false, None) | (false, true, None) => {
                return Err(CatalogFilesError::MissingClrMameProFilePayload(
                    id.database_value(),
                ));
            }
            _ => {
                return Err(CatalogFilesError::MismatchedClrMameProFileOwner(
                    id.database_value(),
                ));
            }
        }
    }
    if let Some((id, _)) = payloads.first_key_value() {
        return Err(CatalogFilesError::MissingOccurrenceOwner(
            id.database_value(),
        ));
    }
    Ok(())
}

/// Load complete CMP payloads without acquiring a connection or opening a transaction.
/// The caller keeps one read transaction open across all batches. Mixed non-CMP IDs have no
/// entry; callers expecting CMP owners must require their returned entries.
pub fn clrmamepro_file_payloads(
    connection: &mut SqliteConnection,
    ids: &[OccurrenceId],
) -> Result<BTreeMap<OccurrenceId, ClrMameProFilePayload>, CatalogFilesError> {
    let ids = ids.iter().copied().collect::<BTreeSet<_>>();
    if let Some(id) = ids.iter().find(|id| id.database_value() <= 0) {
        return Err(invalid_value(
            "CMP occurrence ID",
            id.database_value().to_string(),
        ));
    }
    let ids = ids.into_iter().collect::<Vec<_>>();
    let mut result = BTreeMap::new();
    for batch in ids.chunks(BATCH_SIZE) {
        result.append(&mut load_batch(connection, batch)?);
    }
    Ok(result)
}

fn load_batch(
    connection: &mut SqliteConnection,
    ids: &[OccurrenceId],
) -> Result<Payloads, CatalogFilesError> {
    let requested = queries::requested(ids);
    let owners =
        sql_query(format!("{requested}{}", queries::owners())).load::<OwnerRow>(connection)?;
    validation::owners(&owners)?;
    let mut payloads = load_values(connection, &requested)?;
    load_merges(connection, &requested, &mut payloads)?;
    let positions = sql_query(format!("{requested}{}", queries::positions()))
        .load::<PositionRow>(connection)?;
    validation::positions(&mut payloads, positions)?;
    let digests =
        sql_query(format!("{requested}{}", queries::digests())).load::<DigestRow>(connection)?;
    validation::digests_and_identity(&payloads, &owners, digests)?;
    if payloads.len() != owners.len() {
        return Err(invalid_value(
            "CMP native owner closure",
            "owner/payload count mismatch".into(),
        ));
    }
    Ok(payloads)
}

fn load_values(
    connection: &mut SqliteConnection,
    requested: &str,
) -> Result<Payloads, CatalogFilesError> {
    let rows = sql_query(format!("{requested}{}", queries::roms())).load::<RomRow>(connection)?;
    let mut payloads = rows
        .into_iter()
        .map(validation::rom)
        .collect::<Result<Payloads, _>>()?;
    let samples =
        sql_query(format!("{requested}{}", queries::samples())).load::<SampleRow>(connection)?;
    for row in samples {
        let (id, payload) = validation::sample(row)?;
        if payloads.insert(id, payload).is_some() {
            return Err(CatalogFilesError::MismatchedClrMameProFileOwner(
                id.database_value(),
            ));
        }
    }
    Ok(payloads)
}

fn load_merges(
    connection: &mut SqliteConnection,
    requested: &str,
    payloads: &mut Payloads,
) -> Result<(), CatalogFilesError> {
    let rows =
        sql_query(format!("{requested}{}", queries::merges())).load::<MergeRow>(connection)?;
    for row in rows {
        if !row.valid_storage || !row.valid_owner {
            return Err(invalid_value(
                "CMP merge registry",
                row.occurrence_id.to_string(),
            ));
        }
        let id = OccurrenceId::from_database(row.occurrence_id);
        let Some(ClrMameProFilePayload::Rom(payload)) = payloads.get_mut(&id) else {
            return Err(CatalogFilesError::MissingClrMameProFilePayload(
                row.occurrence_id,
            ));
        };
        let relationship_id = ClrMameProRelationshipId::from_database(row.relationship_id)
            .ok_or_else(|| invalid_value("CMP relationship ID", row.relationship_id.to_string()))?;
        if payload
            .merge
            .replace(ClrMameProMergeReference {
                relationship_id,
                merge_name: row.merge_name,
            })
            .is_some()
        {
            return Err(invalid_value(
                "duplicate CMP merge",
                row.occurrence_id.to_string(),
            ));
        }
    }
    Ok(())
}
