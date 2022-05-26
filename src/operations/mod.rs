use camino::Utf8Path;
use log::info;

use crate::{
    db::{self, SyncPool},
    logiqx, MameResult,
};

pub mod destination;

mod rename;
pub mod scan;

pub use rename::rename;
pub use scan::scan;

// TODO: this should return a Result
pub fn parse_and_insert_datfile(path: &Utf8Path, pool: &SyncPool) -> MameResult<i32> {
    info!("Using datafile: {}", &path);

    logiqx::DataFile::from_path(path)
        .and_then(|datafile| db::traverse_and_insert_data_file(&mut pool.get()?, &datafile))
}
