// Typed Diesel bindings for rebuildable scanned-file inventory only.
// Native catalog readers and writers use the authoritative DDL and typed SQL rows.

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
        archive_backend -> Nullable<Text>,
        archive_member_index -> Nullable<BigInt>,
        scan_root -> Nullable<Text>,
        scan_run -> Nullable<Text>,
        observed_size -> Nullable<BigInt>,
        source_fingerprint -> Nullable<Binary>,
        scan_provenance -> Nullable<Text>,
        bare_file_cache_stamp -> Nullable<Binary>,
        cache_reused -> Bool,
        physical_path -> Nullable<Text>,
    }
}
