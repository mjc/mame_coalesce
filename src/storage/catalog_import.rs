use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    clrmamepro,
    domain::{
        CatalogSetId, DocumentKey, ImportRunKey, OccurrenceId, ParserInterpretationKey, SnapshotKey,
    },
    logiqx::{DocumentMetadata, Game, LocatedGame, ValidatedLogiqx},
    mame::{self, MameRecord, ValidatedMame},
    mame_softwarelist,
    no_intro_pc_xml::Catalog as NoIntroCatalog,
    storage::{
        cached_sql::{InsertBatch, InsertPhase, cached_sql},
        catalog_content::{
            ContentDigestAssertions, ContentIdentityInput, ContentIdentityResolution,
            record_content_identity_conflict, record_occurrence_digest_assertions,
            resolve_content_identity,
        },
        db::Pool,
        documents::DocumentStore,
        mame_attributes::{self, Family, PositionBatch},
    },
};

mod cmp_native;
mod logiqx_cmp_relationships;
mod logiqx_native;
mod mame_bulk;
mod mame_relationships;
mod mame_specification;
mod mame_switches;
mod no_intro_dat_native;
mod no_intro_database_native;
mod no_intro_pc_native;
mod reported_relationships;
mod root_assets;
mod root_sets;
mod software_native;

#[cfg(test)]
mod bulk_tests;

enum SnapshotPublication {
    Published(SnapshotKey),
    Pending(SnapshotKey),
}

impl SnapshotPublication {
    const fn key(&self) -> &SnapshotKey {
        match self {
            Self::Published(key) | Self::Pending(key) => key,
        }
    }

    fn publish(
        self,
        conn: &mut SqliteConnection,
        request: &CatalogImportRequest,
        document_key: &DocumentKey,
        interpretation: &ParserInterpretationKey,
    ) -> crate::Result<SnapshotKey> {
        match self {
            Self::Published(key) => Ok(key),
            Self::Pending(key) => {
                sql_query(
                    "INSERT INTO snapshot_publications \
                     (catalog_key, document_key, interpretation_key, snapshot_key) \
                     VALUES (?, ?, ?, ?)",
                )
                .bind::<Text, _>(request.catalog_key.as_str())
                .bind::<Text, _>(document_key.to_string())
                .bind::<Text, _>(interpretation.as_str())
                .bind::<Text, _>(key.as_str())
                .execute(conn)?;
                Ok(key)
            }
        }
    }
}

#[derive(QueryableByName)]
struct PublishedSnapshot {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
}

#[derive(QueryableByName)]
struct IdentityOnlySnapshot {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    acquisition_source: Option<String>,
    #[diesel(sql_type = BigInt)]
    is_identity_only: i64,
}

#[derive(QueryableByName)]
struct CatalogIdentityRow {
    #[diesel(sql_type = Text)]
    source_key: String,
    #[diesel(sql_type = Text)]
    display_name: String,
}

struct SnapshotData {
    sets: Vec<SnapshotSet>,
    no_intro_document: Option<no_intro_pc_native::DocumentFacts>,
}

#[derive(Clone)]
struct LogiqxDocumentFacts {
    build: Option<String>,
    debug: String,
    debug_was_present: bool,
    file_name: Option<String>,
    sha1: Option<Vec<u8>>,
    header_name: Option<String>,
    header_description: Option<String>,
    header_version: Option<String>,
    header_date: Option<String>,
    header_author: Option<String>,
    header_email: Option<String>,
    header_homepage: Option<String>,
    header_url: Option<String>,
    header_comment: Option<String>,
    header_category: Option<String>,
}

impl LogiqxDocumentFacts {
    fn from_metadata(data_file: &DocumentMetadata) -> Self {
        let header = data_file.header_opt();
        Self {
            build: data_file.build().map(str::to_owned),
            debug: data_file.debug().unwrap_or("no").to_owned(),
            debug_was_present: data_file.debug().is_some(),
            file_name: data_file.file_name().map(str::to_owned),
            sha1: data_file.sha1().map(<[u8]>::to_vec),
            header_name: header.map(|header| header.name().to_owned()),
            header_description: header.and_then(|header| header.description().cloned()),
            header_version: header.and_then(|header| header.version().cloned()),
            header_date: header.and_then(|header| header.date().cloned()),
            header_author: header.and_then(|header| header.author().cloned()),
            header_email: header.and_then(|header| header.email().cloned()),
            header_homepage: header.and_then(|header| header.homepage().cloned()),
            header_url: header.and_then(|header| header.url().cloned()),
            header_comment: header.and_then(|header| header.comment().cloned()),
            header_category: header.and_then(|header| header.category().cloned()),
        }
    }
}

#[derive(Clone)]
struct LogiqxSetFacts {
    source_file: Option<String>,
    is_bios: String,
    is_bios_was_present: bool,
    board: Option<String>,
    rebuild_to: Option<String>,
    description: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
}

impl LogiqxSetFacts {
    fn from_game(game: &Game) -> Self {
        Self {
            source_file: game.sourcefile_opt().map(str::to_owned),
            is_bios: game.isbios_effective().to_owned(),
            is_bios_was_present: game.isbios_was_explicit(),
            board: game.board_opt().map(str::to_owned),
            rebuild_to: game.rebuildto_opt().map(str::to_owned),
            description: game.description_opt().map(str::to_owned),
            year: game.year_opt().map(str::to_owned),
            manufacturer: game.manufacturer_opt().map(str::to_owned),
        }
    }
}

struct SnapshotSet {
    name: String,
    parent: Option<String>,
    runtime_dependencies: Vec<SnapshotDependency>,
    location: crate::logiqx::RecordLocation,
    assets: Vec<SnapshotAsset>,
    switches: Vec<crate::mame::MachineSwitch>,
    bios_sets: Vec<crate::mame::MachineBiosSet>,
    specification: Vec<crate::mame::MachineSpecificationElement>,
    mame_facts: Option<crate::mame::MachineFacts>,
    no_intro_facts: Option<crate::no_intro_pc_xml::GameFacts>,
    logiqx_facts: Option<LogiqxSetFacts>,
    logiqx_details: Option<logiqx_native::GameDetails>,
    cmp_facts: Option<crate::clrmamepro::SetFacts>,
    machine_dependencies: Vec<SnapshotDependency>,
}

struct SnapshotDependency {
    source_field: String,
    target_name: String,
    reference_tag: Option<String>,
    source_order: Option<i64>,
    location: crate::logiqx::RecordLocation,
    attribute_positions: Vec<mame::AttributePosition<mame::MameDeviceReferenceAttribute>>,
}

#[derive(diesel::QueryableByName)]
struct MameDocumentId {
    #[diesel(sql_type = BigInt)]
    document_id: i64,
}

struct SnapshotAsset {
    name: String,
    role: &'static str,
    size: Option<u64>,
    crc: Option<Vec<u8>>,
    md5: Option<Vec<u8>>,
    sha1: Option<Vec<u8>>,
    evidence_scope: &'static str,
    merge: Option<String>,
    dump_status: Option<String>,
    serial: Option<String>,
    date: Option<String>,
    native: NativeAssetFacts,
    location: crate::logiqx::RecordLocation,
}

/// A media entry has exactly one format-specific source owner.
enum NativeAssetFacts {
    Mame {
        attributes: Box<mame::MameAssetAttributes>,
        declarations: mame::MameAssetDeclarations,
        source_order: i64,
        attribute_positions: mame::MameAssetAttributePositions,
    },
    MameSample {
        source_order: i64,
        attribute_positions: Vec<mame::AttributePosition<mame::MameSampleAttribute>>,
    },
    Logiqx(LogiqxAssetAttributes),
    CmpRom(Box<crate::clrmamepro::AssetFacts>),
    CmpSample(crate::clrmamepro::FieldValue),
    NoIntroPc {
        size_text: Option<String>,
        source_order: usize,
        attribute_positions:
            Vec<crate::no_intro_pc_xml::AttributePosition<crate::no_intro_pc_xml::RomAttribute>>,
    },
}

struct LogiqxAssetAttributes {
    attribute_positions: logiqx_native::MediaAttributePositions,
    size_text: Option<String>,
    crc_text: Option<String>,
    md5_text: Option<String>,
    sha1_text: Option<String>,
    identity_eligibility: SourceIdentityEligibility,
    status_was_present: bool,
    source_order: i64,
}

#[derive(Clone, Copy)]
enum SourceIdentityEligibility {
    Usable,
    UninterpretedDeclaration,
}

impl SnapshotAsset {
    const fn filename_only(
        name: String,
        location: crate::logiqx::RecordLocation,
        native: NativeAssetFacts,
    ) -> Self {
        Self {
            name,
            role: "other",
            size: None,
            crc: None,
            md5: None,
            sha1: None,
            evidence_scope: "unknown",
            merge: None,
            dump_status: None,
            serial: None,
            date: None,
            location,
            native,
        }
    }

