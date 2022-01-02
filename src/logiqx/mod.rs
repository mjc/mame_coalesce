use std::fs::File;

mod data_file;
mod game;
mod header;
mod rom;

pub use data_file::DataFile;
pub use game::Game;
pub use rom::Rom;

// TODO: mmap this for shoots and ladders
pub fn load_datafile(name: &str) -> Option<DataFile> {
    let f = File::open(name).ok()?;
    Some(DataFile::from_file(&f))
}
