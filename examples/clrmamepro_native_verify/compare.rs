//! Independent source-to-native comparisons; no import writer helpers are used.

use std::collections::HashMap;

use mame_coalesce::{
    catalog_clrmamepro::{
        self as native, ClrMameProHeaderField as HeaderField, ClrMameProSetField as SetField,
    },
    catalog_files::{
        CatalogFileOccurrence, ClrMameProDumpStatus, ClrMameProEvidenceScope,
        ClrMameProFieldPosition, ClrMameProFilePayload, ClrMameProRomField as RomField,
        ClrMameProRomPayload, DigestAlgorithm, OccurrenceKind, OccurrenceProvenance, SetGroupKind,
        SourceElementKind, SourceLocation,
    },
    clrmamepro as source,
    domain::OccurrenceId,
    logiqx::RecordLocation,
};

use super::{VerifyResult, digest_verify::compare_declared_digests, equal};

macro_rules! same {
    ($prefix:expr, $name:literal, $source:expr, $native:expr) => {
        equal(&format!("{}.{}", $prefix, $name), &$source, &$native)?
    };
}

pub fn location(prefix: &str, source: RecordLocation, native: SourceLocation) -> VerifyResult {
    same!(
        prefix,
        "location",
        (source.line, source.column),
        (native.line, native.column)
    );
    Ok(())
}

fn position(
    prefix: &str,
    name: &str,
    order: usize,
    quoted: bool,
    source: RecordLocation,
    native: &ClrMameProFieldPosition,
) -> VerifyResult {
    same!(
        prefix,
        "position",
        (name, order, quoted, source.line, source.column),
        (
            native.source_field.as_str(),
            native.source_order,
            native.is_quoted,
            native.location.line,
            native.location.column
        )
    );
    Ok(())
}

fn declared(
    prefix: &str,
    source: &source::FieldValue,
    native: &native::ClrMameProDeclaredValue,
) -> VerifyResult {
    same!(
        prefix,
        "value",
        source.value.as_str(),
        native.value.as_str()
    );
    position(
        prefix,
        &source.source_name,
        source.order,
        source.quoted,
        source.location,
        &native.position,
    )
}

fn header_fields(source: &source::Header) -> Vec<(HeaderField, &source::FieldValue)> {
    let mut fields = [
        (HeaderField::Name, &source.name),
        (HeaderField::Description, &source.description),
        (HeaderField::Version, &source.version),
        (HeaderField::Date, &source.date),
        (HeaderField::Author, &source.author),
        (HeaderField::Email, &source.email),
        (HeaderField::Homepage, &source.homepage),
        (HeaderField::Url, &source.url),
        (HeaderField::Comment, &source.comment),
        (HeaderField::Category, &source.category),
        (HeaderField::HeaderDefinition, &source.directives.header),
        (HeaderField::ForceMerging, &source.directives.forcemerging),
        (HeaderField::ForceZipping, &source.directives.forcezipping),
        (HeaderField::ForcePacking, &source.directives.forcepacking),
        (HeaderField::ForceNoDump, &source.directives.forcenodump),
    ]
    .into_iter()
    .filter_map(|(kind, field)| field.as_ref().map(|value| (kind, value)))
    .collect::<Vec<_>>();
    fields.sort_by_key(|(_, value)| value.order);
    fields
}

pub fn header(
    source: &source::Header,
    native: &native::ClrMameProHeader,
    snapshot: &native::ClrMameProSnapshot,
) -> VerifyResult {
    same!(
        "header",
        "block",
        source.source_name.as_str(),
        native.source_block.as_str()
    );
    same!(
        "header",
        "source_order",
        source.source_order,
        native.source_order
    );
    location("header", source.location, native.location)?;
    let fields = header_fields(source);
    same!("header", "fields.count", fields.len(), native.fields.len());
    for ((kind, source), native) in fields.into_iter().zip(&native.fields) {
        let prefix = format!("header.{}", source.source_name.to_ascii_lowercase());
        same!(&prefix, "kind", kind, native.field);
        declared(&prefix, source, &native.value)?;
    }
    same!(
        "snapshot",
        "declared_version",
        source.version.as_ref().map(|field| field.value.as_str()),
        snapshot.declared_version.as_deref()
    );
    header_effective(source, native)
}