    const fn cmp_rom_facts(&self) -> Option<&crate::clrmamepro::AssetFacts> {
        match &self.native {
            NativeAssetFacts::CmpRom(facts) => Some(facts),
            _ => None,
        }
    }

    fn from_logiqx(
        rom: &crate::logiqx::Rom,
        location: crate::logiqx::RecordLocation,
        source_order: i64,
    ) -> crate::Result<Self> {
        let expected = crate::domain::ExpectedEvidence::from_logiqx(rom)?;
        Ok(Self {
            name: rom.name().to_owned(),
            role: "rom",
            size: expected.size,
            crc: expected.crc.map(|value| value.0.to_vec()),
            md5: expected.md5.map(|value| value.0.to_vec()),
            sha1: expected.sha1.map(|value| value.to_vec()),
            evidence_scope: "whole_asset",
            merge: expected.merge,
            dump_status: expected.dump_status,
            serial: expected.serial,
            date: expected.date,
            native: NativeAssetFacts::Logiqx(LogiqxAssetAttributes {
                attribute_positions: logiqx_native::MediaAttributePositions::Rom(
                    rom.attribute_positions().to_vec(),
                ),
                size_text: rom.size_text().map(str::to_owned),
                crc_text: rom.crc_text().map(str::to_owned),
                md5_text: rom.md5_text().map(str::to_owned),
                sha1_text: rom.sha1_text().map(str::to_owned),
                identity_eligibility: if rom.has_uninterpreted_fields() {
                    SourceIdentityEligibility::UninterpretedDeclaration
                } else {
                    SourceIdentityEligibility::Usable
                },
                status_was_present: rom.status_was_explicit(),
                source_order,
            }),
            location,
        })
    }
}

fn interpretation(request: &CatalogImportRequest) -> ParserInterpretationKey {
    ParserInterpretationKey::for_format_with_rules(
        request.format.as_str(),
        request.format.rules_version(),
        &request.scope,
    )
}

impl SnapshotData {
    fn from_no_intro(catalog: NoIntroCatalog) -> Self {
        let sets = catalog
            .entries
            .into_iter()
            .map(|entry| {
                let entry_name = entry.name;
                let no_intro_facts = entry.facts;
                let assets = entry
                    .assets
                    .into_iter()
                    .map(|asset| SnapshotAsset {
                        name: asset.name,
                        role: "rom",
                        size: asset.size,
                        crc: asset.crc,
                        md5: asset.md5,
                        sha1: asset.sha1,
                        evidence_scope: "whole_asset",
                        merge: None,
                        dump_status: None,
                        serial: None,
                        date: None,
                        native: NativeAssetFacts::NoIntroPc {
                            size_text: asset.size_text,
                            source_order: asset.source_order,
                            attribute_positions: asset.attribute_positions,
                        },
                        location: asset.location,
                    })
                    .collect();
                SnapshotSet {
                    name: entry_name,
                    parent: None,
                    runtime_dependencies: Vec::new(),
                    location: entry.location,
                    assets,
                    switches: Vec::new(),
                    bios_sets: Vec::new(),
                    specification: Vec::new(),
                    mame_facts: None,
                    no_intro_facts: Some(no_intro_facts),
                    logiqx_facts: None,
                    logiqx_details: None,
                    cmp_facts: None,
                    machine_dependencies: Vec::new(),
                }
            })
            .collect();
        Self {
            sets,
            no_intro_document: Some(no_intro_pc_native::DocumentFacts {
                location: catalog.document_location,
                header: catalog.header,
            }),
        }
    }
}

fn snapshot_set_from_clrmamepro(mut set: crate::clrmamepro::Set) -> SnapshotSet {
    let samples = std::mem::take(&mut set.native.samples);
    let mut media = set
        .assets
        .into_iter()
        .map(|asset| {
            (
                asset.native.set_order,
                SnapshotAsset {
                    name: asset.name,
                    role: "rom",
                    size: asset.size,
                    crc: asset.crc,
                    md5: asset.md5,
                    sha1: asset.sha1,
                    evidence_scope: "whole_asset",
                    merge: asset.merge,
                    dump_status: asset.status,
                    serial: None,
                    date: None,
                    native: NativeAssetFacts::CmpRom(Box::new(asset.native)),
                    location: asset.location,
                },
            )
        })
        .chain(samples.into_iter().map(|sample| {
            (
                sample.order,
                SnapshotAsset::filename_only(
                    sample.value.clone(),
                    sample.location,
                    NativeAssetFacts::CmpSample(sample),
                ),
            )
        }))
        .collect::<Vec<_>>();
    media.sort_by_key(|(order, _)| *order);
    let assets = media.into_iter().map(|(_, asset)| asset).collect();
    SnapshotSet {
        name: set.name,
        parent: set.parent,
        runtime_dependencies: Vec::new(),
        location: set.location,
        assets,
        switches: Vec::new(),
        bios_sets: Vec::new(),
        specification: Vec::new(),
        mame_facts: None,
        no_intro_facts: None,
        logiqx_facts: None,
        logiqx_details: None,
        cmp_facts: Some(set.native),
        machine_dependencies: Vec::new(),
    }
}

fn logiqx_assets(
    game: &Game,
    rom_locations: &[crate::logiqx::RecordLocation],
) -> crate::Result<Vec<SnapshotAsset>> {
    let mut assets = Vec::with_capacity(game.roms().len());
    for (index, rom) in game.roms().iter().enumerate() {
        let location = *rom_locations.get(index).ok_or_else(|| {
            crate::Error::InvalidPath("Logiqx asset source location is missing".into())
        })?;
        assets.push(SnapshotAsset::from_logiqx(
            rom,
            location,
            logiqx_child_order(game, location)?,
        )?);
    }
    for disk in game.disks() {
        assets.push(SnapshotAsset {
            name: disk.name().to_owned(),
            role: "disk",
            size: None,
            crc: None,
            md5: disk.md5().map(<[u8]>::to_vec),
            sha1: disk.sha1().map(<[u8]>::to_vec),
            evidence_scope: "disk_data",
            merge: disk.merge().map(str::to_owned),
            dump_status: Some(disk.effective_status().to_owned()),
            serial: None,
            date: None,
            native: NativeAssetFacts::Logiqx(LogiqxAssetAttributes {
                attribute_positions: logiqx_native::MediaAttributePositions::Disk(
                    disk.attribute_positions().to_vec(),
                ),
                size_text: None,
                crc_text: None,
                md5_text: disk.md5_text().map(str::to_owned),
                sha1_text: disk.sha1_text().map(str::to_owned),
                identity_eligibility: SourceIdentityEligibility::Usable,
                status_was_present: disk.status_was_explicit(),
                source_order: logiqx_child_order(game, disk.location())?,
            }),
            location: disk.location(),
        });
    }
    for sample in game.samples() {
        assets.push(SnapshotAsset::filename_only(
            sample.name().to_owned(),
            sample.location(),
            NativeAssetFacts::Logiqx(LogiqxAssetAttributes {
                attribute_positions: logiqx_native::MediaAttributePositions::Sample(
                    sample.attribute_positions().to_vec(),
                ),
                size_text: None,
                crc_text: None,
                md5_text: None,
                sha1_text: None,
                identity_eligibility: SourceIdentityEligibility::Usable,
                status_was_present: false,
                source_order: logiqx_child_order(game, sample.location())?,
            }),
        ));
    }
    assets.sort_by_key(|asset| (asset.location.line, asset.location.column));
    Ok(assets)
}

fn logiqx_child_order(game: &Game, location: crate::logiqx::RecordLocation) -> crate::Result<i64> {
    let order = game.child_source_order(location).ok_or_else(|| {
        crate::Error::InvalidPath("Logiqx child source ordinal is missing".into())
    })?;
    checked_order(order, "Logiqx children")
}

