use std::collections::HashMap;

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    clrmamepro::Catalog as ClrMameProCatalog,
    domain::{
        CatalogRecordKind, CatalogRecordRef, DocumentKey, DocumentLocation, ImportRunKey,
        ParserInterpretationKey, RelationshipType, SnapshotKey,
    },
    logiqx::{DataFile, XmlSourceMap},
    mame::{self, ExtensionValue, MameRecord, ValidatedMame},
    mame_softwarelist::SoftwareListCatalog,
    no_intro_pc_xml::Catalog as NoIntroCatalog,
    storage::{
        db::Pool,
        documents::DocumentStore,
        relationships::{SourceRelationshipDraft, insert_source_assertion},
    },
};

mod merges;

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
    #[diesel(sql_type = Text)]
    scope_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    scope_json: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    acquisition_source: Option<String>,
}

#[derive(QueryableByName)]
struct ComponentOrderRow {
    #[diesel(sql_type = BigInt)]
    component_order: i64,
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
    extensions: Vec<StoredExtension>,
}

struct SnapshotSet {
    name: String,
    parent: Option<String>,
    parent_field: Option<String>,
    runtime_dependencies: Vec<SnapshotDependency>,
    metadata: serde_json::Value,
    location: crate::logiqx::RecordLocation,
    assets: Vec<SnapshotAsset>,
    switches: Vec<crate::mame::MachineSwitch>,
    bios_sets: Vec<crate::mame::MachineBiosSet>,
    mame_facts: Option<crate::mame::MachineFacts>,
    machine_dependencies: Vec<SnapshotDependency>,
}

struct SnapshotDependency {
    source_field: String,
    target_name: String,
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
    metadata: serde_json::Value,
    location: crate::logiqx::RecordLocation,
}

