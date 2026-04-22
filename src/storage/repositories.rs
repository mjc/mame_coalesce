use crate::{
    logiqx,
    storage::{
        db::{self, Pool},
        models::NewRomFile,
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

    pub fn import_rom_files(&self, rom_files: &[NewRomFile]) -> diesel::QueryResult<usize> {
        db::import_rom_files(self.pool, rom_files)
    }
}
