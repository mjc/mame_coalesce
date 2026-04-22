use serde::Deserialize;

use super::Rom;

#[derive(Debug, Deserialize)]
pub struct Game {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@sourcefile", default)]
    sourcefile: String,
    #[serde(rename = "@isbios", default)]
    isbios: String,
    #[serde(rename = "@cloneof", default)]
    cloneof: Option<String>,
    #[serde(rename = "@romof", default)]
    romof: String,
    #[serde(rename = "@sampleof", default)]
    sampleof: String,
    #[serde(rename = "@board", default)]
    board: String,
    #[serde(rename = "@rebuildto", default)]
    rebuildto: String,
    #[serde(default)]
    year: String, // should probably be a DateTime
    #[serde(default)]
    manufacturer: String,
    #[serde(rename = "rom", default)]
    roms: Vec<Rom>,
}

impl Game {
    /// Get a reference to the game's name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the game's sourcefile.
    #[must_use]
    pub fn sourcefile(&self) -> &str {
        self.sourcefile.as_ref()
    }

    /// Get a reference to the game's isbios.
    #[must_use]
    pub fn isbios(&self) -> &str {
        self.isbios.as_ref()
    }

    /// Get a reference to the game's romof.
    #[must_use]
    pub fn romof(&self) -> &str {
        self.romof.as_ref()
    }

    /// Get a reference to the game's sampleof.
    #[must_use]
    pub fn sampleof(&self) -> &str {
        self.sampleof.as_ref()
    }

    /// Get a reference to the game's board.
    #[must_use]
    pub fn board(&self) -> &str {
        self.board.as_ref()
    }

    /// Get a reference to the game's rebuildto.
    #[must_use]
    pub fn rebuildto(&self) -> &str {
        self.rebuildto.as_ref()
    }

    /// Get a reference to the game's year.
    #[must_use]
    pub fn year(&self) -> &str {
        self.year.as_ref()
    }

    /// Get a reference to the game's manufacturer.
    #[must_use]
    pub fn manufacturer(&self) -> &str {
        self.manufacturer.as_ref()
    }

    /// Get a reference to the game's roms.
    #[must_use]
    pub fn roms(&self) -> &[Rom] {
        self.roms.as_ref()
    }

    /// Get a reference to the game's cloneof.
    #[must_use]
    pub const fn cloneof(&self) -> Option<&String> {
        self.cloneof.as_ref()
    }
}
