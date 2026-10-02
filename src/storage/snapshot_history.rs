use std::collections::{BTreeMap, BTreeSet};

use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Bool, Nullable, Text},
};

use crate::domain::{
    CatalogKey, CatalogScope, CatalogSnapshotDiff, CatalogSnapshotEntry, QualifiedCatalogSet,
    RelationshipEndpoint, RelationshipExplanation, SetCoverage, SetName, SnapshotKey,
    SnapshotRecordCorrespondence, SnapshotRecordDiff, SnapshotRecordStatus,
    SnapshotRequirementChange,
};

use super::catalog_coverage::CoverageId;
use super::db::Pool;

mod no_intro_database;
mod software;

struct SnapshotRow {
    catalog_key: String,
    scope: CatalogScope,
    format_hint: Option<String>,
    published: bool,
}

#[derive(QueryableByName)]
struct SnapshotHeaderRow {
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    format_hint: Option<String>,
    #[diesel(sql_type = Bool)]
    published: bool,
}

#[derive(QueryableByName)]
struct HistoryRow {
    #[diesel(sql_type = Text)]
    snapshot_key: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[derive(QueryableByName)]
struct SetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    parent_name: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct RequirementRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    asset_name: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    md5: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha256: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_bios: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_offset_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Bool>)]
    mame_status_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_optional: Option<i64>,
    #[diesel(sql_type = Nullable<Bool>)]
    mame_optional_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_sound_only: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_dispose: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_load_flag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_value: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_inverted: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_ovha: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_no_thread: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    mame_disk_index: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_writable: Option<i64>,
    #[diesel(sql_type = Nullable<Bool>)]
    mame_writable_specified: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    mame_writeable: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    logiqx_size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    logiqx_crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    logiqx_md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    logiqx_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    logiqx_status_was_present: Option<bool>,
    #[diesel(sql_type = Nullable<BigInt>)]
    logiqx_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_size_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_crc32_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cmp_status_text: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    cmp_nodump_present: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    cmp_baddump_present: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    no_intro_dat_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_crc_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_md5_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_sha1_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_sha256_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_status_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_serial_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_header_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_date_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    no_intro_dat_mia_text: Option<String>,
}

#[derive(QueryableByName)]
struct CmpRomPositionRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    form_source_order: i64,
    #[diesel(sql_type = Bool)]
    is_quoted: bool,
}

#[derive(QueryableByName)]
struct MachineSwitchRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct MachineSwitchLocationRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    location_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    number: String,
    #[diesel(sql_type = Bool)]
    inverted: bool,
    #[diesel(sql_type = Bool)]
    inverted_specified: bool,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct MachineSwitchValueRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    switch_order: i64,
    #[diesel(sql_type = BigInt)]
    value_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = Bool)]
    is_default: bool,
    #[diesel(sql_type = Bool)]
    default_specified: bool,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct MachineBiosSetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    bios_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Bool)]
    is_default: bool,
    #[diesel(sql_type = Bool)]
    default_specified: bool,
}

#[derive(QueryableByName)]
struct MameMachineDependencyRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    dependency_order: i64,
    #[diesel(sql_type = Text)]
    dependency_kind: String,
    #[diesel(sql_type = Text)]
    target_name: String,
    #[diesel(sql_type = Nullable<Text>)]
    reference_tag: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_order: Option<i64>,
}

#[derive(QueryableByName)]
struct MameChildPositionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    child_kind: String,
    #[diesel(sql_type = BigInt)]
    child_order: i64,
}

#[derive(QueryableByName)]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent MAME DTD flags from one SQLite row.
struct MameMachineFactsRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = BigInt)]
    description_source_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    year_source_order: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    manufacturer_source_order: Option<i64>,
    #[diesel(sql_type = Bool)]
    is_device: bool,
    #[diesel(sql_type = Bool)]
    is_device_specified: bool,
    #[diesel(sql_type = Bool)]
    runnable: bool,
    #[diesel(sql_type = Bool)]
    runnable_specified: bool,
    #[diesel(sql_type = Bool)]
    is_bios: bool,
    #[diesel(sql_type = Bool)]
    is_bios_specified: bool,
    #[diesel(sql_type = Bool)]
    is_mechanical: bool,
    #[diesel(sql_type = Bool)]
    is_mechanical_specified: bool,
    #[diesel(sql_type = Bool)]
    is_consumable: bool,
    #[diesel(sql_type = Bool)]
    is_consumable_specified: bool,
}

#[derive(QueryableByName)]
struct NoIntroGameFactsRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    archive_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name_alt: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    bios_text: Option<String>,
    #[diesel(sql_type = Bool)]
    languages_present: bool,
}

#[derive(QueryableByName)]
struct NoIntroLanguageRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    language_order: i64,
    #[diesel(sql_type = Text)]
    language: String,
}

#[derive(QueryableByName)]
struct NoIntroCloneMarkerRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
}

#[derive(QueryableByName)]
struct NoIntroArchiveLinkRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    target_archive_id: String,
}

#[derive(QueryableByName)]
struct CmpSetFactsRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rebuildto: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    region: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_year_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_month_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    release_day_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Text)]
    source_block: String,
}

#[derive(QueryableByName)]
struct CmpHeaderFactsRow {
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    email: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    category: Option<String>,
    #[diesel(sql_type = Text)]
    source_block: String,
}

#[derive(QueryableByName)]
struct CmpHeaderDirectivesRow {
    #[diesel(sql_type = Nullable<Text>)]
    header_definition: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcepacking: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcemerging_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcezipping_effective: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    forcenodump_effective: Option<String>,
}

#[derive(QueryableByName, PartialEq, Eq)]
struct CmpDocumentMetadataRow {
    #[diesel(sql_type = Bool)]
    header_present: bool,
    #[diesel(sql_type = BigInt)]
    comment_count: i64,
}

#[derive(QueryableByName)]
struct CmpDocumentLayoutRow {
    #[diesel(sql_type = Text)]
    layout_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    set_name: Option<String>,
    #[diesel(sql_type = Text)]
    source_block: String,
    #[diesel(sql_type = BigInt)]
    document_order: i64,
}

#[derive(QueryableByName, PartialEq, Eq)]
struct CmpHeaderFieldPositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Bool)]
    is_quoted: bool,
}

#[derive(QueryableByName)]
struct CmpSetFieldPositionRow {
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Bool)]
    is_quoted: bool,
}

#[derive(QueryableByName)]
struct CmpSamplePositionRow {
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = BigInt)]
    sample_order: i64,
    #[diesel(sql_type = Text)]
    sample_name: String,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Bool)]
    is_quoted: bool,
}

#[derive(QueryableByName)]
struct CmpSampleParentPositionRow {
    #[diesel(sql_type = BigInt)]
    record_id: i64,
    #[diesel(sql_type = Text)]
    target_name: String,
}

#[derive(QueryableByName)]
struct CmpCommentRow {
    #[diesel(sql_type = Text)]
    comment_text: String,
}

#[derive(QueryableByName)]
struct CmpNativeOrderRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct LogiqxSetFactsRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    source_file: Option<String>,
    #[diesel(sql_type = Text)]
    is_bios: String,
    #[diesel(sql_type = Bool)]
    is_bios_was_present: bool,
    #[diesel(sql_type = Nullable<Text>)]
    board: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rebuild_to: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    year: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    manufacturer: Option<String>,
}

#[derive(QueryableByName)]
struct LogiqxGameCommentRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    comment_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    comment_text: String,
}

#[derive(QueryableByName)]
struct LogiqxReleaseRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    release_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    region: String,
    #[diesel(sql_type = Nullable<Text>)]
    language: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Text)]
    default: String,
    #[diesel(sql_type = Bool)]
    default_was_present: bool,
}

#[derive(QueryableByName)]
struct LogiqxBiosSetRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    bios_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Text)]
    is_default: String,
    #[diesel(sql_type = Bool)]
    default_was_present: bool,
}

#[derive(QueryableByName)]
struct LogiqxArchiveReferenceRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    archive_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    archive_name: String,
}

#[derive(QueryableByName, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Each field preserves an independent Logiqx option presence bit.
struct LogiqxClrMameProOptionsRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    header: Option<String>,
    #[diesel(sql_type = Bool)]
    header_was_present: bool,
    #[diesel(sql_type = Text)]
    forcemerging: String,
    #[diesel(sql_type = Bool)]
    forcemerging_was_present: bool,
    #[diesel(sql_type = Text)]
    forcenodump: String,
    #[diesel(sql_type = Bool)]
    forcenodump_was_present: bool,
    #[diesel(sql_type = Text)]
    forcepacking: String,
    #[diesel(sql_type = Bool)]
    forcepacking_was_present: bool,
}

#[derive(QueryableByName, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Each field preserves an independent Logiqx option presence bit.
struct LogiqxRomCenterOptionsRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    plugin: Option<String>,
    #[diesel(sql_type = Bool)]
    plugin_was_present: bool,
    #[diesel(sql_type = Text)]
    rommode: String,
    #[diesel(sql_type = Bool)]
    rommode_was_present: bool,
    #[diesel(sql_type = Text)]
    biosmode: String,
    #[diesel(sql_type = Bool)]
    biosmode_was_present: bool,
    #[diesel(sql_type = Text)]
    samplemode: String,
    #[diesel(sql_type = Bool)]
    samplemode_was_present: bool,
    #[diesel(sql_type = Text)]
    lockrommode: String,
    #[diesel(sql_type = Bool)]
    lockrommode_was_present: bool,
    #[diesel(sql_type = Text)]
    lockbiosmode: String,
    #[diesel(sql_type = Bool)]
    lockbiosmode_was_present: bool,
    #[diesel(sql_type = Text)]
    locksamplemode: String,
    #[diesel(sql_type = Bool)]
    locksamplemode_was_present: bool,
}

#[derive(QueryableByName, serde::Serialize)]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent DTD boolean columns for snapshot comparison.
struct MachineSpecificationRow {
    #[serde(skip)]
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    element_order: i64,
    #[diesel(sql_type = Text)]
    element_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    sample_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    chip_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    chip_tag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    chip_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    chip_clock: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_tag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_rotate: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    flipx: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    flipx_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    display_width: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_height: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_refresh: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_pixclock: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_htotal: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_hbend: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_hbstart: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_vtotal: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_vbend: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    display_vbstart: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sound_channels: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    input_service: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    input_service_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    input_tilt: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    input_tilt_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    input_players: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    input_coins: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    port_tag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    adjuster_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    adjuster_default: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    driver_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    driver_emulation: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    driver_cocktail: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    driver_savestate: Option<String>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_requiresartwork: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_requiresartwork_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_unofficial: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_unofficial_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_nosoundhardware: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_nosoundhardware_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_incomplete: Option<bool>,
    #[diesel(sql_type = Nullable<Bool>)]
    driver_incomplete_specified: Option<bool>,
    #[diesel(sql_type = Nullable<Text>)]
    feature_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    feature_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    feature_overall: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_type: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_tag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_fixed_image: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_mandatory: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_interface: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_instance_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    device_instance_briefname: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    device_instance_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    device_instance_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    slot_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    softwarelist_tag: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    softwarelist_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    softwarelist_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    softwarelist_filter: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ramoption_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ramoption_default: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ramoption_text: Option<String>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct MachineInputControlRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    element_order: i64,
    #[diesel(sql_type = BigInt)]
    control_order: i64,
    #[diesel(sql_type = Text)]
    control_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    player: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    buttons: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    minimum: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    maximum: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    sensitivity: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    keydelta: Option<String>,
    #[diesel(sql_type = Bool)]
    reverse: bool,
    #[diesel(sql_type = Bool)]
    reverse_specified: bool,
    #[diesel(sql_type = Nullable<Text>)]
    ways: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ways2: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    ways3: Option<String>,
}

#[derive(QueryableByName)]
struct MachineAnalogRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    element_order: i64,
    #[diesel(sql_type = BigInt)]
    analog_order: i64,
    #[diesel(sql_type = Text)]
    mask: String,
}

#[derive(QueryableByName)]
struct MachineDeviceExtensionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    element_order: i64,
    #[diesel(sql_type = BigInt)]
    extension_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct MachineSlotOptionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    element_order: i64,
    #[diesel(sql_type = BigInt)]
    option_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    devname: String,
    #[diesel(sql_type = Bool)]
    is_default: bool,
    #[diesel(sql_type = Bool)]
    default_specified: bool,
}

