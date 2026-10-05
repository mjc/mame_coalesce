use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Counts {
    #[diesel(sql_type = BigInt)]
    snapshots: i64,
    #[diesel(sql_type = BigInt)]
    groups: i64,
    #[diesel(sql_type = BigInt)]
    items: i64,
    #[diesel(sql_type = BigInt)]
    occurrences: i64,
    #[diesel(sql_type = BigInt)]
    issued_ids: i64,
}

#[derive(QueryableByName)]
struct NativeFacts {
    #[diesel(sql_type = BigInt)]
    clone_links: i64,
    #[diesel(sql_type = BigInt)]
    list_details: i64,
    #[diesel(sql_type = BigInt)]
    late_notes: i64,
    #[diesel(sql_type = BigInt)]
    late_note_positions: i64,
}

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    document_path: Utf8PathBuf,
}

impl Fixture {
    fn new(xml: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
            .map_err(|_| "non-UTF-8 document path")?;
        let database = Database::open(&database_path)?;
        std::fs::write(&document_path, xml)?;
        Ok(Self {
            _directory: directory,
            database,
            database_path,
            document_path,
        })
    }

    fn request(&self) -> CatalogImportRequest {
        CatalogImportRequest {
            document_path: self.document_path.clone(),
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("single-pass-test-source"),
            source_display_name: "Single pass test".to_owned(),
            catalog_key: CatalogKey::new("single-pass-test-catalog"),
            catalog_display_name: "Single pass test".to_owned(),
            scope: CatalogScope::Complete,
        }
    }

    fn counts(&self) -> TestResult<Counts> {
        let mut connection = SqliteConnection::establish(self.database_path.as_str())?;
        Ok(sql_query(
            "SELECT (SELECT COUNT(*) FROM catalog_snapshots) AS snapshots, \
                    (SELECT COUNT(*) FROM catalog_set_groups) AS groups, \
                    (SELECT COUNT(*) FROM software_items) AS items, \
                    (SELECT COUNT(*) FROM asset_occurrences) AS occurrences, \
                    (SELECT COUNT(*) FROM catalog_contents) AS issued_ids",
        )
        .get_result(&mut connection)?)
    }
}

const LATE_INVALID_XML: &str = r#"<softwarelists>
  <softwarelist name="first"><software name="game">
    <description>Game</description><year>2000</year><publisher>Example</publisher>
    <part name="cart" interface="cart"><dataarea name="rom" size="1">
      <rom name="game.bin" size="1" sha1="0123456789abcdef0123456789abcdef01234567"/>
    </dataarea></part>
  </software></softwarelist>
  <softwarelist name="late"><software name="bad" supported="sometimes">
    <description>Bad</description><year>2001</year><publisher>Example</publisher>
  </software></softwarelist>
</softwarelists>"#;

#[test]
fn late_parse_error_rolls_back_streamed_native_rows_and_file_identities() -> TestResult {
    let fixture = Fixture::new(LATE_INVALID_XML)?;
    let before = fixture.counts()?;

    let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
    diesel::connection::SimpleConnection::batch_execute(
        &mut connection,
        "CREATE TRIGGER software_item_storage_sentinel BEFORE INSERT ON software_items \
         BEGIN SELECT RAISE(ABORT, 'software item storage sentinel'); END;",
    )?;
    drop(connection);

    let imported = app::import_catalog(&fixture.database, &fixture.request());
    assert!(
        imported
            .as_ref()
            .is_err_and(|error| error.to_string().contains("software item storage sentinel")),
        "the import must attempt the first complete item before parsing the later invalid item; got {imported:?}"
    );

    let after = fixture.counts()?;
    assert_eq!(after.snapshots, before.snapshots);
    assert_eq!(after.groups, before.groups);
    assert_eq!(after.items, before.items);
    assert_eq!(after.occurrences, before.occurrences);
    assert_eq!(after.issued_ids, before.issued_ids);
    Ok(())
}

