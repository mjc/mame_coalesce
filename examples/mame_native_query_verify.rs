//! Verify one original MAME machine XML document against a published native snapshot.
//!
//! The caller supplies the original; native queries never reopen it.

use std::collections::HashMap;

use camino::Utf8Path;
use mame_coalesce::{
    catalog_files::{self, MameFilePayload},
    catalog_machines::{self, MachineCursor, MachinePage, MachinePageLimit},
    database::Database,
    domain::{AssetRole, SnapshotKey},
    mame::{self, MameHeader, MameRecord},
};

mod support;

#[path = "support/digest_verify.rs"]
mod digest_verify;
#[path = "support/xml_verify.rs"]
mod xml_verify;

use xml_verify::compare_media_positions;

use self::support::catalog_verify::{VerifyResult, equal, open_existing_catalog};

const PAGE_SIZE: usize = 64;
const MAX_MEDIA_IDS_PER_REQUEST: usize = 10_000;
const EXPECTED_RULES_VERSION: &str = "mame-observed-compat-declared-text-v2";
pub const NOT_COMPARED: &[&str] =
    &["XmlExtension payloads: catalog_machines has no typed extension-payload field"];

fn main() -> VerifyResult {
    let mut arguments = std::env::args().skip(1);
    let database_path = arguments.next().ok_or("missing DATABASE")?;
    let snapshot: SnapshotKey = arguments.next().ok_or("missing SNAPSHOT_KEY")?.parse()?;
    let source_path = arguments.next().ok_or("missing SOURCE_PATH")?;
    if arguments.next().is_some() {
        return Err("expected DATABASE SNAPSHOT_KEY SOURCE_PATH".into());
    }
    let database = open_existing_catalog(Utf8Path::new(&database_path))?;
    let source = std::fs::read(source_path)?;
    let report = verify_mame_source(&database, &snapshot, &source)?;
    println!(
        "VERIFIED snapshot={} machines={} extensions_not_compared={}",
        snapshot.as_str(),
        report.machines,
        report.source_extensions_not_compared
    );
    println!(
        "Memory: original/decoded source buffers and parser bookkeeping remain input-dependent; one parsed machine and one {PAGE_SIZE}-machine native page, including their complete children."
    );
    for limitation in report.not_compared {
        println!("Not compared: {limitation}");
    }
    Ok(())
}

/// Verify an already-open published catalog against source bytes.
///
/// The caller should obtain the database with `open_existing_catalog`; this
/// function never creates or initializes a database. Source parsing and query
/// traversal retain a current machine and one query page, with complete children.
/// Original/decoded buffers and parser coordinate bookkeeping remain input-dependent.
pub fn verify_mame_source(
    database: &Database,
    snapshot: &SnapshotKey,
    source_bytes: &[u8],
) -> VerifyResult<VerificationReport> {
    let completed = mame::read_with::<_, Box<dyn std::error::Error>>(
        source_bytes,
        |header| {
            let mut verifier = Verifier::new(database, snapshot, PAGE_SIZE)?;
            verifier.compare_header(&header)?;
            Ok(verifier)
        },
        Verifier::compare_record,
    )?;
    completed.into_inner().finish()
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerificationReport {
    pub machines: usize,
    pub source_extensions_not_compared: usize,
    pub not_compared: &'static [&'static str],
}

struct Verifier<'db> {
    database: &'db Database,
    snapshot: &'db SnapshotKey,
    page_limit: MachinePageLimit,
    page: MachinePage,
    page_index: usize,
    cursor: Option<MachineCursor>,
    source_machine_count: usize,
    source_extensions_not_compared: usize,
}

impl<'db> Verifier<'db> {
    fn new(
        database: &'db Database,
        snapshot: &'db SnapshotKey,
        page_size: usize,
    ) -> VerifyResult<Self> {
        let page_limit = MachinePageLimit::new(page_size)?;
        let page = catalog_machines::machines_for_snapshot(database, snapshot, None, page_limit)?;
        let cursor = page.next_cursor.clone();
        Ok(Self {
            database,
            snapshot,
            page_limit,
            page,
            page_index: 0,
            cursor,
            source_machine_count: 0,
            source_extensions_not_compared: 0,
        })
    }

