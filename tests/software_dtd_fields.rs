use std::collections::BTreeSet;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey, SnapshotRecordStatus},
};
use quick_xml::{Reader, events::Event};
use sha2::{Digest, Sha256};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const DTD: &str = include_str!("../fixtures/specifications/softwarelist-0.289.dtd");
const XML: &str = include_str!("../fixtures/specifications/software-list-fields.xml");

#[derive(Clone, Copy, Debug)]
enum Owner {
    List,
    Set,
    Item,
    Clone,
    Info,
    SharedFeature,
    Part,
    Feature,
    DataArea,
    Rom,
    DiskArea,
    Disk,
    Switch,
    DipValue,
}

#[derive(Clone, Copy, Debug)]
enum Cell {
    Text(&'static str),
    Integer(i64),
}

struct Field {
    element: &'static str,
    attribute: &'static str,
    owner: Owner,
    column: &'static str,
    expected: Cell,
    changed: &'static str,
}

macro_rules! field {
    ($element:literal, $attribute:literal, $owner:ident, $column:literal, $expected:expr, $changed:literal) => {
        Field {
            element: $element,
            attribute: $attribute,
            owner: Owner::$owner,
            column: $column,
            expected: $expected,
            changed: $changed,
        }
    };
}

// This is deliberately a native-field inventory, not a catch-all value store.
// Its key set must equal the pinned DTD, so adding a declaration cannot silently
// leave a field without a typed SQL witness and a history mutation.
const FIELDS: &[Field] = &[
    field!(
        "softwarelist",
        "name",
        List,
        "name",
        Cell::Text("list α"),
        "other list"
    ),
    field!(
        "softwarelist",
        "description",
        List,
        "description",
        Cell::Text(" List description "),
        "Other list description"
    ),
    field!(
        "software",
        "name",
        Set,
        "set_name",
        Cell::Text("game α"),
        "other game"
    ),
    field!(
        "software",
        "cloneof",
        Clone,
        "target_name",
        Cell::Text("parent"),
        "unknown parent"
    ),
    field!(
        "software",
        "supported",
        Item,
        "supported",
        Cell::Text("partial"),
        "no"
    ),
    field!(
        "info",
        "name",
        Info,
        "name",
        Cell::Text("serial"),
        "other info"
    ),
    field!(
        "info",
        "value",
        Info,
        "value",
        Cell::Text("SER-001"),
        "OTHER-SERIAL"
    ),
    field!(
        "sharedfeat",
        "name",
        SharedFeature,
        "name",
        Cell::Text("compatibility"),
        "other shared feature"
    ),
    field!(
        "sharedfeat",
        "value",
        SharedFeature,
        "value",
        Cell::Text(" PAL "),
        "NTSC"
    ),
    field!(
        "part",
        "name",
        Part,
        "part_name",
        Cell::Text("cart"),
        "other part"
    ),
    field!(
        "part",
        "interface",
        Part,
        "interface",
        Cell::Text("cart-interface"),
        "other-interface"
    ),
    field!(
        "feature",
        "name",
        Feature,
        "name",
        Cell::Text("board"),
        "other feature"
    ),
    field!(
        "feature",
        "value",
        Feature,
        "value",
        Cell::Text(" Mapper "),
        "Other mapper"
    ),
    field!(
        "dataarea",
        "name",
        DataArea,
        "area_name",
        Cell::Text("program"),
        "other data area"
    ),
    field!(
        "dataarea",
        "size",
        DataArea,
        "declared_size_text",
        Cell::Text("0x20"),
        "0x40"
    ),
    field!(
        "dataarea",
        "width",
        DataArea,
        "width",
        Cell::Integer(16),
        "32"
    ),
    field!(
        "dataarea",
        "endianness",
        DataArea,
        "endianness",
        Cell::Text("big"),
        "little"
    ),
    field!(
        "rom",
        "name",
        Rom,
        "name",
        Cell::Text("file α.bin"),
        "other-file.bin"
    ),
    field!(
        "rom",
        "size",
        Rom,
        "size_text",
        Cell::Text("00016"),
        "00020"
    ),
    field!(
        "rom",
        "crc",
        Rom,
        "crc_text",
        Cell::Text("A1B2C3D4"),
        "A1B2C3D5"
    ),
    field!(
        "rom",
        "sha1",
        Rom,
        "sha1_text",
        Cell::Text("0123456789ABCDEF0123456789ABCDEF01234567"),
        "1123456789ABCDEF0123456789ABCDEF01234567"
    ),
    field!(
        "rom",
        "offset",
        Rom,
        "offset_text",
        Cell::Text("0x00"),
        "0x01"
    ),
    field!("rom", "value", Rom, "value", Cell::Text("0xA5"), "0xA6"),
    field!(
        "rom",
        "status",
        Rom,
        "dump_status",
        Cell::Text("baddump"),
        "good"
    ),
    field!(
        "rom",
        "loadflag",
        Rom,
        "load_instruction",
        Cell::Text("load16_byte"),
        "load16_word"
    ),
    field!(
        "diskarea",
        "name",
        DiskArea,
        "area_name",
        Cell::Text("media"),
        "other disk area"
    ),
    field!(
        "disk",
        "name",
        Disk,
        "name",
        Cell::Text("image"),
        "other image"
    ),
    field!(
        "disk",
        "sha1",
        Disk,
        "sha1_text",
        Cell::Text("FEDCBA9876543210FEDCBA9876543210FEDCBA98"),
        "EEDCBA9876543210FEDCBA9876543210FEDCBA98"
    ),
    field!(
        "disk",
        "status",
        Disk,
        "dump_status",
        Cell::Text("nodump"),
        "good"
    ),
    field!(
        "disk",
        "writeable",
        Disk,
        "writeable",
        Cell::Integer(1),
        "no"
    ),
    field!(
        "dipswitch",
        "name",
        Switch,
        "name",
        Cell::Text("Mode"),
        "Other mode"
    ),
    field!(
        "dipswitch",
        "tag",
        Switch,
        "tag",
        Cell::Text(":SW"),
        ":OTHER"
    ),
    field!(
        "dipswitch",
        "mask",
        Switch,
        "mask",
        Cell::Text("0x03"),
        "0x07"
    ),
    field!(
        "dipvalue",
        "name",
        DipValue,
        "name",
        Cell::Text("Default"),
        "Other default"
    ),
    field!(
        "dipvalue",
        "value",
        DipValue,
        "value",
        Cell::Text("0x01"),
        "0x02"
    ),
    field!(
        "dipvalue",
        "default",
        DipValue,
        "is_default",
        Cell::Integer(1),
        "no"
    ),
];

struct Imported {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
    snapshot: SnapshotKey,
    record: i64,
    list: i64,
    part: i64,
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    id: i64,
}

fn import_edition(
    directory: &tempfile::TempDir,
    database: &Database,
    name: &str,
    xml: &str,
) -> TestResult<app::CatalogImportReport> {
    let document_path = Utf8PathBuf::try_from(directory.path().join(format!("{name}.xml")))?;
    std::fs::write(&document_path, xml)?;
    Ok(app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-dtd-fields"),
            source_display_name: "Software DTD fields".into(),
            catalog_key: CatalogKey::new("software-dtd-fields"),
            catalog_display_name: "Software DTD fields".into(),
            scope: CatalogScope::Complete,
        },
    )?)
}

