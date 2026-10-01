use std::collections::HashMap;

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    clrmamepro::Catalog as ClrMameProCatalog,
    domain::{
        CatalogRecordKind, CatalogRecordRef, CatalogSetId, DocumentKey, DocumentLocation,
        ImportRunKey, ParserInterpretationKey, RelationshipType, SnapshotKey,
    },
    logiqx::{DataFile, Game, XmlSourceMap},
    mame::{self, ExtensionValue, MameRecord, ValidatedMame},
    mame_softwarelist::SoftwareListCatalog,
    no_intro_pc_xml::Catalog as NoIntroCatalog,
    storage::{
        catalog_content::{
            ContentDigestAssertions, ContentIdentityResolution, record_content_identity_conflict,
            record_occurrence_digest_assertions, resolve_content_identity,
        },
        db::Pool,
        documents::DocumentStore,
        relationships::{SourceRelationshipDraft, insert_source_assertion},
    },
};

mod cmp_native;
mod mame_specification;
mod merges;
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

#[derive(QueryableByName)]
struct ExtensionOccurrenceId {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

struct SnapshotData {
    version: Option<String>,
    sets: Vec<SnapshotSet>,
    software_lists: Option<SoftwareListCatalog>,
    extensions: Vec<StoredExtension>,
    logiqx_document_facts: Option<LogiqxDocumentFacts>,
    cmp_header_facts: Option<crate::clrmamepro::Header>,
}

#[derive(Clone)]
struct LogiqxDocumentFacts {
    build: Option<String>,
    debug: Option<String>,
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
            debug: data_file.debug().map(str::to_owned),
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
    is_bios: Option<String>,
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
            is_bios: game.isbios_opt().map(str::to_owned),
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
    parent_field: Option<String>,
    runtime_dependencies: Vec<SnapshotDependency>,
    location: crate::logiqx::RecordLocation,
    assets: Vec<SnapshotAsset>,
    switches: Vec<crate::mame::MachineSwitch>,
    bios_sets: Vec<crate::mame::MachineBiosSet>,
    specification: Vec<crate::mame::MachineSpecificationElement>,
    mame_facts: Option<crate::mame::MachineFacts>,
    no_intro_facts: Option<crate::no_intro_pc_xml::GameFacts>,
    logiqx_facts: Option<LogiqxSetFacts>,
    cmp_facts: Option<crate::clrmamepro::SetFacts>,
    machine_dependencies: Vec<SnapshotDependency>,
}

struct SnapshotDependency {
    source_field: String,
    target_name: String,
    reference_tag: Option<String>,
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
    mame_attributes: Option<mame::MameAssetAttributes>,
    cmp_facts: Option<crate::clrmamepro::AssetFacts>,
    location: crate::logiqx::RecordLocation,
}

impl SnapshotAsset {
    fn from_logiqx(
        rom: &crate::logiqx::Rom,
        location: crate::logiqx::RecordLocation,
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
            mame_attributes: None,
            cmp_facts: None,
            location,
        })
    }
}

