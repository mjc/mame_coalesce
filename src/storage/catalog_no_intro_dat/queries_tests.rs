use std::{error::Error, fmt::Write as _, path::Path};

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use tempfile::TempDir;

use crate::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, OccurrenceId, PublishingSourceKey},
    storage::catalog_no_intro_dat::queries,
};

type TestResult<T> = Result<T, Box<dyn Error>>;

#[derive(QueryableByName)]
struct GroupIdRow {
    #[diesel(sql_type = BigInt)]
    set_group_id: i64,
}

#[derive(QueryableByName)]
struct CoverageIdRow {
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[derive(QueryableByName)]
struct GameAnchorRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    list_order: i64,
}

#[derive(QueryableByName)]
struct OccurrenceIdRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

struct Fixture {
    _directory: TempDir,
    database: Database,
    target_snapshot: String,
}

struct SelectedOwners {
    group_id: i64,
    anchors: Vec<GameAnchorRow>,
    game_ids: Vec<i64>,
    occurrence_ids: Vec<i64>,
}

struct QueryPlanCase {
    label: &'static str,
    statement: String,
    aliases: Vec<&'static str>,
}

#[test]
fn production_queries_seek_requested_native_owners_with_unrelated_snapshots_present()
-> TestResult<()> {
    let fixture = populated_fixture()?;
    let mut connection = fixture.database.pool().get()?;
    analyze_in_transaction(&mut connection)?;
    let owners = selected_owners(&mut connection, &fixture.target_snapshot)?;

    assert_provenance_plans(&mut connection, &fixture.target_snapshot)?;
    assert_game_plans(&mut connection, &owners)?;
    assert_child_and_parent_plans(&mut connection, &owners)?;
    assert_rom_plans(&mut connection, &fixture.target_snapshot, &owners)?;
    exercise_shared_payload_loader(&mut connection, &owners.occurrence_ids)?;
    Ok(())
}

fn populated_fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|error| crate::Error::InvalidPath(error.display().to_string()))?;
    let database = Database::open(&database_path)?;
    let target_snapshot = import_populated_dat(&database, directory.path(), "target", 180)?;
    let unrelated_snapshot = import_populated_dat(&database, directory.path(), "unrelated", 220)?;
    let mut connection = database.pool().get()?;
    let coverage = sql_query("SELECT coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text, _>(&unrelated_snapshot)
        .get_result::<CoverageIdRow>(&mut connection)?;
    seed_unrelated_registry(&mut connection, coverage.coverage_id)?;
    Ok(Fixture {
        _directory: directory,
        database,
        target_snapshot,
    })
}

fn seed_unrelated_registry(connection: &mut SqliteConnection, coverage_id: i64) -> TestResult<()> {
    connection.immediate_transaction(|connection| {
        connection.batch_execute(&unrelated_registry_seed_sql(coverage_id))?;
        Ok::<(), diesel::result::Error>(())
    })?;
    Ok(())
}

fn unrelated_registry_seed_sql(coverage_id: i64) -> String {
    UNRELATED_REGISTRY_SEED_SQL.replace("__COVERAGE_ID__", &coverage_id.to_string())
}

const UNRELATED_REGISTRY_SEED_SQL: &str = r"
WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO parser_interpretations(
    interpretation_key, format, parser_name, parser_version, rules_version
)
SELECT printf('plan-extra-interpretation-%03d', n), 'no-intro-dat-v4-compatible',
       'query-plan-fixture', '1', 'query-plan-fixture-v1'
FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO documents(document_key)
SELECT printf('plan-extra-document-%03d', n) FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO catalog_snapshots(
    snapshot_key, catalog_key, document_key, interpretation_key, coverage_id
)
SELECT printf('plan-extra-snapshot-%03d', n), 'plan-catalog-unrelated',
       printf('plan-extra-document-%03d', n),
       printf('plan-extra-interpretation-%03d', n), __COVERAGE_ID__
FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO catalog_set_groups(snapshot_key, kind, list_order)
SELECT printf('plan-extra-snapshot-%03d', n), 'root', 0 FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO no_intro_dat_documents(snapshot_key, source_line, source_column)
SELECT printf('plan-extra-snapshot-%03d', n), 1, 1 FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO no_intro_dat_headers(
    snapshot_key, source_order, source_line, source_column, id_text, name, description, version_text
)
SELECT printf('plan-extra-snapshot-%03d', n), 0, 1, 1,
       '1', 'Plan DAT', 'Plan fixture', '1'
FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
), fields(field_kind, source_order, source_column) AS (
    VALUES(0, 0, 1), (1, 1, 2), (2, 2, 3), (3, 3, 4)
)
INSERT INTO no_intro_dat_header_field_positions(
    snapshot_key, field_kind, source_order, source_line, source_column
)
SELECT printf('plan-extra-snapshot-%03d', n), field_kind, source_order, 1, source_column
FROM numbers CROSS JOIN fields;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO no_intro_dat_parse_counts(
    snapshot_key, game_count, rom_count, category_count, identifier_count, release_count,
    clrmamepro_option_count, romcenter_option_count, header_field_count,
    clrmamepro_field_count, romcenter_field_count, game_field_count, rom_field_count
)
SELECT printf('plan-extra-snapshot-%03d', n), 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0
FROM numbers;

WITH RECURSIVE numbers(n) AS (
    VALUES(1) UNION ALL SELECT n + 1 FROM numbers WHERE n < 128
)
INSERT INTO snapshot_publications(catalog_key, document_key, interpretation_key, snapshot_key)
SELECT 'plan-catalog-unrelated', printf('plan-extra-document-%03d', n),
       printf('plan-extra-interpretation-%03d', n),
       printf('plan-extra-snapshot-%03d', n)
FROM numbers;
";

fn analyze_in_transaction(connection: &mut SqliteConnection) -> TestResult<()> {
    connection.immediate_transaction(|connection| {
        connection.batch_execute("ANALYZE")?;
        Ok::<(), diesel::result::Error>(())
    })?;
    Ok(())
}

fn selected_owners(
    connection: &mut SqliteConnection,
    snapshot: &str,
) -> TestResult<SelectedOwners> {
    let group = sql_query(
        "SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key=? AND kind='root'",
    )
    .bind::<Text, _>(snapshot)
    .get_result::<GroupIdRow>(connection)?;
    let anchors = sql_query(
        "SELECT set_id, list_order FROM catalog_sets \
         WHERE set_group_id=? ORDER BY list_order LIMIT 2",
    )
    .bind::<BigInt, _>(group.set_group_id)
    .load::<GameAnchorRow>(connection)?;
    require_len(&anchors, 2, "two target game anchors")?;
    let first = required(&anchors, 0, "first target anchor")?;
    let second = required(&anchors, 1, "second target anchor")?;
    let first_game_id = first.set_id;
    let second_game_id = second.set_id;
    let game_ids = vec![first_game_id, second_game_id];
    let occurrence_ids = sql_query(
        "SELECT occurrence_id FROM asset_occurrences WHERE record_id IN (?,?) \
         ORDER BY record_id, occurrence_order",
    )
    .bind::<BigInt, _>(first_game_id)
    .bind::<BigInt, _>(second_game_id)
    .load::<OccurrenceIdRow>(connection)?
    .into_iter()
    .map(|row| row.occurrence_id)
    .collect::<Vec<_>>();
    require_len(&occurrence_ids, 2, "two target ROM occurrences")?;
    Ok(SelectedOwners {
        group_id: group.set_group_id,
        anchors,
        game_ids,
        occurrence_ids,
    })
}

fn assert_provenance_plans(connection: &mut SqliteConnection, snapshot: &str) -> TestResult<()> {
    assert_plan_indexed(
        connection,
        "snapshot provenance",
        queries::SNAPSHOT,
        &[],
        Some(snapshot),
        &[
            "snapshots",
            "catalogs",
            "sources",
            "documents",
            "interpretations",
            "publication",
        ],
    )?;
    for (label, statement, table) in [
        (
            "sealed parse counts",
            queries::COUNTS,
            "no_intro_dat_parse_counts",
        ),
        (
            "document owner",
            queries::DOCUMENT,
            "no_intro_dat_documents",
        ),
        ("header owner", queries::HEADER, "no_intro_dat_headers"),
        (
            "ClrMamePro directive",
            queries::CLRMAMEPRO,
            "no_intro_dat_clrmamepro_options",
        ),
        (
            "RomCenter directive",
            queries::ROMCENTER,
            "no_intro_dat_romcenter_options",
        ),
    ] {
        assert_plan_indexed(connection, label, statement, &[], Some(snapshot), &[table])?;
    }
    assert_plan_indexed(
        connection,
        "all snapshot groups",
        queries::ROOT_GROUPS,
        &[],
        Some(snapshot),
        &["catalog_set_groups"],
    )
}

