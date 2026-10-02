use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{self, SoftwareFilePayload},
    catalog_software::{
        self, SoftwareAreaFields, SoftwareDataWidth, SoftwareEndianness, SoftwareEnvelope,
        SoftwarePageLimit, SoftwareSupportedStatus, SoftwareTextField,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Imported {
    directory: tempfile::TempDir,
    database: Database,
    snapshot: SnapshotKey,
}

fn import(xml: &str) -> TestResult<Imported> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let snapshot = import_edition(&directory, &database, "software", xml)?;
    Ok(Imported {
        directory,
        database,
        snapshot,
    })
}

fn import_edition(
    directory: &tempfile::TempDir,
    database: &Database,
    name: &str,
    xml: &str,
) -> TestResult<SnapshotKey> {
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{name}.xml")))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(database, &request(document_path))?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("missing published snapshot")?)
}

fn request(document_path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameSoftwareListXml,
        source_key: PublishingSourceKey::new("software-metadata-query"),
        source_display_name: "Software metadata query".to_owned(),
        catalog_key: CatalogKey::new("software-metadata-query"),
        catalog_display_name: "Software metadata query".to_owned(),
        scope: CatalogScope::Complete,
    }
}

fn first_titles(imported: &Imported) -> TestResult<catalog_software::SoftwareTitlePage> {
    let lists = catalog_software::lists_for_snapshot(
        &imported.database,
        &imported.snapshot,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let list = lists.lists.first().ok_or("missing software list")?;
    Ok(catalog_software::titles_for_list(
        &imported.database,
        &imported.snapshot,
        list.id,
        SoftwarePageLimit::new(10)?,
        None,
    )?)
}

const FAMILIES: &str = r#"<softwarelists build=""><vendor/>
<softwarelist name="same" description=" List "><notes> List notes </notes><vendor/>
<software name="game" cloneof="" supported="partial">
<description> é😀 Title </description><info name="repeat"/><vendor/>
<year>19??</year><info name="repeat" value=""/><publisher/><notes> Notes </notes>
<sharedfeat name="compatibility" value=" ntsc "/>
<part name="cart" interface="cart"><feature name="repeat"/><vendor/>
<dipswitch name="Mode" tag="config" mask="0x03"><dipvalue name="Off" value="00"/>
<vendor/><dipvalue name="On" value="0x01" default="yes"/>
<dipvalue name="Other" value="text" default="no"/></dipswitch>
<dataarea name="same" size="0040"><rom name="a" size="1" offset="0"/>
<vendor/><rom size="1" offset="1" loadflag="continue"/></dataarea>
<feature name="repeat" value=""/><diskarea name="same"><disk name="image"/></diskarea>
<dataarea name="explicit" size="invalid" width="16" endianness="big"/>
</part><part name="cart" interface="other"/></software>
</softwarelist><softwarelist name="same"><software name="empty">
<description/><year/><publisher/></software></softwarelist></softwarelists>"#;

#[test]
fn native_metadata_keeps_values_presence_positions_and_repeated_names() -> TestResult {
    let imported = import(FAMILIES)?;
    let page = first_titles(&imported)?;
    assert_eq!(
        page.snapshot.envelope,
        SoftwareEnvelope::PluralLists {
            build: Some(String::new())
        }
    );
    assert_eq!(page.list.name, "same");
    assert_eq!(page.list.description.as_deref(), Some(" List "));
    assert_eq!(page.list.notes.as_deref(), Some(" List notes "));
    assert_eq!(page.list.source_order, 1);
    assert_eq!(
        page.list
            .text_positions
            .first()
            .ok_or("list notes position")?
            .field,
        SoftwareTextField::Notes
    );
    let title = page.titles.first().ok_or("title")?;
    assert_eq!(title.clone_of.as_deref(), Some(""));
    assert_eq!(title.supported, SoftwareSupportedStatus::Partial);
    assert!(title.supported_specified);
    assert_eq!(title.description, " é😀 Title ");
    assert_eq!(title.publisher, "");
    assert_eq!(title.notes.as_deref(), Some(" Notes "));
    assert_eq!(title.source_order, 2);
    assert_eq!(
        title
            .text_positions
            .iter()
            .map(|p| (p.field, p.source_order))
            .collect::<Vec<_>>(),
        vec![
            (SoftwareTextField::Description, 0),
            (SoftwareTextField::Year, 3),
            (SoftwareTextField::Publisher, 5),
            (SoftwareTextField::Notes, 6)
        ]
    );
    assert!(
        title
            .text_positions
            .iter()
            .all(|p| p.location.line > 0 && p.location.column > 0)
    );
    assert_eq!(
        title
            .info
            .iter()
            .map(|v| (v.name.as_str(), v.value.as_deref(), v.source_order))
            .collect::<Vec<_>>(),
        vec![("repeat", None, 1), ("repeat", Some(""), 4)]
    );
    assert_eq!(
        title
            .shared_features
            .first()
            .ok_or("shared feature")?
            .value
            .as_deref(),
        Some(" ntsc ")
    );
    assert_eq!(title.parts.len(), 2);
    let first = title.parts.first().ok_or("first part")?;
    let second = title.parts.get(1).ok_or("second part")?;
    assert_eq!(first.name, second.name);
    assert_ne!(first.id, second.id);
    assert_eq!(
        (first.interface.as_str(), second.interface.as_str()),
        ("cart", "other")
    );
    assert_eq!(first.source_order, 8);
    assert_eq!(
        first
            .features
            .iter()
            .map(|v| (v.value.as_deref(), v.source_order))
            .collect::<Vec<_>>(),
        vec![(None, 0), (Some(""), 4)]
    );
    Ok(())
}

#[test]
fn dip_switches_keep_raw_values_defaults_and_source_gaps() -> TestResult {
    let imported = import(FAMILIES)?;
    let page = first_titles(&imported)?;
    let title = page.titles.first().ok_or("title")?;
    let part = title.parts.first().ok_or("part")?;
    let switch = part.switches.first().ok_or("switch")?;
    assert_eq!(
        (
            switch.name.as_str(),
            switch.tag.as_str(),
            switch.mask.as_str()
        ),
        ("Mode", "config", "0x03")
    );
    assert_eq!(switch.source_order, 2);
    assert_eq!(
        switch
            .values
            .iter()
            .map(|v| (
                v.value.as_str(),
                v.is_default,
                v.default_specified,
                v.source_order
            ))
            .collect::<Vec<_>>(),
        vec![
            ("00", false, false, 0),
            ("0x01", true, true, 2),
            ("text", false, true, 3)
        ]
    );
    assert!(
        switch
            .values
            .iter()
            .all(|v| v.location.line > 0 && v.location.column > 0)
    );
    Ok(())
}

#[test]
fn areas_keep_native_fields_and_reference_existing_file_payloads() -> TestResult {
    let imported = import(FAMILIES)?;
    let page = first_titles(&imported)?;
    let title = page.titles.first().ok_or("title")?;
    let part = title.parts.first().ok_or("part")?;
    let data = part.areas.first().ok_or("data area")?;
    let disk = part.areas.get(1).ok_or("disk area")?;
    let explicit = part.areas.get(2).ok_or("explicit area")?;
    assert_eq!(data.name, disk.name);
    assert_ne!(data.id, disk.id);
    assert_eq!(
        data.fields,
        SoftwareAreaFields::Data {
            size_text: "0040".into(),
            size: Some(32),
            width: SoftwareDataWidth::Bits8,
            width_specified: false,
            endianness: SoftwareEndianness::Little,
            endianness_specified: false,
        }
    );
    assert_eq!(disk.fields, SoftwareAreaFields::Disk);
    assert_eq!(
        explicit.fields,
        SoftwareAreaFields::Data {
            size_text: "invalid".into(),
            size: None,
            width: SoftwareDataWidth::Bits16,
            width_specified: true,
            endianness: SoftwareEndianness::Big,
            endianness_specified: true,
        }
    );
    assert_eq!(
        (data.source_order, disk.source_order, explicit.source_order),
        (3, 5, 6)
    );
    assert_eq!(data.entry_ids.len(), 2);
    assert_eq!(disk.entry_ids.len(), 1);
    assert!(explicit.entry_ids.is_empty());
    let ids = part
        .areas
        .iter()
        .flat_map(|area| area.entry_ids.iter().copied())
        .collect::<Vec<_>>();
    let files = catalog_files::occurrences_for_ids(&imported.database, &ids)?;
    assert_eq!(files.len(), 3);
    for area in &part.areas {
        for id in &area.entry_ids {
            let file = files
                .iter()
                .find(|file| file.occurrence_id == *id)
                .ok_or("unresolved area entry")?;
            let owner = file
                .provenance
                .software_owner
                .as_ref()
                .ok_or("file owner")?;
            assert_eq!(owner.part_id, part.id.database_value());
            assert_eq!(owner.area_id, area.id.database_value());
            assert_eq!(file.provenance.snapshot_key, imported.snapshot.as_str());
            match (&area.fields, &file.software_file) {
                (SoftwareAreaFields::Data { .. }, Some(SoftwareFilePayload::Rom(_)))
                | (SoftwareAreaFields::Disk, Some(SoftwareFilePayload::Disk(_))) => {}
                _ => return Err("area references wrong file kind".into()),
            }
        }
    }
    Ok(())
}

const REPEATED: &str = r#"<softwarelists><softwarelist name="same">
<software name="repeat"><description>First</description><year>1</year><publisher/></software><vendor/>
<software name="repeat" supported="yes"><description>Second</description><year>2</year><publisher/></software>
</softwarelist><vendor/><softwarelist name="same">
<software name="repeat" supported="no"><description>Third</description><year>3</year><publisher/></software>
</softwarelist></softwarelists>"#;

#[test]
fn keyset_pages_keep_repeated_names_and_reject_wrong_scope_or_generation() -> TestResult {
    let imported = import(REPEATED)?;
    let limit = SoftwarePageLimit::new(1)?;
    let first =
        catalog_software::lists_for_snapshot(&imported.database, &imported.snapshot, limit, None)?;
    let cursor = first.next_cursor.as_ref().ok_or("list cursor")?;
    let second = catalog_software::lists_for_snapshot(
        &imported.database,
        &imported.snapshot,
        limit,
        Some(cursor),
    )?;
    let list_a = first.lists.first().ok_or("first list")?;
    let list_b = second.lists.first().ok_or("second list")?;
    assert_eq!(list_a.name, list_b.name);
    assert_ne!(list_a.id, list_b.id);
    assert_eq!((list_a.source_order, list_b.source_order), (0, 2));
    assert!(second.next_cursor.is_none());
    let titles_a = catalog_software::titles_for_list(
        &imported.database,
        &imported.snapshot,
        list_a.id,
        limit,
        None,
    )?;
    let title_cursor = titles_a.next_cursor.as_ref().ok_or("title cursor")?;
    let titles_b = catalog_software::titles_for_list(
        &imported.database,
        &imported.snapshot,
        list_a.id,
        limit,
        Some(title_cursor),
    )?;
    let a = titles_a.titles.first().ok_or("first title")?;
    let b = titles_b.titles.first().ok_or("second title")?;
    assert_eq!(a.name, b.name);
    assert_ne!(a.id, b.id);
    assert_eq!(
        (a.description.as_str(), b.description.as_str()),
        ("First", "Second")
    );
    assert_eq!((a.source_order, b.source_order), (0, 2));
    assert_eq!(a.supported, SoftwareSupportedStatus::Yes);
    assert!(!a.supported_specified);
    assert!(b.supported_specified);
    assert!(titles_b.next_cursor.is_none());
    assert!(
        catalog_software::titles_for_list(
            &imported.database,
            &imported.snapshot,
            list_b.id,
            limit,
            Some(title_cursor)
        )
        .is_err()
    );
    let other = import(REPEATED)?;
    assert!(
        catalog_software::lists_for_snapshot(&other.database, &other.snapshot, limit, Some(cursor))
            .is_err()
    );
    let other_list =
        catalog_software::lists_for_snapshot(&other.database, &other.snapshot, limit, None)?;
    assert!(
        catalog_software::titles_for_list(
            &other.database,
            &other.snapshot,
            other_list.lists.first().ok_or("other list")?.id,
            limit,
            Some(title_cursor)
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn historical_pages_retain_document_and_snapshot_provenance() -> TestResult {
    let imported = import(REPEATED)?;
    let limit = SoftwarePageLimit::new(1)?;
    let old =
        catalog_software::lists_for_snapshot(&imported.database, &imported.snapshot, limit, None)?;
    let newer = import_edition(
        &imported.directory,
        &imported.database,
        "newer",
        &REPEATED.replace("First", "Changed"),
    )?;
    let current = catalog_software::lists_for_snapshot(&imported.database, &newer, limit, None)?;
    assert_ne!(old.snapshot.document_key, current.snapshot.document_key);
    assert_ne!(old.snapshot.snapshot_key, current.snapshot.snapshot_key);
    assert_eq!(old.snapshot.catalog_key, current.snapshot.catalog_key);
    assert_eq!(old.snapshot.registry_id, current.snapshot.registry_id);
    let old_title = catalog_software::titles_for_list(
        &imported.database,
        &imported.snapshot,
        old.lists.first().ok_or("old list")?.id,
        limit,
        None,
    )?;
    assert_eq!(
        old_title.titles.first().ok_or("old title")?.description,
        "First"
    );
    let continued = catalog_software::lists_for_snapshot(
        &imported.database,
        &imported.snapshot,
        limit,
        old.next_cursor.as_ref(),
    )?;
    assert_eq!(continued.snapshot.document_key, old.snapshot.document_key);
    assert!(
        catalog_software::lists_for_snapshot(
            &imported.database,
            &newer,
            limit,
            old.next_cursor.as_ref()
        )
        .is_err()
    );
    assert!(
        catalog_software::titles_for_list(
            &imported.database,
            &newer,
            old.lists.first().ok_or("old list")?.id,
            limit,
            None
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn empty_plural_lists_distinguish_absent_and_empty_build() -> TestResult {
    for (xml, build) in [
        ("<softwarelists/>", None),
        ("<softwarelists build=\"\"/>", Some(String::new())),
    ] {
        let imported = import(xml)?;
        let page = catalog_software::lists_for_snapshot(
            &imported.database,
            &imported.snapshot,
            SoftwarePageLimit::new(1)?,
            None,
        )?;
        assert_eq!(
            page.snapshot.envelope,
            SoftwareEnvelope::PluralLists { build }
        );
        assert!(page.lists.is_empty());
        assert!(page.next_cursor.is_none());
    }
    assert!(SoftwarePageLimit::new(0).is_err());
    assert!(SoftwarePageLimit::new(501).is_err());
    Ok(())
}

#[test]
fn root_catalogs_cannot_masquerade_as_software_metadata() -> TestResult {
    let imported = import(REPEATED)?;
    let path = Utf8PathBuf::try_from(imported.directory.path().join("root.xml"))?;
    std::fs::write(
        &path,
        "<datafile><game name=\"root\"><rom name=\"file\" size=\"1\"/></game></datafile>",
    )?;
    let mut root_request = request(path);
    root_request.format = CatalogDocumentFormat::Logiqx;
    let report = app::import_catalog(&imported.database, &root_request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("root snapshot")?;
    assert!(
        catalog_software::lists_for_snapshot(
            &imported.database,
            &snapshot,
            SoftwarePageLimit::new(1)?,
            None
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn data_widths_are_typed_and_explicit_defaults_remain_distinct() -> TestResult {
    for (wire, width) in [
        ("8", SoftwareDataWidth::Bits8),
        ("16", SoftwareDataWidth::Bits16),
        ("32", SoftwareDataWidth::Bits32),
        ("64", SoftwareDataWidth::Bits64),
    ] {
        let imported = import(&format!(
            r#"<softwarelist name="list"><software name="title" supported="yes">
        <description/><year/><publisher/><part name="part" interface="cart">
        <dataarea name="area" size="" width="{wire}" endianness="little"/>
        </part></software></softwarelist>"#
        ))?;
        let page = first_titles(&imported)?;
        assert_eq!(page.snapshot.envelope, SoftwareEnvelope::SingleList);
        let title = page.titles.first().ok_or("title")?;
        assert_eq!(title.supported, SoftwareSupportedStatus::Yes);
        assert!(title.supported_specified);
        let area = title
            .parts
            .first()
            .ok_or("part")?
            .areas
            .first()
            .ok_or("area")?;
        assert_eq!(
            area.fields,
            SoftwareAreaFields::Data {
                size_text: String::new(),
                size: None,
                width,
                width_specified: true,
                endianness: SoftwareEndianness::Little,
                endianness_specified: true,
            }
        );
    }
    Ok(())
}

#[test]
fn failed_eof_import_does_not_replace_published_metadata() -> TestResult {
    let imported = import(REPEATED)?;
    let path = Utf8PathBuf::try_from(imported.directory.path().join("broken.xml"))?;
    std::fs::write(&path, format!("{REPEATED}<broken"))?;
    let report = app::import_catalog(&imported.database, &request(path))?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    let page = first_titles(&imported)?;
    assert_eq!(page.titles.len(), 2);
    assert_eq!(
        page.titles.first().ok_or("retained title")?.description,
        "First"
    );
    assert!(
        catalog_software::lists_for_snapshot(
            &imported.database,
            &SnapshotKey::new(
                &CatalogKey::new("missing"),
                &page.snapshot.document_key,
                &page.snapshot.interpretation_key,
            ),
            SoftwarePageLimit::new(1)?,
            None
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn metadata_only_titles_are_queryable_without_rom_files() -> TestResult {
    let imported = import(
        r#"<softwarelist name="list"><software name="empty">
    <description>Empty</description><year>19??</year><publisher>Example</publisher>
    </software></softwarelist>"#,
    )?;
    let lists = catalog_software::lists_for_snapshot(
        &imported.database,
        &imported.snapshot,
        SoftwarePageLimit::new(1)?,
        None,
    )?;
    assert_eq!(lists.lists.len(), 1);
    let list = lists.lists.first().ok_or("missing software list")?;
    let page = catalog_software::titles_for_list(
        &imported.database,
        &imported.snapshot,
        list.id,
        SoftwarePageLimit::new(1)?,
        None,
    )?;
    assert_eq!(page.titles.len(), 1);
    let title = page.titles.first().ok_or("missing metadata-only title")?;
    assert_eq!(title.name, "empty");
    assert_eq!(title.year, "19??");
    assert!(title.parts.is_empty());
    assert!(page.next_cursor.is_none());
    Ok(())
}
