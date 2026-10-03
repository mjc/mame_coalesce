use std::collections::BTreeSet;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{self, ContentOccurrenceLimit, SoftwareFilePayload, XmlAttributePosition},
    catalog_software::{self, SoftwareAreaAttributePositions, SoftwarePageLimit},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};
use quick_xml::{Reader, events::Event};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const XML: &str = include_str!("../fixtures/specifications/software-list-fields.xml");
const DTD: &str = include_str!("../fixtures/specifications/softwarelist-0.289.dtd");

struct Family {
    element: &'static str,
    table: &'static str,
    fields: &'static [&'static str],
    owners: &'static str,
    keys: &'static str,
    selected: &'static str,
}

// This dictionary is independent of the production field enums and selectors.
// Every native join uses the complete owner key, including repeated child order.
const FAMILIES: &[Family] = &[
    Family {
        element: "softwarelist",
        table: "software_list_attribute_positions",
        fields: &["name", "description"],
        owners: "software_lists AS native JOIN catalog_set_groups AS groups ON groups.set_group_id=native.namespace_id",
        keys: "namespace_id",
        selected: "1",
    },
    Family {
        element: "software",
        table: "software_item_attribute_positions",
        fields: &["name", "cloneof", "supported"],
        owners: "software_items AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "record_id",
        selected: "sets.set_name='game α'",
    },
    Family {
        element: "info",
        table: "software_item_info_attribute_positions",
        fields: &["name", "value"],
        owners: "software_item_info AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "record_id,value_order",
        selected: "native.value_order=0",
    },
    Family {
        element: "sharedfeat",
        table: "software_item_shared_feature_attribute_positions",
        fields: &["name", "value"],
        owners: "software_item_shared_features AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "record_id,value_order",
        selected: "native.value_order=0",
    },
    Family {
        element: "part",
        table: "software_part_attribute_positions",
        fields: &["name", "interface"],
        owners: "software_parts AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "part_id",
        selected: "native.part_order=0",
    },
    Family {
        element: "feature",
        table: "software_part_feature_attribute_positions",
        fields: &["name", "value"],
        owners: "software_part_features AS native JOIN software_parts AS part USING(part_id) JOIN catalog_sets AS sets ON sets.set_id=part.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "part_id,value_order",
        selected: "part.part_order=0 AND native.value_order=0",
    },
    Family {
        element: "dataarea",
        table: "software_data_area_attribute_positions",
        fields: &["name", "size", "width", "endianness"],
        owners: "software_data_areas AS native JOIN software_areas AS area USING(area_id) JOIN catalog_sets AS sets ON sets.set_id=area.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "area_id",
        selected: "area.area_order=0",
    },
    Family {
        element: "diskarea",
        table: "software_disk_area_attribute_positions",
        fields: &["name"],
        owners: "software_disk_areas AS native JOIN software_areas AS area USING(area_id) JOIN catalog_sets AS sets ON sets.set_id=area.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "area_id",
        selected: "area.area_order=2",
    },
    Family {
        element: "rom",
        table: "software_rom_attribute_positions",
        fields: &[
            "name", "size", "crc", "sha1", "offset", "value", "status", "loadflag",
        ],
        owners: "software_rom_entries AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "occurrence_id",
        selected: "native.component_order=0",
    },
    Family {
        element: "disk",
        table: "software_disk_attribute_positions",
        fields: &["name", "sha1", "status", "writeable"],
        owners: "software_disk_entries AS native JOIN catalog_sets AS sets ON sets.set_id=native.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "occurrence_id",
        selected: "native.component_order=0",
    },
    Family {
        element: "dipswitch",
        table: "software_part_dipswitch_attribute_positions",
        fields: &["name", "tag", "mask"],
        owners: "software_part_dipswitches AS native JOIN software_parts AS part USING(part_id) JOIN catalog_sets AS sets ON sets.set_id=part.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "part_id,dipswitch_order",
        selected: "part.part_order=0 AND native.dipswitch_order=0",
    },
    Family {
        element: "dipvalue",
        table: "software_part_dip_value_attribute_positions",
        fields: &["name", "value", "default"],
        owners: "software_part_dip_values AS native JOIN software_parts AS part USING(part_id) JOIN catalog_sets AS sets ON sets.set_id=part.record_id JOIN catalog_set_groups AS groups USING(set_group_id)",
        keys: "part_id,dipswitch_order,value_order",
        selected: "part.part_order=0 AND native.dipswitch_order=0 AND native.value_order=0",
    },
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
        .ok_or("invalid independent token offset")?;
    let normalized = prefix.replace("\r\n", "\n").replace('\r', "\n");
    Ok((
        i64::try_from(normalized.chars().filter(|&c| c == '\n').count())? + 1,
        i64::try_from(
            normalized
                .rsplit('\n')
                .next()
                .ok_or("last line")?
                .chars()
                .count(),
        )? + 1,
    ))
}