fn machine_contents(machine: crate::mame::Machine) -> SnapshotSet {
    let runtime_dependencies: Vec<SnapshotDependency> = machine
        .device_refs
        .into_iter()
        .map(|reference| SnapshotDependency {
            source_field: "device_ref".to_owned(),
            target_name: reference.name,
            reference_tag: Some(reference.tag),
            source_order: Some(reference.source_order),
            location: reference.location,
            attribute_positions: reference.attribute_positions,
        })
        .chain(machine.rom_of.into_iter().map(|name| SnapshotDependency {
            source_field: "romof".to_owned(),
            target_name: name,
            reference_tag: None,
            source_order: None,
            location: machine.location,
            attribute_positions: Vec::new(),
        }))
        .chain(
            machine
                .sample_of
                .into_iter()
                .map(|name| SnapshotDependency {
                    source_field: "sampleof".to_owned(),
                    target_name: name,
                    reference_tag: None,
                    source_order: None,
                    location: machine.location,
                    attribute_positions: Vec::new(),
                }),
        )
        .collect();
    let mut assets = machine_assets(machine.assets);
    let mut specification = Vec::with_capacity(machine.specification.len());
    for element in machine.specification {
        if let mame::MachineSpecification::Sample(sample) = element.value {
            assets.push((
                element.element_order,
                SnapshotAsset::filename_only(
                    sample.name,
                    sample.location,
                    NativeAssetFacts::MameSample {
                        source_order: element.element_order,
                        attribute_positions: sample.attribute_positions,
                    },
                ),
            ));
        } else {
            specification.push(element);
        }
    }
    assets.sort_by_key(|(order, _)| *order);
    let assets = assets.into_iter().map(|(_, asset)| asset).collect();
    let switches = machine.switches;
    let bios_sets = machine.bios_sets;
    let mame_facts = machine.facts;
    let machine_dependencies = runtime_dependencies;
    SnapshotSet {
        name: machine.name,
        parent: machine.parent,
        runtime_dependencies: Vec::new(),
        location: machine.location,
        assets,
        switches,
        bios_sets,
        specification,
        machine_dependencies,
        mame_facts: Some(mame_facts),
        no_intro_facts: None,
        logiqx_facts: None,
        logiqx_details: None,
        cmp_facts: None,
    }
}

fn machine_assets(assets: Vec<crate::mame::MachineAsset>) -> Vec<(i64, SnapshotAsset)> {
    assets
        .into_iter()
        .map(|asset| {
            let evidence_scope = if asset.dump_status == mame::MameDumpStatus::NoDump
                || asset.attributes.has_unproven_loading()
            {
                "unknown"
            } else {
                asset
                    .disk_requirement
                    .as_ref()
                    .map_or("whole_asset", |requirement| {
                        requirement.digest_scope().as_str()
                    })
            };
            let sha1 = asset
                .disk_requirement
                .as_ref()
                .and_then(crate::disk::DiskRequirement::expected_sha1)
                .map(|digest| digest.as_bytes().to_vec())
                .or(asset.sha1);
            let merge = asset
                .disk_requirement
                .as_ref()
                .and_then(crate::disk::DiskRequirement::parent)
                .map(|name| name.as_str().to_owned());
            (
                asset.source_order,
                SnapshotAsset {
                    name: asset.name,
                    role: match asset.role {
                        crate::domain::AssetRole::Rom => "rom",
                        crate::domain::AssetRole::Disk => "disk",
                        crate::domain::AssetRole::Other => "other",
                    },
                    size: asset.size,
                    crc: asset.crc,
                    md5: asset.md5,
                    sha1,
                    evidence_scope,
                    merge: asset.merge_name.or(merge),
                    dump_status: Some(asset.dump_status.as_str().to_owned()),
                    serial: None,
                    date: None,
                    native: NativeAssetFacts::Mame {
                        attributes: Box::new(asset.attributes),
                        declarations: asset.declarations,
                        source_order: asset.source_order,
                        attribute_positions: asset.attribute_positions,
                    },
                    location: asset.location,
                },
            )
        })
        .collect()
}

fn retain_and_import_clrmamepro(
    pool: &Pool,
    request: &CatalogImportRequest,
) -> crate::Result<CatalogImportReport> {
    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone())?;
    let retained =
        documents.retain_path_clrmamepro(request.source_key.clone(), &request.document_path)?;
    let bytes = documents.load(&retained.document_key)?;
    import_clrmamepro(
        pool,
        request,
        retained.document_key,
        &retained.acquisition_key.to_string(),
        &bytes,
    )
}

pub fn import(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<CatalogImportReport> {
    if request.format == CatalogDocumentFormat::ClrMamePro {
        return retain_and_import_clrmamepro(pool, request);
    }

    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone())?;
    let retained = match request.format {
        CatalogDocumentFormat::Logiqx(_) => {
            documents.retain_path(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameListXml => {
            documents.retain_path_mame(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameSoftwareListXml => documents
            .retain_path_mame_softwarelist(request.source_key.clone(), &request.document_path)?,
        CatalogDocumentFormat::ClrMamePro => unreachable!("ClrMamePro import is dispatched above"),
        CatalogDocumentFormat::NoIntroPcXml => documents
            .retain_path_no_intro_pc_xml(request.source_key.clone(), &request.document_path)?,
        CatalogDocumentFormat::NoIntroDat(_) => documents
            .retain_path_no_intro_dat_xml(request.source_key.clone(), &request.document_path)?,
        CatalogDocumentFormat::NoIntroDatabase(_) => documents.retain_path_no_intro_database_xml(
            request.source_key.clone(),
            &request.document_path,
        )?,
    };
    let bytes = documents.load(&retained.document_key)?;
    let parsed = match request.format {
        CatalogDocumentFormat::Logiqx(mode) => {
            return import_logiqx(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &bytes,
                mode,
            );
        }
        CatalogDocumentFormat::MameListXml => {
            return import_mame(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &bytes,
            );
        }
        CatalogDocumentFormat::MameSoftwareListXml => {
            return import_mame_softwarelist(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &bytes,
            );
        }
        CatalogDocumentFormat::ClrMamePro => unreachable!("ClrMamePro import is dispatched above"),
        CatalogDocumentFormat::NoIntroPcXml => {
            NoIntroCatalog::parse(&bytes).map(SnapshotData::from_no_intro)
        }
        CatalogDocumentFormat::NoIntroDat(mode) => {
            return import_no_intro_dat(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &bytes,
                mode,
            );
        }
        CatalogDocumentFormat::NoIntroDatabase(mode) => {
            return import_no_intro_database(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &bytes,
                mode,
            );
        }
    };
    let snapshot_data = match parsed {
        Ok(parsed) => parsed,
        Err(error) => {
            return record_failed_import(
                pool,
                request,
                retained.document_key,
                &retained.acquisition_key.to_string(),
                &error,
            );
        }
    };
    publish_snapshot(
        pool,
        request,
        retained.document_key,
        &retained.acquisition_key.to_string(),
        &snapshot_data,
    )
}

enum StreamingImportError {
    Parse(crate::Error),
    Storage(crate::Error),
}

impl From<crate::Error> for StreamingImportError {
    fn from(error: crate::Error) -> Self {
        Self::Parse(error)
    }
}

impl From<diesel::result::Error> for StreamingImportError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Storage(error.into())
    }
}

struct StreamingImport<'a> {
    conn: &'a mut SqliteConnection,
    publication: SnapshotPublication,
    run_key: ImportRunKey,
    machines: mame_bulk::MachineBatch,
}

struct ClrMameProImport<'a> {
    import: StreamingImport<'a>,
    header: Option<crate::clrmamepro::Header>,
    next_comment_order: usize,
}

impl crate::clrmamepro::EventConsumer for ClrMameProImport<'_> {
    type Error = StreamingImportError;

    fn consume(&mut self, event: crate::clrmamepro::Event) -> std::result::Result<(), Self::Error> {
        match event {
            crate::clrmamepro::Event::Comment(comment) => {
                let comment_order = self.next_comment_order;
                self.next_comment_order =
                    self.next_comment_order.checked_add(1).ok_or_else(|| {
                        StreamingImportError::Storage(crate::Error::InvalidPath(
                            "too many ClrMamePro comments".into(),
                        ))
                    })?;
                if let SnapshotPublication::Pending(snapshot) = &self.import.publication {
                    cmp_native::insert_comment(self.import.conn, snapshot, comment_order, &comment)
                        .map_err(StreamingImportError::Storage)?;
                }
            }
            crate::clrmamepro::Event::Header(header, _extensions) => {
                self.header = Some(header);
            }
            crate::clrmamepro::Event::Set(set) => {
                if let SnapshotPublication::Pending(snapshot) = &self.import.publication {
                    let set = snapshot_set_from_clrmamepro(set);
                    insert_snapshot_set(self.import.conn, snapshot, &set)
                        .map_err(StreamingImportError::Storage)?;
                }
            }
            crate::clrmamepro::Event::Extension(extension) => drop(extension),
        }
        Ok(())
    }
}

struct SoftwareListImport<'a> {
    import: StreamingImport<'a>,
    next_list_order: usize,
    current_list: Option<CurrentSoftwareList>,
}

struct CurrentSoftwareList {
    header: mame_softwarelist::SoftwareListHeader,
    namespace: Option<i64>,
    next_item_order: usize,
}