fn imported() -> TestResult<Imported> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let report = import_edition(&directory, &database, "original", XML)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("missing published snapshot")?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let record = sql_query("SELECT set_id AS id FROM catalog_sets WHERE set_name = 'game α'")
        .get_result::<Id>(&mut connection)?
        .id;
    let list = sql_query("SELECT namespace_id AS id FROM software_lists WHERE name = 'list α'")
        .get_result::<Id>(&mut connection)?
        .id;
    let part = sql_query(
        "SELECT part_id AS id FROM software_parts WHERE record_id = ? AND part_order = 0",
    )
    .bind::<BigInt, _>(record)
    .get_result::<Id>(&mut connection)?
    .id;
    Ok(Imported {
        directory,
        database,
        connection,
        snapshot,
        record,
        list,
        part,
    })
}

impl Owner {
    const fn query(self, imported: &Imported) -> (&'static str, &'static str, i64) {
        match self {
            Self::List => ("software_lists", "namespace_id = ?", imported.list),
            Self::Set => ("catalog_sets", "set_id = ?", imported.record),
            Self::Item => ("software_items", "record_id = ?", imported.record),
            Self::Clone => ("software_clone_links", "set_id = ?", imported.record),
            Self::Info => (
                "software_item_info",
                "record_id = ? AND value_order = 0",
                imported.record,
            ),
            Self::SharedFeature => (
                "software_item_shared_features",
                "record_id = ? AND value_order = 0",
                imported.record,
            ),
            Self::Part => ("software_parts", "part_id = ?", imported.part),
            Self::Feature => (
                "software_part_features",
                "part_id = ? AND value_order = 0",
                imported.part,
            ),
            Self::DataArea => (
                "software_data_areas",
                "area_id IN (SELECT area_id FROM software_areas WHERE part_id = ? AND area_order = 0)",
                imported.part,
            ),
            Self::Rom => (
                "software_rom_entries",
                "record_id = ? AND component_order = 0",
                imported.record,
            ),
            Self::DiskArea => (
                "software_disk_areas",
                "area_id IN (SELECT area_id FROM software_areas WHERE part_id = ? AND area_order = 2)",
                imported.part,
            ),
            Self::Disk => ("software_disk_entries", "record_id = ?", imported.record),
            Self::Switch => (
                "software_part_dipswitches",
                "part_id = ? AND dipswitch_order = 0",
                imported.part,
            ),
            Self::DipValue => (
                "software_part_dip_values",
                "part_id = ? AND dipswitch_order = 0 AND value_order = 0",
                imported.part,
            ),
        }
    }
}