fn declared_positions(xml: &str, element: &str, fields: &[&str]) -> TestResult<Vec<Position>> {
    let mut reader = Reader::from_str(xml);
    loop {
        let start = match reader.read_event()? {
            Event::Start(start) | Event::Empty(start) if start.name().as_ref() == element => start,
            Event::Eof => return Err(format!("missing fixture element {element}").into()),
            _ => continue,
        };
        let mut expected = Vec::new();
        for (source_order, attribute) in start.attributes().enumerate() {
            let attribute = attribute?;
            let Some(field_kind) = fields
                .iter()
                .position(|field| attribute.key.as_ref() == *field)
            else {
                continue;
            };
            let offset = (attribute.key.as_ref().as_ptr() as usize)
                .checked_sub(xml.as_ptr() as usize)
                .ok_or("fixture token not borrowed")?;
            let (source_line, source_column) = location(xml, offset)?;
            expected.push(Position {
                field_kind: i64::try_from(field_kind)?,
                source_order: i64::try_from(source_order)?,
                source_line,
                source_column,
            });
        }
        return Ok(expected);
    }
}

fn expected_positions(xml: &str, element: &str, fields: &[&str]) -> TestResult<Vec<Position>> {
    let expected = declared_positions(xml, element, fields)?;
    assert_eq!(
        expected.len(),
        fields.len(),
        "first {element} must supply every independently inventoried field"
    );
    Ok(expected)
}

fn fixture() -> TestResult<String> {
    let root = XML
        .get(XML.find("<softwarelist ").ok_or("fixture root")?..)
        .ok_or("fixture root range")?;
    let mut xml = format!(
        "<!-- Unicode é😀 -->\n<softwarelists xmlns:v='urn:vendor' v:extra='x'\n\tbuild=''>{root}</softwarelists>"
    );
    for family in FAMILIES {
        xml = xml.replacen(
            &format!("<{} ", family.element),
            &format!("<{} xmlns:v='urn:vendor' v:extra='x'\n\t", family.element),
            1,
        );
    }
    Ok(xml.replace('\n', "\r\n"))
}

fn import(
    directory: &tempfile::TempDir,
    xml: &[u8],
) -> TestResult<(Database, Utf8PathBuf, SnapshotKey)> {
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("software.xml"))?;
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-attribute-proof"),
            source_display_name: "Software attributes".into(),
            catalog_key: CatalogKey::new("software-attribute-proof"),
            catalog_display_name: "Software attributes".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("missing snapshot")?;
    assert_eq!(app::load_snapshot_source(&database, &snapshot)?, xml);
    Ok((database, path, snapshot))
}