fn assert_game_plans(connection: &mut SqliteConnection, owners: &SelectedOwners) -> TestResult<()> {
    assert_plan_indexed(
        connection,
        "initial game page",
        &queries::games(false),
        &[owners.group_id, 2],
        None,
        &["sets", "native"],
    )?;
    let anchor = required(&owners.anchors, 0, "cursor anchor")?;
    assert_plan_indexed(
        connection,
        "continuation game page",
        &queries::games(true),
        &[owners.group_id, anchor.list_order, anchor.set_id, 2],
        None,
        &["sets", "native"],
    )?;
    assert_plan_indexed(
        connection,
        "actual cursor anchor",
        queries::CURSOR_ANCHOR,
        &[owners.group_id, anchor.set_id, anchor.list_order],
        None,
        &["sets", "native"],
    )
}

fn assert_child_and_parent_plans(
    connection: &mut SqliteConnection,
    owners: &SelectedOwners,
) -> TestResult<()> {
    for case in child_and_parent_statements(owners.game_ids.len()) {
        assert_plan_indexed(
            connection,
            case.label,
            &case.statement,
            &owners.game_ids,
            None,
            &case.aliases,
        )?;
    }
    assert_plan_indexed(
        connection,
        "ROM attribute positions",
        &queries::position_rows(
            "no_intro_dat_rom_field_positions",
            "occurrence_id",
            owners.occurrence_ids.len(),
        ),
        &owners.occurrence_ids,
        None,
        &["native"],
    )
}

fn child_and_parent_statements(count: usize) -> Vec<QueryPlanCase> {
    vec![
        QueryPlanCase {
            label: "game attribute positions",
            statement: queries::position_rows("no_intro_dat_game_field_positions", "set_id", count),
            aliases: vec!["native"],
        },
        QueryPlanCase {
            label: "categories and family rank",
            statement: queries::category_rows(count),
            aliases: vec!["native"],
        },
        QueryPlanCase {
            label: "identifiers and family rank",
            statement: queries::identifier_rows(count),
            aliases: vec!["native"],
        },
        QueryPlanCase {
            label: "releases and family rank",
            statement: queries::release_rows(count),
            aliases: vec!["native"],
        },
        QueryPlanCase {
            label: "parent relationship registry",
            statement: queries::parent_rows(count),
            aliases: vec!["link", "reported", "identity"],
        },
        QueryPlanCase {
            label: "mixed child source order",
            statement: queries::mixed_child_rows(count),
            aliases: vec!["native", "occurrence", "position"],
        },
    ]
}

fn assert_rom_plans(
    connection: &mut SqliteConnection,
    snapshot: &str,
    owners: &SelectedOwners,
) -> TestResult<()> {
    assert_plan_indexed(
        connection,
        "ROM references in exact published snapshot",
        &queries::rom_rows(owners.game_ids.len()),
        &owners.game_ids,
        Some(snapshot),
        &[
            "occurrence",
            "claim",
            "sets",
            "native_game",
            "groups",
            "snapshot",
            "interpretation",
        ],
    )
}

fn exercise_shared_payload_loader(
    connection: &mut SqliteConnection,
    occurrence_ids: &[i64],
) -> TestResult<()> {
    let ids = occurrence_ids
        .iter()
        .copied()
        .map(OccurrenceId::try_from)
        .collect::<crate::Result<Vec<_>>>()?;
    let payloads = crate::storage::catalog_files::no_intro_dat_rom_payloads(connection, &ids)?;
    if payloads.len() != ids.len() {
        return Err(test_error(
            "shared ROM payload loader returned an unexpected owner count",
        ));
    }
    Ok(())
}

fn assert_plan_indexed(
    connection: &mut SqliteConnection,
    label: &str,
    statement: &str,
    integer_binds: &[i64],
    trailing_text: Option<&str>,
    aliases: &[&str],
) -> TestResult<()> {
    let details = explain(connection, statement, integer_binds, trailing_text)?;
    if details.is_empty() {
        return Err(test_error(format!(
            "{label}: EXPLAIN returned no plan rows"
        )));
    }
    for alias in aliases {
        assert_indexed_alias(label, &details, alias)?;
    }
    Ok(())
}

