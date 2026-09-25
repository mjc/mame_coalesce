use camino::Utf8Path;

use crate::{
    logiqx,
    storage::db::{self, Pool},
};

pub mod scan;

pub fn parse_and_insert_datfile(path: &Utf8Path, pool: &Pool) -> crate::Result<i32> {
    logiqx::DataFile::from_path(path)
        .and_then(|datafile| db::traverse_and_insert_data_file(pool, &datafile))
}
