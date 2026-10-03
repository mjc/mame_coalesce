use std::collections::BTreeSet;

use diesel::{RunQueryDsl, sql_query};
use proptest::prelude::*;

use crate::{
    app::{CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SetName},
    storage::{
        build_catalog::{BuildCatalogRepository, CatalogSelector},
        db,
    },
};

use super::*;

fn rom_file_location(
    in_archive: bool,
    archive_backend: Option<&str>,
    archive_member_index: Option<i64>,
) -> RomFile {
    RomFile {
        id: 1,
        parent_path: "/roms".to_owned(),
        parent_game_name: None,
        path: "/roms/set.zip".to_owned(),
        name: "nested/game.rom".to_owned(),
        crc: None,
        sha1: vec![0; 20],
        md5: None,
        xxhash3: vec![0; 8],
        in_archive,
        archive_backend: archive_backend.map(str::to_owned),
        archive_member_index,
        scan_root: None,
        scan_run: None,
        observed_size: None,
        source_fingerprint: None,
        scan_provenance: None,
        bare_file_cache_stamp: None,
        cache_reused: false,
        physical_path: None,
    }
}

#[test]
fn source_location_restores_typed_archive_identity() -> crate::Result<()> {
    let location = source_location_from_model(&rom_file_location(true, Some("zip"), Some(3)))?;
    assert_eq!(
        location,
        SourceLocation::ArchiveMember {
            path: "/roms/set.zip".to_owned(),
            backend: crate::domain::ArchiveBackend::Zip,
            selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                index: 3,
                name: "nested/game.rom".to_owned(),
            },
        }
    );
    Ok(())
}

#[test]
fn source_location_keeps_legacy_archive_identity_unknown() -> crate::Result<()> {
    let location = source_location_from_model(&rom_file_location(true, None, None))?;
    assert_eq!(
        location,
        SourceLocation::LegacyUnknown {
            path: "/roms/set.zip".to_owned(),
            member_name: Some("nested/game.rom".to_owned()),
        }
    );
    Ok(())
}

const SIMPLE_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
<name>Repository Test</name>
  </header>
  <game name="repo-game" rebuildto="repo-target">
<rom name="repo.rom" size="3" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#;

#[test]
fn repositories_import_and_load_models() -> Result<(), Box<dyn std::error::Error>> {
    let (temp_dir, pool) = file_backed_pool()?;
    import_test_catalog(&pool, &temp_dir)?;

    let run = ScanRunKey::fresh();
    let root = SourceRoot::new("/source");
    let content_sha1 = crate::hashes::sha1_bytes(b"abc");
    let observation = |location| {
        let physical_path = SourcePhysicalPath::from_location(&location);
        let bare_file_cache_stamp = match &location {
            SourceLocation::BareFile { .. } => Some(BareFileCacheStamp::new([4; 32])),
            _ => None,
        };
        crate::domain::SourceObservation {
            source_root: root.clone(),
            scan_run: run,
            location,
            physical_path,
            observed: ObservedContent {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::Computed,
                size: Some(3),
                crc: None,
                md5: None,
                sha1: Some(content_sha1),
                xxh3: crate::hashes::xxhash3_bytes(b"abc"),
            },
            fingerprint: SourceFingerprint::new([7; 20]),
            scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
            bare_file_cache_stamp,
        }
    };
    let scan = CompleteSourceScan::new(
        root.clone(),
        run,
        vec![
            observation(SourceLocation::BareFile {
                path: "/source/repo.rom".to_owned(),
            }),
            observation(SourceLocation::ArchiveMember {
                path: "/source/repo-game.zip".to_owned(),
                backend: crate::domain::ArchiveBackend::Zip,
                selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                    index: 4,
                    name: "repo.rom".to_owned(),
                },
            }),
        ],
    )?;
    let associated = SourceRepository::new(&pool).replace_completed_scan(&scan)?;
    assert_eq!(associated, 2);

    let build_catalog = BuildCatalogRepository::new(&pool).load(
        &CatalogSelector::LatestCatalog("Repository Test".to_owned()),
    )?;
    let requirements = build_catalog.requirements();
    let source_files = SourceRepository::new(&pool).load_source_files_for_root(&root)?;

    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].rom_name(), "repo.rom");
    assert_eq!(
        requirements[0].set_metadata.rebuild_to.as_deref(),
        Some("repo-target")
    );
    assert_eq!(
        build_catalog.set_names(),
        &BTreeSet::from([SetName::new("repo-game")])
    );
    assert_eq!(
        build_catalog.catalog_key().as_str(),
        "repository-test-catalog"
    );
    assert!(!build_catalog.snapshot_key().as_str().is_empty());
    assert_eq!(source_files.len(), 2);
    assert!(source_files.iter().all(|source| {
        source.observed.size == Some(3)
            && source.fingerprint == Some(SourceFingerprint::new([7; 20]))
            && source.scan_run == Some(run)
            && source.scan_provenance == Some(ScanProvenance::StreamedSha1Xxh3V1)
    }));
    assert!(
        matches!(source_files[0].location, SourceLocation::BareFile { .. })
            || matches!(source_files[1].location, SourceLocation::BareFile { .. })
    );
    assert!(source_files.iter().any(|source| matches!(
        &source.location,
        SourceLocation::ArchiveMember {
            backend: crate::domain::ArchiveBackend::Zip,
            selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                index: 4,
                name,
            },
            ..
        } if name == "repo.rom"
    )));

    Ok(())
}

