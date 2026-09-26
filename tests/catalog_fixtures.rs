use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Deserialize)]
struct Matrix {
    schema_version: u32,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
struct Fixture {
    id: String,
    path: String,
    format: String,
    dialect: String,
    source_reference: String,
    version: Option<String>,
    license: String,
    provenance: String,
    sha1: String,
    parser_status: String,
    scope: String,
    normalized_facts: Vec<String>,
    retained_raw_fields: Vec<String>,
    expected_adapter_fields: Vec<String>,
    unsupported_semantics: Vec<String>,
    expected_diagnostics: Vec<String>,
    expected: Option<ExpectedCatalog>,
    expected_error_contains: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExpectedCatalog {
    name: String,
    version: String,
    games: Vec<ExpectedGame>,
}

#[derive(Debug, Deserialize)]
struct ExpectedGame {
    name: String,
    clone_of: Option<String>,
    sourcefile: Option<String>,
    isbios: Option<String>,
    rom_of: Option<String>,
    sample_of: Option<String>,
    description: Option<String>,
    year: Option<String>,
    manufacturer: Option<String>,
    board: Option<String>,
    rebuild_to: Option<String>,
    device_refs: Vec<String>,
    roms: Vec<ExpectedRom>,
}

#[derive(Debug, Deserialize)]
struct ExpectedRom {
    name: String,
    size: Option<u64>,
    crc: Option<String>,
    md5: Option<String>,
    sha1: Option<String>,
    merge: Option<String>,
    status: Option<String>,
    serial: Option<String>,
    date: Option<String>,
}

fn matrix() -> Result<Matrix, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(include_str!(
        "../fixtures/catalog/manifest.json"
    ))?)
}

fn parse_fixture(id: &str) -> Result<mame_coalesce::logiqx::DataFile, Box<dyn std::error::Error>> {
    let fixture = matrix()?
        .fixtures
        .into_iter()
        .find(|fixture| fixture.id == id)
        .ok_or_else(|| format!("missing fixture {id}"))?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture.path);
    let bytes = fs::read(path)?;
    Ok(mame_coalesce::logiqx::DataFile::from_reader(
        bytes.as_slice(),
    )?)
}

const REQUIRED_FIXTURES: &[(&str, &str, &str)] = &[
    ("logiqx-catalog-a-snapshot-1", "logiqx", "logiqx-supported"),
    ("logiqx-catalog-a-snapshot-2", "logiqx", "logiqx-supported"),
    (
        "logiqx-catalog-b-overlap-and-conflict",
        "logiqx",
        "logiqx-supported",
    ),
    (
        "logiqx-catalog-a-filtered-alpha",
        "logiqx",
        "logiqx-supported",
    ),
    ("logiqx-malformed-xml", "logiqx", "malformed-logiqx"),
    ("mame-machine-relationships", "mame-listxml", "fixture-only"),
    (
        "clrmamepro-synthetic-subset",
        "clrmamepro-dat",
        "fixture-only",
    ),
    (
        "mame-software-list-parts",
        "mame-softwarelist-xml",
        "fixture-only",
    ),
];

fn assert_required_fixtures(matrix: &Matrix) -> Result<(), Box<dyn std::error::Error>> {
    for (id, format, parser_status) in REQUIRED_FIXTURES {
        let fixture = matrix
            .fixtures
            .iter()
            .find(|fixture| fixture.id == *id)
            .ok_or_else(|| format!("required fixture {id} is missing"))?;
        assert_eq!(fixture.format, *format, "{id}");
        assert_eq!(fixture.parser_status, *parser_status, "{id}");
    }
    Ok(())
}