struct StoredExtension {
    record_kind: String,
    record_name: Option<String>,
    owner_set_name: Option<String>,
    owner_component_order: Option<String>,
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
                let expected = crate::domain::ExpectedEvidence::from_logiqx(rom)?;
                assets.push(SnapshotAsset {
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
                    metadata: serde_json::Value::Null,
                    location: *rom_locations.get(index).ok_or_else(|| {
                        crate::Error::InvalidPath("Logiqx asset source location is missing".into())
                    })?,
                });
            }
            sets.push(SnapshotSet {
                name: game.name().into(), parent: game.cloneof().map(str::to_owned),
                parent_field: game.cloneof().map(|_| "cloneof".to_owned()),
                runtime_dependencies: game
                    .romof_opt()
                    .map(|name| SnapshotDependency {
                        source_field: "romof".to_owned(),
                        target_name: name.to_owned(),
                        location,
                    })
                    .into_iter()
                    .chain(game.sampleof_opt().map(|name| SnapshotDependency {
                        source_field: "sampleof".to_owned(),
                        target_name: name.to_owned(),
                        location,
                    }))
                    .chain(
                    game.device_refs()
                        .zip(device_ref_locations)
                        .map(|(name, location)| SnapshotDependency {
                            source_field: "device_ref".to_owned(),
                            target_name: name.to_owned(),
                            location: *location,
                        }),
                    )
                    .collect(),
                metadata: serde_json::json!({"source_file": game.sourcefile_opt(), "is_bios": game.isbios_opt(), "rom_of": game.romof_opt(), "sample_of": game.sampleof_opt(), "board": game.board_opt(), "rebuild_to": game.rebuildto_opt(), "description": game.description_opt(), "year": game.year_opt(), "manufacturer": game.manufacturer_opt(), "device_refs": game.device_refs().collect::<Vec<_>>()}),
                location, assets,
                switches: Vec::new(),
                bios_sets: Vec::new(),
                mame_facts: None,
                machine_dependencies: Vec::new(),
            });
        }
        Ok(Self {
            version: data_file.header().version().cloned(),
            sets,
            software_lists: None,
            extensions: stored_logiqx_extensions(data_file, source_map),
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
            .map(|set| {
                extensions.extend(set.extensions.into_iter().map(stored_clrmamepro_extension));
                let parent_field = set.parent.as_ref().map(|_| "cloneof".to_owned());
                let assets = set
                    .assets
                    .into_iter()
                    .map(|asset| {
                        extensions.extend(
                            asset
                                .extensions
                                .into_iter()
                                .map(stored_clrmamepro_extension),
                        );
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
                            metadata: asset.metadata,
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: set.name,
                    parent: set.parent,
                    parent_field,
                    runtime_dependencies: Vec::new(),
                    metadata: set.metadata,
                    location: set.location,
                    assets,
                    switches: Vec::new(),
                    bios_sets: Vec::new(),
                    mame_facts: None,
                    machine_dependencies: Vec::new(),
                }
            })
            .collect();
        Self {
            version: catalog.version,
            sets,
            software_lists: None,
            extensions,
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
            .map(|entry| {
                extensions.extend(entry.extensions.into_iter().map(stored_extension));
                let entry_name = entry.name;
                let assets = entry
                    .assets
                    .into_iter()
                    .enumerate()
                    .map(|(component_order, asset)| {
                        extensions.extend(asset.extensions.into_iter().map(|extension| {
                            let mut extension = stored_extension(extension);
                            extension.owner_set_name = Some(entry_name.clone());
                            extension.owner_component_order = Some(component_order.to_string());
                            extension
                        }));
                        SnapshotAsset {
                            name: asset.name,
                            role: "rom",
                            size: asset.size,
                            crc: asset.crc,
                            md5: None,
                            sha1: asset.sha1,
                            evidence_scope: "whole_asset",
                            merge: None,
                            dump_status: None,
                            serial: None,
                            date: None,
                            metadata: serde_json::Value::Null,
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: entry_name,
                    parent: None,
                    parent_field: None,
                    runtime_dependencies: Vec::new(),
                    metadata: serde_json::json!(entry.metadata),
                    location: entry.location,
                    assets,
                    switches: Vec::new(),
                    bios_sets: Vec::new(),
                    mame_facts: None,
                    machine_dependencies: Vec::new(),
                }
            })
            .collect();
        Self {
            version: catalog.version,
            sets,
            software_lists: None,
            extensions,
        }
    }
}

fn machine_contents(machine: crate::mame::Machine) -> (SnapshotSet, Vec<StoredExtension>) {
    let mut extensions = Vec::new();
    let machine_name = &machine.name;
    extensions.extend(stored_machine_extensions(machine_name, machine.extensions));
    let runtime_dependencies: Vec<SnapshotDependency> = machine
        .device_refs
        .into_iter()
        .map(|reference| SnapshotDependency {
            source_field: "device_ref".to_owned(),
            target_name: reference.name,
            location: reference.location,
        })
        .chain(machine.rom_of.into_iter().map(|name| SnapshotDependency {
            source_field: "romof".to_owned(),
            target_name: name,
            location: machine.location,
        }))
        .chain(
            machine
                .sample_of
                .into_iter()
                .map(|name| SnapshotDependency {
                    source_field: "sampleof".to_owned(),
                    target_name: name,
                    location: machine.location,
                }),
        )
        .collect();
    let assets = machine_assets(machine_name, machine.assets, &mut extensions);
    let parent_field = machine.parent.as_ref().map(|_| "cloneof".to_owned());
    let switches = machine.switches;
    let bios_sets = machine.bios_sets;
    let mame_facts = machine.facts;
    let machine_dependencies = runtime_dependencies
        .iter()
        .map(|dependency| SnapshotDependency {
            source_field: dependency.source_field.clone(),
            target_name: dependency.target_name.clone(),
            location: dependency.location,
        })
        .collect();
    let set = SnapshotSet {
        name: machine.name,
        parent: machine.parent,
        parent_field,
        runtime_dependencies,
        metadata: serde_json::Value::Null,
        location: machine.location,
        assets,
        switches,
        bios_sets,
        machine_dependencies,
        mame_facts: Some(mame_facts),
    };
    (set, extensions)
}

fn machine_assets(
    machine_name: &str,
    assets: Vec<crate::mame::MachineAsset>,
    extensions: &mut Vec<StoredExtension>,
) -> Vec<SnapshotAsset> {
    assets
        .into_iter()
        .enumerate()
        .map(|(component_order, asset)| {
            extensions.extend(asset.extensions.into_iter().map(|extension| {
                let mut extension = stored_extension(extension);
                extension.owner_set_name = Some(machine_name.to_owned());
                extension.owner_component_order = Some(component_order.to_string());
                extension
            }));
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
                .metadata
                .get("merge")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    asset
                        .disk_requirement
                        .as_ref()
                        .and_then(crate::disk::DiskRequirement::parent)
                        .map(|name| name.as_str().to_owned())
                });
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
                merge: merge.or(asset.merge_name),
                dump_status: asset.dump_status,
                serial: None,
                date: None,
                metadata: serde_json::Value::Object(asset.metadata.into_iter().collect()),
                location: asset.location,
            }
        })
        .collect()
}

fn stored_extension(ext: crate::mame::XmlExtension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        owner_set_name: None,
        owner_component_order: None,
        field_name: ext.field_name,
        namespace_uri: ext.namespace_uri,
        value: ext.value,
        location: ext.location,
    }
}

