use crate::models::{Rom, RomFile};

pub struct DestinationBundle {
    archive_path: String,
    destination_name: String,
    source_name: String,
    in_archive: bool,
    game_name: String,
}

impl DestinationBundle {
    pub fn from_rom_and_rom_file(rom: &Rom, rom_file: &RomFile, game_name: &str) -> Self {
        let destination_name = rom.name().to_string();
        let source_name = rom_file.name().to_string();
        let archive_path = rom_file.path().to_string();
        let game_name = game_name.to_string();
        let in_archive = rom_file.in_archive();
        Self {
            archive_path,
            destination_name,
            source_name,
            in_archive,
            game_name,
        }
    }

    /// Get a reference to the destination bundle's archive path.
    pub fn archive_path(&self) -> &str {
        self.archive_path.as_ref()
    }

    /// Get a reference to the destination bundle's destination name.
    pub fn destination_name(&self) -> &str {
        self.destination_name.as_ref()
    }

    /// Get a reference to the destination bundle's source name.
    pub fn source_name(&self) -> &str {
        self.source_name.as_ref()
    }

    /// Get the destination bundle's in archive.
    pub fn in_archive(&self) -> bool {
        self.in_archive
    }
}
