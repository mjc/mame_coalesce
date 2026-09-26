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
    documents (document_key) {
        document_key -> Text,
        sha1 -> Nullable<Binary>,
        byte_length -> Nullable<BigInt>,
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
diesel::joinable!(acquisitions -> publishing_sources (source_key));
diesel::joinable!(acquisitions -> documents (document_key));
diesel::joinable!(catalog_snapshots -> catalogs (catalog_key));
diesel::joinable!(catalog_snapshots -> documents (document_key));
diesel::joinable!(catalog_snapshots -> parser_interpretations (interpretation_key));
diesel::joinable!(import_runs -> catalogs (catalog_key));
diesel::joinable!(import_runs -> documents (document_key));
diesel::joinable!(import_runs -> parser_interpretations (interpretation_key));
diesel::joinable!(rom_files -> roms (rom_id));
diesel::joinable!(roms -> archive_files (archive_file_id));
diesel::joinable!(roms -> games (game_id));

diesel::allow_tables_to_appear_in_same_query!(
    acquisitions,
    archive_files,
    catalogs,
    catalog_snapshots,
    data_files,
    documents,
    games,
    import_runs,
    parser_interpretations,
    publishing_sources,
    rom_files,
    roms,
);
