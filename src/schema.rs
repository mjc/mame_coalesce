table! {
    archive_files (id) {
        id -> Integer,
        path -> Text,
        sha1 -> Binary,
    }
}

table! {
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

table! {
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
    }
}

table! {
    rom_files (id) {
        id -> Integer,
        parent_path -> Text,
        path -> Text,
        name -> Text,
        crc -> Binary,
        sha1 -> Binary,
        md5 -> Binary,
        in_archive -> Bool,
        rom_id -> Nullable<Integer>,
    }
}

table! {
    roms (id) {
        id -> Integer,
        name -> Text,
        size -> Nullable<Integer>,
        md5 -> Nullable<Binary>,
        sha1 -> Nullable<Binary>,
        crc -> Nullable<Binary>,
        date -> Nullable<Date>,
        updated_at -> Nullable<Timestamp>,
        inserted_at -> Nullable<Timestamp>,
        game_id -> Nullable<Integer>,
        archive_file_id -> Nullable<Integer>,
    }
}

joinable!(games -> data_files (data_file_id));
joinable!(rom_files -> roms (rom_id));
joinable!(roms -> archive_files (archive_file_id));
joinable!(roms -> games (game_id));

allow_tables_to_appear_in_same_query!(
    archive_files,
    data_files,
    games,
    rom_files,
    roms,
);
