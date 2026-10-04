#![allow(clippy::expect_used)]

use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::Text};

use super::*;

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

fn rom_row() -> RomRow {
    RomRow {
        occurrence_id: 2,
        occurrence_record_id: 5,
        claim_kind: "software_rom_operation".to_owned(),
        record_id: 5,
        area_id: 8,
        data_area_id: Some(8),
        disk_area_id: None,
        area_record_id: 5,
        part_id: 9,
        part_record_id: 5,
        area_kind: "data".to_owned(),
        area_name: Some("rom".to_owned()),
        area_order: 0,
        part_name: "cart".to_owned(),
        part_order: 0,
        name: None,
        size_text: Some("010".to_owned()),
        size: Some(8),
        offset_text: Some("0x10".to_owned()),
        offset: Some(16),
        value: None,
        crc_text: Some(String::new()),
        sha1_text: Some("bad".to_owned()),
        dump_status: "good".to_owned(),
        status_specified: 0,
        load_instruction: Some("continue".to_owned()),
        evidence_scope: "unknown".to_owned(),
        component_order: 1,
        source_order: 1,
        source_line: 12,
        source_column: 7,
        declaration_occurrence_id: None,
        use_record_id: Some(5),
        actual_declaration_occurrence_id: None,
        declaration_record_id: None,
        declaration_rom_record_id: None,
        declaration_rom_area_id: None,
        declaration_claim_kind: None,
        operation: Some("continue".to_owned()),
    }
}

fn disk_row() -> DiskRow {
    DiskRow {
        occurrence_id: 3,
        occurrence_record_id: 5,
        claim_kind: "software_disk_entry".to_owned(),
        record_id: 5,
        area_id: 10,
        data_area_id: None,
        disk_area_id: Some(10),
        area_record_id: 5,
        part_id: 9,
        part_record_id: 5,
        area_kind: "disk".to_owned(),
        area_name: Some("disks".to_owned()),
        area_order: 1,
        part_name: "cart".to_owned(),
        part_order: 0,
        name: "disk.chd".to_owned(),
        sha1_text: None,
        dump_status: "good".to_owned(),
        status_specified: 0,
        writeable: 0,
        writeable_specified: 0,
        evidence_scope: "chd_header_sha1".to_owned(),
        component_order: 0,
        source_order: 0,
        source_line: 15,
        source_column: 3,
        declaration_occurrence_id: None,
        use_record_id: Some(5),
        operation: Some("disk".to_owned()),
    }
}

fn link_declaration(row: &mut RomRow, declaration_id: i64, area_id: i64) {
    row.declaration_occurrence_id = Some(declaration_id);
    row.actual_declaration_occurrence_id = Some(declaration_id);
    row.declaration_record_id = Some(row.record_id);
    row.declaration_rom_record_id = Some(row.record_id);
    row.declaration_rom_area_id = Some(area_id);
    row.declaration_claim_kind = Some("software_rom_entry".to_owned());
}

fn link_self_declaration(row: &mut RomRow) {
    let id = row.occurrence_id;
    let area_id = row.area_id;
    link_declaration(row, id, area_id);
}

fn row_owner(area_id: i64) -> SoftwareAssetOwner {
    SoftwareAssetOwner {
        part_id: 5,
        area_id,
        part_order: 0,
        part_name: "part".to_owned(),
        area_name: "data".to_owned(),
        area_order: 0,
        area_kind: SoftwareAreaKind::Data,
    }
}

fn software_occurrence(kind: OccurrenceKind) -> CatalogFileOccurrence {
    CatalogFileOccurrence {
        clrmamepro_file: None,
        occurrence_id: OccurrenceId::from_database(42),
        content_id: None,
        canonical_content_id: None,
        provenance: super::super::OccurrenceProvenance {
            source_key: "source".to_owned(),
            source_name: "Source".to_owned(),
            catalog_key: "catalog".to_owned(),
            catalog_name: "Catalog".to_owned(),
            snapshot_key: "snapshot".to_owned(),
            document_key: "document".to_owned(),
            interpretation_key: "interpretation".to_owned(),
            format: "mame_softwarelist_xml".to_owned(),
            set_group_id: 2,
            set_group_kind: SetGroupKind::SoftwareList {
                name: "list".to_owned(),
            },
            set_id: crate::domain::CatalogSetId::from_database(3),
            set_name: "item".to_owned(),
            source_element_kind: SourceElementKind::SoftwareItem,
            occurrence_kind: kind,
            occurrence_order: 0,
            set_location: SourceLocation { line: 1, column: 1 },
            native_occurrence_location: None,
            asset_name: None,
            software_owner: Some(SoftwareAssetOwner {
                part_id: 5,
                area_id: 6,
                part_order: 0,
                part_name: "part".to_owned(),
                area_name: "data".to_owned(),
                area_order: 0,
                area_kind: SoftwareAreaKind::Data,
            }),
        },
        digests: Vec::new(),
        mame_file: None,
        logiqx_file: None,
        software_file: None,
        no_intro_dat_rom: None,
        no_intro_pc_rom: None,
        no_intro_database_file: None,
    }
}

