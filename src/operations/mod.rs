use std::fs::create_dir_all;

use camino::{Utf8Path, Utf8PathBuf};
use indicatif::{ProgressBar, ProgressStyle};
use log::info;

use crate::{db::DbPool, operations::destination::write_all_zips, MameResult};

mod destination;

pub fn rename_roms(
    pool: &DbPool,
    data_file: &Utf8Path,
    bar_style: &ProgressStyle,
    dry_run: bool,
    destination: &Utf8Path,
) -> MameResult<Vec<Utf8PathBuf>> {
    let games = crate::db::load_parents(pool, data_file);
    info!(
        "Processing {} games with {} matching rom files",
        games.len(),
        games
            .iter()
            .map(|(_rom, rom_files)| { rom_files.len() as i64 })
            .sum::<i64>()
    );
    let zip_bar = ProgressBar::new(games.len() as u64);
    zip_bar.set_style(bar_style.clone());
    if dry_run {
        info!("Dry run enabled, not writing zips!");
        Ok(Vec::new())
    } else {
        info!("Saving zips to path: {}", &destination);

        create_dir_all(&destination)?;
        Ok(write_all_zips(&games, destination, &zip_bar))
    }
}