fn header_effective(source: &source::Header, native: &native::ClrMameProHeader) -> VerifyResult {
    let merging = match source.directives.forcemerging.as_ref().map(field_text) {
        Some("none") => Some(native::ClrMameProForceMerging::None),
        Some("split") => Some(native::ClrMameProForceMerging::Split),
        Some("full") => Some(native::ClrMameProForceMerging::Full),
        _ => None,
    };
    let zipping = match source.directives.forcezipping.as_ref().map(field_text) {
        Some("zip") => Some(native::ClrMameProForceZipping::Zip),
        Some("unzip") => Some(native::ClrMameProForceZipping::Unzip),
        _ => None,
    };
    let nodump = match source.directives.forcenodump.as_ref().map(field_text) {
        None | Some("obsolete") => Some(native::ClrMameProForceNoDump::Obsolete),
        Some("required") => Some(native::ClrMameProForceNoDump::Required),
        Some("ignore") => Some(native::ClrMameProForceNoDump::Ignore),
        _ => None,
    };
    same!(
        "header",
        "forcemerging_effective",
        merging,
        native.forcemerging_effective
    );
    same!(
        "header",
        "forcezipping_effective",
        zipping,
        native.forcezipping_effective
    );
    same!(
        "header",
        "forcenodump_effective",
        nodump,
        native.forcenodump_effective
    );
    Ok(())
}

enum Child<'a> {
    Scalar(SetField, &'a source::FieldValue),
    Parent(native::ClrMameProParentKind, &'a source::FieldValue),
    Rom(&'a source::Asset),
    Sample(&'a source::FieldValue),
}

impl Child<'_> {
    const fn order(&self) -> usize {
        match self {
            Self::Scalar(_, value) | Self::Parent(_, value) | Self::Sample(value) => value.order,
            Self::Rom(asset) => asset.native.set_order,
        }
    }
}

fn children(source: &source::Set) -> Vec<Child<'_>> {
    let facts = &source.native;
    let mut children = [
        (SetField::Name, &facts.name),
        (SetField::Description, &facts.description),
        (SetField::Year, &facts.year),
        (SetField::Manufacturer, &facts.manufacturer),
        (SetField::RebuildTo, &facts.rebuildto),
        (SetField::Region, &facts.region),
        (SetField::ReleaseYear, &facts.release_year_text),
        (SetField::ReleaseMonth, &facts.release_month_text),
        (SetField::ReleaseDay, &facts.release_day_text),
        (SetField::Serial, &facts.serial),
    ]
    .into_iter()
    .filter_map(|(kind, field)| field.as_ref().map(|value| Child::Scalar(kind, value)))
    .chain(
        facts
            .cloneof
            .as_ref()
            .map(|value| Child::Parent(native::ClrMameProParentKind::CloneOf, value)),
    )
    .chain(
        facts
            .sampleof
            .as_ref()
            .map(|value| Child::Parent(native::ClrMameProParentKind::SampleOf, value)),
    )
    .chain(source.assets.iter().map(Child::Rom))
    .chain(facts.samples.iter().map(Child::Sample))
    .collect::<Vec<_>>();
    children.sort_by_key(Child::order);
    children
}

struct SetOwners<'a> {
    source: &'a source::Set,
    native: &'a native::ClrMameProSet,
    snapshot: &'a native::ClrMameProSnapshot,
}

pub fn set(
    source: &source::Set,
    native: &native::ClrMameProSet,
    snapshot: &native::ClrMameProSnapshot,
    order: usize,
    files: &mut HashMap<OccurrenceId, CatalogFileOccurrence>,
) -> VerifyResult<usize> {
    let prefix = format!("set[{order}]");
    same!(&prefix, "order", i64::try_from(order)?, native.list_order);
    same!(
        &prefix,
        "block",
        source.native.source_block.as_str(),
        native.source_block.as_str()
    );
    same!(
        &prefix,
        "source_order",
        source.native.document_order,
        native.source_order
    );
    location(&prefix, source.location, native.location)?;
    let expected = children(source);
    same!(
        &prefix,
        "children.count",
        expected.len(),
        native.children.len()
    );
    let mut media = 0_usize;
    let owners = SetOwners {
        source,
        native,
        snapshot,
    };
    for (index, (expected, child)) in expected.iter().zip(&native.children).enumerate() {
        let prefix = format!("{prefix}.child[{index}]");
        same!(
            &prefix,
            "source_order",
            expected.order(),
            child.source_order()
        );
        media = media
            .checked_add(compare_child(
                &prefix, expected, child, &owners, media, files,
            )?)
            .ok_or("too many media entries")?;
    }
    Ok(media)
}

