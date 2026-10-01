//! No-Intro flat DAT declarations. Source text is retained independently of matching evidence.

use crate::{logiqx::RecordLocation, xml_reader::DeclaredText};

mod reader;
pub use reader::{ValidatedNoIntroDat, read_with};

/// Producer schema and the contract used to interpret its declarations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoIntroDatMode {
    V3Strict,
    V3Compatible,
    V4Strict,
    V4Compatible,
}

impl NoIntroDatMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V3Strict => "no-intro-dat-v3-strict",
            Self::V3Compatible => "no-intro-dat-v3-compatible",
            Self::V4Strict => "no-intro-dat-v4-strict",
            Self::V4Compatible => "no-intro-dat-v4-compatible",
        }
    }

    const fn is_strict(self) -> bool {
        matches!(self, Self::V3Strict | Self::V4Strict)
    }

    const fn revision(self) -> u8 {
        match self {
            Self::V3Strict | Self::V3Compatible => 3,
            Self::V4Strict | Self::V4Compatible => 4,
        }
    }
}

#[derive(Debug)]
pub struct Document {
    pub schema_location: Option<String>,
    pub location: RecordLocation,
    pub header: Header,
}

#[derive(Debug)]
pub struct Header {
    pub location: RecordLocation,
    pub source_order: usize,
    pub id: Option<DeclaredText>,
    pub name: Option<DeclaredText>,
    pub description: Option<DeclaredText>,
    pub version: Option<DeclaredText>,
    pub date: Option<DeclaredText>,
    pub author: Option<DeclaredText>,
    pub homepage: Option<DeclaredText>,
    pub url: Option<DeclaredText>,
    pub trademarks: Option<DeclaredText>,
    pub piracy: Option<DeclaredText>,
    pub subset: Option<DeclaredText>,
    pub comment: Option<DeclaredText>,
    pub clrmamepro: Option<ClrMameProOptions>,
    pub romcenter: Option<RomCenterOptions>,
}

#[derive(Debug)]
pub struct ClrMameProOptions {
    pub source_order: usize,
    pub location: RecordLocation,
    pub forcenodump: Option<DeclaredText>,
    pub header: Option<DeclaredText>,
}

#[derive(Debug)]
pub struct RomCenterOptions {
    pub source_order: usize,
    pub location: RecordLocation,
    pub plugin: Option<DeclaredText>,
}

#[derive(Debug)]
pub struct Game {
    /// Zero-based game-family occurrence, used by the common catalog set anchor.
    pub list_order: usize,
    /// Root-local child-element ordinal, including the header/vendor children.
    pub source_order: usize,
    pub location: RecordLocation,
    pub name: DeclaredText,
    pub id: Option<DeclaredText>,
    pub cloneof: Option<DeclaredText>,
    pub cloneofid: Option<DeclaredText>,
    pub description: Option<DeclaredText>,
    pub categories: Vec<DeclaredText>,
    pub identifiers: Vec<DeclaredText>,
    pub releases: Vec<Release>,
    pub roms: Vec<Rom>,
}

#[derive(Debug)]
pub struct Release {
    pub source_order: usize,
    pub location: RecordLocation,
    pub name: DeclaredText,
    pub region: DeclaredText,
}

#[derive(Debug)]
pub struct Rom {
    pub source_order: usize,
    pub location: RecordLocation,
    pub name: DeclaredText,
    pub size: Option<DeclaredText>,
    pub crc: Option<DeclaredText>,
    pub md5: Option<DeclaredText>,
    pub sha1: Option<DeclaredText>,
    pub sha256: Option<DeclaredText>,
    pub status: Option<DeclaredText>,
    pub serial: Option<DeclaredText>,
    pub header: Option<DeclaredText>,
    pub date: Option<DeclaredText>,
    pub mia: Option<DeclaredText>,
}