fn check_native_positions(path: &Utf8PathBuf, snapshot: &SnapshotKey, xml: &str) -> TestResult {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for family in FAMILIES {
        let query = format!(
            "SELECT positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM {} JOIN {} AS positions USING({}) WHERE groups.snapshot_key=? AND {} ORDER BY positions.source_order",
            family.owners, family.table, family.keys, family.selected
        );
        let rows = sql_query(query)
            .bind::<Text, _>(snapshot.as_str())
            .load::<Position>(&mut connection)?;
        assert_eq!(
            rows,
            expected_positions(xml, family.element, family.fields)?,
            "actual native owner for {}",
            family.element
        );
    }
    let wrapper = sql_query("SELECT positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM software_wrapper_headers JOIN software_wrapper_attribute_positions AS positions USING(wrapper_id) WHERE snapshot_key=? ORDER BY positions.source_order")
        .bind::<Text, _>(snapshot.as_str()).load::<Position>(&mut connection)?;
    assert_eq!(
        wrapper,
        expected_positions(xml, "softwarelists", &["build"])?
    );
    Ok(())
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "the independent pinned DTD fixture must have an element and field in every ATTLIST"
)]
fn independent_dictionary_matches_all_pinned_software_attributes() {
    let pinned = DTD
        .lines()
        .filter_map(|line| line.trim().strip_prefix("<!ATTLIST "))
        .map(|line| {
            let mut words = line.split_whitespace();
            (
                words.next().expect("DTD element"),
                words.next().expect("DTD field"),
            )
        })
        .collect::<BTreeSet<_>>();
    let witnessed = FAMILIES
        .iter()
        .flat_map(|family| {
            family
                .fields
                .iter()
                .map(move |field| (family.element, *field))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(pinned.len(), 36);
    assert_eq!(witnessed, pinned);
}

#[test]
fn all_36_pinned_fields_and_wrapper_build_have_native_qname_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = fixture()?;
    let (_database, path, snapshot) = import(&directory, xml.as_bytes())?;
    check_native_positions(&path, &snapshot, &xml)
}

#[test]
fn utf16_and_gzip_preserve_decoded_attribute_coordinates_and_exact_originals() -> TestResult {
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;

    let xml = format!("<?xml version='1.0' encoding='UTF-16'?>\r\n{}", fixture()?);
    let mut little_endian = vec![0xff, 0xfe];
    let mut big_endian = vec![0xfe, 0xff];
    for scalar in xml.encode_utf16() {
        little_endian.extend_from_slice(&scalar.to_le_bytes());
        big_endian.extend_from_slice(&scalar.to_be_bytes());
    }
    let mut compressor = GzEncoder::new(Vec::new(), Compression::fast());
    compressor.write_all(&little_endian)?;
    for bytes in [little_endian, big_endian, compressor.finish()?] {
        let directory = tempfile::tempdir()?;
        let (_database, path, snapshot) = import(&directory, &bytes)?;
        check_native_positions(&path, &snapshot, &xml)?;
    }
    Ok(())
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[test]
fn all_native_position_tables_are_numeric_position_only_and_rowid_free() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&path)?);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for (table, keys) in FAMILIES
        .iter()
        .map(|family| (family.table, family.keys))
        .chain([("software_wrapper_attribute_positions", "wrapper_id")])
    {
        let schema =
            sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='table' AND name=?")
                .bind::<Text, _>(table)
                .get_result::<TextRow>(&mut connection)?
                .value;
        assert!(schema.contains("WITHOUT ROWID"), "{table}");
        let columns = sql_query(format!(
            "SELECT name AS value FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .load::<TextRow>(&mut connection)?;
        let expected = keys
            .split(',')
            .chain(["field_kind", "source_order", "source_line", "source_column"])
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            columns
                .iter()
                .map(|row| row.value.clone())
                .collect::<BTreeSet<_>>(),
            expected,
            "no copied literals or extra owner projections in {table}"
        );
        let numeric = sql_query(format!(
            "SELECT name AS value FROM pragma_table_info('{table}') WHERE type<>'INTEGER'"
        ))
        .load::<TextRow>(&mut connection)?;
        assert!(
            numeric.is_empty(),
            "all position keys and fields are numeric: {table}"
        );
    }
    Ok(())
}

#[test]
fn empty_and_malformed_hash_attributes_still_have_lexical_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = fixture()?
        .replace("A1B2C3D4", "")
        .replace("0123456789ABCDEF0123456789ABCDEF01234567", "not-a-hash")
        .replace("FEDCBA9876543210FEDCBA9876543210FEDCBA98", "");
    let (_database, path, snapshot) = import(&directory, xml.as_bytes())?;
    check_native_positions(&path, &snapshot, &xml)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let evidence = sql_query("SELECT algorithm AS value FROM occurrence_digest_assertions JOIN digest_values USING(digest_id) JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON catalog_sets.set_id=asset_occurrences.record_id JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot.as_str()).load::<TextRow>(&mut connection)?;
    assert!(
        evidence.is_empty(),
        "bad source spellings do not fabricate binary evidence"
    );
    Ok(())
}

fn public_positions<Field>(
    positions: &[XmlAttributePosition<Field>],
    code: impl Fn(&Field) -> i64,
) -> Vec<Position> {
    positions
        .iter()
        .map(|position| Position {
            field_kind: code(&position.field),
            source_order: position.source_order,
            source_line: position.location.line,
            source_column: position.location.column,
        })
        .collect()
}

