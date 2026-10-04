use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_no_intro_database::{
        NoIntroDatabaseGame, NoIntroDatabaseGameChild, NoIntroDatabasePageLimit, games_for_snapshot,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};

#[derive(QueryableByName, Debug)]
struct Diagnostic {
    #[diesel(sql_type = Text)]
    message: String,
}

#[derive(QueryableByName)]
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

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type PublicPosition = (i64, usize, i64, i64);

const NAMESPACE_GAP_XML: &str = r#"<datafile xmlns:v="urn:vendor">
<game name="gaps">
<archive xmlns:v="urn:archive" name="archive" xmlns:w="urn:gap" number="0007"/>
<source>
<details xmlns:v="urn:dump-details" dumper="dumper" xmlns:w="urn:gap" id="001"/>
<serials xmlns:v="urn:dump-serials" box_serial="serial"/>
<file xmlns:v="urn:dump-file" size="0001" xmlns:w="urn:gap" sha1="1111111111111111111111111111111111111111"/>
</source>
<release>
<details xmlns:v="urn:release-details" group="group" xmlns:w="urn:gap" nfo_crc32="12345678"/>
<serials xmlns:v="urn:release-serials" box_serial="serial"/>
<file xmlns:v="urn:release-file" size="0001" xmlns:w="urn:gap" sha1="1111111111111111111111111111111111111111"/>
</release>
</game>
</datafile>"#;

struct GapFamily {
    table: &'static str,
    opening: &'static str,
    fields: &'static [(i64, i64, &'static str)],
}

const GAP_FAMILIES: [GapFamily; 7] = [
    GapFamily {
        table: "no_intro_archive_field_positions",
        opening: "<archive",
        fields: &[(18, 1, "name="), (20, 3, "number=")],
    },
    GapFamily {
        table: "no_intro_dump_details_field_positions",
        opening: "<details xmlns:v=\"urn:dump-details\"",
        fields: &[(4, 1, "dumper="), (5, 3, "id=")],
    },
    GapFamily {
        table: "no_intro_dump_serials_field_positions",
        opening: "<serials xmlns:v=\"urn:dump-serials\"",
        fields: &[(1, 1, "box_serial=")],
    },
    GapFamily {
        table: "no_intro_dump_file_field_positions",
        opening: "<file xmlns:v=\"urn:dump-file\"",
        fields: &[(19, 1, "size="), (17, 3, "sha1=")],
    },
    GapFamily {
        table: "no_intro_release_details_field_positions",
        opening: "<details xmlns:v=\"urn:release-details\"",
        fields: &[(5, 1, "group="), (7, 3, "nfo_crc32=")],
    },
    GapFamily {
        table: "no_intro_release_serials_field_positions",
        opening: "<serials xmlns:v=\"urn:release-serials\"",
        fields: &[(1, 1, "box_serial=")],
    },
    GapFamily {
        table: "no_intro_release_file_field_positions",
        opening: "<file xmlns:v=\"urn:release-file\"",
        fields: &[(14, 1, "size="), (12, 3, "sha1=")],
    },
];

#[test]
fn lexical_namespace_attribute_gaps_do_not_prevent_native_publication() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let input = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(&input, NAMESPACE_GAP_XML)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: input,
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("lexical-gaps"),
            source_display_name: "Lexical gaps".into(),
            catalog_key: CatalogKey::new("lexical-gaps"),
            catalog_display_name: "Lexical gaps".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let diagnostics = sql_query("SELECT message FROM import_diagnostics ORDER BY diagnostic_key")
        .load::<Diagnostic>(&mut connection)?
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert_eq!(
        report.status,
        CatalogImportStatus::Succeeded,
        "{diagnostics:?}"
    );
    let snapshot = report.snapshot_key.ok_or("missing published snapshot")?;
    let page = games_for_snapshot(
        &database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    let game = page.games.first().ok_or("missing game")?;
    let public_positions = public_attribute_positions(game)?;
    for family in &GAP_FAMILIES {
        let queried_positions = public_positions
            .get(family.table)
            .ok_or("missing query position family")?;
        assert_namespace_gap_family(
            &mut connection,
            NAMESPACE_GAP_XML,
            family,
            queried_positions,
        )?;
    }
    Ok(())
}

fn public_attribute_positions(
    game: &NoIntroDatabaseGame,
) -> TestResult<BTreeMap<&'static str, Vec<PublicPosition>>> {
    let archive = game
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::Archive(value) => Some(value),
            _ => None,
        })
        .ok_or("missing archive")?;
    let dump = game
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::DumpSource(value) => Some(value),
            _ => None,
        })
        .ok_or("missing dump history")?;
    let release = game
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::Release(value) => Some(value),
            _ => None,
        })
        .ok_or("missing release history")?;
    macro_rules! positions {
        ($positions:expr) => {
            $positions
                .iter()
                .map(|position| {
                    (
                        position.field as i64,
                        position.source_order,
                        position.location.line,
                        position.location.column,
                    )
                })
                .collect::<Vec<_>>()
        };
    }
    let dump_file = dump.files.first().ok_or("missing dump file")?;
    let release_details = release.details.as_ref().ok_or("missing release details")?;
    let release_file = release.files.first().ok_or("missing release file")?;
    Ok(BTreeMap::from([
        (
            "no_intro_archive_field_positions",
            positions!(archive.attribute_positions),
        ),
        (
            "no_intro_dump_details_field_positions",
            positions!(dump.details_attribute_positions),
        ),
        (
            "no_intro_dump_serials_field_positions",
            positions!(dump.serials_attribute_positions),
        ),
        (
            "no_intro_dump_file_field_positions",
            positions!(dump_file.attribute_positions),
        ),
        (
            "no_intro_release_details_field_positions",
            positions!(release_details.attribute_positions),
        ),
        (
            "no_intro_release_serials_field_positions",
            positions!(release.serials_attribute_positions),
        ),
        (
            "no_intro_release_file_field_positions",
            positions!(release_file.attribute_positions),
        ),
    ]))
}

fn assert_namespace_gap_family(
    connection: &mut SqliteConnection,
    xml: &str,
    family: &GapFamily,
    queried_positions: &[PublicPosition],
) -> TestResult {
    let table = family.table;
    let rows = sql_query(format!(
        "SELECT field_kind,source_order,source_line,source_column FROM {table} ORDER BY source_order"
    )).load::<Position>(connection)?;
    assert_eq!(rows.len(), family.fields.len(), "{table}");
    assert_eq!(queried_positions.len(), family.fields.len(), "{table}");
    let (line_index, line) = xml
        .lines()
        .enumerate()
        .find(|(_, line)| line.starts_with(family.opening))
        .ok_or("missing opening tag")?;
    for ((row, queried), (kind, ordinal, qname)) in rows
        .iter()
        .zip(queried_positions)
        .zip(family.fields.iter().copied())
    {
        assert_eq!(
            (row.field_kind, row.source_order),
            (kind, ordinal),
            "{table}"
        );
        assert_eq!(
            *queried,
            (
                kind,
                usize::try_from(ordinal)?,
                row.source_line,
                row.source_column
            ),
            "{table} public metadata must retain the independently checked lexical position"
        );
        assert_eq!(row.source_line, i64::try_from(line_index + 1)?);
        assert_eq!(
            row.source_column,
            i64::try_from(line.find(qname).ok_or("missing QName")? + 1)?
        );
    }
    Ok(())
}
