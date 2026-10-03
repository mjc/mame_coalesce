use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    clrmamepro::Catalog as ClrMameProCatalog,
    domain::{
        CatalogSetId, DocumentKey, ImportRunKey, OccurrenceId, ParserInterpretationKey, SnapshotKey,
    },
    logiqx::{DataFile, Game, XmlSourceMap},
    mame::{self, MameRecord, ValidatedMame},
    mame_softwarelist::SoftwareListCatalog,
    no_intro_pc_xml::Catalog as NoIntroCatalog,
    storage::{
        catalog_content::{
            ContentDigestAssertions, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions, resolve_content_identity,
        },
        db::Pool,
        documents::DocumentStore,
    },
};

mod cmp_native;
mod logiqx_cmp_relationships;
mod logiqx_native;
mod mame_relationships;
mod mame_specification;
mod no_intro_dat_native;
mod no_intro_database_native;
mod no_intro_pc_native;
mod reported_relationships;
mod root_assets;
mod root_sets;
mod software_native;

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
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    acquisition_source: Option<String>,
}

#[derive(QueryableByName)]
struct IdentityRow {
    #[diesel(sql_type = Text)]
    first_value: String,
    #[diesel(sql_type = Nullable<Text>)]
    second_value: Option<String>,
}

struct SnapshotData {
    version: Option<String>,
    sets: Vec<SnapshotSet>,
    software_lists: Option<SoftwareListCatalog>,
    logiqx_document_facts: Option<LogiqxDocumentFacts>,
    logiqx_document_details: Option<logiqx_native::DocumentDetails>,
    cmp_header_facts: Option<crate::clrmamepro::Header>,
    cmp_comments: Option<Vec<crate::clrmamepro::Comment>>,
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
    fn from_data_file(data_file: &DataFile) -> Self {
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
        attributes: mame::MameAssetAttributes,
        declarations: mame::MameAssetDeclarations,
        source_order: i64,
    },
    MameSample {
        source_order: i64,
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
    ParserInterpretationKey::for_format(request.format.as_str(), &request.scope)
}

impl SnapshotData {
    fn from_logiqx(data_file: &DataFile, source_map: &XmlSourceMap) -> crate::Result<Self> {
        let mut sets = Vec::with_capacity(data_file.games().len());
        for (game_index, game) in data_file.games().iter().enumerate() {
            let location = source_map
                .game_locations
                .get(game_index)
                .copied()
                .ok_or_else(|| {
                    crate::Error::InvalidPath("Logiqx set source location is missing".into())
                })?;
            let rom_locations = source_map.rom_locations.get(game_index).ok_or_else(|| {
                crate::Error::InvalidPath("Logiqx asset source locations are missing".into())
            })?;
            let device_ref_locations =
                source_map
                    .device_ref_locations
                    .get(game_index)
                    .ok_or_else(|| {
                        crate::Error::InvalidPath(
                            "Logiqx device-reference source locations are missing".into(),
                        )
                    })?;
            if game.device_refs().count() != device_ref_locations.len() {
                return Err(crate::Error::InvalidPath(
                    "Logiqx device-reference source locations do not match parsed references"
                        .into(),
                ));
            }
            let assets = logiqx_assets(game, rom_locations)?;
            sets.push(SnapshotSet {
                name: game.name().into(),
                parent: game.cloneof().map(str::to_owned),
                runtime_dependencies: game
                    .romof_opt()
                    .map(|name| SnapshotDependency {
                        source_field: "romof".to_owned(),
                        target_name: name.to_owned(),
                        reference_tag: None,
                        source_order: None,
                        location,
                    })
                    .into_iter()
                    .chain(game.sampleof_opt().map(|name| SnapshotDependency {
                        source_field: "sampleof".to_owned(),
                        target_name: name.to_owned(),
                        reference_tag: None,
                        source_order: None,
                        location,
                    }))
                    .chain(
                        game.device_refs()
                            .zip(device_ref_locations)
                            .map(|(name, location)| SnapshotDependency {
                                source_field: "device_ref".to_owned(),
                                target_name: name.to_owned(),
                                reference_tag: None,
                                source_order: None,
                                location: *location,
                            }),
                    )
                    .collect(),
                location,
                assets,
                switches: Vec::new(),
                bios_sets: Vec::new(),
                specification: Vec::new(),
                mame_facts: None,
                no_intro_facts: None,
                logiqx_facts: Some(LogiqxSetFacts::from_game(game)),
                logiqx_details: Some(logiqx_native::GameDetails::from_game(game)?),
                cmp_facts: None,
                machine_dependencies: Vec::new(),
            });
        }
        Ok(Self {
            version: data_file
                .header_opt()
                .and_then(|header| header.version().cloned()),
            sets,
            software_lists: None,
            logiqx_document_facts: Some(LogiqxDocumentFacts::from_data_file(data_file)),
            logiqx_document_details: Some(logiqx_native::DocumentDetails::from_data_file(
                data_file,
            )?),
            cmp_header_facts: None,
            cmp_comments: None,
            no_intro_document: None,
        })
    }

