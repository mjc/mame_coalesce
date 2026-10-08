use std::{
    collections::BTreeMap,
    env,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use camino::Utf8PathBuf;
use diesel::connection::{Instrumentation, InstrumentationEvent, set_default_instrumentation};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    import_diagnostics::{self, DiagnosticPageLimit},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let timings = env::var_os("CATALOG_SQL_TIMINGS").is_some();
    if timings {
        set_default_instrumentation(|| Some(sql_timings()))?;
    }
    let result = run(env::args().skip(1));
    if timings {
        report_sql_timings();
    }
    result
}

#[derive(Clone, Copy, Default)]
struct QueryTiming {
    count: u32,
    elapsed: Duration,
}

static SQL_TIMINGS: OnceLock<Mutex<BTreeMap<String, QueryTiming>>> = OnceLock::new();

fn sql_timings() -> Box<dyn Instrumentation> {
    sql_timings_with_report(report_sql_timings)
}

fn sql_timings_with_report(mut report: impl FnMut() + Send + 'static) -> Box<dyn Instrumentation> {
    let mut started = None;
    let mut queries_until_report = 10_000;
    Box::new(move |event: InstrumentationEvent<'_>| match event {
        InstrumentationEvent::StartQuery { .. } => started = Some(Instant::now()),
        InstrumentationEvent::FinishQuery { query, .. } => {
            let Some(start) = started.take() else { return };
            let elapsed = start.elapsed();
            let rendered = query.to_string();
            let sql = rendered.split(" -- binds:").next().unwrap_or(&rendered);
            let mut timings = SQL_TIMINGS
                .get_or_init(Mutex::default)
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let timing = timings.entry(sql.to_owned()).or_default();
            timing.count += 1;
            timing.elapsed += elapsed;
            drop(timings);
            queries_until_report -= 1;
            if queries_until_report == 0 {
                queries_until_report = 10_000;
                report();
            }
        }
        _ => {}
    })
}

#[cfg(test)]
mod timing_tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use diesel::{Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query};

    #[test]
    fn sql_timings_reports_completed_query_checkpoints() -> Result<(), Box<dyn std::error::Error>> {
        #[derive(QueryableByName)]
        struct Value {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            value: i64,
        }
        let reports = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&reports);
        let mut connection = SqliteConnection::establish(":memory:")?;
        connection.set_instrumentation(super::sql_timings_with_report(move || {
            observed.fetch_add(1, Ordering::Relaxed);
        }));
        for index in 1..=20_000 {
            let row = sql_query("SELECT 1 AS value").get_result::<Value>(&mut connection)?;
            assert_eq!(row.value, 1);
            assert_eq!(reports.load(Ordering::Relaxed), index / 10_000);
        }
        Ok(())
    }
}

fn report_sql_timings() {
    let Some(timings) = SQL_TIMINGS.get() else {
        return;
    };
    let mut rows = timings
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|(sql, timing)| (sql.clone(), *timing))
        .collect::<Vec<_>>();
    rows.sort_unstable_by_key(|(_, timing)| std::cmp::Reverse(timing.elapsed));
    let calls = rows
        .iter()
        .map(|(_, timing)| u64::from(timing.count))
        .sum::<u64>();
    eprintln!("SQL total_calls={calls} statement_shapes={}", rows.len());
    for (sql, timing) in rows.into_iter().take(20) {
        let abbreviated = sql.split_whitespace().collect::<Vec<_>>().join(" ");
        let abbreviated = abbreviated.chars().take(2_400).collect::<String>();
        eprintln!(
            "SQL seconds={:.6} calls={} average_us={:.1} query={abbreviated}",
            timing.elapsed.as_secs_f64(),
            timing.count,
            timing.elapsed.as_secs_f64() * 1_000_000.0 / f64::from(timing.count),
        );
    }
}

fn run(arguments: impl IntoIterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = arguments.into_iter();
    let format = parse_format(&args.next().ok_or("missing format")?)?;
    let database_path = Utf8PathBuf::from(args.next().ok_or("missing database path")?);
    let document_paths = args.map(Utf8PathBuf::from).collect::<Vec<_>>();
    if document_paths.is_empty() {
        return Err("provide at least one catalog document".into());
    }

    let database = Database::open(&database_path)?;
    let source_key = PublishingSourceKey::new(format!("profile-{}", format.as_str()));
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    for document_path in document_paths {
        let canonical_path = Utf8PathBuf::try_from(std::fs::canonicalize(&document_path)?)?;
        let name = Path::new(document_path.as_str())
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("document path has no UTF-8 file stem: {document_path}"))?
            .to_owned();
        let request = CatalogImportRequest {
            document_path,
            format,
            source_key: source_key.clone(),
            source_display_name: format!("Profiling corpus ({})", format.as_str()),
            catalog_key: CatalogKey::new(format!("profile-{}-{canonical_path}", format.as_str())),
            catalog_display_name: format!("Profiling import: {name}"),
            scope: CatalogScope::Complete,
        };
        println!(
            "IMPORTING document={:?} format={}",
            request.document_path,
            format.as_str()
        );
        let started = Instant::now();
        let report = app::import_catalog(&database, &request)?;
        println!(
            "IMPORTED document={:?} status={:?} run={} snapshot={:?} diagnostics={} elapsed_seconds={:.3}",
            request.document_path,
            report.status,
            report.run_key,
            report.snapshot_key,
            report.diagnostic_count,
            started.elapsed().as_secs_f64(),
        );
        match report.status {
            CatalogImportStatus::Succeeded => succeeded += 1,
            CatalogImportStatus::Failed => {
                failed += 1;
                let page = import_diagnostics::for_run(
                    &database,
                    &report.run_key,
                    None,
                    DiagnosticPageLimit::new(1)?,
                )?;
                eprintln!(
                    "IMPORT_ERROR document={:?} message={:?}",
                    request.document_path, page.run.summary
                );
            }
        }
    }

    println!(
        "format={} succeeded={} failed={}",
        format.as_str(),
        succeeded,
        failed
    );
    if failed == 0 {
        Ok(())
    } else {
        Err(format!("{failed} catalog documents failed validation").into())
    }
}