macro_rules! check {
    ($xml:expr, $positions:expr, $element:literal, $fields:expr) => {
        assert_eq!(
            public_positions($positions, |field| field.code()),
            expected_positions($xml, $element, $fields)?,
            $element
        );
    };
}

fn assert_part_positions(xml: &str, part: &catalog_software::SoftwarePart) -> TestResult {
    check!(
        xml,
        &part.attribute_positions,
        "part",
        &["name", "interface"]
    );
    check!(
        xml,
        &part.features.first().ok_or("feature")?.attribute_positions,
        "feature",
        &["name", "value"]
    );
    let switch = part.switches.first().ok_or("switch")?;
    check!(
        xml,
        &switch.attribute_positions,
        "dipswitch",
        &["name", "tag", "mask"]
    );
    check!(
        xml,
        &switch.values.first().ok_or("dipvalue")?.attribute_positions,
        "dipvalue",
        &["name", "value", "default"]
    );
    let data = part.areas.first().ok_or("data")?;
    let disk = part.areas.get(2).ok_or("disk")?;
    let SoftwareAreaAttributePositions::Data(data_positions) = &data.attribute_positions else {
        return Err("data positions subtype".into());
    };
    check!(
        xml,
        data_positions,
        "dataarea",
        &["name", "size", "width", "endianness"]
    );
    let SoftwareAreaAttributePositions::Disk(disk_positions) = &disk.attribute_positions else {
        return Err("disk positions subtype".into());
    };
    check!(xml, disk_positions, "diskarea", &["name"]);
    Ok(())
}