#[derive(QueryableByName)]
struct MachineConditionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    owner_kind: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_element_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_switch_order: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    owner_child_order: Option<i64>,
    #[diesel(sql_type = BigInt)]
    condition_order: i64,
    #[diesel(sql_type = Text)]
    tag: String,
    #[diesel(sql_type = Text)]
    mask: String,
    #[diesel(sql_type = Text)]
    relation: String,
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(Default)]
struct CatalogRecords {
    sets: BTreeMap<String, Vec<SetRow>>,
    requirements: BTreeMap<i64, BTreeMap<String, Vec<serde_json::Value>>>,
    machine_switches: BTreeMap<i64, Vec<serde_json::Value>>,
    machine_bios_sets: BTreeMap<i64, Vec<serde_json::Value>>,
    mame_machine_dependencies: BTreeMap<i64, Vec<serde_json::Value>>,
    mame_machine_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    mame_machine_specification_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_game_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_game_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_categories: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_identifiers: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_releases: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_rom_order: BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_database_games: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_set_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_text_positions: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_game_comments: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_releases: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_bios_sets: BTreeMap<i64, Vec<serde_json::Value>>,
    logiqx_archive_references: BTreeMap<i64, Vec<serde_json::Value>>,
    cmp_set_facts: BTreeMap<i64, Vec<serde_json::Value>>,
    software_items: BTreeMap<i64, Vec<serde_json::Value>>,
    software_item_lists: BTreeMap<i64, i64>,
    software_list_contexts: BTreeMap<i64, String>,
    software_context_classes: BTreeMap<i64, usize>,
}

pub fn diff(
    pool: &Pool,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<CatalogSnapshotDiff> {
    let mut conn = pool.get()?;
    conn.transaction::<_, crate::Error, _>(|conn| {
        let previous_header = snapshot(conn, previous)?;
        let current_header = snapshot(conn, current)?;
        if previous_header.catalog_key != current_header.catalog_key {
            return Err(crate::Error::InvalidPath(
                "snapshot history can only compare snapshots from the same catalog".to_owned(),
            ));
        }
        if is_software_list_snapshot(&previous_header) != is_software_list_snapshot(&current_header)
        {
            return Err(crate::Error::InvalidPath(
                "snapshot history cannot compare root catalogs with software-list catalogs"
                    .to_owned(),
            ));
        }

        let same_scope = comparable_scope(&previous_header, &current_header);
        let document_metadata_changed =
            document_metadata(conn, previous)? != document_metadata(conn, current)?;
        let mut previous_records = records(conn, previous)?;
        let mut current_records = records(conn, current)?;
        software::classify_parent_contexts(&mut previous_records, &mut current_records);
        let explanations =
            super::relationships::explain_catalog_sets_for_snapshots(conn, previous, current)?;
        let mut evidence_by_set = relationship_evidence_by_set(explanations, previous, current);
        let mut names = BTreeSet::new();
        names.extend(previous_records.sets.keys().cloned());
        names.extend(current_records.sets.keys().cloned());

        let records = names
            .into_iter()
            .map(|name| {
                let before = previous_records.sets.get(&name);
                let after = current_records.sets.get(&name);
                let relationship_evidence = evidence_by_set.remove(&name).unwrap_or_default();
                let comparison = match (before, after) {
                    (None, None) => unreachable!("name came from one of the set maps"),
                    (None, Some(_))
                        if scopes_cover_set(&previous_header, &current_header, &name) =>
                    {
                        no_counterpart(SnapshotRecordStatus::AddedWithinScope)
                    }
                    (Some(_), None)
                        if scopes_cover_set(&previous_header, &current_header, &name) =>
                    {
                        no_counterpart(SnapshotRecordStatus::RemovedWithinScope)
                    }
                    (None, _) | (_, None) => {
                        no_counterpart(absence_status(&previous_header, &current_header, &name))
                    }
                    (Some(before), Some(after)) => {
                        compare_owner_group(before, after, &previous_records, &current_records)
                    }
                };
                SnapshotRecordDiff {
                    set_name: name,
                    status: comparison.status,
                    correspondence: comparison.correspondence,
                    metadata_changed: comparison.metadata_changed,
                    regrouped: comparison.regrouped,
                    requirement_changes: comparison.requirement_changes,
                    relationship_evidence,
                }
            })
            .collect();

        Ok(CatalogSnapshotDiff {
            previous: previous.clone(),
            current: current.clone(),
            same_scope,
            document_metadata_changed,
            records,
        })
    })
}

struct OwnerGroupComparison {
    status: SnapshotRecordStatus,
    correspondence: SnapshotRecordCorrespondence,
    metadata_changed: bool,
    regrouped: bool,
    requirement_changes: Vec<SnapshotRequirementChange>,
}

const fn no_counterpart(status: SnapshotRecordStatus) -> OwnerGroupComparison {
    OwnerGroupComparison {
        status,
        correspondence: SnapshotRecordCorrespondence::NoCounterpart,
        metadata_changed: false,
        regrouped: false,
        requirement_changes: Vec::new(),
    }
}

fn compare_owner_group(
    before: &[SetRow],
    after: &[SetRow],
    previous_records: &CatalogRecords,
    current_records: &CatalogRecords,
) -> OwnerGroupComparison {
    let metadata_changed =
        owner_metadata(before, previous_records) != owner_metadata(after, current_records);
    let regrouped = parent_names(before) != parent_names(after);
    let correspondence = match (before.len(), after.len()) {
        (1, 1) => SnapshotRecordCorrespondence::UniqueName,
        _ if owner_signatures(before, previous_records)
            == owner_signatures(after, current_records) =>
        {
            SnapshotRecordCorrespondence::ExactFacts
        }
        _ => SnapshotRecordCorrespondence::Ambiguous,
    };
    let requirement_changes = match correspondence {
        SnapshotRecordCorrespondence::UniqueName => {
            let previous = before
                .first()
                .and_then(|set| previous_records.requirements.get(&set.set_id));
            let current = after
                .first()
                .and_then(|set| current_records.requirements.get(&set.set_id));
            requirement_changes(previous, current)
        }
        SnapshotRecordCorrespondence::ExactFacts
        | SnapshotRecordCorrespondence::Ambiguous
        | SnapshotRecordCorrespondence::NoCounterpart => Vec::new(),
    };
    let status = match correspondence {
        SnapshotRecordCorrespondence::Ambiguous => SnapshotRecordStatus::Changed,
        SnapshotRecordCorrespondence::UniqueName
        | SnapshotRecordCorrespondence::ExactFacts
        | SnapshotRecordCorrespondence::NoCounterpart
            if metadata_changed || regrouped || !requirement_changes.is_empty() =>
        {
            SnapshotRecordStatus::Changed
        }
        SnapshotRecordCorrespondence::UniqueName
        | SnapshotRecordCorrespondence::ExactFacts
        | SnapshotRecordCorrespondence::NoCounterpart => SnapshotRecordStatus::Unchanged,
    };

    OwnerGroupComparison {
        status,
        correspondence,
        metadata_changed,
        regrouped,
        requirement_changes,
    }
}

#[derive(QueryableByName, PartialEq, Eq)]
struct MameDocumentMetadataRow {
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    debug: bool,
    #[diesel(sql_type = Bool)]
    debug_specified: bool,
    #[diesel(sql_type = Text)]
    config_version: String,
}

#[derive(QueryableByName, PartialEq, Eq)]
struct LogiqxDocumentMetadataRow {
    #[diesel(sql_type = Nullable<Text>)]
    build: Option<String>,
    #[diesel(sql_type = Text)]
    debug: String,
    #[diesel(sql_type = Bool)]
    debug_was_present: bool,
    #[diesel(sql_type = Nullable<Text>)]
    file_name: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Text>)]
    header_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_email: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_comment: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    header_category: Option<String>,
}

#[derive(QueryableByName, PartialEq, Eq)]
struct NoIntroDatHeaderRow {
    #[diesel(sql_type = Nullable<Text>)]
    schema_location: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    version_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    author: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    homepage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    url: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    trademarks: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    piracy: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    subset: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    comment: Option<String>,
}

#[derive(PartialEq, Eq)]
struct DocumentMetadata {
    mame: Option<MameDocumentMetadataRow>,
    logiqx: Option<LogiqxDocumentMetadataRow>,
    clrmamepro: Option<LogiqxClrMameProOptionsRow>,
    romcenter: Option<LogiqxRomCenterOptionsRow>,
    text_positions: Vec<LogiqxTextPositionRow>,
    cmp: Option<serde_json::Value>,
    cmp_document: Option<CmpDocumentMetadataRow>,
    cmp_comments: Vec<String>,
    cmp_layout: Vec<serde_json::Value>,
    no_intro_dat: Option<serde_json::Value>,
    no_intro_database: Option<serde_json::Value>,
    software: Option<serde_json::Value>,
}

#[derive(QueryableByName, PartialEq, Eq)]
struct LogiqxTextPositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatHeaderPositionRow {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatOptionRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Nullable<Text>)]
    first_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    second_text: Option<String>,
}

#[derive(QueryableByName)]
struct NoIntroDatGameRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Nullable<Text>)]
    id_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cloneof_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    cloneofid_text: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    description_text: Option<String>,
}

#[derive(QueryableByName)]
struct NoIntroDatGamePositionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatCategoryRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    category_order: i64,
    #[diesel(sql_type = Text)]
    category: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatIdentifierRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    identifier_order: i64,
    #[diesel(sql_type = Text)]
    identifier: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatReleaseRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    release_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    region: String,
    #[diesel(sql_type = BigInt)]
    name_source_order: i64,
    #[diesel(sql_type = BigInt)]
    region_source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NoIntroDatRomPositionRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

