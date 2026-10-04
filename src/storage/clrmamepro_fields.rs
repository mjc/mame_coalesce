//! Native CMP codes and lexical values shared by import and queries.
use super::{catalog_files::SourceLocation, native_fields::field_enum};

field_enum!(ClrMameProHeaderField {
    Name=0, Description=1, Version=2, Date=3, Author=4, Email=5, Homepage=6,
    Url=7, Comment=8, Category=9, HeaderDefinition=10, ForceMerging=11,
    ForceZipping=12, ForcePacking=13, ForceNoDump=14
});
field_enum!(ClrMameProSetField {
    Name=0, CloneOf=1, Description=2, Year=3, Manufacturer=4, RebuildTo=5,
    SampleOf=6, Region=7, ReleaseYear=8, ReleaseMonth=9, ReleaseDay=10, Serial=11
});
field_enum!(ClrMameProRomField {
    Name=0, Size=1, Crc=2, Crc32=3, Md5=4, Sha1=5, Merge=6, Date=7,
    Serial=8, Status=9, NoDump=10, BadDump=11
});

macro_rules! field_keywords {
    ($kind:ident { $($field:ident => $keyword:literal),+ $(,)? }) => {
        impl $kind {
            pub(crate) const fn keyword(self) -> &'static str {
                match self { $(Self::$field => $keyword,)+ }
            }
        }
    };
}
field_keywords!(ClrMameProHeaderField {
    Name=>"name", Description=>"description", Version=>"version", Date=>"date",
    Author=>"author", Email=>"email", Homepage=>"homepage", Url=>"url", Comment=>"comment",
    Category=>"category", HeaderDefinition=>"header", ForceMerging=>"forcemerging",
    ForceZipping=>"forcezipping", ForcePacking=>"forcepacking", ForceNoDump=>"forcenodump"
});
field_keywords!(ClrMameProSetField {
    Name=>"name", CloneOf=>"cloneof", Description=>"description", Year=>"year",
    Manufacturer=>"manufacturer", RebuildTo=>"rebuildto", SampleOf=>"sampleof", Region=>"region",
    ReleaseYear=>"releaseyear", ReleaseMonth=>"releasemonth", ReleaseDay=>"releaseday", Serial=>"serial"
});
field_keywords!(ClrMameProRomField {
    Name=>"name", Size=>"size", Crc=>"crc", Crc32=>"crc32", Md5=>"md5", Sha1=>"sha1",
    Merge=>"merge", Date=>"date", Serial=>"serial", Status=>"status", NoDump=>"nodump", BadDump=>"baddump"
});

/// Original keyword, quotation, item ordinal and decoded DAT token coordinates.
/// Scalars locate their value token; valueless flags locate their keyword.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProFieldPosition {
    pub source_field: String,
    pub source_order: usize,
    pub is_quoted: bool,
    pub location: SourceLocation,
}

/// A decoded scalar and its native lexical provenance, not original bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProDeclaredValue {
    pub value: String,
    pub position: ClrMameProFieldPosition,
}

/// A closed field and its position; the value stays with its native owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClrMameProPositionedField<Field> {
    pub field: Field,
    pub position: ClrMameProFieldPosition,
}

/// Actual CMP evidence scope, distinct from flat-DAT WholeFile/Unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClrMameProEvidenceScope {
    WholeFile,
    WholeAsset,
}

impl ClrMameProEvidenceScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::WholeFile => "whole_file",
            Self::WholeAsset => "whole_asset",
        }
    }
}

/// Positive once-issued native reported relationship key, not a resolved endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClrMameProRelationshipId(i64);

impl ClrMameProRelationshipId {
    pub(crate) const fn from_database(value: i64) -> Option<Self> {
        if value > 0 { Some(Self(value)) } else { None }
    }

    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}