#[test]
fn public_metadata_and_uuid_media_queries_need_no_source_documents() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = fixture()?;
    let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
    std::fs::rename(
        directory.path().join("software.xml"),
        directory.path().join("retained-input.xml"),
    )?;
    std::fs::rename(
        format!("{path}.documents"),
        directory.path().join("retained-documents"),
    )?;
    assert!(
        app::load_snapshot_source(&database, &snapshot).is_err(),
        "source-free queries must not reparse retained XML"
    );
    let lists = catalog_software::lists_for_snapshot(
        &database,
        &snapshot,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let list = lists.lists.first().ok_or("list")?;
    check!(
        &xml,
        &lists.snapshot.wrapper_attribute_positions,
        "softwarelists",
        &["build"]
    );
    check!(
        &xml,
        &list.attribute_positions,
        "softwarelist",
        &["name", "description"]
    );
    let titles = catalog_software::titles_for_list(
        &database,
        &snapshot,
        list.id,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let title = titles.titles.first().ok_or("title")?;
    check!(
        &xml,
        &title.attribute_positions,
        "software",
        &["name", "cloneof", "supported"]
    );
    check!(
        &xml,
        &title.info.first().ok_or("info")?.attribute_positions,
        "info",
        &["name", "value"]
    );
    check!(
        &xml,
        &title
            .shared_features
            .first()
            .ok_or("sharedfeat")?
            .attribute_positions,
        "sharedfeat",
        &["name", "value"]
    );
    let part = title.parts.first().ok_or("part")?;
    assert_part_positions(&xml, part)?;
    assert_media_positions_and_uuid(&database, &xml, part)
}

fn assert_media_positions_and_uuid(
    database: &Database,
    xml: &str,
    part: &catalog_software::SoftwarePart,
) -> TestResult {
    let data = part.areas.first().ok_or("data")?;
    let disk = part.areas.get(2).ok_or("disk")?;
    let ids = data
        .entry_ids
        .iter()
        .chain(&disk.entry_ids)
        .copied()
        .collect::<Vec<_>>();
    let files = catalog_files::occurrences_for_ids(database, &ids)?;
    assert_eq!(files.len(), 3);
    let declaring_id = *data.entry_ids.first().ok_or("declaring ID")?;
    let declaring = files
        .iter()
        .find(|file| file.occurrence_id == declaring_id)
        .ok_or("declaring payload")?;
    let Some(SoftwareFilePayload::Rom(rom)) = &declaring.software_file else {
        return Err("ROM payload".into());
    };
    check!(
        xml,
        &rom.attribute_positions,
        "rom",
        &[
            "name", "size", "crc", "sha1", "offset", "value", "status", "loadflag"
        ]
    );
    let image = files
        .iter()
        .find_map(|file| match &file.software_file {
            Some(SoftwareFilePayload::Disk(disk)) => Some(disk),
            _ => None,
        })
        .ok_or("disk payload")?;
    check!(
        xml,
        &image.attribute_positions,
        "disk",
        &["name", "sha1", "status", "writeable"]
    );
    let identity = declaring
        .content_id
        .ok_or("eligible ROM retains its shared UUID")?;
    let by_uuid = catalog_files::occurrences_for_content(
        database,
        identity,
        ContentOccurrenceLimit::new(10)?,
        None,
    )?;
    assert_eq!(by_uuid.occurrences, vec![declaring.clone()]);
    assert!(
        files
            .iter()
            .filter(|file| file.occurrence_id != declaring_id)
            .all(|file| file.content_id.is_none()),
        "operation and disk cannot borrow a whole-file UUID"
    );
    Ok(())
}

#[test]
fn every_native_attribute_owner_rejects_post_publication_mutation_and_replacement() -> TestResult {
    use diesel::connection::SimpleConnection;
    let directory = tempfile::tempdir()?;
    let xml = fixture()?;
    let (_database, path, snapshot) = import(&directory, xml.as_bytes())?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    for (table, keys) in FAMILIES
        .iter()
        .map(|family| (family.table, family.keys))
        .chain([("software_wrapper_attribute_positions", "wrapper_id")])
    {
        let key_columns = keys.split(',').collect::<Vec<_>>();
        let key_expression = key_columns.join("||','||");
        let owner = sql_query(format!(
            "SELECT {key_expression} AS value FROM {table} ORDER BY {keys} LIMIT 1"
        ))
        .get_result::<TextRow>(&mut connection)?
        .value;
        let numbers = owner
            .split(',')
            .map(str::parse::<i64>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(numbers.len(), key_columns.len());
        let predicate = key_columns
            .iter()
            .zip(numbers)
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        for statement in [
            format!("UPDATE {table} SET source_column=source_column+1 WHERE {predicate}"),
            format!("DELETE FROM {table} WHERE {predicate}"),
            format!(
                "INSERT OR REPLACE INTO {table}({keys},field_kind,source_order,source_line,source_column) SELECT {keys},field_kind,source_order,source_line,source_column+1 FROM {table} WHERE {predicate}"
            ),
        ] {
            assert!(
                sql_query(&statement).execute(&mut connection).is_err(),
                "published position was mutable with enforcement pragmas off: {statement}"
            );
        }
    }
    check_native_positions(&path, &snapshot, &xml)
}

#[test]
fn clone_explanations_point_to_the_attribute_qname_not_the_software_tag() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = fixture()?;
    let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
    std::fs::rename(
        format!("{path}.documents"),
        directory.path().join("retained-documents"),
    )?;
    assert!(app::load_snapshot_source(&database, &snapshot).is_err());
    let expected = expected_positions(&xml, "software", &["name", "cloneof", "supported"])?;
    let clone = expected
        .iter()
        .find(|position| position.field_kind == 1)
        .ok_or("clone position")?;
    let explanations = app::explain_relationships(&database)?;
    assert_eq!(explanations.len(), 1);
    let explanation = explanations.first().ok_or("clone explanation")?;
    assert_eq!(explanation.source_field.as_deref(), Some("cloneof"));
    let actual = explanation
        .source_location
        .as_ref()
        .ok_or("clone QName location")?;
    assert_eq!(actual.line, clone.source_line);
    assert_eq!(actual.column, clone.source_column);
    Ok(())
}

#[test]
fn direct_publication_requires_all_native_positions_even_with_foreign_keys_off() -> TestResult {
    use diesel::connection::SimpleConnection;
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    drop(Database::open(&path)?);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;
        INSERT INTO publishing_sources(source_key,display_name) VALUES('position-draft','Positions');
        INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES('position-draft','position-draft','Positions');
        INSERT INTO documents(document_key,format_hint) VALUES('position-draft','mame-softwarelist-xml');
        INSERT INTO parser_interpretations(interpretation_key,format,parser_name,parser_version,rules_version) VALUES('position-draft','mame-softwarelist-xml','fixture','1','mame-softwarelist-declared-text-compat-v2');
        INSERT INTO catalog_coverage(coverage_id,kind) VALUES(9876,'complete');
        INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) VALUES('position-draft','position-draft','position-draft','position-draft',9876);
        INSERT INTO software_documents(snapshot_key,envelope_kind) VALUES('position-draft','single_list');
        INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order) VALUES(9876,'position-draft','software_list',0);
        INSERT INTO software_lists(namespace_id,source_order,name,source_line,source_column) VALUES(9876,0,'list',1,1);
        INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES(9876,9876,'software_item',0,'game',1,1);
        INSERT INTO software_items(record_id,source_order,supported,supported_specified,description,year,publisher) VALUES(9876,0,'yes',0,'Game','2000','Publisher');
        INSERT INTO software_item_text_positions(record_id,field_kind,source_order,source_line,source_column) VALUES(9876,0,0,1,1),(9876,1,1,1,1),(9876,2,2,1,1);")?;
    let publish = "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) VALUES('position-draft','position-draft','position-draft','position-draft')";
    assert!(
        sql_query(publish).execute(&mut connection).is_err(),
        "source owners alone cannot replace required list and title attribute witnesses"
    );
    for (owner, field, order, line, column) in [
        ("999999", "0", "0", "1", "1"),
        ("9876", "1", "0", "1", "1"),
        ("9876", "99", "0", "1", "1"),
        ("9876", "0.5", "0", "1", "1"),
        ("9876", "0", "-1", "1", "1"),
        ("9876", "0", "0.5", "1", "1"),
        ("9876", "0", "0", "0", "1"),
        ("9876", "0", "0", "1.5", "1"),
        ("9876", "0", "0", "1", "0"),
        ("9876", "0", "0", "1", "1.5"),
    ] {
        let insert = format!(
            "INSERT INTO software_list_attribute_positions(namespace_id,field_kind,source_order,source_line,source_column) VALUES({owner},{field},{order},{line},{column})"
        );
        assert!(
            sql_query(&insert).execute(&mut connection).is_err(),
            "invalid draft witness accepted with both pragmas off: {insert}"
        );
    }
    connection.batch_execute("INSERT INTO software_list_attribute_positions(namespace_id,field_kind,source_order,source_line,source_column) VALUES(9876,0,0,1,15);")?;
    assert!(
        sql_query(publish).execute(&mut connection).is_err(),
        "title name witness is still missing"
    );
    connection.batch_execute("INSERT INTO software_item_attribute_positions(record_id,field_kind,source_order,source_line,source_column) VALUES(9876,0,0,1,35);")?;
    sql_query(publish).execute(&mut connection)?;
    assert!(sql_query("INSERT INTO software_item_attribute_positions(record_id,field_kind,source_order,source_line,source_column) VALUES(9876,2,1,1,40)").execute(&mut connection).is_err(), "implicit supported default must not fabricate an attribute position");
    Ok(())
}