fn document_metadata(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<DocumentMetadata> {
    let mame = sql_query(
        "SELECT build, debug, debug_specified, config_version \
         FROM mame_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<MameDocumentMetadataRow>(conn)
    .optional()?;
    let logiqx = sql_query(
        "SELECT build, debug, debug_was_present, file_name, sha1, header_name, header_description, \
         header_version, header_date, header_author, header_email, header_homepage, header_url, \
         header_comment, header_category FROM logiqx_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<LogiqxDocumentMetadataRow>(conn)
    .optional()?;
    let mut clrmamepro = sql_query(
        "SELECT source_order, header, header_was_present, forcemerging, \
         forcemerging_was_present, forcenodump, forcenodump_was_present, forcepacking, \
         forcepacking_was_present FROM logiqx_clrmamepro_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<LogiqxClrMameProOptionsRow>(conn)
    .optional()?;
    let mut romcenter = sql_query(
        "SELECT source_order, plugin, plugin_was_present, rommode, rommode_was_present, \
         biosmode, biosmode_was_present, samplemode, samplemode_was_present, lockrommode, \
         lockrommode_was_present, lockbiosmode, lockbiosmode_was_present, locksamplemode, \
         locksamplemode_was_present FROM logiqx_romcenter_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<LogiqxRomCenterOptionsRow>(conn)
    .optional()?;
    let mut text_positions = sql_query(
        "SELECT field_kind, source_order FROM logiqx_header_text_positions \
         WHERE snapshot_key = ? ORDER BY field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<LogiqxTextPositionRow>(conn)?;
    normalize_header_order(&mut text_positions, clrmamepro.as_mut(), romcenter.as_mut());
    let cmp = load_cmp_header_metadata(conn, snapshot)?;
    let cmp_document =
        sql_query("SELECT header_present, comment_count FROM cmp_documents WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<CmpDocumentMetadataRow>(conn)
            .optional()?;
    let cmp_comments = sql_query(
        "SELECT text AS comment_text \
         FROM cmp_comments WHERE snapshot_key = ? ORDER BY comment_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<CmpCommentRow>(conn)?
    .into_iter()
    .map(|comment| comment.comment_text)
    .collect();
    let cmp_layout = load_cmp_document_layout(conn, snapshot)?;
    let no_intro_dat = load_no_intro_dat_header_metadata(conn, snapshot)?;
    let no_intro_database = no_intro_database::load_document(conn, snapshot)?;
    let software = software::load_document(conn, snapshot)?;
    Ok(DocumentMetadata {
        mame,
        logiqx,
        clrmamepro,
        romcenter,
        text_positions,
        cmp,
        cmp_document,
        cmp_comments,
        cmp_layout,
        no_intro_dat,
        no_intro_database,
        software,
    })
}

fn load_no_intro_dat_header_metadata(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Option<serde_json::Value>> {
    let header = sql_query(
        "SELECT document.schema_location, header.id_text, header.name, \
                header.description, header.version_text, header.date, header.author, \
                header.homepage, header.url, header.trademarks, header.piracy, \
                header.subset, header.comment \
         FROM no_intro_dat_headers AS header \
         JOIN no_intro_dat_documents AS document USING (snapshot_key) \
         WHERE header.snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NoIntroDatHeaderRow>(conn)
    .optional()?;
    let Some(header) = header else {
        return Ok(None);
    };
    let mut header_positions = sql_query(
        "SELECT field_kind, source_order FROM no_intro_dat_header_field_positions \
         WHERE snapshot_key = ? ORDER BY source_order, field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<NoIntroDatHeaderPositionRow>(conn)?;
    let clrmamepro = sql_query(
        "SELECT source_order, forcenodump_text AS first_text, header_text AS second_text \
         FROM no_intro_dat_clrmamepro_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NoIntroDatOptionRow>(conn)
    .optional()?;
    let romcenter = sql_query(
        "SELECT source_order, plugin_text AS first_text, CAST(NULL AS TEXT) AS second_text \
         FROM no_intro_dat_romcenter_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<NoIntroDatOptionRow>(conn)
    .optional()?;
    let mut clrmamepro_positions = sql_query(
        "SELECT field_kind, source_order FROM no_intro_dat_clrmamepro_field_positions \
         WHERE snapshot_key = ? ORDER BY source_order, field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<NoIntroDatHeaderPositionRow>(conn)?;
    let mut romcenter_positions = sql_query(
        "SELECT field_kind, source_order FROM no_intro_dat_romcenter_field_positions \
         WHERE snapshot_key = ? ORDER BY source_order, field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<NoIntroDatHeaderPositionRow>(conn)?;
    let child_ranks = header_positions
        .iter()
        .map(|position| position.source_order)
        .chain(clrmamepro.as_ref().map(|options| options.source_order))
        .chain(romcenter.as_ref().map(|options| options.source_order))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .zip(0_i64..)
        .collect::<BTreeMap<_, _>>();
    let clrmamepro_ranks = local_position_ranks(&clrmamepro_positions);
    let romcenter_ranks = local_position_ranks(&romcenter_positions);
    normalize_no_intro_dat_positions(&mut header_positions, &child_ranks);
    normalize_no_intro_dat_positions(&mut clrmamepro_positions, &clrmamepro_ranks);
    normalize_no_intro_dat_positions(&mut romcenter_positions, &romcenter_ranks);
    let clrmamepro = clrmamepro.map(|mut options| {
        options.source_order = child_ranks
            .get(&options.source_order)
            .copied()
            .unwrap_or_default();
        serde_json::json!({
            "source_order": options.source_order,
            "forcenodump_text": options.first_text,
            "header_text": options.second_text,
            "field_positions": no_intro_dat_positions_json(&clrmamepro_positions),
        })
    });
    let romcenter = romcenter.map(|mut options| {
        options.source_order = child_ranks
            .get(&options.source_order)
            .copied()
            .unwrap_or_default();
        serde_json::json!({
            "source_order": options.source_order,
            "plugin_text": options.first_text,
            "field_positions": no_intro_dat_positions_json(&romcenter_positions),
        })
    });
    let header_fields = no_intro_dat_header_fields_json(&header, &header_positions);
    Ok(Some(serde_json::json!({
        "schema_location": header.schema_location,
        "header": header_fields,
        "clrmamepro": clrmamepro,
        "romcenter": romcenter,
    })))
}

fn no_intro_dat_header_fields_json(
    header: &NoIntroDatHeaderRow,
    positions: &[NoIntroDatHeaderPositionRow],
) -> serde_json::Value {
    serde_json::json!({
        "id_text": header.id_text,
        "name": header.name,
        "description": header.description,
        "version_text": header.version_text,
        "date": header.date,
        "author": header.author,
        "homepage": header.homepage,
        "url": header.url,
        "trademarks": header.trademarks,
        "piracy": header.piracy,
        "subset": header.subset,
        "comment": header.comment,
        "field_positions": no_intro_dat_positions_json(positions),
    })
}

fn normalize_no_intro_dat_positions(
    positions: &mut [NoIntroDatHeaderPositionRow],
    ranks: &BTreeMap<i64, i64>,
) {
    for position in positions {
        if let Some(rank) = ranks.get(&position.source_order) {
            position.source_order = *rank;
        }
    }
}

fn local_position_ranks(positions: &[NoIntroDatHeaderPositionRow]) -> BTreeMap<i64, i64> {
    positions
        .iter()
        .map(|position| position.source_order)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .zip(0_i64..)
        .collect()
}

fn no_intro_dat_positions_json(
    positions: &[NoIntroDatHeaderPositionRow],
) -> Vec<serde_json::Value> {
    positions
        .iter()
        .map(|position| {
            serde_json::json!({
                "field_kind": position.field_kind,
                "native_order": position.source_order,
            })
        })
        .collect()
}

fn load_cmp_header_metadata(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Option<serde_json::Value>> {
    let cmp_facts = sql_query(
        "SELECT name, description, version, date, author, email, homepage, url, comment, category, \
         source_block \
         FROM cmp_header_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CmpHeaderFactsRow>(conn)
    .optional()?;
    let cmp_directives = sql_query(
        "SELECT header_definition, forcemerging, forcezipping, forcepacking, forcenodump, \
         forcemerging_effective, forcezipping_effective, forcenodump_effective \
         FROM cmp_header_directives WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CmpHeaderDirectivesRow>(conn)
    .optional()?;
    let mut cmp_positions = sql_query(
        "SELECT field_kind, source_field, source_order, is_quoted \
         FROM cmp_header_field_positions WHERE snapshot_key = ? ORDER BY source_order, field_kind",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<CmpHeaderFieldPositionRow>(conn)?;
    normalize_cmp_header_positions(&mut cmp_positions);
    Ok(cmp_facts.map(|facts| {
        serde_json::json!({
            "name": facts.name,
            "description": facts.description,
            "version": facts.version,
            "date": facts.date,
            "author": facts.author,
            "email": facts.email,
            "homepage": facts.homepage,
            "url": facts.url,
            "comment": facts.comment,
            "category": facts.category,
            "source_block": facts.source_block,
            "directives": cmp_directives.map(|directives| serde_json::json!({
                "header_definition": directives.header_definition,
                "forcemerging": directives.forcemerging,
                "forcezipping": directives.forcezipping,
                "forcepacking": directives.forcepacking,
                "forcenodump": directives.forcenodump,
                "forcemerging_effective": directives.forcemerging_effective,
                "forcezipping_effective": directives.forcezipping_effective,
                "forcenodump_effective": directives.forcenodump_effective,
            })),
            "field_positions": cmp_positions.iter().map(|position| serde_json::json!({
                "field_kind": position.field_kind,
                "source_field": position.source_field,
                "native_order": position.source_order,
                "is_quoted": position.is_quoted,
            })).collect::<Vec<_>>(),
        })
    }))
}

fn load_cmp_document_layout(
    conn: &mut diesel::SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Vec<serde_json::Value>> {
    let rows = sql_query(
        "WITH root_sets AS (
             SELECT sets.set_id, sets.set_name FROM catalog_set_groups AS groups \
             JOIN catalog_sets AS sets USING (set_group_id) \
             WHERE groups.snapshot_key = ? AND groups.kind = 'root'
         )
         SELECT 'header' AS layout_kind, NULL AS set_name, source_block, source_order AS document_order \
         FROM cmp_header_facts WHERE snapshot_key = ?
         UNION ALL
         SELECT 'set' AS layout_kind, root_sets.set_name, facts.source_block, \
                facts.document_order \
         FROM cmp_set_facts AS facts JOIN root_sets ON root_sets.set_id = facts.record_id \
         ORDER BY document_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .load::<CmpDocumentLayoutRow>(conn)?;
    // Compare document structure, never per-owner facts or IDs. Indistinguishable
    // repeated-name owners can permute without inventing snapshot correspondence.
    let ranks = rows
        .iter()
        .map(|row| row.document_order)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .zip(0_i64..)
        .collect::<BTreeMap<_, _>>();
    Ok(rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "kind": row.layout_kind,
                "set_name": row.set_name,
                "source_block": row.source_block,
                "native_order": ranks.get(&row.document_order),
            })
        })
        .collect())
}

fn normalize_cmp_header_positions(positions: &mut [CmpHeaderFieldPositionRow]) {
    let ranks = positions
        .iter()
        .map(|position| position.source_order)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .zip(0_i64..)
        .collect::<BTreeMap<_, _>>();
    for position in positions {
        if let Some(rank) = ranks.get(&position.source_order) {
            position.source_order = *rank;
        }
    }
}

fn normalize_header_order(
    positions: &mut [LogiqxTextPositionRow],
    clrmamepro: Option<&mut LogiqxClrMameProOptionsRow>,
    romcenter: Option<&mut LogiqxRomCenterOptionsRow>,
) {
    let orders = positions
        .iter()
        .map(|position| position.source_order)
        .chain(clrmamepro.as_ref().map(|options| options.source_order))
        .chain(romcenter.as_ref().map(|options| options.source_order))
        .collect::<BTreeSet<_>>();
    let ranks = orders.into_iter().zip(0_i64..).collect::<BTreeMap<_, _>>();
    for position in positions {
        if let Some(rank) = ranks.get(&position.source_order) {
            position.source_order = *rank;
        }
    }
    if let Some(options) = clrmamepro
        && let Some(rank) = ranks.get(&options.source_order)
    {
        options.source_order = *rank;
    }
    if let Some(options) = romcenter
        && let Some(rank) = ranks.get(&options.source_order)
    {
        options.source_order = *rank;
    }
}

fn is_software_list_snapshot(snapshot: &SnapshotRow) -> bool {
    snapshot
        .format_hint
        .as_deref()
        .is_some_and(|hint| hint.split('+').next() == Some("mame-softwarelist-xml"))
}

fn relationship_evidence_by_set(
    explanations: Vec<RelationshipExplanation>,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> BTreeMap<String, Vec<RelationshipExplanation>> {
    let mut evidence_by_set = BTreeMap::<String, Vec<RelationshipExplanation>>::new();
    for explanation in explanations {
        let mut names = BTreeSet::new();
        for endpoint in [&explanation.claim.subject, &explanation.claim.target] {
            if let RelationshipEndpoint::CatalogRecord(record) = endpoint
                && (&record.snapshot == previous || &record.snapshot == current)
                && matches!(
                    record.kind,
                    crate::domain::CatalogRecordKind::Set
                        | crate::domain::CatalogRecordKind::SoftwareItem
                )
            {
                names.insert(record.key.as_str().to_owned());
            }
        }
        for name in names {
            evidence_by_set
                .entry(name)
                .or_default()
                .push(explanation.clone());
        }
    }
    evidence_by_set
}

pub fn history(pool: &Pool, catalog: &CatalogKey) -> crate::Result<Vec<CatalogSnapshotEntry>> {
    let mut conn = pool.get()?;
    let rows = sql_query(
        "SELECT snapshot.snapshot_key, snapshot.document_key, \
                versions.declared_version, \
                snapshot.coverage_id \
         FROM catalog_snapshots AS snapshot \
         JOIN catalog_snapshot_versions AS versions USING (snapshot_key) \
         WHERE snapshot.catalog_key = ? \
         ORDER BY snapshot.document_key, snapshot.interpretation_key, snapshot.snapshot_key",
    )
    .bind::<Text, _>(catalog.as_str())
    .load::<HistoryRow>(&mut conn)?;
    rows.into_iter()
        .map(|row| {
            let scope = super::catalog_coverage::load(
                &mut conn,
                CoverageId::from_database(row.coverage_id),
            )?;
            Ok(CatalogSnapshotEntry {
                snapshot: SnapshotKey::from_persisted(row.snapshot_key),
                document_key: row.document_key,
                declared_version: row.declared_version,
                scope,
            })
        })
        .collect()
}

fn snapshot(conn: &mut diesel::SqliteConnection, key: &SnapshotKey) -> crate::Result<SnapshotRow> {
    let row = sql_query(
        "SELECT snapshot.catalog_key, snapshot.coverage_id, documents.format_hint, \
                EXISTS (SELECT 1 FROM snapshot_publications AS publication \
                        WHERE publication.snapshot_key = snapshot.snapshot_key) AS published \
         FROM catalog_snapshots AS snapshot \
         JOIN documents USING (document_key) WHERE snapshot.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .get_result::<SnapshotHeaderRow>(conn)
    .map_err(|error| match error {
        diesel::result::Error::NotFound => {
            crate::Error::InvalidPath(format!("catalog snapshot {} does not exist", key.as_str()))
        }
        error => error.into(),
    })?;
    let scope = super::catalog_coverage::load(conn, CoverageId::from_database(row.coverage_id))?;
    Ok(SnapshotRow {
        catalog_key: row.catalog_key,
        scope,
        format_hint: row.format_hint,
        published: row.published,
    })
}

fn comparable_scope(previous: &SnapshotRow, current: &SnapshotRow) -> bool {
    if !previous.published || !current.published {
        return false;
    }
    match (&previous.scope, &current.scope) {
        (CatalogScope::Complete, CatalogScope::Complete) => true,
        (CatalogScope::Filtered(previous), CatalogScope::Filtered(current)) => previous == current,
        _ => false,
    }
}

fn absence_status(
    previous: &SnapshotRow,
    current: &SnapshotRow,
    name: &str,
) -> SnapshotRecordStatus {
    let Some(member) = qualified_history_member(previous, name) else {
        return SnapshotRecordStatus::Unknown;
    };
    let explicitly_excluded = [previous, current].into_iter().any(|snapshot| {
        snapshot.published
            && matches!(&snapshot.scope, CatalogScope::Filtered(members) if !members.contains(&member))
    });
    if explicitly_excluded {
        SnapshotRecordStatus::OutOfScope
    } else {
        SnapshotRecordStatus::Unknown
    }
}

fn scopes_cover_set(previous: &SnapshotRow, current: &SnapshotRow, name: &str) -> bool {
    snapshot_covers_set(previous, name) && snapshot_covers_set(current, name)
}

fn snapshot_covers_set(snapshot: &SnapshotRow, name: &str) -> bool {
    if !snapshot.published {
        return false;
    }
    let Some(member) = qualified_history_member(snapshot, name) else {
        return false;
    };
    match &snapshot.scope {
        CatalogScope::Complete => true,
        CatalogScope::Filtered(members) => members.contains(&member),
        CatalogScope::Partial(members) => members.get(&member) == Some(&SetCoverage::Covered),
        CatalogScope::Unknown => false,
    }
}

fn qualified_history_member(snapshot: &SnapshotRow, name: &str) -> Option<QualifiedCatalogSet> {
    if is_software_list_snapshot(snapshot) {
        let (list_name, item_name): (String, String) = serde_json::from_str(name).ok()?;
        Some(QualifiedCatalogSet::SoftwareItem {
            list_name,
            name: SetName::new(item_name),
        })
    } else {
        Some(QualifiedCatalogSet::RootSet(SetName::new(name)))
    }
}

fn records(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<CatalogRecords> {
    let mame_child_ranks = load_mame_child_ranks(conn, key)?;
    let sets = sql_query(
        "SELECT sets.set_id, sets.set_name, parents.parent_name, sets.list_order AS source_order \
         FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING (set_group_id) \
         JOIN snapshot_sets AS parents USING (set_id) \
         WHERE groups.snapshot_key = ? AND groups.kind = 'root' \
         ORDER BY sets.set_name, sets.list_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<SetRow>(conn)?;
    let requirements = load_requirements(conn, key)?;
    let machine_switches = mame_switch_facts(conn, key)?;
    let machine_bios_sets = mame_bios_set_facts(conn, key)?;
    let mame_machine_facts = load_mame_machine_facts(conn, key)?;
    let mame_machine_specification_facts = load_mame_machine_specification_facts(conn, key)?;
    let mame_machine_dependencies = load_mame_machine_dependencies(conn, key)?;
    let no_intro_game_facts = load_no_intro_game_facts(conn, key)?;
    let no_intro_dat_child_ranks = load_no_intro_dat_child_ranks(conn, key)?;
    let no_intro_dat_game_facts =
        load_no_intro_dat_game_facts(conn, key, &no_intro_dat_child_ranks)?;
    let no_intro_dat_categories =
        load_no_intro_dat_categories(conn, key, &no_intro_dat_child_ranks)?;
    let no_intro_dat_identifiers =
        load_no_intro_dat_identifiers(conn, key, &no_intro_dat_child_ranks)?;
    let no_intro_dat_releases = load_no_intro_dat_releases(conn, key, &no_intro_dat_child_ranks)?;
    let no_intro_dat_rom_positions = load_no_intro_dat_rom_positions(conn, key)?;
    let no_intro_dat_rom_order = no_intro_dat_rom_order(
        &requirements,
        &no_intro_dat_rom_positions,
        &no_intro_dat_child_ranks,
    );
    let no_intro_database_games = no_intro_database::load_games(conn, key)?;
    let logiqx_set_facts = load_logiqx_set_facts(conn, key)?;
    let logiqx_text_positions = load_logiqx_text_positions(conn, key)?;
    let logiqx_game_comments = load_logiqx_game_comments(conn, key)?;
    let logiqx_releases = load_logiqx_releases(conn, key)?;
    let logiqx_bios_sets = load_logiqx_bios_sets(conn, key)?;
    let logiqx_archive_references = load_logiqx_archive_references(conn, key)?;
    let cmp_native_ranks = cmp_set_native_ranks(conn, key)?;
    let cmp_set_facts = load_cmp_set_facts(conn, key, &cmp_native_ranks)?;

    let mut result = CatalogRecords::default();
    for set in sets {
        result
            .sets
            .entry(set.set_name.clone())
            .or_default()
            .push(set);
    }
    for owners in result.sets.values_mut() {
        owners.sort_by_key(|owner| owner.source_order);
    }
    result.machine_switches = machine_switches;
    result.machine_bios_sets = machine_bios_sets;
    result.mame_machine_facts = mame_machine_facts;
    result.mame_machine_specification_facts = mame_machine_specification_facts;
    result.mame_machine_dependencies = mame_machine_dependencies;
    result.no_intro_game_facts = no_intro_game_facts;
    result.no_intro_dat_game_facts = no_intro_dat_game_facts;
    result.no_intro_dat_categories = no_intro_dat_categories;
    result.no_intro_dat_identifiers = no_intro_dat_identifiers;
    result.no_intro_dat_releases = no_intro_dat_releases;
    result.no_intro_dat_rom_order = no_intro_dat_rom_order;
    result.no_intro_database_games = no_intro_database_games;
    result.logiqx_set_facts = logiqx_set_facts;
    result.logiqx_text_positions = logiqx_text_positions;
    result.logiqx_game_comments = logiqx_game_comments;
    result.logiqx_releases = logiqx_releases;
    result.logiqx_bios_sets = logiqx_bios_sets;
    result.logiqx_archive_references = logiqx_archive_references;
    result.cmp_set_facts = cmp_set_facts;
    sort_json_groups(&mut result.machine_switches);
    sort_json_groups(&mut result.machine_bios_sets);
    sort_json_groups(&mut result.mame_machine_dependencies);
    sort_json_groups(&mut result.mame_machine_facts);
    sort_json_groups(&mut result.mame_machine_specification_facts);
    sort_json_groups(&mut result.no_intro_game_facts);
    sort_json_groups(&mut result.logiqx_set_facts);
    sort_json_groups(&mut result.logiqx_game_comments);
    sort_json_groups(&mut result.logiqx_releases);
    sort_json_groups(&mut result.logiqx_bios_sets);
    sort_json_groups(&mut result.logiqx_archive_references);
    sort_json_groups(&mut result.cmp_set_facts);
    let cmp_positions = load_cmp_rom_positions(conn, key, &cmp_native_ranks)?;
    assemble_requirements(
        &mut result,
        requirements,
        &cmp_positions,
        &no_intro_dat_rom_positions,
    );
    normalize_logiqx_child_order(&mut result);
    normalize_mame_child_order(&mut result, &mame_child_ranks);
    software::load_records(conn, key, &mut result)?;
    Ok(result)
}

/// Compare native-child ordering without treating vendor-only gaps as edits.
/// The persisted XML ordinals remain unchanged for source provenance.
fn normalize_logiqx_child_order(records: &mut CatalogRecords) {
    let mut orders = BTreeMap::<i64, BTreeSet<i64>>::new();
    for family in [
        &records.logiqx_game_comments,
        &records.logiqx_text_positions,
        &records.logiqx_releases,
        &records.logiqx_bios_sets,
        &records.logiqx_archive_references,
    ] {
        for (set_id, facts) in family {
            for fact in facts {
                if let Some(order) = fact["source_order"].as_i64() {
                    orders.entry(*set_id).or_default().insert(order);
                }
            }
        }
    }
    for (set_id, assets) in &records.requirements {
        for fact in assets.values().flatten() {
            if let Some(order) = fact["logiqx_attributes"]["source_order"].as_i64() {
                orders.entry(*set_id).or_default().insert(order);
            }
        }
    }
    let ranks: BTreeMap<_, BTreeMap<_, _>> = orders
        .into_iter()
        .map(|(set_id, orders)| (set_id, orders.into_iter().zip(0_i64..).collect()))
        .collect();
    for family in [
        &mut records.logiqx_game_comments,
        &mut records.logiqx_text_positions,
        &mut records.logiqx_releases,
        &mut records.logiqx_bios_sets,
        &mut records.logiqx_archive_references,
    ] {
        for (set_id, facts) in family {
            for fact in facts.iter_mut() {
                normalize_native_order(fact, *set_id, &ranks);
            }
            facts.sort_by_key(serde_json::Value::to_string);
        }
    }
    for (set_id, assets) in &mut records.requirements {
        for fact in assets.values_mut().flatten() {
            normalize_native_order(&mut fact["logiqx_attributes"], *set_id, &ranks);
        }
        for facts in assets.values_mut() {
            facts.sort_by_key(serde_json::Value::to_string);
        }
    }
}

fn load_mame_child_ranks(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, BTreeMap<i64, i64>>> {
    let rows = sql_query(concat!(
        "WITH requested_mame_machines(set_id) AS (SELECT set_id FROM snapshot_sets WHERE snapshot_key=?), \
         positions AS (",
        include_str!("db/mame_positions.sql"),
        ") SELECT set_id,source_order,child_kind,child_order FROM positions \
         ORDER BY set_id,source_order,child_kind,child_order"
    ))
    .bind::<Text, _>(key.as_str())
    .load::<MameChildPositionRow>(conn)?;
    let mut positions = BTreeMap::<i64, Vec<(i64, String, i64)>>::new();
    for row in rows {
        positions.entry(row.set_id).or_default().push((
            row.source_order,
            row.child_kind,
            row.child_order,
        ));
    }
    Ok(positions
        .into_iter()
        .map(|(set_id, mut positions)| {
            positions.sort();
            let mut ranks = BTreeMap::new();
            for ((source_order, _, _), rank) in positions.into_iter().zip(0_i64..) {
                ranks.entry(source_order).or_insert(rank);
            }
            (set_id, ranks)
        })
        .collect())
}

/// Compare MAME child ordering using ranks among known native children only.
/// Vendor-only XML children therefore do not shift semantic positions.
fn normalize_mame_child_order(
    records: &mut CatalogRecords,
    ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) {
    for family in [
        &mut records.machine_switches,
        &mut records.machine_bios_sets,
        &mut records.mame_machine_dependencies,
    ] {
        for (set_id, facts) in family {
            for fact in facts.iter_mut() {
                normalize_native_order(fact, *set_id, ranks);
            }
            facts.sort_by_key(serde_json::Value::to_string);
        }
    }

    for (set_id, facts) in &mut records.mame_machine_facts {
        for fact in facts.iter_mut() {
            for field in ["description", "year", "manufacturer"] {
                let source_field = format!("{field}_source_order");
                let native_field = format!("{field}_native_order");
                let source_order = fact.get(&source_field).and_then(serde_json::Value::as_i64);
                let native_order = source_order
                    .and_then(|order| ranks.get(set_id)?.get(&order))
                    .copied();
                if let Some(fields) = fact.as_object_mut() {
                    fields.remove(&source_field);
                    fields.insert(native_field, native_order.into());
                }
            }
        }
        facts.sort_by_key(serde_json::Value::to_string);
    }

    for (set_id, facts) in &mut records.mame_machine_specification_facts {
        for fact in facts.iter_mut() {
            if fact.get("element_order").is_some() {
                let source_order = fact["element_order"].as_i64();
                let native_order = source_order
                    .and_then(|order| ranks.get(set_id)?.get(&order))
                    .copied();
                if let Some(fields) = fact.as_object_mut() {
                    fields.remove("element_order");
                    fields.insert("native_order".into(), native_order.into());
                }
            }
            if fact.get("conditions").is_some()
                && let Some(conditions) = fact["conditions"].as_array_mut()
            {
                for condition in conditions {
                    let source_order = condition["element_order"].as_i64();
                    if let Some(native_order) = source_order
                        .and_then(|order| ranks.get(set_id)?.get(&order))
                        .copied()
                    {
                        condition["element_order"] = native_order.into();
                    }
                }
            }
        }
        facts.sort_by_key(serde_json::Value::to_string);
    }

    for (set_id, assets) in &mut records.requirements {
        for fact in assets.values_mut().flatten() {
            normalize_native_order(&mut fact["mame_attributes"], *set_id, ranks);
        }
        for facts in assets.values_mut() {
            facts.sort_by_key(serde_json::Value::to_string);
        }
    }
}

fn normalize_native_order(
    fact: &mut serde_json::Value,
    set_id: i64,
    ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) {
    let rank = fact["source_order"]
        .as_i64()
        .and_then(|order| ranks.get(&set_id)?.get(&order))
        .copied();
    if let Some(fields) = fact.as_object_mut() {
        fields.remove("source_order");
        fields.insert("native_order".into(), rank.into());
    }
}

const ROOT_REQUIREMENTS_SQL: &str = "SELECT occurrence.occurrence_id, sets.set_id, asset.asset_name, asset.role, asset.size, \
         (SELECT digest.digest FROM asset_requirement_usable_digests AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = asset.set_id AND assertion.provenance = 'source_declared' \
            AND assertion.component_order = asset.component_order \
            AND assertion.scope = asset.evidence_scope AND digest.algorithm = 'crc32') AS crc, \
         (SELECT digest.digest FROM asset_requirement_usable_digests AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = asset.set_id AND assertion.provenance = 'source_declared' \
            AND assertion.component_order = asset.component_order \
            AND assertion.scope = asset.evidence_scope AND digest.algorithm = 'md5') AS md5, \
         (SELECT digest.digest FROM asset_requirement_usable_digests AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = asset.set_id AND assertion.provenance = 'source_declared' \
            AND assertion.component_order = asset.component_order \
            AND assertion.scope = asset.evidence_scope AND digest.algorithm = 'sha1') AS sha1, \
         (SELECT digest.digest FROM asset_requirement_usable_digests AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = asset.set_id AND assertion.provenance = 'source_declared' \
            AND assertion.component_order = asset.component_order \
            AND assertion.scope = asset.evidence_scope \
            AND assertion.scope IN ('whole_asset', 'whole_file') \
            AND digest.algorithm = 'sha256') AS sha256, \
         asset.evidence_scope, asset.evidence_provenance, \
         asset.merge_name, asset.dump_status, asset.serial, asset.date, \
         facts.region AS mame_region, facts.bios AS mame_bios, \
         mame_rom.size_text AS mame_size_text, mame_rom.crc_text AS mame_crc_text, \
         COALESCE(mame_rom.sha1_text, mame_disk.sha1_text) AS mame_sha1_text, \
         mame_rom_raw.md5_text AS mame_md5_text, facts.offset AS mame_offset_text, \
         facts.source_order AS mame_source_order, \
         facts.status_specified AS mame_status_specified, \
         facts.optional AS mame_optional, mame_rom_compat.sound_only AS mame_sound_only, \
         facts.optional_specified AS mame_optional_specified, \
         mame_rom_compat.dispose AS mame_dispose, mame_rom_compat.load_flag AS mame_load_flag, \
         mame_rom_compat.value AS mame_value, \
         mame_rom_compat.inverted AS mame_inverted, mame_rom_compat.ovha AS mame_ovha, \
         mame_rom_compat.no_thread AS mame_no_thread, \
         facts.disk_index AS mame_disk_index, facts.writable AS mame_writable, \
         facts.writable_specified AS mame_writable_specified, \
         mame_disk_compat.writeable AS mame_writeable, rom_claim.size_text AS logiqx_size_text, \
         rom_claim.crc_text AS logiqx_crc_text, \
         COALESCE(rom_claim.md5_text, disk_claim.md5_text) AS logiqx_md5_text, \
         COALESCE(rom_claim.sha1_text, disk_claim.sha1_text) AS logiqx_sha1_text, \
         COALESCE(rom_claim.status_was_present, disk_claim.status_was_present) AS logiqx_status_was_present, \
         COALESCE(rom_claim.source_order, disk_claim.source_order, sample_claim.source_order) AS logiqx_source_order, \
         cmp.size_text AS cmp_size_text, cmp.crc_text AS cmp_crc_text, cmp.crc32_text AS cmp_crc32_text, \
         cmp.md5_text AS cmp_md5_text, cmp.sha1_text AS cmp_sha1_text, cmp.status_text AS cmp_status_text, \
         cmp.nodump_present AS cmp_nodump_present, cmp.baddump_present AS cmp_baddump_present, \
         dat_rom.size_text AS no_intro_dat_size_text, \
         dat_rom.source_order AS no_intro_dat_source_order, \
         CASE WHEN dat_crc_field.occurrence_id IS NULL THEN NULL \
              WHEN dat_crc_value.digest_id IS NOT NULL THEN lower(hex(dat_crc_value.digest)) \
              ELSE dat_crc_field.invalid_text END AS no_intro_dat_crc_text, \
         CASE WHEN dat_md5_field.occurrence_id IS NULL THEN NULL \
              WHEN dat_md5_value.digest_id IS NOT NULL THEN lower(hex(dat_md5_value.digest)) \
              ELSE dat_md5_field.invalid_text END AS no_intro_dat_md5_text, \
         CASE WHEN dat_sha1_field.occurrence_id IS NULL THEN NULL \
              WHEN dat_sha1_value.digest_id IS NOT NULL THEN lower(hex(dat_sha1_value.digest)) \
              ELSE dat_sha1_field.invalid_text END AS no_intro_dat_sha1_text, \
         CASE WHEN dat_sha256_field.occurrence_id IS NULL THEN NULL \
              WHEN dat_sha256_value.digest_id IS NOT NULL THEN lower(hex(dat_sha256_value.digest)) \
              ELSE dat_sha256_field.invalid_text END AS no_intro_dat_sha256_text, \
         dat_rom.status_text AS no_intro_dat_status_text, \
         dat_rom.serial_text AS no_intro_dat_serial_text, dat_rom.header_text AS no_intro_dat_header_text, \
         dat_rom.date_text AS no_intro_dat_date_text, dat_rom.mia_text AS no_intro_dat_mia_text \
         FROM asset_requirement_rows AS asset \
         JOIN snapshot_sets AS sets USING (set_id) \
         JOIN asset_occurrences AS occurrence \
           ON occurrence.record_id = asset.set_id \
         AND occurrence.occurrence_order = asset.component_order \
         LEFT JOIN cmp_rom_claims AS cmp USING (occurrence_id) \
         LEFT JOIN no_intro_dat_rom_claims AS dat_rom USING (occurrence_id) \
         LEFT JOIN no_intro_dat_rom_digest_fields AS dat_crc_field \
           ON dat_crc_field.occurrence_id = occurrence.occurrence_id AND dat_crc_field.field_kind = 2 \
         LEFT JOIN digest_values AS dat_crc_value ON dat_crc_value.digest_id = dat_crc_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS dat_md5_field \
           ON dat_md5_field.occurrence_id = occurrence.occurrence_id AND dat_md5_field.field_kind = 3 \
         LEFT JOIN digest_values AS dat_md5_value ON dat_md5_value.digest_id = dat_md5_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS dat_sha1_field \
           ON dat_sha1_field.occurrence_id = occurrence.occurrence_id AND dat_sha1_field.field_kind = 4 \
         LEFT JOIN digest_values AS dat_sha1_value ON dat_sha1_value.digest_id = dat_sha1_field.digest_id \
         LEFT JOIN no_intro_dat_rom_digest_fields AS dat_sha256_field \
           ON dat_sha256_field.occurrence_id = occurrence.occurrence_id AND dat_sha256_field.field_kind = 5 \
         LEFT JOIN digest_values AS dat_sha256_value ON dat_sha256_value.digest_id = dat_sha256_field.digest_id \
         LEFT JOIN logiqx_rom_claims AS rom_claim \
           ON rom_claim.occurrence_id = occurrence.occurrence_id AND asset.role = 'rom' \
         LEFT JOIN logiqx_disk_claims AS disk_claim \
           ON disk_claim.occurrence_id = occurrence.occurrence_id AND asset.role = 'disk' \
         LEFT JOIN logiqx_sample_claims AS sample_claim \
           ON sample_claim.occurrence_id = occurrence.occurrence_id AND asset.role = 'other' \
         LEFT JOIN mame_asset_facts AS facts \
           ON facts.set_id = asset.set_id \
          AND facts.component_order = asset.component_order \
         LEFT JOIN mame_rom_claims AS mame_rom \
           ON mame_rom.occurrence_id = occurrence.occurrence_id AND asset.role = 'rom' \
         LEFT JOIN mame_rom_compatibility AS mame_rom_raw \
           ON mame_rom_raw.occurrence_id = occurrence.occurrence_id \
         LEFT JOIN mame_disk_claims AS mame_disk \
           ON mame_disk.occurrence_id = occurrence.occurrence_id AND asset.role = 'disk' \
         LEFT JOIN catalog_snapshots AS mame_snapshot \
           ON mame_snapshot.snapshot_key = sets.snapshot_key \
         LEFT JOIN parser_interpretations AS mame_interpretation \
           ON mame_interpretation.interpretation_key = mame_snapshot.interpretation_key \
          AND mame_interpretation.rules_version = 'mame-observed-compat-declared-text-v1' \
         LEFT JOIN mame_rom_compatibility AS mame_rom_compat \
           ON mame_rom_compat.occurrence_id = occurrence.occurrence_id \
          AND mame_interpretation.interpretation_key IS NOT NULL \
         LEFT JOIN mame_disk_compatibility AS mame_disk_compat \
           ON mame_disk_compat.occurrence_id = occurrence.occurrence_id \
          AND mame_interpretation.interpretation_key IS NOT NULL \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, asset.asset_name, asset.component_order";

fn load_requirements(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<Vec<RequirementRow>> {
    Ok(sql_query(ROOT_REQUIREMENTS_SQL)
        .bind::<Text, _>(key.as_str())
        .load::<RequirementRow>(conn)?)
}

fn load_mame_machine_dependencies(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(super::machine_dependencies::MAME_DEPENDENCIES)
        .bind::<Text, _>(key.as_str())
        .load::<MameMachineDependencyRow>(conn)?;
    let mut facts = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        facts
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "order": row.dependency_order,
                "source_order": row.source_order,
                "kind": row.dependency_kind,
                "target": row.target_name,
                "reference_tag": row.reference_tag,
            }));
    }
    Ok(facts)
}

#[derive(QueryableByName)]
struct LogiqxSetTextPositionRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

fn load_logiqx_text_positions(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT position.set_id, position.field_kind, position.source_order \
         FROM catalog_set_groups JOIN catalog_sets USING (set_group_id) \
         JOIN logiqx_game_text_positions AS position USING (set_id) \
         WHERE snapshot_key = ? ORDER BY position.set_id, position.field_kind",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxSetTextPositionRow>(conn)?;
    let mut result = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        result
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "field_kind": row.field_kind,
                "source_order": row.source_order,
            }));
    }
    Ok(result)
}

fn load_logiqx_set_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT set_id, source_file, is_bios, is_bios_was_present, board, rebuild_to, description, year, manufacturer \
         FROM logiqx_set_facts WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxSetFactsRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "source_file": row.source_file,
                "is_bios": row.is_bios,
                "is_bios_was_present": row.is_bios_was_present,
                "board": row.board,
                "rebuild_to": row.rebuild_to,
                "description": row.description,
                "year": row.year,
                "manufacturer": row.manufacturer,
            }));
    }
    Ok(grouped)
}

fn load_logiqx_game_comments(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT comments.set_id, comments.comment_order, comments.source_order, comments.comment_text \
         FROM logiqx_game_comments AS comments JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, comments.source_order, comments.comment_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxGameCommentRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "comment_order": row.comment_order,
                "source_order": row.source_order,
                "comment": row.comment_text,
            }));
    }
    Ok(grouped)
}

fn load_logiqx_releases(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT releases.set_id, releases.release_order, releases.source_order, releases.name, \
         releases.region, releases.language, releases.date, releases.\"default\", \
         releases.default_was_present \
         FROM logiqx_releases AS releases JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, releases.source_order, releases.release_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxReleaseRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "release_order": row.release_order,
                "source_order": row.source_order,
                "name": row.name,
                "region": row.region,
                "language": row.language,
                "date": row.date,
                "default": row.default,
                "default_was_present": row.default_was_present,
            }));
    }
    Ok(grouped)
}

fn load_logiqx_bios_sets(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT bios.set_id, bios.bios_order, bios.source_order, bios.name, bios.description, \
         bios.is_default, bios.default_was_present \
         FROM logiqx_bios_sets AS bios JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, bios.source_order, bios.bios_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxBiosSetRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "bios_order": row.bios_order,
                "source_order": row.source_order,
                "name": row.name,
                "description": row.description,
                "is_default": row.is_default,
                "default_was_present": row.default_was_present,
            }));
    }
    Ok(grouped)
}

