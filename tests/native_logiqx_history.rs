use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey,
        SnapshotRecordCorrespondence, SnapshotRecordDiff, SnapshotRecordStatus,
        SnapshotRequirementChange,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn document(options: &str, games: &[String]) -> String {
    format!(
        "<datafile><header><name>Native history</name>{options}</header>{}</datafile>",
        games.concat()
    )
}

fn game(children: &str) -> String {
    format!("<game name='same'><description>Stable</description>{children}</game>")
}

fn rom(name: &str, size: &str, attributes: &str) -> String {
    format!("<rom name='{name}' size='{size}' crc='12345678' {attributes}/>")
}

fn disk(attributes: &str) -> String {
    format!("<disk name='disk.chd' sha1='0123456789abcdef0123456789abcdef01234567' {attributes}/>")
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
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("native-logiqx-history"),
            source_display_name: "Native Logiqx history".to_owned(),
            catalog_key: CatalogKey::new("native-logiqx-history"),
            catalog_display_name: "Native Logiqx history".to_owned(),
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
        "original source bytes must remain recoverable"
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

fn only_record(diff: &CatalogSnapshotDiff) -> TestResult<&SnapshotRecordDiff> {
    assert!(diff.same_scope);
    assert_eq!(diff.records.len(), 1);
    let record = diff.records.first().ok_or("history record missing")?;
    assert_eq!(record.set_name, "same");
    Ok(record)
}

fn assert_unchanged(record: &SnapshotRecordDiff, context: &str) {
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged, "{context}");
    assert!(!record.regrouped, "{context}");
    assert!(!record.metadata_changed, "{context}");
    assert!(record.requirement_changes.is_empty(), "{context}");
}

fn assert_evidence_change(change: &SnapshotRequirementChange) {
    assert!(!change.size_changed, "{}", change.asset_name);
    assert!(!change.hash_changed, "{}", change.asset_name);
    assert!(change.other_evidence_changed, "{}", change.asset_name);
}

fn single_evidence_change<'a>(
    diff: &'a CatalogSnapshotDiff,
    asset_name: &str,
) -> TestResult<&'a SnapshotRequirementChange> {
    assert!(!diff.document_metadata_changed);
    let record = only_record(diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::UniqueName
    );
    assert!(!record.metadata_changed);
    assert!(!record.regrouped);
    assert_eq!(record.requirement_changes.len(), 1);
    let change = record
        .requirement_changes
        .first()
        .ok_or("requirement change missing")?;
    assert_eq!(change.asset_name, asset_name);
    assert_evidence_change(change);
    Ok(change)
}

#[test]
fn options_default_presence_and_values_change_only_document_metadata() -> TestResult {
    let games = [game(&rom("rom.bin", "16", ""))];
    for (family, attribute, default, changed) in [
        ("clrmamepro", "forcemerging", "split", "full"),
        ("clrmamepro", "forcenodump", "obsolete", "required"),
        ("clrmamepro", "forcepacking", "zip", "unzip"),
        ("romcenter", "rommode", "split", "unmerged"),
        ("romcenter", "biosmode", "split", "merged"),
        ("romcenter", "samplemode", "merged", "unmerged"),
        ("romcenter", "lockrommode", "no", "yes"),
        ("romcenter", "lockbiosmode", "no", "yes"),
        ("romcenter", "locksamplemode", "no", "yes"),
    ] {
        let omitted = format!("<{family}/>");
        let explicit_default = format!("<{family} {attribute}='{default}'/>");
        let different_value = format!("<{family} {attribute}='{changed}'/>");
        for (label, previous, current) in [
            ("default presence", &omitted, &explicit_default),
            ("value", &explicit_default, &different_value),
        ] {
            let context = format!("{family}.{attribute}: {label}");
            let diff = diff_documents(&document(previous, &games), &document(current, &games))?;
            assert!(diff.document_metadata_changed, "{context}");
            let record = only_record(&diff)?;
            assert_eq!(
                record.correspondence,
                SnapshotRecordCorrespondence::UniqueName
            );
            assert_unchanged(record, &context);
        }
    }
    Ok(())
}

#[test]
fn swapping_native_header_options_changes_document_metadata_only() -> TestResult {
    let clrmamepro = "<clrmamepro forcemerging='full'/>";
    let romcenter = "<romcenter lockrommode='yes'/>";
    let games = [game(&rom("rom.bin", "16", ""))];
    let diff = diff_documents(
        &document(&format!("{clrmamepro}{romcenter}"), &games),
        &document(&format!("{romcenter}{clrmamepro}"), &games),
    )?;
    assert!(diff.document_metadata_changed);
    let record = only_record(&diff)?;
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::UniqueName
    );
    assert_unchanged(record, "native header option swap");
    Ok(())
}

