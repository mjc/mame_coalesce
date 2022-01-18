use std::fs::{self, File};

mod data_file;
mod game;
mod header;
mod rom;

use camino::{Utf8Path, Utf8PathBuf};
pub use data_file::DataFile;
pub use game::Game;
pub use rom::Rom;

// TODO: mmap this for shoots and ladders
// TODO: investigate why Err() is unreachable
pub fn load_datafile(name: &Utf8Path) -> Result<DataFile, serde_xml_rs::Error> {
    let f = File::open(name)?;
    DataFile::from_file(&f).map(|mut df| {
        // TODO: ugly
        let canonicalized = fs::canonicalize(&name).unwrap();
        let full_path = Utf8PathBuf::from_path_buf(canonicalized)
            .map(|p| p.to_string())
            .ok();
        df.set_file_name(full_path);
        df
    })
}