fn load_logiqx_archive_references(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT archives.set_id, archives.archive_order, archives.source_order, archives.archive_name \
         FROM logiqx_archive_references AS archives JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, archives.source_order, archives.archive_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<LogiqxArchiveReferenceRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "archive_order": row.archive_order,
                "source_order": row.source_order,
                "archive_name": row.archive_name,
            }));
    }
    Ok(grouped)
}

fn load_cmp_set_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    native_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT sets.set_id, facts.description, facts.year, facts.manufacturer, facts.rebuildto, \
                facts.region, facts.release_year_text, facts.release_month_text, \
                facts.release_day_text, facts.serial, facts.source_block \
         FROM cmp_set_facts AS facts JOIN snapshot_sets AS sets ON sets.set_id = facts.record_id \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, facts.record_id",
    )
    .bind::<Text, _>(key.as_str())
    .load::<CmpSetFactsRow>(conn)?;
    let positions = sql_query(
        "SELECT positions.record_id, positions.field_kind, positions.source_field, \
                positions.source_order, positions.is_quoted \
         FROM cmp_set_field_positions AS positions \
         JOIN snapshot_sets AS sets ON sets.set_id = positions.record_id \
         WHERE sets.snapshot_key = ? ORDER BY positions.record_id, positions.source_order, positions.field_kind",
    )
    .bind::<Text, _>(key.as_str())
    .load::<CmpSetFieldPositionRow>(conn)?;
    let mut field_positions = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    let mut sampleof_positions = BTreeMap::<i64, serde_json::Value>::new();
    for position in positions {
        let rank = native_ranks
            .get(&position.record_id)
            .and_then(|ranks| ranks.get(&position.source_order))
            .copied();
        let field_position = serde_json::json!({
            "field_kind": position.field_kind,
            "source_field": position.source_field,
            "native_order": rank,
            "is_quoted": position.is_quoted,
        });
        if position.field_kind == 6 {
            sampleof_positions.insert(position.record_id, field_position.clone());
        }
        field_positions
            .entry(position.record_id)
            .or_default()
            .push(field_position);
    }
    let mut sample_facts = load_cmp_samples(conn, key, native_ranks)?;
    let mut sampleof = sql_query(
        "SELECT links.record_id, links.target_name \
         FROM cmp_sample_parent_links AS links JOIN snapshot_sets AS sets ON sets.set_id = links.record_id \
         WHERE sets.snapshot_key = ? ORDER BY links.record_id",
    )
    .bind::<Text, _>(key.as_str())
    .load::<CmpSampleParentPositionRow>(conn)?
    .into_iter()
    .map(|parent| {
        let mut value = sampleof_positions
            .remove(&parent.record_id)
            .unwrap_or_else(|| serde_json::json!({}));
        value["target_name"] = serde_json::Value::String(parent.target_name);
        (parent.record_id, value)
    })
    .collect::<BTreeMap<_, _>>();
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "description": row.description,
                "year": row.year,
                "manufacturer": row.manufacturer,
                "rebuildto": row.rebuildto,
                "region": row.region,
                "release_year_text": row.release_year_text,
                "release_month_text": row.release_month_text,
                "release_day_text": row.release_day_text,
                "serial": row.serial,
                "source_block": row.source_block,
                "field_positions": field_positions.remove(&row.set_id).unwrap_or_default(),
                "sampleof": sampleof.remove(&row.set_id),
                "samples": sample_facts.remove(&row.set_id).unwrap_or_default(),
            }));
    }
    Ok(grouped)
}