    fn compare_header(&mut self, source: &MameHeader) -> VerifyResult {
        equal(
            "snapshot.key",
            &self.snapshot.as_str(),
            &self.page.snapshot.snapshot_key.as_str(),
        )?;
        equal(
            "snapshot.format",
            &"mame-listxml",
            &self.page.snapshot.format.as_str(),
        )?;
        equal(
            "snapshot.rules_version",
            &Some(EXPECTED_RULES_VERSION.to_owned()),
            &self.page.snapshot.rules_version,
        )?;
        let catalog = &self.page.snapshot.header;
        equal("header.build", &source.build, &catalog.build)?;
        equal("header.debug", &source.debug, &catalog.debug)?;
        equal(
            "header.debug_specified",
            &source.debug_specified,
            &catalog.debug_specified,
        )?;
        equal(
            "header.config_version",
            &source.config_version,
            &catalog.config_version,
        )?;
        equal(
            "header.attribute_positions",
            &source.attribute_positions,
            &catalog.attribute_positions,
        )?;
        equal(
            "header.location",
            &(source.location.line, source.location.column),
            &(catalog.location.line, catalog.location.column),
        )?;
        self.source_extensions_not_compared += source.extensions.len();
        Ok(())
    }

    fn compare_record(&mut self, record: MameRecord) -> VerifyResult {
        match record {
            MameRecord::Machine(machine) => self.compare_machine(&machine),
            MameRecord::Extension(_) => {
                self.source_extensions_not_compared += 1;
                Ok(())
            }
        }
    }

    fn compare_machine(&mut self, source: &mame::Machine) -> VerifyResult {
        let source_order = i64::try_from(self.source_machine_count)?;
        let database = self.database;
        let catalog = self.next_machine()?;
        equal("machine.name", &source.name, &catalog.name)?;
        equal("machine.list_order", &source_order, &catalog.list_order)?;
        equal(
            "machine.location",
            &(source.location.line, source.location.column),
            &(catalog.location.line, catalog.location.column),
        )?;

        // Both public interfaces use the same native facts type. This compares
        // every raw/defaulted field and both machine attribute-position vectors.
        equal("machine.facts", &source.facts, &catalog.facts)?;
        equal("machine.bios_sets", &source.bios_sets, &catalog.bios_sets)?;
        equal(
            "machine.specification",
            &source.specification,
            &catalog.specification,
        )?;
        compare_dependencies(source, catalog)?;
        compare_switches(source, catalog)?;
        compare_assets(database, source, catalog)?;

        // Source XML extensions are intentionally counted but not projected
        // through JSON. The public typed machine API has no extension field.
        self.source_extensions_not_compared += source.extensions.len()
            + source
                .assets
                .iter()
                .map(|asset| asset.extensions.len())
                .sum::<usize>();
        self.source_machine_count = self
            .source_machine_count
            .checked_add(1)
            .ok_or("source machine count overflow")?;
        Ok(())
    }

    fn next_machine(&mut self) -> VerifyResult<&catalog_machines::Machine> {
        if self.page_index == self.page.machines.len() {
            let Some(cursor) = self.cursor.take() else {
                return Err(format!(
                    "catalog has no machine at source order {}",
                    self.source_machine_count
                )
                .into());
            };
            self.page = catalog_machines::machines_for_snapshot(
                self.database,
                self.snapshot,
                Some(&cursor),
                self.page_limit,
            )?;
            if self.page.machines.is_empty() {
                return Err("machine query returned an empty continuation page".into());
            }
            self.cursor = self.page.next_cursor.clone();
            self.page_index = 0;
        }
        let index = self.page_index;
        self.page_index += 1;
        self.page
            .machines
            .get(index)
            .ok_or_else(|| "machine page index escaped its page".into())
    }

    fn finish(&self) -> VerifyResult<VerificationReport> {
        if self.source_machine_count == 0 {
            return Err("source document contains no completed machines".into());
        }
        if self.page_index < self.page.machines.len() || self.cursor.is_some() {
            let next_order = self
                .page
                .machines
                .get(self.page_index)
                .map(|machine| usize::try_from(machine.list_order))
                .transpose()?
                .unwrap_or(self.source_machine_count);
            return Err(format!("catalog has an extra machine at list order {next_order}").into());
        }
        Ok(VerificationReport {
            machines: self.source_machine_count,
            source_extensions_not_compared: self.source_extensions_not_compared,
            not_compared: NOT_COMPARED,
        })
    }
}

