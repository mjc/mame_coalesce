use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{
        self, CatalogFileOccurrence, SetGroupKind, SoftwareFilePayload, SourceElementKind,
    },
    catalog_software::{self, SoftwareArea, SoftwarePageLimit},
    database::Database,
    domain::{CatalogKey, CatalogScope, ContentDigestAlgorithm, PublishingSourceKey, SnapshotKey},
    software_loading::{
        ByteLayout, LayoutError, SoftwareFileInput, SoftwareLoadError, SoftwareLoadPlan,
        SoftwareLoadWarning,
    },
};
use std::fmt::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct ImportedSoftware {
    _directory: tempfile::TempDir,
    database: Database,
    document_path: Utf8PathBuf,
    snapshot: SnapshotKey,
}

fn import_software(xml: &str) -> TestResult<ImportedSoftware> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    let snapshot = import_edition(&database, &document_path, xml)?;
    Ok(ImportedSoftware {
        _directory: directory,
        database,
        document_path,
        snapshot,
    })
}

fn import_edition(
    database: &Database,
    document_path: &Utf8PathBuf,
    xml: &str,
) -> TestResult<SnapshotKey> {
    std::fs::write(document_path, xml)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: document_path.clone(),
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-loading-integration"),
            source_display_name: "Software loading integration".to_owned(),
            catalog_key: CatalogKey::new("software-loading-integration"),
            catalog_display_name: "Software loading integration".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("missing published snapshot")?)
}