#[derive(QueryableByName)]
struct StoredCell {
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

fn assert_cell(imported: &mut Imported, owner: Owner, column: &str, expected: Cell) -> TestResult {
    let (table, filter, id) = owner.query(imported);
    let rows = sql_query(format!("SELECT typeof({column}) AS kind, CAST({column} AS TEXT) AS value FROM {table} WHERE {filter}"))
        .bind::<BigInt, _>(id).load::<StoredCell>(&mut imported.connection)?;
    assert_eq!(
        rows.len(),
        1,
        "{owner:?}.{column} must have its actual singular owner"
    );
    let row = rows.first().ok_or("missing stored cell")?;
    let (kind, value) = match expected {
        Cell::Text(text) => ("text", text.to_owned()),
        Cell::Integer(integer) => ("integer", integer.to_string()),
    };
    assert_eq!(row.kind, kind, "{owner:?}.{column}");
    assert_eq!(
        row.value.as_deref(),
        Some(value.as_str()),
        "{owner:?}.{column}"
    );
    Ok(())
}

fn assert_snapshot_item_cell(
    imported: &mut Imported,
    snapshot: &SnapshotKey,
    column: &str,
    expected: &str,
) -> TestResult {
    let rows = sql_query(format!(
        "SELECT typeof(items.{column}) AS kind, CAST(items.{column} AS TEXT) AS value \
         FROM catalog_set_groups AS groups \
         JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id \
         JOIN catalog_sets AS sets ON sets.set_group_id = groups.set_group_id \
         JOIN software_items AS items ON items.record_id = sets.set_id \
         WHERE groups.snapshot_key = ? AND groups.kind = 'software_list' \
           AND lists.name = 'list α' AND sets.set_name = 'game α'"
    ))
    .bind::<Text, _>(snapshot.as_str())
    .load::<StoredCell>(&mut imported.connection)?;
    assert_eq!(rows.len(), 1, "changed PCDATA must belong to list α/game α");
    let row = rows.first().ok_or("changed item field missing")?;
    assert_eq!(row.kind, "text", "software_items.{column}");
    assert_eq!(
        row.value.as_deref(),
        Some(expected),
        "software_items.{column}"
    );
    Ok(())
}

fn assert_snapshot_list_cell(
    imported: &mut Imported,
    snapshot: &SnapshotKey,
    column: &str,
    expected: &str,
) -> TestResult {
    let rows = sql_query(format!(
        "SELECT typeof(lists.{column}) AS kind, CAST(lists.{column} AS TEXT) AS value \
         FROM catalog_set_groups AS groups \
         JOIN software_lists AS lists ON lists.namespace_id = groups.set_group_id \
         WHERE groups.snapshot_key = ? AND groups.kind = 'software_list' \
           AND lists.name = 'list α'"
    ))
    .bind::<Text, _>(snapshot.as_str())
    .load::<StoredCell>(&mut imported.connection)?;
    assert_eq!(rows.len(), 1, "changed PCDATA must belong to list α");
    let row = rows.first().ok_or("changed list field missing")?;
    assert_eq!(row.kind, "text", "software_lists.{column}");
    assert_eq!(
        row.value.as_deref(),
        Some(expected),
        "software_lists.{column}"
    );
    Ok(())
}

// Fixture editing takes offsets from the actual token rather than searching
// for a value which may occur in another owner or attribute.
fn edit_attribute(
    xml: &str,
    element: &str,
    field: &str,
    replacement: Option<&str>,
) -> TestResult<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        let event = reader.read_event()?;
        let start = match event {
            Event::Start(start) | Event::Empty(start) if start.name().as_ref() == element => start,
            Event::Eof => return Err(format!("missing {element}@{field}").into()),
            _ => continue,
        };
        for attribute in start.attributes() {
            let attribute = attribute?;
            if attribute.key.as_ref() != field {
                continue;
            }
            let base = xml.as_ptr() as usize;
            let value_start = (attribute.value.as_ptr() as usize)
                .checked_sub(base)
                .ok_or("attribute not borrowed from fixture")?;
            let value_end = value_start
                .checked_add(attribute.value.len())
                .ok_or("attribute offset overflow")?;
            let start = if replacement.is_some() {
                value_start
            } else {
                (attribute.key.as_ref().as_ptr() as usize)
                    .checked_sub(base)
                    .ok_or("attribute key not borrowed from fixture")?
            };
            let end = if replacement.is_some() {
                value_end
            } else {
                value_end
                    .checked_add(1)
                    .ok_or("attribute quote offset overflow")?
            };
            let mut edited = String::new();
            edited.push_str(xml.get(..start).ok_or("invalid attribute start")?);
            edited.push_str(replacement.unwrap_or_default());
            edited.push_str(xml.get(end..).ok_or("invalid attribute end")?);
            return Ok(edited);
        }
        return Err(format!("missing {field} on first {element}").into());
    }
}