fn load_cmp_samples(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    native_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let samples = sql_query(
        "SELECT occurrence.record_id, \
                ROW_NUMBER() OVER (PARTITION BY occurrence.record_id ORDER BY occurrence.occurrence_order)-1 AS sample_order, \
                samples.sample_name, samples.source_field, \
                samples.source_order, samples.is_quoted \
         FROM cmp_samples AS samples JOIN asset_occurrences AS occurrence USING(occurrence_id) \
         JOIN snapshot_sets AS sets ON sets.set_id = occurrence.record_id \
         WHERE sets.snapshot_key = ? ORDER BY occurrence.record_id, occurrence.occurrence_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<CmpSamplePositionRow>(conn)?;
    let mut sample_facts = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for sample in samples {
        let rank = native_ranks
            .get(&sample.record_id)
            .and_then(|ranks| ranks.get(&sample.source_order))
            .copied();
        sample_facts
            .entry(sample.record_id)
            .or_default()
            .push(serde_json::json!({
                "sample_order": sample.sample_order,
                "sample_name": sample.sample_name,
                "source_field": sample.source_field,
                "native_order": rank,
                "is_quoted": sample.is_quoted,
            }));
    }
    Ok(sample_facts)
}

fn cmp_set_native_ranks(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, BTreeMap<i64, i64>>> {
    let rows = sql_query(
        "SELECT positions.record_id AS set_id, positions.source_order \
         FROM cmp_set_field_positions AS positions JOIN snapshot_sets AS sets ON sets.set_id = positions.record_id \
         WHERE sets.snapshot_key = ? \
         UNION ALL SELECT occurrence.record_id AS set_id, samples.source_order \
         FROM cmp_samples AS samples JOIN asset_occurrences AS occurrence USING(occurrence_id) \
         JOIN snapshot_sets AS sets ON sets.set_id = occurrence.record_id \
         WHERE sets.snapshot_key = ? \
         UNION ALL SELECT occurrences.record_id AS set_id, positions.source_order \
         FROM cmp_set_rom_positions AS positions JOIN asset_occurrences AS occurrences USING (occurrence_id) \
         JOIN snapshot_sets AS sets ON sets.set_id = occurrences.record_id WHERE sets.snapshot_key = ? \
         ORDER BY set_id, source_order",
    )
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(key.as_str())
    .load::<CmpNativeOrderRow>(conn)?;
    let mut orders = BTreeMap::<i64, BTreeSet<i64>>::new();
    for row in rows {
        orders
            .entry(row.set_id)
            .or_default()
            .insert(row.source_order);
    }
    Ok(orders
        .into_iter()
        .map(|(set_id, orders)| {
            (
                set_id,
                orders.into_iter().zip(0_i64..).collect::<BTreeMap<_, _>>(),
            )
        })
        .collect())
}

fn load_no_intro_game_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut facts = BTreeMap::<i64, NoIntroGameFactsRow>::new();
    for row in sql_query(
        "SELECT set_id, archive_id, description, name_alt, region, version, \
                bios_text, languages_present FROM no_intro_game_facts \
         WHERE snapshot_key = ? ORDER BY set_name, set_id",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroGameFactsRow>(conn)?
    {
        facts.insert(row.set_id, row);
    }

    let mut languages = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT languages.set_id, languages.language_order, languages.language \
         FROM no_intro_pc_languages AS languages \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY languages.set_id, languages.language_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroLanguageRow>(conn)?
    {
        languages
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({"order": row.language_order, "language": row.language}));
    }

    let clone_markers = sql_query(
        "SELECT markers.set_id FROM no_intro_pc_clone_markers AS markers \
         JOIN snapshot_sets AS sets USING (set_id) WHERE sets.snapshot_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroCloneMarkerRow>(conn)?
    .into_iter()
    .map(|row| row.set_id)
    .collect::<BTreeSet<_>>();

    let clone_links = load_no_intro_archive_links(conn, key, "no_intro_pc_clone_links")?;
    let merge_links = load_no_intro_archive_links(conn, key, "no_intro_pc_merge_links")?;
    Ok(facts
        .into_iter()
        .map(|(set_id, row)| {
            let fact = serde_json::json!({
                "archive_id": row.archive_id,
                "description": row.description,
                "name_alt": row.name_alt,
                "region": row.region,
                "version": row.version,
                "bios_text": row.bios_text,
                "languages_present": row.languages_present,
                "languages": languages.remove(&set_id).unwrap_or_default(),
                "clone_marker": clone_markers.contains(&set_id),
                "clone_links": clone_links.get(&set_id).cloned().unwrap_or_default(),
                "merge_links": merge_links.get(&set_id).cloned().unwrap_or_default(),
            });
            (set_id, vec![fact])
        })
        .collect())
}

fn load_no_intro_dat_game_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut raw_positions = BTreeMap::<i64, Vec<(i64, i64)>>::new();
    for row in sql_query(
        "SELECT position.set_id, position.field_kind, position.source_order \
         FROM no_intro_dat_game_field_positions AS position \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY position.set_id, position.source_order, position.field_kind",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatGamePositionRow>(conn)?
    {
        raw_positions
            .entry(row.set_id)
            .or_default()
            .push((row.field_kind, row.source_order));
    }
    let positions = raw_positions
        .into_iter()
        .map(|(set_id, mut positions)| {
            // Attribute ordinals and child ordinals belong to separate domains.
            positions.sort_by_key(|(kind, order)| (*kind == 4, *order, *kind));
            let mut attribute_order = 0_i64;
            let positions = positions
                .into_iter()
                .map(|(field_kind, source_order)| {
                    let native_order = if field_kind == 4 {
                        child_ranks
                            .get(&set_id)
                            .and_then(|ranks| ranks.get(&source_order))
                            .copied()
                    } else {
                        let rank = attribute_order;
                        attribute_order += 1;
                        Some(rank)
                    };
                    serde_json::json!({
                        "field_kind": field_kind,
                        "native_order": native_order,
                    })
                })
                .collect();
            (set_id, positions)
        })
        .collect::<BTreeMap<_, Vec<_>>>();
    let mut facts = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT games.set_id, games.id_text, clone.target_literal AS cloneof_text, parent_id.target_literal AS cloneofid_text, \
                games.description_text \
         FROM no_intro_dat_games AS games JOIN snapshot_sets AS sets USING (set_id) \
         LEFT JOIN no_intro_dat_set_links AS clone ON clone.set_id = games.set_id AND clone.link_kind = 'cloneof' \
         LEFT JOIN no_intro_dat_set_links AS parent_id ON parent_id.set_id = games.set_id AND parent_id.link_kind = 'cloneofid' \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, sets.set_id",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatGameRow>(conn)?
    {
        facts.insert(
            row.set_id,
            vec![serde_json::json!({
                "id_text": row.id_text,
                "cloneof_text": row.cloneof_text,
                "cloneofid_text": row.cloneofid_text,
                "description_text": row.description_text,
                "field_positions": positions.get(&row.set_id).cloned().unwrap_or_default(),
            })],
        );
    }
    Ok(facts)
}

fn load_no_intro_dat_child_ranks(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, BTreeMap<i64, i64>>> {
    let rows = sql_query(
        "SELECT source.set_id, source.source_order FROM (\
         SELECT games.set_id, position.source_order \
         FROM no_intro_dat_game_field_positions AS position \
         JOIN no_intro_dat_games AS games USING (set_id) \
         WHERE position.field_kind = 4 \
         UNION ALL SELECT category.set_id, category.source_order \
         FROM no_intro_dat_categories AS category \
         UNION ALL SELECT identifier.set_id, identifier.source_order \
         FROM no_intro_dat_identifiers AS identifier \
         UNION ALL SELECT release.set_id, release.source_order \
         FROM no_intro_dat_releases AS release \
         UNION ALL SELECT occurrence.record_id AS set_id, rom.source_order \
         FROM no_intro_dat_rom_claims AS rom \
         JOIN asset_occurrences AS occurrence USING (occurrence_id)\
         ) AS source JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY source.set_id, source.source_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatNativeOrderRow>(conn)?;
    let mut orders = BTreeMap::<i64, BTreeSet<i64>>::new();
    for row in rows {
        orders
            .entry(row.set_id)
            .or_default()
            .insert(row.source_order);
    }
    Ok(orders
        .into_iter()
        .map(|(set_id, orders)| (set_id, orders.into_iter().zip(0_i64..).collect()))
        .collect())
}

#[derive(QueryableByName)]
struct NoIntroDatNativeOrderRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

fn load_no_intro_dat_categories(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT category.set_id, category.category_order, category.category, category.source_order \
         FROM no_intro_dat_categories AS category JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY category.set_id, category.source_order, category.category_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatCategoryRow>(conn)?
    {
        grouped.entry(row.set_id).or_default().push(serde_json::json!({
            "order": row.category_order,
            "value": row.category,
            "native_order": child_ranks
                .get(&row.set_id)
                .and_then(|ranks| ranks.get(&row.source_order)),
        }));
    }
    Ok(grouped)
}

fn load_no_intro_dat_identifiers(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT identifier.set_id, identifier.identifier_order, identifier.identifier, \
                identifier.source_order \
         FROM no_intro_dat_identifiers AS identifier JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY identifier.set_id, identifier.source_order, identifier.identifier_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatIdentifierRow>(conn)?
    {
        grouped.entry(row.set_id).or_default().push(serde_json::json!({
            "order": row.identifier_order,
            "value": row.identifier,
            "native_order": child_ranks
                .get(&row.set_id)
                .and_then(|ranks| ranks.get(&row.source_order)),
        }));
    }
    Ok(grouped)
}

fn load_no_intro_dat_releases(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT release.set_id, release.release_order, release.name, release.region, \
                release.source_order, release.name_source_order, release.region_source_order \
         FROM no_intro_dat_releases AS release JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY release.set_id, release.source_order, release.release_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatReleaseRow>(conn)?
    {
        let (name_order, region_order) = if row.name_source_order < row.region_source_order {
            (0_i64, 1_i64)
        } else {
            (1_i64, 0_i64)
        };
        grouped.entry(row.set_id).or_default().push(serde_json::json!({
            "order": row.release_order,
            "name": row.name,
            "region": row.region,
            "name_attribute_order": name_order,
            "region_attribute_order": region_order,
            "native_order": child_ranks
                .get(&row.set_id)
                .and_then(|ranks| ranks.get(&row.source_order)),
        }));
    }
    Ok(grouped)
}

fn no_intro_dat_rom_order(
    requirements: &[RequirementRow],
    positions: &BTreeMap<i64, Vec<serde_json::Value>>,
    child_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> BTreeMap<i64, Vec<serde_json::Value>> {
    // Preserve sibling facts in native order without adding ordinals to the
    // canonical requirement multiset or matching occurrence IDs across snapshots.
    let mut grouped = BTreeMap::<i64, Vec<(i64, &RequirementRow)>>::new();
    for row in requirements {
        if let Some(order) = row.no_intro_dat_source_order {
            grouped.entry(row.set_id).or_default().push((order, row));
        }
    }
    grouped
        .into_iter()
        .map(|(set_id, mut rows)| {
            rows.sort_by_key(|(order, _)| *order);
            let facts = rows
                .into_iter()
                .map(|(order, row)| {
                    serde_json::json!({
                        "name": row.asset_name,
                        "attributes": no_intro_dat_rom_attributes(row, positions),
                        "native_order": child_ranks.get(&set_id).and_then(|ranks| ranks.get(&order)),
                    })
                })
                .collect();
            (set_id, facts)
        })
        .collect()
}

fn load_no_intro_dat_rom_positions(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    let mut source_ranks = BTreeMap::<i64, BTreeMap<i64, i64>>::new();
    for row in sql_query(
        "SELECT position.occurrence_id, position.field_kind, position.source_order \
         FROM no_intro_dat_rom_field_positions AS position \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         JOIN snapshot_sets AS sets ON sets.set_id = occurrence.record_id \
         WHERE sets.snapshot_key = ? ORDER BY position.occurrence_id, position.source_order, position.field_kind",
    )
    .bind::<Text, _>(key.as_str())
    .load::<NoIntroDatRomPositionRow>(conn)?
    {
        let ranks = source_ranks.entry(row.occurrence_id).or_default();
        let next_rank = i64::try_from(ranks.len()).map_err(|_| {
            crate::Error::DatabaseSchema(
                "No-Intro DAT ROM attribute rank exceeds SQLite's integer range".to_owned(),
            )
        })?;
        let native_order = *ranks.entry(row.source_order).or_insert(next_rank);
        let positions = grouped.entry(row.occurrence_id).or_default();
        positions.push(serde_json::json!({
            "field_kind": row.field_kind,
            "native_order": native_order,
        }));
    }
    Ok(grouped)
}

fn load_no_intro_archive_links(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    table: &str,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let query = format!(
        "SELECT links.set_id, links.target_archive_id FROM {table} AS links \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY links.set_id, links.target_archive_id"
    );
    let mut links = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(query)
        .bind::<Text, _>(key.as_str())
        .load::<NoIntroArchiveLinkRow>(conn)?
    {
        links
            .entry(row.set_id)
            .or_default()
            .push(serde_json::Value::String(row.target_archive_id));
    }
    Ok(links)
}

fn load_cmp_rom_positions(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
    native_ranks: &BTreeMap<i64, BTreeMap<i64, i64>>,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let positions = sql_query(
        "SELECT position.occurrence_id, occurrences.record_id AS set_id, position.field_kind, \
                position.source_field, position.source_order, rom_position.source_order AS form_source_order, \
                position.is_quoted \
         FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING (set_group_id) \
         JOIN asset_occurrences AS occurrences ON occurrences.record_id = sets.set_id \
         JOIN cmp_set_rom_positions AS rom_position USING (occurrence_id) \
         JOIN cmp_rom_field_positions AS position USING (occurrence_id) \
         WHERE groups.snapshot_key = ? ORDER BY position.occurrence_id, position.source_order",
    ).bind::<Text,_>(key.as_str()).load::<CmpRomPositionRow>(conn)?;
    let mut result = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for position in positions {
        let fields = result.entry(position.occurrence_id).or_default();
        fields.push(serde_json::json!({
            "field_kind": position.field_kind, "source_field": position.source_field,
            "native_order": fields.len(),
            "form_native_order": native_ranks.get(&position.set_id)
                .and_then(|ranks| ranks.get(&position.form_source_order)),
            "is_quoted": position.is_quoted,
        }));
    }
    Ok(result)
}

fn no_intro_dat_rom_attributes(
    row: &RequirementRow,
    positions: &BTreeMap<i64, Vec<serde_json::Value>>,
) -> serde_json::Value {
    serde_json::json!({
        "size_text": row.no_intro_dat_size_text,
        "crc_text": row.no_intro_dat_crc_text,
        "md5_text": row.no_intro_dat_md5_text,
        "sha1_text": row.no_intro_dat_sha1_text,
        "sha256_text": row.no_intro_dat_sha256_text,
        "status_text": row.no_intro_dat_status_text,
        "serial_text": row.no_intro_dat_serial_text,
        "header_text": row.no_intro_dat_header_text,
        "date_text": row.no_intro_dat_date_text,
        "mia_text": row.no_intro_dat_mia_text,
        "field_positions": positions.get(&row.occurrence_id),
    })
}

fn assemble_requirements(
    result: &mut CatalogRecords,
    requirements: Vec<RequirementRow>,
    cmp_positions: &BTreeMap<i64, Vec<serde_json::Value>>,
    no_intro_dat_rom_positions: &BTreeMap<i64, Vec<serde_json::Value>>,
) {
    for row in requirements {
        let no_intro_dat_attributes = no_intro_dat_rom_attributes(&row, no_intro_dat_rom_positions);
        let value = serde_json::json!({
            "role": row.role,
            "size": row.size,
            "crc": row.crc.map(hex::encode),
            "md5": row.md5.map(hex::encode),
            "sha1": row.sha1.map(hex::encode),
            "sha256": row.sha256.map(hex::encode),
            "evidence_scope": row.evidence_scope,
            "evidence_provenance": row.evidence_provenance,
            "merge_name": row.merge_name,
            "dump_status": row.dump_status,
            "serial": row.serial,
            "date": row.date,
            "mame_attributes": {
                "size_text": row.mame_size_text,
                "crc_text": row.mame_crc_text,
                "sha1_text": row.mame_sha1_text,
                "md5_text": row.mame_md5_text,
                "region": row.mame_region,
                "bios": row.mame_bios,
                "offset_text": row.mame_offset_text,
                "source_order": row.mame_source_order,
                "status_specified": row.mame_status_specified,
                "optional": row.mame_optional.map(|value| value != 0),
                "optional_specified": row.mame_optional_specified,
                "sound_only": row.mame_sound_only.map(|value| value != 0),
                "dispose": row.mame_dispose.map(|value| value != 0),
                "load_flag": row.mame_load_flag,
                "value": row.mame_value,
                "inverted": row.mame_inverted.map(|value| value != 0),
                "ovha": row.mame_ovha,
                "no_thread": row.mame_no_thread.map(|value| value != 0),
                "disk_index": row.mame_disk_index,
                "writable": row.mame_writable.map(|value| value != 0),
                "writable_specified": row.mame_writable_specified,
                "writeable": row.mame_writeable.map(|value| value != 0),
            },
            "logiqx_attributes": {
                "size_text": row.logiqx_size_text,
                "crc_text": row.logiqx_crc_text,
                "md5_text": row.logiqx_md5_text,
                "sha1_text": row.logiqx_sha1_text,
                "status_was_present": row.logiqx_status_was_present,
                "source_order": row.logiqx_source_order,
            },
            "cmp_declarations": {
                "size_text": row.cmp_size_text, "crc_text": row.cmp_crc_text,
                "crc32_text": row.cmp_crc32_text, "md5_text": row.cmp_md5_text,
                "sha1_text": row.cmp_sha1_text, "status_text": row.cmp_status_text,
                "nodump_present": row.cmp_nodump_present, "baddump_present": row.cmp_baddump_present,
                "field_positions": cmp_positions.get(&row.occurrence_id),
            },
            "no_intro_dat_attributes": no_intro_dat_attributes,
        });
        result
            .requirements
            .entry(row.set_id)
            .or_default()
            .entry(row.asset_name)
            .or_default()
            .push(value);
    }
    for assets in result.requirements.values_mut() {
        for evidence in assets.values_mut() {
            evidence.sort_by_key(serde_json::Value::to_string);
        }
    }
}

fn mame_switch_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let switches = sql_query(
        "SELECT sets.set_id, switches.switch_order, switches.kind, switches.name, \
                switches.tag, switches.mask, switches.source_order \
         FROM machine_switches AS switches JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, switches.switch_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchRow>(conn)?;
    let switch_locations = sql_query(
        "SELECT sets.set_id, locations.switch_order, locations.location_order, \
                locations.name, locations.number, locations.inverted, \
                locations.inverted_specified, locations.source_order \
         FROM machine_switch_locations AS locations \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, locations.switch_order, locations.location_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchLocationRow>(conn)?;
    let switch_values = sql_query(
        "SELECT sets.set_id, switch_values.switch_order, switch_values.value_order, \
                switch_values.name, switch_values.value, switch_values.is_default, \
                switch_values.default_specified, switch_values.source_order \
         FROM machine_switch_values AS switch_values \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, switch_values.switch_order, switch_values.value_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSwitchValueRow>(conn)?;

    let mut nested_orders = BTreeMap::<(i64, i64), BTreeSet<i64>>::new();
    for location in &switch_locations {
        nested_orders
            .entry((location.set_id, location.switch_order))
            .or_default()
            .insert(location.source_order);
    }
    for value in &switch_values {
        nested_orders
            .entry((value.set_id, value.switch_order))
            .or_default()
            .insert(value.source_order);
    }
    let nested_ranks: BTreeMap<_, BTreeMap<_, _>> = nested_orders
        .into_iter()
        .map(|(key, orders)| (key, orders.into_iter().zip(0_i64..).collect()))
        .collect();
    let mut locations = BTreeMap::<(i64, i64), Vec<serde_json::Value>>::new();
    for location in switch_locations {
        let key = (location.set_id, location.switch_order);
        locations
            .entry(key)
            .or_default()
            .push(serde_json::json!({
                "order": location.location_order,
                "native_order": nested_ranks.get(&key).and_then(|ranks| ranks.get(&location.source_order)),
                "name": location.name,
                "number": location.number,
                "inverted": location.inverted,
                "inverted_specified": location.inverted_specified,
            }));
    }
    let mut values = BTreeMap::<(i64, i64), Vec<serde_json::Value>>::new();
    for value in switch_values {
        let key = (value.set_id, value.switch_order);
        values.entry(key).or_default().push(serde_json::json!({
            "order": value.value_order,
            "native_order": nested_ranks.get(&key).and_then(|ranks| ranks.get(&value.source_order)),
            "name": value.name,
            "value": value.value,
            "default": value.is_default,
            "default_specified": value.default_specified,
        }));
    }
    let mut result = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for switch in switches {
        let key = (switch.set_id, switch.switch_order);
        result
            .entry(switch.set_id)
            .or_default()
            .push(serde_json::json!({
                "kind": switch.kind,
                "name": switch.name,
                "tag": switch.tag,
                "mask": switch.mask,
                "source_order": switch.source_order,
                "locations": locations.remove(&key).unwrap_or_default(),
                "values": values.remove(&key).unwrap_or_default(),
            }));
    }
    Ok(result)
}

fn mame_bios_set_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT set_id, bios_order, source_order, name, description, is_default, default_specified \
         FROM machine_bios_sets \
         WHERE snapshot_key = ? ORDER BY set_name, bios_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineBiosSetRow>(conn)?;
    let mut facts = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        facts
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "order": row.bios_order,
                "source_order": row.source_order,
                "name": row.name,
                "description": row.description,
                "default": row.is_default,
                "default_specified": row.default_specified,
            }));
    }
    Ok(facts)
}

fn load_mame_machine_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(
        "SELECT set_id, source_file, description, description_source_order, year, year_source_order, \
         manufacturer, manufacturer_source_order, is_device, is_device_specified, runnable, \
         runnable_specified, is_bios, is_bios_specified, is_mechanical, is_mechanical_specified, \
         is_consumable, is_consumable_specified FROM mame_machine_facts \
         WHERE snapshot_key = ? ORDER BY set_name",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MameMachineFactsRow>(conn)?;
    let mut grouped = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        grouped
            .entry(row.set_id)
            .or_default()
            .push(serde_json::json!({
                "source_file": row.source_file,
                "description": row.description,
                "description_source_order": row.description_source_order,
                "year": row.year,
                "year_source_order": row.year_source_order,
                "manufacturer": row.manufacturer,
                "manufacturer_source_order": row.manufacturer_source_order,
                "is_device": row.is_device,
                "is_device_specified": row.is_device_specified,
                "runnable": row.runnable,
                "runnable_specified": row.runnable_specified,
                "is_bios": row.is_bios,
                "is_bios_specified": row.is_bios_specified,
                "is_mechanical": row.is_mechanical,
                "is_mechanical_specified": row.is_mechanical_specified,
                "is_consumable": row.is_consumable,
                "is_consumable_specified": row.is_consumable_specified,
            }));
    }
    Ok(grouped)
}

const MAME_SPECIFICATION_SQL: &str = concat!(
    "WITH requested_mame_specification_owners(set_id) AS (",
    "SELECT sets.set_id FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) ",
    "WHERE groups.snapshot_key=? AND sets.source_element_kind='mame_machine'), ",
    include_str!("db/mame_specification.sql"),
    " ORDER BY e.set_id, e.element_order"
);

#[cfg(test)]
mod tests;

fn load_mame_machine_specification_facts(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows = sql_query(MAME_SPECIFICATION_SQL)
        .bind::<Text, _>(key.as_str())
        .load::<MachineSpecificationRow>(conn)?;
    let mut elements = BTreeMap::<(i64, i64), serde_json::Value>::new();
    for row in rows {
        let identity = (row.set_id, row.element_order);
        let mut value = serde_json::to_value(row)?;
        remove_source_locations(&mut value);
        elements.insert(identity, value);
    }

    for row in sql_query(
        "SELECT sets.set_id, facts.element_order, facts.control_order, facts.control_type, \
         facts.player, facts.buttons, facts.minimum, facts.maximum, facts.sensitivity, \
         facts.keydelta, facts.reverse, facts.reverse_specified, facts.ways, facts.ways2, facts.ways3 \
         FROM mame_machine_input_controls AS facts JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, facts.element_order, facts.control_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineInputControlRow>(conn)?
    {
        let identity = (row.set_id, row.element_order);
        let value = serde_json::json!({
            "order": row.control_order, "type": row.control_type, "player": row.player,
            "buttons": row.buttons, "minimum": row.minimum, "maximum": row.maximum,
            "sensitivity": row.sensitivity, "keydelta": row.keydelta, "reverse": row.reverse,
            "reverse_specified": row.reverse_specified,
            "ways": row.ways, "ways2": row.ways2, "ways3": row.ways3,
        });
        append_nested_spec_fact(&mut elements, &identity, "controls", value);
    }
    for row in sql_query(
        "SELECT sets.set_id, facts.element_order, facts.analog_order, facts.mask \
         FROM mame_machine_analogs AS facts JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, facts.element_order, facts.analog_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineAnalogRow>(conn)?
    {
        let identity = (row.set_id, row.element_order);
        let value = serde_json::json!({
            "order": row.analog_order, "mask": row.mask,
        });
        append_nested_spec_fact(&mut elements, &identity, "analogs", value);
    }
    for row in sql_query(
        "SELECT sets.set_id, facts.element_order, facts.extension_order, facts.name \
         FROM mame_machine_device_extensions AS facts JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, facts.element_order, facts.extension_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineDeviceExtensionRow>(conn)?
    {
        let identity = (row.set_id, row.element_order);
        let value = serde_json::json!({
            "order": row.extension_order, "name": row.name,
        });
        append_nested_spec_fact(&mut elements, &identity, "extensions", value);
    }
    for row in sql_query(
        "SELECT sets.set_id, facts.element_order, facts.option_order, facts.name, \
         facts.devname, facts.is_default, facts.default_specified \
         FROM mame_machine_slot_options AS facts JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? ORDER BY sets.set_name, facts.element_order, facts.option_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineSlotOptionRow>(conn)?
    {
        let identity = (row.set_id, row.element_order);
        let value = serde_json::json!({
            "order": row.option_order, "name": row.name, "devname": row.devname,
            "default": row.is_default,
            "default_specified": row.default_specified,
        });
        append_nested_spec_fact(&mut elements, &identity, "options", value);
    }

    let mut result = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for ((set_id, _), value) in elements {
        result.entry(set_id).or_default().push(value);
    }
    let conditions_by_set = load_mame_conditions(conn, key)?;
    for (set_id, conditions) in conditions_by_set {
        result
            .entry(set_id)
            .or_default()
            .push(serde_json::json!({"conditions": conditions}));
    }
    Ok(result)
}

fn load_mame_conditions(
    conn: &mut diesel::SqliteConnection,
    key: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let mut conditions_by_set = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in sql_query(
        "SELECT sets.set_id, conditions.owner_kind, conditions.owner_element_order, \
                conditions.owner_switch_order, conditions.owner_child_order, \
                conditions.condition_order, conditions.tag, conditions.mask, \
                conditions.relation, conditions.value \
         FROM mame_machine_conditions AS conditions \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? \
         ORDER BY sets.set_name, conditions.owner_kind, conditions.owner_element_order, \
                  conditions.owner_switch_order, conditions.owner_child_order, conditions.condition_order",
    )
    .bind::<Text, _>(key.as_str())
    .load::<MachineConditionRow>(conn)?
    {
        conditions_by_set.entry(row.set_id).or_default().push(serde_json::json!({
            "owner_kind": row.owner_kind, "element_order": row.owner_element_order,
            "switch_order": row.owner_switch_order, "child_order": row.owner_child_order,
            "order": row.condition_order, "tag": row.tag, "mask": row.mask,
            "relation": row.relation, "value": row.value,
        }));
    }
    sort_json_groups(&mut conditions_by_set);
    Ok(conditions_by_set)
}

fn remove_source_locations(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("source_line");
            object.remove("source_column");
            object.values_mut().for_each(remove_source_locations);
        }
        serde_json::Value::Array(values) => values.iter_mut().for_each(remove_source_locations),
        _ => {}
    }
}

fn append_nested_spec_fact(
    elements: &mut BTreeMap<(i64, i64), serde_json::Value>,
    identity: &(i64, i64),
    field: &str,
    value: serde_json::Value,
) {
    if let Some(parent) = elements
        .get_mut(identity)
        .and_then(serde_json::Value::as_object_mut)
    {
        let nested = parent.entry(field).or_insert_with(|| serde_json::json!([]));
        if let Some(array) = nested.as_array_mut() {
            array.push(value);
        }
    }
}

fn requirement_changes(
    previous: Option<&BTreeMap<String, Vec<serde_json::Value>>>,
    current: Option<&BTreeMap<String, Vec<serde_json::Value>>>,
) -> Vec<SnapshotRequirementChange> {
    let mut names = BTreeSet::new();
    if let Some(previous) = previous {
        names.extend(previous.keys().cloned());
    }
    if let Some(current) = current {
        names.extend(current.keys().cloned());
    }
    names
        .into_iter()
        .filter_map(|asset_name| {
            let before = previous.and_then(|values| values.get(&asset_name));
            let after = current.and_then(|values| values.get(&asset_name));
            if before == after {
                return None;
            }
            let previous_value = before.map(|values| serde_json::Value::Array(values.clone()));
            let current_value = after.map(|values| serde_json::Value::Array(values.clone()));
            let size_changed = field_changed(before, after, "size");
            let hash_changed = ["crc", "md5", "sha1", "sha256"]
                .into_iter()
                .any(|field| field_changed(before, after, field));
            let other_evidence_changed = [
                "role",
                "evidence_scope",
                "evidence_provenance",
                "merge_name",
                "dump_status",
                "serial",
                "date",
                "extensions",
                "cmp_declarations",
                "no_intro_dat_attributes",
                "mame_attributes",
            ]
            .into_iter()
            .any(|field| field_changed(before, after, field))
                || !(size_changed || hash_changed);
            Some(SnapshotRequirementChange {
                asset_name,
                size_changed,
                hash_changed,
                other_evidence_changed,
                previous: previous_value,
                current: current_value,
            })
        })
        .collect()
}

fn field_changed(
    previous: Option<&Vec<serde_json::Value>>,
    current: Option<&Vec<serde_json::Value>>,
    field: &str,
) -> bool {
    field_values(previous, field) != field_values(current, field)
}

fn field_values(rows: Option<&Vec<serde_json::Value>>, field: &str) -> Vec<serde_json::Value> {
    let mut values = rows
        .into_iter()
        .flatten()
        .filter_map(|row| row.get(field).filter(|value| !value.is_null()).cloned())
        .collect::<Vec<_>>();
    values.sort_by_key(serde_json::Value::to_string);
    values
}

fn parent_names(sets: &[SetRow]) -> Vec<Option<String>> {
    let mut names = sets
        .iter()
        .map(|set| set.parent_name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn sort_json_groups<K: Ord>(groups: &mut BTreeMap<K, Vec<serde_json::Value>>) {
    for values in groups.values_mut() {
        values.sort_by_key(serde_json::Value::to_string);
    }
}

fn owner_metadata(sets: &[SetRow], records: &CatalogRecords) -> Vec<serde_json::Value> {
    let mut metadata = sets
        .iter()
        .map(|set| set_metadata(records, set))
        .collect::<Vec<_>>();
    metadata.sort_by_key(serde_json::Value::to_string);
    metadata
}

fn set_metadata(records: &CatalogRecords, set: &SetRow) -> serde_json::Value {
    serde_json::json!({
        "mame_machine_facts": records.mame_machine_facts.get(&set.set_id),
        "mame_machine_specification_facts": records.mame_machine_specification_facts.get(&set.set_id),
        "mame_machine_dependencies": records.mame_machine_dependencies.get(&set.set_id),
        "no_intro_game_facts": records.no_intro_game_facts.get(&set.set_id),
        "no_intro_dat_game_facts": records.no_intro_dat_game_facts.get(&set.set_id),
        "no_intro_dat_categories": records.no_intro_dat_categories.get(&set.set_id),
        "no_intro_dat_identifiers": records.no_intro_dat_identifiers.get(&set.set_id),
        "no_intro_dat_releases": records.no_intro_dat_releases.get(&set.set_id),
        "no_intro_dat_rom_order": records.no_intro_dat_rom_order.get(&set.set_id),
        "no_intro_database_games": records.no_intro_database_games.get(&set.set_id),
        "logiqx_set_facts": records.logiqx_set_facts.get(&set.set_id),
        "logiqx_text_positions": records.logiqx_text_positions.get(&set.set_id),
        "logiqx_game_comments": records.logiqx_game_comments.get(&set.set_id),
        "logiqx_releases": records.logiqx_releases.get(&set.set_id),
        "logiqx_bios_sets": records.logiqx_bios_sets.get(&set.set_id),
        "logiqx_archive_references": records.logiqx_archive_references.get(&set.set_id),
        "cmp_set_facts": records.cmp_set_facts.get(&set.set_id),
        "software_items": records.software_items.get(&set.set_id),
        "software_parent_context": records.software_context_classes.get(&set.set_id),
        "machine_switches": records.machine_switches.get(&set.set_id),
        "machine_bios_sets": records.machine_bios_sets.get(&set.set_id),
    })
}

fn owner_signatures(sets: &[SetRow], records: &CatalogRecords) -> Vec<serde_json::Value> {
    let mut signatures = sets
        .iter()
        .map(|set| {
            serde_json::json!({
                "metadata": set_metadata(records, set),
                "parent": set.parent_name,
                "requirements": records.requirements.get(&set.set_id),
            })
        })
        .collect::<Vec<_>>();
    signatures.sort_by_key(serde_json::Value::to_string);
    signatures
}
