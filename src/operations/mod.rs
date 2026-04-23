use camino::Utf8Path;
use log::info;

use crate::{
    logiqx,
    storage::db::{self, Pool},
};

mod scan;

pub use scan::source;

pub fn parse_and_insert_datfile(path: &Utf8Path, pool: &Pool) -> crate::Result<i32> {
    info!("Using datafile: {}", &path);
    logiqx::DataFile::from_path(path)
        .and_then(|datafile| db::traverse_and_insert_data_file(pool, &datafile))
}
