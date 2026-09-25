use diesel::{
    dsl::sql,
    prelude::*,
    sql_types::{Bool, Text},
};

use crate::{
    domain::{
        CatalogKey, CompleteSourceScan, Crc32Digest, DatRom, EvidenceProvenance, EvidenceScope,
        ExpectedEvidence, Md5Digest, ObservedContent, RequirementKey, ScanProvenance, ScanRunKey,
        SetKey, SetMetadata, SourceFile, SourceFingerprint, SourceLocation, SourceRoot,
    },
    hashes::Sha1Digest,
    storage::{
        db::Pool,
        models::{DataFile, NewRomFile, RomFile},
        schema,
    },
};

#[cfg(test)]
use crate::{logiqx, storage::db};

#[cfg(test)]
pub struct DatRepository<'pool> {
    pool: &'pool Pool,
}

#[cfg(test)]
impl<'pool> DatRepository<'pool> {
    #[must_use]
    pub const fn new(pool: &'pool Pool) -> Self {
        Self { pool }
    }

    pub fn import(&self, data_file: &logiqx::DataFile) -> crate::Result<i32> {
        db::traverse_and_insert_data_file(self.pool, data_file)
    }
}

pub struct SourceRepository<'pool> {
    pool: &'pool Pool,
}

impl<'pool> SourceRepository<'pool> {
    #[must_use]
    pub const fn new(pool: &'pool Pool) -> Self {
        Self { pool }
    }

    pub fn replace_completed_scan(&self, scan: &CompleteSourceScan) -> crate::Result<usize> {
        let rows = scan
            .observations()
            .iter()
            .map(NewRomFile::from_observation)
            .collect::<crate::Result<Vec<_>>>()?;
        crate::storage::db::replace_rom_files_for_source_root(
            self.pool,
            camino::Utf8Path::new(scan.source_root().as_str()),
            &rows,
        )
    }

    pub fn load_source_files_for_root(
        &self,
        source_root: &SourceRoot,
    ) -> crate::Result<Vec<SourceFile>> {
        let mut conn = self.pool.get()?;
        let root = source_root.as_str();
        let mut escaped_root = String::with_capacity(root.len());
        for character in root.chars() {
            match character {
                '*' => escaped_root.push_str("[*]"),
                '?' => escaped_root.push_str("[?]"),
                '[' => escaped_root.push_str("[[]"),
                ']' => escaped_root.push_str("[]]"),
                _ => escaped_root.push(character),
            }
        }
        let pattern = if root == "/" {
            "/*".to_owned()
        } else {
            format!("{escaped_root}/*")
        };
        let within_root = sql::<Bool>("path = ")
            .bind::<Text, _>(root)
            .or(sql::<Bool>("path GLOB ").bind::<Text, _>(pattern));
        schema::rom_files::dsl::rom_files
            .filter(within_root)
            .load::<RomFile>(&mut conn)?
            .into_iter()
            .map(source_file_from_model)
            .collect()
    }
}