#[cfg(test)]
mod tests {
    use diesel::{Connection, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt};

    use super::run;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    fn arguments(database: &std::path::Path, inputs: &[std::path::PathBuf]) -> Vec<String> {
        std::iter::once("logiqx".to_owned())
            .chain(std::iter::once(database.to_string_lossy().into_owned()))
            .chain(
                inputs
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned()),
            )
            .collect()
    }

    fn count(database: &std::path::Path, query: &str) -> Result<i64, Box<dyn std::error::Error>> {
        let mut connection = SqliteConnection::establish(database.to_str().ok_or("UTF-8 path")?)?;
        Ok(sql_query(query).get_result::<Count>(&mut connection)?.count)
    }

    #[test]
    fn failed_document_fails_batch_after_importing_later_documents() -> TestResult {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("catalog.sqlite");
        let invalid = directory.path().join("invalid.dat");
        let valid = directory.path().join("valid.dat");
        std::fs::write(&invalid, "<datafile><game")?;
        std::fs::write(&valid, "<datafile><game name=\"valid\"/></datafile>")?;
        let result = run(arguments(&database, &[invalid, valid]));
        assert_eq!(
            count(&database, "SELECT COUNT(*) AS count FROM catalog_snapshots")?,
            1
        );
        assert_eq!(
            count(
                &database,
                "SELECT COUNT(*) AS count FROM import_runs WHERE status='failed'"
            )?,
            1
        );
        assert!(
            result.is_err(),
            "a failed import must fail the profiling command"
        );
        Ok(())
    }

    #[test]
    fn equal_basenames_in_different_directories_are_distinct_catalogs() -> TestResult {
        let directory = tempfile::tempdir()?;
        let database = directory.path().join("catalog.sqlite");
        let first = directory.path().join("first/same.dat");
        let second = directory.path().join("second/same.dat");
        std::fs::create_dir_all(first.parent().ok_or("first parent")?)?;
        std::fs::create_dir_all(second.parent().ok_or("second parent")?)?;
        std::fs::write(&first, "<datafile><game name=\"first\"/></datafile>")?;
        std::fs::write(&second, "<datafile><game name=\"second\"/></datafile>")?;
        run(arguments(&database, &[first.clone(), second.clone()]))?;
        assert_eq!(
            count(&database, "SELECT COUNT(*) AS count FROM catalogs")?,
            2
        );
        assert_eq!(
            count(&database, "SELECT COUNT(*) AS count FROM catalog_snapshots")?,
            2
        );
        run(arguments(&database, &[first, second]))?;
        assert_eq!(
            count(&database, "SELECT COUNT(*) AS count FROM catalogs")?,
            2
        );
        assert_eq!(
            count(&database, "SELECT COUNT(*) AS count FROM catalog_snapshots")?,
            2
        );
        assert_eq!(
            count(
                &database,
                "SELECT COUNT(*) AS count FROM import_runs WHERE status='succeeded'"
            )?,
            4
        );
        Ok(())
    }
}

fn parse_format(value: &str) -> Result<CatalogDocumentFormat, Box<dyn std::error::Error>> {
    match value {
        "logiqx" => Ok(CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        )),
        "logiqx-dtd15" => Ok(CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::StrictDtd15,
        )),
        "machine" => Ok(CatalogDocumentFormat::MameListXml),
        "software-list" => Ok(CatalogDocumentFormat::MameSoftwareListXml),
        "clrmamepro" => Ok(CatalogDocumentFormat::ClrMamePro),
        "no-intro-pc-xml" => Ok(CatalogDocumentFormat::NoIntroPcXml),
        "no-intro-dat-v3-strict" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V3Strict,
        )),
        "no-intro-dat-v3-compatible" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V3Compatible,
        )),
        "no-intro-dat-v4-strict" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V4Strict,
        )),
        "no-intro-dat-v4-compatible" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V4Compatible,
        )),
        "no-intro-database-xml-compatible" => Ok(CatalogDocumentFormat::NoIntroDatabase(
            mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode::ObservedCompatible,
        )),
        "no-intro-database-xml-nul-compatible" => Ok(CatalogDocumentFormat::NoIntroDatabase(
            mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode::NullRecoveryCompatible,
        )),
        _ => Err(format!("unsupported catalog format: {value}").into()),
    }
}
