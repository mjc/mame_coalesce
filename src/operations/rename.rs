use std::fs::create_dir_all;

use camino::{Utf8Path, Utf8PathBuf};
use log::info;

use crate::{db::Pool, operations::destination::write_all_zips, MameResult};

pub fn roms(
    pool: &Pool,
    data_file: &Utf8Path,
    dry_run: bool,
    destination: &Utf8Path,
) -> MameResult<Vec<Utf8PathBuf>> {
    let games = crate::db::load_parents(pool, data_file)?;
    info!(
        "Processing {} games with {} matching rom files",
        games.len(),
        games
            .values()
            .map(|rom_files| { rom_files.len() as u64 })
            .sum::<u64>()
    );

    if dry_run {
        info!("Dry run enabled, not writing zips!");
        Ok(Vec::new())
    } else {
        info!("Saving zips to path: {}", &destination);

        create_dir_all(destination)?;
        Ok(write_all_zips(&games, destination))
    }
}