    const fn from_mame_softwarelist(catalog: SoftwareListCatalog) -> Self {
        Self {
            // Wrapper build text has one native owner, not a second snapshot copy.
            version: None,
            sets: Vec::new(),
            software_lists: Some(catalog),
            logiqx_document_facts: None,
            logiqx_document_details: None,
            cmp_header_facts: None,
            cmp_comments: None,
            no_intro_document: None,
        }
    }

    fn from_clrmamepro(catalog: ClrMameProCatalog) -> Self {
        let sets =
            catalog
                .sets
                .into_iter()
                .map(|mut set| {
                    let mut media =
                        set.assets
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
                            .chain(std::mem::take(&mut set.native.samples).into_iter().map(
                                |sample| {
                                    (
                                        sample.order,
                                        SnapshotAsset::filename_only(
                                            sample.value.clone(),
                                            sample.location,
                                            NativeAssetFacts::CmpSample(sample),
                                        ),
                                    )
                                },
                            ))
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
                })
                .collect();
        Self {
            version: catalog.version,
            sets,
            software_lists: None,
            logiqx_document_facts: None,
            logiqx_document_details: None,
            cmp_header_facts: catalog.header,
            cmp_comments: Some(catalog.comments),
            no_intro_document: None,
        }
    }

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
            version: None,
            sets,
            software_lists: None,
            logiqx_document_facts: None,
            logiqx_document_details: None,
            cmp_header_facts: None,
            cmp_comments: None,
            no_intro_document: Some(no_intro_pc_native::DocumentFacts {
                location: catalog.document_location,
                header: catalog.header,
            }),
        }
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
        })
        .chain(machine.rom_of.into_iter().map(|name| SnapshotDependency {
            source_field: "romof".to_owned(),
            target_name: name,
            reference_tag: None,
            source_order: None,
            location: machine.location,
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
                        attributes: asset.attributes,
                        declarations: asset.declarations,
                        source_order: asset.source_order,
                    },
                    location: asset.location,
                },
            )
        })
        .collect()
}

