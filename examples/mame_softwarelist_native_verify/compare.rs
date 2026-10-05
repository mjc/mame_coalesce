//! Independent source-to-query comparisons; no import writer helpers are used.

use std::collections::HashMap;

use mame_coalesce::{
    catalog_files::{
        CatalogFileOccurrence, DigestAlgorithm, OccurrenceKind, SoftwareAreaKind,
        SoftwareDiskPayload, SoftwareFileOperation, SoftwareFilePayload, SoftwareRomPayload,
        SourceElementKind, SourceLocation,
    },
    catalog_software as native,
    domain::OccurrenceId,
    logiqx::RecordLocation,
    mame_softwarelist as source,
};

use super::{
    VerifyResult, digest_verify::compare_declared_digests, equal,
    xml_verify::compare_media_positions,
};

macro_rules! same {
    ($prefix:expr, $field:literal, $source:expr, $native:expr) => {
        equal(&format!("{}.{}", $prefix, $field), &$source, &$native)?
    };
}

fn record(
    prefix: &str,
    order: usize,
    source_order: usize,
    location: RecordLocation,
    native_order: i64,
    native_source_order: i64,
    native_location: SourceLocation,
) -> VerifyResult {
    same!(prefix, "order", i64::try_from(order)?, native_order);
    same!(
        prefix,
        "source_order",
        i64::try_from(source_order)?,
        native_source_order
    );
    same!(
        prefix,
        "location",
        (location.line, location.column),
        (native_location.line, native_location.column)
    );
    Ok(())
}

pub fn text_positions(
    prefix: &str,
    source: &[source::SoftwareTextPosition],
    native: &[native::SoftwareTextPosition],
) -> VerifyResult {
    same!(prefix, "text_positions.count", source.len(), native.len());
    for (source, native) in source.iter().zip(native) {
        same!(
            prefix,
            "text_positions",
            (
                &source.field,
                i64::try_from(source.source_order)?,
                source.location.line,
                source.location.column
            ),
            (
                &native.field,
                native.source_order,
                native.location.line,
                native.location.column
            )
        );
    }
    Ok(())
}

pub fn title(
    source: &source::SoftwareItem,
    native: &native::SoftwareTitle,
    order: usize,
    list: &native::SoftwareList,
    files: &mut HashMap<OccurrenceId, CatalogFileOccurrence>,
) -> VerifyResult {
    let prefix = format!("list[{}].title[{order}]", list.order);
    title_fields(source, native, order, &prefix)?;
    let mut occurrence_order = 0_usize;
    for (index, (part_source, part_native)) in source.parts.iter().zip(&native.parts).enumerate() {
        part(part_source, part_native, index, &prefix)?;
        for (area_index, (area_source, area_native)) in
            part_source.areas.iter().zip(&part_native.areas).enumerate()
        {
            let prefix = format!("{prefix}.part[{index}].area[{area_index}]");
            let mut declaration = None;
            for (component_index, (component, id)) in area_source
                .components
                .iter()
                .zip(&area_native.entry_ids)
                .enumerate()
            {
                let file = files
                    .remove(id)
                    .ok_or_else(|| format!("{prefix}: missing or duplicate occurrence {id:?}"))?;
                let component_prefix = format!("{prefix}.entry[{component_index}]");
                owner(
                    &file,
                    native,
                    list,
                    part_native,
                    area_native,
                    occurrence_order,
                    &component_prefix,
                )?;
                entry(
                    component,
                    &file,
                    component_index,
                    &mut declaration,
                    &component_prefix,
                )?;
                occurrence_order = occurrence_order
                    .checked_add(1)
                    .ok_or("too many software entries")?;
            }
        }
    }
    Ok(())
}