fn compare_child(
    prefix: &str,
    expected: &Child<'_>,
    child: &native::ClrMameProSetChild,
    owners: &SetOwners<'_>,
    media: usize,
    files: &mut HashMap<OccurrenceId, CatalogFileOccurrence>,
) -> VerifyResult<usize> {
    match (expected, child) {
        (
            Child::Scalar(kind, value),
            native::ClrMameProSetChild::Field {
                field,
                value: actual,
            },
        ) => {
            same!(&prefix, "kind", kind, field);
            declared(
                &format!("{prefix}.{}", value.source_name.to_ascii_lowercase()),
                value,
                actual,
            )?;
            Ok(0)
        }
        (Child::Parent(kind, value), native::ClrMameProSetChild::Parent(actual)) => {
            same!(&prefix, "parent_kind", kind, &actual.kind);
            declared(&format!("{prefix}.parent"), value, &actual.target)?;
            Ok(0)
        }
        (Child::Rom(asset), native::ClrMameProSetChild::Rom(actual)) => {
            let file = take_file(files, actual.occurrence_id)?;
            owner(
                prefix,
                &file,
                owners,
                expected,
                media,
                actual.occurrence_order,
            )?;
            let Some(ClrMameProFilePayload::Rom(payload)) = file.clrmamepro_file.as_ref() else {
                return Err(format!("{prefix}: missing or wrong-kind ROM payload").into());
            };
            same!(&prefix, "page_payload", &actual.payload, payload.as_ref());
            rom(asset, &actual.payload, &file, prefix)?;
            Ok(1)
        }
        (Child::Sample(value), native::ClrMameProSetChild::Sample(actual)) => {
            let file = take_file(files, actual.occurrence_id)?;
            owner(
                prefix,
                &file,
                owners,
                expected,
                media,
                actual.occurrence_order,
            )?;
            let Some(ClrMameProFilePayload::Sample(payload)) = file.clrmamepro_file.as_ref() else {
                return Err(format!("{prefix}: missing or wrong-kind sample payload").into());
            };
            same!(&prefix, "page_payload", &actual.payload, payload);
            same!(
                &prefix,
                "sample.name",
                value.value.as_str(),
                actual.payload.name.as_str()
            );
            position(
                prefix,
                &value.source_name,
                value.order,
                value.quoted,
                value.location,
                &actual.payload.position,
            )?;
            compare_declared_digests(
                &format!("{prefix}.sample.digests"),
                std::iter::empty(),
                &file.digests,
            )?;
            unlinked(prefix, &file)?;
            Ok(1)
        }
        _ => Err(format!("{prefix}: native child kind differs from source").into()),
    }
}

fn take_file(
    files: &mut HashMap<OccurrenceId, CatalogFileOccurrence>,
    id: OccurrenceId,
) -> VerifyResult<CatalogFileOccurrence> {
    files
        .remove(&id)
        .ok_or_else(|| format!("missing or duplicate occurrence {id:?}").into())
}

