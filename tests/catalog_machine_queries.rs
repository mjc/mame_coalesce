use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_machines::{MachinePageLimit, machines_for_snapshot},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn public_machine_pages_are_bounded_and_pin_the_native_owner() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(
        &document_path,
        "<mame build='0.289-test' mameconfig='10'>\
         <machine name='first'><description>First machine</description>\
           <rom name='program.bin' size='4'/><disk name='media.chd'/>\
         </machine>\
         <machine name='second'><description>Second machine</description></machine>\
         </mame>",
    )?;

    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("machine-query-regression"),
            source_display_name: "Machine query regression".to_owned(),
            catalog_key: CatalogKey::new("machine-query-regression"),
            catalog_display_name: "Machine query regression".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("import did not publish a snapshot")?;
    let limit = MachinePageLimit::new(1)?;

    let first = machines_for_snapshot(&database, &snapshot, None, limit)?;
    assert_eq!(first.snapshot.snapshot_key, snapshot);
    assert_eq!(first.snapshot.header.config_version, "10");
    assert_eq!(first.machines.len(), 1);
    assert_eq!(first.machines[0].name, "first");
    assert_eq!(first.machines[0].list_order, 0);
    assert_eq!(first.machines[0].assets.len(), 2);
    assert_eq!(
        first.machines[0].assets[0].kind,
        mame_coalesce::catalog_machines::MachineAssetKind::Rom
    );
    assert_eq!(
        first.machines[0].assets[1].kind,
        mame_coalesce::catalog_machines::MachineAssetKind::Disk
    );
    let cursor = first
        .next_cursor
        .as_ref()
        .ok_or("second machine page missing")?;

    let second = machines_for_snapshot(&database, &snapshot, Some(cursor), limit)?;
    assert_eq!(second.machines.len(), 1);
    assert_eq!(second.machines[0].name, "second");
    assert_eq!(second.machines[0].list_order, 1);
    assert_ne!(first.machines[0].id, second.machines[0].id);
    assert!(second.next_cursor.is_none());
    Ok(())
}

#[test]
fn public_machine_reader_returns_all_specification_families_and_config_locations() -> TestResult {
    use mame_coalesce::catalog_machines::{
        MachineSpecification as Spec, MachineSwitchLocationKind, MachineSwitchValueKind,
    };

    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("all-fields.xml"))?;
    std::fs::write(
        &document_path,
        include_str!("../fixtures/catalog/mame/all-fields.xml"),
    )?;

    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("machine-query-all-fields"),
            source_display_name: "Machine query all fields".to_owned(),
            catalog_key: CatalogKey::new("machine-query-all-fields"),
            catalog_display_name: "Machine query all fields".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("import did not publish a snapshot")?;
    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(10)?)?;
    let machine = page
        .machines
        .iter()
        .find(|machine| machine.name == "complete")
        .ok_or("complete machine missing")?;
    let families = machine
        .specification
        .iter()
        .map(|element| match &element.value {
            Spec::Sample(_) => "sample",
            Spec::Chip(_) => "chip",
            Spec::Display(_) => "display",
            Spec::Sound(_) => "sound",
            Spec::Input(_) => "input",
            Spec::Port(_) => "port",
            Spec::Adjuster(_) => "adjuster",
            Spec::Driver(_) => "driver",
            Spec::Feature(_) => "feature",
            Spec::Device(_) => "device",
            Spec::Slot(_) => "slot",
            Spec::SoftwareList(_) => "softwarelist",
            Spec::RamOption(_) => "ramoption",
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(families.len(), 13);
    assert_eq!(machine.switches.len(), 2);
    assert_eq!(machine.switches[1].locations.len(), 1);
    assert_eq!(machine.switches[1].locations[0].name, "JP1");
    assert_eq!(
        machine.switches[1].locations[0].kind,
        MachineSwitchLocationKind::ConfigurationLocation
    );
    assert_eq!(machine.switches[1].values[0].name, "");
    assert_eq!(
        machine.switches[1].values[0].kind,
        MachineSwitchValueKind::ConfigurationSetting
    );
    Ok(())
}