#[test]
fn pinned_dtd_inventory_has_a_native_typed_witness_for_every_attribute() -> TestResult {
    assert_eq!(
        format!("{:x}", Sha256::digest(DTD.as_bytes())),
        "3b14fa382113bc1c259b2a119346b0c7b4777ebdd52e6610bdc293008af4b549"
    );
    let declarations = DTD
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!ATTLIST "))
        .map(|line| {
            let mut words = line.split_whitespace();
            Ok((
                words.next().ok_or("missing DTD element")?,
                words.next().ok_or("missing DTD attribute")?,
            ))
        })
        .collect::<TestResult<BTreeSet<_>>>()?;
    let witnessed = FIELDS
        .iter()
        .map(|field| (field.element, field.attribute))
        .collect::<BTreeSet<_>>();
    assert_eq!(FIELDS.len(), 36);
    assert_eq!(
        witnessed.len(),
        FIELDS.len(),
        "duplicate witness hides an uncovered field"
    );
    assert_eq!(declarations, witnessed);
    let mut imported = imported()?;
    for field in FIELDS {
        assert_cell(&mut imported, field.owner, field.column, field.expected)?;
    }
    for (owner, column, expected) in [
        (Owner::List, "notes", " List notes é😀 "),
        (Owner::Item, "description", " Title é😀 "),
        (Owner::Item, "year", "19??"),
        (Owner::Item, "publisher", " Publisher & company "),
        (Owner::Item, "notes", " Item notes "),
    ] {
        assert_cell(&mut imported, owner, column, Cell::Text(expected))?;
    }
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Exhaustive field edits independently check native values and exact history owners"
)]
fn every_dtd_attribute_is_visible_in_native_snapshot_history() -> TestResult {
    let mut imported = imported()?;
    for (index, field) in FIELDS.iter().enumerate() {
        let xml = edit_attribute(XML, field.element, field.attribute, Some(field.changed))?;
        let report = import_edition(
            &imported.directory,
            &imported.database,
            &format!("field-{index}"),
            &xml,
        )?;
        assert_eq!(
            report.status,
            CatalogImportStatus::Succeeded,
            "{}@{}",
            field.element,
            field.attribute
        );
        let after = report
            .snapshot_key
            .ok_or("changed fixture did not publish")?;
        let diff = app::diff_catalog_snapshots(&imported.database, &imported.snapshot, &after)?;
        match (field.element, field.attribute) {
            ("softwarelist", _) => {
                assert!(diff.document_metadata_changed, "list@{}", field.attribute);
                if field.attribute == "description" {
                    assert_eq!(diff.records.len(), 2);
                    assert!(
                        diff.records
                            .iter()
                            .all(|record| record.status == SnapshotRecordStatus::Unchanged
                                && !record.metadata_changed)
                    );
                } else {
                    assert_eq!(diff.records.len(), 4);
                    for (list, status) in [
                        ("list α", SnapshotRecordStatus::RemovedWithinScope),
                        (field.changed, SnapshotRecordStatus::AddedWithinScope),
                    ] {
                        for title in ["game α", "parent"] {
                            let key = serde_json::to_string(&(list, title))?;
                            assert_eq!(
                                diff.records
                                    .iter()
                                    .find(|record| record.set_name == key)
                                    .ok_or("renamed list title absent")?
                                    .status,
                                status
                            );
                        }
                    }
                }
            }
            ("software", "name") => {
                assert_eq!(diff.records.len(), 3);
                assert!(
                    diff.records
                        .iter()
                        .any(|record| record.status == SnapshotRecordStatus::AddedWithinScope)
                );
                assert!(
                    diff.records
                        .iter()
                        .any(|record| record.status == SnapshotRecordStatus::RemovedWithinScope)
                );
                for (title, status) in [
                    ("game α", SnapshotRecordStatus::RemovedWithinScope),
                    (field.changed, SnapshotRecordStatus::AddedWithinScope),
                    ("parent", SnapshotRecordStatus::Unchanged),
                ] {
                    let key = serde_json::to_string(&("list α", title))?;
                    assert_eq!(
                        diff.records
                            .iter()
                            .find(|record| record.set_name == key)
                            .ok_or("renamed title absent")?
                            .status,
                        status
                    );
                }
            }
            _ => {
                assert_eq!(diff.records.len(), 2);
                for (title, status, metadata) in [
                    ("game α", SnapshotRecordStatus::Changed, true),
                    ("parent", SnapshotRecordStatus::Unchanged, false),
                ] {
                    let key = serde_json::to_string(&("list α", title))?;
                    let record = diff
                        .records
                        .iter()
                        .find(|record| record.set_name == key)
                        .ok_or("exact title absent from history")?;
                    assert_eq!(
                        record.status, status,
                        "{}@{} / {title}",
                        field.element, field.attribute
                    );
                    assert_eq!(
                        record.metadata_changed, metadata,
                        "{}@{} / {title}",
                        field.element, field.attribute
                    );
                }
            }
        }
        select_snapshot_owners(&mut imported, &after)?;
        let expected = match field.expected {
            Cell::Text(_) => Cell::Text(field.changed),
            Cell::Integer(_) => Cell::Integer(match field.changed {
                "yes" => 1,
                "no" => 0,
                number => number.parse()?,
            }),
        };
        assert_cell(&mut imported, field.owner, field.column, expected)?;
    }
    Ok(())
}