pub struct BuildRepository<'pool> {
    pool: &'pool Pool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFileSelector<'a> {
    FileName(&'a str),
    Name(&'a str),
}

impl<'a> DataFileSelector<'a> {
    #[must_use]
    const fn value(self) -> &'a str {
        match self {
            Self::FileName(value) | Self::Name(value) => value,
        }
    }
}

impl<'pool> BuildRepository<'pool> {
    #[must_use]
    pub const fn new(pool: &'pool Pool) -> Self {
        Self { pool }
    }

    pub fn load_dat_roms(&self, selector: DataFileSelector<'_>) -> crate::Result<Vec<DatRom>> {
        let mut conn = self.pool.get()?;
        let data_file = match selector {
            DataFileSelector::FileName(value) => schema::data_files::dsl::data_files
                .filter(schema::data_files::dsl::file_name.eq(value))
                .first::<DataFile>(&mut conn)?,
            DataFileSelector::Name(value) => schema::data_files::dsl::data_files
                .filter(schema::data_files::dsl::name.eq(value))
                .first::<DataFile>(&mut conn)?,
        };
        let dat_name = selector.value().to_owned();

        let rows = schema::games::dsl::games
            .filter(schema::games::dsl::data_file_id.eq(data_file.id))
            .inner_join(schema::roms::dsl::roms)
            .load::<(crate::storage::models::Game, crate::storage::models::Rom)>(&mut conn)?;

        let catalog_key = CatalogKey::fresh();
        rows.into_iter()
            .map(|(game, rom)| {
                let size = u64::try_from(rom.size).map_err(|_| {
                    crate::Error::InvalidRomSize(u64::from(rom.size.unsigned_abs()))
                })?;
                let expected = ExpectedEvidence {
                    scope: EvidenceScope::WholeAsset,
                    provenance: EvidenceProvenance::LegacyCache,
                    size: Some(size),
                    crc: digest_from_db::<4>(Some(rom.crc), "roms.crc", &rom.name)?
                        .map(Crc32Digest),
                    md5: digest_from_db::<16>(Some(rom.md5), "roms.md5", &rom.name)?.map(Md5Digest),
                    sha1: digest_from_db(Some(rom.sha1), "roms.sha1", &rom.name)?,
                    merge: None,
                    dump_status: None,
                    serial: None,
                    date: None,
                };
                Ok(DatRom {
                    catalog_name: dat_name.clone(),
                    key: RequirementKey::new(SetKey::new(catalog_key.clone(), game.name), rom.name),
                    parent_name: game.clone_of,
                    set_metadata: SetMetadata {
                        is_bios: game.is_bios,
                        rom_of: game.rom_of,
                        sample_of: game.sample_of,
                        board: game.board,
                        year: game.year,
                        manufacturer: game.manufacturer,
                        ..SetMetadata::default()
                    },
                    role: crate::domain::AssetRole::Rom,
                    component_order: None,
                    expected,
                })
            })
            .collect()
    }
}

fn source_file_from_model(rom_file: RomFile) -> crate::Result<SourceFile> {
    let location = source_location_from_model(&rom_file)?;
    let sha1 = sha1_digest_from_db(rom_file.sha1, "rom_files.sha1", &rom_file.name)?;
    let xxh3 = digest_from_db::<8>(Some(rom_file.xxhash3), "rom_files.xxhash3", &rom_file.name)?
        .ok_or_else(|| {
            crate::Error::InvalidHash(format!(
                "rom_files.xxhash3 for {} is missing",
                rom_file.name
            ))
        })?;
    let source_size = rom_file
        .observed_size
        .map(|size| {
            u64::try_from(size).map_err(|_| crate::Error::InvalidRomSize(size.unsigned_abs()))
        })
        .transpose()?;
    let fingerprint = digest_from_db::<20>(
        rom_file.source_fingerprint,
        "rom_files.source_fingerprint",
        &rom_file.name,
    )?
    .map(SourceFingerprint::new);
    let scan_run = match rom_file.scan_run.as_deref() {
        Some(key) => Some(ScanRunKey::from_storage_key(key).ok_or_else(|| {
            crate::Error::InvalidPath(format!(
                "source {} has an invalid scan run key",
                rom_file.path
            ))
        })?),
        None => None,
    };
    let scan_provenance = match rom_file.scan_provenance.as_deref() {
        Some(value) => Some(ScanProvenance::from_storage_key(value).ok_or_else(|| {
            crate::Error::InvalidPath(format!(
                "source {} has unknown scan provenance",
                rom_file.path
            ))
        })?),
        None => None,
    };
    if rom_file.scan_root.is_some() != scan_run.is_some()
        || scan_run.is_some() != fingerprint.is_some()
        || fingerprint.is_some() != scan_provenance.is_some()
    {
        return Err(crate::Error::InvalidPath(format!(
            "source {} has incomplete scan provenance",
            rom_file.path
        )));
    }
    let content_provenance = if scan_provenance.is_some() {
        EvidenceProvenance::Computed
    } else {
        EvidenceProvenance::Unknown
    };
    Ok(SourceFile {
        source_root: rom_file.scan_root.unwrap_or(rom_file.parent_path),
        location,
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: content_provenance,
            size: source_size,
            crc: None,
            md5: None,
            sha1: Some(sha1),
            xxh3,
        },
        fingerprint,
        scan_run,
        scan_provenance,
    })
}