fn compare_dependencies(
    source: &mame::Machine,
    catalog: &catalog_machines::Machine,
) -> VerifyResult {
    // Link values and device references are compared as ordered source facts.
    // Query relationship IDs and generated set IDs are deliberately excluded.
    for (kind, source_target) in [
        ("cloneof", source.parent.as_deref()),
        ("romof", source.rom_of.as_deref()),
        ("sampleof", source.sample_of.as_deref()),
    ] {
        let catalog_target =
            catalog
                .dependencies
                .iter()
                .find_map(|dependency| match (kind, dependency) {
                    (
                        "cloneof",
                        catalog_machines::MachineDependency::CloneOf { target_name, .. },
                    )
                    | ("romof", catalog_machines::MachineDependency::RomOf { target_name, .. })
                    | (
                        "sampleof",
                        catalog_machines::MachineDependency::SampleOf { target_name, .. },
                    ) => Some(target_name.as_str()),
                    _ => None,
                });
        equal(&format!("machine.{kind}"), &source_target, &catalog_target)?;
    }

    let source_references = source
        .device_refs
        .iter()
        .map(|reference| (&reference.name, &reference.tag, reference.source_order))
        .collect::<Vec<_>>();
    let catalog_references = catalog
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            catalog_machines::MachineDependency::DeviceReference(reference) => {
                Some((&reference.name, &reference.tag, reference.source_order))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    equal(
        "machine.device_references",
        &source_references,
        &catalog_references,
    )?;
    let catalog_refs = catalog
        .dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            catalog_machines::MachineDependency::DeviceReference(reference) => Some(reference),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (index, (left, right)) in source.device_refs.iter().zip(catalog_refs).enumerate() {
        let field = format!("machine.device_references[{index}]");
        equal(
            &format!("{field}.location"),
            &(left.location.line, left.location.column),
            &(right.location.line, right.location.column),
        )?;
        equal(
            &format!("{field}.attribute_positions"),
            &left.attribute_positions,
            &right.attribute_positions,
        )?;
    }
    Ok(())
}

fn compare_switches(source: &mame::Machine, catalog: &catalog_machines::Machine) -> VerifyResult {
    equal(
        "machine.switch_count",
        &source.switches.len(),
        &catalog.switches.len(),
    )?;
    for (index, (left, right)) in source.switches.iter().zip(&catalog.switches).enumerate() {
        let prefix = format!("machine.switches[{index}]");
        equal(&format!("{prefix}.kind"), &left.kind, &right.kind)?;
        equal(&format!("{prefix}.name"), &left.name, &right.name)?;
        equal(&format!("{prefix}.tag"), &left.tag, &right.tag)?;
        equal(&format!("{prefix}.mask"), &left.mask, &right.mask)?;
        equal(
            &format!("{prefix}.location"),
            &(left.location.line, left.location.column),
            &(right.location.line, right.location.column),
        )?;
        equal(
            &format!("{prefix}.source_order"),
            &left.source_order,
            &right.source_order,
        )?;
        equal(
            &format!("{prefix}.attribute_positions"),
            &left.attribute_positions,
            &right.attribute_positions,
        )?;
        equal(
            &format!("{prefix}.condition"),
            &left.condition,
            &right.condition,
        )?;
        compare_switch_locations(&prefix, left, right)?;
        compare_switch_values(&prefix, left, right)?;
    }
    Ok(())
}

fn compare_switch_locations(
    prefix: &str,
    left: &mame::MachineSwitch,
    right: &catalog_machines::MachineSwitch,
) -> VerifyResult {
    equal(
        &format!("{prefix}.locations.len"),
        &left.locations.len(),
        &right.locations.len(),
    )?;
    for (child_index, (source_location, catalog_location)) in
        left.locations.iter().zip(&right.locations).enumerate()
    {
        let child = format!("{prefix}.locations[{child_index}]");
        equal(
            &format!("{child}.name"),
            &source_location.name,
            &catalog_location.name,
        )?;
        equal(
            &format!("{child}.number"),
            &source_location.number,
            &catalog_location.number,
        )?;
        equal(
            &format!("{child}.inverted"),
            &source_location.inverted,
            &catalog_location.inverted,
        )?;
        equal(
            &format!("{child}.inverted_specified"),
            &source_location.inverted_specified,
            &catalog_location.inverted_specified,
        )?;
        equal(
            &format!("{child}.source_order"),
            &source_location.source_order,
            &catalog_location.source_order,
        )?;
        equal(
            &format!("{child}.location"),
            &(
                source_location.location.line,
                source_location.location.column,
            ),
            &(
                catalog_location.location.line,
                catalog_location.location.column,
            ),
        )?;
        let expected_kind = match left.kind {
            mame::MachineSwitchKind::DipSwitch => {
                catalog_machines::MachineSwitchLocationKind::DipLocation
            }
            mame::MachineSwitchKind::Configuration => {
                catalog_machines::MachineSwitchLocationKind::ConfigurationLocation
            }
        };
        equal(
            &format!("{child}.kind"),
            &expected_kind,
            &catalog_location.kind,
        )?;
        equal(
            &format!("{child}.attribute_positions"),
            &source_location.attribute_positions,
            &catalog_location.attribute_positions,
        )?;
    }
    Ok(())
}

fn compare_switch_values(
    prefix: &str,
    left: &mame::MachineSwitch,
    right: &catalog_machines::MachineSwitch,
) -> VerifyResult {
    equal(
        &format!("{prefix}.values.len"),
        &left.values.len(),
        &right.values.len(),
    )?;
    for (child_index, (source_value, catalog_value)) in
        left.values.iter().zip(&right.values).enumerate()
    {
        let child = format!("{prefix}.values[{child_index}]");
        equal(
            &format!("{child}.name"),
            &source_value.name,
            &catalog_value.name,
        )?;
        equal(
            &format!("{child}.value"),
            &source_value.value,
            &catalog_value.value,
        )?;
        equal(
            &format!("{child}.default"),
            &source_value.default,
            &catalog_value.default,
        )?;
        equal(
            &format!("{child}.default_specified"),
            &source_value.default_specified,
            &catalog_value.default_specified,
        )?;
        equal(
            &format!("{child}.source_order"),
            &source_value.source_order,
            &catalog_value.source_order,
        )?;
        equal(
            &format!("{child}.location"),
            &(source_value.location.line, source_value.location.column),
            &(catalog_value.location.line, catalog_value.location.column),
        )?;
        equal(
            &format!("{child}.condition"),
            &source_value.condition,
            &catalog_value.condition,
        )?;
        let expected_value_kind = match left.kind {
            mame::MachineSwitchKind::DipSwitch => {
                catalog_machines::MachineSwitchValueKind::DipValue
            }
            mame::MachineSwitchKind::Configuration => {
                catalog_machines::MachineSwitchValueKind::ConfigurationSetting
            }
        };
        equal(
            &format!("{child}.kind"),
            &expected_value_kind,
            &catalog_value.kind,
        )?;
        equal(
            &format!("{child}.attribute_positions"),
            &source_value.attribute_positions,
            &catalog_value.attribute_positions,
        )?;
    }
    Ok(())
}

enum SourceMedia<'a> {
    RomOrDisk(&'a mame::MachineAsset),
    Sample {
        element: &'a mame::MachineSpecificationElement,
        name: &'a str,
        source_order: i64,
        location: (i64, i64),
    },
}

impl SourceMedia<'_> {
    fn name(&self) -> &str {
        match self {
            Self::RomOrDisk(asset) => &asset.name,
            Self::Sample { name, .. } => name,
        }
    }

    const fn source_order(&self) -> i64 {
        match self {
            Self::RomOrDisk(asset) => asset.source_order,
            Self::Sample { source_order, .. } => *source_order,
        }
    }

    const fn location(&self) -> (i64, i64) {
        match self {
            Self::RomOrDisk(asset) => (asset.location.line, asset.location.column),
            Self::Sample { location, .. } => *location,
        }
    }
}