#[test]
fn game_child_edits_change_metadata_without_changing_asset_evidence() -> TestResult {
    for (label, previous, current) in [
        (
            "comment text",
            "<comment>Before</comment>",
            "<comment>After</comment>",
        ),
        (
            "comment order",
            "<comment>A</comment><comment>B</comment>",
            "<comment>B</comment><comment>A</comment>",
        ),
        (
            "comment multiplicity",
            "<comment>A</comment>",
            "<comment>A</comment><comment>A</comment>",
        ),
        (
            "release name",
            "<release name='A' region='US'/>",
            "<release name='B' region='US'/>",
        ),
        (
            "release region",
            "<release name='A' region='US'/>",
            "<release name='A' region='EU'/>",
        ),
        (
            "release language",
            "<release name='A' region='US' language='en'/>",
            "<release name='A' region='US' language='fr'/>",
        ),
        (
            "release date",
            "<release name='A' region='US' date='1990'/>",
            "<release name='A' region='US' date='1991'/>",
        ),
        (
            "release default presence",
            "<release name='A' region='US'/>",
            "<release name='A' region='US' default='no'/>",
        ),
        (
            "release default value",
            "<release name='A' region='US' default='no'/>",
            "<release name='A' region='US' default='yes'/>",
        ),
        (
            "bios name",
            "<biosset name='a' description='Base'/>",
            "<biosset name='b' description='Base'/>",
        ),
        (
            "bios description",
            "<biosset name='a' description='Base'/>",
            "<biosset name='a' description='Alternate'/>",
        ),
        (
            "bios default presence",
            "<biosset name='a' description='Base'/>",
            "<biosset name='a' description='Base' default='no'/>",
        ),
        (
            "bios default value",
            "<biosset name='a' description='Base' default='no'/>",
            "<biosset name='a' description='Base' default='yes'/>",
        ),
        (
            "archive name",
            "<archive name='a.zip'/>",
            "<archive name='b.zip'/>",
        ),
        (
            "archive multiplicity",
            "<archive name='a.zip'/>",
            "<archive name='a.zip'/><archive name='a.zip'/>",
        ),
    ] {
        // Put the unchanged ROM first so child edits cannot move its source ordinal.
        let asset = rom("rom.bin", "16", "");
        let diff = diff_documents(
            &document("", &[game(&format!("{asset}{previous}"))]),
            &document("", &[game(&format!("{asset}{current}"))]),
        )?;
        assert!(!diff.document_metadata_changed, "{label}");
        let record = only_record(&diff)?;
        assert_eq!(record.status, SnapshotRecordStatus::Changed, "{label}");
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::UniqueName,
            "{label}"
        );
        assert!(record.metadata_changed, "{label}");
        assert!(!record.regrouped, "{label}");
        assert!(record.requirement_changes.is_empty(), "{label}");
    }
    Ok(())
}

#[test]
fn rom_leading_zero_edits_change_lossless_evidence_not_size_or_hash() -> TestResult {
    let diff = diff_documents(
        &document("", &[game(&rom("rom.bin", "16", ""))]),
        &document("", &[game(&rom("rom.bin", "00016", ""))]),
    )?;
    let change = single_evidence_change(&diff, "rom.bin")?;
    let previous = change
        .previous
        .as_ref()
        .ok_or("previous ROM evidence missing")?;
    let current = change
        .current
        .as_ref()
        .ok_or("current ROM evidence missing")?;
    assert_eq!(previous.pointer("/0/size"), Some(&serde_json::json!(16)));
    assert_eq!(current.pointer("/0/size"), Some(&serde_json::json!(16)));
    assert_eq!(
        previous.pointer("/0/logiqx_attributes/size_text"),
        Some(&serde_json::json!("16"))
    );
    assert_eq!(
        current.pointer("/0/logiqx_attributes/size_text"),
        Some(&serde_json::json!("00016"))
    );
    Ok(())
}

