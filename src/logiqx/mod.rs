use crate::fs;

mod data_file;
mod game;
mod header;
mod rom;

pub use data_file::DataFile;
pub use game::Game;
pub use rom::Rom;

pub fn load_datafile(name: String) -> Result<DataFile, &'static str> {
    match fs::read_to_string(name) {
        Ok(contents) => Ok(DataFile::from_str(&contents)),
        Err(_) => Err("Unable to parse datafile"),
    }
}