#[test]
fn omitted_defaults_and_optional_attributes_have_no_synthetic_positions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = "<softwarelists><softwarelist name='list'><software name='game α'>
        <description>Game</description><year>2000</year><publisher>Publisher</publisher>
        <info name='repeat'/><info name='repeat' value=''/>
        <sharedfeat name='repeat'/><sharedfeat name='repeat' value=''/>
        <part name='cart' interface='cart'><feature name='repeat'/><feature name='repeat' value=''/>
        <dataarea name='rom' size='8'><rom/></dataarea>
        <dataarea name='rom' size='' width='8' endianness='little'/>
        <diskarea name='disk'><disk name='disk'/></diskarea>
        <dipswitch name='Mode' tag='M' mask='1'><dipvalue name='Same' value='0'/><dipvalue name='Same' value='' default='no'/></dipswitch>
        </part></software></softwarelist></softwarelists>";
    let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for family in FAMILIES {
        let query = format!(
            "SELECT positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM {} JOIN {} AS positions USING({}) WHERE groups.snapshot_key=? AND {} ORDER BY positions.source_order",
            family.owners, family.table, family.keys, family.selected
        );
        let rows = sql_query(query)
            .bind::<Text, _>(snapshot.as_str())
            .load::<Position>(&mut connection)?;
        assert_eq!(
            rows,
            declared_positions(xml, family.element, family.fields)?,
            "no synthetic defaults for {}",
            family.element
        );
    }
    let lists = catalog_software::lists_for_snapshot(
        &database,
        &snapshot,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    assert!(lists.snapshot.wrapper_attribute_positions.is_empty());
    let list = lists.lists.first().ok_or("list")?;
    let titles = catalog_software::titles_for_list(
        &database,
        &snapshot,
        list.id,
        SoftwarePageLimit::new(10)?,
        None,
    )?;
    let title = titles.titles.first().ok_or("title")?;
    assert_omitted_metadata_positions(title)?;
    let files = catalog_files::occurrences_for_ids(
        &database,
        &title
            .parts
            .first()
            .ok_or("part")?
            .areas
            .iter()
            .flat_map(|area| area.entry_ids.iter().copied())
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(files.len(), 2);
    for file in files {
        assert!(file.content_id.is_none());
        match file.software_file {
            Some(SoftwareFilePayload::Rom(rom)) => assert!(rom.attribute_positions.is_empty()),
            Some(SoftwareFilePayload::Disk(disk)) => assert_eq!(disk.attribute_positions.len(), 1),
            _ => return Err("native software payload".into()),
        }
    }
    Ok(())
}

fn assert_omitted_metadata_positions(title: &catalog_software::SoftwareTitle) -> TestResult {
    for family in [
        &title.info,
        &title.shared_features,
        &title.parts.first().ok_or("part")?.features,
    ] {
        assert_eq!(
            family.len(),
            2,
            "repeated native named values remain separate"
        );
        assert_eq!(
            family
                .first()
                .ok_or("omitted value")?
                .attribute_positions
                .len(),
            1
        );
        assert_eq!(
            family
                .get(1)
                .ok_or("empty value")?
                .attribute_positions
                .len(),
            2
        );
    }
    let part = title.parts.first().ok_or("part")?;
    let SoftwareAreaAttributePositions::Data(defaulted) =
        &part.areas.first().ok_or("data")?.attribute_positions
    else {
        return Err("data subtype".into());
    };
    let SoftwareAreaAttributePositions::Data(explicit) = &part
        .areas
        .get(1)
        .ok_or("explicit data")?
        .attribute_positions
    else {
        return Err("explicit data subtype".into());
    };
    assert_eq!(defaulted.len(), 2);
    assert_eq!(explicit.len(), 4);
    let switch = part.switches.first().ok_or("switch")?;
    assert_eq!(
        switch
            .values
            .first()
            .ok_or("default omitted")?
            .attribute_positions
            .len(),
        2
    );
    assert_eq!(
        switch
            .values
            .get(1)
            .ok_or("default explicit")?
            .attribute_positions
            .len(),
        3
    );
    Ok(())
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[test]
fn late_document_failure_leaves_no_new_native_witnesses_or_file_identities() -> TestResult {
    let directory = tempfile::tempdir()?;
    let xml = fixture()?;
    let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let tables = FAMILIES
        .iter()
        .map(|family| family.table)
        .chain([
            "software_wrapper_attribute_positions",
            "software_documents",
            "software_wrapper_headers",
            "catalog_set_groups",
            "catalog_sets",
            "asset_occurrences",
            "catalog_contents",
            "digest_values",
            "occurrence_digest_assertions",
            "reported_catalog_relationships",
        ])
        .collect::<Vec<_>>();
    let mut before = Vec::new();
    for table in &tables {
        before.push(
            sql_query(format!("SELECT COUNT(*) AS value FROM {table}"))
                .get_result::<CountRow>(&mut connection)?
                .value,
        );
    }
    let document_path = Utf8PathBuf::try_from(directory.path().join("late-failure.xml"))?;
    let invalid = format!("{xml}<softwarelist name='trailing'>");
    std::fs::write(&document_path, invalid)?;
    let failed = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-attribute-proof"),
            source_display_name: "Software attributes".into(),
            catalog_key: CatalogKey::new("software-attribute-proof"),
            catalog_display_name: "Software attributes".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    for (table, count) in tables.iter().zip(before) {
        assert_eq!(
            sql_query(format!("SELECT COUNT(*) AS value FROM {table}"))
                .get_result::<CountRow>(&mut connection)?
                .value,
            count,
            "late failure altered {table}"
        );
    }
    check_native_positions(&path, &snapshot, &xml)
}

#[test]
fn readers_reject_fractional_attribute_ordinals_instead_of_truncating_them() -> TestResult {
    use diesel::connection::SimpleConnection;
    for table in FAMILIES
        .iter()
        .map(|family| family.table)
        .chain(["software_wrapper_attribute_positions"])
    {
        let directory = tempfile::tempdir()?;
        let xml = fixture()?;
        let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
        let lists = catalog_software::lists_for_snapshot(
            &database,
            &snapshot,
            SoftwarePageLimit::new(10)?,
            None,
        )?;
        let list = lists.lists.first().ok_or("list")?;
        let titles = catalog_software::titles_for_list(
            &database,
            &snapshot,
            list.id,
            SoftwarePageLimit::new(10)?,
            None,
        )?;
        let ids = titles
            .titles
            .iter()
            .flat_map(|title| &title.parts)
            .flat_map(|part| &part.areas)
            .flat_map(|area| area.entry_ids.iter().copied())
            .collect::<Vec<_>>();
        let mut connection = SqliteConnection::establish(path.as_str())?;
        // Test-only damage bypasses insertion/publication defenses. Readers must
        // still reject SQLite REAL values rather than let Diesel cast them to i64.
        connection.batch_execute(&format!("DROP TRIGGER {table}_immutable_update; PRAGMA ignore_check_constraints=ON; UPDATE {table} SET source_order=100.5 WHERE field_kind=0; PRAGMA ignore_check_constraints=OFF;"))?;
        match table {
            "software_list_attribute_positions" | "software_wrapper_attribute_positions" => {
                assert!(
                    catalog_software::lists_for_snapshot(
                        &database,
                        &snapshot,
                        SoftwarePageLimit::new(10)?,
                        None
                    )
                    .is_err()
                );
            }
            "software_rom_attribute_positions" | "software_disk_attribute_positions" => {
                assert!(catalog_files::occurrences_for_ids(&database, &ids).is_err());
            }
            _ => {
                assert!(
                    catalog_software::titles_for_list(
                        &database,
                        &snapshot,
                        list.id,
                        SoftwarePageLimit::new(10)?,
                        None
                    )
                    .is_err()
                );
            }
        }
        assert!(
            app::diff_catalog_snapshots(&database, &snapshot, &snapshot).is_err(),
            "history accepted fractional native ordinals: {table}"
        );
    }
    Ok(())
}

#[test]
fn history_rejects_missing_attribute_witnesses_for_every_native_owner() -> TestResult {
    use diesel::connection::SimpleConnection;
    let xml = fixture()?;
    let mut accepted = Vec::new();
    for table in FAMILIES
        .iter()
        .map(|family| family.table)
        .chain(["software_wrapper_attribute_positions"])
    {
        let directory = tempfile::tempdir()?;
        let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
        let mut connection = SqliteConnection::establish(path.as_str())?;
        // Test-only damage bypasses the seal. Every required field remains a
        // source fact: an absent witness is corruption, not an empty vector.
        connection.batch_execute(&format!("DROP TRIGGER {table}_immutable_delete;"))?;
        assert!(
            sql_query(format!("DELETE FROM {table} WHERE field_kind=0"))
                .execute(&mut connection)?
                > 0,
            "independent damaged owner: {table}"
        );
        if app::diff_catalog_snapshots(&database, &snapshot, &snapshot).is_ok() {
            accepted.push(table);
        }
    }
    assert!(
        accepted.is_empty(),
        "history accepted missing native witnesses: {accepted:?}"
    );
    Ok(())
}

#[test]
fn wrapper_history_rejects_invalid_attribute_coordinates() -> TestResult {
    use diesel::connection::SimpleConnection;
    let xml = fixture()?;
    let mut accepted = Vec::new();
    for (column, value) in [
        ("source_line", "0"),
        ("source_column", "0"),
        ("source_line", "1.5"),
        ("source_column", "1.5"),
    ] {
        let directory = tempfile::tempdir()?;
        let (database, path, snapshot) = import(&directory, xml.as_bytes())?;
        let mut connection = SqliteConnection::establish(path.as_str())?;
        connection.batch_execute(&format!("DROP TRIGGER software_wrapper_attribute_positions_immutable_update; PRAGMA ignore_check_constraints=ON; UPDATE software_wrapper_attribute_positions SET {column}={value}; PRAGMA ignore_check_constraints=OFF;"))?;
        assert!(
            catalog_software::lists_for_snapshot(
                &database,
                &snapshot,
                SoftwarePageLimit::new(10)?,
                None
            )
            .is_err()
        );
        if app::diff_catalog_snapshots(&database, &snapshot, &snapshot).is_ok() {
            accepted.push((column, value));
        }
    }
    assert!(
        accepted.is_empty(),
        "wrapper history accepted invalid QName coordinates: {accepted:?}"
    );
    Ok(())
}
