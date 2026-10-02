use camino::Utf8PathBuf;

use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    mame::{self, Machine, MameRecord},
};

use super::{
    MachineAssetKind, MachineDependency, MachinePageLimit, MachineSwitchLocationKind,
    MachineSwitchValueKind, machines_for_snapshot,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BIOS_AND_ASSETS: &str = r#"<mame build="roundtrip" debug="yes" mameconfig="10">
  <machine name="declared" cloneof="base" romof="base" sampleof="samples"
           isdevice="no" runnable="yes" isbios="no" ismechanical="no" isconsumable="no">
    <vendor-child/>
    <description>Declared facts</description>
    <year>1994</year>
    <rom name="declared.rom" size="16" crc="12345678" optional="yes"/>
    <manufacturer>Example</manufacturer>
    <biosset name="primary" description="Primary BIOS" default="yes"/>
    <disk name="declared.chd" sha1="0123456789abcdef0123456789abcdef01234567"
          writable="no" status="good"/>
    <device_ref name="cart" tag=":cart"/>
    <dipswitch name="Raw DIP" tag=":DSW" mask="0x00AF">
      <vendor-value/>
      <diplocation name="SW1" number="2" inverted="yes"/>
      <dipvalue name="Hex choice" value="0x00A0" default="yes"/>
    </dipswitch>
    <configuration name="Raw config" tag=":CFG" mask="0x0F">
      <confsetting name="" value="0x0A"/>
    </configuration>
  </machine>
  <machine name="base">
    <description>Base</description>
    <biosset name="fallback" description="Fallback BIOS"/>
    <rom name="base.rom" size="1"/>
    <disk name="base.chd" sha1="0123456789abcdef0123456789abcdef01234567"/>
  </machine>
</mame>"#;

#[test]
fn published_machine_pages_roundtrip_typed_parser_models_without_source_xml() -> TestResult {
    let directory = tempfile::tempdir()?;
    let all_fields = Utf8PathBuf::try_from(directory.path().join("all-fields.xml"))?;
    std::fs::copy(
        "fixtures/catalog/mame/all-fields.xml",
        all_fields.as_std_path(),
    )?;
    let bios_assets = Utf8PathBuf::try_from(directory.path().join("bios-assets.xml"))?;
    std::fs::write(&bios_assets, BIOS_AND_ASSETS)?;

    roundtrip_document(&directory, &all_fields, "roundtrip-all-fields")?;
    roundtrip_document(&directory, &bios_assets, "roundtrip-bios-assets")?;
    Ok(())
}

fn roundtrip_document(
    directory: &tempfile::TempDir,
    document_path: &Utf8PathBuf,
    key: &str,
) -> TestResult {
    let bytes = std::fs::read(document_path)?;
    let (expected_header, expected_machines) = parse_expected(&bytes)?;
    let database_path = Utf8PathBuf::try_from(directory.path().join(format!("{key}.sqlite")))?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document_path.clone(),
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new(key),
            source_display_name: key.to_owned(),
            catalog_key: CatalogKey::new(key),
            catalog_display_name: key.to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("import did not publish a snapshot")?;

    // The query must be backed by stored native facts, not a second read of its input.
    std::fs::remove_file(document_path)?;
    assert!(!document_path.exists());

    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(100)?)?;
    assert_eq!(page.snapshot.snapshot_key, snapshot);
    assert_eq!(page.snapshot.header.build, expected_header.build);
    assert_eq!(page.snapshot.header.debug, expected_header.debug);
    assert_eq!(
        page.snapshot.header.debug_specified,
        expected_header.debug_specified
    );
    assert_eq!(
        page.snapshot.header.config_version,
        expected_header.config_version
    );
    assert_eq!(page.snapshot.header.location, expected_header.location);
    assert!(page.next_cursor.is_none());
    assert_eq!(page.machines.len(), expected_machines.len());

    for (order, (actual, expected)) in page.machines.iter().zip(&expected_machines).enumerate() {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.list_order, i64::try_from(order)?);
        assert_eq!(actual.location, expected.location);
        assert_eq!(actual.facts, expected.facts);
        assert_eq!(actual.bios_sets, expected.bios_sets);
        assert_switches(&actual.switches, &expected.switches);
        assert_eq!(actual.specification, expected.specification);
        assert_dependencies(actual, expected);
        assert_asset_references(actual, expected)?;
    }

    assert_fixture_witnesses(&page, &expected_machines, key)
}

