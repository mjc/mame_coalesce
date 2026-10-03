use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_machines::{MachineAssetKind, MachinePageLimit, machines_for_snapshot},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey, SnapshotRecordStatus},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const NATIVE_THEN_COMPAT: &str = r#"<mame xmlns:v="urn:vendor" mameconfig="10"><machine name="system" v:marker="retained" isconsumable="no"><description>System</description><rom name="system.rom" size="1"/></machine></mame>"#;

const COMPAT_THEN_NATIVE: &str = r#"<mame xmlns:v="urn:vendor" mameconfig="10"><machine isconsumable="no" name="system" v:marker="retained"><description>System</description><rom name="system.rom" size="1"/></machine></mame>"#;

const VENDOR_GAP_AND_LAYOUT_ONLY: &str = r#"<mame xmlns:v="urn:vendor" mameconfig="10">
  <machine v:marker="retained" name="system" isconsumable="no">
    <description>System</description>
    <rom name="system.rom" size="1"/>
  </machine>
</mame>"#;

fn request(path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("mame-attribute-history"),
        source_display_name: "MAME attribute history".to_owned(),
        catalog_key: CatalogKey::new("mame-attribute-history"),
        catalog_display_name: "MAME attribute history".to_owned(),
        scope: CatalogScope::Complete,
    }
}

fn import_snapshot(database: &Database, path: &Utf8PathBuf, xml: &str) -> TestResult<SnapshotKey> {
    std::fs::write(path, xml)?;
    let report = app::import_catalog(database, &request(path.clone()))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("published snapshot missing")?)
}

fn assert_rom_claim(database: &Database, snapshot: &SnapshotKey) -> TestResult {
    let page = machines_for_snapshot(database, snapshot, None, MachinePageLimit::new(10)?)?;
    let machine = page
        .machines
        .iter()
        .find(|machine| machine.name == "system")
        .ok_or("system machine missing from public query")?;
    assert_eq!(machine.assets.len(), 1);
    assert_eq!(machine.assets[0].kind, MachineAssetKind::Rom);
    assert_eq!(machine.assets[0].source_order, 1);
    Ok(())
}

#[test]
fn machine_attribute_history_tracks_native_compat_order_and_ignores_vendor_gaps() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let original_path = Utf8PathBuf::try_from(directory.path().join("native-then-compat.xml"))?;
    let crossed_path = Utf8PathBuf::try_from(directory.path().join("compat-then-native.xml"))?;
    let neutral_path = Utf8PathBuf::try_from(directory.path().join("vendor-gap-and-layout.xml"))?;

    let original = import_snapshot(&database, &original_path, NATIVE_THEN_COMPAT)?;
    let crossed = import_snapshot(&database, &crossed_path, COMPAT_THEN_NATIVE)?;
    let neutral = import_snapshot(&database, &neutral_path, VENDOR_GAP_AND_LAYOUT_ONLY)?;
    assert_ne!(original, crossed);
    assert_ne!(original, neutral);

    for snapshot in [&original, &crossed, &neutral] {
        assert_rom_claim(&database, snapshot)?;
    }

    let neutral_diff = app::diff_catalog_snapshots(&database, &original, &neutral)?;
    assert!(!neutral_diff.document_metadata_changed);
    let neutral_machine = neutral_diff
        .records
        .iter()
        .find(|record| record.set_name == "system")
        .ok_or("vendor/layout control machine history missing")?;
    assert_eq!(neutral_machine.status, SnapshotRecordStatus::Unchanged);
    assert!(!neutral_machine.metadata_changed);
    assert!(neutral_machine.requirement_changes.is_empty());

    let crossed_diff = app::diff_catalog_snapshots(&database, &original, &crossed)?;
    assert!(!crossed_diff.document_metadata_changed);
    let crossed_machine = crossed_diff
        .records
        .iter()
        .find(|record| record.set_name == "system")
        .ok_or("crossed machine history missing")?;
    assert!(crossed_machine.requirement_changes.is_empty());
    assert_eq!(crossed_machine.status, SnapshotRecordStatus::Changed);
    assert!(crossed_machine.metadata_changed);
    Ok(())
}

