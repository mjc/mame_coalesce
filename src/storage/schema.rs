diesel::table! {
    archive_files (id) {
        id -> Integer,
        path -> Text,
        sha1 -> Binary,
    }
}

diesel::table! {
    acquisitions (acquisition_key) {
        acquisition_key -> Text,
        source_key -> Text,
        document_key -> Text,
        source_uri -> Nullable<Text>,
        method -> Nullable<Text>,
        acquired_at -> Nullable<Timestamp>,
        transport_metadata_json -> Nullable<Text>,
        expected_sha256 -> Nullable<Binary>,
        verification_status -> Text,
    }
}

diesel::table! {
    acquisition_attempts (attempt_key) {
        attempt_key -> Text,
        source_key -> Text,
        source_uri -> Nullable<Text>,
        method -> Nullable<Text>,
        attempted_at -> Timestamp,
        transport_metadata_json -> Nullable<Text>,
        expected_sha256 -> Nullable<Binary>,
        outcome -> Text,
        verification_status -> Text,
        document_key -> Nullable<Text>,
        acquisition_key -> Nullable<Text>,
        diagnostic -> Nullable<Text>,
    }
}

diesel::table! {
    catalogs (catalog_key) {
        catalog_key -> Text,
        source_key -> Text,
        display_name -> Text,
    }
}

diesel::table! {
    catalog_snapshots (snapshot_key) {
        snapshot_key -> Text,
        catalog_key -> Text,
        document_key -> Text,
        interpretation_key -> Text,
        acquisition_key -> Nullable<Text>,
        declared_version -> Nullable<Text>,
        scope_kind -> Text,
        scope_json -> Nullable<Text>,
        parent_snapshot_key -> Nullable<Text>,
    }
}

diesel::table! {
    snapshot_publications (catalog_key, document_key, interpretation_key) {
        catalog_key -> Text,
        document_key -> Text,
        interpretation_key -> Text,
        snapshot_key -> Text,
        published_at -> Timestamp,
    }
}

