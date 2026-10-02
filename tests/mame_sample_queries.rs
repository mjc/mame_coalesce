use camino::Utf8PathBuf;
use diesel::{Connection, QueryableByName, RunQueryDsl, sql_query, sql_types::BigInt};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{MameFilePayload, OccurrenceKind, occurrences_for_ids},
    catalog_machines::{MachinePageLimit, machines_for_snapshot},
    check_integrity, create_backup,
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    restore_backup,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

const XML: &str = r#"<mame mameconfig="10"><machine name="audio">
  <description>Audio</description>
  <sample name="click"/>
  <rom name="rom" size="3" sha1="0123456789abcdef0123456789abcdef01234567"/>
  <sample name="click"/>
  <disk name="disk"/>
  <sample name=""/>
</machine><machine name="other"><description>Other</description><sample name="click"/></machine></mame>"#;

fn request(document_path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("sample-source"),
        source_display_name: "Sample source".into(),
        catalog_key: CatalogKey::new("sample-catalog"),
        catalog_display_name: "Sample catalog".into(),
        scope: CatalogScope::Complete,
    }
}

fn import(database: &Database, path: &Utf8PathBuf, xml: &str) -> TestResult<SnapshotKey> {
    std::fs::write(path, xml)?;
    let report = app::import_catalog(database, &request(path.clone()))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("missing published snapshot")?)
}

#[test]
fn filename_only_samples_have_distinct_media_owners_in_machine_source_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(&database, &document_path, XML)?;
    std::fs::remove_file(document_path)?;
    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(5)?)?;
    assert_eq!(page.machines.len(), 2);
    let audio = &page.machines[0];
    assert_eq!(audio.assets.len(), 5, "samples need actual media owners");
    assert_eq!(page.machines[1].assets.len(), 1);
    assert_eq!(
        audio
            .assets
            .iter()
            .map(|asset| asset.source_order)
            .collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    let ids = page
        .machines
        .iter()
        .flat_map(|machine| machine.assets.iter())
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&database, &ids)?;
    assert_eq!(files.len(), 6);
    let samples = files
        .iter()
        .filter(|file| file.provenance.occurrence_kind == OccurrenceKind::MameSample)
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 4);
    assert!(
        samples
            .iter()
            .all(|sample| sample.content_id.is_none() && sample.digests.is_empty())
    );
    for sample in &samples {
        let Some(MameFilePayload::Sample(payload)) = &sample.mame_file else {
            return Err("sample owner did not return a typed sample payload".into());
        };
        assert_eq!(
            Some(payload.name.as_str()),
            sample.provenance.asset_name.as_deref()
        );
        assert_eq!(
            Some(payload.location),
            sample.provenance.native_occurrence_location
        );
        assert!(payload.source_order > 0);
    }
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.provenance.asset_name.as_deref())
            .collect::<Vec<_>>(),
        [Some("click"), Some("click"), Some(""), Some("click")]
    );
    Ok(())
}

#[test]
fn sample_owners_and_external_originals_survive_reimport_and_paired_restore() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(&database, &path, XML)?;
    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(5)?)?;
    let ids = page
        .machines
        .iter()
        .flat_map(|machine| machine.assets.iter())
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&database, &ids)?;
    assert_eq!(files.len(), 6);
    assert_eq!(import(&database, &path, XML)?, snapshot);
    assert_eq!(occurrences_for_ids(&database, &ids)?, files);
    drop(database);
    let backup = Utf8PathBuf::try_from(directory.path().join("catalog.backup"))?;
    let restored_path = Utf8PathBuf::try_from(directory.path().join("restored.sqlite"))?;
    create_backup(&database_path, &backup)?;
    restore_backup(&backup, &restored_path, RestorePolicy::CreateNew)?;
    std::fs::remove_file(path)?;
    let restored = Database::open(&restored_path)?;
    assert_eq!(occurrences_for_ids(&restored, &ids)?, files);
    assert_eq!(
        machines_for_snapshot(&restored, &snapshot, None, MachinePageLimit::new(5)?)?,
        page
    );
    assert_eq!(
        app::load_snapshot_source(&restored, &snapshot)?,
        XML.as_bytes()
    );
    drop(restored);
    assert!(check_integrity(&restored_path)?.is_clean());
    Ok(())
}

#[test]
fn sample_history_uses_native_names_and_failed_eof_publishes_no_new_owners() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let previous = import(&database, &path, XML)?;
    let changed = XML.replacen("<sample name=\"click\"/>", "<sample name=\"pop\"/>", 1);
    let current = import(&database, &path, &changed)?;
    let history = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let audio = history
        .records
        .iter()
        .find(|record| record.set_name == "audio")
        .ok_or("sample history has no audio record")?;
    assert!(
        audio.metadata_changed,
        "sample name change was not retained as a native fact"
    );
    let page = machines_for_snapshot(&database, &current, None, MachinePageLimit::new(5)?)?;
    let ids = page
        .machines
        .iter()
        .flat_map(|machine| machine.assets.iter())
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&database, &ids)?;
    std::fs::write(
        &path,
        "<mame mameconfig='10'><machine name='unpublished'><description>Bad EOF</description><sample name='lost'/></machine>",
    )?;
    let failed = app::import_catalog(&database, &request(path))?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(occurrences_for_ids(&database, &ids)?, files);
    let mut connection = diesel::SqliteConnection::establish(database_path.as_str())?;
    assert_eq!(
        sql_query("SELECT count(*) AS count FROM mame_samples WHERE name='lost'")
            .get_result::<Count>(&mut connection)?
            .count,
        0
    );
    Ok(())
}
