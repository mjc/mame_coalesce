use diesel::{
    QueryableByName,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};

#[derive(QueryableByName)]
pub(super) struct OwnerRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub claim_kind: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub rom_id: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub sample_id: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    pub content_uuid: Option<Vec<u8>>,
    #[diesel(sql_type = Bool)]
    pub valid_owner: bool,
    #[diesel(sql_type = Bool)]
    pub valid_identity: bool,
}

#[derive(QueryableByName)]
pub(super) struct RomRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = Text)]
    pub name: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    pub crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub crc32_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub status_text: Option<String>,
    #[diesel(sql_type = Bool)]
    pub nodump_present: bool,
    #[diesel(sql_type = Bool)]
    pub baddump_present: bool,
    #[diesel(sql_type = Nullable<Text>)]
    pub dump_status: Option<String>,
    #[diesel(sql_type = Text)]
    pub evidence_scope: String,
    #[diesel(sql_type = BigInt)]
    pub source_order: i64,
    #[diesel(sql_type = BigInt)]
    pub source_line: i64,
    #[diesel(sql_type = BigInt)]
    pub source_column: i64,
    #[diesel(sql_type = Bool)]
    pub valid_storage: bool,
}

#[derive(QueryableByName)]
pub(super) struct SampleRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = Text)]
    pub sample_name: String,
    #[diesel(sql_type = Text)]
    pub source_field: String,
    #[diesel(sql_type = BigInt)]
    pub source_order: i64,
    #[diesel(sql_type = Bool)]
    pub is_quoted: bool,
    #[diesel(sql_type = BigInt)]
    pub source_line: i64,
    #[diesel(sql_type = BigInt)]
    pub source_column: i64,
    #[diesel(sql_type = Bool)]
    pub valid_storage: bool,
}

#[derive(QueryableByName)]
pub(super) struct PositionRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    pub field_kind: i64,
    #[diesel(sql_type = Text)]
    pub source_field: String,
    #[diesel(sql_type = BigInt)]
    pub source_order: i64,
    #[diesel(sql_type = Bool)]
    pub is_quoted: bool,
    #[diesel(sql_type = BigInt)]
    pub source_line: i64,
    #[diesel(sql_type = BigInt)]
    pub source_column: i64,
    #[diesel(sql_type = Bool)]
    pub valid_storage: bool,
}

#[derive(QueryableByName)]
pub(super) struct MergeRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    pub relationship_id: i64,
    #[diesel(sql_type = Text)]
    pub merge_name: String,
    #[diesel(sql_type = Bool)]
    pub valid_storage: bool,
    #[diesel(sql_type = Bool)]
    pub valid_owner: bool,
}

#[derive(QueryableByName)]
pub(super) struct DigestRow {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    pub algorithm: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    pub digest: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    pub scope: String,
    #[diesel(sql_type = Text)]
    pub provenance: String,
    #[diesel(sql_type = Bool)]
    pub valid_storage: bool,
}