#[test]
fn rom_rows_keep_source_values_and_reject_corrupt_owner_and_enums() {
    let payload = rom_payload(rom_row()).expect("valid ROM row");
    assert_eq!(payload.size_text.as_deref(), Some("010"));
    assert_eq!(payload.size, Some(8));
    assert_eq!(payload.offset_text.as_deref(), Some("0x10"));
    assert_eq!(payload.crc_text.as_deref(), Some(""));
    assert_eq!(payload.sha1_text.as_deref(), Some("bad"));
    assert_eq!(payload.operation, SoftwareFileOperation::Continue);
    assert_eq!(payload.declaration_occurrence_id, None);

    let mut invalid_status = rom_row();
    invalid_status.dump_status = "corrupt".to_owned();
    assert!(matches!(
        rom_payload(invalid_status),
        Err(CatalogFilesError::InvalidStoredValue { .. })
    ));

    let mut invalid_flag = rom_row();
    invalid_flag.status_specified = 2;
    assert!(matches!(
        rom_payload(invalid_flag),
        Err(CatalogFilesError::InvalidStoredValue { .. })
    ));

    let mut wrong_record = rom_row();
    wrong_record.area_record_id = 99;
    assert!(matches!(
        rom_payload(wrong_record),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut wrong_use = rom_row();
    wrong_use.use_record_id = Some(99);
    assert!(matches!(
        rom_payload(wrong_use),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut dangling_declaration = rom_row();
    dangling_declaration.declaration_occurrence_id = Some(1);
    assert!(matches!(
        rom_payload(dangling_declaration),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut wrong_operation = rom_row();
    wrong_operation.operation = Some("reload".to_owned());
    assert!(matches!(
        rom_payload(wrong_operation),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));
}

#[test]
fn rom_payload_rejects_whole_file_scope_on_operations_empty_names_and_nodump() {
    for case in ["operation", "empty", "nodump"] {
        let mut row = rom_row();
        row.evidence_scope = "whole_asset".to_owned();
        if case != "operation" {
            row.claim_kind = "software_rom_entry".to_owned();
            row.load_instruction = None;
            row.operation = Some("load".to_owned());
            row.name = Some(if case == "empty" { "" } else { "undumped.bin" }.to_owned());
            if case == "nodump" {
                row.dump_status = "nodump".to_owned();
                row.status_specified = 1;
            }
            link_self_declaration(&mut row);
        }
        assert!(
            matches!(
                rom_payload(row),
                Err(CatalogFilesError::InvalidStoredValue {
                    field: "software ROM evidence scope",
                    ..
                })
            ),
            "{case}"
        );
    }
}

#[test]
fn rom_declaration_links_match_claim_operation_and_area() {
    let mut declaration = rom_row();
    declaration.claim_kind = "software_rom_entry".to_owned();
    declaration.load_instruction = None;
    declaration.operation = Some("load".to_owned());
    declaration.name = Some("primary.bin".to_owned());
    declaration.evidence_scope = "whole_asset".to_owned();
    link_self_declaration(&mut declaration);
    assert!(rom_payload(declaration).is_ok());

    let mut foreign_area = rom_row();
    let foreign_area_id = foreign_area.area_id + 1;
    link_declaration(&mut foreign_area, 1, foreign_area_id);
    assert!(matches!(
        rom_payload(foreign_area),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut entry_without_self_link = rom_row();
    entry_without_self_link.claim_kind = "software_rom_entry".to_owned();
    entry_without_self_link.load_instruction = None;
    entry_without_self_link.operation = Some("load".to_owned());
    assert!(matches!(
        rom_payload(entry_without_self_link),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut entry_with_control_operation = rom_row();
    entry_with_control_operation.claim_kind = "software_rom_entry".to_owned();
    link_self_declaration(&mut entry_with_control_operation);
    assert!(matches!(
        rom_payload(entry_with_control_operation),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut fill_with_declaration = rom_row();
    fill_with_declaration.load_instruction = Some("fill".to_owned());
    fill_with_declaration.operation = Some("fill".to_owned());
    let fill_area_id = fill_with_declaration.area_id;
    link_declaration(&mut fill_with_declaration, 1, fill_area_id);
    assert!(matches!(
        rom_payload(fill_with_declaration),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut operation_with_self_link = rom_row();
    link_self_declaration(&mut operation_with_self_link);
    assert!(matches!(
        rom_payload(operation_with_self_link),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut unnamed_load_with_self_link = rom_row();
    unnamed_load_with_self_link.load_instruction = None;
    unnamed_load_with_self_link.operation = Some("load".to_owned());
    link_self_declaration(&mut unnamed_load_with_self_link);
    assert!(matches!(
        rom_payload(unnamed_load_with_self_link),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut unnamed_load_without_declaration = rom_row();
    unnamed_load_without_declaration.load_instruction = None;
    unnamed_load_without_declaration.operation = Some("load".to_owned());
    assert!(rom_payload(unnamed_load_without_declaration).is_ok());

    let mut unnamed_load_linked_to_another_declaration = rom_row();
    unnamed_load_linked_to_another_declaration.load_instruction = None;
    unnamed_load_linked_to_another_declaration.operation = Some("load".to_owned());
    let area_id = unnamed_load_linked_to_another_declaration.area_id;
    link_declaration(&mut unnamed_load_linked_to_another_declaration, 1, area_id);
    assert!(matches!(
        rom_payload(unnamed_load_linked_to_another_declaration),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));
}

#[test]
fn disk_rows_reject_bad_boolean_and_cross_kind_owner() {
    let payload = disk_payload(disk_row()).expect("valid disk row");
    assert_eq!(payload.name, "disk.chd");
    assert!(!payload.writeable);

    let mut invalid_boolean = disk_row();
    invalid_boolean.writeable = 2;
    assert!(matches!(
        disk_payload(invalid_boolean),
        Err(CatalogFilesError::InvalidStoredValue { .. })
    ));

    let mut wrong_kind = disk_row();
    wrong_kind.area_kind = "data".to_owned();
    assert!(matches!(
        disk_payload(wrong_kind),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));

    let mut disk_linked_to_rom_declaration = disk_row();
    disk_linked_to_rom_declaration.declaration_occurrence_id = Some(1);
    assert!(matches!(
        disk_payload(disk_linked_to_rom_declaration),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(_))
    ));
}

#[test]
fn attachment_rejects_missing_payload_and_provenance_owner_mismatch() {
    let pool = crate::storage::db::create_db_pool(":memory:").expect("initialized schema");
    let mut connection = pool.get().expect("database connection");
    super::super::create_request_table(&mut connection).expect("request table");

    let mut occurrence = software_occurrence(OccurrenceKind::SoftwareRomEntry);
    assert!(matches!(
        attach_payloads(&mut connection, std::slice::from_mut(&mut occurrence)),
        Err(CatalogFilesError::MissingSoftwareFilePayload(42))
    ));

    let mut occurrence = software_occurrence(OccurrenceKind::SoftwareRomEntry);
    assert!(matches!(
        validate_raw_owner(&occurrence, "software_rom_entry", &row_owner(99)),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(42))
    ));
    occurrence.provenance.occurrence_kind = OccurrenceKind::SoftwareDiskEntry;
    assert!(matches!(
        validate_raw_owner(&occurrence, "software_rom_entry", &row_owner(6)),
        Err(CatalogFilesError::MismatchedSoftwareFileOwner(42))
    ));
}

#[test]
fn native_payload_selects_use_requested_ids_and_primary_key_joins() {
    const ROM_SEARCHES: &[&str] = &[
        "occurrence",
        "rom",
        "area",
        "part",
        "file_use",
        "declaration",
        "declared_rom",
        "declared_occurrence",
    ];
    const DISK_SEARCHES: &[&str] = &["occurrence", "disk", "area", "part", "file_use"];

    let pool = crate::storage::db::create_db_pool(":memory:").expect("initialized schema");
    let mut connection = pool.get().expect("database connection");
    super::super::create_request_table(&mut connection).expect("request table");

    for (statement, indexed_owners) in [
        (format!("EXPLAIN QUERY PLAN {}", rom_select()), ROM_SEARCHES),
        (
            format!("EXPLAIN QUERY PLAN {}", disk_select()),
            DISK_SEARCHES,
        ),
    ] {
        let details = sql_query(statement)
            .load::<ExplainRow>(&mut connection)
            .expect("query plans")
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        let plan = details.join("\n");
        assert!(
            details
                .iter()
                .filter(|line| line.starts_with("SCAN "))
                .all(|line| line.contains("requested")),
            "unbounded table scan in plan:\n{plan}"
        );
        for alias in indexed_owners {
            assert!(
                plan.contains(&format!("SEARCH {alias} USING INTEGER PRIMARY KEY")),
                "missing indexed search for {alias}:\n{plan}"
            );
        }
    }
}
