use diesel::prelude::*;

use crate::{
    domain::{DatRom, SourceFile, SourceKind},
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
        Ok(schema::rom_files::dsl::rom_files
            .load::<RomFile>(&mut conn)?
            .into_iter()
            .map(source_file_from_model)
            .collect())
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

        Ok(rows
            .into_iter()
            .map(|(game, rom)| DatRom {
                dat_name: dat_name.clone(),
                game_name: game.name,
                parent_name: game.clone_of,
                rom_name: rom.name,
                sha1: hex::encode(rom.sha1),
            })
            .collect())
    }
}

fn source_file_from_model(rom_file: RomFile) -> SourceFile {
    let kind = if rom_file.in_archive {
        if std::path::Path::new(&rom_file.path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            SourceKind::ZipEntry
        } else {
            SourceKind::ArchiveEntry
        }
    } else {
        SourceKind::BareFile
    };

    SourceFile {
        source_root: rom_file.parent_path,
        canonical_path: rom_file.path,
        entry_name: rom_file.in_archive.then_some(rom_file.name),
        sha1: hex::encode(rom_file.sha1),
        kind,
    }
}