fn assert_fixture_witnesses(
    page: &super::MachinePage,
    expected_machines: &[Machine],
    key: &str,
) -> TestResult {
    if key == "roundtrip-all-fields" {
        let defaults = page
            .machines
            .iter()
            .find(|machine| machine.name == "defaults")
            .ok_or("all-fields defaults machine missing")?;
        assert!(!defaults.facts.flags.is_device_specified());
        assert!(!defaults.facts.flags.is_runnable_specified());
        assert!(!defaults.facts.flags.is_bios_specified());
        assert!(!defaults.facts.flags.is_mechanical_specified());
        assert!(!defaults.facts.flags.is_consumable_specified());
        let complete = page
            .machines
            .iter()
            .find(|machine| machine.name == "complete")
            .ok_or("all-fields complete machine missing")?;
        assert_eq!(complete.specification.len(), 14);
        assert!(
            complete
                .switches
                .iter()
                .any(|switch| switch.kind == mame::MachineSwitchKind::DipSwitch)
        );
        assert!(
            complete
                .switches
                .iter()
                .any(|switch| switch.kind == mame::MachineSwitchKind::Configuration)
        );
    } else {
        let declared = &page.machines[0];
        let parsed_declared = &expected_machines[0];
        assert_eq!(declared.bios_sets.len(), 1);
        assert!(declared.bios_sets[0].default_specified);
        assert!(!page.machines[1].bios_sets[0].default_specified);
        assert_eq!(declared.assets.len(), 2);
        assert_eq!(declared.assets[0].kind, MachineAssetKind::Rom);
        assert_eq!(declared.assets[1].kind, MachineAssetKind::Disk);
        assert_ne!(
            declared.assets[0].occurrence_id,
            declared.assets[1].occurrence_id
        );
        assert!(parsed_declared.assets[0].attributes.optional_specified);
        assert!(!parsed_declared.assets[0].attributes.status_specified);
        assert!(!parsed_declared.assets[1].attributes.optional_specified);
        assert!(parsed_declared.assets[1].attributes.status_specified);
        assert!(parsed_declared.assets[1].attributes.writable_specified);
        assert!(!expected_machines[1].bios_sets[0].default_specified);
        assert!(!expected_machines[1].assets[0].attributes.optional_specified);
        assert!(!expected_machines[1].assets[0].attributes.status_specified);
        assert!(!expected_machines[1].assets[1].attributes.writable_specified);
        assert!(page.snapshot.header.debug);
        assert!(page.snapshot.header.debug_specified);
        assert!(declared.facts.flags.is_device_specified());
        assert!(declared.facts.flags.is_runnable_specified());
        assert!(declared.facts.flags.is_bios_specified());
        assert!(declared.facts.flags.is_mechanical_specified());
        assert!(declared.facts.flags.is_consumable_specified());
        assert_eq!(declared.switches[0].mask, "0x00AF");
        assert_eq!(declared.switches[0].values[0].value, "0x00A0");
        assert!(declared.switches[0].locations[0].inverted_specified);
        assert!(!declared.switches[1].values[0].default_specified);
        assert_eq!(declared.switches[1].values[0].name, "");
        // Direct-child ordinals retain the vendor nodes' gaps instead of compacting known rows.
        assert_eq!(declared.facts.description_source_order, 1);
        assert_eq!(declared.facts.year_source_order, Some(2));
        assert_eq!(declared.facts.manufacturer_source_order, Some(4));
        assert_eq!(declared.assets[0].source_order, 3);
        assert_eq!(declared.assets[1].source_order, 6);
        assert_eq!(declared.switches[0].source_order, 8);
        assert_eq!(declared.switches[0].locations[0].source_order, 1);
        assert_eq!(declared.switches[0].values[0].source_order, 2);
    }
    Ok(())
}

