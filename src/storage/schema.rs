diesel::table! {
    archive_files (id) {
        id -> Integer,
        path -> Text,
        sha1 -> Binary,
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
diesel::joinable!(rom_files -> roms (rom_id));
diesel::joinable!(roms -> archive_files (archive_file_id));
diesel::joinable!(roms -> games (game_id));

diesel::allow_tables_to_appear_in_same_query!(archive_files, data_files, games, rom_files, roms,);
