#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{self, MameFilePayload, XmlAttributePosition},
    catalog_machines::{
        self, AttributePosition, Machine, MachineDependency, MachinePageLimit, MachineSpecification,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};
use quick_xml::{Reader, events::Event};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const XML: &str = include_str!("../fixtures/specifications/mame-machine-fields.xml");
const DTD: &str = include_str!("../fixtures/specifications/mame-0.289.dtd");

struct Family {
    element: &'static str,
    index: usize,
    owner: &'static str,
    keys: &'static str,
    fields: &'static [&'static str],
    selected: &'static str,
    compatibility: bool,
}

macro_rules! family {
    ($element:literal,$index:literal,$owner:literal,$keys:literal,[$($field:literal),+],$selected:literal) => {
        Family { element:$element,index:$index,owner:$owner,keys:$keys,fields:&[$($field),+],selected:$selected,compatibility:false }
    };
    (compat $element:literal,$owner:literal,$keys:literal,[$($field:literal),+]) => {
        Family { element:$element,index:0,owner:$owner,keys:$keys,fields:&[$($field),+],selected:"1",compatibility:true }
    };
}

// Independent of production selectors, enums, expected views and family codes.
// Repeated XML names exercise the complete real SQL keys, including condition_order.
const FAMILIES: &[Family] = &[
    family!(
        "mame",
        0,
        "mame_document_facts",
        "document_id",
        ["build", "debug", "mameconfig"],
        "1"
    ),
    family!(
        "machine",
        0,
        "mame_machines",
        "set_id",
        [
            "name",
            "sourcefile",
            "isbios",
            "isdevice",
            "ismechanical",
            "runnable",
            "cloneof",
            "romof",
            "sampleof"
        ],
        "1"
    ),
    family!(
        "biosset",
        0,
        "mame_bios_sets",
        "set_id,bios_order",
        ["name", "description", "default"],
        "native.bios_order=0"
    ),
    family!(
        "rom",
        0,
        "mame_rom_claims",
        "occurrence_id",
        [
            "name", "bios", "size", "crc", "sha1", "merge", "region", "offset", "status",
            "optional"
        ],
        "1"
    ),
    family!(
        "disk",
        0,
        "mame_disk_claims",
        "occurrence_id",
        [
            "name", "sha1", "merge", "region", "index", "writable", "status", "optional"
        ],
        "1"
    ),
    family!(
        "device_ref",
        0,
        "mame_device_references",
        "set_id,reference_order",
        ["tag", "name"],
        "native.reference_order=0"
    ),
    family!("sample", 0, "mame_samples", "occurrence_id", ["name"], "1"),
    family!(
        "chip",
        0,
        "mame_machine_chips",
        "set_id,element_order",
        ["name", "tag", "type", "clock"],
        "1"
    ),
    family!(
        "display",
        0,
        "mame_machine_displays",
        "set_id,element_order",
        [
            "tag", "type", "rotate", "flipx", "width", "height", "refresh", "pixclock", "htotal",
            "hbend", "hbstart", "vtotal", "vbend", "vbstart"
        ],
        "1"
    ),
    family!(
        "sound",
        0,
        "mame_machine_sounds",
        "set_id,element_order",
        ["channels"],
        "1"
    ),
    family!(
        "input",
        0,
        "mame_machine_inputs",
        "set_id,element_order",
        ["service", "tilt", "players", "coins"],
        "1"
    ),
    family!(
        "control",
        0,
        "mame_machine_input_controls",
        "set_id,element_order,control_order",
        [
            "type",
            "player",
            "buttons",
            "minimum",
            "maximum",
            "sensitivity",
            "keydelta",
            "reverse",
            "ways",
            "ways2",
            "ways3"
        ],
        "native.control_order=0"
    ),
    family!(
        "dipswitch",
        0,
        "machine_switches",
        "set_id,switch_order",
        ["name", "tag", "mask"],
        "native.switch_order=0"
    ),
    family!(
        "diplocation",
        0,
        "machine_switch_locations",
        "set_id,switch_order,location_order",
        ["name", "number", "inverted"],
        "native.switch_order=0 AND native.location_order=0"
    ),
    family!(
        "dipvalue",
        0,
        "machine_switch_values",
        "set_id,switch_order,value_order",
        ["name", "value", "default"],
        "native.switch_order=0 AND native.value_order=0"
    ),
    family!(
        "condition",
        0,
        "machine_switch_conditions",
        "set_id,switch_order,condition_order",
        ["tag", "mask", "relation", "value"],
        "native.switch_order=0 AND native.condition_order=0"
    ),
    family!(
        "condition",
        1,
        "machine_switch_value_conditions",
        "set_id,switch_order,value_order,condition_order",
        ["tag", "mask", "relation", "value"],
        "native.switch_order=0 AND native.value_order=0 AND native.condition_order=0"
    ),
    family!(
        "condition",
        4,
        "mame_machine_adjuster_conditions",
        "set_id,element_order,condition_order",
        ["tag", "mask", "relation", "value"],
        "native.condition_order=0"
    ),
    family!(
        "port",
        0,
        "mame_machine_ports",
        "set_id,element_order",
        ["tag"],
        "1"
    ),
    family!(
        "analog",
        0,
        "mame_machine_analogs",
        "set_id,element_order,analog_order",
        ["mask"],
        "native.analog_order=0"
    ),
    family!(
        "adjuster",
        0,
        "mame_machine_adjusters",
        "set_id,element_order",
        ["name", "default"],
        "1"
    ),
    family!(
        "driver",
        0,
        "mame_machine_drivers",
        "set_id,element_order",
        [
            "status",
            "emulation",
            "cocktail",
            "savestate",
            "requiresartwork",
            "unofficial",
            "nosoundhardware",
            "incomplete"
        ],
        "1"
    ),
    family!(
        "feature",
        0,
        "mame_machine_features",
        "set_id,element_order",
        ["type", "status", "overall"],
        "1"
    ),
    family!(
        "device",
        0,
        "mame_machine_devices",
        "set_id,element_order",
        ["type", "tag", "fixed_image", "mandatory", "interface"],
        "1"
    ),
    family!(
        "instance",
        0,
        "mame_machine_device_instances",
        "set_id,element_order",
        ["name", "briefname"],
        "1"
    ),
    family!(
        "extension",
        0,
        "mame_machine_device_extensions",
        "set_id,element_order,extension_order",
        ["name"],
        "native.extension_order=0"
    ),
    family!(
        "slot",
        0,
        "mame_machine_slots",
        "set_id,element_order",
        ["name"],
        "1"
    ),
    family!(
        "slotoption",
        0,
        "mame_machine_slot_options",
        "set_id,element_order,option_order",
        ["name", "devname", "default"],
        "native.option_order=0"
    ),
    family!(
        "softwarelist",
        0,
        "mame_machine_software_lists",
        "set_id,element_order",
        ["tag", "name", "status", "filter"],
        "1"
    ),
    family!(
        "ramoption",
        0,
        "mame_machine_ram_options",
        "set_id,element_order",
        ["name", "default"],
        "1"
    ),
    family!(compat "machine","mame_machine_compatibility","set_id",["isconsumable"]),
    family!(compat "rom","mame_rom_compatibility","occurrence_id",["md5","soundonly","dispose","loadflag","value","inverted","ovha","nothread"]),
    family!(compat "disk","mame_disk_compatibility","occurrence_id",["writeable"]),
    family!(
        "configuration",
        0,
        "machine_switches",
        "set_id,switch_order",
        ["name", "tag", "mask"],
        "native.switch_order=1"
    ),
    family!(
        "conflocation",
        0,
        "machine_switch_locations",
        "set_id,switch_order,location_order",
        ["name", "number", "inverted"],
        "native.switch_order=1 AND native.location_order=0"
    ),
    family!(
        "confsetting",
        0,
        "machine_switch_values",
        "set_id,switch_order,value_order",
        ["name", "value", "default"],
        "native.switch_order=1 AND native.value_order=0"
    ),
    family!(
        "condition",
        2,
        "machine_switch_conditions",
        "set_id,switch_order,condition_order",
        ["tag", "mask", "relation", "value"],
        "native.switch_order=1 AND native.condition_order=0"
    ),
    family!(
        "condition",
        3,
        "machine_switch_value_conditions",
        "set_id,switch_order,value_order,condition_order",
        ["tag", "mask", "relation", "value"],
        "native.switch_order=1 AND native.value_order=0 AND native.condition_order=0"
    ),
    family!(
        "extension",
        1,
        "mame_machine_device_extensions",
        "set_id,element_order,extension_order",
        ["name"],
        "native.extension_order=1"
    ),
];

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct Position {
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

fn location(xml: &str, offset: usize) -> TestResult<(i64, i64)> {
    let prefix = xml
        .get(..offset)
        .ok_or("invalid independent token offset")?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    Ok((
        i64::try_from(prefix.chars().filter(|&c| c == '\n').count())? + 1,
        i64::try_from(
            prefix
                .rsplit('\n')
                .next()
                .ok_or("last line")?
                .chars()
                .count(),
        )? + 1,
    ))
}

fn expected(xml: &str, family: &Family) -> TestResult<Vec<Position>> {
    let mut reader = Reader::from_str(xml);
    let mut index = 0;
    loop {
        let start = match reader.read_event()? {
            Event::Start(start) | Event::Empty(start)
                if start.name().as_ref() == family.element =>
            {
                start
            }
            Event::Eof => {
                return Err(format!("missing {}[{}]", family.element, family.index).into());
            }
            _ => continue,
        };
        if index != family.index {
            index += 1;
            continue;
        }
        let mut result = Vec::new();
        for (source_order, attribute) in start.attributes().enumerate() {
            let attribute = attribute?;
            let Some(field_kind) = family
                .fields
                .iter()
                .position(|field| attribute.key.as_ref() == *field)
            else {
                continue;
            };
            let offset = (attribute.key.as_ref().as_ptr() as usize)
                .checked_sub(xml.as_ptr() as usize)
                .ok_or("fixture token not borrowed")?;
            let (source_line, source_column) = location(xml, offset)?;
            result.push(Position {
                field_kind: i64::try_from(field_kind)?,
                source_order: i64::try_from(source_order)?,
                source_line,
                source_column,
            });
        }
        return Ok(result);
    }
}

fn import(
    directory: &tempfile::TempDir,
    bytes: &[u8],
) -> TestResult<(Database, Utf8PathBuf, SnapshotKey)> {
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("machine.xml"))?;
    std::fs::write(&document_path, bytes)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("attribute-provenance"),
            source_display_name: "MAME attributes".into(),
            catalog_key: CatalogKey::new("attribute-provenance"),
            catalog_display_name: "MAME attributes".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("published snapshot missing")?;
    assert_eq!(app::load_snapshot_source(&database, &snapshot)?, bytes);
    Ok((database, path, snapshot))
}

