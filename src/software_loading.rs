//! Checked, source-backed MAME 0.289 software-list loading recipes.
//!
//! Recipes borrow catalog facts. ROM bytes and destination regions are never
//! stored in SQLite, and an invalid recipe does not alter imported facts.

mod layout;

pub use layout::{ByteLayout, BytePlacement, LayoutError};

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    catalog_files::{
        CatalogFileOccurrence, DigestAlgorithm, OccurrenceId, OccurrenceKind, SetGroupKind,
        SoftwareAreaKind, SoftwareDumpStatus, SoftwareFileOperation, SoftwareFilePayload,
        SoftwareRomPayload, SourceElementKind,
    },
    catalog_software::{SoftwareArea, SoftwareAreaFields},
};

#[derive(Clone, Copy)]
struct Entry<'a> {
    id: OccurrenceId,
    rom: &'a SoftwareRomPayload,
}

/// One physical file declaration and its distinct verification/progress sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoftwareLoadFile<'a> {
    pub declaration: OccurrenceId,
    pub name: &'a str,
    pub expected_length: u64,
    pub progress_length: u64,
    /// Highest file cursor required by any initial or reload run.
    pub read_length: u64,
    pub status: SoftwareDumpStatus,
    crc: Option<[u8; 4]>,
    sha1: Option<[u8; 20]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIndex(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Read {
        file: FileIndex,
        start: u64,
        end: u64,
        placement: BytePlacement,
    },
    Fill {
        start: u64,
        end: u64,
        value: u8,
    },
}

/// A source-qualified warning; no warning is evidence of observed ROM bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoftwareLoadWarning {
    PartialGroup {
        occurrence: OccurrenceId,
        length: u64,
        layout: ByteLayout,
    },
    MissingChecksums {
        declaration: OccurrenceId,
    },
    MissingNoDump {
        declaration: OccurrenceId,
    },
    KnownBadDump {
        declaration: OccurrenceId,
    },
    LengthMismatch {
        declaration: OccurrenceId,
        expected: u64,
        actual: u64,
    },
    ChecksumMismatch {
        declaration: OccurrenceId,
        algorithm: DigestAlgorithm,
    },
}

/// A borrowed physical source file, keyed by its declaring occurrence, not its name.
#[derive(Clone, Copy, Debug)]
pub struct SoftwareFileInput<'a> {
    pub declaration: OccurrenceId,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
enum BoundFile<'a> {
    Present(&'a [u8]),
    MissingNoDump,
}

/// A fully checked recipe; construction never changes the catalog or ROM bytes.
#[derive(Debug)]
pub struct SoftwareLoadPlan<'a> {
    region_length: u64,
    files: Vec<SoftwareLoadFile<'a>>,
    steps: Vec<Step>,
    warnings: Vec<SoftwareLoadWarning>,
}

impl<'a> SoftwareLoadPlan<'a> {
    /// Interpret exactly one native data area's complete, ordered occurrences.
    ///
    /// Missing, duplicate, wrong-area or cross-edition entries are rejected.
    /// Invalid numeric source facts remain queryable; this method does not edit them.
    pub fn from_area(
        area: &SoftwareArea,
        occurrences: &'a [CatalogFileOccurrence],
    ) -> Result<Self, SoftwareLoadError> {
        let SoftwareAreaFields::Data { size_text, .. } = &area.fields else {
            return Err(SoftwareLoadError::NotDataArea);
        };
        let region_length = number(None, "region size", Some(size_text))?;
        let by_id = occurrences
            .iter()
            .map(|row| (row.occurrence_id, row))
            .collect::<BTreeMap<_, _>>();
        if by_id.len() != occurrences.len() || by_id.len() != area.entry_ids.len() {
            return Err(SoftwareLoadError::IncompleteArea);
        }
        let mut seen = BTreeSet::new();
        let mut entries = Vec::with_capacity(area.entry_ids.len());
        let mut context = None;
        let mut previous_order = None;
        for (index, id) in area.entry_ids.iter().enumerate() {
            if !seen.insert(*id) {
                return Err(SoftwareLoadError::IncompleteArea);
            }
            let occurrence = by_id.get(id).ok_or(SoftwareLoadError::IncompleteArea)?;
            let Some(owner) = &occurrence.provenance.software_owner else {
                return Err(SoftwareLoadError::IncompleteArea);
            };
            if owner.area_id != area.id.database_value()
                || owner.area_kind != SoftwareAreaKind::Data
                || owner.area_name != area.name
                || owner.area_order != area.order
                || owner.part_id <= 0
                || owner.part_order < 0
                || occurrence.provenance.source_element_kind != SourceElementKind::SoftwareItem
                || !matches!(
                    occurrence.provenance.set_group_kind,
                    SetGroupKind::SoftwareList { .. }
                )
                || occurrence.provenance.format
                    != crate::app::CatalogDocumentFormat::MameSoftwareListXml.as_str()
            {
                return Err(SoftwareLoadError::IncompleteArea);
            }
            let current = (
                &occurrence.provenance.source_key,
                &occurrence.provenance.catalog_key,
                &occurrence.provenance.snapshot_key,
                &occurrence.provenance.document_key,
                &occurrence.provenance.interpretation_key,
                occurrence.provenance.set_id,
                occurrence.provenance.set_group_id,
                owner.part_id,
                &owner.part_name,
                owner.part_order,
            );
            if context
                .as_ref()
                .is_some_and(|previous| *previous != current)
            {
                return Err(SoftwareLoadError::IncompleteArea);
            }
            context = Some(current);
            let Some(SoftwareFilePayload::Rom(rom)) = &occurrence.software_file else {
                return Err(SoftwareLoadError::IncompleteArea);
            };
            let expected_kind = if rom.operation == SoftwareFileOperation::Load {
                OccurrenceKind::SoftwareRomEntry
            } else {
                OccurrenceKind::SoftwareRomOperation
            };
            if occurrence.provenance.occurrence_kind != expected_kind {
                return Err(SoftwareLoadError::IncompleteArea);
            }
            if rom.component_order
                != i64::try_from(index).map_err(|_| SoftwareLoadError::Overflow)?
                || rom.source_order < 0
                || previous_order.is_some_and(|previous| previous >= rom.source_order)
            {
                return Err(SoftwareLoadError::IncompleteArea);
            }
            previous_order = Some(rom.source_order);
            entries.push(Entry { id: *id, rom });
        }
        compile(region_length, &entries)
    }