pub fn import(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<CatalogImportReport> {
    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone())?;
    let retained = match request.format {
        CatalogDocumentFormat::Logiqx => {
            documents.retain_path(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameListXml => {
            documents.retain_path_mame(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameSoftwareListXml => documents
            .retain_path_mame_softwarelist(request.source_key.clone(), &request.document_path)?,
        CatalogDocumentFormat::ClrMamePro => {
            documents.retain_path_clrmamepro(request.source_key.clone(), &request.document_path)?
        }
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
        CatalogDocumentFormat::Logiqx => DataFile::from_reader_with_source_map(bytes.as_slice())
            .and_then(|(data_file, source_map)| SnapshotData::from_logiqx(&data_file, &source_map)),
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
            SoftwareListCatalog::parse(&bytes).map(SnapshotData::from_mame_softwarelist)
        }
        CatalogDocumentFormat::ClrMamePro => {
            ClrMameProCatalog::parse(&bytes).map(SnapshotData::from_clrmamepro)
        }
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
}

impl StreamingImport<'_> {
    fn consume(&mut self, record: MameRecord) -> crate::Result<()> {
        match record {
            MameRecord::Machine(machine) => {
                let set = machine_contents(*machine);
                if let SnapshotPublication::Pending(key) = &self.publication {
                    insert_snapshot_set(self.conn, key, &set)?;
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
                let publication = prepare_snapshot(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                    None,
                )
                .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &publication {
                    insert_mame_document_facts(conn, key, &header)
                        .map_err(StreamingImportError::Storage)?;
                }
                drop(header.extensions);
                let run_key = ImportRunKey::fresh();
                insert_import_run(
                    conn,
                    request,
                    &document_key,
                    &interpretation,
                    acquisition_key,
                    &run_key,
                    Some(publication.key()),
                    "succeeded",
                    None,
                )?;
                let sink = StreamingImport {
                    conn,
                    publication,
                    run_key,
                };
                Ok(sink)
            },
            |sink, record| sink.consume(record).map_err(StreamingImportError::Storage),
        )?;
        publish_mame_import(validated, request, &document_key, &interpretation)
            .map_err(StreamingImportError::Storage)
    });
    // Release the connection before recording a failure in a separate transaction.
    drop(conn);
    match result {
        Ok(report) => Ok(report),
        Err(StreamingImportError::Storage(error)) => Err(error),
        Err(StreamingImportError::Parse(error)) => {
            record_failed_import(pool, request, document_key, acquisition_key, &error)
        }
    }
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

fn finish_streaming_import(
    sink: StreamingImport<'_>,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
) -> crate::Result<CatalogImportReport> {
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
    digest_scope: no_intro_dat_native::DigestScope,
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
                let publication = prepare_snapshot(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                    None,
                )
                .map_err(StreamingImportError::Storage)?;
                let counts = no_intro_dat_native::ImportCounts::from_document(&document);
                let digest_scope = no_intro_dat_native::DigestScope::from_document(&document);
                if let SnapshotPublication::Pending(key) = &publication {
                    no_intro_dat_native::insert_document(conn, key, &document)
                        .map_err(StreamingImportError::Storage)?;
                }
                let run_key = ImportRunKey::fresh();
                insert_import_run(
                    conn,
                    request,
                    &document_key,
                    &interpretation,
                    acquisition_key,
                    &run_key,
                    Some(publication.key()),
                    "succeeded",
                    None,
                )?;
                Ok(NoIntroDatImport {
                    import: StreamingImport {
                        conn,
                        publication,
                        run_key,
                    },
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
    match result {
        Ok(report) => Ok(report),
        Err(StreamingImportError::Storage(error)) => Err(error),
        Err(StreamingImportError::Parse(error)) => {
            record_failed_import(pool, request, document_key, acquisition_key, &error)
        }
    }
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
        let validated = crate::no_intro_db_xml::read_with::<_, StreamingImportError>(
            bytes,
            mode,
            |document| {
                let publication = prepare_snapshot(
                    conn,
                    request,
                    &document_key,
                    acquisition_key,
                    &interpretation,
                    None,
                )
                .map_err(StreamingImportError::Storage)?;
                let counts = no_intro_database_native::ImportCounts::from_document(&document)
                    .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &publication {
                    no_intro_database_native::insert_document(conn, key, &document)
                        .map_err(StreamingImportError::Storage)?;
                }
                let run_key = ImportRunKey::fresh();
                insert_import_run(
                    conn,
                    request,
                    &document_key,
                    &interpretation,
                    acquisition_key,
                    &run_key,
                    Some(publication.key()),
                    "succeeded",
                    None,
                )?;
                Ok(NoIntroDatabaseImport {
                    import: StreamingImport {
                        conn,
                        publication,
                        run_key,
                    },
                    counts,
                })
            },
            |sink, game| {
                sink.counts
                    .include_game(&game)
                    .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &sink.import.publication {
                    no_intro_database_native::insert_game(sink.import.conn, key, &game)
                        .map_err(StreamingImportError::Storage)?;
                }
                Ok(())
            },
        )?;
        let (sink, recovery) = validated.into_parts();
        let mut diagnostic_count = 0_usize;
        if let Some(recovery) = &recovery {
            for warning in recovery.warnings() {
                let diagnostic = super::import_diagnostics::insert_recovery_warning(
                    sink.import.conn,
                    &sink.import.run_key,
                    &document_key,
                    &warning,
                )
                .map_err(StreamingImportError::Storage)?;
                super::import_diagnostics::link_no_intro_details(sink.import.conn, &diagnostic)
                    .map_err(StreamingImportError::Storage)?;
                diagnostic_count = diagnostic_count.checked_add(1).ok_or_else(|| {
                    StreamingImportError::Storage(crate::Error::DatabaseSchema(
                        "diagnostic count overflow".into(),
                    ))
                })?;
            }
        }
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
    match result {
        Ok(report) => Ok(report),
        Err(StreamingImportError::Storage(error)) => Err(error),
        Err(StreamingImportError::Parse(error)) => {
            record_failed_import(pool, request, document_key, acquisition_key, &error)
        }
    }
}

fn ensure_source(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<()> {
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        sql_query(
            "INSERT INTO publishing_sources (source_key, display_name) VALUES (?, ?) \
             ON CONFLICT(source_key) DO NOTHING",
        )
        .bind::<Text, _>(request.source_key.as_str())
        .bind::<Text, _>(&request.source_display_name)
        .execute(conn)?;
        let source = sql_query(
            "SELECT display_name AS first_value, NULL AS second_value \
             FROM publishing_sources WHERE source_key = ?",
        )
        .bind::<Text, _>(request.source_key.as_str())
        .get_result::<IdentityRow>(conn)?;
        if source.first_value != request.source_display_name {
            return Err(crate::Error::SourceIdentityConflict(
                request.source_key.as_str().to_owned(),
            ));
        }
        Ok(())
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
    let diagnostic = error.to_string();
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
            None,
            "failed",
            Some(&diagnostic),
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
            Some(&snapshot_key),
            "succeeded",
            None,
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
    let publication = prepare_snapshot(
        conn,
        request,
        document_key,
        acquisition_key,
        interpretation,
        snapshot_data.version.as_deref(),
    )?;
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
    version: Option<&str>,
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
        "SELECT snapshot.snapshot_key, snapshot.declared_version, snapshot.coverage_id, \
                acquisition.source_key AS acquisition_source \
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
        row.declared_version.as_deref() == version
            && row.coverage_id == coverage_id.database_value()
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
              declared_version, coverage_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(document_key.to_string())
        .bind::<Text, _>(interpretation.as_str())
        .bind::<Nullable<Text>, _>(Some(acquisition_key.to_owned()))
        .bind::<Nullable<Text>, _>(version)
        .bind::<BigInt, _>(coverage_id.database_value())
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
    sql_query(
        "INSERT INTO publishing_sources (source_key, display_name) VALUES (?, ?) \
         ON CONFLICT(source_key) DO NOTHING",
    )
    .bind::<Text, _>(request.source_key.as_str())
    .bind::<Text, _>(&request.source_display_name)
    .execute(conn)?;
    let source = sql_query(
        "SELECT display_name AS first_value, locator AS second_value \
         FROM publishing_sources WHERE source_key = ?",
    )
    .bind::<Text, _>(request.source_key.as_str())
    .get_result::<IdentityRow>(conn)?;
    if source.first_value != request.source_display_name {
        return Err(crate::Error::SourceIdentityConflict(
            request.source_key.as_str().to_owned(),
        ));
    }

    sql_query(
        "INSERT INTO catalogs (catalog_key, source_key, display_name) VALUES (?, ?, ?) \
         ON CONFLICT(catalog_key) DO NOTHING",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(request.source_key.as_str())
    .bind::<Text, _>(&request.catalog_display_name)
    .execute(conn)?;
    let catalog = sql_query(
        "SELECT source_key AS first_value, display_name AS second_value \
         FROM catalogs WHERE catalog_key = ?",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .get_result::<IdentityRow>(conn)?;
    if catalog.first_value != request.source_key.as_str()
        || catalog.second_value.as_deref() != Some(request.catalog_display_name.as_str())
    {
        return Err(crate::Error::CatalogIdentityConflict(
            request.catalog_key.as_str().to_owned(),
        ));
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
    .bind::<Text, _>(ParserInterpretationKey::rules_version(
        request.format.as_str(),
    ))
    .execute(conn)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_import_run(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    interpretation: &ParserInterpretationKey,
    acquisition_key: &str,
    run_key: &ImportRunKey,
    snapshot_key: Option<&SnapshotKey>,
    status: &str,
    diagnostic: Option<&str>,
) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO import_runs \
         (run_key, catalog_key, document_key, interpretation_key, acquisition_key, snapshot_key, \
          status, started_at, finished_at, diagnostic) \
         VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?)",
    )
    .bind::<Text, _>(run_key.to_string())
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(document_key.to_string())
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Nullable<Text>, _>(Some(acquisition_key.to_owned()))
    .bind::<Nullable<Text>, _>(snapshot_key.map(|key| key.as_str().to_owned()))
    .bind::<Text, _>(status)
    .bind::<Nullable<Text>, _>(diagnostic.map(str::to_owned))
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

    if let Some(catalog) = &snapshot_data.software_lists {
        software_native::insert(conn, snapshot_key, catalog)?;
    }
    if let Some(facts) = &snapshot_data.logiqx_document_facts {
        insert_logiqx_document_facts(conn, snapshot_key, facts)?;
    }
    if let Some(details) = &snapshot_data.logiqx_document_details {
        details.insert(conn, snapshot_key)?;
    }
    if let Some(header) = &snapshot_data.cmp_header_facts {
        cmp_native::insert_header_facts(conn, snapshot_key, header)?;
    }
    if let Some(comments) = &snapshot_data.cmp_comments {
        cmp_native::insert_document_facts(
            conn,
            snapshot_key,
            snapshot_data.cmp_header_facts.is_some(),
            comments,
        )?;
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
    insert_machine_switches(conn, set_id, set)?;
    insert_machine_bios_sets(conn, set_id, set)?;
    Ok(owner)
}

fn insert_asset_requirement(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set_id: i64,
    component_order: i64,
    asset: &SnapshotAsset,
) -> crate::Result<OccurrenceId> {
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
    let resolution = if oversized_pc_size
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
            if !declarations.declarations_interpretable())
    {
        ContentIdentityResolution::NoEligibleEvidence
    } else {
        resolve_content_identity(conn, size, digests)?
    };
    let content_uuid = resolution
        .content_id()
        .map(|content_id| content_id.as_bytes().to_vec());
    let occurrence = root_assets::insert(
        conn,
        CatalogSetId::from_database(set_id),
        component_order,
        asset,
        content_uuid,
    )?;
    reported_relationships::insert_asset_merge(conn, snapshot_key, occurrence, asset)?;
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
    sql_query(
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
    .execute(conn)?;
    if facts.flags.is_consumable_specified() {
        sql_query("INSERT INTO mame_machine_compatibility(set_id,is_consumable,is_consumable_specified) VALUES (?,?,1)")
            .bind::<BigInt, _>(set_id)
            .bind::<diesel::sql_types::Bool, _>(facts.flags.is_consumable())
            .execute(conn)?;
    }
    Ok(())
}

fn insert_mame_document_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    header: &mame::MameHeader,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO mame_document_facts \
         (snapshot_key, build, debug, debug_specified, config_version, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(header.build.as_deref())
    .bind::<diesel::sql_types::Bool, _>(header.debug)
    .bind::<diesel::sql_types::Bool, _>(header.debug_specified)
    .bind::<Text, _>(&header.config_version)
    .bind::<BigInt, _>(header.location.line)
    .bind::<BigInt, _>(header.location.column)
    .execute(conn)?;
    Ok(())
}

fn insert_machine_bios_sets(
    conn: &mut SqliteConnection,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (bios_order, bios_set) in set.bios_sets.iter().enumerate() {
        sql_query(
            "INSERT INTO mame_bios_sets \
             (set_id, bios_order, name, description, is_default, default_specified, source_order, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(checked_order(bios_order, "MAME BIOS sets")?)
        .bind::<Text, _>(&bios_set.name)
        .bind::<Text, _>(&bios_set.description)
        .bind::<diesel::sql_types::Bool, _>(bios_set.is_default)
        .bind::<diesel::sql_types::Bool, _>(bios_set.default_specified)
        .bind::<BigInt, _>(bios_set.source_order)
        .bind::<BigInt, _>(bios_set.location.line)
        .bind::<BigInt, _>(bios_set.location.column)
        .execute(conn)?;
    }
    Ok(())
}

fn insert_machine_switches(
    conn: &mut SqliteConnection,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (switch_order, switch) in set.switches.iter().enumerate() {
        let switch_order = checked_order(switch_order, "machine switches")?;
        sql_query(
            "INSERT INTO machine_switches \
             (set_id, switch_order, kind, name, tag, mask, source_order, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(switch_order)
        .bind::<Text, _>(switch.kind.as_str())
        .bind::<Text, _>(&switch.name)
        .bind::<Text, _>(&switch.tag)
        .bind::<Text, _>(&switch.mask)
        .bind::<BigInt, _>(switch.source_order)
        .bind::<BigInt, _>(switch.location.line)
        .bind::<BigInt, _>(switch.location.column)
        .execute(conn)?;
        if let Some(condition) = &switch.condition {
            mame_specification::insert_switch_condition(conn, set_id, switch_order, condition)?;
        }

        for (location_order, location) in switch.locations.iter().enumerate() {
            sql_query(
                "INSERT INTO machine_switch_locations \
                 (set_id, switch_order, location_order, source_order, name, number, inverted, inverted_specified, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(location_order, "machine switch locations")?)
            .bind::<BigInt, _>(location.source_order)
            .bind::<Text, _>(&location.name)
            .bind::<Text, _>(&location.number)
            .bind::<diesel::sql_types::Bool, _>(location.inverted)
            .bind::<diesel::sql_types::Bool, _>(location.inverted_specified)
            .bind::<BigInt, _>(location.location.line)
            .bind::<BigInt, _>(location.location.column)
            .execute(conn)?;
        }

        for (value_order, switch_value) in switch.values.iter().enumerate() {
            sql_query(
                "INSERT INTO machine_switch_values \
                 (set_id, switch_order, value_order, source_order, name, value, is_default, default_specified, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(value_order, "machine switch values")?)
            .bind::<BigInt, _>(switch_value.source_order)
            .bind::<Text, _>(&switch_value.name)
            .bind::<Text, _>(&switch_value.value)
            .bind::<diesel::sql_types::Bool, _>(switch_value.default)
            .bind::<diesel::sql_types::Bool, _>(switch_value.default_specified)
            .bind::<BigInt, _>(switch_value.location.line)
            .bind::<BigInt, _>(switch_value.location.column)
            .execute(conn)?;
            if let Some(condition) = &switch_value.condition {
                mame_specification::insert_switch_value_condition(
                    conn,
                    set_id,
                    switch_order,
                    checked_order(value_order, "MAME switch values")?,
                    condition,
                )?;
            }
        }
    }
    Ok(())
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