fn start_streaming_import<'a>(
    conn: &'a mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    acquisition_key: &str,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<StreamingImport<'a>> {
    let publication =
        prepare_snapshot(conn, request, document_key, acquisition_key, interpretation)?;
    let run_key = ImportRunKey::fresh();
    insert_import_run(
        conn,
        request,
        document_key,
        interpretation,
        acquisition_key,
        &run_key,
        CompletedRun::Succeeded(publication.key()),
    )?;
    Ok(StreamingImport {
        conn,
        publication,
        run_key,
        machines: mame_bulk::MachineBatch::default(),
    })
}

fn logiqx_contents(record: &LocatedGame) -> crate::Result<SnapshotSet> {
    let game = &record.game;
    let location = record.location;
    if game.device_refs().count() != record.device_ref_locations.len() {
        return Err(crate::Error::InvalidPath(
            "Logiqx device-reference source locations do not match parsed references".into(),
        ));
    }
    let mut runtime_dependencies = game
        .romof_opt()
        .map(|name| SnapshotDependency {
            source_field: "romof".to_owned(),
            target_name: name.to_owned(),
            reference_tag: None,
            source_order: None,
            location,
            attribute_positions: Vec::new(),
        })
        .into_iter()
        .chain(game.sampleof_opt().map(|name| SnapshotDependency {
            source_field: "sampleof".to_owned(),
            target_name: name.to_owned(),
            reference_tag: None,
            source_order: None,
            location,
            attribute_positions: Vec::new(),
        }))
        .collect::<Vec<_>>();
    for (name, location) in game.device_refs().zip(&record.device_ref_locations) {
        runtime_dependencies.push(SnapshotDependency {
            source_field: "device_ref".to_owned(),
            target_name: name.to_owned(),
            reference_tag: None,
            source_order: Some(logiqx_child_order(game, *location)?),
            location: *location,
            attribute_positions: Vec::new(),
        });
    }
    Ok(SnapshotSet {
        name: game.name().into(),
        parent: game.cloneof().map(str::to_owned),
        runtime_dependencies,
        location,
        assets: logiqx_assets(game, &record.rom_locations)?,
        switches: Vec::new(),
        bios_sets: Vec::new(),
        specification: Vec::new(),
        mame_facts: None,
        no_intro_facts: None,
        logiqx_facts: Some(LogiqxSetFacts::from_game(game)),
        logiqx_details: Some(logiqx_native::GameDetails::from_game(game)?),
        cmp_facts: None,
        machine_dependencies: Vec::new(),
    })
}

fn import_logiqx(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
    mode: crate::logiqx::LogiqxMode,
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let validated = crate::logiqx::read_with_mode::<_, StreamingImportError>(
            bytes,
            mode,
            |_| {
                start_streaming_import(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                )
                .map_err(StreamingImportError::Storage)
            },
            |sink, record| {
                // Reimports still consume and validate every record and late metadata,
                // but cannot recreate any of the immutable published owners.
                if let SnapshotPublication::Pending(key) = &sink.publication {
                    let set = logiqx_contents(&record)?;
                    insert_snapshot_set(sink.conn, key, &set)
                        .map_err(StreamingImportError::Storage)?;
                }
                Ok(())
            },
        )?;
        publish_logiqx_import(validated, request, &document_key, &interpretation)
            .map_err(StreamingImportError::Storage)
    });
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn publish_logiqx_import(
    validated: ValidatedLogiqx<StreamingImport<'_>>,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<CatalogImportReport> {
    let (metadata, sink) = validated.into_parts();
    if let SnapshotPublication::Pending(key) = &sink.publication {
        insert_logiqx_document_facts(
            sink.conn,
            key,
            &LogiqxDocumentFacts::from_metadata(&metadata),
        )?;
        logiqx_native::DocumentDetails::from_metadata(&metadata)?.insert(sink.conn, key)?;
    }
    finish_streaming_import(sink, request, document_key, interpretation)
}

impl StreamingImport<'_> {
    fn consume(&mut self, record: MameRecord) -> crate::Result<()> {
        match record {
            MameRecord::Machine(machine) => {
                let set = machine_contents(*machine);
                if let SnapshotPublication::Pending(key) = &self.publication {
                    self.machines.push(self.conn, key, set)?;
                }
                Ok(())
            }
            MameRecord::Extension(extension) => {
                // Vendor-only data remains recoverable from the external source document.
                drop(extension);
                Ok(())
            }
        }
    }
}

impl SoftwareListImport<'_> {
    fn start_list(&mut self, header: mame_softwarelist::SoftwareListHeader) -> crate::Result<()> {
        if self.current_list.is_some() {
            return Err(crate::Error::InvalidPath(
                "software-list reader started a list before ending the previous list".into(),
            ));
        }
        let namespace = if let SnapshotPublication::Pending(key) = &self.import.publication {
            Some(software_native::insert_list_group(
                self.import.conn,
                key,
                self.next_list_order,
            )?)
        } else {
            None
        };
        self.next_list_order = checked_order(self.next_list_order, "software lists")?
            .checked_add(1)
            .and_then(|order| usize::try_from(order).ok())
            .ok_or_else(|| crate::Error::InvalidPath("too many software lists".into()))?;
        self.current_list = Some(CurrentSoftwareList {
            header,
            namespace,
            next_item_order: 0,
        });
        Ok(())
    }

    fn insert_item(&mut self, item: &mame_softwarelist::SoftwareItem) -> crate::Result<()> {
        let current_list = self.current_list.as_mut().ok_or_else(|| {
            crate::Error::InvalidPath("software-list reader emitted an item outside a list".into())
        })?;
        let order = current_list.next_item_order;
        current_list.next_item_order = checked_order(order, "software items")?
            .checked_add(1)
            .and_then(|order| usize::try_from(order).ok())
            .ok_or_else(|| crate::Error::InvalidPath("too many software items".into()))?;
        if let SnapshotPublication::Pending(key) = &self.import.publication {
            let namespace = current_list.namespace.ok_or_else(|| {
                crate::Error::InvalidPath(
                    "pending software-list item has no allocated list group".into(),
                )
            })?;
            software_native::insert_item(self.import.conn, key, namespace, item, order)?;
        }
        Ok(())
    }

    fn finish_list(
        &mut self,
        metadata: &mame_softwarelist::SoftwareListMetadata,
    ) -> crate::Result<()> {
        let current_list = self.current_list.take().ok_or_else(|| {
            crate::Error::InvalidPath(
                "software-list reader ended a list that was not started".into(),
            )
        })?;
        if let SnapshotPublication::Pending(_) = &self.import.publication {
            let namespace = current_list.namespace.ok_or_else(|| {
                crate::Error::InvalidPath(
                    "pending software-list detail has no allocated list group".into(),
                )
            })?;
            software_native::insert_list_details(
                self.import.conn,
                namespace,
                &current_list.header,
                metadata,
            )?;
        }
        Ok(())
    }
}

fn import_mame(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let validated = mame::read_with::<_, StreamingImportError>(
            bytes,
            |header| {
                let sink = start_streaming_import(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                )
                .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &sink.publication {
                    insert_mame_document_facts(sink.conn, key, &header)
                        .map_err(StreamingImportError::Storage)?;
                }
                drop(header.extensions);
                Ok(sink)
            },
            |sink, record| sink.consume(record).map_err(StreamingImportError::Storage),
        )?;
        publish_mame_import(validated, request, &document_key, &interpretation)
            .map_err(StreamingImportError::Storage)
    });
    // Release the connection before recording a failure in a separate transaction.
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn import_clrmamepro(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let import = start_streaming_import(
            conn,
            request,
            &document_key,
            acquisition_key,
            &interpretation,
        )
        .map_err(StreamingImportError::Storage)?;
        let mut cmp = ClrMameProImport {
            import,
            header: None,
            next_comment_order: 0,
        };
        let eof = clrmamepro::read_with(bytes, &mut cmp)?;
        if cmp.next_comment_order != eof.comment_count()
            || cmp.header.is_some() != eof.header_present()
        {
            return Err(StreamingImportError::Storage(crate::Error::InvalidPath(
                "ClrMamePro reader seal does not match streamed document facts".into(),
            )));
        }

        let ClrMameProImport {
            import,
            header,
            next_comment_order,
        } = cmp;
        if let SnapshotPublication::Pending(snapshot) = &import.publication {
            if let Some(header) = &header {
                cmp_native::insert_header_facts(import.conn, snapshot, header)
                    .map_err(StreamingImportError::Storage)?;
            }
            cmp_native::insert_document_facts(
                import.conn,
                snapshot,
                eof.header_present(),
                next_comment_order,
            )
            .map_err(StreamingImportError::Storage)?;
        }
        finish_streaming_import(import, request, &document_key, &interpretation)
            .map_err(StreamingImportError::Storage)
    });
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn import_mame_softwarelist(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let validated = mame_softwarelist::read_with::<_, StreamingImportError>(
            bytes,
            |header| {
                let import = start_streaming_import(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                )
                .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &import.publication {
                    software_native::insert_document(import.conn, key, &header)
                        .map_err(StreamingImportError::Storage)?;
                }
                Ok(SoftwareListImport {
                    import,
                    next_list_order: 0,
                    current_list: None,
                })
            },
            |sink, header| {
                sink.start_list(header)
                    .map_err(StreamingImportError::Storage)
            },
            |sink, item| {
                sink.insert_item(&item)
                    .map_err(StreamingImportError::Storage)
            },
            |sink, metadata| {
                sink.finish_list(&metadata)
                    .map_err(StreamingImportError::Storage)
            },
            |_sink, extension| {
                // As before, vendor-only software-list data remains in the external original.
                drop(extension);
                Ok(())
            },
        )?;
        finish_streaming_import(
            validated.into_inner().import,
            request,
            &document_key,
            &interpretation,
        )
        .map_err(StreamingImportError::Storage)
    });
    // Parser failures are recorded only after rolling back and releasing the write connection.
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn publish_mame_import(
    validated: ValidatedMame<StreamingImport<'_>>,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<CatalogImportReport> {
    finish_streaming_import(
        validated.into_inner(),
        request,
        document_key,
        interpretation,
    )
}