fn assert_indexed_alias(label: &str, details: &[String], alias: &str) -> TestResult<()> {
    let indexed_search = details
        .iter()
        .any(|detail| detail.starts_with(&format!("search {alias} ")) && detail.contains("using"));
    if !indexed_search {
        return Err(test_error(format!(
            "{label}: no indexed lookup for {alias}; plan: {details:?}"
        )));
    }
    let scanned = details
        .iter()
        .any(|detail| detail.starts_with(&format!("scan {alias}")));
    if scanned {
        return Err(test_error(format!(
            "{label}: unbounded scan of {alias}; plan: {details:?}"
        )));
    }
    Ok(())
}

fn explain(
    connection: &mut SqliteConnection,
    statement: &str,
    integer_binds: &[i64],
    trailing_text: Option<&str>,
) -> diesel::QueryResult<Vec<String>> {
    let mut query =
        sql_query(format!("EXPLAIN QUERY PLAN {statement}")).into_boxed::<diesel::sqlite::Sqlite>();
    for value in integer_binds {
        query = query.bind::<BigInt, _>(*value);
    }
    if let Some(value) = trailing_text {
        query = query.bind::<Text, _>(value);
    }
    query.load::<PlanRow>(connection).map(|rows| {
        rows.into_iter()
            .map(|row| row.detail.to_ascii_lowercase())
            .collect()
    })
}

fn import_populated_dat(
    database: &Database,
    root: &Path,
    label: &str,
    game_count: usize,
) -> TestResult<String> {
    let input = Utf8PathBuf::from_path_buf(root.join(format!("{label}.dat")))
        .map_err(|error| crate::Error::InvalidPath(error.display().to_string()))?;
    write_fixture(&input, game_count)?;
    let report = app::import_catalog(database, &import_request(input, label))?;
    if report.status != CatalogImportStatus::Succeeded {
        return Err(test_error(format!(
            "{label} No-Intro DAT setup import did not publish"
        )));
    }
    report
        .snapshot_key
        .map(|snapshot| snapshot.as_str().to_owned())
        .ok_or_else(|| test_error(format!("{label} No-Intro DAT snapshot key missing")))
}

fn write_fixture(path: &Utf8PathBuf, game_count: usize) -> TestResult<()> {
    let mut xml = String::from(
        "<datafile><header><id>1</id><name>Plan DAT</name><description>Plan</description>\
         <version>1</version><clrmamepro forcenodump='ignore' header='global'/>\
         <romcenter plugin='plugin'/></header>",
    );
    for index in 0..game_count {
        write_game(&mut xml, index)?;
    }
    xml.push_str("</datafile>");
    std::fs::write(path, xml)?;
    Ok(())
}

fn write_game(xml: &mut String, index: usize) -> std::fmt::Result {
    let parent = if index == 0 {
        String::new()
    } else {
        " cloneof='game0000' cloneofid='0000'".to_owned()
    };
    write!(
        xml,
        "<game name='game{index:04}'{parent}><description>Game {index}</description>\
         <category>Arcade</category><game_id>{index:04}</game_id>\
         <release name='World' region='US'/>\
         <rom name='file{index}.bin' size='4' crc='{index:08x}'/></game>"
    )
}

fn import_request(document_path: Utf8PathBuf, label: &str) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        source_key: PublishingSourceKey::new(format!("plan-source-{label}")),
        source_display_name: format!("Plan source {label}"),
        catalog_key: CatalogKey::new(format!("plan-catalog-{label}")),
        catalog_display_name: format!("Plan catalog {label}"),
        scope: CatalogScope::Complete,
    }
}

fn require_len<T>(items: &[T], expected: usize, label: &str) -> TestResult<()> {
    if items.len() != expected {
        return Err(test_error(format!(
            "expected {expected} {label}, found {}",
            items.len()
        )));
    }
    Ok(())
}

fn required<'a, T>(items: &'a [T], index: usize, label: &str) -> TestResult<&'a T> {
    items
        .get(index)
        .ok_or_else(|| test_error(format!("missing {label} at position {index}")))
}

fn test_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}
