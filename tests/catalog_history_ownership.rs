use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey, SnapshotRecordCorrespondence,
        SnapshotRecordStatus,
    },
};

fn import_logiqx(
    database: &Database,
    directory: &tempfile::TempDir,
    file_name: &str,
    contents: &str,
) -> Result<SnapshotKey, Box<dyn std::error::Error>> {
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join(file_name))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::Logiqx(
                mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
            ),
            source_key: PublishingSourceKey::new("repeated-owner-history"),
            source_display_name: "Repeated owner history".to_owned(),
            catalog_key: CatalogKey::new("repeated-owner-history"),
            catalog_display_name: "Repeated owner history".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    report
        .snapshot_key
        .ok_or_else(|| "Logiqx import did not produce a snapshot".into())
}

fn snapshot_pair(
    directory: &tempfile::TempDir,
    previous: &str,
    current: &str,
) -> Result<(Database, SnapshotKey, SnapshotKey), Box<dyn std::error::Error>> {
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let previous = import_logiqx(&database, directory, "previous.dat", previous)?;
    let current = import_logiqx(&database, directory, "current.dat", current)?;
    Ok((database, previous, current))
}

#[test]
fn repeated_same_name_owner_permutation_with_identical_metadata_is_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (database, previous, current) = snapshot_pair(
        &directory,
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>Same</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>Same</description><rom name='beta.bin' size='8'/></game></datafile>",
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>Same</description><rom name='beta.bin' size='8'/></game><game name='same'><description>Same</description><rom name='alpha.bin' size='4'/></game></datafile>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated-name record missing from diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn repeated_same_name_cross_assignment_is_changed_and_ambiguous()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (database, previous, current) = snapshot_pair(
        &directory,
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>First owner</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>Second owner</description><rom name='beta.bin' size='8'/></game></datafile>",
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>Second owner</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>First owner</description><rom name='beta.bin' size='8'/></game></datafile>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated-name record missing from diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::Ambiguous
    );
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn repeated_same_name_permutation_of_distinct_whole_owners_is_unchanged()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (database, previous, current) = snapshot_pair(
        &directory,
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>First owner</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>Second owner</description><rom name='beta.bin' size='8'/></game></datafile>",
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>Second owner</description><rom name='beta.bin' size='8'/></game><game name='same'><description>First owner</description><rom name='alpha.bin' size='4'/></game></datafile>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated-name record missing from diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn inserting_a_repeated_name_owner_reports_an_ambiguous_group_change()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (database, previous, current) = snapshot_pair(
        &directory,
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>First owner</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>Second owner</description><rom name='beta.bin' size='8'/></game></datafile>",
        "<datafile><header><name>Repeated owners</name></header><game name='same'><description>First owner</description><rom name='alpha.bin' size='4'/></game><game name='same'><description>Second owner</description><rom name='beta.bin' size='8'/></game><game name='same'><description>Third owner</description><rom name='gamma.bin' size='12'/></game></datafile>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated-name record missing from diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::Ambiguous
    );
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn repeated_name_vendor_fields_do_not_change_native_history()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let (database, previous, current) = snapshot_pair(
        &directory,
        "<datafile><header><name>Repeated owners</name></header><game name='same' future='first'><description>Same</description><rom name='alpha.bin' size='4'/></game><game name='same' future='second'><description>Same</description><rom name='beta.bin' size='8'/></game></datafile>",
        "<datafile><header><name>Repeated owners</name></header><game name='same' future='second'><description>Same</description><rom name='alpha.bin' size='4'/></game><game name='same' future='first'><description>Same</description><rom name='beta.bin' size='8'/></game></datafile>",
    )?;

    let diff = app::diff_catalog_snapshots(&database, &previous, &current)?;
    let record = diff
        .records
        .iter()
        .find(|record| record.set_name == "same")
        .ok_or("repeated-name record missing from diff")?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert_eq!(
        record.correspondence,
        SnapshotRecordCorrespondence::ExactFacts
    );
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());

    let recovered_source = app::load_snapshot_source(&database, &current)?;
    assert!(String::from_utf8(recovered_source)?.contains("future='second'"));
    Ok(())
}