    /// Physical file groups, not one file per segment or reload.
    #[must_use]
    pub fn files(&self) -> &[SoftwareLoadFile<'_>] {
        &self.files
    }

    /// Declared destination size, independently of any source file's size.
    #[must_use]
    pub const fn region_length(&self) -> u64 {
        self.region_length
    }

    /// Warnings established by checked interpretation, before binding ROM bytes.
    #[must_use]
    pub fn warnings(&self) -> &[SoftwareLoadWarning] {
        &self.warnings
    }

    /// Validate every physical source before any destination is changed.
    ///
    /// Length/checksum mismatches warn as in MAME. A short read is rejected;
    /// missing no-dump files are explicitly represented, never fabricated.
    pub fn bind<'plan, 'bytes>(
        &'plan self,
        inputs: &[SoftwareFileInput<'bytes>],
    ) -> Result<ReadySoftwareLoad<'plan, 'bytes>, SoftwareLoadError> {
        let supplied = inputs
            .iter()
            .map(|input| (input.declaration, input.bytes))
            .collect::<BTreeMap<_, _>>();
        let declared = self
            .files
            .iter()
            .map(|file| file.declaration)
            .collect::<BTreeSet<_>>();
        if supplied.len() != inputs.len() || supplied.keys().any(|id| !declared.contains(id)) {
            return Err(SoftwareLoadError::UnexpectedInput);
        }
        let mut files = Vec::with_capacity(self.files.len());
        let mut warnings = self.warnings.clone();
        for file in &self.files {
            let Some(bytes) = supplied.get(&file.declaration).copied() else {
                if file.status != SoftwareDumpStatus::NoDump {
                    return Err(SoftwareLoadError::MissingFile(file.declaration));
                }
                warnings.push(SoftwareLoadWarning::MissingNoDump {
                    declaration: file.declaration,
                });
                files.push(BoundFile::MissingNoDump);
                continue;
            };
            let actual = u64::try_from(bytes.len()).map_err(|_| SoftwareLoadError::Overflow)?;
            if actual < file.read_length {
                return Err(SoftwareLoadError::ShortFile {
                    declaration: file.declaration,
                    required: file.read_length,
                    actual,
                });
            }
            if actual != file.expected_length {
                warnings.push(SoftwareLoadWarning::LengthMismatch {
                    declaration: file.declaration,
                    expected: file.expected_length,
                    actual,
                });
            }
            if file.status != SoftwareDumpStatus::NoDump {
                if let Some(expected) = file.crc
                    && expected != crc32fast::hash(bytes).to_be_bytes()
                {
                    warnings.push(SoftwareLoadWarning::ChecksumMismatch {
                        declaration: file.declaration,
                        algorithm: DigestAlgorithm::Crc32,
                    });
                }
                if let Some(expected) = file.sha1
                    && expected != crate::hashes::sha1_bytes(bytes)
                {
                    warnings.push(SoftwareLoadWarning::ChecksumMismatch {
                        declaration: file.declaration,
                        algorithm: DigestAlgorithm::Sha1,
                    });
                }
            }
            files.push(BoundFile::Present(bytes));
        }
        Ok(ReadySoftwareLoad {
            plan: self,
            files,
            warnings,
        })
    }
}

/// Sources have been checked; only this state can execute a recipe.
#[derive(Debug)]
pub struct ReadySoftwareLoad<'plan, 'bytes> {
    plan: &'plan SoftwareLoadPlan<'plan>,
    files: Vec<BoundFile<'bytes>>,
    warnings: Vec<SoftwareLoadWarning>,
}

impl ReadySoftwareLoad<'_, '_> {
    /// Write a preflighted region without owning or copying any source file.
    pub fn execute(self, region: &mut [u8]) -> Result<Vec<SoftwareLoadWarning>, SoftwareLoadError> {
        if u64::try_from(region.len()).map_err(|_| SoftwareLoadError::Overflow)?
            != self.plan.region_length
        {
            return Err(SoftwareLoadError::WrongRegionSize);
        }
        for step in &self.plan.steps {
            match *step {
                Step::Read {
                    file,
                    start,
                    end,
                    placement,
                } => {
                    let source = self
                        .files
                        .get(file.0)
                        .ok_or(SoftwareLoadError::IncompleteArea)?;
                    if let BoundFile::Present(bytes) = source {
                        let start =
                            usize::try_from(start).map_err(|_| SoftwareLoadError::Overflow)?;
                        let end = usize::try_from(end).map_err(|_| SoftwareLoadError::Overflow)?;
                        let segment = bytes
                            .get(start..end)
                            .ok_or(SoftwareLoadError::IncompleteArea)?;
                        placement.apply(segment, region)?;
                    }
                }
                Step::Fill { start, end, value } => {
                    let start = usize::try_from(start).map_err(|_| SoftwareLoadError::Overflow)?;
                    let end = usize::try_from(end).map_err(|_| SoftwareLoadError::Overflow)?;
                    region
                        .get_mut(start..end)
                        .ok_or(SoftwareLoadError::IncompleteArea)?
                        .fill(value);
                }
            }
        }
        Ok(self.warnings)
    }
}

#[derive(Debug, Error)]
pub enum SoftwareLoadError {
    #[error("loading requires a native software data area")]
    NotDataArea,
    #[error("occurrences do not form exactly one complete ordered native area")]
    IncompleteArea,
    #[error("invalid {field} at source occurrence {occurrence:?}")]
    InvalidNumber {
        occurrence: Option<OccurrenceId>,
        field: &'static str,
    },
    #[error("invalid checksum declaration at source occurrence {0:?}")]
    InvalidChecksum(OccurrenceId),
    #[error("unresolved or mismatched loading operation at {0:?}")]
    UnresolvedOperation(OccurrenceId),
    #[error("loading arithmetic exceeds the supported range")]
    Overflow,
    #[error("a file was supplied more than once or was not declared by this recipe")]
    UnexpectedInput,
    #[error("missing required physical file for {0:?}")]
    MissingFile(OccurrenceId),
    #[error("physical file {declaration:?} has {actual} bytes, requires at least {required}")]
    ShortFile {
        declaration: OccurrenceId,
        required: u64,
        actual: u64,
    },
    #[error("destination size does not match the declared data area")]
    WrongRegionSize,
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

struct Run {
    file: FileIndex,
    layout: ByteLayout,
    cursor: u64,
    length: u64,
    first: bool,
}

fn compile<'a>(
    region_length: u64,
    entries: &[Entry<'a>],
) -> Result<SoftwareLoadPlan<'a>, SoftwareLoadError> {
    let mut plan = SoftwareLoadPlan {
        region_length,
        files: Vec::new(),
        steps: Vec::new(),
        warnings: Vec::new(),
    };
    let mut active = None;
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry.id) {
            return Err(SoftwareLoadError::IncompleteArea);
        }
        let size = number(Some(entry.id), "size", entry.rom.size_text.as_deref())?;
        let instruction = entry.rom.load_instruction;
        let operation = entry.rom.operation;
        match operation {
            SoftwareFileOperation::Load => {
                finish_run(&mut plan, active.take())?;
                let mut run = start_file(&mut plan, entry)?;
                read_step(&mut plan, &mut run, entry, size)?;
                active = Some(run);
            }
            SoftwareFileOperation::Fill => {
                if instruction != Some(crate::catalog_files::SoftwareLoadInstruction::Fill)
                    || entry.rom.declaration_occurrence_id.is_some()
                {
                    return Err(SoftwareLoadError::UnresolvedOperation(entry.id));
                }
                finish_run(&mut plan, active.take())?;
                let start = offset(entry)?;
                let placement = ByteLayout::Linear.placement(start, size, region_length)?;
                let value = match entry.rom.value.as_deref() {
                    None | Some("") => 0,
                    Some(value) => u8::try_from(number(Some(entry.id), "fill value", Some(value))?)
                        .map_err(|_| SoftwareLoadError::InvalidNumber {
                            occurrence: Some(entry.id),
                            field: "fill value",
                        })?,
                };
                plan.steps.push(Step::Fill {
                    start,
                    end: placement.end(),
                    value,
                });
            }
            SoftwareFileOperation::Continue
            | SoftwareFileOperation::Ignore
            | SoftwareFileOperation::Reload
            | SoftwareFileOperation::ReloadPlain => {
                use crate::catalog_files::SoftwareLoadInstruction as Load;
                let expected = match operation {
                    SoftwareFileOperation::Continue => Load::Continue,
                    SoftwareFileOperation::Ignore => Load::Ignore,
                    SoftwareFileOperation::Reload => Load::Reload,
                    SoftwareFileOperation::ReloadPlain => Load::ReloadPlain,
                    SoftwareFileOperation::Load | SoftwareFileOperation::Fill => {
                        return Err(SoftwareLoadError::UnresolvedOperation(entry.id));
                    }
                };
                let run = active
                    .as_mut()
                    .ok_or(SoftwareLoadError::UnresolvedOperation(entry.id))?;
                let file = plan
                    .files
                    .get(run.file.0)
                    .ok_or(SoftwareLoadError::IncompleteArea)?;
                if instruction != Some(expected)
                    || entry.rom.declaration_occurrence_id != Some(file.declaration)
                {
                    return Err(SoftwareLoadError::UnresolvedOperation(entry.id));
                }
                if matches!(
                    operation,
                    SoftwareFileOperation::Reload | SoftwareFileOperation::ReloadPlain
                ) {
                    close_run(&mut plan, run)?;
                    run.cursor = 0;
                    run.length = 0;
                    run.first = false;
                    if operation == SoftwareFileOperation::ReloadPlain {
                        run.layout = ByteLayout::Linear;
                    }
                }
                if operation == SoftwareFileOperation::Ignore {
                    run.length = run
                        .length
                        .checked_add(size)
                        .ok_or(SoftwareLoadError::Overflow)?;
                } else {
                    read_step(&mut plan, run, entry, size)?;
                }
            }
        }
    }
    finish_run(&mut plan, active)?;
    Ok(plan)
}