struct StoredExtension {
    record_kind: String,
    record_name: Option<String>,
    owner_set_index: Option<usize>,
    owner_component_index: Option<usize>,
    field_name: String,
    namespace_uri: Option<String>,
    value: ExtensionValue,
    location: crate::logiqx::RecordLocation,
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
            let mut assets = Vec::with_capacity(game.roms().len());
            for (index, rom) in game.roms().iter().enumerate() {
                let location = *rom_locations.get(index).ok_or_else(|| {
                    crate::Error::InvalidPath("Logiqx asset source location is missing".into())
                })?;
                assets.push(SnapshotAsset::from_logiqx(rom, location)?);
            }
            sets.push(SnapshotSet {
                name: game.name().into(),
                parent: game.cloneof().map(str::to_owned),
                parent_field: game.cloneof().map(|_| "cloneof".to_owned()),
                runtime_dependencies: game
                    .romof_opt()
                    .map(|name| SnapshotDependency {
                        source_field: "romof".to_owned(),
                        target_name: name.to_owned(),
                        reference_tag: None,
                        location,
                    })
                    .into_iter()
                    .chain(game.sampleof_opt().map(|name| SnapshotDependency {
                        source_field: "sampleof".to_owned(),
                        target_name: name.to_owned(),
                        reference_tag: None,
                        location,
                    }))
                    .chain(
                        game.device_refs()
                            .zip(device_ref_locations)
                            .map(|(name, location)| SnapshotDependency {
                                source_field: "device_ref".to_owned(),
                                target_name: name.to_owned(),
                                reference_tag: None,
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
            extensions: stored_logiqx_extensions(source_map),
            logiqx_document_facts: Some(LogiqxDocumentFacts::from_data_file(data_file)),
            cmp_header_facts: None,
        })
    }

    fn from_mame_softwarelist(catalog: SoftwareListCatalog) -> Self {
        let version = catalog.build.clone();
        let extensions = catalog
            .extensions
            .iter()
            .cloned()
            .map(stored_extension)
            .collect();
        Self {
            version,
            sets: Vec::new(),
            software_lists: Some(catalog),
            extensions,
            logiqx_document_facts: None,
            cmp_header_facts: None,
        }
    }

    fn from_clrmamepro(catalog: ClrMameProCatalog) -> Self {
        let mut extensions = catalog
            .extensions
            .into_iter()
            .map(stored_clrmamepro_extension)
            .collect::<Vec<_>>();
        let sets = catalog
            .sets
            .into_iter()
            .enumerate()
            .map(|(set_index, set)| {
                extensions.extend(set.extensions.into_iter().map(|extension| {
                    stored_clrmamepro_extension(extension).with_set_owner(set_index)
                }));
                let parent_field = set.parent.as_ref().map(|_| "cloneof".to_owned());
                let assets = set
                    .assets
                    .into_iter()
                    .enumerate()
                    .map(|(component_index, asset)| {
                        extensions.extend(asset.extensions.into_iter().map(|extension| {
                            stored_clrmamepro_extension(extension)
                                .with_asset_owner(set_index, component_index)
                        }));
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
                            mame_attributes: None,
                            cmp_facts: Some(asset.native),
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: set.name,
                    parent: set.parent,
                    parent_field,
                    runtime_dependencies: Vec::new(),
                    location: set.location,
                    assets,
                    switches: Vec::new(),
                    bios_sets: Vec::new(),
                    specification: Vec::new(),
                    mame_facts: None,
                    no_intro_facts: None,
                    logiqx_facts: None,
                    cmp_facts: Some(set.native),
                    machine_dependencies: Vec::new(),
                }
            })
            .collect();
        Self {
            version: catalog.version,
            sets,
            software_lists: None,
            extensions,
            logiqx_document_facts: None,
            cmp_header_facts: catalog.header,
        }
    }

    fn from_no_intro(catalog: NoIntroCatalog) -> Self {
        let mut extensions = catalog
            .extensions
            .into_iter()
            .map(stored_extension)
            .collect::<Vec<_>>();
        let sets = catalog
            .entries
            .into_iter()
            .enumerate()
            .map(|(set_index, entry)| {
                extensions.extend(
                    entry
                        .extensions
                        .into_iter()
                        .map(|extension| stored_extension(extension).with_set_owner(set_index)),
                );
                let entry_name = entry.name;
                let no_intro_facts = entry.facts;
                let assets = entry
                    .assets
                    .into_iter()
                    .enumerate()
                    .map(|(component_order, asset)| {
                        extensions.extend(asset.extensions.into_iter().map(|extension| {
                            stored_extension(extension).with_asset_owner(set_index, component_order)
                        }));
                        SnapshotAsset {
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
                            mame_attributes: None,
                            cmp_facts: None,
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: entry_name,
                    parent: None,
                    parent_field: None,
                    runtime_dependencies: Vec::new(),
                    location: entry.location,
                    assets,
                    switches: Vec::new(),
                    bios_sets: Vec::new(),
                    specification: Vec::new(),
                    mame_facts: None,
                    no_intro_facts: Some(no_intro_facts),
                    logiqx_facts: None,
                    cmp_facts: None,
                    machine_dependencies: Vec::new(),
                }
            })
            .collect();
        Self {
            version: catalog.version,
            sets,
            software_lists: None,
            extensions,
            logiqx_document_facts: None,
            cmp_header_facts: None,
        }
    }
}

fn machine_contents(machine: crate::mame::Machine) -> (SnapshotSet, Vec<StoredExtension>) {
    let mut extensions = Vec::new();
    extensions.extend(machine.extensions.into_iter().map(stored_extension));
    let runtime_dependencies: Vec<SnapshotDependency> = machine
        .device_refs
        .into_iter()
        .map(|reference| SnapshotDependency {
            source_field: "device_ref".to_owned(),
            target_name: reference.name,
            reference_tag: Some(reference.tag),
            location: reference.location,
        })
        .chain(machine.rom_of.into_iter().map(|name| SnapshotDependency {
            source_field: "romof".to_owned(),
            target_name: name,
            reference_tag: None,
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
                    location: machine.location,
                }),
        )
        .collect();
    let assets = machine_assets(machine.assets, &mut extensions);
    let parent_field = machine.parent.as_ref().map(|_| "cloneof".to_owned());
    let switches = machine.switches;
    let bios_sets = machine.bios_sets;
    let mame_facts = machine.facts;
    let machine_dependencies = runtime_dependencies
        .iter()
        .map(|dependency| SnapshotDependency {
            source_field: dependency.source_field.clone(),
            target_name: dependency.target_name.clone(),
            reference_tag: dependency.reference_tag.clone(),
            location: dependency.location,
        })
        .collect();
    let set = SnapshotSet {
        name: machine.name,
        parent: machine.parent,
        parent_field,
        runtime_dependencies,
        location: machine.location,
        assets,
        switches,
        bios_sets,
        specification: machine.specification,
        machine_dependencies,
        mame_facts: Some(mame_facts),
        no_intro_facts: None,
        logiqx_facts: None,
        cmp_facts: None,
    };
    for extension in &mut extensions {
        extension.owner_set_index = Some(0);
    }
    (set, extensions)
}

fn machine_assets(
    assets: Vec<crate::mame::MachineAsset>,
    extensions: &mut Vec<StoredExtension>,
) -> Vec<SnapshotAsset> {
    assets
        .into_iter()
        .enumerate()
        .map(|(component_order, asset)| {
            extensions.extend(
                asset.extensions.into_iter().map(|extension| {
                    stored_extension(extension).with_asset_owner(0, component_order)
                }),
            );
            let evidence_scope = asset
                .disk_requirement
                .as_ref()
                .map_or("whole_asset", |requirement| {
                    requirement.digest_scope().as_str()
                });
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
                mame_attributes: Some(asset.attributes),
                cmp_facts: None,
                location: asset.location,
            }
        })
        .collect()
}

fn stored_extension(ext: crate::mame::XmlExtension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        owner_set_index: None,
        owner_component_index: None,
        field_name: ext.field_name,
        namespace_uri: ext.namespace_uri,
        value: ext.value,
        location: ext.location,
    }
}

fn stored_logiqx_extensions(source_map: &XmlSourceMap) -> Vec<StoredExtension> {
    let mut rom_owners = HashMap::new();
    for (game_index, locations) in source_map.rom_locations.iter().enumerate() {
        for (component_order, location) in locations.iter().enumerate() {
            rom_owners.insert(
                (location.line, location.column),
                (game_index, component_order),
            );
        }
    }
    source_map
        .unsupported_attributes
        .iter()
        .map(|ext| {
            let mut stored = StoredExtension {
                record_kind: ext.record_kind.clone(),
                record_name: ext.record_name.clone(),
                owner_set_index: None,
                owner_component_index: None,
                field_name: ext.field_name.clone(),
                namespace_uri: ext.namespace_uri.clone(),
                value: serde_json::json!(ext.value).into(),
                location: ext.location,
            };
            if ext.record_kind == "rom" {
                let owner = rom_owners
                    .get(&(ext.location.line, ext.location.column))
                    .copied();
                if let Some((game_index, component_order)) = owner {
                    stored = stored.with_asset_owner(game_index, component_order);
                }
            } else if ext.record_kind != "document" {
                // Logiqx currently exposes child positions only for ROMs; other nested
                // extensions keep set ownership until their source map carries ordinals.
                let owner_index = source_map
                    .game_locations
                    .partition_point(|location| {
                        (location.line, location.column) <= (ext.location.line, ext.location.column)
                    })
                    .checked_sub(1);
                if let Some(game_index) = owner_index {
                    stored = stored.with_set_owner(game_index);
                }
            }
            stored
        })
        .collect()
}

fn stored_clrmamepro_extension(ext: crate::clrmamepro::Extension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        owner_set_index: None,
        owner_component_index: None,
        field_name: ext.field_name,
        namespace_uri: None,
        value: ext.value.into(),
        location: ext.location,
    }
}

impl StoredExtension {
    const fn with_set_owner(mut self, set_index: usize) -> Self {
        self.owner_set_index = Some(set_index);
        self
    }

    const fn with_asset_owner(mut self, set_index: usize, component_index: usize) -> Self {
        self.owner_set_index = Some(set_index);
        self.owner_component_index = Some(component_index);
        self
    }
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
    fn extensions(
        &mut self,
        extensions: impl IntoIterator<Item = StoredExtension>,
        set_ids: &[CatalogSetId],
    ) -> crate::Result<()> {
        if let SnapshotPublication::Pending(key) = &self.publication {
            let extensions: Vec<_> = extensions.into_iter().collect();
            insert_stored_extensions(self.conn, key, &extensions, set_ids)?;
        }
        Ok(())
    }

    fn consume(&mut self, record: MameRecord) -> crate::Result<()> {
        match record {
            MameRecord::Machine(machine) => {
                let (set, extensions) = machine_contents(*machine);
                if let SnapshotPublication::Pending(key) = &self.publication {
                    let owner = insert_snapshot_set(self.conn, key, &set)?;
                    self.extensions(extensions, &[owner])?;
                } else {
                    self.extensions(extensions, &[])?;
                }
                Ok(())
            }
            MameRecord::Extension(extension) => {
                self.extensions(std::iter::once(stored_extension(extension)), &[])
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
                    header.build.as_deref(),
                )
                .map_err(StreamingImportError::Storage)?;
                if let SnapshotPublication::Pending(key) = &publication {
                    insert_mame_document_facts(
                        conn,
                        key,
                        header.debug,
                        header.config_version.as_deref(),
                        header.location,
                    )
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
                let mut sink = StreamingImport {
                    conn,
                    publication,
                    run_key,
                };
                sink.extensions(header.extensions.into_iter().map(stored_extension), &[])
                    .map_err(StreamingImportError::Storage)?;
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
    let sink = validated.into_inner();
    if let SnapshotPublication::Pending(key) = &sink.publication {
        merges::persist_snapshot_merges(sink.conn, key)?;
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
    let (record_kind, record_name, source_line, source_column) = match error {
        crate::Error::CatalogParse {
            record_kind,
            record_name,
            line,
            column,
            ..
        } => (record_kind.clone(), record_name.clone(), *line, *column),
        _ => (None, None, None, None),
    };
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
        sql_query(
            "INSERT INTO import_diagnostics \
            (diagnostic_key, run_key, code, message, record_kind, record_name, source_line, source_column) \
             VALUES (?, ?, 'parse_failed', ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
        .bind::<Text, _>(run_key.to_string())
        .bind::<Text, _>(&diagnostic)
        .bind::<Nullable<Text>, _>(record_kind)
        .bind::<Nullable<Text>, _>(record_name)
        .bind::<Nullable<BigInt>, _>(source_line)
        .bind::<Nullable<BigInt>, _>(source_column)
        .execute(conn)?;
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
         VALUES (?, ?, 'mame_coalesce', ?, 'normalization-v1') \
         ON CONFLICT(interpretation_key) DO NOTHING",
    )
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Text, _>(request.format.as_str())
    .bind::<Nullable<Text>, _>(Some(env!("CARGO_PKG_VERSION").to_owned()))
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
    let mut set_ids = Vec::with_capacity(snapshot_data.sets.len());
    for set in &snapshot_data.sets {
        set_ids.push(insert_snapshot_set(conn, snapshot_key, set)?);
    }

    merges::persist_snapshot_merges(conn, snapshot_key)?;

    if let Some(catalog) = &snapshot_data.software_lists {
        software_native::insert(conn, snapshot_key, catalog)?;
    }
    if let Some(facts) = &snapshot_data.logiqx_document_facts {
        insert_logiqx_document_facts(conn, snapshot_key, facts)?;
    }
    if let Some(header) = &snapshot_data.cmp_header_facts {
        cmp_native::insert_header_facts(conn, snapshot_key, header)?;
    }

    insert_stored_extensions(conn, snapshot_key, &snapshot_data.extensions, &set_ids)?;
    Ok(())
}

fn insert_stored_extensions(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    extensions: &[StoredExtension],
    set_ids: &[CatalogSetId],
) -> crate::Result<()> {
    for extension in extensions {
        let owner_set = extension
            .owner_set_index
            .map(|index| {
                set_ids.get(index).copied().ok_or_else(|| {
                    crate::Error::InvalidPath(
                        "catalog extension source set index is out of range".into(),
                    )
                })
            })
            .transpose()?;
        let owner_occurrence_id = match (owner_set, extension.owner_component_index) {
            (Some(set_id), Some(component_index)) => Some(
                sql_query(
                    "SELECT occurrence_id AS value FROM asset_occurrences \
                     WHERE record_id = ? AND occurrence_order = ?",
                )
                .bind::<BigInt, _>(set_id.as_i64())
                .bind::<BigInt, _>(checked_order(
                    component_index,
                    "extension owner components",
                )?)
                .get_result::<ExtensionOccurrenceId>(conn)?
                .value,
            ),
            (None, Some(_)) => {
                return Err(crate::Error::InvalidPath(
                    "catalog extension occurrence has no source set".into(),
                ));
            }
            (_, None) => None,
        };
        insert_snapshot_extension(
            conn,
            snapshot_key,
            extension,
            owner_set,
            owner_occurrence_id,
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
        insert_mame_machine_dependencies(conn, set_id, set)?;
        mame_specification::insert(conn, set_id, set)?;
    }
    if let Some(facts) = &set.no_intro_facts {
        insert_no_intro_game_facts(conn, set_id, facts)?;
    }
    if let Some(facts) = &set.logiqx_facts {
        insert_logiqx_set_facts(conn, set_id, facts)?;
    }
    if let Some(facts) = &set.cmp_facts {
        cmp_native::insert_set_facts(conn, set_id, facts)?;
    }

    persist_set_relationships(conn, snapshot_key, owner, set)?;

    for (order, asset) in set.assets.iter().enumerate() {
        insert_asset_requirement(conn, set_id, checked_order(order, "catalog assets")?, asset)?;
    }
    insert_machine_switches(conn, set_id, set)?;
    insert_machine_bios_sets(conn, set_id, set)?;
    Ok(owner)
}

fn insert_asset_requirement(
    conn: &mut SqliteConnection,
    set_id: i64,
    component_order: i64,
    asset: &SnapshotAsset,
) -> crate::Result<()> {
    let size = asset
        .size
        .map(i64::try_from)
        .transpose()
        .map_err(|_| crate::Error::InvalidRomSize(asset.size.unwrap_or_default()))?;
    let digests = ContentDigestAssertions::new(
        asset.evidence_scope,
        asset.crc.as_deref(),
        asset.md5.as_deref(),
        asset.sha1.as_deref(),
        None,
    );
    // MAME listxml emits complete file size (output_rom/rom_file_size),
    // unlike software-list ROM entries, whose size is one load segment.
    let resolution = if asset.role == "other" {
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
        size,
        content_uuid,
    )?;
    if let Some(facts) = &asset.cmp_facts {
        cmp_native::insert_rom_facts(conn, occurrence.database_value(), facts)?;
    }
    record_occurrence_digest_assertions(conn, occurrence, digests, "source_declared")?;
    record_content_identity_conflict(conn, occurrence, &resolution)?;
    Ok(())
}

fn sqlite_mame_offset(
    attributes: Option<&mame::MameAssetAttributes>,
) -> crate::Result<Option<i64>> {
    attributes
        .and_then(|attributes| attributes.offset)
        .map(|value| i64::try_from(value.0))
        .transpose()
        .map_err(|_| crate::Error::InvalidPath("MAME asset offset exceeds SQLite range".into()))
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
         (snapshot_key, build, debug, file_name, sha1, header_name, header_description, \
          header_version, header_date, header_author, header_email, header_homepage, header_url, \
          header_comment, header_category) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Nullable<Text>, _>(facts.build.as_deref())
    .bind::<Nullable<Text>, _>(facts.debug.as_deref())
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
         (set_id, source_file, is_bios, board, rebuild_to, description, year, manufacturer) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Nullable<Text>, _>(facts.source_file.as_deref())
    .bind::<Nullable<Text>, _>(facts.is_bios.as_deref())
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
    set_id: i64,
    facts: &crate::no_intro_pc_xml::GameFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO no_intro_pc_games \
         (set_id, archive_id, description, description_line, description_column, \
          name_alt, region, version, bios_text, languages_present) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Nullable<Text>, _>(
        facts
            .archive_id
            .as_ref()
            .map(crate::no_intro_pc_xml::ArchiveId::as_str),
    )
    .bind::<Nullable<Text>, _>(facts.description.as_deref())
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
            sql_query("INSERT INTO no_intro_pc_clone_links(set_id,target_archive_id) VALUES (?,?)")
                .bind::<BigInt, _>(set_id)
                .bind::<Text, _>(target.as_str())
                .execute(conn)?;
        }
        None => {}
    }
    if let Some(target) = &facts.merge_of {
        sql_query("INSERT INTO no_intro_pc_merge_links(set_id,target_archive_id) VALUES (?,?)")
            .bind::<BigInt, _>(set_id)
            .bind::<Text, _>(target.as_str())
            .execute(conn)?;
    }
    Ok(())
}

fn insert_mame_machine_dependencies(
    conn: &mut SqliteConnection,
    set_id: i64,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (order, dependency) in set.machine_dependencies.iter().enumerate() {
        sql_query(
            "INSERT INTO mame_machine_dependencies \
             (set_id, dependency_order, dependency_kind, target_name, reference_tag, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(checked_order(order, "MAME machine dependencies")?)
        .bind::<Text, _>(&dependency.source_field)
        .bind::<Text, _>(&dependency.target_name)
        .bind::<Nullable<Text>, _>(dependency.reference_tag.as_deref())
        .bind::<BigInt, _>(dependency.location.line)
        .bind::<BigInt, _>(dependency.location.column)
        .execute(conn)?;
    }
    Ok(())
}

fn insert_mame_machine_facts(
    conn: &mut SqliteConnection,
    set_id: i64,
    facts: &crate::mame::MachineFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO mame_machines \
         (set_id, source_file, description, description_line, description_column, \
          year, year_line, year_column, manufacturer, manufacturer_line, manufacturer_column, \
          is_device, runnable, is_bios, is_mechanical, is_consumable, attributes_line, attributes_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Nullable<Text>, _>(facts.source_file.as_deref())
    .bind::<Text, _>(&facts.description)
    .bind::<BigInt, _>(facts.description_location.line)
    .bind::<BigInt, _>(facts.description_location.column)
    .bind::<Nullable<Text>, _>(facts.year.as_deref())
    .bind::<Nullable<BigInt>, _>(facts.year_location.map(|location| location.line))
    .bind::<Nullable<BigInt>, _>(facts.year_location.map(|location| location.column))
    .bind::<Nullable<Text>, _>(facts.manufacturer.as_deref())
    .bind::<Nullable<BigInt>, _>(
        facts.manufacturer_location.map(|location| location.line),
    )
    .bind::<Nullable<BigInt>, _>(
        facts.manufacturer_location.map(|location| location.column),
    )
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_device())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_runnable())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_bios())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_mechanical())
    .bind::<diesel::sql_types::Bool, _>(facts.flags.is_consumable())
    .bind::<BigInt, _>(facts.attributes_location.line)
    .bind::<BigInt, _>(facts.attributes_location.column)
    .execute(conn)?;
    Ok(())
}

fn insert_mame_document_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    debug: bool,
    config_version: Option<&str>,
    location: crate::logiqx::RecordLocation,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO mame_document_facts \
         (snapshot_key, debug, config_version, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<diesel::sql_types::Bool, _>(debug)
    .bind::<Nullable<Text>, _>(config_version)
    .bind::<BigInt, _>(location.line)
    .bind::<BigInt, _>(location.column)
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
             (set_id, bios_order, name, description, is_default, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(checked_order(bios_order, "MAME BIOS sets")?)
        .bind::<Text, _>(&bios_set.name)
        .bind::<Nullable<Text>, _>(bios_set.description.as_deref())
        .bind::<diesel::sql_types::Bool, _>(bios_set.is_default)
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
        let mask = i64::try_from(switch.mask).map_err(|_| {
            crate::Error::InvalidPath("MAME switch mask exceeds SQLite INTEGER".into())
        })?;
        sql_query(
            "INSERT INTO machine_switches \
             (set_id, switch_order, kind, name, tag, mask, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(switch_order)
        .bind::<Text, _>(switch.kind.as_str())
        .bind::<Text, _>(&switch.name)
        .bind::<Text, _>(&switch.tag)
        .bind::<BigInt, _>(mask)
        .bind::<BigInt, _>(switch.location.line)
        .bind::<BigInt, _>(switch.location.column)
        .execute(conn)?;
        if let Some(condition) = &switch.condition {
            mame_specification::insert_switch_condition(conn, set_id, switch_order, condition)?;
        }

        for (location_order, location) in switch.locations.iter().enumerate() {
            sql_query(
                "INSERT INTO machine_switch_locations \
                 (set_id, switch_order, location_order, name, number, inverted, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(location_order, "machine switch locations")?)
            .bind::<Text, _>(&location.name)
            .bind::<Text, _>(&location.number)
            .bind::<diesel::sql_types::Bool, _>(location.inverted)
            .bind::<BigInt, _>(location.location.line)
            .bind::<BigInt, _>(location.location.column)
            .execute(conn)?;
        }

        for (value_order, switch_value) in switch.values.iter().enumerate() {
            let value = i64::try_from(switch_value.value).map_err(|_| {
                crate::Error::InvalidPath("MAME switch value exceeds SQLite INTEGER".into())
            })?;
            sql_query(
                "INSERT INTO machine_switch_values \
                 (set_id, switch_order, value_order, name, value, is_default, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(value_order, "machine switch values")?)
            .bind::<Text, _>(&switch_value.name)
            .bind::<BigInt, _>(value)
            .bind::<diesel::sql_types::Bool, _>(switch_value.default)
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

fn insert_snapshot_extension(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    extension: &StoredExtension,
    owner_set_id: Option<CatalogSetId>,
    owner_occurrence_id: Option<i64>,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO snapshot_extensions \
         (snapshot_key, record_kind, record_name, owner_set_id, owner_occurrence_id, \
          field_name, namespace_uri, raw_value_json, \
          source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(&extension.record_kind)
    .bind::<Nullable<Text>, _>(&extension.record_name)
    .bind::<Nullable<BigInt>, _>(owner_set_id.map(CatalogSetId::as_i64))
    .bind::<Nullable<BigInt>, _>(owner_occurrence_id)
    .bind::<Text, _>(&extension.field_name)
    .bind::<Nullable<Text>, _>(extension.namespace_uri.as_deref())
    .bind::<Text, _>(extension.value.as_str())
    .bind::<BigInt, _>(extension.location.line)
    .bind::<BigInt, _>(extension.location.column)
    .execute(conn)?;
    Ok(())
}

fn persist_set_relationships(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    owner: CatalogSetId,
    set: &SnapshotSet,
) -> crate::Result<()> {
    let subject = CatalogRecordRef::new(snapshot.clone(), CatalogRecordKind::Set, &set.name)
        .with_owner(owner);
    let location = Some(DocumentLocation {
        line: set.location.line,
        column: set.location.column,
    });
    if let Some(parent) = &set.parent {
        insert_source_assertion(
            conn,
            SourceRelationshipDraft {
                relation_type: RelationshipType::SourceParentClone,
                subject: subject.clone(),
                target: CatalogRecordRef::new(snapshot.clone(), CatalogRecordKind::Set, parent),
                source_field: set.parent_field.as_deref().unwrap_or("parent_name").into(),
                source_location: location,
                evidence: serde_json::json!({"target_name": parent}),
            },
        )?;
    }
    for dependency in &set.runtime_dependencies {
        if set.mame_facts.is_some() {
            continue;
        }
        insert_source_assertion(
            conn,
            SourceRelationshipDraft {
                relation_type: RelationshipType::RuntimeDependency,
                subject: subject.clone(),
                target: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::Set,
                    &dependency.target_name,
                ),
                source_field: dependency.source_field.clone(),
                source_location: Some(DocumentLocation {
                    line: dependency.location.line,
                    column: dependency.location.column,
                }),
                evidence: serde_json::json!({
                    "source_field": dependency.source_field,
                    "target_name": dependency.target_name
                }),
            },
        )?;
    }
    Ok(())
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
