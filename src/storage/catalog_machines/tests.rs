use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, connection::SimpleConnection, sql_query, sql_types::Text,
};

use super::{Database, queries};

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

type PlanQuery = (&'static str, &'static str, &'static [&'static str]);

const CHILD_POSITION_TABLES: &[&str] = &[
    "mame_machines",
    "mame_bios_sets",
    "mame_machine_dependencies",
    "machine_switches",
    "mame_machine_samples",
    "mame_machine_chips",
    "mame_machine_displays",
    "mame_machine_sounds",
    "mame_machine_inputs",
    "mame_machine_ports",
    "mame_machine_adjusters",
    "mame_machine_drivers",
    "mame_machine_features",
    "mame_machine_devices",
    "mame_machine_slots",
    "mame_machine_software_lists",
    "mame_machine_ram_options",
    "asset_occurrences",
    "mame_rom_claims",
    "mame_disk_claims",
];
const NATIVE_ALIAS: &[&str] = &["native"];
const ASSET_ALIASES: &[&str] = &["occurrences", "claims"];

#[test]
fn requested_machine_queries_seek_native_primary_keys() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let mut connection = database.pool().get()?;
    connection.batch_execute(
        "CREATE TEMP TABLE catalog_machine_requested_owners \
           (owner_id INTEGER PRIMARY KEY) WITHOUT ROWID;",
    )?;
    sql_query("INSERT INTO temp.catalog_machine_requested_owners(owner_id) VALUES (1)")
        .execute(&mut connection)?;
    connection.batch_execute("ANALYZE; ANALYZE temp.catalog_machine_requested_owners")?;

    for &(label, query, aliases) in PLAN_QUERIES {
        assert_query_uses_owner_keys(&mut connection, label, query, aliases)?;
    }
    Ok(())
}

const PLAN_QUERIES: &[PlanQuery] = &[
    (
        "child positions",
        queries::CHILD_POSITIONS,
        CHILD_POSITION_TABLES,
    ),
    ("BIOS sets", queries::BIOS_SETS, NATIVE_ALIAS),
    ("clone links", queries::CLONE_LINKS, NATIVE_ALIAS),
    ("dependencies", queries::DEPENDENCIES, NATIVE_ALIAS),
    ("ROM assets", queries::ROM_ASSETS, ASSET_ALIASES),
    ("disk assets", queries::DISK_ASSETS, ASSET_ALIASES),
    ("switches", queries::SWITCHES, NATIVE_ALIAS),
    (
        "switch conditions",
        queries::SWITCH_CONDITIONS,
        NATIVE_ALIAS,
    ),
    ("switch locations", queries::SWITCH_LOCATIONS, NATIVE_ALIAS),
    ("switch values", queries::SWITCH_VALUES, NATIVE_ALIAS),
    (
        "switch value conditions",
        queries::SWITCH_VALUE_CONDITIONS,
        NATIVE_ALIAS,
    ),
    ("samples", queries::SPEC_SAMPLES, NATIVE_ALIAS),
    ("chips", queries::SPEC_CHIPS, NATIVE_ALIAS),
    ("displays", queries::SPEC_DISPLAYS, NATIVE_ALIAS),
    ("sounds", queries::SPEC_SOUNDS, NATIVE_ALIAS),
    ("inputs", queries::SPEC_INPUTS, NATIVE_ALIAS),
    ("controls", queries::SPEC_CONTROLS, NATIVE_ALIAS),
    ("ports", queries::SPEC_PORTS, NATIVE_ALIAS),
    ("analogs", queries::SPEC_ANALOGS, NATIVE_ALIAS),
    ("adjusters", queries::SPEC_ADJUSTERS, NATIVE_ALIAS),
    (
        "adjuster conditions",
        queries::SPEC_ADJUSTER_CONDITIONS,
        NATIVE_ALIAS,
    ),
    ("drivers", queries::SPEC_DRIVERS, NATIVE_ALIAS),
    ("features", queries::SPEC_FEATURES, NATIVE_ALIAS),
    ("devices", queries::SPEC_DEVICES, NATIVE_ALIAS),
    (
        "device instances",
        queries::SPEC_DEVICE_INSTANCES,
        NATIVE_ALIAS,
    ),
    (
        "device extensions",
        queries::SPEC_DEVICE_EXTENSIONS,
        NATIVE_ALIAS,
    ),
    ("slots", queries::SPEC_SLOTS, NATIVE_ALIAS),
    ("slot options", queries::SPEC_SLOT_OPTIONS, NATIVE_ALIAS),
    (
        "software-list references",
        queries::SPEC_SOFTWARE_LISTS,
        NATIVE_ALIAS,
    ),
    ("RAM options", queries::SPEC_RAM_OPTIONS, NATIVE_ALIAS),
];

fn assert_query_uses_owner_keys(
    connection: &mut diesel::SqliteConnection,
    label: &str,
    query: &str,
    aliases: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let details = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
        .load::<ExplainRow>(connection)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    assert!(!details.is_empty(), "{label} query returned no plan rows");
    let scans_native = details.iter().any(|detail| {
        aliases
            .iter()
            .any(|alias| detail.starts_with(&format!("SCAN {alias}")))
    });
    assert!(!scans_native, "{label} has an unbounded scan: {details:?}");

    for alias in aliases {
        let seek = details
            .iter()
            .find(|detail| detail.starts_with(&format!("SEARCH {alias} USING")))
            .ok_or_else(|| {
                std::io::Error::other(format!("{label} does not seek {alias}: {details:?}"))
            })?;
        let expected_key = match *alias {
            "claims" | "mame_rom_claims" | "mame_disk_claims" => "INTEGER PRIMARY KEY",
            "occurrences" | "asset_occurrences" => "INDEX",
            _ => "PRIMARY KEY",
        };
        assert!(
            seek.contains(expected_key),
            "{label} does not use {alias}'s expected key ({expected_key}): {details:?}"
        );
    }
    Ok(())
}
