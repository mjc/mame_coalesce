use std::path::Path;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::Text,
};
use mame_coalesce::{
    app::{
        self, CatalogDocumentFormat, CatalogImportRequest, DiskAuditRequest, DiskAuditState,
        SourceScanRequest,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct ImportDiagnosticRow {
    #[diesel(sql_type = Text)]
    message: String,
}

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

fn software_list_request() -> mame_coalesce::Result<CatalogImportRequest> {
    let mut request = mame_request()?;
    request.document_path = Utf8PathBuf::from_path_buf(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/catalog/mame/software-list.xml"),
    )
    .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    request.format = CatalogDocumentFormat::MameSoftwareListXml;
    request.catalog_key = CatalogKey::new("disk-audit-software-fixture");
    "Disk audit software fixture".clone_into(&mut request.catalog_display_name);
    Ok(request)
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

    std::fs::create_dir_all(source_dir.path().join("demo_machine"))?;
    std::fs::write(
        source_dir.path().join("demo_machine/demo_disk.chd"),
        b"synthetic container bytes",
    )?;
    let present = app::audit_disks(&database, &request)?;
    assert_eq!(present.disks[0].state, DiskAuditState::UnverifiedContainer);
    assert_eq!(
        present.disks[0].expected_logical_sha1.as_deref(),
        Some("1123456789abcdef0123456789abcdef01234567")
    );
    app::scan_source(
        &database,
        &SourceScanRequest {
            source_path: request.source_path.clone(),
            jobs: 1,
        },
    )?;
    std::fs::remove_file(source_dir.path().join("demo_machine/demo_disk.chd"))?;
    let removed = app::audit_disks(&database, &request)?;
    assert_eq!(removed.disks[0].state, DiskAuditState::Missing);
    let json = serde_json::to_value(&present)?;
    assert!(json["disks"][0].get("observed_sha1").is_none());
    Ok(())
}

#[test]
fn disk_audit_names_catalog_without_published_snapshot() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let Err(error) = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "missing-catalog".to_owned(),
            source_path,
        },
    ) else {
        return Err(
            std::io::Error::other("expected the unpublished catalog to fail explicitly").into(),
        );
    };
    assert_eq!(
        error.to_string(),
        "no published snapshot exists for catalog missing-catalog"
    );
    Ok(())
}

#[test]
fn disk_audit_reports_unsupported_container_name() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    std::fs::create_dir_all(source_dir.path().join("demo_machine"))?;
    std::fs::write(
        source_dir.path().join("demo_machine/demo_disk"),
        b"not a CHD",
    )?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
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

#[test]
fn disk_audit_reports_software_list_disk_with_full_context() -> mame_coalesce::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(db_dir.path().join("software-audit.db"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let database = Database::open(&database_path)?;
    app::import_catalog(&database, &software_list_request()?)?;
    let source_dir = tempfile::tempdir()?;
    let path = source_dir
        .path()
        .join("demo_cart/demo_original/demo-disk.chd");
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| std::io::Error::other("nested software media path has no parent"))?,
    )?;
    std::fs::write(path, b"container bytes")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-software-fixture".to_owned(),
            source_path,
        },
    )?;
    assert_eq!(report.disks.len(), 2);
    let disk = report
        .disks
        .iter()
        .find(|entry| entry.disk_name == "demo-disk")
        .ok_or_else(|| std::io::Error::other("fixture software disk is reported"))?;
    assert_eq!(disk.state, DiskAuditState::UnverifiedContainer);
    assert_eq!(disk.list_name.as_deref(), Some("demo_cart"));
    assert_eq!(disk.item_name.as_deref(), Some("demo_game"));
    assert_eq!(disk.part_name.as_deref(), Some("cart"));
    let json = serde_json::to_value(&report)?;
    let json_disk = report
        .disks
        .iter()
        .position(|entry| entry.disk_name == "demo-disk")
        .ok_or_else(|| std::io::Error::other("fixture software disk is reported"))?;
    assert_eq!(json["disks"][json_disk]["list_name"], "demo_cart");
    assert_eq!(json["disks"][json_disk]["item_name"], "demo_game");
    assert_eq!(json["disks"][json_disk]["part_name"], "cart");
    Ok(())
}

