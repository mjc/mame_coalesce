use camino::Utf8PathBuf;
use diesel::Connection as _;
use diesel::{QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::Text};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct ColumnName {
    #[diesel(sql_type = Text)]
    name: String,
}

#[test]
fn snapshot_identity_has_no_second_source_version_owner() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let columns =
        sql_query("PRAGMA table_info(catalog_snapshots)").load::<ColumnName>(&mut connection)?;
    assert!(
        columns
            .iter()
            .all(|column| column.name != "declared_version"),
        "a catalog edition cannot own a second copy of its native header version"
    );
    Ok(())
}

#[test]
fn logiqx_and_cmp_history_versions_are_native_header_facts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let root = Utf8PathBuf::try_from(directory.path().to_path_buf())?;
    let database = Database::open(&root.join("catalog.sqlite"))?;
    for (key, format, text, version) in [
        (
            "logiqx-version",
            CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
            "<datafile><header><name>Native</name><version> logiqx-v </version></header><game name='same'/></datafile>",
            " logiqx-v ",
        ),
        (
            "cmp-version",
            CatalogDocumentFormat::ClrMamePro,
            "clrmamepro (name Native version \" cmp-v \" ) game (name same)",
            " cmp-v ",
        ),
    ] {
        let document_path = root.join(format!("{key}.dat"));
        std::fs::write(&document_path, text)?;
        let catalog_key = CatalogKey::new(key);
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: document_path.clone(),
                format,
                source_key: PublishingSourceKey::new("native-version-publisher"),
                source_display_name: "Native version publisher".into(),
                catalog_key: catalog_key.clone(),
                catalog_display_name: key.into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        // Public history is a database query, independent of the input path.
        std::fs::remove_file(document_path)?;
        let history = app::catalog_snapshot_history(&database, &catalog_key)?;
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].snapshot,
            report.snapshot_key.ok_or("missing snapshot")?
        );
        assert_eq!(
            history[0].declared_version.as_deref(),
            Some(version),
            "{key}"
        );
    }
    Ok(())
}