fn title_fields(
    source: &source::SoftwareItem,
    native: &native::SoftwareTitle,
    order: usize,
    prefix: &str,
) -> VerifyResult {
    record(
        prefix,
        order,
        source.source_order,
        source.location,
        native.order,
        native.source_order,
        native.location,
    )?;
    same!(prefix, "name", source.name.as_str(), native.name.as_str());
    same!(
        prefix,
        "cloneof",
        source
            .clone_of
            .as_ref()
            .map(source::SoftwareItemName::as_str),
        native.clone_of.as_deref()
    );
    same!(
        prefix,
        "supported",
        source.supported.unwrap_or_default(),
        native.supported
    );
    same!(
        prefix,
        "supported_specified",
        source.supported_specified,
        native.supported_specified
    );
    same!(
        prefix,
        "description",
        source.description,
        native.description
    );
    same!(prefix, "year", source.year, native.year);
    same!(prefix, "publisher", source.publisher, native.publisher);
    same!(prefix, "notes", source.notes, native.notes);
    compare_media_positions(
        &format!("{prefix}.attributes"),
        &source.attribute_positions,
        &native.attribute_positions,
    )?;
    text_positions(prefix, &source.text_positions, &native.text_positions)?;
    named_values(&format!("{prefix}.info"), &source.info, &native.info)?;
    named_values(
        &format!("{prefix}.sharedfeat"),
        &source.shared_features,
        &native.shared_features,
    )?;
    same!(
        prefix,
        "parts.count",
        source.parts.len(),
        native.parts.len()
    );
    Ok(())
}

fn entry(
    source: &source::SoftwareComponent,
    file: &CatalogFileOccurrence,
    index: usize,
    declaration: &mut Option<OccurrenceId>,
    prefix: &str,
) -> VerifyResult {
    match (source, file.software_file.as_ref()) {
        (source::SoftwareComponent::Rom(source), Some(SoftwareFilePayload::Rom(native))) => {
            rom(source, native, file, prefix)?;
            let operation = operation(source.load);
            let expected_kind = if operation == SoftwareFileOperation::Load {
                *declaration = Some(file.occurrence_id);
                OccurrenceKind::SoftwareRomEntry
            } else {
                if operation == SoftwareFileOperation::Fill {
                    *declaration = None;
                }
                OccurrenceKind::SoftwareRomOperation
            };
            same!(
                prefix,
                "kind",
                expected_kind,
                file.provenance.occurrence_kind
            );
            same!(
                prefix,
                "declaration",
                *declaration,
                native.declaration_occurrence_id
            );
            record(
                prefix,
                index,
                source.source_order,
                source.location,
                native.component_order,
                native.source_order,
                native.location,
            )?;
        }
        (source::SoftwareComponent::Disk(source), Some(SoftwareFilePayload::Disk(native))) => {
            disk(source, native, file, prefix)?;
            same!(
                prefix,
                "kind",
                OccurrenceKind::SoftwareDiskEntry,
                file.provenance.occurrence_kind
            );
            record(
                prefix,
                index,
                source.source_order,
                source.location,
                native.component_order,
                native.source_order,
                native.location,
            )?;
        }
        _ => return Err(format!("{prefix}: native payload kind differs from source").into()),
    }
    Ok(())
}

fn named_values(
    prefix: &str,
    source: &[source::NamedValue],
    native: &[native::SoftwareNamedValue],
) -> VerifyResult {
    same!(prefix, "count", source.len(), native.len());
    for (index, (source, native)) in source.iter().zip(native).enumerate() {
        let prefix = format!("{prefix}[{index}]");
        record(
            &prefix,
            index,
            source.source_order,
            source.location,
            native.order,
            native.source_order,
            native.location,
        )?;
        same!(prefix, "name", source.name, native.name);
        same!(prefix, "value", source.value, native.value);
        compare_media_positions(
            &format!("{prefix}.attributes"),
            &source.attribute_positions,
            &native.attribute_positions,
        )?;
    }
    Ok(())
}