fn compare_assets(
    database: &Database,
    source: &mame::Machine,
    catalog: &catalog_machines::Machine,
) -> VerifyResult {
    let mut source_media = source
        .assets
        .iter()
        .map(SourceMedia::RomOrDisk)
        .collect::<Vec<_>>();
    for specification in &source.specification {
        if let mame::MachineSpecification::Sample(sample) = &specification.value {
            source_media.push(SourceMedia::Sample {
                element: specification,
                name: &sample.name,
                source_order: specification.element_order,
                location: (sample.location.line, sample.location.column),
            });
        }
    }
    source_media.sort_by_key(SourceMedia::source_order);
    equal(
        "machine.asset_count",
        &source_media.len(),
        &catalog.assets.len(),
    )?;
    for (chunk_index, chunk) in catalog.assets.chunks(MAX_MEDIA_IDS_PER_REQUEST).enumerate() {
        let occurrence_ids = chunk
            .iter()
            .map(|asset| asset.occurrence_id)
            .collect::<Vec<_>>();
        let payloads = catalog_files::occurrences_for_ids(database, &occurrence_ids)?
            .into_iter()
            .map(|occurrence| (occurrence.occurrence_id, occurrence))
            .collect::<HashMap<_, _>>();
        equal("machine.media_payload_count", &chunk.len(), &payloads.len())?;

        let first_index = chunk_index * MAX_MEDIA_IDS_PER_REQUEST;
        for (index, (left, right)) in source_media.iter().skip(first_index).zip(chunk).enumerate() {
            let index = first_index + index;
            let prefix = format!("machine.assets[{index}]");
            let expected_kind = match left {
                SourceMedia::RomOrDisk(asset) if asset.role == AssetRole::Rom => {
                    catalog_machines::MachineAssetKind::Rom
                }
                SourceMedia::RomOrDisk(_) => catalog_machines::MachineAssetKind::Disk,
                SourceMedia::Sample { .. } => catalog_machines::MachineAssetKind::Sample,
            };
            equal(&format!("{prefix}.kind"), &expected_kind, &right.kind)?;
            equal(
                &format!("{prefix}.source_order"),
                &left.source_order(),
                &right.source_order,
            )?;
            equal(
                &format!("{prefix}.location"),
                &left.location(),
                &(right.location.line, right.location.column),
            )?;

            let occurrence = payloads
                .get(&right.occurrence_id)
                .ok_or_else(|| format!("{prefix}: native occurrence reference did not resolve"))?;
            equal(
                &format!("{prefix}.occurrence_id"),
                &right.occurrence_id,
                &occurrence.occurrence_id,
            )?;
            let expected_name = Some(left.name());
            equal(
                &format!("{prefix}.occurrence_name"),
                &expected_name,
                &occurrence.provenance.asset_name.as_deref(),
            )?;
            let payload = occurrence
                .mame_file
                .as_ref()
                .ok_or_else(|| format!("{prefix}: query omitted the typed MAME media payload"))?;
            match (left, payload) {
                (
                    SourceMedia::RomOrDisk(asset),
                    MameFilePayload::Rom(_) | MameFilePayload::Disk(_),
                ) => compare_media_payload(&prefix, asset, payload, &occurrence.digests)?,
                (SourceMedia::Sample { .. }, MameFilePayload::Sample(sample)) => {
                    compare_sample_payload(&prefix, left, sample, &occurrence.digests)?;
                }
                _ => return Err(format!("{prefix}: source/query media kind differs").into()),
            }
        }
    }
    Ok(())
}