fn check_positions(path: &Utf8PathBuf, snapshot: &SnapshotKey, xml: &str) -> TestResult {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for family in FAMILIES {
        let table = format!("{}_attribute_positions", family.owner);
        let (joins, selected) = if family.keys == "document_id" {
            (
                "JOIN catalog_snapshots AS groups ON groups.snapshot_key=native.snapshot_key",
                "1",
            )
        } else if family.keys == "occurrence_id" {
            (
                "JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
                "sets.set_name='complete'",
            )
        } else {
            (
                "JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id)",
                "sets.set_name='complete'",
            )
        };
        let rows = sql_query(format!("SELECT position.field_kind,position.source_order,position.source_line,position.source_column FROM {} AS native {joins} JOIN {table} AS position USING({}) WHERE groups.snapshot_key=? AND {selected} AND {} ORDER BY position.source_order", family.owner,family.keys,family.selected))
            .bind::<Text,_>(snapshot.as_str()).load::<Position>(&mut connection)?;
        assert_eq!(
            rows,
            expected(xml, family)?,
            "{}[{}] actual complete {} owner",
            family.element,
            family.index,
            family.owner
        );
    }
    Ok(())
}

#[test]
fn independent_dictionary_matches_125_pinned_and_ten_compatibility_attributes() -> TestResult {
    let pinned = DTD
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!ATTLIST "))
        .map(|line| {
            let mut words = line.split_whitespace();
            Ok((
                words.next().ok_or("DTD element")?,
                words.next().ok_or("DTD field")?,
            ))
        })
        .collect::<TestResult<BTreeSet<_>>>()?;
    let inventory = |compatibility| {
        FAMILIES
            .iter()
            .filter(move |family| family.compatibility == compatibility)
            .flat_map(|family| {
                family
                    .fields
                    .iter()
                    .map(move |field| (family.element, *field))
            })
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(pinned.len(), 125);
    assert_eq!(inventory(false), pinned);
    assert_eq!(inventory(true).len(), 10);
    assert_eq!(
        FAMILIES
            .iter()
            .map(|family| family.owner)
            .collect::<BTreeSet<_>>()
            .len(),
        33
    );
    Ok(())
}

#[test]
fn all_machine_attribute_families_have_actual_owner_qname_witnesses() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = XML.replace('\n', "\r\n");
    let (_database, path, snapshot) = import(&directory, xml.as_bytes())?;
    check_positions(&path, &snapshot, &xml)
}