fn part(
    source: &source::SoftwarePart,
    native: &native::SoftwarePart,
    index: usize,
    parent: &str,
) -> VerifyResult {
    let prefix = format!("{parent}.part[{index}]");
    record(
        &prefix,
        index,
        source.source_order,
        source.location,
        native.order,
        native.source_order,
        native.location,
    )?;
    same!(prefix, "name", source.name.as_str(), native.name.as_str());
    same!(prefix, "interface", source.interface, native.interface);
    compare_media_positions(
        &format!("{prefix}.attributes"),
        &source.attribute_positions,
        &native.attribute_positions,
    )?;
    named_values(
        &format!("{prefix}.feature"),
        &source.features,
        &native.features,
    )?;
    same!(
        prefix,
        "dipswitch.count",
        source.dipswitches.len(),
        native.switches.len()
    );
    for (index, (source, native)) in source.dipswitches.iter().zip(&native.switches).enumerate() {
        switch(source, native, index, &prefix)?;
    }
    same!(
        prefix,
        "areas.count",
        source.areas.len(),
        native.areas.len()
    );
    for (index, (source, native)) in source.areas.iter().zip(&native.areas).enumerate() {
        area(source, native, index, &prefix)?;
    }
    Ok(())
}

fn switch(
    source: &source::SoftwareDipSwitch,
    native: &native::SoftwareDipSwitch,
    index: usize,
    parent: &str,
) -> VerifyResult {
    let prefix = format!("{parent}.dipswitch[{index}]");
    record(
        &prefix,
        index,
        source.source_order,
        source.location,
        native.order,
        native.source_order,
        native.location,
    )?;
    same!(prefix, "name", source.name.as_str(), native.name.as_str());
    same!(prefix, "tag", source.tag, native.tag);
    same!(prefix, "mask", source.mask, native.mask);
    compare_media_positions(
        &format!("{prefix}.attributes"),
        &source.attribute_positions,
        &native.attribute_positions,
    )?;
    same!(
        prefix,
        "values.count",
        source.values.len(),
        native.values.len()
    );
    for (index, (source, native)) in source.values.iter().zip(&native.values).enumerate() {
        let prefix = format!("{prefix}.value[{index}]");
        record(
            &prefix,
            index,
            source.source_order,
            source.location,
            native.order,
            native.source_order,
            native.location,
        )?;
        same!(prefix, "name", source.name, native.name);
        same!(prefix, "value", source.value, native.value);
        same!(prefix, "default", source.is_default, native.is_default);
        same!(
            prefix,
            "default_specified",
            source.default_specified,
            native.default_specified
        );
        compare_media_positions(
            &format!("{prefix}.attributes"),
            &source.attribute_positions,
            &native.attribute_positions,
        )?;
    }
    Ok(())
}

fn area(
    source: &source::SoftwareArea,
    native: &native::SoftwareArea,
    index: usize,
    parent: &str,
) -> VerifyResult {
    let prefix = format!("{parent}.area[{index}]");
    record(
        &prefix,
        index,
        source.source_order,
        source.location,
        native.order,
        native.source_order,
        native.location,
    )?;
    same!(prefix, "name", source.name.as_str(), native.name.as_str());
    match (&source.attribute_positions, &native.attribute_positions) {
        (
            source::SoftwareAreaAttributePositions::Data(source),
            native::SoftwareAreaAttributePositions::Data(native),
        ) => compare_media_positions(&format!("{prefix}.attributes"), source, native)?,
        (
            source::SoftwareAreaAttributePositions::Disk(source),
            native::SoftwareAreaAttributePositions::Disk(native),
        ) => compare_media_positions(&format!("{prefix}.attributes"), source, native)?,
        _ => return Err(format!("{prefix}: area attribute family differs").into()),
    }
    match (source.kind, &native.fields) {
        (
            source::AreaKind::Data,
            native::SoftwareAreaFields::Data {
                size_text,
                size,
                width,
                width_specified,
                endianness,
                endianness_specified,
            },
        ) => {
            same!(
                prefix,
                "size_text",
                source.declared_size_text.as_deref(),
                Some(size_text.as_str())
            );
            same!(prefix, "size", checked_number(source.declared_size), *size);
            let width = match width {
                native::SoftwareDataWidth::Bits8 => 8,
                native::SoftwareDataWidth::Bits16 => 16,
                native::SoftwareDataWidth::Bits32 => 32,
                native::SoftwareDataWidth::Bits64 => 64,
            };
            same!(prefix, "width", source.width.unwrap_or(8), width);
            same!(
                prefix,
                "width_specified",
                source.width_specified,
                *width_specified
            );
            same!(
                prefix,
                "endianness",
                source.endianness.unwrap_or(source::Endianness::Little),
                *endianness
            );
            same!(
                prefix,
                "endianness_specified",
                source.endianness_specified,
                *endianness_specified
            );
        }
        (source::AreaKind::Disk, native::SoftwareAreaFields::Disk) => {}
        _ => return Err(format!("{prefix}: native area kind differs from source").into()),
    }
    same!(
        prefix,
        "entries.count",
        source.components.len(),
        native.entry_ids.len()
    );
    Ok(())
}