fn owner(
    prefix: &str,
    file: &CatalogFileOccurrence,
    owners: &SetOwners<'_>,
    expected: &Child<'_>,
    order: usize,
    native_order: i64,
) -> VerifyResult {
    let SetOwners {
        source,
        native,
        snapshot,
    } = owners;
    let (kind, source_location, name) = match expected {
        Child::Rom(asset) => (
            OccurrenceKind::ClrMameProRom,
            asset.location,
            asset.name.as_str(),
        ),
        Child::Sample(sample) => (
            OccurrenceKind::ClrMameProSample,
            sample.location,
            sample.value.as_str(),
        ),
        Child::Scalar(..) | Child::Parent(..) => {
            return Err(format!("{prefix}: non-media child has an occurrence owner").into());
        }
    };
    let actual = &file.provenance;
    same!(prefix, "owner.set", native.id, actual.set_id);
    same!(
        prefix,
        "owner.name",
        source.name.as_str(),
        actual.set_name.as_str()
    );
    same!(
        prefix,
        "owner.kind",
        SourceElementKind::ClrMameProSet,
        actual.source_element_kind
    );
    same!(
        prefix,
        "owner.group",
        SetGroupKind::Root,
        actual.set_group_kind
    );
    snapshot_owner(prefix, snapshot, actual)?;
    same!(prefix, "kind", kind, actual.occurrence_kind);
    same!(prefix, "media_order", i64::try_from(order)?, native_order);
    same!(
        prefix,
        "owner.media_order",
        native_order,
        actual.occurrence_order
    );
    same!(
        prefix,
        "asset_name",
        Some(name),
        actual.asset_name.as_deref()
    );
    location(
        &format!("{prefix}.owner.set"),
        source.location,
        actual.set_location,
    )?;
    same!(
        prefix,
        "owner.native_location",
        Some(SourceLocation {
            line: source_location.line,
            column: source_location.column
        }),
        actual.native_occurrence_location
    );
    same!(prefix, "owner.software", &None, &actual.software_owner);
    Ok(())
}

fn snapshot_owner(
    prefix: &str,
    snapshot: &native::ClrMameProSnapshot,
    actual: &OccurrenceProvenance,
) -> VerifyResult {
    let document = snapshot.document_key.to_string();
    for (field, expected, actual) in [
        ("format", snapshot.format.as_str(), actual.format.as_str()),
        (
            "snapshot",
            snapshot.snapshot_key.as_str(),
            actual.snapshot_key.as_str(),
        ),
        ("document", document.as_str(), actual.document_key.as_str()),
        (
            "interpretation",
            snapshot.interpretation_key.as_str(),
            actual.interpretation_key.as_str(),
        ),
        (
            "source",
            snapshot.source_key.as_str(),
            actual.source_key.as_str(),
        ),
        (
            "catalog",
            snapshot.catalog_key.as_str(),
            actual.catalog_key.as_str(),
        ),
    ] {
        equal(&format!("{prefix}.owner.{field}"), expected, actual)?;
    }
    Ok(())
}

const fn field_text(field: &source::FieldValue) -> &str {
    field.value.as_str()
}

pub fn rom(
    source: &source::Asset,
    native: &ClrMameProRomPayload,
    file: &CatalogFileOccurrence,
    prefix: &str,
) -> VerifyResult {
    let facts = &source.native;
    same!(prefix, "name", source.name.as_str(), native.name.as_str());
    rom_text(prefix, facts, native)?;
    same!(
        prefix,
        "size",
        source.size.map(i64::try_from).transpose()?,
        native.size
    );
    same!(
        prefix,
        "nodump",
        facts.nodump.is_some(),
        native.nodump_present
    );
    same!(
        prefix,
        "baddump",
        facts.baddump.is_some(),
        native.baddump_present
    );
    same!(prefix, "source_order", facts.set_order, native.source_order);
    location(prefix, source.location, native.location)?;
    same!(
        prefix,
        "evidence_scope",
        ClrMameProEvidenceScope::WholeAsset,
        native.evidence_scope
    );
    let status = match source.status.as_deref() {
        Some("good") => Some(ClrMameProDumpStatus::Good),
        Some("baddump") => Some(ClrMameProDumpStatus::BadDump),
        Some("nodump") => Some(ClrMameProDumpStatus::NoDump),
        Some("verified") => Some(ClrMameProDumpStatus::Verified),
        _ => None,
    };
    same!(prefix, "dump_status", status, native.dump_status);
    rom_positions(prefix, facts, native)?;
    rom_digests(prefix, facts, file)?;
    let conflicting_crc = facts
        .crc
        .as_ref()
        .zip(facts.crc32.as_ref())
        .is_some_and(|(a, b)| !a.value.eq_ignore_ascii_case(&b.value));
    let conflicting_dump = facts.nodump.is_some() && facts.baddump.is_some()
        || facts.status_field.is_some() && (facts.nodump.is_some() || facts.baddump.is_some());
    if source.sha1.is_none() || conflicting_crc || conflicting_dump {
        unlinked(prefix, file)?;
    }
    Ok(())
}

