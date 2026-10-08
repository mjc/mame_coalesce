mod attributes;
mod data_file;
mod dtd15;
mod game;
mod header;
mod reader;
mod rom;

/// Select the compatibility or pinned DTD 1.5 interpretation of Logiqx XML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogiqxMode {
    /// Preserve the repository's historical `logiqx-declared-text-compat-v2` behavior.
    ObservedCompatible,
    /// Apply the grammar and attribute rules from the pinned Logiqx DTD 1.5.
    StrictDtd15,
}

pub use crate::xml_reader::{AttributeLocation, AttributePosition, SourceExtent};
pub use attributes::{
    BiosSetAttribute, ClrMameProAttribute, DiskAttribute, DocumentAttribute, GameAttribute,
    NameAttribute, ReleaseAttribute, RomAttribute, RomCenterAttribute,
};
pub use data_file::RecordLocation;
pub use data_file::{DataFile, DocumentMetadata, UnsupportedAttribute};
pub use game::{
    Archive, BiosSet, Game, GameTextField, GameTextPosition, NativeComment, Release, Sample,
};
pub use header::{
    ClrMameProOptions, Header, HeaderTextField, HeaderTextPosition, RomCenterOptions,
};
pub use reader::{
    DocumentAttributes, LocatedGame, LogiqxCaptureProof, ValidatedLogiqx, read_with, read_with_mode,
};
pub use rom::{Disk, Rom};