#[test]
fn scan_metadata_requires_observed_size_on_insert_and_update()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, pool) = file_backed_pool()?;
    let root = SourceRoot::new("/source");
    let run = ScanRunKey::fresh();
    let location = SourceLocation::BareFile {
        path: "/source/game.rom".to_owned(),
    };
    let observation = crate::domain::SourceObservation {
        source_root: root.clone(),
        scan_run: run,
        physical_path: SourcePhysicalPath::from_location(&location),
        location,
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size: Some(3),
            crc: None,
            md5: None,
            sha1: Some(crate::hashes::sha1_bytes(b"abc")),
            xxh3: crate::hashes::xxhash3_bytes(b"abc"),
        },
        fingerprint: SourceFingerprint::new([7; 20]),
        scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
        bare_file_cache_stamp: None,
    };
    let scan = CompleteSourceScan::new(root.clone(), run, vec![observation.clone()])?;
    SourceRepository::new(&pool).replace_completed_scan(&scan)?;

    let mut malformed = NewRomFile::from_observation(&observation)?;
    malformed.observed_size = None;
    let result = db::replace_rom_files_for_source_root(
        &pool,
        camino::Utf8Path::new("/source"),
        &[malformed],
    );
    let Err(error) = result else {
        return Err("insert without observed_size unexpectedly succeeded".into());
    };
    assert!(
        error
            .to_string()
            .contains("scan metadata must be stored together")
    );

    let mut conn = pool.get()?;
    let result = sql_query("UPDATE rom_files SET observed_size = NULL WHERE scan_root = ?")
        .bind::<Text, _>(root.as_str())
        .execute(&mut conn);
    let Err(error) = result else {
        return Err("update without observed_size unexpectedly succeeded".into());
    };
    assert!(
        error
            .to_string()
            .contains("scan metadata must be stored together")
    );
    Ok(())
}

