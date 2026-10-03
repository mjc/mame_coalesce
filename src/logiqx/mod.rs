mod attributes;
mod data_file;
mod game;
mod header;
mod rom;

pub use crate::xml_reader::{AttributeLocation, AttributePosition};
pub use attributes::{
    BiosSetAttribute, ClrMameProAttribute, DiskAttribute, DocumentAttribute, GameAttribute,
    NameAttribute, ReleaseAttribute, RomAttribute, RomCenterAttribute,
};
pub use data_file::DataFile;
pub use data_file::RecordLocation;
pub(crate) use data_file::XmlSourceMap;
pub use game::{
    Archive, BiosSet, Game, GameTextField, GameTextPosition, NativeComment, Release, Sample,
};
pub use header::{
    ClrMameProOptions, Header, HeaderTextField, HeaderTextPosition, RomCenterOptions,
};
pub use rom::{Disk, Rom};
