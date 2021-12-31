use super::Rom;

#[derive(Debug, Deserialize)]
pub struct Game {
    pub name: String,
    #[serde(default)]
    pub sourcefile: String,
    #[serde(default)]
    pub isbios: String,
    #[serde(default)]
    pub cloneof: String,
    #[serde(default)]
    pub romof: String,
    #[serde(default)]
    pub sampleof: String,
    #[serde(default)]
    pub board: String,
    #[serde(default)]
    pub rebuildto: String,
    #[serde(default)]
    pub year: String, // should probably be a DateTime
    #[serde(default)]
    pub manufacturer: String,
    #[serde(rename = "rom", default)]
    pub roms: Vec<Rom>,
}

impl Game {
    /// Get a reference to the game's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the game's sourcefile.
    pub fn sourcefile(&self) -> &str {
        self.sourcefile.as_ref()
    }

    /// Get a reference to the game's isbios.
    pub fn isbios(&self) -> &str {
        self.isbios.as_ref()
    }

    /// Get a reference to the game's cloneof.
    pub fn cloneof(&self) -> &str {
        self.cloneof.as_ref()
    }

    /// Get a reference to the game's romof.
    pub fn romof(&self) -> &str {
        self.romof.as_ref()
    }

    /// Get a reference to the game's sampleof.
    pub fn sampleof(&self) -> &str {
        self.sampleof.as_ref()
    }

    /// Get a reference to the game's board.
    pub fn board(&self) -> &str {
        self.board.as_ref()
    }

    /// Get a reference to the game's rebuildto.
    pub fn rebuildto(&self) -> &str {
        self.rebuildto.as_ref()
    }

    /// Get a reference to the game's year.
    pub fn year(&self) -> &str {
        self.year.as_ref()
    }

    /// Get a reference to the game's manufacturer.
    pub fn manufacturer(&self) -> &str {
        self.manufacturer.as_ref()
    }

    /// Get a reference to the game's roms.
    pub fn roms(&self) -> &[Rom] {
        self.roms.as_ref()
    }
}