#[test]
fn scan_association_count_ignores_other_source_roots() -> Result<(), Box<dyn std::error::Error>> {
    let (temp_dir, pool) = file_backed_pool()?;
    import_test_catalog(&pool, &temp_dir)?;

    let matching_root = SourceRoot::new("/matching");
    let matching_run = ScanRunKey::fresh();
    let matching_location = SourceLocation::BareFile {
        path: "/matching/repo.rom".to_owned(),
    };
    let matching_observation = crate::domain::SourceObservation {
        source_root: matching_root.clone(),
        scan_run: matching_run,
        physical_path: SourcePhysicalPath::from_location(&matching_location),
        location: matching_location,
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size: Some(3),
            crc: None,
            md5: None,
            sha1: Some(crate::hashes::sha1_bytes(b"abc")),
            xxh3: crate::hashes::xxhash3_bytes(b"abc"),
        },
        fingerprint: SourceFingerprint::new([7; 20]),
        scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
        bare_file_cache_stamp: None,
    };
    let matching_scan =
        CompleteSourceScan::new(matching_root, matching_run, vec![matching_observation])?;
    assert_eq!(
        SourceRepository::new(&pool).replace_completed_scan(&matching_scan)?,
        1
    );

    let other_root = SourceRoot::new("/other");
    let other_run = ScanRunKey::fresh();
    let other_location = SourceLocation::BareFile {
        path: "/other/unmatched.rom".to_owned(),
    };
    let other_observation = crate::domain::SourceObservation {
        source_root: other_root.clone(),
        scan_run: other_run,
        physical_path: SourcePhysicalPath::from_location(&other_location),
        location: other_location,
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size: Some(9),
            crc: None,
            md5: None,
            sha1: Some(crate::hashes::sha1_bytes(b"unmatched")),
            xxh3: crate::hashes::xxhash3_bytes(b"unmatched"),
        },
        fingerprint: SourceFingerprint::new([8; 20]),
        scan_provenance: ScanProvenance::StreamedSha1Xxh3V1,
        bare_file_cache_stamp: None,
    };
    let other_scan = CompleteSourceScan::new(other_root, other_run, vec![other_observation])?;
    assert_eq!(
        SourceRepository::new(&pool).replace_completed_scan(&other_scan)?,
        0
    );
    Ok(())
}

#[test]
fn legacy_archive_models_keep_known_backend_priority() -> crate::Result<()> {
    let archived_file = |path: &str| RomFile {
        id: 1,
        parent_path: "/source".to_owned(),
        parent_game_name: None,
        path: path.to_owned(),
        name: "game.rom".to_owned(),
        crc: None,
        sha1: crate::hashes::sha1_bytes(b"content").to_vec(),
        md5: None,
        xxhash3: crate::hashes::xxhash3_bytes(b"content").to_vec(),
        in_archive: true,
        archive_backend: None,
        archive_member_index: None,
        scan_root: None,
        scan_run: None,
        observed_size: None,
        source_fingerprint: None,
        scan_provenance: None,
        bare_file_cache_stamp: None,
        cache_reused: false,
        physical_path: None,
    };

    let zip = source_location_from_model(&archived_file("/source/z.zip"))?;
    let seven_zip = source_location_from_model(&archived_file("/source/a.7z"))?;

    assert!(zip.priority() < seven_zip.priority());
    Ok(())
}

#[test]
fn source_inventory_query_treats_like_wildcards_as_path_characters()
-> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, pool) = file_backed_pool()?;
    let mut conn = pool.get()?;
    for path in [
        "/source%_dir/inside.rom",
        "/sourceXXdir/false-match.rom",
        "/source%_directory/sibling.rom",
    ] {
        sql_query(
            "INSERT INTO rom_files (parent_path, path, name, sha1, xxhash3, in_archive) VALUES (?, ?, ?, ?, ?, 0)",
        )
        .bind::<diesel::sql_types::Text, _>("/source")
        .bind::<diesel::sql_types::Text, _>(path)
        .bind::<diesel::sql_types::Text, _>(path.rsplit('/').next().unwrap_or(""))
        .bind::<diesel::sql_types::Binary, _>(vec![1; 20])
        .bind::<diesel::sql_types::Binary, _>(vec![2; 8])
        .execute(&mut conn)?;
    }

    let source_files = SourceRepository::new(&pool)
        .load_source_files_for_root(&SourceRoot::new("/source%_dir"))?;
    assert_eq!(source_files.len(), 1);
    assert_eq!(source_files[0].location.path(), "/source%_dir/inside.rom");
    assert_eq!(
        source_files[0].observed.provenance,
        EvidenceProvenance::Unknown
    );
    Ok(())
}