fn owner(
    file: &CatalogFileOccurrence,
    title: &native::SoftwareTitle,
    list: &native::SoftwareList,
    part: &native::SoftwarePart,
    area: &native::SoftwareArea,
    order: usize,
    prefix: &str,
) -> VerifyResult {
    let provenance = &file.provenance;
    same!(
        prefix,
        "list_id",
        list.id.database_value(),
        provenance.set_group_id
    );
    same!(
        prefix,
        "list_kind",
        mame_coalesce::catalog_files::SetGroupKind::SoftwareList {
            name: list.name.clone()
        },
        provenance.set_group_kind
    );
    same!(prefix, "title_id", title.id, provenance.set_id);
    same!(prefix, "title_name", title.name, provenance.set_name);
    same!(
        prefix,
        "element_kind",
        SourceElementKind::SoftwareItem,
        provenance.source_element_kind
    );
    same!(
        prefix,
        "occurrence_order",
        i64::try_from(order)?,
        provenance.occurrence_order
    );
    same!(
        prefix,
        "title_location",
        title.location,
        provenance.set_location
    );
    let owner = provenance
        .software_owner
        .as_ref()
        .ok_or("missing native software owner")?;
    let area_kind = match area.fields {
        native::SoftwareAreaFields::Data { .. } => SoftwareAreaKind::Data,
        native::SoftwareAreaFields::Disk => SoftwareAreaKind::Disk,
    };
    same!(prefix, "part_id", part.id.database_value(), owner.part_id);
    same!(prefix, "part_order", part.order, owner.part_order);
    same!(prefix, "part_name", part.name, owner.part_name);
    same!(prefix, "area_id", area.id.database_value(), owner.area_id);
    same!(prefix, "area_order", area.order, owner.area_order);
    same!(prefix, "area_name", area.name, owner.area_name);
    same!(prefix, "area_kind", area_kind, owner.area_kind);
    Ok(())
}

fn rom_fields(
    source: &source::SoftwareRom,
    native: &SoftwareRomPayload,
    prefix: &str,
) -> VerifyResult {
    same!(
        prefix,
        "name",
        source.name.as_ref().map(source::ComponentName::as_str),
        native.name.as_deref()
    );
    same!(prefix, "size_text", source.size_text, native.size_text);
    same!(prefix, "size", checked_number(source.size), native.size);
    same!(
        prefix,
        "offset_text",
        source.offset_text,
        native.offset_text
    );
    same!(
        prefix,
        "offset",
        checked_number(source.offset),
        native.offset
    );
    same!(prefix, "value", source.value, native.value);
    same!(prefix, "crc_text", source.crc_text, native.crc_text);
    same!(prefix, "sha1_text", source.sha1_text, native.sha1_text);
    same!(
        prefix,
        "status",
        source.status.unwrap_or_default(),
        native.status
    );
    same!(
        prefix,
        "status_specified",
        source.status_specified,
        native.status_specified
    );
    same!(prefix, "loadflag", source.load, native.load_instruction);
    Ok(())
}

