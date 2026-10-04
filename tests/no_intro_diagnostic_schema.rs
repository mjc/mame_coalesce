use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};

#[path = "../src/storage/db/ddl.rs"]
mod bundled_ddl;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct NameRow {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct ColumnRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    #[diesel(column_name = "type")]
    column_type: String,
    #[diesel(sql_type = BigInt)]
    notnull: i64,
    #[diesel(sql_type = BigInt)]
    pk: i64,
}

#[derive(QueryableByName)]
struct IndexRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = BigInt)]
    unique: i64,
}

#[derive(QueryableByName)]
struct IndexColumnRow {
    #[diesel(sql_type = BigInt)]
    seqno: i64,
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[derive(QueryableByName)]
struct TriggerSqlRow {
    #[diesel(sql_type = Text)]
    sql: String,
}

#[derive(QueryableByName)]
struct ForeignKeyRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = BigInt)]
    seq: i64,
    #[diesel(sql_type = Text)]
    #[diesel(column_name = "table")]
    table_name: String,
    #[diesel(sql_type = Text)]
    #[diesel(column_name = "from")]
    from_column: String,
    #[diesel(sql_type = Text)]
    #[diesel(column_name = "to")]
    to_column: String,
}

fn production_schema() -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")?;
    connection.batch_execute(bundled_ddl::SCHEMA)?;
    connection.batch_execute("PRAGMA foreign_keys=ON")?;
    Ok(connection)
}