#[test]
fn rom_and_disk_explicit_good_status_changes_presence_not_effective_status() -> TestResult {
    for (name, previous, current) in [
        (
            "rom.bin",
            rom("rom.bin", "16", ""),
            rom("rom.bin", "16", "status='good'"),
        ),
        ("disk.chd", disk(""), disk("status='good'")),
    ] {
        let diff = diff_documents(
            &document("", &[game(&previous)]),
            &document("", &[game(&current)]),
        )?;
        let change = single_evidence_change(&diff, name)?;
        let previous = change
            .previous
            .as_ref()
            .ok_or("previous asset evidence missing")?;
        let current = change
            .current
            .as_ref()
            .ok_or("current asset evidence missing")?;
        assert_eq!(
            previous.pointer("/0/dump_status"),
            Some(&serde_json::json!("good")),
            "{name}"
        );
        assert_eq!(
            current.pointer("/0/dump_status"),
            Some(&serde_json::json!("good")),
            "{name}"
        );
        assert_eq!(
            previous.pointer("/0/logiqx_attributes/status_was_present"),
            Some(&serde_json::json!(false)),
            "{name}"
        );
        assert_eq!(
            current.pointer("/0/logiqx_attributes/status_was_present"),
            Some(&serde_json::json!(true)),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn native_cross_family_swaps_change_metadata_and_media_order() -> TestResult {
    for (name, media) in [
        ("rom.bin", rom("rom.bin", "16", "")),
        ("disk.chd", disk("")),
        ("intro", "<sample name='intro'/>".to_owned()),
    ] {
        let comment = "<comment>Stable</comment>";
        let diff = diff_documents(
            &document("", &[game(&format!("{comment}{media}"))]),
            &document("", &[game(&format!("{media}{comment}"))]),
        )?;
        assert!(!diff.document_metadata_changed);
        let record = only_record(&diff)?;
        assert_eq!(record.status, SnapshotRecordStatus::Changed, "{name}");
        assert!(record.metadata_changed, "{name}");
        assert_eq!(record.requirement_changes.len(), 1, "{name}");
        let change = &record.requirement_changes[0];
        assert_eq!(change.asset_name, name);
        assert_evidence_change(change);
        for (evidence, order) in [(&change.previous, 1), (&change.current, 0)] {
            assert_eq!(
                evidence
                    .as_ref()
                    .ok_or("media evidence missing")?
                    .pointer("/0/logiqx_attributes/native_order"),
                Some(&serde_json::json!(order)),
                "{name}"
            );
        }
    }
    Ok(())
}

#[test]
fn rom_source_order_changes_evidence_without_changing_game_metadata() -> TestResult {
    let first = rom("first.bin", "16", "");
    let second = rom("second.bin", "16", "");
    let diff = diff_documents(
        &document("", &[game(&format!("{first}{second}"))]),
        &document("", &[game(&format!("{second}{first}"))]),
    )?;
    assert!(!diff.document_metadata_changed);
    let record = only_record(&diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::UniqueName
    );
    assert!(!record.metadata_changed);
    assert!(!record.regrouped);
    assert_eq!(record.requirement_changes.len(), 2);
    for (name, previous_order, current_order) in [("first.bin", 0, 1), ("second.bin", 1, 0)] {
        let change = record
            .requirement_changes
            .iter()
            .find(|change| change.asset_name == name)
            .ok_or("reordered ROM evidence missing")?;
        assert_evidence_change(change);
        let previous = change
            .previous
            .as_ref()
            .ok_or("previous ROM evidence missing")?;
        let current = change
            .current
            .as_ref()
            .ok_or("current ROM evidence missing")?;
        assert_eq!(
            previous.pointer("/0/logiqx_attributes/native_order"),
            Some(&serde_json::json!(previous_order)),
            "{name}"
        );
        assert_eq!(
            current.pointer("/0/logiqx_attributes/native_order"),
            Some(&serde_json::json!(current_order)),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn vendor_child_insertions_between_native_children_leave_history_unchanged() -> TestResult {
    let options = "<clrmamepro forcemerging='full'/><romcenter lockrommode='yes'/>";
    let inserted_options =
        "<clrmamepro forcemerging='full'/><future/><romcenter lockrommode='yes'/>";
    let assets = format!(
        "{}{}",
        rom("rom.bin", "00016", "status='good'"),
        disk("status='good'")
    );
    let native_tail = "<release name='Release' region='US'/><biosset name='base' description='Base'/><archive name='container.zip'/><sample name='intro'/>";
    let children = format!("<comment>Stable</comment>{assets}{native_tail}");
    let inserted_children = format!("<comment>Stable</comment><future/>{assets}{native_tail}");
    let previous = document(options, &[game(&children)]);
    for (label, current_options, current_children) in [
        ("header insertion", inserted_options, &children),
        ("game insertion", options, &inserted_children),
        (
            "header and game insertions",
            inserted_options,
            &inserted_children,
        ),
    ] {
        let current = document(current_options, &[game(current_children)]);
        let diff = diff_documents(&previous, &current)?;
        assert!(!diff.document_metadata_changed, "{label}");
        let record = only_record(&diff)?;
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::UniqueName,
            "{label}"
        );
        assert_unchanged(record, label);
    }
    Ok(())
}

#[test]
fn vendor_gaps_across_digit_boundary_preserve_repeated_asset_evidence_multisets() -> TestResult {
    for (label, asset) in [
        ("repeated ROMs", rom("rom.bin", "16", "")),
        ("repeated disks", disk("")),
    ] {
        let previous_children = format!("<comment>Stable</comment>{asset}{asset}");
        // Raw media ordinals move from 2/3 to 9/10, reversing lexical JSON sort order.
        let current_children = format!(
            "<comment>Stable</comment>{}{asset}{asset}",
            "<future/>".repeat(7)
        );
        let diff = diff_documents(
            &document("", &[game(&previous_children)]),
            &document("", &[game(&current_children)]),
        )?;
        assert!(!diff.document_metadata_changed, "{label}");
        let record = only_record(&diff)?;
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::UniqueName,
            "{label}"
        );
        assert_unchanged(record, label);
    }
    Ok(())
}

#[test]
fn whitespace_only_source_position_moves_leave_all_native_history_unchanged() -> TestResult {
    let options = "<clrmamepro forcemerging='full'/><romcenter lockrommode='yes'/>";
    let children = format!(
        "<comment>A</comment><comment>B</comment>\
         <release name='Release' region='US' default='no'/>\
         <biosset name='base' description='Base' default='yes'/>\
         <archive name='container.zip'/>{}{}<sample name='intro'/>",
        rom("rom.bin", "00016", "status='good'"),
        disk("status='good'")
    );
    let compact = document(options, &[game(&children)]);
    let moved = format!("\n  {}\n", compact.replace("><", ">\n    <"));
    let diff = diff_documents(&compact, &moved)?;
    assert!(!diff.document_metadata_changed);
    let record = only_record(&diff)?;
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::UniqueName
    );
    assert_unchanged(record, "whitespace-only source-position moves");
    Ok(())
}

#[test]
fn repeated_name_native_owners_are_compared_as_complete_multisets() -> TestResult {
    for (family, first_metadata, second_metadata) in [
        ("comments", "<comment>A</comment>", "<comment>B</comment>"),
        (
            "releases",
            "<release name='A' region='US'/>",
            "<release name='B' region='EU'/>",
        ),
        (
            "bios sets",
            "<biosset name='a' description='A'/>",
            "<biosset name='b' description='B'/>",
        ),
        (
            "archives",
            "<archive name='a.zip'/>",
            "<archive name='b.zip'/>",
        ),
    ] {
        let first_rom = rom("first.bin", "16", "");
        let second_rom = rom("second.bin", "16", "");
        let first = game(&format!("{first_rom}{first_metadata}"));
        let second = game(&format!("{second_rom}{second_metadata}"));
        let previous = document("", &[first.clone(), first.clone(), second.clone()]);
        let permuted = document("", &[second.clone(), first.clone(), first.clone()]);
        let diff = diff_documents(&previous, &permuted)?;
        assert!(!diff.document_metadata_changed, "{family}");
        let record = only_record(&diff)?;
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::ExactFacts,
            "{family}"
        );
        assert_unchanged(record, family);

        // The metadata and asset multisets still match, but whole owners no longer do.
        let cross_assigned = document(
            "",
            &[
                game(&format!("{first_rom}{second_metadata}")),
                game(&format!("{second_rom}{first_metadata}")),
            ],
        );
        let pair = document("", &[first.clone(), second.clone()]);
        let diff = diff_documents(&pair, &cross_assigned)?;
        assert!(!diff.document_metadata_changed, "{family}");
        let record = only_record(&diff)?;
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::Ambiguous,
            "{family}"
        );
        assert_eq!(record.status, SnapshotRecordStatus::Changed, "{family}");
        assert!(!record.metadata_changed, "{family}");
        assert!(!record.regrouped, "{family}");
        assert!(record.requirement_changes.is_empty(), "{family}");

        // A repeated identical owner is not discarded as it would be by set comparison.
        let diff = diff_documents(&pair, &previous)?;
        assert!(!diff.document_metadata_changed, "{family}");
        let record = only_record(&diff)?;
        assert_eq!(
            record.correspondence,
            SnapshotRecordCorrespondence::Ambiguous,
            "{family}"
        );
        assert_eq!(record.status, SnapshotRecordStatus::Changed, "{family}");
        assert!(record.metadata_changed, "{family}");
        assert!(record.regrouped, "{family}");
        assert!(record.requirement_changes.is_empty(), "{family}");
    }
    Ok(())
}
