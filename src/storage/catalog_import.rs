use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    app::{CatalogImportReport, CatalogImportRequest, CatalogImportStatus},
    domain::{DocumentKey, ImportRunKey, ParserInterpretationKey, SnapshotKey},
    logiqx::{DataFile, XmlSourceMap},
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

pub fn import(pool: &Pool, request: &CatalogImportRequest) -> crate::Result<CatalogImportReport> {
    ensure_source(pool, request)?;
    let documents = DocumentStore::from_pool(pool.clone());
    let retained = documents.retain_path(request.source_key.clone(), &request.document_path)?;
    let bytes = documents.load(&retained.document_key)?;
    let (data_file, source_map) = match DataFile::from_reader_with_source_map(bytes.as_slice()) {
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
        &data_file,
        &source_map,
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
    let interpretation = ParserInterpretationKey::logiqx_v1(&request.scope);
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
    data_file: &DataFile,
    source_map: &XmlSourceMap,
) -> crate::Result<CatalogImportReport> {
    let interpretation = ParserInterpretationKey::logiqx_v1(&request.scope);
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
            .bind::<Nullable<Text>, _>(data_file.header().version().cloned())
            .bind::<Text, _>(scope_kind)
            .bind::<Nullable<Text>, _>(scope_json.clone())
            .execute(conn)?;
            insert_snapshot_contents(conn, &snapshot_key, data_file, source_map)?;
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
        for extension in &source_map.unsupported_attributes {
            let raw_value_json = serde_json::to_string(&extension.value)?;
            sql_query(
                "INSERT INTO import_diagnostics \
                 (diagnostic_key, run_key, code, message, record_kind, record_name, \
                  field_name, raw_value_json, source_line, source_column) \
                 VALUES (?, ?, 'unsupported_attribute', ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
            .bind::<Text, _>(run_key.to_string())
            .bind::<Text, _>(format!("unsupported attribute {}", extension.field_name))
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
            diagnostic_count: source_map.unsupported_attributes.len(),
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
         VALUES (?, 'logiqx', 'mame_coalesce', ?, 'normalization-v1', ?) \
         ON CONFLICT(interpretation_key) DO NOTHING",
    )
    .bind::<Text, _>(interpretation.as_str())
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
    data_file: &DataFile,
    source_map: &XmlSourceMap,
) -> crate::Result<()> {
    for (game_index, game) in data_file.games().iter().enumerate() {
        let set_location = source_map
            .game_locations
            .get(game_index)
            .copied()
            .ok_or_else(|| {
                crate::Error::InvalidPath("Logiqx set source location is missing".into())
            })?;
        let metadata_json = serde_json::json!({
            "source_file": game.sourcefile_opt(),
            "is_bios": game.isbios_opt(),
            "rom_of": game.romof_opt(),
            "sample_of": game.sampleof_opt(),
            "board": game.board_opt(),
            "rebuild_to": game.rebuildto_opt(),
            "description": game.description_opt(),
            "year": game.year_opt(),
            "manufacturer": game.manufacturer_opt(),
            "device_refs": game.device_refs().collect::<Vec<_>>(),
        })
        .to_string();
        sql_query(
            "INSERT INTO snapshot_sets \
             (snapshot_key, set_name, parent_name, metadata_json, source_line, source_column) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(snapshot_key.as_str())
        .bind::<Text, _>(game.name())
        .bind::<Nullable<Text>, _>(game.cloneof().map(str::to_owned))
        .bind::<Text, _>(metadata_json)
        .bind::<BigInt, _>(set_location.line)
        .bind::<BigInt, _>(set_location.column)
        .execute(conn)?;

        let rom_locations = source_map.rom_locations.get(game_index).ok_or_else(|| {
            crate::Error::InvalidPath("Logiqx asset source locations are missing".into())
        })?;
        for (component_order, rom) in game.roms().iter().enumerate() {
            let source_order = component_order;
            let component_order = i64::try_from(source_order)
                .map_err(|_| crate::Error::InvalidPath("too many Logiqx assets".into()))?;
            let location = rom_locations.get(source_order).copied().ok_or_else(|| {
                crate::Error::InvalidPath("Logiqx asset source location is missing".into())
            })?;
            let expected = crate::domain::ExpectedEvidence::from_logiqx(rom)?;
            let size = expected
                .size
                .map(i64::try_from)
                .transpose()
                .map_err(|_| crate::Error::InvalidRomSize(expected.size.unwrap_or_default()))?;
            sql_query(
                "INSERT INTO asset_requirements \
                 (snapshot_key, set_name, component_order, asset_name, role, size, crc, md5, sha1, \
                  evidence_scope, evidence_provenance, merge_name, dump_status, serial, date, \
                  source_line, source_column) \
                 VALUES (?, ?, ?, ?, 'rom', ?, ?, ?, ?, 'whole_asset', 'source_declared', ?, ?, ?, ?, ?, ?)",
            )
            .bind::<Text, _>(snapshot_key.as_str())
            .bind::<Text, _>(game.name())
            .bind::<BigInt, _>(component_order)
            .bind::<Text, _>(rom.name())
            .bind::<Nullable<BigInt>, _>(size)
            .bind::<Nullable<Binary>, _>(expected.crc.map(|digest| digest.0.to_vec()))
            .bind::<Nullable<Binary>, _>(expected.md5.map(|digest| digest.0.to_vec()))
            .bind::<Nullable<Binary>, _>(expected.sha1.map(|digest| digest.to_vec()))
            .bind::<Nullable<Text>, _>(expected.merge)
            .bind::<Nullable<Text>, _>(expected.dump_status)
            .bind::<Nullable<Text>, _>(expected.serial)
            .bind::<Nullable<Text>, _>(expected.date)
            .bind::<BigInt, _>(location.line)
            .bind::<BigInt, _>(location.column)
            .execute(conn)?;
        }
    }

    for extension in &source_map.unsupported_attributes {
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