#[test]
fn added_and_removed_media_names_are_requirement_changes_not_machine_metadata() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let previous_path = Utf8PathBuf::try_from(directory.path().join("previous.xml"))?;
    let current_path = Utf8PathBuf::try_from(directory.path().join("current.xml"))?;
    let previous = import_snapshot(
        &database,
        &previous_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='old-rom'/><disk name='old-disk'/><sample name='retained-sample'/></machine></mame>",
    )?;
    let current = import_snapshot(
        &database,
        &current_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='new-rom'/><disk name='new-disk'/><sample name='retained-sample'/></machine></mame>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let machine = diff
        .records
        .iter()
        .find(|record| record.set_name == "system")
        .ok_or("media history machine missing")?;
    assert_eq!(machine.status, SnapshotRecordStatus::Changed);
    assert!(!machine.metadata_changed);
    assert_eq!(machine.requirement_changes.len(), 4);
    for (name, was_present) in [
        ("old-rom", true),
        ("new-rom", false),
        ("old-disk", true),
        ("new-disk", false),
    ] {
        let change = machine
            .requirement_changes
            .iter()
            .find(|change| change.asset_name == name)
            .ok_or("renamed media requirement change missing")?;
        assert_eq!(change.previous.is_some(), was_present, "{name}");
        assert_eq!(change.current.is_some(), !was_present, "{name}");
        assert!(!change.size_changed, "{name}");
        assert!(!change.hash_changed, "{name}");
    }
    Ok(())
}

#[test]
fn media_attribute_order_tracks_each_repeated_name_declaration() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let original_path = Utf8PathBuf::try_from(directory.path().join("original.xml"))?;
    let swapped_path = Utf8PathBuf::try_from(directory.path().join("swapped.xml"))?;
    let reordered_path = Utf8PathBuf::try_from(directory.path().join("reordered.xml"))?;
    let original = import_snapshot(
        &database,
        &original_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='repeat' size='1' crc='aaaaaaaa'/><rom name='repeat' crc='bbbbbbbb' size='2'/><disk name='repeat' writable='no' sha1='cccccccccccccccccccccccccccccccccccccccc'/></machine></mame>",
    )?;
    let swapped = import_snapshot(
        &database,
        &swapped_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='repeat' crc='aaaaaaaa' size='1'/><rom name='repeat' size='2' crc='bbbbbbbb'/><disk name='repeat' writable='no' sha1='cccccccccccccccccccccccccccccccccccccccc'/></machine></mame>",
    )?;
    let reordered = import_snapshot(
        &database,
        &reordered_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='repeat' crc='aaaaaaaa' size='1'/><rom name='repeat' crc='bbbbbbbb' size='2'/><disk name='repeat' writable='no' sha1='cccccccccccccccccccccccccccccccccccccccc'/></machine></mame>",
    )?;

    let swapped_diff = app::diff_catalog_snapshots(&database, &original, &swapped)?;
    let swapped_machine = swapped_diff
        .records
        .iter()
        .find(|record| record.set_name == "system")
        .ok_or("swapped duplicate-media history missing")?;
    assert_eq!(swapped_machine.status, SnapshotRecordStatus::Changed);
    assert!(swapped_machine.metadata_changed);
    assert!(swapped_machine.requirement_changes.is_empty());

    let reordered_diff = app::diff_catalog_snapshots(&database, &swapped, &reordered)?;
    let reordered_machine = reordered_diff
        .records
        .iter()
        .find(|record| record.set_name == "system")
        .ok_or("reordered-media history missing")?;
    assert_eq!(reordered_machine.status, SnapshotRecordStatus::Changed);
    assert!(reordered_machine.metadata_changed);
    assert!(reordered_machine.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn media_attribute_order_cancellation_keeps_kind_and_name_boundaries() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let baseline_path = Utf8PathBuf::try_from(directory.path().join("baseline.xml"))?;
    let baseline = import_snapshot(
        &database,
        &baseline_path,
        "<mame mameconfig='10'><machine name='system'><description>System</description><rom name='same' size='1'/><disk name='same' merge='missing'/><rom size='1' name='other'/></machine></mame>",
    )?;

    for (variant, xml) in [
        (
            "names",
            "<mame mameconfig='10'><machine name='system'><description>System</description><rom size='1' name='same'/><disk name='same' merge='missing'/><rom name='other' size='1'/></machine></mame>",
        ),
        (
            "kinds",
            "<mame mameconfig='10'><machine name='system'><description>System</description><rom size='1' name='same'/><disk merge='missing' name='same'/><rom size='1' name='other'/></machine></mame>",
        ),
    ] {
        let path = Utf8PathBuf::try_from(directory.path().join(format!("{variant}.xml")))?;
        let current = import_snapshot(&database, &path, xml)?;
        let diff = app::diff_catalog_snapshots(&database, &baseline, &current)?;
        let machine = diff
            .records
            .iter()
            .find(|record| record.set_name == "system")
            .ok_or("media-order cancellation history missing")?;
        assert_eq!(machine.status, SnapshotRecordStatus::Changed, "{variant}");
        assert!(machine.metadata_changed, "{variant}");
        assert!(machine.requirement_changes.is_empty(), "{variant}");
    }
    Ok(())
}