#[test]
fn encoded_and_compressed_machine_attributes_keep_decoded_positions_and_originals() -> TestResult {
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;
    let xml = XML
        .replace("encoding=\"UTF-8\"", "encoding=\"UTF-16\"")
        .replace('\n', "\r\n");
    let mut little = vec![0xff, 0xfe];
    let mut big = vec![0xfe, 0xff];
    for code in xml.encode_utf16() {
        little.extend_from_slice(&code.to_le_bytes());
        big.extend_from_slice(&code.to_be_bytes());
    }
    let mut compressor = GzEncoder::new(Vec::new(), Compression::fast());
    compressor.write_all(&little)?;
    for bytes in [little, big, compressor.finish()?] {
        let directory = tempfile::tempdir()?;
        let (_database, path, snapshot) = import(&directory, &bytes)?;
        check_positions(&path, &snapshot, &xml)?;
    }
    Ok(())
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type=Text)]
    value: String,
}

#[test]
fn all_machine_position_companions_are_numeric_rowid_free_and_value_free() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&path)?);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let mut seen = BTreeSet::new();
    for family in FAMILIES {
        if !seen.insert(family.owner) {
            continue;
        }
        let table = format!("{}_attribute_positions", family.owner);
        let schema =
            sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='table' AND name=?")
                .bind::<Text, _>(&table)
                .get_result::<TextRow>(&mut connection)?
                .value;
        assert!(schema.contains("WITHOUT ROWID"), "{table}");
        let columns = sql_query(format!(
            "SELECT name AS value FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .load::<TextRow>(&mut connection)?;
        let expected = family
            .keys
            .split(',')
            .chain(["field_kind", "source_order", "source_line", "source_column"])
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            columns
                .into_iter()
                .map(|row| row.value)
                .collect::<BTreeSet<_>>(),
            expected,
            "no copied literals or other projections in {table}"
        );
        assert!(
            sql_query(format!(
                "SELECT name AS value FROM pragma_table_info('{table}') WHERE type<>'INTEGER'"
            ))
            .load::<TextRow>(&mut connection)?
            .is_empty(),
            "{table}"
        );
        let foreign_keys = sql_query(format!(
            "SELECT \"from\" AS value FROM pragma_foreign_key_list('{table}') WHERE \"table\"='{}'",
            family.owner
        ))
        .load::<TextRow>(&mut connection)?;
        assert_eq!(
            foreign_keys
                .into_iter()
                .map(|row| row.value)
                .collect::<BTreeSet<_>>(),
            family.keys.split(',').map(str::to_owned).collect(),
            "complete actual owner FK: {table}"
        );
    }
    Ok(())
}