#[test]
fn disk_audit_follows_software_clone_chain_and_flat_parent_media_layout()
-> mame_coalesce::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(db_dir.path().join("software-clones.db"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let database = Database::open(&database_path)?;
    let catalog_dir = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::from_path_buf(catalog_dir.path().join("software.xml"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    std::fs::write(
        &document_path,
        br#"<softwarelists><softwarelist name="clone_list" description="Clone fixture">
          <software name="grandparent"><description>Grandparent</description><year>1998</year><publisher>Example</publisher>
            <part name="cart" interface="cart"><diskarea name="media"><disk name="ancestor-disk" sha1="fedcba9876543210fedcba9876543210fedcba98" /></diskarea></part>
          </software>
          <software name="parent" cloneof="grandparent"><description>Parent</description><year>1999</year><publisher>Example</publisher></software>
          <software name="child" cloneof="parent"><description>Child</description><year>2000</year><publisher>Example</publisher>
            <part name="cart" interface="cart"><diskarea name="media"><disk name="child-disk" sha1="fedcba9876543210fedcba9876543210fedcba98" /></diskarea></part>
          </software>
        </softwarelist></softwarelists>"#,
    )?;
    let mut request = software_list_request()?;
    request.document_path = document_path;
    request.catalog_key = CatalogKey::new("disk-audit-software-clones");
    let imported = app::import_catalog(&database, &request)?;
    let mut connection =
        SqliteConnection::establish(database_path.as_str()).map_err(std::io::Error::other)?;
    let diagnostics =
        sql_query("SELECT message FROM import_diagnostics WHERE run_key = ? ORDER BY source_line")
            .bind::<Text, _>(imported.run_key.to_string())
            .load::<ImportDiagnosticRow>(&mut connection)?;
    assert_eq!(
        imported.status,
        app::CatalogImportStatus::Succeeded,
        "catalog import diagnostics: {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );

    let source_dir = tempfile::tempdir()?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let misplaced_path = source_dir.path().join("child/ancestor-disk.chd");
    std::fs::create_dir_all(
        misplaced_path
            .parent()
            .ok_or_else(|| std::io::Error::other("misplaced media path has no parent"))?,
    )?;
    std::fs::write(&misplaced_path, b"misplaced ancestor disk")?;
    let misplaced_report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-software-clones".to_owned(),
            source_path: source_path.clone(),
        },
    )?;
    let misplaced_child = misplaced_report
        .disks
        .iter()
        .find(|entry| entry.item_name.as_deref() == Some("child"))
        .ok_or_else(|| std::io::Error::other("child disk requirement is reported"))?;
    assert_eq!(misplaced_child.state, DiskAuditState::AmbiguousLocation);
    std::fs::remove_file(misplaced_path)?;

    let media_path = source_dir.path().join("grandparent/ancestor-disk.chd");
    std::fs::create_dir_all(
        media_path
            .parent()
            .ok_or_else(|| std::io::Error::other("grandparent media path has no parent"))?,
    )?;
    std::fs::write(media_path, b"ancestor disk")?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-software-clones".to_owned(),
            source_path,
        },
    )?;
    let child = report
        .disks
        .iter()
        .find(|entry| entry.item_name.as_deref() == Some("child"))
        .ok_or_else(|| std::io::Error::other("child disk requirement is reported"))?;
    assert_eq!(child.disk_name, "child-disk");
    assert_eq!(child.state, DiskAuditState::UnverifiedContainer);
    Ok(())
}

#[test]
fn disk_audit_does_not_assign_same_basename_from_unrelated_set() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    let unrelated = source_dir.path().join("unrelated_set/demo_disk.chd");
    std::fs::create_dir_all(
        unrelated
            .parent()
            .ok_or_else(|| std::io::Error::other("unrelated set path has no parent"))?,
    )?;
    std::fs::write(unrelated, b"unrelated container")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-fixture".to_owned(),
            source_path,
        },
    )?;
    assert_eq!(report.disks[0].state, DiskAuditState::AmbiguousLocation);
    Ok(())
}