fn first_area(
    imported: &ImportedSoftware,
    snapshot: &SnapshotKey,
    area_name: &str,
) -> TestResult<(catalog_software::SoftwareTitlePage, SoftwareArea)> {
    let lists = catalog_software::lists_for_snapshot(
        &imported.database,
        snapshot,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let list = lists.lists.first().ok_or("missing software list")?;
    let titles = catalog_software::titles_for_list(
        &imported.database,
        snapshot,
        list.id,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let title = titles.titles.first().ok_or("missing software title")?;
    let area = title
        .parts
        .iter()
        .flat_map(|part| &part.areas)
        .find(|area| area.name == area_name)
        .ok_or("missing software area")?
        .clone();
    Ok((titles, area))
}

fn area_occurrences(
    imported: &ImportedSoftware,
    area: &SoftwareArea,
) -> TestResult<Vec<CatalogFileOccurrence>> {
    Ok(catalog_files::occurrences_for_ids(
        &imported.database,
        &area.entry_ids,
    )?)
}

fn assert_context_corruption_rejected(
    area: &SoftwareArea,
    entries: &[CatalogFileOccurrence],
    corrupt: impl FnOnce(&mut CatalogFileOccurrence) -> TestResult,
) -> TestResult {
    let mut corrupted = entries.to_vec();
    corrupt(corrupted.get_mut(1).ok_or("area needs multiple entries")?)?;
    assert_ne!(
        corrupted, entries,
        "corruption witness must change a source fact"
    );
    assert!(SoftwareLoadPlan::from_area(area, &corrupted).is_err());
    Ok(())
}

const LOADING_XML: &str = r#"<softwarelist name="loading">
<software name="game"><description>Loading fixture</description><year>2000</year><publisher>Example</publisher>
<part name="cart" interface="cart">
<dataarea name="rom" size="32">
<rom name="program.bin" size="4" offset="0x00" loadflag="load16_word_swap" status="baddump" crc="00000000"/>
<rom size="2" offset="0x08" loadflag="continue"/>
<rom size="2" loadflag="ignore"/>
<rom size="9" offset="0x10" loadflag="reload"/>
</dataarea>
<dataarea name="edge" size="31"><rom name="edge.bin" size="1" offset="0x1e" loadflag="load16_word_swap"/></dataarea>
<dataarea name="invalid" size="16"><rom name="invalid.bin" size="bad-size" offset="0"/></dataarea>
<diskarea name="disks"><disk name="image.chd"/></diskarea>
</part></software></softwarelist>"#;

#[test]
fn imported_native_area_loads_after_source_removal_and_borrows_file_bytes() -> TestResult {
    let imported = import_software(LOADING_XML)?;
    std::fs::remove_file(&imported.document_path)?;

    // Both metadata and file payloads are obtained from the published native catalog.
    let (title_page, area) = first_area(&imported, &imported.snapshot, "rom")?;
    assert_eq!(
        title_page.titles.first().ok_or("missing title")?.name,
        "game"
    );
    let entries = area_occurrences(&imported, &area)?;
    assert_eq!(entries.len(), 4);
    assert!(
        entries
            .iter()
            .all(|entry| entry.provenance.snapshot_key == imported.snapshot.as_str())
    );
    assert!(entries.iter().all(|entry| {
        entry.provenance.format == CatalogDocumentFormat::MameSoftwareListXml.as_str()
    }));
    assert!(entries.iter().all(|entry| matches!(
        entry.software_file.as_ref(),
        Some(SoftwareFilePayload::Rom(_))
    )));

    let unordered_entries = entries.iter().rev().cloned().collect::<Vec<_>>();
    let plan = SoftwareLoadPlan::from_area(&area, &unordered_entries)?;
    assert_eq!(plan.region_length(), 32);
    assert_eq!(plan.files().len(), 1);
    let file = plan.files().first().ok_or("missing physical file")?;
    assert_eq!(file.name, "program.bin");
    assert_eq!(file.expected_length, 8);
    assert_eq!(file.progress_length, 9);
    assert_eq!(file.read_length, 9);

    let source_bytes = [1_u8, 2, 3, 4, 5, 6, 7, 8, 9];
    let declaration = file.declaration;
    let rejected = plan.bind(&[SoftwareFileInput {
        declaration,
        bytes: &source_bytes,
    }])?;
    let mut undersized_region = [0xa5_u8; 31];
    assert!(matches!(
        rejected.execute(&mut undersized_region),
        Err(SoftwareLoadError::WrongRegionSize)
    ));
    assert_eq!(undersized_region, [0xa5_u8; 31]);

    let ready = plan.bind(&[SoftwareFileInput {
        declaration,
        bytes: &source_bytes,
    }])?;
    let mut region = [0_u8; 32];
    let warnings = ready.execute(&mut region)?;

    assert_eq!(&region[0..4], &[2, 1, 4, 3]);
    assert_eq!(&region[8..10], &[6, 5]);
    assert_eq!(&region[16..24], &[2, 1, 4, 3, 6, 5, 8, 7]);
    assert_eq!(&region[24..26], &[0, 9]);
    assert!(warnings.iter().any(|warning| matches!(
        warning,
        SoftwareLoadWarning::LengthMismatch {
            declaration: found,
            expected: 8,
            actual: 9,
        } if *found == declaration
    )));
    assert!(warnings.iter().any(|warning| matches!(
        warning,
        SoftwareLoadWarning::PartialGroup {
            occurrence,
            length: 9,
            layout: ByteLayout::WordSwap16,
        } if area.entry_ids.last() == Some(occurrence)
    )));
    assert!(warnings.iter().any(|warning| matches!(
        warning,
        SoftwareLoadWarning::KnownBadDump { declaration: found }
            if *found == declaration
    )));
    assert!(warnings.iter().any(|warning| matches!(
        warning,
        SoftwareLoadWarning::ChecksumMismatch {
            declaration: found,
            algorithm: ContentDigestAlgorithm::Crc32,
        } if *found == declaration
    )));

    // The reversed byte in the primary area's partial word occupies its high
    // lane at byte 25; the analogous edge placement pads through byte 32 and
    // is rejected by its 31-byte region.
    let edge = first_area(&imported, &imported.snapshot, "edge")?.1;
    let edge_entries = area_occurrences(&imported, &edge)?;
    assert!(matches!(
        SoftwareLoadPlan::from_area(&edge, &edge_entries),
        Err(SoftwareLoadError::Layout(LayoutError::OutOfBounds {
            end: 32,
            region_length: 31,
        }))
    ));
    Ok(())
}

#[test]
fn area_plan_rejects_missing_duplicate_wrong_area_and_cross_edition_rows() -> TestResult {
    let imported = import_software(LOADING_XML)?;
    let original_area = first_area(&imported, &imported.snapshot, "rom")?.1;
    let original_entries = area_occurrences(&imported, &original_area)?;
    assert!(SoftwareLoadPlan::from_area(&original_area, &original_entries[..3]).is_err());

    let mut duplicate = original_entries.clone();
    duplicate.push(
        original_entries
            .first()
            .ok_or("missing first entry")?
            .clone(),
    );
    assert!(SoftwareLoadPlan::from_area(&original_area, &duplicate).is_err());

    let edge_area = first_area(&imported, &imported.snapshot, "edge")?.1;
    let edge_entries = area_occurrences(&imported, &edge_area)?;
    assert!(SoftwareLoadPlan::from_area(&original_area, &edge_entries).is_err());

    let next_document = imported.document_path.with_file_name("next.xml");
    let next_snapshot = import_edition(
        &imported.database,
        &next_document,
        &LOADING_XML.replace("Loading fixture", "Next edition"),
    )?;
    let next_area = first_area(&imported, &next_snapshot, "rom")?.1;
    let next_entries = area_occurrences(&imported, &next_area)?;
    assert_ne!(
        original_entries
            .first()
            .ok_or("missing original entry")?
            .provenance
            .snapshot_key,
        next_entries
            .first()
            .ok_or("missing next entry")?
            .provenance
            .snapshot_key
    );
    assert!(SoftwareLoadPlan::from_area(&original_area, &next_entries).is_err());
    Ok(())
}

#[test]
fn area_plan_rejects_corrupted_public_source_and_owner_provenance() -> TestResult {
    let imported = import_software(LOADING_XML)?;
    let area = first_area(&imported, &imported.snapshot, "rom")?.1;
    let entries = area_occurrences(&imported, &area)?;
    let next_document = imported.document_path.with_file_name("next.xml");
    let next_snapshot = import_edition(
        &imported.database,
        &next_document,
        &LOADING_XML.replace("Loading fixture", "Next edition"),
    )?;
    let next_area = first_area(&imported, &next_snapshot, "rom")?.1;
    let next_entries = area_occurrences(&imported, &next_area)?;
    let other_occurrence = next_entries.first().ok_or("missing next-edition entry")?;
    let other_owner = other_occurrence
        .provenance
        .software_owner
        .as_ref()
        .ok_or("missing next-edition owner")?;

    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.source_element_kind = SourceElementKind::MameMachine;
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.set_group_kind = SetGroupKind::Root;
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.format = "wrong-format".to_owned();
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.source_key.push_str("-other");
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.catalog_key.push_str("-other");
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.snapshot_key = other_occurrence.provenance.snapshot_key.clone();
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.document_key = other_occurrence.provenance.document_key.clone();
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        // Different editions normally share the same parser interpretation.
        entry
            .provenance
            .interpretation_key
            .push_str("-different-contract");
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.set_id = other_occurrence.provenance.set_id;
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry.provenance.set_group_id += 1;
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry
            .provenance
            .software_owner
            .as_mut()
            .ok_or("missing software owner")?
            .part_id = other_owner.part_id;
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry
            .provenance
            .software_owner
            .as_mut()
            .ok_or("missing software owner")?
            .part_name
            .push_str("-other");
        Ok(())
    })?;
    assert_context_corruption_rejected(&area, &entries, |entry| {
        entry
            .provenance
            .software_owner
            .as_mut()
            .ok_or("missing software owner")?
            .part_order += 1;
        Ok(())
    })?;
    assert_native_order_corruption_rejected(&area, &entries)
}

