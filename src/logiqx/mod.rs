use std::{fs::File, path::Path};

mod data_file;
mod game;
mod header;
mod rom;

pub use data_file::DataFile;
pub use game::Game;
pub use rom::Rom;

// TODO: mmap this for shoots and ladders
// TODO: investigate why Err() is unreachable
pub fn load_datafile(name: &Path) -> Result<DataFile, serde_xml_rs::Error> {
    let f = File::open(name)?;
    DataFile::from_file(&f).map(|mut df| {
        // TODO: ugly
        df.set_file_name(name.to_str().map(|s| s.to_string()));
        df
    })
}