fn source_location_from_model(rom_file: &RomFile) -> crate::Result<SourceLocation> {
    if !rom_file.in_archive {
        if rom_file.archive_backend.is_some() || rom_file.archive_member_index.is_some() {
            return Err(crate::Error::InvalidPath(format!(
                "bare source {} has archive member identity",
                rom_file.path
            )));
        }
        return Ok(SourceLocation::BareFile {
            path: rom_file.path.clone(),
        });
    }
    match (&rom_file.archive_backend, rom_file.archive_member_index) {
        (Some(backend), Some(index)) => {
            let backend =
                crate::domain::ArchiveBackend::from_storage_key(backend).ok_or_else(|| {
                    crate::Error::InvalidPath(format!(
                        "archive source {} has unknown backend {backend:?}",
                        rom_file.path
                    ))
                })?;
            let index = u64::try_from(index).map_err(|_| {
                crate::Error::InvalidPath(format!(
                    "archive source {} has negative member index {index}",
                    rom_file.path
                ))
            })?;
            Ok(SourceLocation::ArchiveMember {
                path: rom_file.path.clone(),
                backend,
                selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                    index,
                    name: rom_file.name.clone(),
                },
            })
        }
        (None, None) => Ok(SourceLocation::LegacyUnknown {
            path: rom_file.path.clone(),
            member_name: Some(rom_file.name.clone()),
        }),
        _ => Err(crate::Error::InvalidPath(format!(
            "archive source {} has incomplete member identity",
            rom_file.path
        ))),
    }
}

fn sha1_digest_from_db(bytes: Vec<u8>, column: &str, label: &str) -> crate::Result<Sha1Digest> {
    digest_from_db::<20>(Some(bytes), column, label)?
        .ok_or_else(|| crate::Error::InvalidHash(format!("{column} for {label} is missing")))
}

fn digest_from_db<const N: usize>(
    bytes: Option<Vec<u8>>,
    column: &str,
    label: &str,
) -> crate::Result<Option<[u8; N]>> {
    bytes.map_or(Ok(None), |bytes| {
        let len = bytes.len();
        bytes.try_into().map(Some).map_err(|_| {
            crate::Error::InvalidHash(format!(
                "{column} for {label} has length {len}; expected {N} bytes"
            ))
        })
    })
}

#[cfg(test)]
mod tests {
    use diesel::{RunQueryDsl, sql_query};
    use proptest::prelude::*;

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
            rom_id: None,
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
  <game name="repo-game">
    <rom name="repo.rom" size="3" sha1="a9993e364706816aba3e25717850c26c9cd0d89d" md5="900150983cd24fb0d6963f7d28e17f72" crc="12345678"/>
  </game>
</datafile>"#;

    #[test]
    fn repositories_import_and_load_models() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp_dir, pool) = file_backed_pool()?;
        let data_file = logiqx::DataFile::from_reader(SIMPLE_DAT.as_bytes())?;
        let data_file_id = DatRepository::new(&pool).import(&data_file)?;
        assert!(data_file_id > 0);

        let run = ScanRunKey::fresh();
        let root = SourceRoot::new("/source");
        let content_sha1 = crate::hashes::sha1_bytes(b"abc");
        let observation = |location| crate::domain::SourceObservation {
            source_root: root.clone(),
            scan_run: run,
            location,
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

        let dat_roms =
            BuildRepository::new(&pool).load_dat_roms(DataFileSelector::Name("Repository Test"))?;
        let source_files = SourceRepository::new(&pool).load_source_files_for_root(&root)?;

        assert_eq!(dat_roms.len(), 1);
        assert_eq!(dat_roms[0].rom_name(), "repo.rom");
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
    fn load_dat_roms_rejects_invalid_sha1_length() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp_dir, pool) = file_backed_pool()?;
        let mut conn = pool.get()?;
        sql_query("INSERT INTO data_files (id, name) VALUES (1, 'Bad DAT')").execute(&mut conn)?;
        sql_query("INSERT INTO games (id, name, data_file_id) VALUES (1, 'bad-game', 1)")
            .execute(&mut conn)?;
        sql_query(
            r"
            INSERT INTO roms (
                name,
                size,
                md5,
                sha1,
                crc,
                game_id
            )
            VALUES (
                'bad.rom',
                1,
                x'00000000000000000000000000000000',
                x'0102',
                x'00000000',
                1
            )
            ",
        )
        .execute(&mut conn)?;

        let repository = BuildRepository::new(&pool);
        let Err(error) = repository.load_dat_roms(DataFileSelector::Name("Bad DAT")) else {
            return Err("expected invalid SHA1 length to fail".into());
        };

        match error {
            crate::Error::InvalidHash(message) => {
                assert!(message.contains("roms.sha1"));
                assert!(message.contains("bad.rom"));
                assert!(message.contains("length 2"));
            }
            other => return Err(format!("expected InvalidHash error, got {other}").into()),
        }
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
}