fn stored_logiqx_extensions(
    data_file: &DataFile,
    source_map: &XmlSourceMap,
) -> Vec<StoredExtension> {
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
                owner_set_name: None,
                owner_component_order: None,
                field_name: ext.field_name.clone(),
                namespace_uri: ext.namespace_uri.clone(),
                value: serde_json::json!(ext.value).into(),
                location: ext.location,
            };
            if ext.record_kind == "rom" {
                let owner = rom_owners
                    .get(&(ext.location.line, ext.location.column))
                    .copied();
                if let Some((game_index, component_order)) = owner
                    && let Some(game) = data_file.games().get(game_index)
                {
                    stored.owner_set_name = Some(game.name().to_owned());
                    stored.owner_component_order = Some(component_order.to_string());
                }
            } else if ext.record_kind != "game" && ext.record_kind != "document" {
                let owner_index = source_map
                    .game_locations
                    .partition_point(|location| {
                        (location.line, location.column) <= (ext.location.line, ext.location.column)
                    })
                    .checked_sub(1);
                if let Some(game) = owner_index.and_then(|index| data_file.games().get(index)) {
                    stored.owner_set_name = Some(game.name().to_owned());
                }
            }
            stored
        })
        .collect()
}

fn stored_machine_extensions(
    machine_name: &str,
    extensions: Vec<crate::mame::XmlExtension>,
) -> impl Iterator<Item = StoredExtension> + '_ {
    extensions.into_iter().map(move |extension| {
        let mut extension = stored_extension(extension);
        if extension.record_kind == "device_ref" {
            extension.owner_set_name = Some(machine_name.to_owned());
        }
        extension
    })
}