fn compare_sample_payload(
    prefix: &str,
    source: &SourceMedia<'_>,
    sample: &catalog_files::MameSamplePayload,
    digests: &[catalog_files::OccurrenceDigest],
) -> VerifyResult {
    let SourceMedia::Sample {
        element,
        name,
        source_order,
        ..
    } = source
    else {
        return Err(format!("{prefix}: source/query media kind differs").into());
    };
    let mame::MachineSpecification::Sample(source_sample) = &element.value else {
        return Err(format!("{prefix}: source sample owner changed during comparison").into());
    };
    equal(
        &format!("{prefix}.sample.name"),
        name,
        &sample.name.as_str(),
    )?;
    equal(
        &format!("{prefix}.sample.source_order"),
        source_order,
        &sample.source_order,
    )?;
    equal(
        &format!("{prefix}.sample.location"),
        &(source_sample.location.line, source_sample.location.column),
        &(sample.location.line, sample.location.column),
    )?;
    digest_verify::compare_declared_digests(&format!("{prefix}.digests"), [], digests)?;
    compare_media_positions(
        &format!("{prefix}.sample.attribute_positions"),
        &source_sample.attribute_positions,
        &sample.attribute_positions,
    )?;
    Ok(())
}

fn compare_media_payload(
    prefix: &str,
    source: &mame::MachineAsset,
    catalog: &MameFilePayload,
    digests: &[catalog_files::OccurrenceDigest],
) -> VerifyResult {
    let has_unproven_loading = source.attributes.load_flag.is_some()
        || source.attributes.value.is_some()
        || source.attributes.inverted.is_some()
        || source.attributes.ovha.is_some()
        || source.attributes.no_thread.is_some();
    let evidence_scope =
        if source.dump_status == mame::MameDumpStatus::NoDump || has_unproven_loading {
            "unknown"
        } else {
            source
                .disk_requirement
                .as_ref()
                .map_or("whole_asset", |requirement| {
                    requirement.digest_scope().as_str()
                })
        };
    digest_verify::compare_declared_digests(
        &format!("{prefix}.digests"),
        [
            source
                .crc
                .as_deref()
                .map(|value| (catalog_files::DigestAlgorithm::Crc32, value, evidence_scope)),
            source
                .md5
                .as_deref()
                .map(|value| (catalog_files::DigestAlgorithm::Md5, value, evidence_scope)),
            source
                .sha1
                .as_deref()
                .map(|value| (catalog_files::DigestAlgorithm::Sha1, value, evidence_scope)),
        ]
        .into_iter()
        .flatten(),
        digests,
    )?;
    match (source.role, catalog) {
        (AssetRole::Rom, MameFilePayload::Rom(value)) => {
            compare_rom_payload(prefix, source, value, evidence_scope)?;
        }
        (AssetRole::Disk, MameFilePayload::Disk(value)) => {
            compare_disk_payload(prefix, source, value, evidence_scope)?;
        }
        _ => return Err(format!("{prefix}: source/query media kind differs").into()),
    }
    Ok(())
}