#[test]
fn sha1_digest_from_db_accepts_twenty_byte_input() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = vec![42; 20];
    let digest = sha1_digest_from_db(bytes.clone(), "test.sha1", "exact")?;

    assert_eq!(digest.as_slice(), bytes.as_slice());
    Ok(())
}

#[test]
fn load_source_files_rejects_invalid_sha1_length() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, pool) = file_backed_pool()?;
    let mut conn = pool.get()?;
    sql_query(
        r"
        INSERT INTO rom_files (
            parent_path,
            path,
            name,
            sha1,
            xxhash3,
            in_archive
        )
        VALUES (
            '/source',
            '/source/bad.rom',
            'bad.rom',
            x'0102',
            x'0000000000000000',
            0
        )
        ",
    )
    .execute(&mut conn)?;

    let repository = SourceRepository::new(&pool);
    let Err(error) = repository.load_source_files_for_root(&SourceRoot::new("/source")) else {
        return Err("expected invalid SHA1 length to fail".into());
    };

    match error {
        crate::Error::InvalidHash(message) => {
            assert!(message.contains("rom_files.sha1"));
            assert!(message.contains("bad.rom"));
            assert!(message.contains("length 2"));
        }
        other => return Err(format!("expected InvalidHash error, got {other}").into()),
    }
    Ok(())
}

#[test]
fn digest_values_reject_invalid_sha1_length() -> Result<(), Box<dyn std::error::Error>> {
    let (_temp_dir, pool) = file_backed_pool()?;
    let mut conn = pool.get()?;
    let Err(error) = sql_query("INSERT INTO digest_values (algorithm, digest) VALUES (?, ?)")
        .bind::<diesel::sql_types::Text, _>("sha1")
        .bind::<diesel::sql_types::Binary, _>(vec![1, 2])
        .execute(&mut conn)
    else {
        return Err("expected digest_values SHA-1 length constraint to fail".into());
    };
    assert!(error.to_string().contains("CHECK constraint failed"));
    Ok(())
}

fn import_test_catalog(
    pool: &Pool,
    temp_dir: &tempfile::TempDir,
) -> Result<(), Box<dyn std::error::Error>> {
    let document_path = temp_dir.path().join("repository-test.dat");
    std::fs::write(&document_path, SIMPLE_DAT)?;
    let document_path = camino::Utf8PathBuf::from_path_buf(document_path)
        .map_err(|_| "temporary catalog path is not UTF-8")?;
    let request = CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::Logiqx(crate::logiqx::LogiqxMode::ObservedCompatible),
        source_key: PublishingSourceKey::new("repository-test-source"),
        source_display_name: "Repository test source".to_owned(),
        catalog_key: CatalogKey::new("repository-test-catalog"),
        catalog_display_name: "Repository Test".to_owned(),
        scope: CatalogScope::Unknown,
    };
    let report = crate::storage::catalog_import::import(pool, &request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(())
}

fn file_backed_pool() -> Result<(tempfile::TempDir, Pool), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let database_path = temp_dir.path().join("test.db");
    let database_url = database_path
        .to_str()
        .ok_or("temporary database path is not UTF-8")?;
    let pool = crate::storage::db::create_db_pool(database_url)?;
    Ok((temp_dir, pool))
}

proptest! {
    #[test]
    fn sha1_digest_from_db_accepts_exactly_twenty_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..40)) {
        let result = sha1_digest_from_db(bytes.clone(), "test.sha1", "generated");

        if bytes.len() == 20 {
            let digest = result.map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(digest.as_slice(), bytes.as_slice());
        } else {
            prop_assert!(matches!(result, Err(crate::Error::InvalidHash(_))));
        }
    }
}