fn field_family(element: &str, index: usize, compatibility: bool) -> &'static Family {
    FAMILIES
        .iter()
        .find(|family| {
            family.element == element
                && family.index == index
                && family.compatibility == compatibility
        })
        .expect("independent owner dictionary entry")
}

fn public_expected(family: &Family) -> TestResult<Vec<(&'static str, i64, i64, i64)>> {
    expected(XML, family)?
        .into_iter()
        .map(|position| {
            let field = family
                .fields
                .get(usize::try_from(position.field_kind)?)
                .ok_or("independent field code")?;
            Ok((
                *field,
                position.source_order,
                position.source_line,
                position.source_column,
            ))
        })
        .collect()
}

fn check_core<Field: Copy>(
    family: &Family,
    positions: &[AttributePosition<Field>],
    name: fn(Field) -> &'static str,
) -> TestResult {
    let actual = positions
        .iter()
        .map(|position| {
            Ok((
                name(position.field),
                i64::try_from(position.source_order)?,
                position.location.line,
                position.location.column,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        actual,
        public_expected(family)?,
        "public {}[{}] compatibility={}",
        family.element,
        family.index,
        family.compatibility
    );
    Ok(())
}

fn check_file<Field: Copy>(
    family: &Family,
    positions: &[XmlAttributePosition<Field>],
    name: fn(Field) -> &'static str,
) -> TestResult {
    let actual = positions
        .iter()
        .map(|position| {
            (
                name(position.field),
                position.source_order,
                position.location.line,
                position.location.column,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        public_expected(family)?,
        "public media {} compatibility={}",
        family.element,
        family.compatibility
    );
    Ok(())
}

macro_rules! check_core {
    ($element:literal, $index:expr, $positions:expr) => {
        check_core(field_family($element, $index, false), $positions, |field| {
            field.as_str()
        })?
    };
}

fn check_switches(machine: &Machine) -> TestResult {
    let mut condition_index = 0;
    for (index, switch) in machine.switches.iter().enumerate() {
        let (tag, location_tag, value_tag) = match index {
            0 => ("dipswitch", "diplocation", "dipvalue"),
            1 => ("configuration", "conflocation", "confsetting"),
            _ => return Err("unexpected synthetic switch".into()),
        };
        check_core(
            field_family(tag, 0, false),
            &switch.attribute_positions,
            catalog_machines::MameSwitchAttribute::as_str,
        )?;
        check_core!(
            "condition",
            condition_index,
            &switch
                .condition
                .as_ref()
                .expect("switch condition")
                .attribute_positions
        );
        condition_index += 1;
        let location = switch.locations.first().expect("switch location");
        check_core(
            field_family(location_tag, 0, false),
            &location.attribute_positions,
            catalog_machines::MameSwitchLocationAttribute::as_str,
        )?;
        let value = switch.values.first().expect("switch value");
        check_core(
            field_family(value_tag, 0, false),
            &value.attribute_positions,
            catalog_machines::MameSwitchValueAttribute::as_str,
        )?;
        check_core!(
            "condition",
            condition_index,
            &value
                .condition
                .as_ref()
                .expect("value condition")
                .attribute_positions
        );
        condition_index += 1;
    }
    assert_eq!(condition_index, 4);
    Ok(())
}

fn check_hardware(machine: &Machine) -> TestResult {
    for element in &machine.specification {
        match &element.value {
            MachineSpecification::Sample(value) => {
                check_core!("sample", 0, &value.attribute_positions);
            }
            MachineSpecification::Chip(value) => check_core!("chip", 0, &value.attribute_positions),
            MachineSpecification::Display(value) => {
                check_core!("display", 0, &value.attribute_positions);
            }
            MachineSpecification::Sound(value) => {
                check_core!("sound", 0, &value.attribute_positions);
            }
            MachineSpecification::Input(value) => {
                check_core!("input", 0, &value.attribute_positions);
                check_core!(
                    "control",
                    0,
                    &value.controls.first().expect("control").attribute_positions
                );
            }
            MachineSpecification::Port(value) => {
                check_core!("port", 0, &value.attribute_positions);
                check_core!(
                    "analog",
                    0,
                    &value.analogs.first().expect("analog").attribute_positions
                );
            }
            MachineSpecification::Adjuster(value) => {
                check_core!("adjuster", 0, &value.attribute_positions);
                check_core!(
                    "condition",
                    4,
                    &value
                        .condition
                        .as_ref()
                        .expect("adjuster condition")
                        .attribute_positions
                );
            }
            MachineSpecification::Driver(value) => {
                check_core!("driver", 0, &value.attribute_positions);
            }
            MachineSpecification::Feature(value) => {
                check_core!("feature", 0, &value.attribute_positions);
            }
            MachineSpecification::Device(value) => {
                check_core!("device", 0, &value.attribute_positions);
                check_core!(
                    "instance",
                    0,
                    &value
                        .instance
                        .as_ref()
                        .expect("device instance")
                        .attribute_positions
                );
                for (index, extension) in value.extensions.iter().enumerate() {
                    check_core!("extension", index, &extension.attribute_positions);
                }
            }
            MachineSpecification::Slot(value) => {
                check_core!("slot", 0, &value.attribute_positions);
                check_core!(
                    "slotoption",
                    0,
                    &value
                        .options
                        .first()
                        .expect("slot option")
                        .attribute_positions
                );
            }
            MachineSpecification::SoftwareList(value) => {
                check_core!("softwarelist", 0, &value.attribute_positions);
            }
            MachineSpecification::RamOption(value) => {
                check_core!("ramoption", 0, &value.attribute_positions);
            }
        }
    }
    assert_eq!(machine.specification.len(), 13);
    Ok(())
}

fn check_media(database: &Database, machine: &Machine) -> TestResult {
    let ids = machine
        .assets
        .iter()
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = catalog_files::occurrences_for_ids(database, &ids)?;
    assert_eq!(files.len(), 3);
    for occurrence in files {
        assert!(
            occurrence.content_id.is_none(),
            "compatibility loading ROM, CHD disk and sample have no whole-file identity"
        );
        match occurrence.mame_file.expect("native MAME payload") {
            MameFilePayload::Rom(value) => {
                check_file(
                    field_family("rom", 0, false),
                    &value.attribute_positions,
                    catalog_files::MameRomAttribute::as_str,
                )?;
                check_file(
                    field_family("rom", 0, true),
                    &value.compatibility_attribute_positions,
                    catalog_files::MameRomCompatibilityAttribute::as_str,
                )?;
            }
            MameFilePayload::Disk(value) => {
                check_file(
                    field_family("disk", 0, false),
                    &value.attribute_positions,
                    catalog_files::MameDiskAttribute::as_str,
                )?;
                check_file(
                    field_family("disk", 0, true),
                    &value.compatibility_attribute_positions,
                    catalog_files::MameDiskCompatibilityAttribute::as_str,
                )?;
            }
            MameFilePayload::Sample(value) => check_file(
                field_family("sample", 0, false),
                &value.attribute_positions,
                catalog_files::MameSampleAttribute::as_str,
            )?,
        }
    }
    Ok(())
}

#[test]
fn all_public_machine_and_file_attribute_families_are_source_free() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database, path, snapshot) = import(&directory, XML.as_bytes())?;
    std::fs::rename(
        directory.path().join("machine.xml"),
        directory.path().join("unavailable.xml"),
    )?;
    std::fs::rename(
        format!("{path}.documents"),
        directory.path().join("unavailable.documents"),
    )?;
    assert!(app::load_snapshot_source(&database, &snapshot).is_err());
    let page = catalog_machines::machines_for_snapshot(
        &database,
        &snapshot,
        None,
        MachinePageLimit::new(10)?,
    )?;
    check_core!("mame", 0, &page.snapshot.header.attribute_positions);
    let machine = page
        .machines
        .iter()
        .find(|machine| machine.name == "complete")
        .expect("complete machine");
    check_core!("machine", 0, &machine.facts.attribute_positions);
    check_core(
        field_family("machine", 0, true),
        &machine.facts.compatibility_attribute_positions,
        catalog_machines::MameMachineCompatibilityAttribute::as_str,
    )?;
    check_core!(
        "biosset",
        0,
        &machine.bios_sets.first().expect("BIOS").attribute_positions
    );
    let reference = machine
        .dependencies
        .iter()
        .find_map(|dependency| match dependency {
            MachineDependency::DeviceReference(reference) => Some(reference),
            _ => None,
        })
        .expect("device reference");
    check_core!("device_ref", 0, &reference.attribute_positions);
    check_switches(machine)?;
    check_hardware(machine)?;
    check_media(&database, machine)?;
    let defaults = page
        .machines
        .iter()
        .find(|machine| machine.name == "defaults")
        .expect("defaults machine");
    assert_eq!(defaults.facts.attribute_positions.len(), 1);
    assert!(defaults.facts.compatibility_attribute_positions.is_empty());
    let eligible = catalog_files::occurrences_for_ids(
        &database,
        &[defaults.assets.first().expect("eligible ROM").occurrence_id],
    )?;
    assert!(
        eligible
            .first()
            .expect("eligible payload")
            .content_id
            .is_some()
    );
    let diff = app::diff_catalog_snapshots(&database, &snapshot, &snapshot)?;
    assert!(!diff.document_metadata_changed);
    assert!(
        diff.records
            .iter()
            .all(|record| !record.metadata_changed && record.requirement_changes.is_empty())
    );
    Ok(())
}

fn republish(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(snapshot.as_str()).execute(connection)
}

#[test]
fn every_actual_attribute_owner_rejects_mutation_and_incomplete_publication() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (_database, path, snapshot) = import(&directory, XML.as_bytes())?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let mut seen = BTreeSet::new();
    for family in FAMILIES {
        if !seen.insert(family.owner) {
            continue;
        }
        let table = format!("{}_attribute_positions", family.owner);
        for sql in [
            format!("UPDATE {table} SET source_line=source_line+1"),
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table} LIMIT 1"),
        ] {
            assert!(
                sql_query(&sql).execute(&mut connection).is_err(),
                "{table}: {sql}"
            );
        }
    }
    assert_eq!(seen.len(), 33);
    connection.batch_execute("DROP TRIGGER snapshot_publications_are_immutable_delete")?;
    sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str())
        .execute(&mut connection)?;
    for owner in seen {
        let table = format!("{owner}_attribute_positions");
        connection.batch_execute(&format!("DROP TRIGGER {table}_attribute_position_delete"))?;
        // A successful seal proves that the failure below is about this missing
        // owner's witnesses, not an already sealed or otherwise invalid edition.
        connection.batch_execute("SAVEPOINT intact")?;
        assert_eq!(republish(&mut connection, &snapshot)?, 1);
        connection.batch_execute("ROLLBACK TO intact; RELEASE intact; SAVEPOINT damaged")?;
        assert!(sql_query(format!("DELETE FROM {table}")).execute(&mut connection)? > 0);
        assert!(
            republish(&mut connection, &snapshot).is_err(),
            "missing witnesses in {table}"
        );
        connection.batch_execute("ROLLBACK TO damaged; RELEASE damaged")?;
    }
    assert_eq!(republish(&mut connection, &snapshot)?, 1);
    Ok(())
}