#[test]
fn actual_trailing_eof_failure_rolls_back_completed_items_and_issued_ids() -> TestResult {
    let fixture = Fixture::new(concat!(
        "<softwarelist name=\"list\"><software name=\"game\">",
        "<description>Game</description><year>2000</year><publisher>Example</publisher>",
        "<part name=\"cart\" interface=\"cart\"><dataarea name=\"rom\" size=\"1\">",
        "<rom name=\"game.bin\" size=\"1\" ",
        "sha1=\"0123456789abcdef0123456789abcdef01234567\"/>",
        "</dataarea></part></software></softwarelist><trailing/>"
    ))?;
    let before = fixture.counts()?;

    let report = app::import_catalog(&fixture.database, &fixture.request())?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);

    let after = fixture.counts()?;
    assert_eq!(after.snapshots, before.snapshots);
    assert_eq!(after.groups, before.groups);
    assert_eq!(after.items, before.items);
    assert_eq!(after.occurrences, before.occurrences);
    assert_eq!(after.issued_ids, before.issued_ids);
    Ok(())
}

#[test]
fn clone_item_precedes_late_list_details_and_published_reimport_reuses_owners() -> TestResult {
    let fixture = Fixture::new(concat!(
        "<softwarelists><softwarelist name=\"later\" description=\"Detailed list\">",
        "<software name=\"child\" cloneof=\"parent\"><description>Child</description>",
        "<year>2000</year><publisher>Example</publisher></software>",
        "<notes>Late notes</notes></softwarelist></softwarelists>"
    ))?;
    let first = app::import_catalog(&fixture.database, &fixture.request())?;
    assert_eq!(first.status, app::CatalogImportStatus::Succeeded);
    let snapshot = first
        .snapshot_key
        .ok_or("successful import had no snapshot")?;

    let mut connection = SqliteConnection::establish(fixture.database_path.as_str())?;
    let facts = sql_query(
        r"WITH target_groups AS (
             SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key = ?
         )
         SELECT
             (SELECT COUNT(*) FROM software_clone_links AS links
              JOIN catalog_sets AS sets ON sets.set_id = links.set_id
              JOIN target_groups USING (set_group_id)) AS clone_links,
             (SELECT COUNT(*) FROM software_lists AS lists
              JOIN target_groups ON target_groups.set_group_id = lists.namespace_id) AS list_details,
             (SELECT COUNT(*) FROM software_lists AS lists
              JOIN target_groups ON target_groups.set_group_id = lists.namespace_id
              WHERE lists.notes = 'Late notes') AS late_notes,
             (SELECT COUNT(*) FROM software_list_text_positions AS positions
              JOIN target_groups ON target_groups.set_group_id = positions.namespace_id
              WHERE positions.field_kind = 3) AS late_note_positions",
    )
    .bind::<diesel::sql_types::Text, _>(snapshot.as_str())
    .get_result::<NativeFacts>(&mut connection)?;
    assert_eq!(
        facts.clone_links, 1,
        "clone identity was written at item completion"
    );
    assert_eq!(facts.list_details, 1);
    assert_eq!(facts.late_notes, 1);
    assert_eq!(facts.late_note_positions, 1);

    diesel::connection::SimpleConnection::batch_execute(
        &mut connection,
        "CREATE TRIGGER software_reimport_item_sentinel BEFORE INSERT ON software_items \
         BEGIN SELECT RAISE(ABORT, 'published software item was recreated'); END; \
         CREATE TRIGGER software_reimport_list_sentinel BEFORE INSERT ON software_lists \
         BEGIN SELECT RAISE(ABORT, 'published software list was recreated'); END;",
    )?;
    drop(connection);

    let before_reimport = fixture.counts()?;
    let repeated = app::import_catalog(&fixture.database, &fixture.request())?;
    assert_eq!(repeated.status, app::CatalogImportStatus::Succeeded);
    assert_eq!(repeated.snapshot_key.as_ref(), Some(&snapshot));
    assert_eq!(fixture.counts()?.issued_ids, before_reimport.issued_ids);
    assert_eq!(fixture.counts()?.items, before_reimport.items);
    Ok(())
}
