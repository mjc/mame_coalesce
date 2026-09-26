use std::path::Path;

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, DiskAuditRequest, DiskAuditState},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

fn mame_request() -> mame_coalesce::Result<CatalogImportRequest> {
    let document_path = Utf8PathBuf::from_path_buf(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/machine.xml"),
    )
    .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    Ok(CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("disk-audit-test"),
        source_display_name: "Disk audit test".to_owned(),
        catalog_key: CatalogKey::new("disk-audit-fixture"),
        catalog_display_name: "Disk audit fixture".to_owned(),
        scope: CatalogScope::Complete,
    })
}

fn setup() -> mame_coalesce::Result<(tempfile::TempDir, Database, tempfile::TempDir)> {
    let db_dir = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(db_dir.path().join("audit.db"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let database = Database::open(&database_path)?;
    app::import_catalog(&database, &mame_request()?)?;
    let source_dir = tempfile::tempdir()?;
    Ok((db_dir, database, source_dir))
}

#[test]
fn disk_audit_reports_missing_then_unverified_without_exposing_container_hash()
-> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    let request = DiskAuditRequest {
        catalog_key: "disk-audit-fixture".to_owned(),
        source_path: Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
            .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?,
    };

    let missing = app::audit_disks(&database, &request)?;
    assert_eq!(missing.disks.len(), 1);
    assert_eq!(missing.disks[0].state, DiskAuditState::Missing);
    assert_eq!(missing.schema_version, 1);

    std::fs::write(
        source_dir.path().join("demo_disk.chd"),
        b"synthetic container bytes",
    )?;
    app::scan_source(
        &database,
        &app::SourceScanRequest {
            source_path: request.source_path.clone(),
            jobs: 1,
        },
    )?;
    let present = app::audit_disks(&database, &request)?;
    assert_eq!(present.disks[0].state, DiskAuditState::UnverifiedContainer);
    assert_eq!(
        present.disks[0].expected_logical_sha1.as_deref(),
        Some("1123456789abcdef0123456789abcdef01234567")
    );
    let json = serde_json::to_value(&present)?;
    assert!(json["disks"][0].get("observed_sha1").is_none());
    Ok(())
}

#[test]
fn disk_audit_reports_unsupported_container_name() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    std::fs::write(source_dir.path().join("demo_disk"), b"not a CHD")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    app::scan_source(
        &database,
        &app::SourceScanRequest {
            source_path: source_path.clone(),
            jobs: 1,
        },
    )?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-fixture".to_owned(),
            source_path,
        },
    )?;
    assert_eq!(report.disks[0].state, DiskAuditState::UnsupportedContainer);
    Ok(())
}
