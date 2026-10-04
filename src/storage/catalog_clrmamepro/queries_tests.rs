use super::{document, queries};
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use std::{error::Error, fmt::Write as _};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type=BigInt)]
    value: i64,
}
#[derive(QueryableByName)]
struct AnchorRow {
    #[diesel(sql_type=BigInt)]
    set_id: i64,
    #[diesel(sql_type=BigInt)]
    list_order: i64,
}
#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type=Text)]
    detail: String,
}

#[test]
fn production_queries_seek_actual_cmp_owners_with_unrelated_registry_rows() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "database path is not UTF-8")?;
    let database = Database::open(&path)?;
    let target = import_fixture(&database, directory.path(), "target", 180)?;
    let unrelated = import_fixture(&database, directory.path(), "unrelated", 220)?;
    let mut connection = database.pool().get()?;
    seed_registry(&mut connection, &unrelated)?;
    connection.batch_execute("ANALYZE")?;
    let group =
        sql_query("SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key=?")
            .bind::<Text, _>(&target)
            .get_result::<IdRow>(&mut connection)?
            .value;
    let anchors=sql_query("SELECT set_id,list_order FROM catalog_sets WHERE set_group_id=? ORDER BY list_order LIMIT 3")
        .bind::<BigInt,_>(group).load::<AnchorRow>(&mut connection)?;
    let anchor = anchors.first().ok_or("missing query-plan anchor")?;
    assert_plan(
        &mut connection,
        "snapshot",
        queries::SNAPSHOT,
        &[],
        Some(&target),
        &["snapshots", "header"],
    )?;
    assert_plan(
        &mut connection,
        "root group",
        queries::GROUPS,
        &[],
        Some(&target),
        &["catalog_set_groups"],
    )?;
    assert_plan(
        &mut connection,
        "initial set page",
        &queries::sets(false),
        &[group, 3],
        None,
        &["sets", "native"],
    )?;
    assert_plan(
        &mut connection,
        "continuation",
        &queries::sets(true),
        &[group, anchor.list_order, anchor.set_id, 3],
        None,
        &["sets", "native"],
    )?;
    assert_plan(
        &mut connection,
        "actual anchor",
        &queries::anchor(),
        &[group, anchor.set_id, anchor.list_order],
        None,
        &["sets", "native"],
    )?;
    let ids = anchors.iter().map(|row| row.set_id).collect::<Vec<_>>();
    assert_children_plans(&mut connection, &ids)?;
    for (label, statement, aliases) in document::plan_statements() {
        assert_plan(
            &mut connection,
            label,
            statement,
            &[],
            Some(&target),
            &aliases,
        )?;
    }
    Ok(())
}

fn assert_children_plans(connection: &mut SqliteConnection, ids: &[i64]) -> TestResult {
    for (label, statement, aliases) in [
        (
            "set field positions",
            queries::set_positions(ids.len()),
            vec!["native"],
        ),
        (
            "actual parent references",
            queries::parents(ids.len()),
            vec!["native", "registry", "reported"],
        ),
        (
            "mixed media enumeration",
            queries::media(ids.len()),
            vec!["occurrence"],
        ),
    ] {
        assert_plan(connection, label, &statement, ids, None, &aliases)?;
    }
    Ok(())
}

