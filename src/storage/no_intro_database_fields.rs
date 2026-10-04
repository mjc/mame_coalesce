//! Closed native field-code ledgers shared by No-Intro database import and query.

macro_rules! field_enum {
    ($name:ident { $($field:ident = $code:literal),+ $(,)? }) => {
        #[repr(i64)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($field = $code),+ }

        impl $name {
            pub(crate) const fn from_code(code: i64) -> Option<Self> {
                match code { $($code => Some(Self::$field),)+ _ => None }
            }
        }
    };
}

field_enum!(NoIntroDatabaseArchiveField {
    Additional=0, Adult=1, Aftermarket=2, Alt=3, Bios=4, Categories=5, Complete=6,
    Dat=7, DatterNote=8, Description=9, Devstatus=10, GameId1=11, GameId2=12,
    Langchecked=13, Languages=14, Licensed=15, Listed=16, Mergename=17, Name=18,
    NameAlt=19, Number=20, Physical=21, Region=22, Regparent=23, Showlang=24,
    Special1=25, Special2=26, StickyNote=27, Version1=28, Version2=29, Clone=30,
    MergeOf=31
});

field_enum!(NoIntroDatabaseDumpDetailsField {
    Comment1=0, Comment2=1, DumpDate=2, DumpDateInfo=3, Dumper=4, Id=5, Link1=6,
    Link2=7, Link3=8, MediaTitle=9, NoDump=10, Origin=11, OriginalFormat=12,
    Project=13, ReleaseDate=14, ReleaseDateInfo=15, Region=16, RomInfo=17,
    Section=18, Tool=19
});

field_enum!(NoIntroDatabaseDumpSerialsField {
    BoxBarcode=0, BoxSerial=1, ChipSerial=2, DigitalSerial1=3, DigitalSerial2=4,
    LockoutSerial=5, MediaSerial1=6, MediaSerial2=7, MediaSerial3=8, MediaStamp=9,
    PcbSerial=10, RomChipSerial1=11, RomChipSerial2=12, SaveChipSerial=13
});

field_enum!(NoIntroDatabaseDumpFileField {
    Bad=0, Crc32=1, Date=2, Extension=3, Filter=4, ForceName=5, ForceSceneName=6,
    Format=7, Header=8, Id=9, Item=10, Md5=11, Mia=12, Note=13, OriginSha256=14,
    OriginSize=15, Serial=16, Sha1=17, Sha256=18, Size=19, Unique=20,
    UpdateType=21, Version=22
});

field_enum!(NoIntroDatabaseReleaseDetailsField {
    ArchiveName=0, Category=1, Comment=2, Date=3, Directory=4, Group=5, Id=6,
    NfoCrc32=7, NfoSize=8, NfoCrc=9, NfoName=10, NfoLegacySize=11, Origin=12,
    OriginalFormat=13, Region=14, RomInfo=15, Tool=16
});

field_enum!(NoIntroDatabaseReleaseSerialsField {
    BoxBarcode=0, BoxSerial=1, MediaSerial1=2, MediaStamp=3, PcbSerial=4,
    RomChipSerial1=5
});

field_enum!(NoIntroDatabaseReleaseFileField {
    Bad=0, Crc32=1, Extension=2, ForceName=3, ForceSceneName=4, Format=5, Header=6,
    Id=7, Item=8, Md5=9, Note=10, Serial=11, Sha1=12, Sha256=13, Size=14,
    UpdateType=15, Version=16
});