fn compare_rom_payload(
    prefix: &str,
    source: &mame::MachineAsset,
    value: &catalog_files::MameRomPayload,
    evidence_scope: &str,
) -> VerifyResult {
    equal(&format!("{prefix}.name"), &source.name, &value.name)?;
    equal(
        &format!("{prefix}.declarations"),
        &source.declarations,
        &value.declarations,
    )?;
    let expected_scope = if evidence_scope == "unknown" {
        catalog_files::MameRomEvidenceScope::Unknown
    } else {
        catalog_files::MameRomEvidenceScope::WholeFile
    };
    equal(
        &format!("{prefix}.evidence_scope"),
        &expected_scope,
        &value.evidence_scope,
    )?;
    equal(
        &format!("{prefix}.evidence_provenance"),
        &"source_declared",
        &value.evidence_provenance.as_str(),
    )?;
    let source_size = source.size.map(i64::try_from).transpose()?;
    equal(
        &format!("{prefix}.size_projection"),
        &source_size,
        &value.size,
    )?;
    equal(
        &format!("{prefix}.location"),
        &(source.location.line, source.location.column),
        &(value.location.line, value.location.column),
    )?;
    equal(
        &format!("{prefix}.merge_name"),
        &source.merge_name,
        &value.merge_name,
    )?;
    equal(
        &format!("{prefix}.dump_status"),
        &source.dump_status,
        &value.dump_status,
    )?;
    equal(
        &format!("{prefix}.status_specified"),
        &source.attributes.status_specified,
        &value.status_specified,
    )?;
    equal(
        &format!("{prefix}.source_order"),
        &source.source_order,
        &value.source_order,
    )?;
    equal(
        &format!("{prefix}.region"),
        &source.attributes.region,
        &value.region,
    )?;
    equal(
        &format!("{prefix}.bios"),
        &source.attributes.bios,
        &value.bios,
    )?;
    equal(
        &format!("{prefix}.optional"),
        &source.attributes.optional,
        &value.optional,
    )?;
    equal(
        &format!("{prefix}.optional_specified"),
        &source.attributes.optional_specified,
        &value.optional_specified,
    )?;
    compare_rom_compatibility(prefix, source, value)
}