fn stored_clrmamepro_extension(ext: crate::clrmamepro::Extension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        owner_set_name: None,
        owner_component_order: None,
        field_name: ext.field_name,
        namespace_uri: None,
        value: ext.value.into(),
        location: ext.location,
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
    ) -> crate::Result<()> {
        for extension in extensions {
            if let SnapshotPublication::Pending(key) = &self.publication {
                insert_snapshot_extension(self.conn, key, &extension)?;
            }
        }
        Ok(())
    }

    fn consume(&mut self, record: MameRecord) -> crate::Result<()> {
        match record {
            MameRecord::Machine(machine) => {
                let (set, extensions) = machine_contents(*machine);
                if let SnapshotPublication::Pending(key) = &self.publication {
                    insert_snapshot_set(self.conn, key, &set)?;
                }
                self.extensions(extensions)
            }
            MameRecord::Extension(extension) => {
                self.extensions(std::iter::once(stored_extension(extension)))
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
                sink.extensions(header.extensions.into_iter().map(stored_extension))
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

    let (scope_kind, scope_json) = request.scope.as_storage();
    let identity_rows = sql_query(
        "SELECT snapshot.snapshot_key, snapshot.declared_version, snapshot.scope_kind, \
                snapshot.scope_json, acquisition.source_key AS acquisition_source \
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
            && row.scope_kind == scope_kind
            && row.scope_json.as_deref() == scope_json.as_deref()
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
              declared_version, scope_kind, scope_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(document_key.to_string())
        .bind::<Text, _>(interpretation.as_str())
        .bind::<Nullable<Text>, _>(Some(acquisition_key.to_owned()))
        .bind::<Nullable<Text>, _>(version)
        .bind::<Text, _>(scope_kind)
        .bind::<Nullable<Text>, _>(scope_json)
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

    let (scope_kind, scope_details) = request.scope.as_storage();
    let options_json = serde_json::json!({
        "snapshot_scope": scope_kind,
        "scope_details": scope_details,
    })
    .to_string();
    sql_query(
        "INSERT INTO parser_interpretations \
         (interpretation_key, format, parser_name, parser_version, rules_version, options_json) \
         VALUES (?, ?, 'mame_coalesce', ?, 'normalization-v1', ?) \
         ON CONFLICT(interpretation_key) DO NOTHING",
    )
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Text, _>(request.format.as_str())
    .bind::<Nullable<Text>, _>(Some(env!("CARGO_PKG_VERSION").to_owned()))
    .bind::<Nullable<Text>, _>(Some(options_json))
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
    for set in &snapshot_data.sets {
        insert_snapshot_set(conn, snapshot_key, set)?;
    }

    for set in &snapshot_data.sets {
        for (component_order, asset) in set.assets.iter().enumerate() {
            let component_order = i64::try_from(component_order)
                .map_err(|_| crate::Error::InvalidPath("too many catalog assets".into()))?;
            persist_asset_merge_relationship(conn, snapshot_key, set, asset, component_order)?;
        }
    }

    if let Some(catalog) = &snapshot_data.software_lists {
        insert_software_list_contents(conn, snapshot_key, catalog)?;
    }

    for extension in &snapshot_data.extensions {
        insert_snapshot_extension(conn, snapshot_key, extension)?;
    }
    Ok(())
}

fn insert_snapshot_set(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO snapshot_sets \
         (snapshot_key, set_name, parent_name, metadata_json, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(&set.name)
    .bind::<Nullable<Text>, _>(set.parent.as_deref())
    .bind::<Text, _>(serde_json::to_string(&set.metadata)?)
    .bind::<BigInt, _>(set.location.line)
    .bind::<BigInt, _>(set.location.column)
    .execute(conn)?;

    if let Some(facts) = &set.mame_facts {
        insert_mame_machine_facts(conn, snapshot_key, set, facts)?;
        insert_mame_machine_dependencies(conn, snapshot_key, set)?;
    }

    persist_set_relationships(conn, snapshot_key, set)?;

    for (order, asset) in set.assets.iter().enumerate() {
        let component_order = i64::try_from(order)
            .map_err(|_| crate::Error::InvalidPath("too many catalog assets".into()))?;
        let size = asset
            .size
            .map(i64::try_from)
            .transpose()
            .map_err(|_| crate::Error::InvalidRomSize(asset.size.unwrap_or_default()))?;
        sql_query(
            "INSERT INTO asset_requirements \
             (snapshot_key, set_name, component_order, asset_name, role, size, crc, md5, sha1, \
             evidence_scope, evidence_provenance, merge_name, dump_status, serial, date, metadata_json, \
              source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'source_declared', ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&set.name)
        .bind::<BigInt, _>(component_order)
        .bind::<Text, _>(&asset.name)
        .bind::<Text, _>(asset.role)
        .bind::<Nullable<BigInt>, _>(size)
        .bind::<Nullable<Binary>, _>(asset.crc.as_deref())
        .bind::<Nullable<Binary>, _>(asset.md5.as_deref())
        .bind::<Nullable<Binary>, _>(asset.sha1.as_deref())
        .bind::<Text, _>(asset.evidence_scope)
        .bind::<Nullable<Text>, _>(asset.merge.as_deref())
        .bind::<Nullable<Text>, _>(asset.dump_status.as_deref())
        .bind::<Nullable<Text>, _>(asset.serial.as_deref())
        .bind::<Nullable<Text>, _>(asset.date.as_deref())
        .bind::<Text, _>(serde_json::to_string(&asset.metadata)?)
        .bind::<BigInt, _>(asset.location.line)
        .bind::<BigInt, _>(asset.location.column)
        .execute(conn)?;
    }
    insert_machine_switches(conn, snapshot_key, set)?;
    insert_machine_bios_sets(conn, snapshot_key, set)?;
    Ok(())
}

fn insert_mame_machine_dependencies(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (order, dependency) in set.machine_dependencies.iter().enumerate() {
        sql_query(
            "INSERT INTO mame_machine_dependencies \
             (snapshot_key, set_name, dependency_order, dependency_kind, target_name, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&set.name)
        .bind::<BigInt, _>(checked_order(order, "MAME machine dependencies")?)
        .bind::<Text, _>(&dependency.source_field)
        .bind::<Text, _>(&dependency.target_name)
        .bind::<BigInt, _>(dependency.location.line)
        .bind::<BigInt, _>(dependency.location.column)
        .execute(conn)?;
    }
    Ok(())
}

fn insert_mame_machine_facts(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
    facts: &crate::mame::MachineFacts,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO mame_machine_facts \
         (snapshot_key, set_name, source_file, description, description_line, description_column, \
          year, year_line, year_column, manufacturer, manufacturer_line, manufacturer_column, \
          is_device, runnable, is_bios, is_mechanical, is_consumable, attributes_line, attributes_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(&set.name)
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
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (bios_order, bios_set) in set.bios_sets.iter().enumerate() {
        sql_query(
            "INSERT INTO machine_bios_sets \
             (snapshot_key, set_name, bios_order, name, description, is_default, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&set.name)
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
    snapshot_key: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<()> {
    for (switch_order, switch) in set.switches.iter().enumerate() {
        let switch_order = checked_order(switch_order, "machine switches")?;
        let mask = i64::try_from(switch.mask).map_err(|_| {
            crate::Error::InvalidPath("MAME switch mask exceeds SQLite INTEGER".into())
        })?;
        sql_query(
            "INSERT INTO machine_switches \
             (snapshot_key, set_name, switch_order, kind, name, tag, mask, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&set.name)
        .bind::<BigInt, _>(switch_order)
        .bind::<Text, _>(switch.kind.as_str())
        .bind::<Text, _>(&switch.name)
        .bind::<Text, _>(&switch.tag)
        .bind::<BigInt, _>(mask)
        .bind::<BigInt, _>(switch.location.line)
        .bind::<BigInt, _>(switch.location.column)
        .execute(conn)?;

        for (location_order, location) in switch.locations.iter().enumerate() {
            sql_query(
                "INSERT INTO machine_switch_locations \
                 (snapshot_key, set_name, switch_order, location_order, name, number, inverted, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<Text, _>(&set.name)
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
                 (snapshot_key, set_name, switch_order, value_order, name, value, is_default, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<Text, _>(&set.name)
            .bind::<BigInt, _>(switch_order)
            .bind::<BigInt, _>(checked_order(value_order, "machine switch values")?)
            .bind::<Text, _>(&switch_value.name)
            .bind::<BigInt, _>(value)
            .bind::<diesel::sql_types::Bool, _>(switch_value.default)
            .bind::<BigInt, _>(switch_value.location.line)
            .bind::<BigInt, _>(switch_value.location.column)
            .execute(conn)?;
        }
    }
    Ok(())
}

fn insert_snapshot_extension(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    extension: &StoredExtension,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO snapshot_extensions \
         (snapshot_key, record_kind, record_name, owner_set_name, owner_component_order, field_name, namespace_uri, raw_value_json, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(&extension.record_kind)
    .bind::<Nullable<Text>, _>(&extension.record_name)
    .bind::<Nullable<Text>, _>(&extension.owner_set_name)
    .bind::<Nullable<Text>, _>(&extension.owner_component_order)
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
    set: &SnapshotSet,
) -> crate::Result<()> {
    let subject = CatalogRecordRef::new(snapshot.clone(), CatalogRecordKind::Set, &set.name);
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

fn persist_asset_merge_relationship(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    set: &SnapshotSet,
    asset: &SnapshotAsset,
    component_order: i64,
) -> crate::Result<()> {
    let merge_set = set
        .runtime_dependencies
        .iter()
        .find(|dependency| dependency.source_field == "romof")
        .map(|dependency| dependency.target_name.as_str())
        .or(set.parent.as_deref());
    let (Some(parent), Some(merged_name)) = (merge_set, asset.merge.as_deref()) else {
        return Ok(());
    };
    merges::persist_merge_relationship(
        conn,
        snapshot,
        merges::MergeDeclaration {
            set_name: &set.name,
            component_order,
            asset_name: &asset.name,
            merged_name,
            parent,
            role: asset.role,
            sha1: asset.sha1.as_deref(),
            crc: asset.crc.as_deref(),
            size: asset.size,
            location: asset.location,
        },
    )?;
    Ok(())
}

fn insert_software_list_contents(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    catalog: &SoftwareListCatalog,
) -> crate::Result<()> {
    for (list_order, list) in catalog.lists.iter().enumerate() {
        insert_software_list(conn, snapshot_key, list, list_order)?;
    }
    Ok(())
}

fn insert_software_list(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    list: &crate::mame_softwarelist::SoftwareList,
    list_order: usize,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_lists \
         (snapshot_key, list_name, list_order, description, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(list.name.as_str())
    .bind::<BigInt, _>(checked_order(list_order, "software lists")?)
    .bind::<Nullable<Text>, _>(list.description.as_deref())
    .bind::<BigInt, _>(list.location.line)
    .bind::<BigInt, _>(list.location.column)
    .execute(conn)?;

    for (item_order, item) in list.items.iter().enumerate() {
        insert_software_item(conn, snapshot_key, list.name.as_str(), item, item_order)?;
    }
    Ok(())
}

fn insert_software_item(
    conn: &mut SqliteConnection,
    snapshot_key: &SnapshotKey,
    list_name: &str,
    item: &crate::mame_softwarelist::SoftwareItem,
    item_order: usize,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_items \
         (snapshot_key, list_name, item_name, item_order, supported, description, year, \
          publisher, notes, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(list_name)
    .bind::<Text, _>(item.name.as_str())
    .bind::<BigInt, _>(checked_order(item_order, "software items")?)
    .bind::<Nullable<Text>, _>(
        item.supported
            .map(crate::mame_softwarelist::SupportedStatus::as_str),
    )
    .bind::<Text, _>(&item.description)
    .bind::<Text, _>(&item.year)
    .bind::<Text, _>(&item.publisher)
    .bind::<Nullable<Text>, _>(item.notes.as_deref())
    .bind::<BigInt, _>(item.location.line)
    .bind::<BigInt, _>(item.location.column)
    .execute(conn)?;

    insert_software_item_named_values(
        conn,
        "software_item_info",
        snapshot_key,
        list_name,
        item.name.as_str(),
        &item.info,
    )?;
    insert_software_item_named_values(
        conn,
        "software_item_shared_features",
        snapshot_key,
        list_name,
        item.name.as_str(),
        &item.shared_features,
    )?;

    if let Some(parent) = &item.clone_of {
        sql_query(
            "INSERT INTO software_item_dependencies \
             (snapshot_key, list_name, item_name, dependency_kind, target_item_name, \
              source_line, source_column) VALUES (?, ?, ?, 'clone_of', ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(list_name)
        .bind::<Text, _>(item.name.as_str())
        .bind::<Text, _>(parent.as_str())
        .bind::<BigInt, _>(item.location.line)
        .bind::<BigInt, _>(item.location.column)
        .execute(conn)?;
        insert_source_assertion(
            conn,
            SourceRelationshipDraft {
                relation_type: RelationshipType::SourceParentClone,
                subject: CatalogRecordRef::new(
                    snapshot_key.clone(),
                    CatalogRecordKind::SoftwareItem,
                    super::catalog_reconciliation::record_key(&(list_name, item.name.as_str()))?,
                ),
                target: CatalogRecordRef::new(
                    snapshot_key.clone(),
                    CatalogRecordKind::SoftwareItem,
                    super::catalog_reconciliation::record_key(&(list_name, parent.as_str()))?,
                ),
                source_field: "cloneof".to_owned(),
                source_location: Some(DocumentLocation {
                    line: item.location.line,
                    column: item.location.column,
                }),
                evidence: serde_json::json!({
                    "list_name": list_name,
                    "target_item_name": parent.as_str()
                }),
            },
        )?;
    }

    let scope = SoftwareItemScope {
        snapshot_key,
        list_name,
        item_name: item.name.as_str(),
    };
    for (part_order, part) in item.parts.iter().enumerate() {
        insert_software_part(conn, scope, part, part_order)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct SoftwareItemScope<'a> {
    snapshot_key: &'a SnapshotKey,
    list_name: &'a str,
    item_name: &'a str,
}

#[derive(Clone, Copy)]
struct SoftwarePartScope<'a> {
    item: SoftwareItemScope<'a>,
    part_name: &'a str,
}

fn insert_software_part(
    conn: &mut SqliteConnection,
    item: SoftwareItemScope<'_>,
    part: &crate::mame_softwarelist::SoftwarePart,
    part_order: usize,
) -> crate::Result<()> {
    sql_query(
        "INSERT INTO software_parts \
         (snapshot_key, list_name, item_name, part_name, part_order, interface, \
          source_line, source_column) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(item.snapshot_key.as_str())
    .bind::<Text, _>(item.list_name)
    .bind::<Text, _>(item.item_name)
    .bind::<Text, _>(part.name.as_str())
    .bind::<BigInt, _>(checked_order(part_order, "software parts")?)
    .bind::<Text, _>(&part.interface)
    .bind::<BigInt, _>(part.location.line)
    .bind::<BigInt, _>(part.location.column)
    .execute(conn)?;

    for (value_order, value) in part.features.iter().enumerate() {
        sql_query(
            "INSERT INTO software_part_features \
             (snapshot_key, list_name, item_name, part_name, value_order, name, value, \
              source_line, source_column) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(item.snapshot_key.as_str())
        .bind::<Text, _>(item.list_name)
        .bind::<Text, _>(item.item_name)
        .bind::<Text, _>(part.name.as_str())
        .bind::<BigInt, _>(checked_order(value_order, "software part features")?)
        .bind::<Text, _>(&value.name)
        .bind::<Nullable<Text>, _>(value.value.as_deref())
        .bind::<BigInt, _>(value.location.line)
        .bind::<BigInt, _>(value.location.column)
        .execute(conn)?;
    }

    let scope = SoftwarePartScope {
        item,
        part_name: part.name.as_str(),
    };
    for (area_order, area) in part.areas.iter().enumerate() {
        insert_software_area(conn, scope, area, area_order)?;
    }
    Ok(())
}

fn insert_software_area(
    conn: &mut SqliteConnection,
    part: SoftwarePartScope<'_>,
    area: &crate::mame_softwarelist::SoftwareArea,
    area_order: usize,
) -> crate::Result<()> {
    let declared_size = area
        .declared_size
        .map(i64::try_from)
        .transpose()
        .map_err(|_| crate::Error::InvalidRomSize(area.declared_size.unwrap_or_default()))?;
    sql_query(
        "INSERT INTO software_areas \
         (snapshot_key, list_name, item_name, part_name, area_name, area_kind, area_order, \
          declared_size, width, endianness, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(part.item.snapshot_key.as_str())
    .bind::<Text, _>(part.item.list_name)
    .bind::<Text, _>(part.item.item_name)
    .bind::<Text, _>(part.part_name)
    .bind::<Text, _>(area.name.as_str())
    .bind::<Text, _>(area.kind.as_str())
    .bind::<BigInt, _>(checked_order(area_order, "software areas")?)
    .bind::<Nullable<BigInt>, _>(declared_size)
    .bind::<Nullable<BigInt>, _>(area.width.map(i64::from))
    .bind::<Nullable<Text>, _>(
        area.endianness
            .map(crate::mame_softwarelist::Endianness::as_str),
    )
    .bind::<BigInt, _>(area.location.line)
    .bind::<BigInt, _>(area.location.column)
    .execute(conn)?;

    for (component_order, component) in area.components.iter().enumerate() {
        insert_software_component(conn, part, area, area_order, component_order, component)?;
    }
    Ok(())
}

fn insert_software_component(
    conn: &mut SqliteConnection,
    part: SoftwarePartScope<'_>,
    area: &crate::mame_softwarelist::SoftwareArea,
    area_order: usize,
    component_order: usize,
    component: &crate::mame_softwarelist::SoftwareComponent,
) -> crate::Result<()> {
    let (name, size, crc, sha1, evidence_scope, offset, value, status, writeable, load) =
        match component {
            crate::mame_softwarelist::SoftwareComponent::Rom(rom) => (
                rom.name
                    .as_ref()
                    .map(crate::mame_softwarelist::ComponentName::as_str),
                rom.size,
                rom.crc.map(|digest| digest.to_vec()),
                rom.sha1.map(|digest| digest.to_vec()),
                "whole_asset",
                rom.offset,
                rom.value.as_deref(),
                rom.status.as_ref().map(|status| status.as_str()),
                None,
                rom.load
                    .as_ref()
                    .map(crate::mame_softwarelist::LoadInstruction::as_str),
            ),
            crate::mame_softwarelist::SoftwareComponent::Disk(disk) => (
                Some(disk.requirement.name().as_str()),
                None,
                None,
                disk.requirement
                    .expected_sha1()
                    .map(|digest| digest.as_bytes().to_vec()),
                disk.requirement.digest_scope().as_str(),
                None,
                None,
                disk.status.as_ref().map(|status| status.as_str()),
                disk.writeable.map(i64::from),
                None,
            ),
        };
    let size = size
        .map(i64::try_from)
        .transpose()
        .map_err(|_| crate::Error::InvalidRomSize(size.unwrap_or_default()))?;
    let offset = offset
        .map(i64::try_from)
        .transpose()
        .map_err(|_| crate::Error::InvalidRomSize(offset.unwrap_or_default()))?;
    sql_query(
        "INSERT INTO software_components \
         (snapshot_key, list_name, item_name, part_name, area_order, area_kind, area_name, component_order, \
          component_kind, component_name, size, crc, sha1, evidence_scope, offset, value, dump_status, writeable, \
          load_instruction, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(part.item.snapshot_key.as_str())
    .bind::<Text, _>(part.item.list_name)
    .bind::<Text, _>(part.item.item_name)
    .bind::<Text, _>(part.part_name)
    .bind::<BigInt, _>(checked_order(area_order, "software areas")?)
    .bind::<Text, _>(area.kind.as_str())
    .bind::<Text, _>(area.name.as_str())
    .bind::<BigInt, _>(checked_order(component_order, "software components")?)
    .bind::<Text, _>(component.kind())
    .bind::<Nullable<Text>, _>(name)
    .bind::<Nullable<BigInt>, _>(size)
    .bind::<Nullable<Binary>, _>(crc)
    .bind::<Nullable<Binary>, _>(sha1)
    .bind::<Text, _>(evidence_scope)
    .bind::<Nullable<BigInt>, _>(offset)
    .bind::<Nullable<Text>, _>(value)
    .bind::<Nullable<Text>, _>(status)
    .bind::<Nullable<BigInt>, _>(writeable)
    .bind::<Nullable<Text>, _>(load)
    .bind::<BigInt, _>(component.location().line)
    .bind::<BigInt, _>(component.location().column)
    .execute(conn)?;
    Ok(())
}

fn insert_software_item_named_values(
    conn: &mut SqliteConnection,
    table: &str,
    snapshot_key: &SnapshotKey,
    list_name: &str,
    item_name: &str,
    values: &[crate::mame_softwarelist::NamedValue],
) -> crate::Result<()> {
    for (value_order, value) in values.iter().enumerate() {
        sql_query(format!(
            "INSERT INTO {table} \
             (snapshot_key, list_name, item_name, value_order, name, value, source_line, \
              source_column) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        ))
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(list_name)
        .bind::<Text, _>(item_name)
        .bind::<BigInt, _>(checked_order(value_order, "software item values")?)
        .bind::<Text, _>(&value.name)
        .bind::<Nullable<Text>, _>(value.value.as_deref())
        .bind::<BigInt, _>(value.location.line)
        .bind::<BigInt, _>(value.location.column)
        .execute(conn)?;
    }
    Ok(())
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
