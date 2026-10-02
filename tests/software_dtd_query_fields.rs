use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{
        self, OccurrenceKind, SetGroupKind, SoftwareAreaKind, SoftwareDumpStatus,
        SoftwareFileOperation, SoftwareFilePayload, SoftwareLoadInstruction, SourceElementKind,
    },
    catalog_software::{
        self, SoftwareAreaFields, SoftwareDataWidth, SoftwareEndianness, SoftwarePageLimit,
        SoftwareSupportedStatus, SoftwareTextField,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SOFTWARE_LIST_FIELDS: &str =
    include_str!("../fixtures/specifications/software-list-fields.xml");

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "One explicit public-query witness per pinned DTD field and actual owner"
)]
fn pinned_0289_software_fields_round_trip_through_public_catalog_queries() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, SOFTWARE_LIST_FIELDS)?;

    let imported = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-dtd-query-fields"),
            source_display_name: "Software DTD query fields".to_owned(),
            catalog_key: CatalogKey::new("software-dtd-query-fields"),
            catalog_display_name: "Software DTD query fields".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    let snapshot = imported.snapshot_key.ok_or("published snapshot missing")?;

    // softwarelist: @name and @description plus list-scoped <notes>.
    let list_page = catalog_software::lists_for_snapshot(
        &database,
        &snapshot,
        SoftwarePageLimit::new(1)?,
        None,
    )?;
    assert!(list_page.next_cursor.is_none());
    let list = list_page.lists.first().ok_or("software list missing")?;
    assert_eq!(list.name, "list α");
    assert_eq!(list.description.as_deref(), Some(" List description "));
    assert_eq!(list.notes.as_deref(), Some(" List notes é😀 "));
    assert_eq!(list.source_order, 0);
    assert_eq!(
        list.text_positions
            .iter()
            .map(|position| (position.field, position.source_order))
            .collect::<Vec<_>>(),
        [(SoftwareTextField::Notes, 0)]
    );

    let title_page = catalog_software::titles_for_list(
        &database,
        &snapshot,
        list.id,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    assert!(title_page.next_cursor.is_none());
    assert_eq!(title_page.titles.len(), 2);
    let title = title_page.titles.first().ok_or("software item missing")?;

    // software: @name, @cloneof, @supported (typed value and explicitness).
    assert_eq!(title.name, "game α");
    assert_eq!(title.clone_of.as_deref(), Some("parent"));
    assert_eq!(title.supported, SoftwareSupportedStatus::Partial);
    assert!(title.supported_specified);

    // The four item PCDATA fields preserve their source spelling, spaces, and order.
    assert_eq!(title.description, " Title é😀 ");
    assert_eq!(title.year, "19??");
    assert_eq!(title.publisher, " Publisher & company ");
    assert_eq!(title.notes.as_deref(), Some(" Item notes "));
    assert_eq!(
        title
            .text_positions
            .iter()
            .map(|position| (position.field, position.source_order))
            .collect::<Vec<_>>(),
        [
            (SoftwareTextField::Description, 0),
            (SoftwareTextField::Year, 1),
            (SoftwareTextField::Publisher, 2),
            (SoftwareTextField::Notes, 3),
        ]
    );

    // info and sharedfeat each preserve repeated names and distinguish absent from empty @value.
    assert_eq!(
        title
            .info
            .iter()
            .map(|value| (
                value.name.as_str(),
                value.value.as_deref(),
                value.source_order
            ))
            .collect::<Vec<_>>(),
        [
            ("serial", Some("SER-001"), 4),
            ("serial", None, 5),
            ("serial", Some(""), 6),
        ]
    );
    assert_eq!(
        title
            .shared_features
            .iter()
            .map(|value| (
                value.name.as_str(),
                value.value.as_deref(),
                value.source_order
            ))
            .collect::<Vec<_>>(),
        [
            ("compatibility", Some(" PAL "), 7),
            ("compatibility", None, 8),
            ("compatibility", Some(""), 9),
        ]
    );

    // part @name/@interface and feature @name/@value; duplicate part and feature names remain rows.
    assert_eq!(title.parts.len(), 2);
    let part = title.parts.first().ok_or("first part missing")?;
    let repeated_part = title.parts.get(1).ok_or("second part missing")?;
    assert_eq!(
        (part.name.as_str(), part.interface.as_str()),
        ("cart", "cart-interface")
    );
    assert_eq!(
        (
            repeated_part.name.as_str(),
            repeated_part.interface.as_str()
        ),
        ("cart", "")
    );
    assert_ne!(part.id, repeated_part.id);
    assert_eq!((part.source_order, repeated_part.source_order), (10, 11));
    assert_eq!(
        part.features
            .iter()
            .map(|value| (
                value.name.as_str(),
                value.value.as_deref(),
                value.source_order
            ))
            .collect::<Vec<_>>(),
        [
            ("board", Some(" Mapper "), 0),
            ("board", None, 1),
            ("board", Some(""), 2),
        ]
    );

    // dataarea @name/@size/@width/@endianness, including DTD defaults and explicitness flags.
    assert_eq!(part.areas.len(), 3);
    let data = part.areas.first().ok_or("data area missing")?;
    let defaulted_data = part.areas.get(1).ok_or("second data area missing")?;
    let disk = part.areas.get(2).ok_or("disk area missing")?;
    assert_eq!((data.name.as_str(), data.source_order), ("program", 3));
    assert_eq!(
        data.fields,
        SoftwareAreaFields::Data {
            size_text: "0x20".to_owned(),
            size: Some(32),
            width: SoftwareDataWidth::Bits16,
            width_specified: true,
            endianness: SoftwareEndianness::Big,
            endianness_specified: true,
        }
    );
    assert_eq!(
        (defaulted_data.name.as_str(), defaulted_data.source_order),
        ("program", 4)
    );
    assert_eq!(
        defaulted_data.fields,
        SoftwareAreaFields::Data {
            size_text: String::new(),
            size: None,
            width: SoftwareDataWidth::Bits8,
            width_specified: false,
            endianness: SoftwareEndianness::Little,
            endianness_specified: false,
        }
    );

    // diskarea @name; dipswitch @name/@tag/@mask and dipvalue @name/@value/@default.
    assert_eq!((disk.name.as_str(), disk.source_order), ("media", 5));
    let switch = part.switches.first().ok_or("DIP switch missing")?;
    assert_eq!(
        (
            switch.name.as_str(),
            switch.tag.as_str(),
            switch.mask.as_str()
        ),
        ("Mode", ":SW", "0x03")
    );
    assert_eq!(switch.source_order, 6);
    assert_eq!(
        switch
            .values
            .iter()
            .map(|value| (
                value.name.as_str(),
                value.value.as_str(),
                value.is_default,
                value.default_specified,
                value.source_order,
            ))
            .collect::<Vec<_>>(),
        [
            ("Default", "0x01", true, true, 0),
            ("Default", "", false, false, 1),
        ]
    );

    // Resolve the API's actual IDs; each payload must retain its title, part, area, and source order.
    assert_eq!(data.entry_ids.len(), 2);
    assert_eq!(defaulted_data.entry_ids.len(), 0);
    assert_eq!(disk.entry_ids.len(), 1);
    let files = catalog_files::occurrences_for_ids(
        &database,
        &data
            .entry_ids
            .iter()
            .chain(&disk.entry_ids)
            .copied()
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(files.len(), 3);

    for (id, source_order, occurrence_kind) in [
        (data.entry_ids[0], 0, OccurrenceKind::SoftwareRomEntry),
        (data.entry_ids[1], 1, OccurrenceKind::SoftwareRomOperation),
        (disk.entry_ids[0], 0, OccurrenceKind::SoftwareDiskEntry),
    ] {
        let file = files
            .iter()
            .find(|file| file.occurrence_id == id)
            .ok_or("queried occurrence ID missing from payload results")?;
        assert_eq!(file.provenance.set_id, title.id);
        assert_eq!(file.provenance.set_name, "game α");
        assert_eq!(
            file.provenance.set_group_kind,
            SetGroupKind::SoftwareList {
                name: "list α".to_owned()
            }
        );
        assert_eq!(
            file.provenance.source_element_kind,
            SourceElementKind::SoftwareItem
        );
        assert_eq!(file.provenance.occurrence_kind, occurrence_kind);
        assert_eq!(file.provenance.snapshot_key, snapshot.as_str());
        let owner = file
            .provenance
            .software_owner
            .as_ref()
            .ok_or("software occurrence owner missing")?;
        assert_eq!(owner.part_id, part.id.database_value());
        assert_eq!(owner.part_order, 0);
        assert_eq!(owner.part_name, "cart");
        assert_eq!(
            owner.area_id,
            if occurrence_kind == OccurrenceKind::SoftwareDiskEntry {
                disk.id.database_value()
            } else {
                data.id.database_value()
            }
        );
        assert_eq!(
            owner.area_name,
            if occurrence_kind == OccurrenceKind::SoftwareDiskEntry {
                "media"
            } else {
                "program"
            }
        );
        assert_eq!(
            owner.area_order,
            if occurrence_kind == OccurrenceKind::SoftwareDiskEntry {
                2
            } else {
                0
            }
        );
        assert_eq!(
            owner.area_kind,
            if occurrence_kind == OccurrenceKind::SoftwareDiskEntry {
                SoftwareAreaKind::Disk
            } else {
                SoftwareAreaKind::Data
            }
        );

        match (&file.software_file, occurrence_kind) {
            (Some(SoftwareFilePayload::Rom(rom)), OccurrenceKind::SoftwareRomEntry) => {
                assert_eq!(rom.name.as_deref(), Some("file α.bin"));
                assert_eq!(rom.size_text.as_deref(), Some("00016"));
                assert_eq!(rom.size, Some(14));
                assert_eq!(rom.crc_text.as_deref(), Some("A1B2C3D4"));
                assert_eq!(
                    rom.sha1_text.as_deref(),
                    Some("0123456789ABCDEF0123456789ABCDEF01234567")
                );
                assert_eq!(rom.offset_text.as_deref(), Some("0x00"));
                assert_eq!(rom.offset, Some(0));
                assert_eq!(rom.value.as_deref(), Some("0xA5"));
                assert_eq!(rom.status, SoftwareDumpStatus::BadDump);
                assert!(rom.status_specified);
                assert_eq!(
                    rom.load_instruction,
                    Some(SoftwareLoadInstruction::Load16Byte)
                );
                assert_eq!(rom.operation, SoftwareFileOperation::Load);
                assert_eq!((rom.component_order, rom.source_order), (0, source_order));
                assert_eq!(rom.declaration_occurrence_id, Some(id));
            }
            (Some(SoftwareFilePayload::Rom(rom)), OccurrenceKind::SoftwareRomOperation) => {
                assert_eq!(rom.name, None);
                assert_eq!(rom.size_text.as_deref(), Some("2"));
                assert_eq!(rom.size, Some(2));
                assert_eq!(rom.crc_text, None);
                assert_eq!(rom.sha1_text, None);
                assert_eq!(rom.offset_text.as_deref(), Some("0x10"));
                assert_eq!(rom.offset, Some(16));
                assert_eq!(rom.value.as_deref(), Some("0x55"));
                assert_eq!(rom.status, SoftwareDumpStatus::Good);
                assert!(!rom.status_specified);
                assert_eq!(
                    rom.load_instruction,
                    Some(SoftwareLoadInstruction::Continue)
                );
                assert_eq!(rom.operation, SoftwareFileOperation::Continue);
                assert_eq!((rom.component_order, rom.source_order), (1, source_order));
                assert_eq!(rom.declaration_occurrence_id, Some(data.entry_ids[0]));
            }
            (Some(SoftwareFilePayload::Disk(image)), OccurrenceKind::SoftwareDiskEntry) => {
                assert_eq!(image.name, "image");
                assert_eq!(
                    image.sha1_text.as_deref(),
                    Some("FEDCBA9876543210FEDCBA9876543210FEDCBA98")
                );
                assert_eq!(image.status, SoftwareDumpStatus::NoDump);
                assert!(image.status_specified);
                assert!(image.writeable);
                assert!(image.writeable_specified);
                assert_eq!(
                    (image.component_order, image.source_order),
                    (0, source_order)
                );
            }
            _ => return Err("software occurrence has the wrong native payload kind".into()),
        }
    }

    // The second item is metadata-only; required empty PCDATA survives and it owns no parts/files.
    let metadata_only = title_page
        .titles
        .get(1)
        .ok_or("metadata-only item missing")?;
    assert_eq!(metadata_only.name, "parent");
    assert_eq!(metadata_only.description, "");
    assert_eq!(metadata_only.year, "");
    assert_eq!(metadata_only.publisher, "");
    assert_eq!(metadata_only.supported, SoftwareSupportedStatus::Yes);
    assert!(!metadata_only.supported_specified);
    assert!(metadata_only.notes.is_none());
    assert!(metadata_only.parts.is_empty());
    Ok(())
}

#[test]
fn omitted_dtd_defaults_remain_typed_and_marked_unspecified() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("defaults.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        r#"<softwarelist name="defaults"><software name="game">
          <description/><year/><publisher/><part name="cart" interface="cart">
            <dataarea name="program" size="1"><rom size="1"/></dataarea>
            <diskarea name="media"><disk name="image"/></diskarea>
          </part></software></softwarelist>"#,
    )?;
    let imported = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-dtd-defaults"),
            source_display_name: "Software DTD defaults".to_owned(),
            catalog_key: CatalogKey::new("software-dtd-defaults"),
            catalog_display_name: "Software DTD defaults".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    let snapshot = imported.snapshot_key.ok_or("published snapshot missing")?;
    let lists = catalog_software::lists_for_snapshot(
        &database,
        &snapshot,
        SoftwarePageLimit::new(1)?,
        None,
    )?;
    let list = lists.lists.first().ok_or("software list missing")?;
    let titles = catalog_software::titles_for_list(
        &database,
        &snapshot,
        list.id,
        SoftwarePageLimit::new(1)?,
        None,
    )?;
    let title = titles.titles.first().ok_or("software item missing")?;
    assert_eq!(title.supported, SoftwareSupportedStatus::Yes);
    assert!(!title.supported_specified);
    let part = title.parts.first().ok_or("software part missing")?;
    let data = part.areas.first().ok_or("data area missing")?;
    let disk = part.areas.get(1).ok_or("disk area missing")?;
    assert_eq!(
        data.fields,
        SoftwareAreaFields::Data {
            size_text: "1".to_owned(),
            size: Some(1),
            width: SoftwareDataWidth::Bits8,
            width_specified: false,
            endianness: SoftwareEndianness::Little,
            endianness_specified: false,
        }
    );
    let ids = [data.entry_ids[0], disk.entry_ids[0]];
    let files = catalog_files::occurrences_for_ids(&database, &ids)?;
    assert_eq!(files.len(), 2);
    let rom_file = files
        .iter()
        .find(|file| file.occurrence_id == ids[0])
        .ok_or("ROM occurrence missing")?;
    let Some(SoftwareFilePayload::Rom(rom)) = &rom_file.software_file else {
        return Err("expected a ROM payload".into());
    };
    assert_eq!(rom.status, SoftwareDumpStatus::Good);
    assert!(!rom.status_specified);
    assert_eq!(rom.load_instruction, None);

    let disk_file = files
        .iter()
        .find(|file| file.occurrence_id == ids[1])
        .ok_or("disk occurrence missing")?;
    let Some(SoftwareFilePayload::Disk(image)) = &disk_file.software_file else {
        return Err("expected a disk payload".into());
    };
    assert_eq!(image.status, SoftwareDumpStatus::Good);
    assert!(!image.status_specified);
    assert!(!image.writeable);
    assert!(!image.writeable_specified);
    Ok(())
}