fn assert_native_order_corruption_rejected(
    area: &SoftwareArea,
    entries: &[CatalogFileOccurrence],
) -> TestResult {
    assert_context_corruption_rejected(area, entries, |entry| {
        let Some(SoftwareFilePayload::Rom(rom)) = entry.software_file.as_mut() else {
            return Err("missing ROM payload".into());
        };
        rom.component_order = 0;
        Ok(())
    })?;
    let first_source_order = match entries
        .first()
        .ok_or("missing first component")?
        .software_file
        .as_ref()
    {
        Some(SoftwareFilePayload::Rom(rom)) => rom.source_order,
        _ => return Err("first component has no ROM payload".into()),
    };
    assert_context_corruption_rejected(area, entries, |entry| {
        let Some(SoftwareFilePayload::Rom(rom)) = entry.software_file.as_mut() else {
            return Err("missing ROM payload".into());
        };
        rom.source_order = first_source_order;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn invalid_raw_numeric_source_remains_queryable_when_interpretation_fails() -> TestResult {
    let imported = import_software(LOADING_XML)?;
    let (title_page, area) = first_area(&imported, &imported.snapshot, "invalid")?;
    assert_eq!(
        title_page
            .titles
            .first()
            .ok_or("missing title")?
            .description,
        "Loading fixture"
    );
    assert_eq!(area.name, "invalid");
    assert_eq!(area.entry_ids.len(), 1);
    let entries = area_occurrences(&imported, &area)?;
    let occurrence = entries.first().ok_or("missing invalid ROM")?;
    let Some(SoftwareFilePayload::Rom(rom)) = occurrence.software_file.as_ref() else {
        return Err("invalid entry has no native ROM payload".into());
    };
    assert_eq!(occurrence.provenance.set_name, "game");
    assert_eq!(
        occurrence.provenance.source_element_kind,
        SourceElementKind::SoftwareItem
    );
    assert_eq!(
        occurrence.provenance.format,
        CatalogDocumentFormat::MameSoftwareListXml.as_str()
    );
    let owner = occurrence
        .provenance
        .software_owner
        .as_ref()
        .ok_or("missing invalid ROM owner")?;
    assert_eq!(
        (owner.part_name.as_str(), owner.area_name.as_str()),
        ("cart", "invalid")
    );
    assert_eq!(rom.name.as_deref(), Some("invalid.bin"));
    assert_eq!(rom.size_text.as_deref(), Some("bad-size"));
    assert_eq!(rom.size, None);
    assert_eq!(rom.offset_text.as_deref(), Some("0"));
    assert_eq!(rom.offset, Some(0));
    assert!(SoftwareLoadPlan::from_area(&area, &entries).is_err());
    assert_eq!(area_occurrences(&imported, &area)?, entries);
    let (after_failure, _) = first_area(&imported, &imported.snapshot, "invalid")?;
    assert_eq!(
        after_failure
            .titles
            .first()
            .ok_or("missing title after failure")?
            .description,
        "Loading fixture"
    );
    Ok(())
}

#[test]
fn native_unsigned_projections_keep_signed_raw_text_without_executable_numbers() -> TestResult {
    use catalog_software::SoftwareAreaFields;

    let signed = [
        "+010", "+08", "+2", "+0x2", "0x+2", "0X+2", "0+2", "-0", "-2", "0x-2",
    ];
    let valid = [
        ("010", 8),
        ("0x10", 16),
        ("10", 10),
        ("0", 0),
        ("0X10", 16),
        ("00", 0),
    ];
    let mut areas = String::new();
    for (index, value) in signed.iter().enumerate() {
        write!(
            areas,
            "<dataarea name='signed{index}' size='{value}'><rom name='file{index}.bin' size='{value}' offset='{value}'/></dataarea>"
        )?;
    }
    for (index, (value, _)) in valid.iter().enumerate() {
        write!(areas, "<dataarea name='valid{index}' size='{value}'/>")?;
    }
    let xml = format!(
        "<softwarelist name='numbers'><software name='game'><description>Numbers</description><year>2000</year><publisher>Test</publisher><part name='cart' interface='cart'>{areas}</part></software></softwarelist>"
    );
    let imported = import_software(&xml)?;
    std::fs::remove_file(&imported.document_path)?;
    for (index, value) in signed.iter().enumerate() {
        let area = first_area(&imported, &imported.snapshot, &format!("signed{index}"))?.1;
        let SoftwareAreaFields::Data {
            size_text, size, ..
        } = &area.fields
        else {
            return Err("expected data area".into());
        };
        assert_eq!(size_text, value);
        assert_eq!(*size, None, "signed region size {value:?}");
        let entries = area_occurrences(&imported, &area)?;
        let Some(SoftwareFilePayload::Rom(rom)) = entries
            .first()
            .and_then(|entry| entry.software_file.as_ref())
        else {
            return Err("expected native ROM".into());
        };
        assert_eq!(rom.size_text.as_deref(), Some(*value));
        assert_eq!(rom.offset_text.as_deref(), Some(*value));
        assert_eq!(rom.size, None, "signed ROM size {value:?}");
        assert_eq!(rom.offset, None, "signed offset {value:?}");
        assert!(matches!(
            SoftwareLoadPlan::from_area(&area, &entries),
            Err(SoftwareLoadError::InvalidNumber {
                field: "region size",
                ..
            })
        ));
        assert_eq!(area_occurrences(&imported, &area)?, entries);
    }
    for (index, (value, expected)) in valid.iter().enumerate() {
        let area = first_area(&imported, &imported.snapshot, &format!("valid{index}"))?.1;
        let SoftwareAreaFields::Data {
            size_text, size, ..
        } = &area.fields
        else {
            return Err("expected data area".into());
        };
        assert_eq!(size_text, value);
        assert_eq!(*size, Some(*expected));
        assert_eq!(
            SoftwareLoadPlan::from_area(&area, &[])?.region_length(),
            u64::try_from(*expected)?
        );
    }
    Ok(())
}

#[test]
fn invalid_size_offset_and_fill_value_each_prevent_execution_independently() -> TestResult {
    for (attribute, expected_field) in [
        ("size", "size"),
        ("offset", "offset"),
        ("value", "fill value"),
    ] {
        for raw in ["0x+2", "2;"] {
            let attributes = match attribute {
                "size" => format!("name='file.bin' size='{raw}' offset='0'"),
                "offset" => format!("name='file.bin' size='1' offset='{raw}'"),
                "value" => format!("size='1' offset='0' loadflag='fill' value='{raw}'"),
                _ => return Err("invalid test attribute".into()),
            };
            let xml = format!(
                "<softwarelist name='numbers'><software name='game'><description>Numbers</description><year>2000</year><publisher>Test</publisher><part name='cart' interface='cart'><dataarea name='rom' size='32'><rom {attributes}/></dataarea></part></software></softwarelist>"
            );
            let imported = import_software(&xml)?;
            std::fs::remove_file(&imported.document_path)?;
            let area = first_area(&imported, &imported.snapshot, "rom")?.1;
            let entries = area_occurrences(&imported, &area)?;
            let occurrence = entries.first().ok_or("missing native entry")?;
            let Some(SoftwareFilePayload::Rom(rom)) = occurrence.software_file.as_ref() else {
                return Err("missing ROM payload".into());
            };
            let retained = match attribute {
                "size" => rom.size_text.as_deref(),
                "offset" => rom.offset_text.as_deref(),
                "value" => rom.value.as_deref(),
                _ => return Err("invalid test attribute".into()),
            };
            assert_eq!(retained, Some(raw));
            assert!(matches!(SoftwareLoadPlan::from_area(&area, &entries),
                Err(SoftwareLoadError::InvalidNumber { field, occurrence: Some(id) })
                if field == expected_field && id == occurrence.occurrence_id
            ));
            assert_eq!(area_occurrences(&imported, &area)?, entries);
        }
    }
    Ok(())
}