fn parse_expected(bytes: &[u8]) -> TestResult<(mame::MameHeader, Vec<Machine>)> {
    let mut header = None;
    let parsed = mame::read_with::<_, crate::Error>(
        bytes,
        |parsed_header| {
            header = Some(parsed_header);
            Ok(Vec::new())
        },
        |machines, record| {
            if let MameRecord::Machine(machine) = record {
                machines.push(*machine);
            }
            Ok(())
        },
    )?;
    Ok((
        header.ok_or("stream reader did not provide its header")?,
        parsed.into_inner(),
    ))
}

fn assert_dependencies(actual: &super::Machine, expected: &Machine) {
    let mut expected_count = 0;
    for (target_name, dependency) in [
        (expected.parent.as_ref(), 0),
        (expected.rom_of.as_ref(), 1),
        (expected.sample_of.as_ref(), 2),
    ] {
        if let Some(target_name) = target_name {
            expected_count += 1;
            let expected_dependency = match dependency {
                0 => MachineDependency::CloneOf {
                    target_name: target_name.clone(),
                    location: expected.location,
                },
                1 => MachineDependency::RomOf {
                    target_name: target_name.clone(),
                    location: expected.location,
                },
                _ => MachineDependency::SampleOf {
                    target_name: target_name.clone(),
                    location: expected.location,
                },
            };
            assert!(actual.dependencies.contains(&expected_dependency));
        }
    }
    for reference in &expected.device_refs {
        expected_count += 1;
        assert!(
            actual
                .dependencies
                .contains(&MachineDependency::DeviceReference(reference.clone()))
        );
    }
    assert_eq!(actual.dependencies.len(), expected_count);
}

fn assert_switches(actual: &[super::MachineSwitch], expected: &[mame::MachineSwitch]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.kind, expected.kind);
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.tag, expected.tag);
        assert_eq!(actual.mask, expected.mask);
        assert_eq!(actual.source_order, expected.source_order);
        assert_eq!(actual.location, expected.location);
        assert_eq!(actual.condition, expected.condition);
        let location_kind = match expected.kind {
            mame::MachineSwitchKind::DipSwitch => MachineSwitchLocationKind::DipLocation,
            mame::MachineSwitchKind::Configuration => {
                MachineSwitchLocationKind::ConfigurationLocation
            }
        };
        assert_eq!(actual.locations.len(), expected.locations.len());
        for (actual, expected) in actual.locations.iter().zip(&expected.locations) {
            assert_eq!(actual.kind, location_kind);
            assert_eq!(actual.name, expected.name);
            assert_eq!(actual.number, expected.number);
            assert_eq!(actual.inverted, expected.inverted);
            assert_eq!(actual.inverted_specified, expected.inverted_specified);
            assert_eq!(actual.source_order, expected.source_order);
            assert_eq!(actual.location, expected.location);
        }
        let value_kind = match expected.kind {
            mame::MachineSwitchKind::DipSwitch => MachineSwitchValueKind::DipValue,
            mame::MachineSwitchKind::Configuration => MachineSwitchValueKind::ConfigurationSetting,
        };
        assert_eq!(actual.values.len(), expected.values.len());
        for (actual, expected) in actual.values.iter().zip(&expected.values) {
            assert_eq!(actual.kind, value_kind);
            assert_eq!(actual.name, expected.name);
            assert_eq!(actual.value, expected.value);
            assert_eq!(actual.default, expected.default);
            assert_eq!(actual.default_specified, expected.default_specified);
            assert_eq!(actual.condition, expected.condition);
            assert_eq!(actual.source_order, expected.source_order);
            assert_eq!(actual.location, expected.location);
        }
    }
}

fn assert_asset_references(actual: &super::Machine, expected: &Machine) -> TestResult {
    assert_eq!(actual.assets.len(), expected.assets.len());
    for (reference, asset) in actual.assets.iter().zip(&expected.assets) {
        let expected_kind = match asset.role {
            crate::domain::AssetRole::Rom => MachineAssetKind::Rom,
            crate::domain::AssetRole::Disk => MachineAssetKind::Disk,
            crate::domain::AssetRole::Other => {
                return Err("unexpected non-ROM/disk MAME asset".into());
            }
        };
        assert_eq!(reference.kind, expected_kind);
        assert_eq!(reference.source_order, asset.source_order);
        assert_eq!(reference.location, asset.location);
    }
    Ok(())
}