fn start_file<'a>(
    plan: &mut SoftwareLoadPlan<'a>,
    entry: &Entry<'a>,
) -> Result<Run, SoftwareLoadError> {
    let layout = ByteLayout::from_load(entry.rom.load_instruction)
        .ok_or(SoftwareLoadError::UnresolvedOperation(entry.id))?;
    let name = entry
        .rom
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .ok_or(SoftwareLoadError::UnresolvedOperation(entry.id))?;
    if entry.rom.declaration_occurrence_id != Some(entry.id) {
        return Err(SoftwareLoadError::UnresolvedOperation(entry.id));
    }
    let crc = checksum::<4>(entry, entry.rom.crc_text.as_deref())?;
    let sha1 = checksum::<20>(entry, entry.rom.sha1_text.as_deref())?;
    if entry.rom.status != SoftwareDumpStatus::NoDump && crc.is_none() && sha1.is_none() {
        plan.warnings.push(SoftwareLoadWarning::MissingChecksums {
            declaration: entry.id,
        });
    }
    if entry.rom.status == SoftwareDumpStatus::BadDump {
        plan.warnings.push(SoftwareLoadWarning::KnownBadDump {
            declaration: entry.id,
        });
    }
    let file = FileIndex(plan.files.len());
    plan.files.push(SoftwareLoadFile {
        declaration: entry.id,
        name,
        expected_length: 0,
        progress_length: 0,
        read_length: 0,
        status: entry.rom.status,
        crc,
        sha1,
    });
    Ok(Run {
        file,
        layout,
        cursor: 0,
        length: 0,
        first: true,
    })
}