fn assert_plan(
    connection: &mut SqliteConnection,
    label: &str,
    statement: &str,
    integers: &[i64],
    text: Option<&str>,
    aliases: &[&str],
) -> TestResult {
    let mut query =
        sql_query(format!("EXPLAIN QUERY PLAN {statement}")).into_boxed::<diesel::sqlite::Sqlite>();
    for value in integers {
        query = query.bind::<BigInt, _>(*value);
    }
    if let Some(value) = text {
        query = query.bind::<Text, _>(value);
    }
    let details = query
        .load::<PlanRow>(connection)?
        .into_iter()
        .map(|row| row.detail.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for alias in aliases {
        if !details.iter().any(|detail| {
            detail.starts_with(&format!("search {alias} ")) && detail.contains("using")
        }) || details
            .iter()
            .any(|detail| detail.starts_with(&format!("scan {alias} ")))
        {
            return Err(
                format!("{label}: no bounded indexed lookup for {alias}: {details:?}").into(),
            );
        }
    }
    Ok(())
}

fn import_fixture(
    database: &Database,
    directory: &std::path::Path,
    label: &str,
    sets: usize,
) -> TestResult<String> {
    let path = Utf8PathBuf::from_path_buf(directory.join(format!("{label}.dat")))
        .map_err(|_| "input path is not UTF-8")?;
    let mut text = String::from(
        "; document comment\nclrmamepro ( NAME \"Plan CMP\" VERSION \"1\" forcenodump ignore )\n",
    );
    for index in 0..sets {
        writeln!(
            text,
            "GAME ( name \"game{index:04}\" cloneof \"game0000\" sampleof \"samples\" sample \"a.wav\" ROM ( NAME \"a.bin\" SIZE 0004 CRC AABBCCDD CRC32 aabbccdd SHA1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA merge \"parent.bin\" ) )"
        )?;
    }
    std::fs::write(&path, text)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::ClrMamePro,
            source_key: PublishingSourceKey::new(format!("cmp-plan-{label}")),
            source_display_name: label.into(),
            catalog_key: CatalogKey::new(format!("cmp-plan-{label}")),
            catalog_display_name: label.into(),
            scope: CatalogScope::Complete,
        },
    )?;
    if report.status != CatalogImportStatus::Succeeded {
        return Err("plan fixture failed to publish".into());
    }
    Ok(report
        .snapshot_key
        .ok_or("missing plan fixture snapshot")?
        .as_str()
        .to_owned())
}

fn seed_registry(connection: &mut SqliteConnection, unrelated: &str) -> TestResult {
    let coverage =
        sql_query("SELECT coverage_id AS value FROM catalog_snapshots WHERE snapshot_key=?")
            .bind::<Text, _>(unrelated)
            .get_result::<IdRow>(connection)?
            .value;
    let sql = REGISTRY_SEED.replace("__COVERAGE_ID__", &coverage.to_string());
    connection.immediate_transaction(|connection| connection.batch_execute(&sql))?;
    Ok(())
}

// Genuine native owner chains in unpublished unrelated editions; guards stay enabled.
// Extra registry cardinality prevents a tiny-table plan from masking global view work.
const REGISTRY_SEED: &str = r"
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO parser_interpretations(interpretation_key,format,rules_version)
SELECT printf('cmp-plan-interpretation-%03d',n),'clrmamepro-dat','clrmamepro-declared-text-compat-v1' FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO documents(document_key) SELECT printf('cmp-plan-document-%03d',n) FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
SELECT printf('cmp-plan-snapshot-%03d',n),'cmp-plan-unrelated',printf('cmp-plan-document-%03d',n),
 printf('cmp-plan-interpretation-%03d',n),__COVERAGE_ID__ FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO catalog_set_groups(snapshot_key,kind,list_order)
SELECT printf('cmp-plan-snapshot-%03d',n),'root',0 FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO cmp_documents(snapshot_key,header_present,comment_count)
SELECT printf('cmp-plan-snapshot-%03d',n),1,1 FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO cmp_header_facts(snapshot_key,source_block,source_order,source_line,source_column,version)
SELECT printf('cmp-plan-snapshot-%03d',n),'clrmamepro',0,1,1,'1' FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO cmp_header_directives(snapshot_key) SELECT printf('cmp-plan-snapshot-%03d',n) FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO cmp_header_field_positions(snapshot_key,field_kind,source_field,source_order,is_quoted,source_line,source_column)
SELECT printf('cmp-plan-snapshot-%03d',n),2,'version',0,1,1,20 FROM numbers;
WITH RECURSIVE numbers(n) AS (VALUES(1) UNION ALL SELECT n+1 FROM numbers WHERE n<128)
INSERT INTO cmp_comments(snapshot_key,comment_order,text,source_line,source_column)
SELECT printf('cmp-plan-snapshot-%03d',n),0,'; unrelated',1,1 FROM numbers;
";