pub fn rom(
    source: &source::SoftwareRom,
    native: &SoftwareRomPayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    rom_fields(source, native, prefix)?;
    let operation = operation(source.load);
    same!(prefix, "operation", operation, native.operation);
    let scope = if operation == SoftwareFileOperation::Load
        && source
            .name
            .as_ref()
            .is_some_and(|name| !name.as_str().is_empty())
        && source.status != Some(source::DumpStatus::NoDump)
    {
        "whole_asset"
    } else {
        "unknown"
    };
    same!(
        prefix,
        "evidence_scope",
        scope,
        native.evidence_scope.as_str()
    );
    same!(
        prefix,
        "asset_name",
        if operation == SoftwareFileOperation::Load {
            source.name.as_ref().map(source::ComponentName::as_str)
        } else {
            None
        },
        file.provenance.asset_name.as_deref()
    );
    same!(
        prefix,
        "native_location",
        Some(native.location),
        file.provenance.native_occurrence_location
    );
    if scope != "whole_asset"
        || source.sha1.is_none()
        || (source.crc_text.is_some() && source.crc.is_none())
    {
        unlinked_content(prefix, file)?;
    }
    compare_media_positions(
        &format!("{prefix}.attributes"),
        &source.attribute_positions,
        &native.attribute_positions,
    )?;
    compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        source
            .crc
            .as_ref()
            .map(|digest| (DigestAlgorithm::Crc32, digest.as_slice(), scope))
            .into_iter()
            .chain(
                source
                    .sha1
                    .as_ref()
                    .map(|digest| (DigestAlgorithm::Sha1, digest.as_slice(), scope)),
            ),
        &file.digests,
    )
}

pub fn disk(
    source: &source::SoftwareDisk,
    native: &SoftwareDiskPayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    same!(
        prefix,
        "name",
        source.requirement.name().as_str(),
        native.name.as_str()
    );
    same!(prefix, "sha1_text", source.sha1_text, native.sha1_text);
    same!(
        prefix,
        "status",
        source.status.unwrap_or_default(),
        native.status
    );
    same!(
        prefix,
        "status_specified",
        source.status_specified,
        native.status_specified
    );
    same!(
        prefix,
        "writeable",
        source.writeable.unwrap_or(false),
        native.writeable
    );
    same!(
        prefix,
        "writeable_specified",
        source.writeable_specified,
        native.writeable_specified
    );
    same!(
        prefix,
        "evidence_scope",
        source.requirement.digest_scope().as_str(),
        native.evidence_scope.as_str()
    );
    same!(
        prefix,
        "asset_name",
        Some(source.requirement.name().as_str()),
        file.provenance.asset_name.as_deref()
    );
    same!(
        prefix,
        "native_location",
        Some(native.location),
        file.provenance.native_occurrence_location
    );
    unlinked_content(prefix, file)?;
    compare_media_positions(
        &format!("{prefix}.attributes"),
        &source.attribute_positions,
        &native.attribute_positions,
    )?;
    let sha1 = source.requirement.expected_sha1();
    compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        sha1.as_ref().map(|digest| {
            (
                DigestAlgorithm::Sha1,
                digest.as_bytes().as_slice(),
                source.requirement.digest_scope().as_str(),
            )
        }),
        &file.digests,
    )
}

fn unlinked_content(prefix: &str, file: &CatalogFileOccurrence) -> VerifyResult {
    same!(prefix, "content_uuid", None, file.content_id);
    same!(
        prefix,
        "canonical_content_uuid",
        None,
        file.canonical_content_id
    );
    Ok(())
}

fn checked_number(source: Option<u64>) -> Option<i64> {
    source.and_then(|value| i64::try_from(value).ok())
}

const fn operation(load: Option<source::LoadInstruction>) -> SoftwareFileOperation {
    use source::LoadInstruction as Load;
    match load {
        Some(Load::Continue) => SoftwareFileOperation::Continue,
        Some(Load::Reload) => SoftwareFileOperation::Reload,
        Some(Load::ReloadPlain) => SoftwareFileOperation::ReloadPlain,
        Some(Load::Ignore) => SoftwareFileOperation::Ignore,
        Some(Load::Fill) => SoftwareFileOperation::Fill,
        None
        | Some(
            Load::Load16Byte
            | Load::Load16Word
            | Load::Load16WordSwap
            | Load::Load32Byte
            | Load::Load32Word
            | Load::Load32WordSwap
            | Load::Load32Dword
            | Load::Load64Word
            | Load::Load64WordSwap,
        ) => SoftwareFileOperation::Load,
    }
}