fn assert_fixture_record(fixture: &Fixture) -> Result<(), Box<dyn std::error::Error>> {
    assert!(fixture.version.is_some(), "missing version: {}", fixture.id);
    assert_eq!(fixture.license, "CC0-1.0", "{}", fixture.id);
    assert!(
        fixture.provenance.to_lowercase().contains("synthetic"),
        "{}",
        fixture.id
    );
    assert!(!fixture.dialect.is_empty(), "{}", fixture.id);
    assert!(!fixture.source_reference.is_empty(), "{}", fixture.id);
    assert!(!fixture.scope.is_empty(), "{}", fixture.id);
    assert!(!fixture.normalized_facts.is_empty(), "{}", fixture.id);
    assert!(
        fixture
            .retained_raw_fields
            .iter()
            .all(|field| !field.is_empty())
    );
    assert!(
        fixture
            .unsupported_semantics
            .iter()
            .all(|semantics| !semantics.is_empty())
    );

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fixture.path);
    let bytes = fs::read(path)?;
    assert!(bytes.len() < 16 * 1024, "fixture too large: {}", fixture.id);
    assert_eq!(
        hex::encode(Sha1::digest(&bytes)),
        fixture.sha1,
        "fixture bytes changed without updating manifest: {}",
        fixture.id
    );

    match fixture.parser_status.as_str() {
        "logiqx-supported" => {
            assert_eq!(fixture.format, "logiqx");
            assert!(fixture.expected_adapter_fields.is_empty(), "{}", fixture.id);
            assert!(
                fixture.expected.is_some(),
                "missing parsed expectations: {}",
                fixture.id
            );
            assert!(fixture.expected_error_contains.is_none(), "{}", fixture.id);
        }
        "fixture-only" => {
            assert!(fixture.expected.is_none(), "{}", fixture.id);
            assert!(!fixture.unsupported_semantics.is_empty(), "{}", fixture.id);
            assert!(fixture.retained_raw_fields.is_empty(), "{}", fixture.id);
            assert!(
                !fixture.expected_adapter_fields.is_empty(),
                "missing future adapter field expectations: {}",
                fixture.id
            );
        }
        "malformed-logiqx" => {
            assert_eq!(fixture.format, "logiqx");
            assert!(fixture.expected_error_contains.is_some(), "{}", fixture.id);
            assert!(!fixture.expected_diagnostics.is_empty(), "{}", fixture.id);
        }
        status => {
            return Err(format!("unknown parser status {status:?} for {}", fixture.id).into());
        }
    }
    Ok(())
}

#[test]
fn fixture_manifest_is_complete_and_byte_pinned() -> Result<(), Box<dyn std::error::Error>> {
    let matrix = matrix()?;
    assert_eq!(matrix.schema_version, 1);
    assert!(matrix.fixtures.len() >= REQUIRED_FIXTURES.len());
    assert_required_fixtures(&matrix)?;

    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    for fixture in &matrix.fixtures {
        assert!(
            ids.insert(&fixture.id),
            "duplicate fixture id: {}",
            fixture.id
        );
        assert!(
            paths.insert(&fixture.path),
            "duplicate fixture path: {}",
            fixture.path
        );
        assert_fixture_record(fixture)?;
    }
    Ok(())
}

#[test]
fn manifest_logiqx_expectations_match_parsed_catalog_facts()
-> Result<(), Box<dyn std::error::Error>> {
    for fixture in matrix()?.fixtures {
        if fixture.parser_status != "logiqx-supported" {
            continue;
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fixture.path);
        let bytes = fs::read(path)?;
        let parsed = mame_coalesce::logiqx::DataFile::from_reader(bytes.as_slice())?;
        let expected = fixture
            .expected
            .ok_or_else(|| format!("missing expected facts for {}", fixture.id))?;

        assert_eq!(parsed.header().name(), expected.name, "{}", fixture.id);
        assert_eq!(
            parsed.header().version().map(String::as_str),
            Some(expected.version.as_str()),
            "{}",
            fixture.id
        );
        assert_eq!(parsed.games().len(), expected.games.len(), "{}", fixture.id);
        for (actual, expected) in parsed.games().iter().zip(&expected.games) {
            assert_parsed_game(actual, expected, &fixture.id);
        }
    }
    Ok(())
}