fn compare_rom_compatibility(
    prefix: &str,
    source: &mame::MachineAsset,
    value: &catalog_files::MameRomPayload,
) -> VerifyResult {
    let compatibility = value.compatibility.as_ref();
    equal(
        &format!("{prefix}.compatibility.soundonly"),
        &source.attributes.sound_only.as_ref(),
        &compatibility.and_then(|item| item.sound_only.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.dispose"),
        &source.attributes.dispose.as_ref(),
        &compatibility.and_then(|item| item.dispose.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.loadflag"),
        &source.attributes.load_flag.as_ref(),
        &compatibility.and_then(|item| item.load_flag.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.value"),
        &source.attributes.value.as_ref(),
        &compatibility.and_then(|item| item.value.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.inverted"),
        &source.attributes.inverted.as_ref(),
        &compatibility.and_then(|item| item.inverted.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.ovha"),
        &source.attributes.ovha.as_ref(),
        &compatibility.and_then(|item| item.ovha.as_ref()),
    )?;
    equal(
        &format!("{prefix}.compatibility.nothread"),
        &source.attributes.no_thread.as_ref(),
        &compatibility.and_then(|item| item.no_thread.as_ref()),
    )?;
    let mame::MameAssetAttributePositions::Rom {
        native,
        compatibility: source_compatibility,
    } = &source.attribute_positions
    else {
        return Err(format!("{prefix}: source parser returned non-ROM position owner").into());
    };
    compare_media_positions(
        &format!("{prefix}.native_positions"),
        native,
        &value.attribute_positions,
    )?;
    compare_media_positions(
        &format!("{prefix}.compatibility_positions"),
        source_compatibility,
        &value.compatibility_attribute_positions,
    )?;
    Ok(())
}

fn compare_disk_payload(
    prefix: &str,
    source: &mame::MachineAsset,
    value: &catalog_files::MameDiskPayload,
    evidence_scope: &str,
) -> VerifyResult {
    equal(&format!("{prefix}.name"), &source.name, &value.name)?;
    equal(
        &format!("{prefix}.declarations"),
        &source.declarations,
        &value.declarations,
    )?;
    equal(
        &format!("{prefix}.evidence_scope"),
        &evidence_scope,
        &value.evidence_scope.as_str(),
    )?;
    equal(
        &format!("{prefix}.evidence_provenance"),
        &"source_declared",
        &value.evidence_provenance.as_str(),
    )?;
    equal(
        &format!("{prefix}.location"),
        &(source.location.line, source.location.column),
        &(value.location.line, value.location.column),
    )?;
    equal(
        &format!("{prefix}.merge_name"),
        &source.merge_name,
        &value.merge_name,
    )?;
    equal(
        &format!("{prefix}.dump_status"),
        &source.dump_status,
        &value.dump_status,
    )?;
    equal(
        &format!("{prefix}.status_specified"),
        &source.attributes.status_specified,
        &value.status_specified,
    )?;
    equal(
        &format!("{prefix}.source_order"),
        &source.source_order,
        &value.source_order,
    )?;
    equal(
        &format!("{prefix}.region"),
        &source.attributes.region,
        &value.region,
    )?;
    equal(
        &format!("{prefix}.index"),
        &source.attributes.disk_index,
        &value.disk_index,
    )?;
    equal(
        &format!("{prefix}.writable"),
        &source.attributes.writable,
        &Some(value.writable),
    )?;
    equal(
        &format!("{prefix}.writable_specified"),
        &source.attributes.writable_specified,
        &value.writable_specified,
    )?;
    equal(
        &format!("{prefix}.optional"),
        &source.attributes.optional,
        &value.optional,
    )?;
    equal(
        &format!("{prefix}.optional_specified"),
        &source.attributes.optional_specified,
        &value.optional_specified,
    )?;
    compare_disk_compatibility(prefix, source, value)
}

fn compare_disk_compatibility(
    prefix: &str,
    source: &mame::MachineAsset,
    value: &catalog_files::MameDiskPayload,
) -> VerifyResult {
    let compatibility = value.compatibility.as_ref();
    equal(
        &format!("{prefix}.compatibility.writeable"),
        &source.attributes.writeable,
        &compatibility.map(|item| item.writeable),
    )?;
    let mame::MameAssetAttributePositions::Disk {
        native,
        compatibility: source_compatibility,
    } = &source.attribute_positions
    else {
        return Err(format!("{prefix}: source parser returned non-disk position owner").into());
    };
    compare_media_positions(
        &format!("{prefix}.native_positions"),
        native,
        &value.attribute_positions,
    )?;
    compare_media_positions(
        &format!("{prefix}.compatibility_positions"),
        source_compatibility,
        &value.compatibility_attribute_positions,
    )?;
    Ok(())
}

/// Open an existing initialized catalog through the shared verifier preflight.
pub fn verify_existing_catalog(
    database_path: &Utf8Path,
    snapshot: &SnapshotKey,
    source_bytes: &[u8],
) -> VerifyResult<VerificationReport> {
    let database = open_existing_catalog(database_path)?;
    verify_mame_source(&database, snapshot, source_bytes)
}

#[cfg(test)]
#[path = "mame_native_query_verify/tests.rs"]
mod tests;