fn publish_edition(database: &Database, path: &Utf8PathBuf, xml: &str) -> TestResult<SnapshotKey> {
    std::fs::write(path, xml)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path.clone(),
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("attribute-provenance"),
            source_display_name: "MAME attributes".into(),
            catalog_key: CatalogKey::new("attribute-provenance"),
            catalog_display_name: "MAME attributes".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.expect("published edition"))
}

fn reorder_two_fields(family: &Family) -> TestResult<String> {
    let mut reader = Reader::from_str(XML);
    let mut index = 0;
    loop {
        let start = match reader.read_event()? {
            Event::Start(start) | Event::Empty(start)
                if start.name().as_ref() == family.element =>
            {
                start
            }
            Event::Eof => return Err("missing reorder owner".into()),
            _ => continue,
        };
        if index != family.index {
            index += 1;
            continue;
        }
        let mut spans = Vec::new();
        for attribute in start.attributes() {
            let attribute = attribute?;
            if !family.fields.contains(&attribute.key.as_ref()) {
                continue;
            }
            let begin = (attribute.key.as_ref().as_ptr() as usize)
                .checked_sub(XML.as_ptr() as usize)
                .ok_or("unborrowed key")?;
            let end = (attribute.value.as_ref().as_ptr() as usize)
                .checked_sub(XML.as_ptr() as usize)
                .ok_or("unborrowed value")?
                .checked_add(attribute.value.as_ref().len())
                .and_then(|end| end.checked_add(1))
                .ok_or("attribute span overflow")?;
            assert_eq!(
                XML.get(end - 1..end),
                Some("\""),
                "synthetic attributes use double quotes"
            );
            spans.push((begin, end));
            if let [(a, b), (c, d)] = spans.as_slice() {
                return Ok(format!(
                    "{}{}{}{}{}",
                    XML.get(..*a).ok_or("prefix")?,
                    XML.get(*c..*d).ok_or("second field")?,
                    XML.get(*b..*c).ok_or("gap")?,
                    XML.get(*a..*b).ok_or("first field")?,
                    XML.get(*d..).ok_or("suffix")?
                ));
            }
        }
        return Err("reorder requires two declared fields".into());
    }
}