// Called only after the catalog transaction's connection has been released.
// Parse diagnostics survive in their own transaction; storage failures stay errors.
fn finish_streaming_result(
    result: Result<CatalogImportReport, StreamingImportError>,
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
) -> crate::Result<CatalogImportReport> {
    match result {
        Ok(report) => Ok(report),
        Err(StreamingImportError::Storage(error)) => Err(error),
        Err(StreamingImportError::Parse(error)) => {
            record_failed_import(pool, request, document_key, acquisition_key, &error)
        }
    }
}

fn finish_streaming_import(
    mut sink: StreamingImport<'_>,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<CatalogImportReport> {
    if let SnapshotPublication::Pending(key) = &sink.publication {
        sink.machines.flush(sink.conn, key)?;
    }
    let snapshot_key =
        sink.publication
            .publish(sink.conn, request, document_key, interpretation)?;
    Ok(CatalogImportReport {
        snapshot_key: Some(snapshot_key),
        run_key: sink.run_key,
        status: CatalogImportStatus::Succeeded,
        diagnostic_count: 0,
    })
}

struct NoIntroDatImport<'a> {
    import: StreamingImport<'a>,
    counts: no_intro_dat_native::ImportCounts,
    digest_scope: no_intro_dat_native::NoIntroDatEvidenceScope,
}

fn import_no_intro_dat(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
    mode: crate::NoIntroDatMode,
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let validated = crate::no_intro_dat_xml::read_with::<_, StreamingImportError>(
            bytes,
            mode,
            |document| {
                let import = start_streaming_import(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                )
                .map_err(StreamingImportError::Storage)?;
                let counts = no_intro_dat_native::ImportCounts::from_document(&document);
                let digest_scope =
                    no_intro_dat_native::NoIntroDatEvidenceScope::from_document(&document);
                if let SnapshotPublication::Pending(key) = &import.publication {
                    no_intro_dat_native::insert_document(import.conn, key, &document)
                        .map_err(StreamingImportError::Storage)?;
                }
                Ok(NoIntroDatImport {
                    import,
                    counts,
                    digest_scope,
                })
            },
            |sink, game| {
                sink.counts
                    .include_game(&game)
                    .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &sink.import.publication {
                    no_intro_dat_native::insert_game(
                        sink.import.conn,
                        key,
                        &game,
                        sink.digest_scope,
                    )
                    .map_err(StreamingImportError::Storage)?;
                }
                Ok(())
            },
        )?;
        publish_no_intro_dat_import(validated, request, &document_key, &interpretation)
            .map_err(StreamingImportError::Storage)
    });
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn publish_no_intro_dat_import(
    validated: crate::no_intro_dat_xml::ValidatedNoIntroDat<NoIntroDatImport<'_>>,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<CatalogImportReport> {
    let sink = validated.into_inner();
    if let SnapshotPublication::Pending(key) = &sink.import.publication {
        no_intro_dat_native::seal_document(sink.import.conn, key, &sink.counts)?;
    }
    finish_streaming_import(sink.import, request, document_key, interpretation)
}

struct NoIntroDatabaseImport<'a> {
    import: StreamingImport<'a>,
    counts: no_intro_database_native::ImportCounts,
    diagnostic_count: usize,
    extent: crate::no_intro_db_xml::XmlSourceExtent,
}

fn import_no_intro_database(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    bytes: &[u8],
    mode: crate::no_intro_db_xml::NoIntroDatabaseMode,
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let mut conn = pool.get()?;
    let result = conn.immediate_transaction::<_, StreamingImportError, _>(|conn| {
        ensure_identities(conn, request, &interpretation).map_err(StreamingImportError::Storage)?;
        let validated = crate::no_intro_db_xml::read_with_recovery::<_, StreamingImportError>(
            bytes,
            mode,
            |document, warnings| {
                let import = start_streaming_import(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                )
                .map_err(StreamingImportError::Storage)?;
                let counts = no_intro_database_native::ImportCounts::from_document(&document)
                    .map_err(StreamingImportError::Storage)?;
                let mut diagnostic_count = 0;
                no_intro_database_native::WarningWriter {
                    conn: import.conn,
                    publication: &import.publication,
                    run: &import.run_key,
                    document: &document_key,
                    count: &mut diagnostic_count,
                }
                .document(&document, warnings)
                .map_err(StreamingImportError::Storage)?;
                Ok(NoIntroDatabaseImport {
                    import,
                    counts,
                    diagnostic_count,
                    extent: document.extent,
                })
            },
            |sink, game, warnings| {
                sink.counts
                    .include_game(&game)
                    .map_err(StreamingImportError::Storage)?;
                no_intro_database_native::WarningWriter {
                    conn: sink.import.conn,
                    publication: &sink.import.publication,
                    run: &sink.import.run_key,
                    document: &document_key,
                    count: &mut sink.diagnostic_count,
                }
                .game(&game, sink.extent, warnings)
                .map_err(StreamingImportError::Storage)?;
                Ok(())
            },
            |sink, warnings| {
                let owner = super::import_diagnostics::NoIntroDiagnosticOwner::ExportDocument {
                    snapshot: sink.import.publication.key().clone(),
                    extent: sink.extent,
                };
                no_intro_database_native::WarningWriter {
                    conn: sink.import.conn,
                    publication: &sink.import.publication,
                    run: &sink.import.run_key,
                    document: &document_key,
                    count: &mut sink.diagnostic_count,
                }
                .emit_before(warnings, sink.extent.end(), &owner)
                .map_err(StreamingImportError::Storage)?;
                if warnings.peek().is_some() {
                    return Err(StreamingImportError::Storage(crate::Error::DatabaseSchema(
                        "unassociated warning after document end".into(),
                    )));
                }
                Ok(())
            },
        )?;
        let sink = validated.into_inner();
        let diagnostic_count = sink.diagnostic_count;
        if let SnapshotPublication::Pending(key) = &sink.import.publication {
            no_intro_database_native::seal_document(sink.import.conn, key, &sink.counts)
                .map_err(StreamingImportError::Storage)?;
        }
        let mut report =
            finish_streaming_import(sink.import, request, &document_key, &interpretation)
                .map_err(StreamingImportError::Storage)?;
        report.diagnostic_count = diagnostic_count;
        Ok(report)
    });
    drop(conn);
    finish_streaming_result(result, pool, request, document_key, acquisition_key)
}

fn ensure_source(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<()> {
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        super::publishing_sources::ensure(conn, &request.source_key, &request.source_display_name)
    })
}

fn record_failed_import(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    error: &crate::Error,
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let run_key = ImportRunKey::fresh();
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        ensure_identities(conn, request, &interpretation)?;
        insert_import_run(
            conn,
            request,
            &document_key,
            &interpretation,
            acquisition_key,
            &run_key,
            CompletedRun::Failed,
        )?;
        super::import_diagnostics::insert_parse_error(conn, &run_key, &document_key, error)?;
        Ok(CatalogImportReport {
            snapshot_key: None,
            run_key,
            status: CatalogImportStatus::Failed,
            diagnostic_count: 1,
        })
    })
}

