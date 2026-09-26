use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    domain::{DocumentKey, ImportRunKey, ParserInterpretationKey, SnapshotKey},
    logiqx::{DataFile, XmlSourceMap},
    mame::MameCatalog,
    storage::{db::Pool, documents::DocumentStore},
};

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
struct IdentityRow {
    #[diesel(sql_type = Text)]
    first_value: String,
    #[diesel(sql_type = Nullable<Text>)]
    second_value: Option<String>,
}

struct SnapshotData {
    version: Option<String>,
    sets: Vec<SnapshotSet>,
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
                            merge: asset.merge_name,
                            dump_status: asset.dump_status,
                            serial: None,
                            date: None,
                            metadata: serde_json::json!(asset.metadata),
                            location: asset.location,
                        }
                    })
                    .collect();
                SnapshotSet {
                    name: machine.name,
                    parent: machine.parent,
                    metadata: serde_json::json!(machine.metadata),
                    location: machine.location,
                    assets,
                }
            })
            .collect();
        Self {
            version: catalog.build,
            sets,
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

pub fn import(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<CatalogImportReport> {
    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone());
    let retained = match request.format {
        CatalogDocumentFormat::Logiqx => {
            documents.retain_path(request.source_key.clone(), &request.document_path)?
        }
        CatalogDocumentFormat::MameListXml => {
            documents.retain_path_mame(request.source_key.clone(), &request.document_path)?
        }
    };
    let bytes = documents.load(&retained.document_key)?;
    let parsed = match request.format {
        CatalogDocumentFormat::Logiqx => DataFile::from_reader_with_source_map(bytes.as_slice())
            .and_then(|(data_file, source_map)| SnapshotData::from_logiqx(&data_file, &source_map)),
        CatalogDocumentFormat::MameListXml => {
            MameCatalog::parse(&bytes).map(SnapshotData::from_mame)
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
             (diagnostic_key, run_key, code, message) VALUES (?, ?, 'parse_failed', ?)",
        )
        .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
        .bind::<Text, _>(run_key.to_string())
        .bind::<Text, _>(&diagnostic)
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

fn ensure_snapshot_publication(
    conn: &mut SqliteConnection,
    request: &CatalogImportRequest,
    document_key: &DocumentKey,
    acquisition_key: &str,
    interpretation: &ParserInterpretationKey,
    snapshot_data: &SnapshotData,
) -> crate::Result<SnapshotKey> {
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
        return Ok(SnapshotKey::from_persisted(published.snapshot_key));
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
        row.declared_version.as_deref() == snapshot_data.version.as_deref()
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
        .bind::<Nullable<Text>, _>(snapshot_data.version.clone())
        .bind::<Text, _>(scope_kind)
        .bind::<Nullable<Text>, _>(scope_json)
        .execute(conn)?;
        snapshot_key
    };
    insert_snapshot_contents(conn, &snapshot_key, snapshot_data)?;
    sql_query(
        "INSERT INTO snapshot_publications \
         (catalog_key, document_key, interpretation_key, snapshot_key) \
         VALUES (?, ?, ?, ?)",
    )
    .bind::<Text, _>(request.catalog_key.as_str())
    .bind::<Text, _>(document_key.to_string())
    .bind::<Text, _>(interpretation.as_str())
    .bind::<Text, _>(snapshot_key.as_str())
    .execute(conn)?;
    Ok(snapshot_key)
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