fn select_snapshot_owners(imported: &mut Imported, snapshot: &SnapshotKey) -> TestResult {
    imported.list = sql_query("SELECT set_group_id AS id FROM catalog_set_groups WHERE snapshot_key = ? AND list_order = 0")
        .bind::<Text, _>(snapshot.as_str()).get_result::<Id>(&mut imported.connection)?.id;
    imported.record = sql_query(
        "SELECT set_id AS id FROM catalog_sets WHERE set_group_id = ? AND list_order = 0",
    )
    .bind::<BigInt, _>(imported.list)
    .get_result::<Id>(&mut imported.connection)?
    .id;
    imported.part = sql_query(
        "SELECT part_id AS id FROM software_parts WHERE record_id = ? AND part_order = 0",
    )
    .bind::<BigInt, _>(imported.record)
    .get_result::<Id>(&mut imported.connection)?
    .id;
    Ok(())
}

#[test]
fn every_required_dtd_attribute_fails_without_publishing_partial_facts() -> TestResult {
    let mut imported = imported()?;
    let initial_counts = native_counts(&mut imported.connection)?;
    let mut checked = 0;
    for line in DTD.lines().filter(|line| line.contains("#REQUIRED")) {
        let mut words = line
            .trim()
            .strip_prefix("<!ATTLIST ")
            .ok_or("unsupported DTD declaration")?
            .split_whitespace();
        let element = words.next().ok_or("missing DTD element")?;
        let attribute = words.next().ok_or("missing DTD attribute")?;
        let xml = edit_attribute(XML, element, attribute, None)?;
        let report = import_edition(
            &imported.directory,
            &imported.database,
            &format!("required-{checked}"),
            &xml,
        )?;
        assert_eq!(
            report.status,
            CatalogImportStatus::Failed,
            "missing {element}@{attribute} published"
        );
        assert!(report.snapshot_key.is_none());
        assert!(report.diagnostic_count > 0);
        checked += 1;
        assert_eq!(
            native_counts(&mut imported.connection)?,
            initial_counts,
            "{element}@{attribute} leaked native facts or altered published facts"
        );
    }
    assert_eq!(checked, 16);
    Ok(())
}