fn publish_snapshot(
    pool: &Pool,
    request: &CatalogImportRequest,
    document_key: DocumentKey,
    acquisition_key: &str,
    snapshot_data: &SnapshotData,
) -> crate::Result<CatalogImportReport> {
    let interpretation = interpretation(request);
    let run_key = ImportRunKey::fresh();
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        ensure_identities(conn, request, &interpretation)?;
        let snapshot_key = ensure_snapshot_publication(
            conn,
            request,
            &document_key,
            acquisition_key,
            &interpretation,
            snapshot_data,
        )?;

        insert_import_run(
            conn,
            request,
            &document_key,
            &interpretation,
            acquisition_key,
            &run_key,
            CompletedRun::Succeeded(&snapshot_key),
        )?;
        Ok(CatalogImportReport {
            snapshot_key: Some(snapshot_key.clone()),
            run_key,
            status: CatalogImportStatus::Succeeded,
            diagnostic_count: 0,
        })
    })
}

fn ensure_snapshot_publication(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    acquisition_key: &str,
    interpretation: &ParserInterpretationKey,
    snapshot_data: &SnapshotData,
) -> crate::Result<SnapshotKey> {
    let publication =
        prepare_snapshot(conn, request, document_key, acquisition_key, interpretation)?;
    if let SnapshotPublication::Pending(key) = &publication {
        insert_snapshot_contents(conn, key, snapshot_data)?;
    }
    publication.publish(conn, request, document_key, interpretation)
}

