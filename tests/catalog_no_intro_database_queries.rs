use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{NoIntroDumpSourceId, NoIntroReleaseId},
    catalog_no_intro_database::{self, NoIntroDatabasePageLimit},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    no_intro_db_xml::{EnvelopeKind, HeaderFieldKind, NoIntroDatabaseMode},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn game_page_limits_are_checked_before_querying() {
    assert!(NoIntroDatabasePageLimit::new(0).is_err());
    assert!(NoIntroDatabasePageLimit::new(501).is_err());
    assert!(NoIntroDatabasePageLimit::new(1).is_ok());
    assert!(NoIntroDatabasePageLimit::new(500).is_ok());
}

#[test]
fn native_dump_and_release_ids_reject_nonpositive_numbers() -> TestResult {
    for value in [i64::MIN, -1, 0] {
        assert!(NoIntroDumpSourceId::try_from(value).is_err());
        assert!(NoIntroReleaseId::try_from(value).is_err());
    }
    for value in [1, i64::MAX] {
        assert_eq!(NoIntroDumpSourceId::try_from(value)?.as_i64(), value);
        assert_eq!(NoIntroReleaseId::try_from(value)?.as_i64(), value);
    }
    Ok(())
}

#[test]
fn omitted_and_empty_headers_remain_distinct_on_empty_game_pages() -> TestResult {
    for (index, (xml, present)) in [
        ("<datafile/>", false),
        ("<datafile><header/></datafile>", true),
        ("<header/><datafile/>", true),
    ]
    .into_iter()
    .enumerate()
    {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let input = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
        std::fs::write(&input, xml)?;
        let database = Database::open(&path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: input,
                format: CatalogDocumentFormat::NoIntroDatabase(
                    NoIntroDatabaseMode::ObservedCompatible,
                ),
                source_key: PublishingSourceKey::new(format!("empty-header-{index}")),
                source_display_name: "Empty header witness".into(),
                catalog_key: CatalogKey::new(format!("empty-header-{index}")),
                catalog_display_name: "Empty header witness".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        let snapshot = report.snapshot_key.ok_or("missing snapshot")?;
        let page = catalog_no_intro_database::games_for_snapshot(
            &database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        assert!(page.games.is_empty());
        assert!(page.next_cursor.is_none());
        assert_eq!(page.document.header.is_some(), present);
        if let Some(header) = page.document.header {
            assert!(header.fields.is_empty());
        }
    }
    Ok(())
}

#[test]
fn native_empty_export_document_is_queryable_without_source_objects() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let input = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    let xml = "<header><version>one</version><author></author><version>two</version><piracy>p</piracy><trademarks>t</trademarks><url>u</url></header><datafile/>";
    std::fs::write(&input, xml)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: input.clone(),
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("native-query-source"),
            source_display_name: "Native query source".into(),
            catalog_key: CatalogKey::new("native-query-catalog"),
            catalog_display_name: "Native query catalog".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("missing snapshot")?;
    std::fs::rename(&input, directory.path().join("unavailable-original"))?;
    let retained = Utf8PathBuf::from(format!("{path}.documents"));
    assert!(retained.is_dir());
    std::fs::rename(&retained, directory.path().join("unavailable-objects"))?;
    assert!(app::load_snapshot_source(&database, &snapshot).is_err());

    let page = catalog_no_intro_database::games_for_snapshot(
        &database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert!(page.games.is_empty());
    assert!(page.next_cursor.is_none());
    assert_eq!(page.document.envelope, EnvelopeKind::SiblingHeaderDatafile);
    assert_eq!(page.document.mode, NoIntroDatabaseMode::ObservedCompatible);
    let header = page.document.header.ok_or("missing native header")?;
    assert_eq!(
        header
            .fields
            .iter()
            .map(|field| field.kind)
            .collect::<Vec<_>>(),
        [
            HeaderFieldKind::Version,
            HeaderFieldKind::Author,
            HeaderFieldKind::Version,
            HeaderFieldKind::Piracy,
            HeaderFieldKind::Trademarks,
            HeaderFieldKind::Url
        ]
    );
    assert_eq!(
        header
            .fields
            .iter()
            .map(|field| field.value.as_str())
            .collect::<Vec<_>>(),
        ["one", "", "two", "p", "t", "u"]
    );
    for (order, (field, marker)) in header
        .fields
        .iter()
        .zip([
            "<version>one",
            "<author>",
            "<version>two",
            "<piracy>",
            "<trademarks>",
            "<url>",
        ])
        .enumerate()
    {
        assert_eq!(field.value.source_order, order);
        assert_eq!(field.value.location.line, 1);
        assert_eq!(
            field.value.location.column,
            i64::try_from(xml.find(marker).ok_or("missing header child")? + 1)?
        );
    }
    Ok(())
}
