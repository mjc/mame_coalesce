use std::fs::create_dir_all;

use camino::{Utf8Path, Utf8PathBuf};
use diesel::Connection;
use log::info;

use crate::{operations::destination::write_all_zips, MameResult};

pub fn rename_roms(
    conn: &mut impl Connection<Backend = diesel::sqlite::Sqlite>,
    data_file: &Utf8Path,
    dry_run: bool,
    destination: &Utf8Path,
) -> MameResult<Vec<Utf8PathBuf>> {
    let games = crate::db::load_parents(conn, data_file)?;
    info!(
        "Processing {} games with {} matching rom files",
        games.len(),
        games
            .iter()
            .map(|(_rom, rom_files)| { rom_files.len() as u64 })
            .sum::<u64>()
    );

    if dry_run {
        info!("Dry run enabled, not writing zips!");
        Ok(Vec::new())
    } else {
        info!("Saving zips to path: {}", &destination);

        create_dir_all(&destination)?;
        Ok(write_all_zips(&games, destination))
    }
}