fn prepare_snapshot(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    acquisition_key: &str,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<SnapshotPublication> {
    let published = sql_query(
        "SELECT snapshot_key FROM snapshot_publications \
         WHERE catalog_key = ? AND document_key = ? AND interpretation_key = ? \
         LIMIT 1",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(document_key.to_string())
    .bind::<Text, _>(interpretation.as_str())
    .get_result::<PublishedSnapshot>(conn)
    .optional()?;
    if let Some(published) = published {
        return Ok(SnapshotPublication::Published(SnapshotKey::from_persisted(
            published.snapshot_key,
        )));
    }

    let coverage_id = super::catalog_coverage::ensure(conn, &request.scope)?;
    let identity_rows = sql_query(
        "SELECT snapshot.snapshot_key, snapshot.coverage_id, \
                acquisition.source_key AS acquisition_source, \
                NOT EXISTS (SELECT 1 FROM catalog_set_groups WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM mame_document_facts WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM logiqx_document_facts WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM cmp_documents WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM cmp_header_facts WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM no_intro_pc_documents WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM software_documents WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM no_intro_dat_documents WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=snapshot.snapshot_key) \
                AND NOT EXISTS (SELECT 1 FROM catalog_relationships WHERE snapshot_key=snapshot.snapshot_key) \
                AS is_identity_only \
         FROM catalog_snapshots AS snapshot \
         LEFT JOIN acquisitions AS acquisition \
           ON acquisition.acquisition_key = snapshot.acquisition_key \
         WHERE snapshot.catalog_key = ? AND snapshot.document_key = ? \
           AND snapshot.interpretation_key = ? \
         ORDER BY snapshot.snapshot_key",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(document_key.to_string())
    .bind::<Text, _>(interpretation.as_str())
    .load::<IdentityOnlySnapshot>(conn)?;
    let matching_identity = identity_rows.iter().find(|row| {
        row.is_identity_only == 1
            && row.coverage_id == coverage_id.as_i64()
            && row.acquisition_source.as_deref() == Some(request.source_key.as_str())
    });
    let snapshot_key = if let Some(existing) = matching_identity {
        SnapshotKey::from_persisted(existing.snapshot_key.clone())
    } else {
        let snapshot_key = if identity_rows.is_empty() {
            SnapshotKey::new(&request.catalog_key, document_key, interpretation)
        } else {
            SnapshotKey::new_publication(&request.catalog_key, document_key, interpretation)
        };
        sql_query(
            "INSERT INTO catalog_snapshots \
             (snapshot_key, catalog_key, document_key, interpretation_key, acquisition_key, \
              coverage_id) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(document_key.to_string())
        .bind::<Text, _>(interpretation.as_str())
        .bind::<Nullable<Text>, _>(Some(acquisition_key.to_owned()))
        .bind::<BigInt, _>(coverage_id.as_i64())
        .execute(conn)?;
        snapshot_key
    };
    Ok(SnapshotPublication::Pending(snapshot_key))
}

fn ensure_identities(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<()> {
    super::publishing_sources::ensure(conn, &request.source_key, &request.source_display_name)?;
    let catalog = sql_query("SELECT source_key, display_name FROM catalogs WHERE catalog_key = ?")
        .bind::<Text, _>(request.catalog_key.as_str())
        .get_result::<CatalogIdentityRow>(conn)
        .optional()?;
    match catalog {
        Some(catalog)
            if catalog.source_key != request.source_key.as_str()
                || catalog.display_name != request.catalog_display_name =>
        {
            return Err(crate::Error::CatalogIdentityConflict(
                request.catalog_key.as_str().to_owned(),
            ));
        }
        Some(_) => {}
        None => {
            sql_query(
                "INSERT INTO catalogs (catalog_key, source_key, display_name) VALUES (?, ?, ?)",
            )
            .bind::<Text, _>(request.catalog_key.as_str())
            .bind::<Text, _>(request.source_key.as_str())
            .bind::<Text, _>(&request.catalog_display_name)
            .execute(conn)?;
        }
    }

    sql_query(
        "INSERT INTO parser_interpretations \
         (interpretation_key, format, parser_name, parser_version, rules_version) \
         VALUES (?, ?, 'mame_coalesce', ?, ?) \
         ON CONFLICT(interpretation_key) DO NOTHING",
    )
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Text, _>(request.format.as_str())
    .bind::<Nullable<Text>, _>(Some(env!("CARGO_PKG_VERSION").to_owned()))
    .bind::<Text, _>(request.format.rules_version())
    .execute(conn)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum CompletedRun<'a> {
    Succeeded(&'a SnapshotKey),
    Failed,
}

fn insert_import_run(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
    acquisition_key: &str,
    run_key: &ImportRunKey,
    outcome: CompletedRun<'_>,
) -> diesel::QueryResult<usize> {
    let (snapshot_key, status) = match outcome {
        CompletedRun::Succeeded(snapshot) => (Some(snapshot.as_str()), "succeeded"),
        CompletedRun::Failed => (None, "failed"),
    };
    sql_query(
        "INSERT INTO import_runs \
         (run_key, catalog_key, document_key, interpretation_key, acquisition_key, snapshot_key, \
          status, started_at, finished_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind::<Text, _>(run_key.to_string())
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(document_key.to_string())
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Nullable<Text>, _>(Some(acquisition_key.to_owned()))
    .bind::<Nullable<Text>, _>(snapshot_key)
    .bind::<Text, _>(status)
    .execute(conn)
}

fn insert_snapshot_contents(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    snapshot_data: &SnapshotData,
) -> crate::Result<()> {
    if let Some(document) = &snapshot_data.no_intro_document {
        no_intro_pc_native::insert_document(conn, snapshot_key, document)?;
    }
    for set in &snapshot_data.sets {
        insert_snapshot_set(conn, snapshot_key, set)?;
    }

    Ok(())
}

fn insert_snapshot_set(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<CatalogSetId> {
    let owner = root_sets::insert(conn, snapshot_key, set)?;
    let set_id = owner.as_i64();

    if let Some(facts) = &set.mame_facts {
        insert_mame_machine_facts(conn, set_id, facts)?;
        mame_relationships::insert(conn, snapshot_key, owner, set)?;
        let mut reference_order = 0_i64;
        for dependency in set
            .machine_dependencies
            .iter()
            .filter(|item| item.source_field == "device_ref")
        {
            mame_attributes::insert(
                conn,
                Family::DeviceReference,
                &[set_id, reference_order],
                &dependency.attribute_positions,
                mame::MameDeviceReferenceAttribute::code,
            )?;
            reference_order = reference_order.checked_add(1).ok_or_else(|| {
                crate::Error::InvalidPath("too many MAME device references".into())
            })?;
        }
        mame_specification::insert(conn, set_id, set)?;
    }
    if let Some(facts) = &set.no_intro_facts {
        insert_no_intro_game_facts(conn, snapshot_key, owner, facts)?;
    }
    if let Some(facts) = &set.logiqx_facts {
        insert_logiqx_set_facts(conn, set_id, facts)?;
        logiqx_cmp_relationships::insert_logiqx(conn, snapshot_key, owner, set)?;
    }
    if let Some(details) = &set.logiqx_details {
        details.insert(conn, owner)?;
    }
    if let Some(facts) = &set.cmp_facts {
        cmp_native::insert_set_facts(conn, snapshot_key, owner, facts)?;
    }

    for (order, asset) in set.assets.iter().enumerate() {
        insert_asset_requirement(
            conn,
            snapshot_key,
            set_id,
            checked_order(order, "catalog assets")?,
            asset,
        )?;
    }
    insert_mame_switches_and_bios(conn, set_id, set)?;
    Ok(owner)
}

fn insert_mame_switches_and_bios(
    conn: &mut SqliteConnection,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    let mut values = InsertBatch::new();
    mame_switches::insert_values_bulk(conn, &mut values, set_id, set)?;
    values.flush(conn)?;

    let mut positions = PositionBatch::default();
    mame_switches::insert_positions_bulk(conn, &mut positions, set_id, set)?;
    positions.flush(conn)
}

fn insert_asset_requirement(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set_id: i64,
    component_order: i64,
    asset: &SnapshotAsset,
) -> crate::Result<OccurrenceId> {
    let input = asset_identity_input(asset)?;
    let digests = input.assertions;
    let resolution = if input.eligible {
        resolve_content_identity(conn, input.size, digests)?
    } else {
        ContentIdentityResolution::NoEligibleEvidence
    };
    let content_uuid = resolution
        .content_id()
        .map(|content_id| content_id.as_bytes().to_vec());
    let occurrence = root_assets::insert(
        conn,
        CatalogSetId::from_database(set_id),
        component_order,
        asset,
        content_uuid.as_deref(),
    )?;
    reported_relationships::insert_asset_merge(conn, snapshot_key, occurrence, asset)?;
    if let NativeAssetFacts::Logiqx(attributes) = &asset.native {
        attributes.attribute_positions.insert(conn, occurrence)?;
    }
    if let Some(facts) = asset.cmp_rom_facts() {
        cmp_native::insert_rom_positions(conn, occurrence.database_value(), facts)?;
    }
    record_occurrence_digest_assertions(conn, occurrence, digests, "source_declared")?;
    if let NativeAssetFacts::NoIntroPc {
        attribute_positions,
        ..
    } = &asset.native
    {
        no_intro_pc_native::insert_rom_positions(conn, occurrence, attribute_positions)?;
    }
    if let Some(facts) = asset.cmp_rom_facts() {
        for field in [&facts.crc, &facts.crc32].into_iter().flatten() {
            let digest = hex::decode(&field.value).map_err(|error| {
                crate::Error::InvalidHash(format!("invalid ClrMamePro CRC declaration: {error}"))
            })?;
            record_occurrence_digest_assertions(
                conn,
                occurrence,
                ContentDigestAssertions::new(asset.evidence_scope, Some(&digest), None, None, None),
                "source_declared",
            )?;
        }
    }
    record_content_identity_conflict(conn, occurrence, &resolution)?;
    Ok(occurrence)
}

fn asset_identity_input(asset: &SnapshotAsset) -> crate::Result<ContentIdentityInput<'_>> {
    let parsed_size = asset.size.map(i64::try_from).transpose();
    let oversized_pc_size =
        matches!(&asset.native, NativeAssetFacts::NoIntroPc { .. }) && parsed_size.is_err();
    let size = if oversized_pc_size {
        None
    } else {
        parsed_size.map_err(|_| crate::Error::InvalidRomSize(asset.size.unwrap_or_default()))?
    };
    let digests = ContentDigestAssertions::new(
        asset.evidence_scope,
        asset.crc.as_deref(),
        asset.md5.as_deref(),
        asset.sha1.as_deref(),
        None,
    );
    // MAME listxml emits complete file size (output_rom/rom_file_size),
    // unlike software-list ROM entries, whose size is one load segment.
    let ineligible = oversized_pc_size
        || asset.role == "other"
        || asset
            .cmp_rom_facts()
            .is_some_and(crate::clrmamepro::AssetFacts::has_conflicting_declarations)
        || matches!(
            &asset.native,
            NativeAssetFacts::Logiqx(LogiqxAssetAttributes {
                identity_eligibility: SourceIdentityEligibility::UninterpretedDeclaration,
                ..
            })
        )
        || matches!(&asset.native, NativeAssetFacts::Mame { declarations, .. }
            if !declarations.declarations_interpretable());
    Ok(ContentIdentityInput {
        size,
        assertions: digests,
        eligible: !ineligible,
    })
}

fn sqlite_mame_boolean(value: mame::MameBoolean) -> i64 {
    i64::from(value.as_bool())
}

fn insert_logiqx_document_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    facts: &LogiqxDocumentFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO logiqx_document_facts \
         (snapshot_key, build, debug, debug_was_present, file_name, sha1, header_name, header_description, \
          header_version, header_date, header_author, header_email, header_homepage, header_url, \
          header_comment, header_category) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(facts.build.as_deref())
    .bind::<Text, _>(&facts.debug)
    .bind::<diesel::sql_types::Bool, _>(facts.debug_was_present)
    .bind::<Nullable<Text>, _>(facts.file_name.as_deref())
    .bind::<Nullable<Binary>, _>(facts.sha1.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_name.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_description.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_version.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_date.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_author.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_email.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_homepage.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_url.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_comment.as_deref())
    .bind::<Nullable<Text>, _>(facts.header_category.as_deref())
    .execute(conn)?;
    Ok(())
}

fn insert_logiqx_set_facts(
    conn: &mut SqliteConnection,
    set_id: i64,
    facts: &LogiqxSetFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO logiqx_games \
         (set_id, source_file, is_bios, is_bios_was_present, board, rebuild_to, description, year, manufacturer) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Nullable<Text>, _>(facts.source_file.as_deref())
    .bind::<Text, _>(&facts.is_bios)
    .bind::<diesel::sql_types::Bool, _>(facts.is_bios_was_present)
    .bind::<Nullable<Text>, _>(facts.board.as_deref())
    .bind::<Nullable<Text>, _>(facts.rebuild_to.as_deref())
    .bind::<Nullable<Text>, _>(facts.description.as_deref())
    .bind::<Nullable<Text>, _>(facts.year.as_deref())
    .bind::<Nullable<Text>, _>(facts.manufacturer.as_deref())
    .execute(conn)?;
    Ok(())
}

fn insert_no_intro_game_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    owner: CatalogSetId,
    facts: &crate::no_intro_pc_xml::GameFacts,
) -> crate::Result<()> {
    let set_id = owner.as_i64();
    sql_query(
        "INSERT INTO no_intro_pc_games \
         (set_id, document_order, archive_id, description, description_order, description_line, description_column, \
          name_alt, region, version, bios_text, languages_present) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(checked_order(facts.source_order, "P/C document fields")?)
    .bind::<Nullable<Text>, _>(
        facts
            .archive_id
            .as_ref()
            .map(crate::no_intro_pc_xml::ArchiveId::as_str),
    )
    .bind::<Nullable<Text>, _>(facts.description.as_deref())
    .bind::<Nullable<BigInt>, _>(facts.description_order.map(|order| checked_order(order, "P/C game fields")).transpose()?)
    .bind::<Nullable<BigInt>, _>(facts.description_location.map(|location| location.line))
    .bind::<Nullable<BigInt>, _>(facts.description_location.map(|location| location.column))
    .bind::<Nullable<Text>, _>(facts.name_alt.as_deref())
    .bind::<Nullable<Text>, _>(facts.region.as_deref())
    .bind::<Nullable<Text>, _>(facts.version.as_deref())
    .bind::<Nullable<Text>, _>(facts.bios_text.as_deref())
    .bind::<diesel::sql_types::Bool, _>(facts.languages.is_some())
    .execute(conn)?;
    if let Some(languages) = &facts.languages {
        for (order, language) in languages.iter().enumerate() {
            sql_query(
                "INSERT INTO no_intro_pc_languages(set_id,language_order,language) VALUES (?,?,?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(checked_order(order, "P/C languages")?)
            .bind::<Text, _>(language)
            .execute(conn)?;
        }
    }
    match &facts.clone_reference {
        Some(crate::no_intro_pc_xml::CloneReference::Parent) => {
            sql_query("INSERT INTO no_intro_pc_clone_markers(set_id) VALUES (?)")
                .bind::<BigInt, _>(set_id)
                .execute(conn)?;
        }
        Some(crate::no_intro_pc_xml::CloneReference::Archive(target)) => {
            reported_relationships::insert_reference(
                conn,
                snapshot_key,
                reported_relationships::ReferenceOwner::NoIntroPc {
                    set: owner,
                    field: crate::domain::NoIntroArchiveReferenceField::Clone,
                },
                target.as_str(),
            )?;
        }
        None => {}
    }
    if let Some(target) = &facts.merge_of {
        reported_relationships::insert_reference(
            conn,
            snapshot_key,
            reported_relationships::ReferenceOwner::NoIntroPc {
                set: owner,
                field: crate::domain::NoIntroArchiveReferenceField::MergeOf,
            },
            target.as_str(),
        )?;
    }
    no_intro_pc_native::insert_game_positions(conn, owner, &facts.attribute_positions)?;
    Ok(())
}

fn insert_mame_machine_facts(
    conn: &mut SqliteConnection,
    set_id: i64,
    facts: &crate::mame::MachineFacts,
) -> crate::Result<()> {
    let mut values = InsertBatch::default();
    enqueue_mame_machine_facts(conn, &mut values, set_id, facts)?;
    values.flush(conn)?;
    let mut positions = PositionBatch::default();
    enqueue_mame_machine_positions(conn, &mut positions, set_id, facts)?;
    positions.flush(conn)
}

fn enqueue_mame_machine_facts(
    conn: &mut SqliteConnection,
    batch: &mut InsertBatch,
    set_id: i64,
    facts: &crate::mame::MachineFacts,
) -> crate::Result<()> {
    cached_sql(
        "INSERT INTO mame_machines \
         (set_id, source_file, description, description_source_order, description_line, description_column, \
          year, year_source_order, year_line, year_column, manufacturer, manufacturer_source_order, manufacturer_line, manufacturer_column, \
          is_device, is_device_specified, runnable, runnable_specified, is_bios, is_bios_specified, \
          is_mechanical, is_mechanical_specified, attributes_line, attributes_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Nullable<Text>, _>(facts.source_file.as_deref())
    .bind::<Text, _>(&facts.description)
    .bind::<BigInt, _>(facts.description_source_order)
    .bind::<BigInt, _>(facts.description_location.line)
    .bind::<BigInt, _>(facts.description_location.column)
    .bind::<Nullable<Text>, _>(facts.year.as_deref())
    .bind::<Nullable<BigInt>, _>(facts.year_source_order)
    .bind::<Nullable<BigInt>, _>(facts.year_location.map(|location| location.line))
    .bind::<Nullable<BigInt>, _>(facts.year_location.map(|location| location.column))
    .bind::<Nullable<Text>, _>(facts.manufacturer.as_deref())
    .bind::<Nullable<BigInt>, _>(facts.manufacturer_source_order)
    .bind::<Nullable<BigInt>, _>(
        facts.manufacturer_location.map(|location| location.line),
    )
    .bind::<Nullable<BigInt>, _>(
        facts.manufacturer_location.map(|location| location.column),
    )
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_device())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_device_specified())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_runnable())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_runnable_specified())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_bios())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_bios_specified())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_mechanical())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_mechanical_specified())
    .bind::<BigInt, _>(facts.attributes_location.line)
    .bind::<BigInt, _>(facts.attributes_location.column)
    .enqueue(batch, conn, InsertPhase::Parents)?;
    if facts.flags.is_consumable_specified() {
        cached_sql("INSERT INTO mame_machine_compatibility(set_id,is_consumable,is_consumable_specified) VALUES (?,?,1)")
            .bind::<BigInt, _>(set_id)
            .bind::<diesel::sql_types::Bool, _>(facts.flags.is_consumable())
            .enqueue(batch, conn, InsertPhase::Children)?;
    }
    Ok(())
}

