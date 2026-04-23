use diesel::prelude::*;

use crate::{
    domain::{DatRom, SourceFile, SourceKind},
    hashes::Sha1Digest,
    logiqx,
    storage::{
        db::{self, Pool},
        models::{DataFile, NewRomFile, RomFile},
        schema,
    },
};

pub struct DatRepository<'pool> {
    pool: &'pool Pool,
}

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

    pub fn import_rom_files(&self, rom_files: &[NewRomFile]) -> crate::Result<usize> {
        db::import_rom_files(self.pool, rom_files)
    }

    pub fn load_source_files(&self) -> crate::Result<Vec<SourceFile>> {
        let mut conn = self.pool.get()?;
        schema::rom_files::dsl::rom_files
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
            .load::<(crate::models::Game, crate::models::Rom)>(&mut conn)?;

        rows.into_iter()
            .map(|(game, rom)| {
                let sha1 = sha1_digest_from_db(rom.sha1, "roms.sha1", &rom.name)?;
                Ok(DatRom {
                    dat_name: dat_name.clone(),
                    game_name: game.name,
                    parent_name: game.clone_of,
                    rom_name: rom.name,
                    sha1,
                })
            })
            .collect()
    }
}

fn source_file_from_model(rom_file: RomFile) -> crate::Result<SourceFile> {
    let kind = source_kind_from_rom_file(&rom_file);
    let sha1 = sha1_digest_from_db(rom_file.sha1, "rom_files.sha1", &rom_file.name)?;

    Ok(SourceFile {
        source_root: rom_file.parent_path,
        canonical_path: rom_file.path,
        entry_name: rom_file.in_archive.then_some(rom_file.name),
        sha1,
        kind,
    })
}

fn source_kind_from_rom_file(rom_file: &RomFile) -> SourceKind {
    if !rom_file.in_archive {
        return SourceKind::BareFile;
    }

    if std::path::Path::new(&rom_file.path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        SourceKind::ZipEntry
    } else {
        SourceKind::ArchiveEntry
    }
}

fn sha1_digest_from_db(bytes: Vec<u8>, column: &str, label: &str) -> crate::Result<Sha1Digest> {
    let len = bytes.len();
    bytes.try_into().map_err(|_| {
        crate::Error::InvalidHash(format!(
            "{column} for {label} has length {len}; expected 20 bytes"
        ))
    })
}

#[cfg(test)]
mod tests {
    use diesel::{RunQueryDsl, sql_query};
    use proptest::prelude::*;

    use super::*;

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

        let rom_file = NewRomFile {
            parent_path: "/source".to_owned(),
            path: "/source/repo.rom".to_owned(),
            name: "repo.rom".to_owned(),
            sha1: crate::hashes::sha1_bytes(b"abc"),
            xxhash3: crate::hashes::xxhash3_bytes(b"abc"),
            in_archive: false,
            rom_id: None,
        };
        let associated = SourceRepository::new(&pool).import_rom_files(&[rom_file])?;
        assert_eq!(associated, 1);

        let dat_roms =
            BuildRepository::new(&pool).load_dat_roms(DataFileSelector::Name("Repository Test"))?;
        let source_files = SourceRepository::new(&pool).load_source_files()?;

        assert_eq!(dat_roms.len(), 1);
        assert_eq!(dat_roms[0].rom_name, "repo.rom");
        assert_eq!(source_files.len(), 1);
        assert_eq!(source_files[0].kind, SourceKind::BareFile);
        Ok(())
    }

    #[test]
    fn source_kind_derives_archive_variants_from_rom_file() {
        let mut rom_file = RomFile {
            id: 1,
            parent_path: "/source".to_owned(),
            parent_game_name: None,
            path: "/source/archive.zip".to_owned(),
            name: "entry.rom".to_owned(),
            crc: None,
            sha1: crate::hashes::sha1_bytes(b"abc").to_vec(),
            md5: None,
            xxhash3: crate::hashes::xxhash3_bytes(b"abc").to_vec(),
            in_archive: true,
            rom_id: None,
        };

        assert_eq!(source_kind_from_rom_file(&rom_file), SourceKind::ZipEntry);
        rom_file.path = "/source/archive.7z".to_owned();
        assert_eq!(
            source_kind_from_rom_file(&rom_file),
            SourceKind::ArchiveEntry
        );
        rom_file.in_archive = false;
        assert_eq!(source_kind_from_rom_file(&rom_file), SourceKind::BareFile);
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
        let Err(error) = repository.load_source_files() else {
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
        let pool = crate::db::create_db_pool(database_url)?;
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
