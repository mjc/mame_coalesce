use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey,
        SnapshotRecordDiff, SnapshotRecordStatus,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn document(root_attributes: &str, header: &str, games: &[String]) -> String {
    format!(
        "<datafile{root_attributes}><header><name>Attribute provenance</name><description>Stable header</description><version>1</version><author>Test fixture</author>{header}</header>{}</datafile>",
        games.join("\n")
    )
}

fn named_game(name: &str, attributes: &str, children: &str) -> String {
    format!(
        "<game name='{name}'{attributes}><description>Stable game</description>{children}</game>"
    )
}

fn import(
    database: &Database,
    directory: &tempfile::TempDir,
    file_name: &str,
    contents: &str,
) -> TestResult<SnapshotKey> {
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join(file_name))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::Logiqx(
                mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
            ),
            source_key: PublishingSourceKey::new("logiqx-attribute-provenance"),
            source_display_name: "Logiqx attribute provenance".to_owned(),
            catalog_key: CatalogKey::new("logiqx-attribute-provenance"),
            catalog_display_name: "Logiqx attribute provenance".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("Logiqx import did not produce a snapshot")?;
    assert_eq!(
        app::load_snapshot_source(database, &snapshot)?,
        contents.as_bytes(),
        "the original source XML must remain recoverable"
    );
    Ok(snapshot)
}

fn diff_documents(previous: &str, current: &str) -> TestResult<CatalogSnapshotDiff> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let previous = import(&database, &directory, "previous.dat", previous)?;
    let current = import(&database, &directory, "current.dat", current)?;
    assert_ne!(
        previous, current,
        "fixtures must produce distinct snapshots"
    );
    Ok(app::diff_catalog_snapshots(&database, &previous, &current)?)
}

fn record<'a>(diff: &'a CatalogSnapshotDiff, name: &str) -> TestResult<&'a SnapshotRecordDiff> {
    assert!(diff.same_scope);
    let matches = diff
        .records
        .iter()
        .filter(|record| record.set_name == name)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one history record for {name}");
    matches
        .first()
        .copied()
        .ok_or_else(|| "history record missing".into())
}

fn assert_metadata_order_change(record: &SnapshotRecordDiff, context: &str) {
    assert!(record.metadata_changed, "{context}");
    assert!(!record.regrouped, "{context}");
    assert!(record.requirement_changes.is_empty(), "{context}");
}

fn assert_evidence_order_change(
    record: &SnapshotRecordDiff,
    asset_name: &str,
    context: &str,
) -> TestResult {
    assert!(!record.metadata_changed, "{context}");
    assert!(!record.regrouped, "{context}");
    assert_eq!(record.requirement_changes.len(), 1, "{context}");
    let change = record
        .requirement_changes
        .first()
        .ok_or("asset evidence change missing")?;
    assert_eq!(change.asset_name, asset_name, "{context}");
    assert!(!change.size_changed, "{context}");
    assert!(!change.hash_changed, "{context}");
    assert!(change.other_evidence_changed, "{context}");
    Ok(())
}