fn two_document_diagnostics() -> TestResult<SqliteConnection> {
    let mut connection = production_schema()?;
    connection.batch_execute(
        "INSERT INTO publishing_sources(source_key,display_name) VALUES('source','Source');
         INSERT INTO documents(document_key) VALUES('document-a'),('document-b');
         INSERT INTO parser_interpretations(interpretation_key,format)
             VALUES('interpretation','no-intro-database-xml-nul-compatible');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES(1,'complete');
         INSERT INTO catalogs(catalog_key,source_key,display_name)
             VALUES('catalog','source','Catalog');
         INSERT INTO catalog_snapshots(
             snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
             VALUES('snapshot-a','catalog','document-a','interpretation',1),
                   ('snapshot-b','catalog','document-b','interpretation',1);
         INSERT INTO no_intro_exports(
             snapshot_key,envelope_kind,header_present,source_line,source_column,
             document_end_line,document_end_column)
             VALUES('snapshot-a','single_datafile',0,1,2,10,1),
                   ('snapshot-b','single_datafile',0,1,2,10,1);
         INSERT INTO import_runs(
             run_key,catalog_key,document_key,interpretation_key,snapshot_key,status)
             VALUES('run-a','catalog','document-a','interpretation','snapshot-a','succeeded');
         INSERT INTO import_diagnostics(
             diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
             source_line,source_column,coordinate_view,column_convention)
             VALUES('diagnostic-a','run-a',0,'document-a','warning','xml_nul_recovered',
                    'Recovered NUL',2,1,'transport_decoded_xml_text','unicode_scalar_1based');",
    )?;
    Ok(connection)
}

const ALL_OWNER_DIAGNOSTICS_SQL: &str =
        "INSERT INTO publishing_sources(source_key,display_name) VALUES('source','Source');
         INSERT INTO documents(document_key) VALUES('document');
         INSERT INTO parser_interpretations(interpretation_key,format)
             VALUES('interpretation','no-intro-database-xml-nul-compatible');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES(1,'complete');
         INSERT INTO catalogs(catalog_key,source_key,display_name)
             VALUES('catalog','source','Catalog');
         INSERT INTO catalog_snapshots(
             snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
             VALUES('snapshot','catalog','document','interpretation',1);
         INSERT INTO import_runs(
             run_key,catalog_key,document_key,interpretation_key,snapshot_key,status)
             VALUES('run','catalog','document','interpretation','snapshot','succeeded');
         INSERT INTO no_intro_exports(
             snapshot_key,envelope_kind,header_present,source_line,source_column,
             document_end_line,document_end_column)
             VALUES('snapshot','sibling_header_datafile',1,3,1,40,1);
         INSERT INTO no_intro_export_headers(
             snapshot_key,source_line,source_column,source_end_line,source_end_column)
             VALUES('snapshot',1,2,3,1);
         INSERT INTO no_intro_header_fields(
             snapshot_key,source_order,field_kind,value,source_line,source_column,
             source_end_line,source_end_column)
             VALUES('snapshot',0,0,'Header',1,4,2,1);
         INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
             VALUES(1,'snapshot','root',0);
         INSERT INTO catalog_sets(
             set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
             VALUES(1,1,'no_intro_database_game',0,'game',3,1);
         INSERT INTO no_intro_database_games(
             set_id,name_source_order,name_source_line,name_source_column,
             source_end_line,source_end_column)
             VALUES(1,0,3,15,40,1);
         INSERT INTO no_intro_archive_descriptions(
             archive_id,set_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(30,1,0,4,1,4,10);
         INSERT INTO no_intro_dump_sources(
             dump_source_id,set_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(10,1,1,6,1,25,1);
         INSERT INTO no_intro_dump_details(
             dump_source_id,source_order,source_line,source_column,
             source_end_line,source_end_column,opening_end_line,opening_end_column)
             VALUES(10,0,7,1,9,1,7,10);
         INSERT INTO no_intro_dump_serials(
             dump_source_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(10,1,10,1,11,1);
         INSERT INTO no_intro_releases(
             release_id,set_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(20,1,2,26,1,38,1);
         INSERT INTO no_intro_release_details(
             release_id,source_order,source_line,source_column,
             source_end_line,source_end_column,opening_end_line,opening_end_column)
             VALUES(20,0,27,1,29,1,27,10);
         INSERT INTO no_intro_release_serials(
             release_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(20,1,30,1,31,1);
         INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind)
             VALUES(100,1,0,'no_intro_database_source_file'),
                   (200,1,1,'no_intro_database_release_file');
         INSERT INTO no_intro_dump_files(
             occurrence_id,dump_source_id,set_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(100,10,1,2,12,1,13,1);
         INSERT INTO no_intro_release_files(
             occurrence_id,release_id,set_id,source_order,source_line,source_column,
             source_end_line,source_end_column)
             VALUES(200,20,1,2,32,1,33,1);
         INSERT INTO import_diagnostics(
             diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
             source_line,source_column,coordinate_view,column_convention)
             VALUES
             ('d-document','run',0,'document','warning','xml_nul_recovered','Recovered NUL',1,1,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-header','run',1,'document','warning','xml_nul_recovered','Recovered NUL',1,3,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-field','run',2,'document','warning','xml_nul_recovered','Recovered NUL',1,5,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-game','run',3,'document','warning','xml_nul_recovered','Recovered NUL',3,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-archive','run',4,'document','warning','xml_nul_recovered','Recovered NUL',4,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-dump-source','run',5,'document','warning','xml_nul_recovered','Recovered NUL',7,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-dump-details','run',6,'document','warning','xml_nul_recovered','Recovered NUL',8,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-dump-serials','run',7,'document','warning','xml_nul_recovered','Recovered NUL',10,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-dump-file','run',8,'document','warning','xml_nul_recovered','Recovered NUL',12,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-release','run',9,'document','warning','xml_nul_recovered','Recovered NUL',26,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-release-details','run',10,'document','warning','xml_nul_recovered','Recovered NUL',27,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-release-serials','run',11,'document','warning','xml_nul_recovered','Recovered NUL',30,2,'transport_decoded_xml_text','unicode_scalar_1based'),
             ('d-release-file','run',12,'document','warning','xml_nul_recovered','Recovered NUL',32,2,'transport_decoded_xml_text','unicode_scalar_1based');
         INSERT INTO no_intro_export_diagnostics VALUES('d-document','run','snapshot');
         INSERT INTO no_intro_export_header_diagnostics VALUES('d-header','run','snapshot');
         INSERT INTO no_intro_header_field_diagnostics VALUES('d-field','run','snapshot',0);
         INSERT INTO no_intro_game_diagnostics VALUES('d-game','run','snapshot',1);
         INSERT INTO no_intro_archive_diagnostics VALUES('d-archive','run','snapshot',30);
         INSERT INTO no_intro_dump_source_diagnostics VALUES('d-dump-source','run','snapshot',10);
         INSERT INTO no_intro_dump_details_diagnostics VALUES('d-dump-details','run','snapshot',10);
         INSERT INTO no_intro_dump_serials_diagnostics VALUES('d-dump-serials','run','snapshot',10);
         INSERT INTO no_intro_dump_file_diagnostics VALUES('d-dump-file','run','snapshot',100);
         INSERT INTO no_intro_release_diagnostics VALUES('d-release','run','snapshot',20);
         INSERT INTO no_intro_release_details_diagnostics VALUES('d-release-details','run','snapshot',20);
         INSERT INTO no_intro_release_serials_diagnostics VALUES('d-release-serials','run','snapshot',20);
         INSERT INTO no_intro_release_file_diagnostics VALUES('d-release-file','run','snapshot',200);";

fn all_owner_diagnostics() -> TestResult<SqliteConnection> {
    let mut connection = production_schema()?;
    connection.batch_execute(ALL_OWNER_DIAGNOSTICS_SQL)?;
    Ok(connection)
}

fn add_recovery_diagnostic(
    connection: &mut SqliteConnection,
    diagnostic_key: &str,
    order: i64,
    line: i64,
    column: i64,
) -> TestResult {
    sql_query(
        "INSERT INTO import_diagnostics(
             diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
             source_line,source_column,coordinate_view,column_convention)
         VALUES(?,'run',?,'document','warning','xml_nul_recovered','Recovered NUL',?,?,'transport_decoded_xml_text','unicode_scalar_1based')",
    )
    .bind::<Text, _>(diagnostic_key)
    .bind::<BigInt, _>(order)
    .bind::<BigInt, _>(line)
    .bind::<BigInt, _>(column)
    .execute(connection)?;
    Ok(())
}

fn assert_sql_rejected(connection: &mut SqliteConnection, sql: &str, reason: &str) {
    assert!(connection.batch_execute(sql).is_err(), "{reason}");
}

fn explain(connection: &mut SqliteConnection, query: &str) -> TestResult<Vec<String>> {
    let plan: Vec<PlanRow> = sql_query(format!("EXPLAIN QUERY PLAN {query}")).load(connection)?;
    Ok(plan.into_iter().map(|row| row.detail).collect())
}

fn trigger_sql(connection: &mut SqliteConnection, name: &str) -> TestResult<String> {
    let row: TriggerSqlRow =
        sql_query("SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?")
            .bind::<Text, _>(name)
            .get_result(connection)?;
    Ok(row.sql)
}

fn native_diagnostic_relations(connection: &mut SqliteConnection) -> TestResult<Vec<String>> {
    let rows: Vec<NameRow> = sql_query(
        "SELECT name FROM sqlite_schema WHERE type='table' \
         AND name GLOB 'no_intro_*_diagnostics' ORDER BY name",
    )
    .load(connection)?;
    assert_eq!(rows.len(), 13);
    Ok(rows.into_iter().map(|row| row.name).collect())
}

fn populate_run_guard_evidence(connection: &mut SqliteConnection) -> TestResult<Vec<String>> {
    let relations = native_diagnostic_relations(connection)?;
    for run_number in 0..32 {
        let run_key = format!("plan-run-{run_number}");
        sql_query(
            "INSERT INTO import_runs( \
                 run_key,catalog_key,document_key,interpretation_key,snapshot_key,status) \
             VALUES(?,'catalog','document','interpretation','snapshot','succeeded')",
        )
        .bind::<Text, _>(&run_key)
        .execute(connection)?;
        for (order, relation) in relations.iter().enumerate() {
            let diagnostic_key = format!("plan-diagnostic-{run_number}-{order}");
            sql_query(format!(
                "INSERT INTO import_diagnostics( \
                     diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message, \
                     source_line,source_column,coordinate_view,column_convention) \
                 SELECT ?,?,?,document_key,severity,code,message, \
                        source_line,source_column,coordinate_view,column_convention \
                 FROM import_diagnostics WHERE diagnostic_key=( \
                     SELECT diagnostic_key FROM {relation} WHERE run_key='run' LIMIT 1)"
            ))
            .bind::<Text, _>(&diagnostic_key)
            .bind::<Text, _>(&run_key)
            .bind::<BigInt, _>(i64::try_from(order)?)
            .execute(connection)?;

            let owner_columns = columns(connection, relation)?
                .into_iter()
                .filter(|column| column.name != "diagnostic_key" && column.name != "run_key")
                .map(|column| column.name)
                .collect::<Vec<_>>()
                .join(",");
            sql_query(format!(
                "INSERT INTO {relation}(diagnostic_key,run_key,{owner_columns}) \
                 SELECT ?,?,{owner_columns} FROM {relation} WHERE run_key='run' LIMIT 1"
            ))
            .bind::<Text, _>(&diagnostic_key)
            .bind::<Text, _>(&run_key)
            .execute(connection)?;
        }
    }
    for relation in &relations {
        let count: CountRow = sql_query(format!("SELECT COUNT(*) AS count FROM {relation}"))
            .get_result(connection)?;
        assert_eq!(
            count.count, 33,
            "populated trigger-plan fixture: {relation}"
        );
    }
    connection.batch_execute("ANALYZE")?;
    Ok(relations)
}

fn run_trigger_predicate_query(
    connection: &mut SqliteConnection,
    name: &str,
) -> TestResult<String> {
    let sql = trigger_sql(connection, name)?;
    let (_, after_when) = sql
        .split_once("\nWHEN ")
        .ok_or_else(|| std::io::Error::other(format!("{name} has no WHEN predicate")))?;
    let (predicate, _) = after_when
        .split_once("\nBEGIN ")
        .ok_or_else(|| std::io::Error::other(format!("{name} has no trigger body")))?;
    let predicate = predicate
        .replace("OLD.run_key", "(SELECT old_run_key FROM requested)")
        .replace("NEW.run_key", "(SELECT new_run_key FROM requested)");
    Ok(format!(
        "WITH requested(old_run_key,new_run_key) AS (VALUES (?,?)) \
         SELECT ({predicate}) AS count"
    ))
}

fn table_exists(connection: &mut SqliteConnection, table: &str) -> TestResult<bool> {
    let found: Vec<NameRow> =
        sql_query("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
            .bind::<Text, _>(table)
            .load(connection)?;
    Ok(found.first().is_some_and(|row| row.name == table))
}

fn columns(connection: &mut SqliteConnection, table: &str) -> TestResult<Vec<ColumnRow>> {
    Ok(sql_query(format!("PRAGMA table_info({table})")).load(connection)?)
}

fn has_unique_columns(
    connection: &mut SqliteConnection,
    table: &str,
    expected: &[&str],
) -> TestResult<bool> {
    let indexes: Vec<IndexRow> =
        sql_query(format!("PRAGMA index_list({table})")).load(connection)?;
    for index in indexes.into_iter().filter(|index| index.unique == 1) {
        let mut actual: Vec<IndexColumnRow> =
            sql_query(format!("PRAGMA index_info({})", index.name)).load(connection)?;
        actual.sort_by_key(|column| column.seqno);
        if actual
            .into_iter()
            .map(|column| column.name)
            .eq(expected.iter().map(|column| (*column).to_owned()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn assert_typed_link_foreign_keys(
    connection: &mut SqliteConnection,
    relation: &str,
    native_from: &str,
    native_table: &str,
    native_to: &str,
) -> TestResult {
    let foreign_keys: Vec<ForeignKeyRow> =
        sql_query(format!("PRAGMA foreign_key_list({relation})")).load(connection)?;
    let mut groups = std::collections::BTreeMap::<i64, Vec<(i64, String, String, String)>>::new();
    for key in foreign_keys {
        groups.entry(key.id).or_default().push((
            key.seq,
            key.table_name,
            key.from_column,
            key.to_column,
        ));
    }
    let mut normalized = groups
        .into_values()
        .map(|mut columns| {
            columns.sort_by_key(|column| column.0);
            columns
        })
        .collect::<Vec<_>>();
    normalized.sort();
    let mut expected = vec![
        vec![
            (
                0,
                "import_diagnostics".to_owned(),
                "diagnostic_key".to_owned(),
                "diagnostic_key".to_owned(),
            ),
            (
                1,
                "import_diagnostics".to_owned(),
                "run_key".to_owned(),
                "run_key".to_owned(),
            ),
        ],
        vec![
            (
                0,
                "import_runs".to_owned(),
                "run_key".to_owned(),
                "run_key".to_owned(),
            ),
            (
                1,
                "import_runs".to_owned(),
                "snapshot_key".to_owned(),
                "snapshot_key".to_owned(),
            ),
        ],
        native_from
            .split(',')
            .zip(native_to.split(','))
            .enumerate()
            .map(|(seq, (from, to))| {
                Ok((
                    i64::try_from(seq)?,
                    native_table.to_owned(),
                    from.to_owned(),
                    to.to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, std::num::TryFromIntError>>()?,
    ];
    expected.sort();
    assert_eq!(normalized, expected, "foreign keys on {relation}");

    let mut primary_key = columns(connection, relation)?
        .into_iter()
        .filter(|column| column.pk > 0)
        .collect::<Vec<_>>();
    primary_key.sort_by_key(|column| column.pk);
    let actual = primary_key
        .into_iter()
        .map(|column| column.name)
        .collect::<Vec<_>>();
    let mut expected_pk = vec!["diagnostic_key"];
    expected_pk.extend(native_from.split(','));
    assert_eq!(actual, expected_pk, "composite primary key on {relation}");
    Ok(())
}

#[test]
fn fresh_schema_has_ordered_diagnostics_and_thirteen_typed_owner_relations() -> TestResult {
    let mut connection = production_schema()?;
    let relations = [
        "no_intro_export_diagnostics",
        "no_intro_export_header_diagnostics",
        "no_intro_header_field_diagnostics",
        "no_intro_game_diagnostics",
        "no_intro_archive_diagnostics",
        "no_intro_dump_source_diagnostics",
        "no_intro_dump_details_diagnostics",
        "no_intro_dump_serials_diagnostics",
        "no_intro_dump_file_diagnostics",
        "no_intro_release_diagnostics",
        "no_intro_release_details_diagnostics",
        "no_intro_release_serials_diagnostics",
        "no_intro_release_file_diagnostics",
    ];

    for relation in relations {
        assert!(
            table_exists(&mut connection, relation)?,
            "fresh production DDL is missing typed diagnostic relation {relation}"
        );
    }

    let diagnostic_columns = columns(&mut connection, "import_diagnostics")?;
    let Some(diagnostic_order) = diagnostic_columns
        .iter()
        .find(|column| column.name == "diagnostic_order")
    else {
        return Err(
            std::io::Error::other("import_diagnostics.diagnostic_order is required").into(),
        );
    };
    assert_eq!(diagnostic_order.column_type, "INTEGER");
    assert_eq!(diagnostic_order.notnull, 1);
    assert!(has_unique_columns(
        &mut connection,
        "import_diagnostics",
        &["run_key", "diagnostic_order"]
    )?);

    let run_columns = columns(&mut connection, "import_runs")?;
    assert!(
        !run_columns.iter().any(|column| column.name == "diagnostic"),
        "failure text must have one owner in import_diagnostics"
    );
    assert!(has_unique_columns(
        &mut connection,
        "import_runs",
        &["run_key", "snapshot_key"]
    )?);
    Ok(())
}

#[test]
fn fresh_schema_stores_document_and_all_twelve_native_owner_ends() -> TestResult {
    let mut connection = production_schema()?;
    let owners = [
        "no_intro_export_headers",
        "no_intro_header_fields",
        "no_intro_database_games",
        "no_intro_archive_descriptions",
        "no_intro_dump_sources",
        "no_intro_dump_details",
        "no_intro_dump_serials",
        "no_intro_dump_files",
        "no_intro_releases",
        "no_intro_release_details",
        "no_intro_release_serials",
        "no_intro_release_files",
    ];

    for owner in owners {
        let owner_columns = columns(&mut connection, owner)?;
        for column_name in ["source_end_line", "source_end_column"] {
            let Some(column) = owner_columns
                .iter()
                .find(|column| column.name == column_name)
            else {
                return Err(std::io::Error::other(format!(
                    "{owner}.{column_name} must persist its source extent"
                ))
                .into());
            };
            assert_eq!(column.column_type, "INTEGER", "{owner}.{column_name}");
            assert_eq!(column.notnull, 1, "{owner}.{column_name}");
        }
    }

    let document_columns = columns(&mut connection, "no_intro_exports")?;
    for column_name in ["document_end_line", "document_end_column"] {
        let Some(column) = document_columns
            .iter()
            .find(|column| column.name == column_name)
        else {
            return Err(std::io::Error::other(format!(
                "no_intro_exports.{column_name} is required"
            ))
            .into());
        };
        assert_eq!(column.column_type, "INTEGER");
        assert_eq!(column.notnull, 1);
    }
    Ok(())
}

#[test]
fn every_typed_link_has_complete_diagnostic_run_snapshot_and_native_foreign_keys() -> TestResult {
    let mut connection = production_schema()?;
    let relations = [
        (
            "no_intro_export_diagnostics",
            "snapshot_key",
            "no_intro_exports",
            "snapshot_key",
        ),
        (
            "no_intro_export_header_diagnostics",
            "snapshot_key",
            "no_intro_export_headers",
            "snapshot_key",
        ),
        (
            "no_intro_header_field_diagnostics",
            "snapshot_key,source_order",
            "no_intro_header_fields",
            "snapshot_key,source_order",
        ),
        (
            "no_intro_game_diagnostics",
            "set_id",
            "no_intro_database_games",
            "set_id",
        ),
        (
            "no_intro_archive_diagnostics",
            "archive_id",
            "no_intro_archive_descriptions",
            "archive_id",
        ),
        (
            "no_intro_dump_source_diagnostics",
            "dump_source_id",
            "no_intro_dump_sources",
            "dump_source_id",
        ),
        (
            "no_intro_dump_details_diagnostics",
            "dump_source_id",
            "no_intro_dump_details",
            "dump_source_id",
        ),
        (
            "no_intro_dump_serials_diagnostics",
            "dump_source_id",
            "no_intro_dump_serials",
            "dump_source_id",
        ),
        (
            "no_intro_dump_file_diagnostics",
            "occurrence_id",
            "no_intro_dump_files",
            "occurrence_id",
        ),
        (
            "no_intro_release_diagnostics",
            "release_id",
            "no_intro_releases",
            "release_id",
        ),
        (
            "no_intro_release_details_diagnostics",
            "release_id",
            "no_intro_release_details",
            "release_id",
        ),
        (
            "no_intro_release_serials_diagnostics",
            "release_id",
            "no_intro_release_serials",
            "release_id",
        ),
        (
            "no_intro_release_file_diagnostics",
            "occurrence_id",
            "no_intro_release_files",
            "occurrence_id",
        ),
    ];

    for (relation, native_from, native_table, native_to) in relations {
        assert_typed_link_foreign_keys(
            &mut connection,
            relation,
            native_from,
            native_table,
            native_to,
        )?;
    }
    Ok(())
}

#[test]
fn direct_sql_rejects_a_cross_snapshot_document_link_with_foreign_keys_off() -> TestResult {
    let mut connection = two_document_diagnostics()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;

    let result = sql_query(
        "INSERT INTO no_intro_export_diagnostics(diagnostic_key,run_key,snapshot_key)
         VALUES('diagnostic-a','run-a','snapshot-b')",
    )
    .execute(&mut connection);
    assert!(
        result.is_err(),
        "a diagnostic linked to another document snapshot was accepted with FK checks disabled"
    );
    Ok(())
}

#[test]
fn direct_sql_rejects_a_document_location_at_its_exclusive_end() -> TestResult {
    let mut connection = two_document_diagnostics()?;
    connection.batch_execute("UPDATE import_diagnostics SET source_line=10,source_column=1")?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;

    let result = sql_query(
        "INSERT INTO no_intro_export_diagnostics(diagnostic_key,run_key,snapshot_key)
         VALUES('diagnostic-a','run-a','snapshot-a')",
    )
    .execute(&mut connection);
    assert!(
        result.is_err(),
        "the half-open document extent accepted a diagnostic at its exclusive end"
    );
    Ok(())
}

#[test]
fn all_thirteen_typed_relations_accept_valid_extents_and_nested_multi_owner_links() -> TestResult {
    let mut connection = all_owner_diagnostics()?;
    sql_query(
        "INSERT INTO no_intro_dump_details_diagnostics
             (diagnostic_key,run_key,snapshot_key,dump_source_id)
         VALUES('d-dump-source','run','snapshot',10)",
    )
    .execute(&mut connection)?;

    let relations = [
        ("no_intro_export_diagnostics", 1),
        ("no_intro_export_header_diagnostics", 1),
        ("no_intro_header_field_diagnostics", 1),
        ("no_intro_game_diagnostics", 1),
        ("no_intro_archive_diagnostics", 1),
        ("no_intro_dump_source_diagnostics", 1),
        ("no_intro_dump_details_diagnostics", 2),
        ("no_intro_dump_serials_diagnostics", 1),
        ("no_intro_dump_file_diagnostics", 1),
        ("no_intro_release_diagnostics", 1),
        ("no_intro_release_details_diagnostics", 1),
        ("no_intro_release_serials_diagnostics", 1),
        ("no_intro_release_file_diagnostics", 1),
    ];
    for (relation, expected) in relations {
        let count: CountRow = sql_query(format!("SELECT COUNT(*) AS count FROM {relation}"))
            .get_result(&mut connection)?;
        assert_eq!(count.count, expected, "{relation}");
    }
    Ok(())
}

#[test]
fn direct_sql_guards_wrong_run_snapshot_native_class_and_extent_with_foreign_keys_off() -> TestResult
{
    let mut connection = all_owner_diagnostics()?;
    add_recovery_diagnostic(&mut connection, "d-wrong-run", 13, 4, 2)?;
    add_recovery_diagnostic(&mut connection, "d-wrong-snapshot", 14, 4, 2)?;
    add_recovery_diagnostic(&mut connection, "d-wrong-class", 15, 32, 2)?;
    add_recovery_diagnostic(&mut connection, "d-outside-owner", 16, 4, 10)?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;

    for (sql, reason) in [
        (
            "INSERT INTO no_intro_archive_diagnostics VALUES('d-wrong-run','wrong-run','snapshot',30)",
            "wrong diagnostic/run pair bypassed the FK",
        ),
        (
            "INSERT INTO no_intro_archive_diagnostics VALUES('d-wrong-snapshot','run','other-snapshot',30)",
            "wrong snapshot bypassed the run/snapshot FK",
        ),
        (
            "INSERT INTO no_intro_dump_file_diagnostics VALUES('d-wrong-class','run','snapshot',200)",
            "release-file occurrence was accepted as a dump-file owner",
        ),
        (
            "INSERT INTO no_intro_archive_diagnostics VALUES('d-outside-owner','run','snapshot',30)",
            "diagnostic at the archive's exclusive end was accepted",
        ),
    ] {
        assert_sql_rejected(&mut connection, sql, reason);
    }
    Ok(())
}

fn reject_invalid_typed_link(
    connection: &mut SqliteConnection,
    index: usize,
    owner: TypedLinkOwnerExtent,
) -> TestResult {
    let TypedLinkOwnerExtent {
        relation,
        diagnostic_key,
        owner_column,
        owner_key,
        end_line,
        end_column,
    } = owner;
    let owner_column = if owner_column == "snapshot_key" {
        String::new()
    } else {
        format!(",{owner_column}")
    };
    let owner_key = if owner_key == "snapshot" {
        String::new()
    } else {
        format!(",{owner_key}")
    };
    let wrong_run_key = format!("wrong-run-{index}");
    let wrong_snapshot_key = format!("wrong-snapshot-{index}");
    let wrong_run_diagnostic = format!("d-wrong-run-{index}");
    let wrong_snapshot_diagnostic = format!("d-wrong-snapshot-{index}");
    let at_end_key = format!("d-at-end-{index}");
    let first_order = 13 + 3 * i64::try_from(index)?;
    for (key, offset) in [(&wrong_run_diagnostic, 0), (&wrong_snapshot_diagnostic, 1)] {
        assert_eq!(
            sql_query(
                "INSERT INTO import_diagnostics \
                 (diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,\
                  source_line,source_column,coordinate_view,column_convention) \
                 SELECT ?,run_key,?,document_key,severity,code,message,\
                  source_line,source_column,coordinate_view,column_convention \
                 FROM import_diagnostics WHERE diagnostic_key=?",
            )
            .bind::<Text, _>(key)
            .bind::<BigInt, _>(first_order + offset)
            .bind::<Text, _>(diagnostic_key)
            .execute(connection)?,
            1,
            "fresh provenance witness must clone its actual owner's diagnostic"
        );
    }
    add_recovery_diagnostic(
        connection,
        &at_end_key,
        first_order + 2,
        end_line,
        end_column,
    )?;

    assert_sql_rejected(
        connection,
        &format!(
            "INSERT INTO {relation}(diagnostic_key,run_key,snapshot_key{owner_column}) \
             SELECT '{wrong_run_diagnostic}','{wrong_run_key}',snapshot_key{owner_key} \
             FROM {relation} WHERE diagnostic_key='{diagnostic_key}'"
        ),
        &format!("{relation} accepted a diagnostic linked to a different run"),
    );
    assert_sql_rejected(
        connection,
        &format!(
            "INSERT INTO {relation}(diagnostic_key,run_key,snapshot_key{owner_column}) \
             SELECT '{wrong_snapshot_diagnostic}',run_key,'{wrong_snapshot_key}'{owner_key} \
             FROM {relation} WHERE diagnostic_key='{diagnostic_key}'"
        ),
        &format!("{relation} accepted a diagnostic linked to a different snapshot"),
    );
    assert_sql_rejected(
        connection,
        &format!(
            "INSERT INTO {relation}(diagnostic_key,run_key,snapshot_key{owner_column}) \
             SELECT diagnostic_key,run_key,'snapshot'{owner_key} \
             FROM import_diagnostics WHERE diagnostic_key='{at_end_key}'"
        ),
        &format!("{relation} accepted a diagnostic at its exclusive end"),
    );
    Ok(())
}

#[derive(Clone, Copy)]
struct TypedLinkOwnerExtent {
    relation: &'static str,
    diagnostic_key: &'static str,
    owner_column: &'static str,
    owner_key: &'static str,
    end_line: i64,
    end_column: i64,
}

const TYPED_LINK_OWNER_EXTENTS: [TypedLinkOwnerExtent; 13] = [
    TypedLinkOwnerExtent {
        relation: "no_intro_export_diagnostics",
        diagnostic_key: "d-document",
        owner_column: "snapshot_key",
        owner_key: "snapshot",
        end_line: 40,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_export_header_diagnostics",
        diagnostic_key: "d-header",
        owner_column: "snapshot_key",
        owner_key: "snapshot",
        end_line: 3,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_header_field_diagnostics",
        diagnostic_key: "d-field",
        owner_column: "source_order",
        owner_key: "0",
        end_line: 2,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_game_diagnostics",
        diagnostic_key: "d-game",
        owner_column: "set_id",
        owner_key: "1",
        end_line: 40,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_archive_diagnostics",
        diagnostic_key: "d-archive",
        owner_column: "archive_id",
        owner_key: "30",
        end_line: 4,
        end_column: 10,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_dump_source_diagnostics",
        diagnostic_key: "d-dump-source",
        owner_column: "dump_source_id",
        owner_key: "10",
        end_line: 25,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_dump_details_diagnostics",
        diagnostic_key: "d-dump-details",
        owner_column: "dump_source_id",
        owner_key: "10",
        end_line: 9,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_dump_serials_diagnostics",
        diagnostic_key: "d-dump-serials",
        owner_column: "dump_source_id",
        owner_key: "10",
        end_line: 11,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_dump_file_diagnostics",
        diagnostic_key: "d-dump-file",
        owner_column: "occurrence_id",
        owner_key: "100",
        end_line: 13,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_release_diagnostics",
        diagnostic_key: "d-release",
        owner_column: "release_id",
        owner_key: "20",
        end_line: 38,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_release_details_diagnostics",
        diagnostic_key: "d-release-details",
        owner_column: "release_id",
        owner_key: "20",
        end_line: 29,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_release_serials_diagnostics",
        diagnostic_key: "d-release-serials",
        owner_column: "release_id",
        owner_key: "20",
        end_line: 31,
        end_column: 1,
    },
    TypedLinkOwnerExtent {
        relation: "no_intro_release_file_diagnostics",
        diagnostic_key: "d-release-file",
        owner_column: "occurrence_id",
        owner_key: "200",
        end_line: 33,
        end_column: 1,
    },
];

#[test]
fn every_typed_link_rejects_wrong_run_snapshot_and_exclusive_end_with_foreign_keys_off()
-> TestResult {
    let mut connection = all_owner_diagnostics()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF")?;

    for (index, owner) in TYPED_LINK_OWNER_EXTENTS.into_iter().enumerate() {
        reject_invalid_typed_link(&mut connection, index, owner)?;
    }
    Ok(())
}

#[test]
fn diagnostic_insert_guard_checks_text_storage_and_nonnegative_order() -> TestResult {
    let mut connection = all_owner_diagnostics()?;
    for (sql, reason) in [
        (
            "INSERT INTO import_diagnostics(
                 diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
                 source_line,source_column,coordinate_view,column_convention)
             VALUES('d-code-blob','run',13,'document','warning',x'786d6c5f6e756c5f7265636f7665726564','Recovered NUL',4,2,'transport_decoded_xml_text','unicode_scalar_1based')",
            "BLOB diagnostic code was accepted",
        ),
        (
            "INSERT INTO import_diagnostics(
                 diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
                 source_line,source_column,coordinate_view,column_convention)
             VALUES('d-message-blob','run',13,'document','warning','xml_nul_recovered',x'5265636f7665726564204e554c',4,2,'transport_decoded_xml_text','unicode_scalar_1based')",
            "BLOB diagnostic message was accepted",
        ),
        (
            "INSERT INTO import_diagnostics(
                 diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
                 source_line,source_column,coordinate_view,column_convention)
             VALUES('d-negative-order','run',-1,'document','warning','xml_nul_recovered','Recovered NUL',4,2,'transport_decoded_xml_text','unicode_scalar_1based')",
            "negative diagnostic order was accepted",
        ),
    ] {
        assert_sql_rejected(&mut connection, sql, reason);
    }
    Ok(())
}

#[test]
fn diagnostic_and_native_extents_reject_noninteger_and_reversed_boundaries() -> TestResult {
    let mut connection = all_owner_diagnostics()?;
    for (sql, reason) in [
        (
            "INSERT INTO no_intro_archive_descriptions(
                 archive_id,set_id,source_order,source_line,source_column,
                 source_end_line,source_end_column)
             VALUES(31,1,3,4,1,x'35',10)",
            "BLOB extent endpoint was accepted",
        ),
        (
            "INSERT INTO no_intro_dump_sources(
                 dump_source_id,set_id,source_order,source_line,source_column,
                 source_end_line,source_end_column)
             VALUES(11,1,3,20,1,20,1)",
            "zero-length nonempty native element extent was accepted",
        ),
        (
            "INSERT INTO no_intro_dump_details(
                 dump_source_id,source_order,source_line,source_column,
                 source_end_line,source_end_column,opening_end_line,opening_end_column)
             VALUES(10,3,18,1,19,1,20,1)",
            "details opening-end boundary after the final end was accepted",
        ),
    ] {
        assert_sql_rejected(&mut connection, sql, reason);
    }

    connection.batch_execute(
        "INSERT INTO catalog_sets(
             set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
         VALUES(2,1,'no_intro_database_game',1,'later-game',4,1)",
    )?;
    assert_sql_rejected(
        &mut connection,
        "INSERT INTO no_intro_database_games(
             set_id,name_source_order,name_source_line,name_source_column,
             source_end_line,source_end_column)
         VALUES(2,0,4,10,4,1)",
        "game extent ending at its inherited catalog-set start was accepted",
    );
    Ok(())
}

#[test]
fn linked_diagnostics_runs_and_native_rows_reject_update_delete_and_replace_with_triggers_off()
-> TestResult {
    let mut connection = all_owner_diagnostics()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    for (sql, reason) in [
        (
            "UPDATE no_intro_dump_details_diagnostics SET run_key='rewritten' WHERE diagnostic_key='d-dump-details'",
            "native owner link update was accepted",
        ),
        (
            "DELETE FROM no_intro_dump_details_diagnostics WHERE diagnostic_key='d-dump-details'",
            "native owner link delete was accepted",
        ),
        (
            "INSERT OR REPLACE INTO no_intro_dump_details_diagnostics VALUES('d-dump-details','run','snapshot',10)",
            "REPLACE removed an existing native owner link",
        ),
        (
            "INSERT OR REPLACE INTO no_intro_dump_details(
                 dump_source_id,source_order,source_line,source_column,
                 source_end_line,source_end_column,opening_end_line,opening_end_column,comment1)
             VALUES(10,0,7,1,9,1,7,10,'replaced')",
            "REPLACE removed an attached native details row",
        ),
        (
            "UPDATE import_diagnostics SET message='rewritten' WHERE diagnostic_key='d-dump-details'",
            "linked diagnostic update was accepted",
        ),
        (
            "DELETE FROM import_diagnostics WHERE diagnostic_key='d-dump-details'",
            "linked diagnostic delete was accepted",
        ),
        (
            "INSERT OR REPLACE INTO import_diagnostics(
                 diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,
                 source_line,source_column,coordinate_view,column_convention)
             VALUES('replacement','run',6,'document','warning','xml_nul_recovered','replacement',8,2,'transport_decoded_xml_text','unicode_scalar_1based')",
            "REPLACE erased attached evidence through a run/order collision",
        ),
        (
            "UPDATE import_runs SET status='failed' WHERE run_key='run'",
            "linked import-run update was accepted",
        ),
        (
            "DELETE FROM import_runs WHERE run_key='run'",
            "linked import-run delete was accepted",
        ),
        (
            "INSERT OR REPLACE INTO import_runs(
                 run_key,catalog_key,document_key,interpretation_key,snapshot_key,status)
             VALUES('run','catalog','document','interpretation','snapshot','succeeded')",
            "REPLACE erased attached import-run evidence",
        ),
        (
            "UPDATE no_intro_dump_details SET comment1='rewritten' WHERE dump_source_id=10",
            "linked native owner update was accepted",
        ),
        (
            "DELETE FROM no_intro_dump_details WHERE dump_source_id=10",
            "linked native owner delete was accepted",
        ),
    ] {
        assert_sql_rejected(&mut connection, sql, reason);
    }
    Ok(())
}

#[test]
fn every_typed_link_rejects_update_delete_and_replace_with_foreign_keys_and_recursion_off()
-> TestResult {
    let mut connection = all_owner_diagnostics()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let links = [
        (
            "no_intro_export_diagnostics",
            "d-document",
            "INSERT OR REPLACE INTO no_intro_export_diagnostics VALUES('d-document','run','snapshot')",
        ),
        (
            "no_intro_export_header_diagnostics",
            "d-header",
            "INSERT OR REPLACE INTO no_intro_export_header_diagnostics VALUES('d-header','run','snapshot')",
        ),
        (
            "no_intro_header_field_diagnostics",
            "d-field",
            "INSERT OR REPLACE INTO no_intro_header_field_diagnostics VALUES('d-field','run','snapshot',0)",
        ),
        (
            "no_intro_game_diagnostics",
            "d-game",
            "INSERT OR REPLACE INTO no_intro_game_diagnostics VALUES('d-game','run','snapshot',1)",
        ),
        (
            "no_intro_archive_diagnostics",
            "d-archive",
            "INSERT OR REPLACE INTO no_intro_archive_diagnostics VALUES('d-archive','run','snapshot',30)",
        ),
        (
            "no_intro_dump_source_diagnostics",
            "d-dump-source",
            "INSERT OR REPLACE INTO no_intro_dump_source_diagnostics VALUES('d-dump-source','run','snapshot',10)",
        ),
        (
            "no_intro_dump_details_diagnostics",
            "d-dump-details",
            "INSERT OR REPLACE INTO no_intro_dump_details_diagnostics VALUES('d-dump-details','run','snapshot',10)",
        ),
        (
            "no_intro_dump_serials_diagnostics",
            "d-dump-serials",
            "INSERT OR REPLACE INTO no_intro_dump_serials_diagnostics VALUES('d-dump-serials','run','snapshot',10)",
        ),
        (
            "no_intro_dump_file_diagnostics",
            "d-dump-file",
            "INSERT OR REPLACE INTO no_intro_dump_file_diagnostics VALUES('d-dump-file','run','snapshot',100)",
        ),
        (
            "no_intro_release_diagnostics",
            "d-release",
            "INSERT OR REPLACE INTO no_intro_release_diagnostics VALUES('d-release','run','snapshot',20)",
        ),
        (
            "no_intro_release_details_diagnostics",
            "d-release-details",
            "INSERT OR REPLACE INTO no_intro_release_details_diagnostics VALUES('d-release-details','run','snapshot',20)",
        ),
        (
            "no_intro_release_serials_diagnostics",
            "d-release-serials",
            "INSERT OR REPLACE INTO no_intro_release_serials_diagnostics VALUES('d-release-serials','run','snapshot',20)",
        ),
        (
            "no_intro_release_file_diagnostics",
            "d-release-file",
            "INSERT OR REPLACE INTO no_intro_release_file_diagnostics VALUES('d-release-file','run','snapshot',200)",
        ),
    ];
    for (relation, diagnostic_key, replacement) in links {
        assert_sql_rejected(
            &mut connection,
            &format!(
                "UPDATE {relation} SET run_key='rewritten' WHERE diagnostic_key='{diagnostic_key}'"
            ),
            &format!("UPDATE changed attached evidence in {relation}"),
        );
        assert_sql_rejected(
            &mut connection,
            &format!("DELETE FROM {relation} WHERE diagnostic_key='{diagnostic_key}'"),
            &format!("DELETE removed attached evidence in {relation}"),
        );
        assert_sql_rejected(
            &mut connection,
            replacement,
            &format!("INSERT OR REPLACE rewrote attached evidence in {relation}"),
        );
    }
    Ok(())
}

#[test]
fn populated_schema_indexes_support_run_order_and_typed_link_seeks() -> TestResult {
    let mut connection = all_owner_diagnostics()?;
    populate_index_evidence(&mut connection)?;
    assert_run_order_index(&mut connection)?;
    assert_typed_owner_seek_indexes(&mut connection)?;
    Ok(())
}

fn populate_index_evidence(connection: &mut SqliteConnection) -> TestResult {
    for run_number in 0..40 {
        let run_key = format!("unrelated-run-{run_number}");
        sql_query(
            "INSERT INTO import_runs(
                 run_key,catalog_key,document_key,interpretation_key,status)
             VALUES(?,'catalog','document','interpretation','failed')",
        )
        .bind::<Text, _>(&run_key)
        .execute(connection)?;
        for diagnostic_order in 0..20 {
            let diagnostic_key = format!("unrelated-{run_number}-{diagnostic_order}");
            sql_query(
                "INSERT INTO import_diagnostics(
                     diagnostic_key,run_key,diagnostic_order,document_key,
                     severity,code,message)
                 VALUES(?,?,?,'document','error','import_failed','Failed import')",
            )
            .bind::<Text, _>(&diagnostic_key)
            .bind::<Text, _>(&run_key)
            .bind::<BigInt, _>(diagnostic_order)
            .execute(connection)?;
        }
    }
    Ok(())
}

fn assert_run_order_index(connection: &mut SqliteConnection) -> TestResult {
    let page_plan = explain(
        connection,
        "SELECT diagnostic_key FROM import_diagnostics \
         WHERE run_key='run' AND diagnostic_order>=0 \
         ORDER BY diagnostic_order LIMIT 10",
    )?
    .join("\n");
    assert!(
        page_plan.contains("SEARCH import_diagnostics USING INDEX")
            && page_plan.contains("run_key")
            && page_plan.contains("diagnostic_order"),
        "schema must support run/order seeks with the declared unique index; plan was:\n{page_plan}"
    );
    Ok(())
}

fn assert_typed_owner_seek_indexes(connection: &mut SqliteConnection) -> TestResult {
    let owners = [
        ("no_intro_export_diagnostics", "d-document", "snapshot_key"),
        (
            "no_intro_export_header_diagnostics",
            "d-header",
            "snapshot_key",
        ),
        (
            "no_intro_header_field_diagnostics",
            "d-field",
            "source_order",
        ),
        ("no_intro_game_diagnostics", "d-game", "set_id"),
        ("no_intro_archive_diagnostics", "d-archive", "archive_id"),
        (
            "no_intro_dump_source_diagnostics",
            "d-dump-source",
            "dump_source_id",
        ),
        (
            "no_intro_dump_details_diagnostics",
            "d-dump-details",
            "dump_source_id",
        ),
        (
            "no_intro_dump_serials_diagnostics",
            "d-dump-serials",
            "dump_source_id",
        ),
        (
            "no_intro_dump_file_diagnostics",
            "d-dump-file",
            "occurrence_id",
        ),
        ("no_intro_release_diagnostics", "d-release", "release_id"),
        (
            "no_intro_release_details_diagnostics",
            "d-release-details",
            "release_id",
        ),
        (
            "no_intro_release_serials_diagnostics",
            "d-release-serials",
            "release_id",
        ),
        (
            "no_intro_release_file_diagnostics",
            "d-release-file",
            "occurrence_id",
        ),
    ];
    for (relation, diagnostic_key, owner_column) in owners {
        let plan = explain(
            connection,
            &format!(
                "SELECT {owner_column} FROM {relation} WHERE diagnostic_key='{diagnostic_key}'"
            ),
        )?
        .join("\n");
        assert!(
            plan.contains(&format!("SEARCH {relation} USING PRIMARY KEY")),
            "schema must support diagnostic-key lookup through {relation}'s primary key; plan was:\n{plan}"
        );
    }
    Ok(())
}

#[test]
fn actual_run_guard_predicates_seek_every_native_relation_for_fresh_and_linked_keys() -> TestResult
{
    let mut connection = all_owner_diagnostics()?;
    let relations = populate_run_guard_evidence(&mut connection)?;
    let fresh = "fresh-run-7cbaf8e0-4c01-43b9-965f-31c60c3ef789";
    for (trigger, old_key, new_key, expected) in [
        (
            "no_intro_owned_diagnostic_runs_reject_replace",
            fresh,
            fresh,
            0,
        ),
        (
            "no_intro_owned_diagnostic_runs_immutable_update",
            fresh,
            fresh,
            0,
        ),
        (
            "no_intro_owned_diagnostic_runs_immutable_update",
            "run",
            fresh,
            1,
        ),
        (
            "no_intro_owned_diagnostic_runs_immutable_update",
            fresh,
            "run",
            1,
        ),
        (
            "no_intro_owned_diagnostic_runs_immutable_delete",
            fresh,
            fresh,
            0,
        ),
        (
            "no_intro_owned_diagnostic_runs_immutable_delete",
            "run",
            fresh,
            1,
        ),
        (
            "no_intro_owned_diagnostic_runs_reject_replace",
            fresh,
            "run",
            1,
        ),
    ] {
        let query = run_trigger_predicate_query(&mut connection, trigger)?;
        let result: CountRow = sql_query(&query)
            .bind::<Text, _>(old_key)
            .bind::<Text, _>(new_key)
            .get_result(&mut connection)?;
        assert_eq!(result.count, expected, "{trigger}: {old_key} -> {new_key}");

        let plan: Vec<PlanRow> = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
            .bind::<Text, _>(old_key)
            .bind::<Text, _>(new_key)
            .load(&mut connection)?;
        let details = plan
            .iter()
            .map(|row| row.detail.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for relation in &relations {
            assert!(
                plan.iter().any(|row| {
                    row.detail.starts_with(&format!("SEARCH {relation} "))
                        && row.detail.contains("run_key=?")
                }) && !plan
                    .iter()
                    .any(|row| row.detail.starts_with(&format!("SCAN {relation}"))),
                "actual {trigger} predicate must seek {relation} by run key; plan was:\n{details}"
            );
        }
    }
    Ok(())
}

#[test]
fn every_run_guard_preserves_orphan_link_protection_with_foreign_keys_and_recursion_off()
-> TestResult {
    let mut connection = all_owner_diagnostics()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let relations = native_diagnostic_relations(&mut connection)?;
    let replacement_guard = "no_intro_owned_diagnostic_runs_reject_replace";
    let replacement_guard_sql = trigger_sql(&mut connection, replacement_guard)?;

    for (index, relation) in relations.iter().enumerate() {
        let orphan_run = format!("orphan-run-{index}");
        let orphan_diagnostic = format!("orphan-diagnostic-{index}");
        let owner_guard = format!("{relation}_require_matching_owner");
        let owner_guard_sql = trigger_sql(&mut connection, &owner_guard)?;
        let owner_columns = columns(&mut connection, relation)?
            .into_iter()
            .filter(|column| column.name != "diagnostic_key" && column.name != "run_key")
            .map(|column| column.name)
            .collect::<Vec<_>>()
            .join(",");

        // Seed a damaged client's orphan row; restore insertion validation before
        // exercising the unchanged run guards. No diagnostic or run exists yet.
        connection.batch_execute(&format!("DROP TRIGGER {owner_guard}"))?;
        sql_query(format!(
            "INSERT INTO {relation}(diagnostic_key,run_key,{owner_columns}) \
             SELECT ?,?,{owner_columns} FROM {relation} WHERE run_key='run' LIMIT 1"
        ))
        .bind::<Text, _>(&orphan_diagnostic)
        .bind::<Text, _>(&orphan_run)
        .execute(&mut connection)?;
        connection.batch_execute(&owner_guard_sql)?;

        let insert_run = format!(
            "INSERT INTO import_runs( \
                 run_key,catalog_key,document_key,interpretation_key,snapshot_key,status) \
             VALUES('{orphan_run}','catalog','document','interpretation','snapshot','succeeded')"
        );
        assert_sql_rejected(
            &mut connection,
            &insert_run,
            &format!("{relation} orphan evidence allowed insertion of its missing run"),
        );
        let candidate_run = format!("rename-candidate-{index}");
        connection.batch_execute(&format!(
            "INSERT INTO import_runs( \
                 run_key,catalog_key,document_key,interpretation_key,snapshot_key,status) \
             VALUES('{candidate_run}','catalog','document','interpretation','snapshot','succeeded')"
        ))?;
        assert_sql_rejected(
            &mut connection,
            &format!(
                "UPDATE import_runs SET run_key='{orphan_run}' WHERE run_key='{candidate_run}'"
            ),
            &format!("{relation} orphan evidence allowed renaming another run to its key"),
        );

        // Seed the surviving run of an orphan diagnostic, then restore the exact
        // production replacement guard before testing OLD-key protections.
        connection.batch_execute(&format!("DROP TRIGGER {replacement_guard}"))?;
        connection.batch_execute(&insert_run)?;
        connection.batch_execute(&replacement_guard_sql)?;
        for sql in [
            format!("UPDATE import_runs SET status=status WHERE run_key='{orphan_run}'"),
            format!("DELETE FROM import_runs WHERE run_key='{orphan_run}'"),
            insert_run.replacen("INSERT INTO", "INSERT OR REPLACE INTO", 1),
        ] {
            assert_sql_rejected(
                &mut connection,
                &sql,
                &format!("{relation} orphan diagnostic evidence lost run immutability"),
            );
        }
    }
    Ok(())
}
