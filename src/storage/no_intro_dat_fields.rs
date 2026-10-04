//! Closed native field codes shared by flat No-Intro DAT import and query.

use super::native_fields::field_enum;

field_enum!(NoIntroDatHeaderField {
    Id=0, Name=1, Description=2, Version=3, Date=4, Author=5, Homepage=6,
    Url=7, Trademarks=8, Piracy=9, Subset=10, Comment=11
});
field_enum!(NoIntroDatClrMameProField { ForceNoDump=0, Header=1 });
field_enum!(NoIntroDatRomCenterField { Plugin=0 });
field_enum!(NoIntroDatGameField { Name=0, Id=1, CloneOf=2, CloneOfId=3, Description=4 });
field_enum!(NoIntroDatRomField {
    Name=0, Size=1, Crc=2, Md5=3, Sha1=4, Sha256=5, Status=6, Serial=7,
    Header=8, Date=9, Mia=10
});

/// Whether this DAT declaration describes a whole file or an unspecified scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoIntroDatEvidenceScope {
    WholeFile,
    Unknown,
}

impl NoIntroDatEvidenceScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::WholeFile => "whole_file",
            Self::Unknown => "unknown",
        }
    }
}