#[test]
fn root_build_and_debug_attribute_order_is_document_metadata() -> TestResult {
    let game = named_game("stable", "", "");
    let previous = document(" build='2026' debug='yes'", "", std::slice::from_ref(&game));
    let current = document(" debug='yes' build='2026'", "", &[game]);
    let diff = diff_documents(&previous, &current)?;

    assert!(diff.document_metadata_changed);
    let record = record(&diff, "stable")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn header_option_attribute_order_is_document_metadata() -> TestResult {
    let mut unchanged_options = Vec::new();
    for (label, options) in [
        (
            "clrmamepro",
            (
                "<clrmamepro forcemerging='full' forcenodump='obsolete' forcepacking='zip'/>",
                "<clrmamepro forcepacking='zip' forcenodump='obsolete' forcemerging='full'/>",
            ),
        ),
        (
            "romcenter",
            (
                "<romcenter rommode='split' biosmode='split' samplemode='merged' lockrommode='yes'/>",
                "<romcenter lockrommode='yes' samplemode='merged' biosmode='split' rommode='split'/>",
            ),
        ),
    ] {
        let game = named_game("stable", "", "");
        let previous = document("", options.0, std::slice::from_ref(&game));
        let current = document("", options.1, &[game]);
        let diff = diff_documents(&previous, &current)?;
        if !diff.document_metadata_changed {
            unchanged_options.push(label);
        }
        let record = record(&diff, "stable")?;
        assert_eq!(record.status, SnapshotRecordStatus::Unchanged, "{label}");
        assert!(!record.metadata_changed, "{label}");
        assert!(record.requirement_changes.is_empty(), "{label}");
    }
    assert!(
        unchanged_options.is_empty(),
        "lost option attribute order: {unchanged_options:?}"
    );
    Ok(())
}

#[test]
fn declared_attribute_order_is_retained_per_game_owner() -> TestResult {
    // Each distinct game changes exactly one owner's known attribute order. The
    // paired snapshots keep all values and each owner's assertions independent.
    let previous_games = [
        named_game("game-fields", " sourcefile='source.zip' isbios='no'", ""),
        named_game(
            "release-fields",
            "",
            "<release name='World' region='US' language='en'/>",
        ),
        named_game(
            "bios-fields",
            "",
            "<biosset name='base' description='Base' default='yes'/>",
        ),
        named_game(
            "rom-fields",
            "",
            "<rom name='rom.bin' size='16' crc='12345678' sha1='0123456789abcdef0123456789abcdef01234567' status='good' />",
        ),
        named_game(
            "disk-fields",
            "",
            "<disk name='disk.chd' sha1='0123456789abcdef0123456789abcdef01234567' md5='0123456789abcdef0123456789abcdef' status='good' />",
        ),
    ];
    let current_games = [
        named_game("game-fields", " isbios='no' sourcefile='source.zip'", ""),
        named_game(
            "release-fields",
            "",
            "<release region='US' name='World' language='en'/>",
        ),
        named_game(
            "bios-fields",
            "",
            "<biosset description='Base' name='base' default='yes'/>",
        ),
        named_game(
            "rom-fields",
            "",
            "<rom name='rom.bin' crc='12345678' size='16' sha1='0123456789abcdef0123456789abcdef01234567' status='good' />",
        ),
        named_game(
            "disk-fields",
            "",
            "<disk name='disk.chd' md5='0123456789abcdef0123456789abcdef' sha1='0123456789abcdef0123456789abcdef01234567' status='good' />",
        ),
    ];
    let diff = diff_documents(
        &document("", "", &previous_games),
        &document("", "", &current_games),
    )?;

    assert!(!diff.document_metadata_changed);
    assert_eq!(diff.records.len(), previous_games.len());
    let unchanged_owners = [
        "game-fields",
        "release-fields",
        "bios-fields",
        "rom-fields",
        "disk-fields",
    ]
    .into_iter()
    .map(|name| Ok((name, record(&diff, name)?.status)))
    .collect::<TestResult<Vec<_>>>()?
    .into_iter()
    .filter_map(|(name, status)| (status != SnapshotRecordStatus::Changed).then_some(name))
    .collect::<Vec<_>>();
    assert!(
        unchanged_owners.is_empty(),
        "lost native attribute order: {unchanged_owners:?}"
    );
    for name in ["game-fields", "release-fields", "bios-fields"] {
        assert_metadata_order_change(record(&diff, name)?, name);
    }
    assert_evidence_order_change(record(&diff, "rom-fields")?, "rom.bin", "ROM attributes")?;
    assert_evidence_order_change(record(&diff, "disk-fields")?, "disk.chd", "disk attributes")?;
    Ok(())
}

#[test]
fn vendor_attribute_insertion_and_reindent_do_not_change_native_history() -> TestResult {
    // Namespaced vendor extensions exercise compatibility parsing; this does
    // not claim strict DTD validation of extension attributes or document layout.
    let game = named_game(
        "stable",
        " sourcefile='source.zip'",
        "<rom name='rom.bin' size='16' crc='12345678' />",
    );
    let previous = format!(
        "<datafile xmlns:vendor='urn:vendor'><header><name>Attribute provenance</name><description>Stable header</description><version>1</version><author>Test fixture</author></header>{game}</datafile>"
    );
    let current = "<datafile\n  xmlns:vendor='urn:vendor' vendor:note='kept by source'\n>\n  <header>\n    <name>Attribute provenance</name>\n    <description>Stable header</description>\n    <version>1</version>\n    <author>Test fixture</author>\n  </header>\n  <game\n    name='stable' sourcefile='source.zip' vendor:game='extension'\n  ><description>Stable game</description><rom name='rom.bin' size='16' crc='12345678' vendor:rom='extension' /></game>\n</datafile>";
    let diff = diff_documents(&previous, current)?;

    assert!(!diff.document_metadata_changed);
    let record = record(&diff, "stable")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}