fn enqueue_mame_machine_positions(
    conn: &mut SqliteConnection,
    positions: &mut PositionBatch,
    set_id: i64,
    facts: &crate::mame::MachineFacts,
) -> crate::Result<()> {
    positions.queue(
        conn,
        Family::Machine,
        &[set_id],
        &facts.attribute_positions,
        mame::MameMachineAttribute::code,
    )?;
    if facts.flags.is_consumable_specified() {
        positions.queue(
            conn,
            Family::MachineCompatibility,
            &[set_id],
            &facts.compatibility_attribute_positions,
            mame::MameMachineCompatibilityAttribute::code,
        )?;
    }
    Ok(())
}

fn insert_mame_document_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &mame::MameHeader,
) -> crate::Result<()> {
    let document = sql_query(
        "INSERT INTO mame_document_facts \
         (snapshot_key, build, debug, debug_specified, config_version, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING document_id",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(header.build.as_deref())
    .bind::<diesel::sql_types::Bool, _>(header.debug)
    .bind::<diesel::sql_types::Bool, _>(header.debug_specified)
    .bind::<Text, _>(&header.config_version)
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column)
    .get_result::<MameDocumentId>(conn)?;
    mame_attributes::insert(
        conn,
        Family::Document,
        &[document.document_id],
        &header.attribute_positions,
        mame::MameDocumentAttribute::code,
    )?;
    Ok(())
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}

#[cfg(test)]
mod machine_switch_batch_tests {
    use std::fmt::Write as _;
    use std::sync::{Arc, Mutex};

    use diesel::{
        Connection, RunQueryDsl, SqliteConnection,
        connection::{InstrumentationEvent, SimpleConnection},
        sql_query,
    };

    use super::{insert_mame_switches_and_bios, machine_contents};

    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "test fixture setup and statement-count assertions must fail loudly"
    )]
    fn machine_switch_rows_use_bounded_insert_statements() -> crate::Result<()> {
        let mut xml = String::from(
            "<mame mameconfig='10'><machine name='test'><description>Test</description>",
        );
        for index in 0..130 {
            write!(
                xml,
                "<dipswitch name='S{index}' tag=':S{index}' mask='1'><diplocation name='L{index}' number='{index}'/><dipvalue name='V{index}' value='{index}'/></dipswitch>"
            )
            .expect("writing a string cannot fail");
        }
        xml.push_str("</machine></mame>");
        let catalog = crate::mame::MameCatalog::parse(xml.as_bytes())?;
        let machine = catalog
            .machines
            .into_iter()
            .next()
            .ok_or_else(|| crate::Error::InvalidPath("parsed machine missing".into()))?;
        let set = machine_contents(machine);

        let mut connection =
            SqliteConnection::establish(":memory:").expect("open in-memory SQLite");
        connection.batch_execute(
            "CREATE TABLE machine_switches (
                set_id INTEGER, switch_order INTEGER, kind TEXT, name TEXT, tag TEXT, mask TEXT,
                source_order INTEGER, source_line INTEGER, source_column INTEGER
            );
            CREATE TABLE machine_switch_locations (
                set_id INTEGER, switch_order INTEGER, location_order INTEGER, source_order INTEGER,
                name TEXT, number TEXT, inverted INTEGER, inverted_specified INTEGER,
                source_line INTEGER, source_column INTEGER
            );
            CREATE TABLE machine_switch_values (
                set_id INTEGER, switch_order INTEGER, value_order INTEGER, source_order INTEGER,
                name TEXT, value TEXT, is_default INTEGER, default_specified INTEGER,
                source_line INTEGER, source_column INTEGER
            );
            CREATE TABLE machine_switches_attribute_positions (
                set_id INTEGER, switch_order INTEGER, field_kind INTEGER, source_order INTEGER,
                source_line INTEGER, source_column INTEGER
            );
            CREATE TABLE machine_switch_locations_attribute_positions (
                set_id INTEGER, switch_order INTEGER, location_order INTEGER, field_kind INTEGER,
                source_order INTEGER, source_line INTEGER, source_column INTEGER
            );
            CREATE TABLE machine_switch_values_attribute_positions (
                set_id INTEGER, switch_order INTEGER, value_order INTEGER, field_kind INTEGER,
                source_order INTEGER, source_line INTEGER, source_column INTEGER
            );",
        )?;
        let statements = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = Arc::clone(&statements);
        connection.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                let query = query.to_string();
                if [
                    "INSERT INTO machine_switches ",
                    "INSERT INTO machine_switch_locations ",
                    "INSERT INTO machine_switch_values ",
                ]
                .iter()
                .any(|prefix| query.starts_with(prefix))
                {
                    captured.lock().expect("statement-count mutex").push(query);
                }
            }
        });

        insert_mame_switches_and_bios(&mut connection, 7, &set)?;

        let statements = std::mem::take(&mut *statements.lock().expect("statement-count mutex"));
        assert_eq!(
            statements.len(),
            6,
            "130 rows per table should split at SQLite's bind limit; statements: {statements:?}"
        );
        for table in [
            "machine_switches",
            "machine_switch_locations",
            "machine_switch_values",
        ] {
            assert_eq!(
                statements
                    .iter()
                    .filter(|statement| statement.starts_with(&format!("INSERT INTO {table} ")))
                    .count(),
                2,
                "unexpected batch count for {table}"
            );
        }
        assert!(
            statements
                .iter()
                .all(|statement| statement.matches('?').count() <= 999)
        );
        let rows = sql_query(
            "SELECT (SELECT count(*) FROM machine_switches) + \
                    (SELECT count(*) FROM machine_switch_locations) + \
                    (SELECT count(*) FROM machine_switch_values) AS count",
        )
        .get_result::<Count>(&mut connection)?;
        assert_eq!(rows.count, 390);
        Ok(())
    }
}