fn read_step(
    plan: &mut SoftwareLoadPlan<'_>,
    run: &mut Run,
    entry: &Entry<'_>,
    size: u64,
) -> Result<(), SoftwareLoadError> {
    let placement = run
        .layout
        .placement(offset(entry)?, size, plan.region_length)?;
    let end = run
        .cursor
        .checked_add(size)
        .ok_or(SoftwareLoadError::Overflow)?;
    run.length = run
        .length
        .checked_add(size)
        .ok_or(SoftwareLoadError::Overflow)?;
    let file = plan
        .files
        .get_mut(run.file.0)
        .ok_or(SoftwareLoadError::IncompleteArea)?;
    file.read_length = file.read_length.max(end);
    if placement.has_partial_group() {
        plan.warnings.push(SoftwareLoadWarning::PartialGroup {
            occurrence: entry.id,
            length: size,
            layout: run.layout,
        });
    }
    plan.steps.push(Step::Read {
        file: run.file,
        start: run.cursor,
        end,
        placement,
    });
    run.cursor = end;
    Ok(())
}

fn finish_run(plan: &mut SoftwareLoadPlan<'_>, run: Option<Run>) -> Result<(), SoftwareLoadError> {
    if let Some(run) = run {
        close_run(plan, &run)?;
    }
    Ok(())
}