fn rom_text(
    prefix: &str,
    facts: &source::AssetFacts,
    native: &ClrMameProRomPayload,
) -> VerifyResult {
    for (field, expected, actual) in [
        (
            "size_text",
            facts.size.as_ref(),
            native.size_text.as_deref(),
        ),
        ("crc", facts.crc.as_ref(), native.crc_text.as_deref()),
        ("crc32", facts.crc32.as_ref(), native.crc32_text.as_deref()),
        ("md5", facts.md5.as_ref(), native.md5_text.as_deref()),
        ("sha1", facts.sha1.as_ref(), native.sha1_text.as_deref()),
        ("date", facts.date.as_ref(), native.date.as_deref()),
        ("serial", facts.serial.as_ref(), native.serial.as_deref()),
        (
            "status",
            facts.status_field.as_ref(),
            native.status_text.as_deref(),
        ),
        (
            "merge",
            facts.merge.as_ref(),
            native.merge.as_ref().map(|merge| merge.merge_name.as_str()),
        ),
    ] {
        equal(
            &format!("{prefix}.{field}"),
            &expected.map(field_text),
            &actual,
        )?;
    }
    Ok(())
}

fn unlinked(prefix: &str, file: &CatalogFileOccurrence) -> VerifyResult {
    same!(prefix, "content_id", &None, &file.content_id);
    same!(
        prefix,
        "canonical_content_id",
        &None,
        &file.canonical_content_id
    );
    Ok(())
}

fn rom_positions(
    prefix: &str,
    facts: &source::AssetFacts,
    native: &ClrMameProRomPayload,
) -> VerifyResult {
    let scalars = [
        (RomField::Name, &facts.name),
        (RomField::Size, &facts.size),
        (RomField::Crc, &facts.crc),
        (RomField::Crc32, &facts.crc32),
        (RomField::Md5, &facts.md5),
        (RomField::Sha1, &facts.sha1),
        (RomField::Merge, &facts.merge),
        (RomField::Date, &facts.date),
        (RomField::Serial, &facts.serial),
        (RomField::Status, &facts.status_field),
    ];
    let mut expected = scalars
        .into_iter()
        .filter_map(|(kind, field)| {
            field
                .as_ref()
                .map(|f| (kind, f.source_name.as_str(), f.order, f.quoted, f.location))
        })
        .chain(
            [
                (RomField::NoDump, &facts.nodump),
                (RomField::BadDump, &facts.baddump),
            ]
            .into_iter()
            .filter_map(|(kind, field)| {
                field
                    .as_ref()
                    .map(|f| (kind, f.source_name.as_str(), f.order, false, f.location))
            }),
        )
        .collect::<Vec<_>>();
    expected.sort_by_key(|(_, _, order, _, _)| *order);
    same!(
        prefix,
        "positions.count",
        expected.len(),
        native.field_positions.len()
    );
    for ((kind, name, order, quoted, location), actual) in
        expected.into_iter().zip(&native.field_positions)
    {
        same!(prefix, "position.kind", kind, actual.field);
        position(prefix, name, order, quoted, location, &actual.position)?;
    }
    Ok(())
}

fn rom_digests(
    prefix: &str,
    facts: &source::AssetFacts,
    file: &CatalogFileOccurrence,
) -> VerifyResult {
    let mut expected = [
        (DigestAlgorithm::Crc32, &facts.crc),
        (DigestAlgorithm::Crc32, &facts.crc32),
        (DigestAlgorithm::Md5, &facts.md5),
        (DigestAlgorithm::Sha1, &facts.sha1),
    ]
    .into_iter()
    .filter_map(|(kind, field)| {
        field
            .as_ref()
            .map(|field| hex::decode(&field.value).map(|bytes| (kind, bytes)))
    })
    .collect::<Result<Vec<_>, _>>()?;
    expected.sort();
    expected.dedup();
    compare_declared_digests(
        &format!("{prefix}.declared_digests"),
        expected
            .iter()
            .map(|(kind, bytes)| (*kind, bytes.as_slice(), "whole_asset")),
        &file.digests,
    )
}
