//! Closed fields of pinned Logiqx DTD 1.5 and explicitly named compatibility.

use crate::xml_reader::attribute_fields;

attribute_fields!(DocumentAttribute {
    Build = 0 => "build",
    Debug = 1 => "debug",
});

attribute_fields!(ClrMameProAttribute {
    Header = 0 => "header",
    ForceMerging = 1 => "forcemerging",
    ForceNoDump = 2 => "forcenodump",
    ForcePacking = 3 => "forcepacking",
});

attribute_fields!(RomCenterAttribute {
    Plugin = 0 => "plugin",
    RomMode = 1 => "rommode",
    BiosMode = 2 => "biosmode",
    SampleMode = 3 => "samplemode",
    LockRomMode = 4 => "lockrommode",
    LockBiosMode = 5 => "lockbiosmode",
    LockSampleMode = 6 => "locksamplemode",
});

attribute_fields!(GameAttribute {
    Name = 0 => "name",
    SourceFile = 1 => "sourcefile",
    IsBios = 2 => "isbios",
    CloneOf = 3 => "cloneof",
    RomOf = 4 => "romof",
    SampleOf = 5 => "sampleof",
    Board = 6 => "board",
    RebuildTo = 7 => "rebuildto",
});

attribute_fields!(ReleaseAttribute {
    Name = 0 => "name",
    Region = 1 => "region",
    Language = 2 => "language",
    Date = 3 => "date",
    Default = 4 => "default",
});

attribute_fields!(BiosSetAttribute {
    Name = 0 => "name",
    Description = 1 => "description",
    Default = 2 => "default",
});

attribute_fields!(RomAttribute {
    Name = 0 => "name",
    Size = 1 => "size",
    Crc = 2 => "crc",
    Sha1 = 3 => "sha1",
    Md5 = 4 => "md5",
    Merge = 5 => "merge",
    Status = 6 => "status",
    Date = 7 => "date",
    CompatibilitySerial = 8 => "serial",
});

attribute_fields!(DiskAttribute {
    Name = 0 => "name",
    Sha1 = 1 => "sha1",
    Md5 = 2 => "md5",
    Merge = 3 => "merge",
    Status = 4 => "status",
});

attribute_fields!(NameAttribute {
    Name = 0 => "name",
});
