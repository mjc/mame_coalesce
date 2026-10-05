use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Text},
};

use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

#[derive(QueryableByName)]
struct SchemaVersion {
    #[diesel(sql_type = BigInt)]
    schema_version: i64,
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

fn temp_schema_version(conn: &mut diesel::SqliteConnection) -> crate::Result<i64> {
    Ok(sql_query("PRAGMA temp.schema_version")
        .get_result::<SchemaVersion>(conn)?
        .schema_version)
}

fn import_mame_snapshot(
    database: &Database,
    directory: &tempfile::TempDir,
    catalog: &str,
) -> crate::Result<SnapshotKey> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join(format!("{catalog}.xml")))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
    std::fs::write(
        &path,
        include_str!("../../../../fixtures/specifications/mame-machine-fields.xml"),
    )?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new(catalog),
            source_display_name: catalog.into(),
            catalog_key: CatalogKey::new(catalog),
            catalog_display_name: catalog.into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    report
        .snapshot_key
        .ok_or_else(|| crate::Error::InvalidPath("successful publication missing snapshot".into()))
}

#[test]
fn history_sql_no_mame_owners_preserves_temp_schema_version() -> crate::Result<()> {
    let pool = crate::storage::db::create_db_pool(":memory:")?;
    let mut conn = pool.get()?;
    let schema_before = temp_schema_version(&mut conn)?;

    let history = super::records(
        &mut conn,
        &SnapshotKey::from_persisted("snapshot-with-no-mame-owners".into()),
        &BTreeMap::new(),
    )?;

    assert!(history.machines.is_empty());
    assert!(history.media.is_empty());
    assert_eq!(temp_schema_version(&mut conn)?, schema_before);
    Ok(())
}

#[test]
fn history_sql_repeated_mame_record_reads_keep_temp_schema_stable() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let snapshot = import_mame_snapshot(&database, &directory, "history-sql-churn")?;
    let mut conn = database.pool().get()?;
    let child_ranks = super::super::load_mame_child_ranks(&mut conn, &snapshot)?;

    let first = super::records(&mut conn, &snapshot, &child_ranks)?;
    assert!(!first.machines.is_empty());
    let initialized_schema = temp_schema_version(&mut conn)?;

    for _ in 0..3 {
        let history = super::records(&mut conn, &snapshot, &child_ranks)?;
        assert_eq!(history.machines, first.machines);
        assert_eq!(history.media, first.media);
        assert_eq!(temp_schema_version(&mut conn)?, initialized_schema);
    }
    Ok(())
}

#[test]
fn history_sql_error_does_not_poison_later_reads() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let snapshot = import_mame_snapshot(&database, &directory, "history-sql-churn")?;
    let mut conn = database.pool().get()?;

    assert!(super::records(&mut conn, &snapshot, &BTreeMap::new()).is_err());

    let child_ranks = super::super::load_mame_child_ranks(&mut conn, &snapshot)?;
    let history = super::records(&mut conn, &snapshot, &child_ranks)?;
    assert!(!history.machines.is_empty());
    Ok(())
}

#[test]
fn history_sql_family_queries_seek_only_requested_numeric_owners() -> crate::Result<()> {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let requested = import_mame_snapshot(&database, &directory, "history-sql-requested")?;
    import_mame_snapshot(&database, &directory, "history-sql-unrelated")?;
    let mut conn = database.pool().get()?;

    for family in super::FAMILIES {
        let plan = sql_query(format!("EXPLAIN QUERY PLAN {}", super::query(family)))
            .bind::<Text, _>(requested.as_str())
            .load::<ExplainRow>(&mut conn)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        let scans = plan
            .iter()
            .filter(|detail| {
                detail.strip_prefix("SCAN ").is_some_and(|scan| {
                    let alias = scan.split_whitespace().next().unwrap_or_default();
                    matches!(
                        alias,
                        "groups" | "sets" | "occurrence" | "native" | "position"
                    )
                })
            })
            .collect::<Vec<_>>();
        assert!(
            scans.is_empty(),
            "{family:?} history query scans unbounded source tables: {scans:?}; plan: {plan:?}"
        );
        assert!(
            plan.iter().any(|detail| {
                detail.starts_with("SEARCH groups USING ") && detail.contains("snapshot_key=?")
            }),
            "{family:?} history query does not seek the requested snapshot key: {plan:?}"
        );
        let owner_key = if matches!(
            family,
            super::Family::Rom
                | super::Family::Disk
                | super::Family::Sample
                | super::Family::RomCompatibility
                | super::Family::DiskCompatibility
        ) {
            "occurrence_id=?"
        } else {
            "set_id=?"
        };
        assert!(
            plan.iter().any(|detail| {
                detail.starts_with("SEARCH position USING ") && detail.contains(owner_key)
            }),
            "{family:?} history query does not seek its numeric owner key {owner_key}: {plan:?}"
        );
    }
    Ok(())
}