diesel::table! {
    snapshot_sets (snapshot_key, set_name) {
        snapshot_key -> Text,
        set_name -> Text,
        parent_name -> Nullable<Text>,
        metadata_json -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_lists (snapshot_key, list_name) {
        snapshot_key -> Text,
        list_name -> Text,
        list_order -> BigInt,
        description -> Nullable<Text>,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_items (snapshot_key, list_name, item_name) {
        snapshot_key -> Text,
        list_name -> Text,
        item_name -> Text,
        item_order -> BigInt,
        supported -> Nullable<Text>,
        description -> Text,
        year -> Text,
        publisher -> Text,
        notes -> Nullable<Text>,
        info_json -> Text,
        shared_features_json -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_parts (snapshot_key, list_name, item_name, part_name) {
        snapshot_key -> Text,
        list_name -> Text,
        item_name -> Text,
        part_name -> Text,
        part_order -> BigInt,
        interface -> Text,
        features_json -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_areas (snapshot_key, list_name, item_name, part_name, area_order) {
        snapshot_key -> Text,
        list_name -> Text,
        item_name -> Text,
        part_name -> Text,
        area_name -> Text,
        area_kind -> Text,
        area_order -> BigInt,
        declared_size -> Nullable<BigInt>,
        width -> Nullable<BigInt>,
        endianness -> Nullable<Text>,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_components (snapshot_key, list_name, item_name, part_name, area_order, component_order) {
        snapshot_key -> Text,
        list_name -> Text,
        item_name -> Text,
        part_name -> Text,
        area_order -> BigInt,
        area_kind -> Text,
        area_name -> Text,
        component_order -> BigInt,
        component_kind -> Text,
        component_name -> Nullable<Text>,
        size -> Nullable<BigInt>,
        crc -> Nullable<Binary>,
        sha1 -> Nullable<Binary>,
        offset -> Nullable<BigInt>,
        value -> Nullable<Text>,
        dump_status -> Nullable<Text>,
        writeable -> Nullable<BigInt>,
        load_instruction -> Nullable<Text>,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    software_item_dependencies (snapshot_key, list_name, item_name, dependency_kind) {
        snapshot_key -> Text,
        list_name -> Text,
        item_name -> Text,
        dependency_kind -> Text,
        target_item_name -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    asset_requirements (snapshot_key, set_name, component_order) {
        snapshot_key -> Text,
        set_name -> Text,
        component_order -> BigInt,
        asset_name -> Text,
        role -> Text,
        size -> Nullable<BigInt>,
        crc -> Nullable<Binary>,
        md5 -> Nullable<Binary>,
        sha1 -> Nullable<Binary>,
        evidence_scope -> Text,
        evidence_provenance -> Text,
        merge_name -> Nullable<Text>,
        dump_status -> Nullable<Text>,
        serial -> Nullable<Text>,
        date -> Nullable<Text>,
        metadata_json -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    snapshot_extensions (extension_id) {
        extension_id -> BigInt,
        snapshot_key -> Text,
        record_kind -> Text,
        record_name -> Nullable<Text>,
        field_name -> Text,
        namespace_uri -> Nullable<Text>,
        raw_value_json -> Text,
        source_line -> BigInt,
        source_column -> BigInt,
    }
}

diesel::table! {
    import_diagnostics (diagnostic_key) {
        diagnostic_key -> Text,
        run_key -> Text,
        code -> Text,
        message -> Text,
        record_kind -> Nullable<Text>,
        record_name -> Nullable<Text>,
        field_name -> Nullable<Text>,
        raw_value_json -> Nullable<Text>,
        source_line -> Nullable<BigInt>,
        source_column -> Nullable<BigInt>,
    }
}

diesel::table! {
    documents (document_key) {
        document_key -> Text,
        sha1 -> Nullable<Binary>,
        byte_length -> Nullable<BigInt>,
        sha256 -> Nullable<Binary>,
        payload -> Nullable<Binary>,
        format_hint -> Nullable<Text>,
        retention_status -> Text,
    }
}

diesel::table! {
    data_files (id) {
        id -> Integer,
        build -> Nullable<Text>,
        debug -> Nullable<Text>,
        file_name -> Nullable<Text>,
        name -> Text,
        description -> Nullable<Text>,
        category -> Nullable<Text>,
        version -> Nullable<Text>,
        author -> Nullable<Text>,
        email -> Nullable<Text>,
        homepage -> Nullable<Text>,
        url -> Nullable<Text>,
        sha1 -> Nullable<Binary>,
    }
}

diesel::table! {
    games (id) {
        id -> Integer,
        name -> Text,
        is_bios -> Nullable<Text>,
        clone_of -> Nullable<Text>,
        rom_of -> Nullable<Text>,
        sample_of -> Nullable<Text>,
        board -> Nullable<Text>,
        rebuildto -> Nullable<Text>,
        year -> Nullable<Text>,
        manufacturer -> Nullable<Text>,
        data_file_id -> Nullable<Integer>,
        parent_id -> Nullable<Integer>,
    }
}

diesel::table! {
    import_runs (run_key) {
        run_key -> Text,
        catalog_key -> Text,
        document_key -> Text,
        interpretation_key -> Text,
        acquisition_key -> Nullable<Text>,
        snapshot_key -> Nullable<Text>,
        status -> Text,
        started_at -> Nullable<Timestamp>,
        finished_at -> Nullable<Timestamp>,
        diagnostic -> Nullable<Text>,
    }
}

diesel::table! {
    parser_interpretations (interpretation_key) {
        interpretation_key -> Text,
        format -> Text,
        parser_name -> Nullable<Text>,
        parser_version -> Nullable<Text>,
        rules_version -> Nullable<Text>,
        options_json -> Nullable<Text>,
    }
}

diesel::table! {
    publishing_sources (source_key) {
        source_key -> Text,
        display_name -> Text,
        locator -> Nullable<Text>,
    }
}

diesel::table! {
    rom_files (id) {
        id -> Integer,
        parent_path -> Text,
        parent_game_name -> Nullable<Text>,
        path -> Text,
        name -> Text,
        crc -> Nullable<Binary>,
        sha1 -> Binary,
        md5 -> Nullable<Binary>,
        xxhash3 -> Binary,
        in_archive -> Bool,
        rom_id -> Nullable<Integer>,
    }
}

diesel::table! {
    roms (id) {
        id -> Integer,
        name -> Text,
        size -> Integer,
        md5 -> Binary,
        sha1 -> Binary,
        crc -> Binary,
        date -> Nullable<Date>,
        updated_at -> Nullable<Timestamp>,
        inserted_at -> Nullable<Timestamp>,
        game_id -> Nullable<Integer>,
        archive_file_id -> Nullable<Integer>,
    }
}

diesel::joinable!(games -> data_files (data_file_id));
diesel::joinable!(catalogs -> publishing_sources (source_key));
diesel::joinable!(acquisition_attempts -> publishing_sources (source_key));
diesel::joinable!(acquisition_attempts -> documents (document_key));
diesel::joinable!(acquisition_attempts -> acquisitions (acquisition_key));
diesel::joinable!(acquisitions -> publishing_sources (source_key));
diesel::joinable!(acquisitions -> documents (document_key));
diesel::joinable!(catalog_snapshots -> catalogs (catalog_key));
diesel::joinable!(catalog_snapshots -> documents (document_key));
diesel::joinable!(catalog_snapshots -> parser_interpretations (interpretation_key));
diesel::joinable!(snapshot_sets -> catalog_snapshots (snapshot_key));
diesel::joinable!(snapshot_extensions -> catalog_snapshots (snapshot_key));
diesel::joinable!(import_diagnostics -> import_runs (run_key));
diesel::joinable!(import_runs -> catalogs (catalog_key));
diesel::joinable!(import_runs -> documents (document_key));
diesel::joinable!(import_runs -> parser_interpretations (interpretation_key));
diesel::joinable!(rom_files -> roms (rom_id));
diesel::joinable!(roms -> archive_files (archive_file_id));
diesel::joinable!(roms -> games (game_id));

diesel::allow_tables_to_appear_in_same_query!(
    acquisitions,
    acquisition_attempts,
    asset_requirements,
    archive_files,
    catalogs,
    catalog_snapshots,
    data_files,
    documents,
    games,
    import_runs,
    import_diagnostics,
    parser_interpretations,
    publishing_sources,
    rom_files,
    roms,
    software_areas,
    software_components,
    software_item_dependencies,
    software_items,
    software_lists,
    software_parts,
    snapshot_extensions,
    snapshot_sets,
    snapshot_publications,
);
