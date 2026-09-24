use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    clrmamepro::Catalog as ClrMameProCatalog,
    domain::{DocumentKey, ImportRunKey, ParserInterpretationKey, SnapshotKey},
    logiqx::{DataFile, XmlSourceMap},
    mame::MameCatalog,
    mame_softwarelist::SoftwareListCatalog,
    no_intro_pc_xml::Catalog as NoIntroCatalog,
    storage::{db::Pool, documents::DocumentStore},
};

#[derive(QueryableByName)]
struct ExistingSnapshot {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
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
    metadata: serde_json::Value,
    location: crate::logiqx::RecordLocation,
    assets: Vec<SnapshotAsset>,
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
    field_name: String,
    namespace_uri: Option<String>,
    value: serde_json::Value,
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
                metadata: serde_json::json!({"source_file": game.sourcefile_opt(), "is_bios": game.isbios_opt(), "rom_of": game.romof_opt(), "sample_of": game.sampleof_opt(), "board": game.board_opt(), "rebuild_to": game.rebuildto_opt(), "description": game.description_opt(), "year": game.year_opt(), "manufacturer": game.manufacturer_opt(), "device_refs": game.device_refs().collect::<Vec<_>>()}),
                location, assets,
            });
        }
        Ok(Self {
            version: data_file.header().version().cloned(),
            sets,
            software_lists: None,
            extensions: source_map
                .unsupported_attributes
                .iter()
                .map(|ext| StoredExtension {
                    record_kind: ext.record_kind.clone(),
                    record_name: ext.record_name.clone(),
                    field_name: ext.field_name.clone(),
                    namespace_uri: ext.namespace_uri.clone(),
                    value: serde_json::json!(ext.value),
                    location: ext.location,
                })
                .collect(),
        })
    }

    fn from_mame(catalog: MameCatalog) -> Self {
        let mut extensions = Vec::new();
        extensions.extend(catalog.extensions.into_iter().map(stored_extension));
        let sets = catalog
            .machines
            .into_iter()
            .map(|machine| {
                extensions.extend(machine.extensions.into_iter().map(stored_extension));
                let assets = machine
                    .assets
                    .into_iter()
                    .map(|asset| {
                        extensions.extend(asset.extensions.into_iter().map(stored_extension));
                        SnapshotAsset {
                            name: asset.name,
                            role: asset.role,
                            size: asset.size,
                            crc: asset.crc,
                            md5: asset.md5,
                            sha1: asset.sha1,
                            evidence_scope: if asset.role == "disk" {
                                "disk_data"
                            } else {
                                "whole_asset"
                            },
                            merge: None,
                            dump_status: None,
                            serial: None,
                            date: None,
                            metadata: serde_json::json!(asset.metadata),
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: machine.name,
                    parent: None,
                    metadata: serde_json::json!(machine.metadata),
                    location: machine.location,
                    assets,
                }
            })
            .collect();
        Self {
            version: catalog.build,
            sets,
            software_lists: None,
            extensions,
        }
    }

    fn from_mame_softwarelist(catalog: SoftwareListCatalog) -> Self {
        let extensions = catalog
            .extensions
            .iter()
            .cloned()
            .map(stored_extension)
            .collect();
        Self {
            version: None,
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
                    metadata: set.metadata,
                    location: set.location,
                    assets,
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
                let assets = entry
                    .assets
                    .into_iter()
                    .map(|asset| {
                        extensions.extend(asset.extensions.into_iter().map(stored_extension));
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
                    name: entry.name,
                    parent: None,
                    metadata: serde_json::json!(entry.metadata),
                    location: entry.location,
                    assets,
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

fn stored_extension(ext: crate::mame::XmlExtension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        field_name: ext.field_name,
        namespace_uri: ext.namespace_uri,
        value: ext.value,
        location: ext.location,
    }
}

fn stored_clrmamepro_extension(ext: crate::clrmamepro::Extension) -> StoredExtension {
    StoredExtension {
        record_kind: ext.record_kind,
        record_name: ext.record_name,
        field_name: ext.field_name,
        namespace_uri: None,
        value: ext.value,
        location: ext.location,
    }
}

pub fn import(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<CatalogImportReport> {
    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone());
    let retained = match request.format {
        CatalogDocumentFormat::Logiqx => {
            documents.retain_path(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameListXml
        | CatalogDocumentFormat::MameSoftwareListXml
        | CatalogDocumentFormat::ClrMamePro
        | CatalogDocumentFormat::NoIntroPcXml => {
            documents.retain_path_raw(request.source_key.clone(), &request.document_path)?
        }
    };
    let bytes = documents.load(&retained.document_key)?;
    let parsed = match request.format {
        CatalogDocumentFormat::Logiqx => DataFile::from_reader_with_source_map(bytes.as_slice())
            .and_then(|(data_file, source_map)| SnapshotData::from_logiqx(&data_file, &source_map)),
        CatalogDocumentFormat::MameListXml => {
            MameCatalog::parse(&bytes).map(SnapshotData::from_mame)
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
    let snapshot_key = SnapshotKey::new(&request.catalog_key, &document_key, &interpretation);
    let run_key = ImportRunKey::fresh();
    let (scope_kind, scope_json) = request.scope.as_storage();
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        ensure_identities(conn, request, &interpretation)?;
        let existing = sql_query(
            "SELECT snapshot_key FROM catalog_snapshots \
             WHERE catalog_key = ? AND document_key = ? AND interpretation_key = ? \
             ORDER BY snapshot_key LIMIT 1",
        )
        .bind::<Text, _>(request.catalog_key.as_str())
        .bind::<Text, _>(document_key.to_string())
        .bind::<Text, _>(interpretation.as_str())
        .get_result::<ExistingSnapshot>(conn)
        .optional()?;
        let snapshot_key = if let Some(existing) = existing {
            SnapshotKey::from_persisted(existing.snapshot_key)
        } else {
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
            .bind::<Nullable<Text>, _>(snapshot_data.version.clone())
            .bind::<Text, _>(scope_kind)
            .bind::<Nullable<Text>, _>(scope_json.clone())
            .execute(conn)?;
            insert_snapshot_contents(conn, &snapshot_key, snapshot_data)?;
            snapshot_key
        };

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
        for extension in &snapshot_data.extensions {
            let raw_value_json = serde_json::to_string(&extension.value)?;
            let code = if extension.field_name.starts_with("element:") {
                "unsupported_element"
            } else {
                "unsupported_attribute"
            };
            sql_query(
                "INSERT INTO import_diagnostics \
                 (diagnostic_key, run_key, code, message, record_kind, record_name, \
                  field_name, raw_value_json, source_line, source_column) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
            .bind::<Text, _>(run_key.to_string())
            .bind::<Text, _>(code)
            .bind::<Text, _>(format!("uninterpreted XML field {}", extension.field_name))
            .bind::<Text, _>(&extension.record_kind)
            .bind::<Nullable<Text>, _>(&extension.record_name)
            .bind::<Nullable<Text>, _>(Some(extension.field_name.clone()))
            .bind::<Nullable<Text>, _>(Some(raw_value_json))
            .bind::<Nullable<BigInt>, _>(Some(extension.location.line))
            .bind::<Nullable<BigInt>, _>(Some(extension.location.column))
            .execute(conn)?;
        }
        Ok(CatalogImportReport {
            snapshot_key: Some(snapshot_key.clone()),
            run_key,
            status: CatalogImportStatus::Succeeded,
            diagnostic_count: snapshot_data.extensions.len(),
        })
    })
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
        sql_query(
            "INSERT INTO snapshot_sets \
             (snapshot_key, set_name, parent_name, metadata_json, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&set.name)
        .bind::<Nullable<Text>, _>(set.parent.clone())
        .bind::<Text, _>(serde_json::to_string(&set.metadata)?)
        .bind::<BigInt, _>(set.location.line)
        .bind::<BigInt, _>(set.location.column)
        .execute(conn)?;

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
            .bind::<Nullable<Binary>, _>(asset.crc.clone())
            .bind::<Nullable<Binary>, _>(asset.md5.clone())
            .bind::<Nullable<Binary>, _>(asset.sha1.clone())
            .bind::<Text, _>(asset.evidence_scope)
            .bind::<Nullable<Text>, _>(asset.merge.clone())
            .bind::<Nullable<Text>, _>(asset.dump_status.clone())
            .bind::<Nullable<Text>, _>(asset.serial.clone())
            .bind::<Nullable<Text>, _>(asset.date.clone())
            .bind::<Text, _>(serde_json::to_string(&asset.metadata)?)
            .bind::<BigInt, _>(asset.location.line)
            .bind::<BigInt, _>(asset.location.column)
            .execute(conn)?;
        }
    }

    if let Some(catalog) = &snapshot_data.software_lists {
        insert_software_list_contents(conn, snapshot_key, catalog)?;
    }

    for extension in &snapshot_data.extensions {
        sql_query(
            "INSERT INTO snapshot_extensions \
             (snapshot_key, record_kind, record_name, field_name, namespace_uri, raw_value_json, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(&extension.record_kind)
        .bind::<Nullable<Text>, _>(&extension.record_name)
        .bind::<Text, _>(&extension.field_name)
        .bind::<Nullable<Text>, _>(extension.namespace_uri.as_deref())
        .bind::<Text, _>(serde_json::to_string(&extension.value)?)
        .bind::<BigInt, _>(extension.location.line)
        .bind::<BigInt, _>(extension.location.column)
        .execute(conn)?;
    }
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
          publisher, notes, info_json, shared_features_json, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(snapshot_key.as_str())
    .bind::<Text, _>(list_name)
    .bind::<Text, _>(item.name.as_str())
    .bind::<BigInt, _>(checked_order(item_order, "software items")?)
    .bind::<Text, _>(item.supported.as_str())
    .bind::<Text, _>(&item.description)
    .bind::<Text, _>(&item.year)
    .bind::<Text, _>(&item.publisher)
    .bind::<Nullable<Text>, _>(item.notes.as_deref())
    .bind::<Text, _>(named_values_json(&item.info))
    .bind::<Text, _>(named_values_json(&item.shared_features))
    .bind::<BigInt, _>(item.location.line)
    .bind::<BigInt, _>(item.location.column)
    .execute(conn)?;

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
         (snapshot_key, list_name, item_name, part_name, part_order, interface, features_json, \
          source_line, source_column) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(item.snapshot_key.as_str())
    .bind::<Text, _>(item.list_name)
    .bind::<Text, _>(item.item_name)
    .bind::<Text, _>(part.name.as_str())
    .bind::<BigInt, _>(checked_order(part_order, "software parts")?)
    .bind::<Text, _>(&part.interface)
    .bind::<Text, _>(named_values_json(&part.features))
    .bind::<BigInt, _>(part.location.line)
    .bind::<BigInt, _>(part.location.column)
    .execute(conn)?;

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
        insert_software_component(conn, part, area, component_order, component)?;
    }
    Ok(())
}

fn insert_software_component(
    conn: &mut SqliteConnection,
    part: SoftwarePartScope<'_>,
    area: &crate::mame_softwarelist::SoftwareArea,
    component_order: usize,
    component: &crate::mame_softwarelist::SoftwareComponent,
) -> crate::Result<()> {
    let (name, size, crc, sha1, offset, value, status, writeable, load) = match component {
        crate::mame_softwarelist::SoftwareComponent::Rom(rom) => (
            rom.name
                .as_ref()
                .map(crate::mame_softwarelist::ComponentName::as_str),
            rom.size,
            rom.crc.map(|digest| digest.to_vec()),
            rom.sha1.map(|digest| digest.to_vec()),
            rom.offset,
            rom.value.as_deref(),
            rom.status.as_ref().map(|status| status.as_str()),
            None,
            rom.load
                .as_ref()
                .map(crate::mame_softwarelist::LoadInstruction::as_str),
        ),
        crate::mame_softwarelist::SoftwareComponent::Disk(disk) => (
            Some(disk.name.as_str()),
            None,
            None,
            disk.sha1.map(|digest| digest.to_vec()),
            None,
            None,
            disk.status.as_ref().map(|status| status.as_str()),
            Some(i64::from(disk.writeable)),
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
         (snapshot_key, list_name, item_name, part_name, area_kind, area_name, component_order, \
          component_kind, component_name, size, crc, sha1, offset, value, dump_status, writeable, \
          load_instruction, source_line, source_column) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(part.item.snapshot_key.as_str())
    .bind::<Text, _>(part.item.list_name)
    .bind::<Text, _>(part.item.item_name)
    .bind::<Text, _>(part.part_name)
    .bind::<Text, _>(area.kind.as_str())
    .bind::<Text, _>(area.name.as_str())
    .bind::<BigInt, _>(checked_order(component_order, "software components")?)
    .bind::<Text, _>(component.kind())
    .bind::<Nullable<Text>, _>(name)
    .bind::<Nullable<BigInt>, _>(size)
    .bind::<Nullable<Binary>, _>(crc)
    .bind::<Nullable<Binary>, _>(sha1)
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

fn named_values_json(values: &[crate::mame_softwarelist::NamedValue]) -> String {
    serde_json::json!(
        values
            .iter()
            .map(|value| serde_json::json!({
                "name": value.name,
                "value": value.value,
                "source_line": value.location.line,
                "source_column": value.location.column,
            }))
            .collect::<Vec<_>>()
    )
    .to_string()
}

fn checked_order(order: usize, kind: &str) -> crate::Result<i64> {
    i64::try_from(order).map_err(|_| crate::Error::InvalidPath(format!("too many {kind}")))
}