#[test]
fn every_multifield_owner_tracks_recognized_order_as_metadata_only() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (database, _path, original) = import(&directory, XML.as_bytes())?;
    let input = Utf8PathBuf::try_from(directory.path().join("machine.xml"))?;
    let neutral = XML
        .replace("v:marker=\"β\" ", "")
        .replace(
            "<machine sourcefile",
            "<machine v:marker=\"β\" v:padding=\"γ\" sourcefile",
        )
        .replace('\t', "  ")
        .replace('\n', "\r\n");
    let neutral = publish_edition(&database, &input, &neutral)?;
    let unchanged = app::diff_catalog_snapshots(&database, &original, &neutral)?;
    assert!(!unchanged.document_metadata_changed);
    assert!(
        unchanged
            .records
            .iter()
            .all(|record| !record.metadata_changed && record.requirement_changes.is_empty())
    );
    let mut checked = 0;
    for family in FAMILIES.iter().filter(|family| family.fields.len() > 1) {
        let changed = publish_edition(&database, &input, &reorder_two_fields(family)?)?;
        let diff = app::diff_catalog_snapshots(&database, &original, &changed)?;
        assert!(
            diff.records
                .iter()
                .all(|record| record.requirement_changes.is_empty()),
            "{}[{}] order changed matching requirements",
            family.element,
            family.index
        );
        if family.element == "mame" {
            assert!(diff.document_metadata_changed);
            assert!(diff.records.iter().all(|record| !record.metadata_changed));
        } else {
            assert!(!diff.document_metadata_changed);
            assert!(
                diff.records
                    .iter()
                    .find(|record| record.set_name == "complete")
                    .expect("complete history")
                    .metadata_changed,
                "{}[{}] compatibility={}",
                family.element,
                family.index,
                family.compatibility
            );
        }
        checked += 1;
    }
    assert_eq!(
        checked,
        FAMILIES
            .iter()
            .filter(|family| family.fields.len() > 1)
            .count()
    );
    Ok(())
}