fn assert_parsed_game(
    actual: &mame_coalesce::logiqx::Game,
    expected: &ExpectedGame,
    fixture_id: &str,
) {
    assert_eq!(actual.name(), expected.name, "{fixture_id}");
    assert_eq!(
        actual.cloneof(),
        expected.clone_of.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.sourcefile_opt(),
        expected.sourcefile.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.isbios_opt(),
        expected.isbios.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.romof_opt(),
        expected.rom_of.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.sampleof_opt(),
        expected.sample_of.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.description_opt(),
        expected.description.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(actual.year_opt(), expected.year.as_deref(), "{fixture_id}");
    assert_eq!(
        actual.manufacturer_opt(),
        expected.manufacturer.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.board_opt(),
        expected.board.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.rebuildto_opt(),
        expected.rebuild_to.as_deref(),
        "{fixture_id}"
    );
    assert_eq!(
        actual.device_refs().collect::<Vec<_>>(),
        expected.device_refs,
        "{fixture_id}"
    );
    assert_eq!(actual.roms().len(), expected.roms.len(), "{fixture_id}");
    for (actual, expected) in actual.roms().iter().zip(&expected.roms) {
        assert_parsed_rom(actual, expected, fixture_id);
    }
}

fn assert_parsed_rom(
    actual: &mame_coalesce::logiqx::Rom,
    expected: &ExpectedRom,
    fixture_id: &str,
) {
    assert_eq!(actual.name(), expected.name, "{fixture_id}");
    assert_eq!(actual.size(), expected.size, "{fixture_id}");
    assert_eq!(actual.crc().map(hex::encode), expected.crc, "{fixture_id}");
    assert_eq!(actual.md5().map(hex::encode), expected.md5, "{fixture_id}");
    assert_eq!(
        actual.sha1().map(hex::encode),
        expected.sha1,
        "{fixture_id}"
    );
    assert_eq!(actual.merge(), expected.merge.as_deref(), "{fixture_id}");
    assert_eq!(actual.status(), expected.status.as_deref(), "{fixture_id}");
    assert_eq!(actual.serial(), expected.serial.as_deref(), "{fixture_id}");
    assert_eq!(actual.date(), expected.date.as_deref(), "{fixture_id}");
}

#[test]
fn malformed_logiqx_fixture_fails_with_declared_diagnostic()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = matrix()?
        .fixtures
        .into_iter()
        .find(|fixture| fixture.parser_status == "malformed-logiqx")
        .ok_or("manifest has no malformed Logiqx fixture")?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fixture.path);
    let bytes = fs::read(path)?;
    let error = mame_coalesce::logiqx::DataFile::from_reader(bytes.as_slice())
        .err()
        .ok_or("malformed fixture unexpectedly parsed")?;
    let expected = fixture
        .expected_error_contains
        .ok_or("malformed fixture has no expected diagnostic")?;
    assert!(error.to_string().contains(&expected), "{error}");
    Ok(())
}

#[test]
fn manifest_facts_cover_overlap_snapshot_and_filtered_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let a_v1 = parse_fixture("logiqx-catalog-a-snapshot-1")?;
    let b = parse_fixture("logiqx-catalog-b-overlap-and-conflict")?;
    let a_v2 = parse_fixture("logiqx-catalog-a-snapshot-2")?;
    let filtered = parse_fixture("logiqx-catalog-a-filtered-alpha")?;

    let a_v1_roms = a_v1.games()[0].roms();
    let b_roms = b.games()[0].roms();
    assert_ne!(a_v1.games()[0].name(), b.games()[0].name());
    assert_ne!(a_v1_roms[0].name(), b_roms[0].name());
    assert_eq!(a_v1_roms[0].size(), b_roms[0].size());
    assert_eq!(a_v1_roms[0].crc(), b_roms[0].crc());
    assert_eq!(a_v1_roms[0].sha1(), b_roms[0].sha1());
    assert_ne!(a_v1_roms[1].sha1(), b_roms[1].sha1());

    let a_v2_roms = a_v2.games()[0].roms();
    assert_eq!(a_v1_roms[0].sha1(), a_v2_roms[0].sha1());
    assert_ne!(a_v1_roms[1].sha1(), a_v2_roms[1].sha1());
    assert_eq!(filtered.games()[0].roms().len(), 1);
    assert_eq!(a_v2.games()[0].roms().len(), 2);
    let filtered_scope = matrix()?
        .fixtures
        .into_iter()
        .find(|fixture| fixture.id == "logiqx-catalog-a-filtered-alpha")
        .ok_or("missing filtered fixture record")?
        .scope;
    assert!(filtered_scope.contains("not deletion"));
    Ok(())
}
