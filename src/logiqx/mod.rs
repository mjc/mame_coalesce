mod attributes;
mod data_file;
mod game;
mod header;
mod reader;
mod rom;

pub use crate::xml_reader::{AttributeLocation, AttributePosition};
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
pub use reader::{DocumentAttributes, LocatedGame, ValidatedLogiqx, read_with};
pub use rom::{Disk, Rom};