#[test]
fn disk_audit_respects_case_sensitive_catalog_locations() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    let wrong_case = source_dir.path().join("Demo_Machine/demo_disk.chd");
    std::fs::create_dir_all(
        wrong_case
            .parent()
            .ok_or_else(|| std::io::Error::other("wrong-case set path has no parent"))?,
    )?;
    std::fs::write(wrong_case, b"container in wrong-case directory")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-fixture".to_owned(),
            source_path,
        },
    )?;
    assert_eq!(report.disks[0].state, DiskAuditState::AmbiguousLocation);
    Ok(())
}

#[test]
fn disk_audit_accepts_case_insensitive_chd_extension() -> mame_coalesce::Result<()> {
    let (_db_dir, database, source_dir) = setup()?;
    let chd_path = source_dir.path().join("demo_machine/demo_disk.CHD");
    std::fs::create_dir_all(
        chd_path
            .parent()
            .ok_or_else(|| std::io::Error::other("CHD path has no parent"))?,
    )?;
    std::fs::write(chd_path, b"CHD container")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-fixture".to_owned(),
            source_path,
        },
    )?;
    assert_eq!(report.disks[0].state, DiskAuditState::UnverifiedContainer);
    Ok(())
}

#[test]
fn disk_audit_follows_explicit_machine_clone_parent_layout() -> mame_coalesce::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(db_dir.path().join("parent-audit.db"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let database = Database::open(&database_path)?;
    let temp = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::from_path_buf(temp.path().join("clone.xml"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    std::fs::write(
        &document_path,
        br#"<mame><machine name="parent"><description>Parent</description><disk name="parent-media" sha1="1123456789abcdef0123456789abcdef01234567"/></machine><machine name="clone" cloneof="parent"><description>Clone</description><disk name="clone-media" merge="parent-media" sha1="2123456789abcdef0123456789abcdef01234567"/></machine></mame>"#,
    )?;
    let mut request = mame_request()?;
    request.document_path = document_path;
    request.catalog_key = CatalogKey::new("disk-audit-clone-fixture");
    app::import_catalog(&database, &request)?;
    let source_dir = tempfile::tempdir()?;
    let inherited = source_dir.path().join("parent/parent-media.chd");
    std::fs::create_dir_all(
        inherited
            .parent()
            .ok_or_else(|| std::io::Error::other("parent set path has no parent"))?,
    )?;
    std::fs::write(inherited, b"parent media")?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-clone-fixture".to_owned(),
            source_path,
        },
    )?;
    let clone = report
        .disks
        .iter()
        .find(|entry| entry.set_name == "clone")
        .ok_or_else(|| std::io::Error::other("clone disk is reported"))?;
    assert_eq!(clone.state, DiskAuditState::UnverifiedContainer);
    Ok(())
}

#[test]
fn disk_audit_uses_last_publication_by_insertion_order() -> mame_coalesce::Result<()> {
    let db_dir = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(db_dir.path().join("publication-order.db"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let database = Database::open(&database_path)?;
    app::import_catalog(&database, &mame_request()?)?;
    {
        let mut conn = SqliteConnection::establish(database_path.as_str())
            .map_err(|error| mame_coalesce::Error::InvalidPath(error.to_string()))?;
        sql_query("DROP TRIGGER snapshot_publications_are_immutable_update").execute(&mut conn)?;
        sql_query(
            "UPDATE snapshot_publications SET published_at = '9999-01-01 00:00:00' \
             WHERE catalog_key = 'disk-audit-fixture'",
        )
        .execute(&mut conn)?;
    }

    let temp = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::from_path_buf(temp.path().join("newer.xml"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    std::fs::write(
        &document_path,
        b"<mame><machine name=\"new_machine\"><description>New machine</description><year>2001</year><manufacturer>Example</manufacturer></machine></mame>",
    )?;
    let mut newer_request = mame_request()?;
    newer_request.document_path = document_path;
    app::import_catalog(&database, &newer_request)?;

    let source_dir = tempfile::tempdir()?;
    let source_path = Utf8PathBuf::from_path_buf(source_dir.path().to_owned())
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let report = app::audit_disks(
        &database,
        &DiskAuditRequest {
            catalog_key: "disk-audit-fixture".to_owned(),
            source_path,
        },
    )?;
    assert!(report.disks.is_empty());
    Ok(())
}