fn native_counts(connection: &mut SqliteConnection) -> TestResult<Vec<i64>> {
    [
        "catalog_set_groups",
        "catalog_sets",
        "software_documents",
        "software_wrapper_headers",
        "software_lists",
        "software_items",
        "software_list_text_positions",
        "software_item_text_positions",
        "software_item_info",
        "software_item_shared_features",
        "software_parts",
        "software_part_features",
        "software_part_dipswitches",
        "software_part_dip_values",
        "software_areas",
        "software_data_areas",
        "software_disk_areas",
        "software_rom_entries",
        "software_disk_entries",
        "software_file_declarations",
        "software_file_uses",
        "asset_occurrences",
        "catalog_contents",
        "digest_values",
        "occurrence_digest_assertions",
        "snapshot_publications",
    ]
    .into_iter()
    .map(|table| {
        Ok(sql_query(format!("SELECT count(*) AS id FROM {table}"))
            .get_result::<Id>(connection)?
            .id)
    })
    .collect()
}

#[test]
fn every_enumerated_dtd_attribute_rejects_unknown_values_without_leaking_facts() -> TestResult {
    let mut imported = imported()?;
    let initial_counts = native_counts(&mut imported.connection)?;
    let mut checked = 0;
    for declaration in DTD
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!ATTLIST "))
    {
        let mut words = declaration.split_whitespace();
        let element = words.next().ok_or("missing DTD element")?;
        let attribute = words.next().ok_or("missing DTD attribute")?;
        let domain = words.next().ok_or("missing DTD domain")?;
        if !domain.starts_with('(') {
            continue;
        }
        let xml = edit_attribute(XML, element, attribute, Some("invalid-enumeration"))?;
        let report = import_edition(
            &imported.directory,
            &imported.database,
            &format!("enum-{checked}"),
            &xml,
        )?;
        assert_eq!(
            report.status,
            CatalogImportStatus::Failed,
            "{element}@{attribute}"
        );
        assert!(report.snapshot_key.is_none());
        assert!(report.diagnostic_count > 0);
        assert_eq!(
            native_counts(&mut imported.connection)?,
            initial_counts,
            "{element}@{attribute}"
        );
        checked += 1;
    }
    assert_eq!(checked, 8);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "Keep all five DTD text placements and their independent owner assertions together"
)]
fn all_pcdata_fields_are_visible_in_their_actual_document_or_title_history() -> TestResult {
    let mut imported = imported()?;
    let declared_pcdata = DTD
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!ELEMENT "))
        .filter(|line| line.contains("(#PCDATA)"))
        .map(|line| line.split_whitespace().next().ok_or("missing PCDATA name"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    assert_eq!(
        declared_pcdata,
        BTreeSet::from(["notes", "description", "year", "publisher"])
    );
    for (index, (old, new, item_field, column, expected)) in [
        (
            "<notes> List notes é😀 </notes>",
            "<notes> Other list notes </notes>",
            false,
            "notes",
            " Other list notes ",
        ),
        (
            "<description> Title é😀 </description>",
            "<description> Other title </description>",
            true,
            "description",
            " Other title ",
        ),
        (
            "<year>19??</year>",
            "<year>20??</year>",
            true,
            "year",
            "20??",
        ),
        (
            "<publisher> Publisher &amp; company </publisher>",
            "<publisher> Other publisher </publisher>",
            true,
            "publisher",
            " Other publisher ",
        ),
        (
            "<notes> Item notes </notes>",
            "<notes> Other item notes </notes>",
            true,
            "notes",
            " Other item notes ",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(XML.matches(old).count(), 1, "ambiguous PCDATA fixture edit");
        let xml = XML.replacen(old, new, 1);
        let report = import_edition(
            &imported.directory,
            &imported.database,
            &format!("pcdata-{index}"),
            &xml,
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        let after = report
            .snapshot_key
            .ok_or("PCDATA fixture did not publish")?;
        let diff = app::diff_catalog_snapshots(&imported.database, &imported.snapshot, &after)?;
        // The document history value is the complete ordered software-list tree,
        // so an item edit changes both that tree and its title record.
        assert!(diff.document_metadata_changed, "PCDATA field {column}");
        assert_eq!(diff.records.len(), 2, "PCDATA field {column}");
        let changed = diff
            .records
            .iter()
            .find(|record| record.set_name == r#"["list α","game α"]"#)
            .ok_or("changed title is absent from history")?;
        let parent = diff
            .records
            .iter()
            .find(|record| record.set_name == r#"["list α","parent"]"#)
            .ok_or("parent title is absent from history")?;
        if item_field {
            assert_eq!(changed.status, SnapshotRecordStatus::Changed, "{column}");
            assert!(changed.metadata_changed, "{column}");
            assert_eq!(parent.status, SnapshotRecordStatus::Unchanged, "{column}");
            assert!(
                !parent.metadata_changed,
                "parent metadata changed for {column}"
            );
            assert_snapshot_item_cell(&mut imported, &after, column, expected)?;
        } else {
            assert_eq!(
                changed.status,
                SnapshotRecordStatus::Unchanged,
                "list notes"
            );
            assert!(
                !changed.metadata_changed,
                "list notes changed title metadata"
            );
            assert_eq!(parent.status, SnapshotRecordStatus::Unchanged, "list notes");
            assert!(
                !parent.metadata_changed,
                "list notes changed parent metadata"
            );
            assert_snapshot_list_cell(&mut imported, &after, column, expected)?;
        }
    }
    Ok(())
}

#[test]
fn required_item_pcdata_rejects_missing_and_duplicate_fields_without_leaking_facts() -> TestResult {
    let mut imported = imported()?;
    let initial_counts = native_counts(&mut imported.connection)?;
    for (index, element) in [
        "<description> Title é😀 </description>",
        "<year>19??</year>",
        "<publisher> Publisher &amp; company </publisher>",
    ]
    .into_iter()
    .enumerate()
    {
        for (case, replacement) in [
            ("missing", String::new()),
            ("duplicate", format!("{element}{element}")),
        ] {
            assert_eq!(XML.matches(element).count(), 1);
            let xml = XML.replacen(element, &replacement, 1);
            let report = import_edition(
                &imported.directory,
                &imported.database,
                &format!("{case}-pcdata-{index}"),
                &xml,
            )?;
            assert_eq!(
                report.status,
                CatalogImportStatus::Failed,
                "{case} {element}"
            );
            assert!(report.snapshot_key.is_none());
            assert!(report.diagnostic_count > 0);
            assert_eq!(
                native_counts(&mut imported.connection)?,
                initial_counts,
                "{case} {element}"
            );
        }
    }
    Ok(())
}