fn close_run(plan: &mut SoftwareLoadPlan<'_>, run: &Run) -> Result<(), SoftwareLoadError> {
    let file = plan
        .files
        .get_mut(run.file.0)
        .ok_or(SoftwareLoadError::IncompleteArea)?;
    if run.first {
        file.expected_length = run.length;
    }
    file.progress_length = file.progress_length.max(run.length);
    Ok(())
}

fn number(
    occurrence: Option<OccurrenceId>,
    field: &'static str,
    value: Option<&str>,
) -> Result<u64, SoftwareLoadError> {
    value
        .and_then(|value| crate::mame_softwarelist::parse_number(value).ok())
        .ok_or(SoftwareLoadError::InvalidNumber { occurrence, field })
}

fn offset(entry: &Entry<'_>) -> Result<u64, SoftwareLoadError> {
    entry
        .rom
        .offset_text
        .as_deref()
        .map_or(Ok(0), |value| number(Some(entry.id), "offset", Some(value)))
}

fn checksum<const N: usize>(
    entry: &Entry<'_>,
    value: Option<&str>,
) -> Result<Option<[u8; N]>, SoftwareLoadError> {
    if entry.rom.status == SoftwareDumpStatus::NoDump {
        return Ok(None);
    }
    let Some(value) = value else {
        return Ok(None);
    };
    let mut digest = [0; N];
    hex::decode_to_slice(value, &mut digest)
        .map_err(|_| SoftwareLoadError::InvalidChecksum(entry.id))?;
    Ok(Some(digest))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use crate::catalog_files::{
        SoftwareDumpStatus, SoftwareFileOperation, SoftwareLoadInstruction, SourceLocation,
    };

    fn rom(
        id: i64,
        size: &str,
        load: Option<SoftwareLoadInstruction>,
        operation: SoftwareFileOperation,
    ) -> SoftwareRomPayload {
        SoftwareRomPayload {
            attribute_positions: Vec::new(),
            name: (operation == SoftwareFileOperation::Load).then(|| "file.bin".into()),
            size_text: Some(size.into()),
            size: size.parse().ok(),
            offset_text: None,
            offset: None,
            value: None,
            crc_text: None,
            sha1_text: None,
            status: SoftwareDumpStatus::Good,
            status_specified: false,
            load_instruction: load,
            evidence_scope: "whole_asset".into(),
            component_order: id - 1,
            source_order: id - 1,
            location: SourceLocation { line: 1, column: 1 },
            declaration_occurrence_id: Some(OccurrenceId::from_database(1)),
            operation,
        }
    }

    #[test]
    fn first_run_counts_ignore_without_mistaking_segment_size_for_file_size() {
        let load = rom(1, "2", None, SoftwareFileOperation::Load);
        let ignore = rom(
            2,
            "3",
            Some(SoftwareLoadInstruction::Ignore),
            SoftwareFileOperation::Ignore,
        );
        let continuation = rom(
            3,
            "1",
            Some(SoftwareLoadInstruction::Continue),
            SoftwareFileOperation::Continue,
        );
        let entries = [&load, &ignore, &continuation]
            .into_iter()
            .zip(1..)
            .map(|(rom, id)| Entry {
                id: OccurrenceId::from_database(id),
                rom,
            })
            .collect::<Vec<_>>();
        let plan = compile(16, &entries).expect("valid file run");
        assert_eq!(plan.files().len(), 1);
        assert_eq!(plan.files()[0].expected_length, 6);
        assert_eq!(plan.files()[0].progress_length, 6);
    }

    #[test]
    fn reload_progress_does_not_replace_first_run_verification_length() {
        let load = rom(1, "2", None, SoftwareFileOperation::Load);
        let reload = rom(
            2,
            "4",
            Some(SoftwareLoadInstruction::Reload),
            SoftwareFileOperation::Reload,
        );
        let entries = [
            Entry {
                id: OccurrenceId::from_database(1),
                rom: &load,
            },
            Entry {
                id: OccurrenceId::from_database(2),
                rom: &reload,
            },
        ];
        let plan = compile(16, &entries).expect("valid reload run");
        assert_eq!(plan.files().len(), 1);
        assert_eq!(plan.files()[0].expected_length, 2);
        assert_eq!(plan.files()[0].progress_length, 4);
    }

    fn entries(roms: &[SoftwareRomPayload]) -> Vec<Entry<'_>> {
        roms.iter()
            .zip(1..)
            .map(|(rom, id)| Entry {
                id: OccurrenceId::from_database(id),
                rom,
            })
            .collect()
    }

    fn input(bytes: &[u8]) -> SoftwareFileInput<'_> {
        SoftwareFileInput {
            declaration: OccurrenceId::from_database(1),
            bytes,
        }
    }

    #[test]
    fn ignore_counts_length_but_does_not_seek_or_validate_unused_offset() {
        let load = rom(1, "2", None, SoftwareFileOperation::Load);
        let mut ignore = rom(
            2,
            "3",
            Some(SoftwareLoadInstruction::Ignore),
            SoftwareFileOperation::Ignore,
        );
        ignore.offset_text = Some("not an offset".into());
        let mut continuation = rom(
            3,
            "1",
            Some(SoftwareLoadInstruction::Continue),
            SoftwareFileOperation::Continue,
        );
        continuation.offset_text = Some("4".into());
        let roms = [load, ignore, continuation];
        let plan = compile(8, &entries(&roms)).expect("checked run");
        assert_eq!(plan.files()[0].read_length, 3);
        let mut region = [0xaa; 8];
        let warnings = plan
            .bind(&[input(&[10, 20, 30, 40, 50, 60])])
            .expect("whole file")
            .execute(&mut region)
            .expect("region");
        assert_eq!(region, [10, 20, 0xaa, 0xaa, 30, 0xaa, 0xaa, 0xaa]);
        assert!(
            !warnings
                .iter()
                .any(|warning| matches!(warning, SoftwareLoadWarning::LengthMismatch { .. }))
        );
    }

    #[test]
    fn reload_plain_resets_cursor_and_changes_inherited_layout() {
        let load = rom(
            1,
            "2",
            Some(SoftwareLoadInstruction::Load16Byte),
            SoftwareFileOperation::Load,
        );
        let mut reload = rom(
            2,
            "2",
            Some(SoftwareLoadInstruction::ReloadPlain),
            SoftwareFileOperation::ReloadPlain,
        );
        reload.offset_text = Some("4".into());
        let mut continuation = rom(
            3,
            "1",
            Some(SoftwareLoadInstruction::Continue),
            SoftwareFileOperation::Continue,
        );
        continuation.offset_text = Some("6".into());
        let mut inherited_reload = rom(
            4,
            "2",
            Some(SoftwareLoadInstruction::Reload),
            SoftwareFileOperation::Reload,
        );
        inherited_reload.offset_text = Some("8".into());
        let roms = [load, reload, continuation, inherited_reload];
        let plan = compile(10, &entries(&roms)).expect("checked reloads");
        assert_eq!(
            (
                plan.files()[0].expected_length,
                plan.files()[0].progress_length
            ),
            (2, 3)
        );
        let mut region = [0xaa; 10];
        let warnings = plan
            .bind(&[input(&[10, 20, 30])])
            .expect("file")
            .execute(&mut region)
            .expect("region");
        assert_eq!(region, [10, 0xaa, 20, 0xaa, 10, 20, 30, 0xaa, 10, 20]);
        assert!(warnings.contains(&SoftwareLoadWarning::LengthMismatch {
            declaration: OccurrenceId::from_database(1),
            expected: 2,
            actual: 3
        }));
    }

    #[test]
    fn partial_groups_warn_and_use_high_lane_with_padded_bounds() {
        let roms = [rom(
            1,
            "3",
            Some(SoftwareLoadInstruction::Load32WordSwap),
            SoftwareFileOperation::Load,
        )];
        assert!(matches!(
            compile(5, &entries(&roms)),
            Err(SoftwareLoadError::Layout(LayoutError::OutOfBounds {
                end: 6,
                ..
            }))
        ));
        let plan = compile(6, &entries(&roms)).expect("padded bounds");
        let mut region = [0xaa; 6];
        let warnings = plan
            .bind(&[input(&[10, 20, 30])])
            .expect("file")
            .execute(&mut region)
            .expect("region");
        assert_eq!(region, [20, 10, 0xaa, 0xaa, 0xaa, 30]);
        assert!(warnings.contains(&SoftwareLoadWarning::PartialGroup {
            occurrence: OccurrenceId::from_database(1),
            length: 3,
            layout: ByteLayout::WordSwap32
        }));
    }

    #[test]
    fn checksums_cover_the_complete_file_not_segments_or_reloads() {
        let bytes = [10, 20, 30, 40, 50];
        let mut load = rom(
            1,
            "2",
            Some(SoftwareLoadInstruction::Load16Byte),
            SoftwareFileOperation::Load,
        );
        load.crc_text = Some(hex::encode(crc32fast::hash(&bytes).to_be_bytes()));
        load.sha1_text = Some(hex::encode(crate::hashes::sha1_bytes(&bytes)));
        let mut ignore = rom(
            2,
            "3",
            Some(SoftwareLoadInstruction::Ignore),
            SoftwareFileOperation::Ignore,
        );
        // A control's spurious hashes are retained source facts, not new assertions.
        ignore.crc_text = Some("not a hash".into());
        let mut reload = rom(
            3,
            "1",
            Some(SoftwareLoadInstruction::Reload),
            SoftwareFileOperation::Reload,
        );
        reload.offset_text = Some("4".into());
        let roms = [load, ignore, reload];
        let plan = compile(8, &entries(&roms)).expect("whole-file hashes");
        let mut region = [0xaa; 8];
        let warnings = plan
            .bind(&[input(&bytes)])
            .expect("verified file")
            .execute(&mut region)
            .expect("region");
        assert!(warnings.is_empty());
        assert_eq!(region, [10, 0xaa, 20, 0xaa, 10, 0xaa, 0xaa, 0xaa]);
        let wrong = [10, 20, 30, 40, 51];
        let warnings = plan
            .bind(&[input(&wrong)])
            .expect("mismatch warns")
            .execute(&mut region)
            .expect("region");
        assert_eq!(
            warnings
                .iter()
                .filter(|warning| matches!(warning, SoftwareLoadWarning::ChecksumMismatch { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn nodump_does_not_fabricate_bytes_or_checksum_verification() {
        let mut load = rom(1, "2", None, SoftwareFileOperation::Load);
        load.status = SoftwareDumpStatus::NoDump;
        load.crc_text = Some("invalid but ignored".into());
        load.sha1_text = Some("also invalid".into());
        let roms = [load];
        let plan = compile(4, &entries(&roms)).expect("nodump assertions skipped");
        let mut region = [0xaa; 4];
        let warnings = plan
            .bind(&[])
            .expect("missing nodump allowed")
            .execute(&mut region)
            .expect("region");
        assert_eq!(region, [0xaa; 4]);
        assert_eq!(
            warnings,
            [SoftwareLoadWarning::MissingNoDump {
                declaration: OccurrenceId::from_database(1)
            }]
        );
        let warnings = plan
            .bind(&[input(&[10, 20])])
            .expect("observed bytes")
            .execute(&mut region)
            .expect("region");
        assert!(warnings.is_empty());
        assert_eq!(region, [10, 20, 0xaa, 0xaa]);
    }

    #[test]
    fn fill_is_separate_from_files_and_ends_the_active_chain() {
        let load = rom(1, "2", None, SoftwareFileOperation::Load);
        let mut fill = rom(
            2,
            "2",
            Some(SoftwareLoadInstruction::Fill),
            SoftwareFileOperation::Fill,
        );
        fill.declaration_occurrence_id = None;
        fill.offset_text = Some("2".into());
        fill.value = Some("0x7f".into());
        let mut second = rom(3, "2", None, SoftwareFileOperation::Load);
        second.declaration_occurrence_id = Some(OccurrenceId::from_database(3));
        second.offset_text = Some("4".into());
        let roms = [load.clone(), fill.clone(), second];
        let plan = compile(6, &entries(&roms)).expect("two files and a fill");
        assert_eq!(plan.files().len(), 2);
        let mut region = [0xaa; 6];
        let inputs = [
            input(&[10, 20]),
            SoftwareFileInput {
                declaration: OccurrenceId::from_database(3),
                bytes: &[30, 40],
            },
        ];
        plan.bind(&inputs)
            .expect("both files")
            .execute(&mut region)
            .expect("region");
        assert_eq!(region, [10, 20, 127, 127, 30, 40]);
        let dangling = rom(
            3,
            "1",
            Some(SoftwareLoadInstruction::Continue),
            SoftwareFileOperation::Continue,
        );
        let roms = [load, fill, dangling];
        assert!(matches!(
            compile(8, &entries(&roms)),
            Err(SoftwareLoadError::UnresolvedOperation(_))
        ));
    }

    #[test]
    fn refuses_short_missing_duplicate_and_unexpected_files_before_writes() {
        let load = rom(1, "2", None, SoftwareFileOperation::Load);
        let reload = rom(
            2,
            "4",
            Some(SoftwareLoadInstruction::Reload),
            SoftwareFileOperation::Reload,
        );
        let roms = [load, reload];
        let plan = compile(8, &entries(&roms)).expect("recipe");
        assert!(matches!(
            plan.bind(&[]),
            Err(SoftwareLoadError::MissingFile(_))
        ));
        assert!(matches!(
            plan.bind(&[input(&[1, 2])]),
            Err(SoftwareLoadError::ShortFile { required: 4, .. })
        ));
        assert!(matches!(
            plan.bind(&[input(&[1, 2, 3, 4]), input(&[1, 2, 3, 4])]),
            Err(SoftwareLoadError::UnexpectedInput)
        ));
        assert!(matches!(
            plan.bind(&[SoftwareFileInput {
                declaration: OccurrenceId::from_database(99),
                bytes: &[1, 2, 3, 4]
            }]),
            Err(SoftwareLoadError::UnexpectedInput)
        ));
        let mut region = [0xaa; 7];
        assert!(matches!(
            plan.bind(&[input(&[1, 2, 3, 4])])
                .expect("valid source")
                .execute(&mut region),
            Err(SoftwareLoadError::WrongRegionSize)
        ));
        assert_eq!(region, [0xaa; 7]);
    }

    #[test]
    fn raw_numbers_are_checked_without_mutating_source_facts() {
        for size in [
            "",
            "-1",
            "08",
            "0x",
            "9223372036854775808",
            "2 trailing",
            "+010",
            "+08",
            "0x+2",
        ] {
            let roms = [rom(1, size, None, SoftwareFileOperation::Load)];
            assert!(matches!(
                compile(8, &entries(&roms)),
                Err(SoftwareLoadError::InvalidNumber { field: "size", .. })
            ));
            assert_eq!(roms[0].size_text.as_deref(), Some(size));
        }
        for size in ["2", "02", "0x2", "0X2"] {
            let roms = [rom(1, size, None, SoftwareFileOperation::Load)];
            assert_eq!(
                compile(8, &entries(&roms)).expect("valid base").files()[0].expected_length,
                2
            );
        }
        let roms = [rom(1, "0", None, SoftwareFileOperation::Load)];
        assert!(matches!(
            compile(8, &entries(&roms)),
            Err(SoftwareLoadError::Layout(LayoutError::ZeroLength))
        ));
    }

    #[test]
    fn invalid_chains_and_checksum_assertions_do_not_become_recipes() {
        for (instruction, operation) in [
            (
                SoftwareLoadInstruction::Continue,
                SoftwareFileOperation::Continue,
            ),
            (
                SoftwareLoadInstruction::Reload,
                SoftwareFileOperation::Reload,
            ),
            (
                SoftwareLoadInstruction::ReloadPlain,
                SoftwareFileOperation::ReloadPlain,
            ),
            (
                SoftwareLoadInstruction::Ignore,
                SoftwareFileOperation::Ignore,
            ),
        ] {
            let roms = [rom(1, "1", Some(instruction), operation)];
            assert!(matches!(
                compile(8, &entries(&roms)),
                Err(SoftwareLoadError::UnresolvedOperation(_))
            ));
        }
        let mut load = rom(1, "2", None, SoftwareFileOperation::Load);
        load.crc_text = Some("short".into());
        let roms = [load];
        assert!(matches!(
            compile(8, &entries(&roms)),
            Err(SoftwareLoadError::InvalidChecksum(_))
        ));
    }

    #[test]
    fn fill_defaults_and_checked_values_are_independent_of_file_inputs() {
        for value in [None, Some(""), Some("0"), Some("0xff"), Some("0377")] {
            let mut fill = rom(
                1,
                "2",
                Some(SoftwareLoadInstruction::Fill),
                SoftwareFileOperation::Fill,
            );
            fill.declaration_occurrence_id = None;
            fill.value = value.map(str::to_owned);
            let roms = [fill];
            let plan = compile(4, &entries(&roms)).expect("valid fill");
            assert!(plan.files().is_empty());
            let mut region = [0xaa; 4];
            assert!(
                plan.bind(&[])
                    .expect("no sources")
                    .execute(&mut region)
                    .expect("fill")
                    .is_empty()
            );
            let expected = if matches!(value, Some("0xff" | "0377")) {
                255
            } else {
                0
            };
            assert_eq!(region, [expected, expected, 0xaa, 0xaa]);
        }
        for value in ["256", "-1", "bad", "08"] {
            let mut fill = rom(
                1,
                "2",
                Some(SoftwareLoadInstruction::Fill),
                SoftwareFileOperation::Fill,
            );
            fill.declaration_occurrence_id = None;
            fill.value = Some(value.into());
            let roms = [fill];
            assert!(matches!(
                compile(4, &entries(&roms)),
                Err(SoftwareLoadError::InvalidNumber {
                    field: "fill value",
                    ..
                })
            ));
        }
    }

    #[test]
    fn bad_dump_remains_a_warning_even_when_its_checksum_matches() {
        let bytes = [1, 2];
        let mut load = rom(1, "2", None, SoftwareFileOperation::Load);
        load.status = SoftwareDumpStatus::BadDump;
        load.crc_text = Some(hex::encode(crc32fast::hash(&bytes).to_be_bytes()));
        let roms = [load];
        let plan = compile(2, &entries(&roms)).expect("bad dump is still usable");
        let mut region = [0; 2];
        assert_eq!(
            plan.bind(&[input(&bytes)])
                .expect("file")
                .execute(&mut region)
                .expect("region"),
            [SoftwareLoadWarning::KnownBadDump {
                declaration: OccurrenceId::from_database(1)
            }]
        );
        assert_eq!(region, bytes);
    }
}
