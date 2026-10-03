//! Closed fields of the application's synthetic P/C grammar.

use crate::xml_reader::attribute_fields;

attribute_fields!(GameAttribute {
    Name = 0 => "name",
    Id = 1 => "id",
    NameAlt = 2 => "namealt",
    Region = 3 => "region",
    Languages = 4 => "languages",
    Version = 5 => "version",
    Bios = 6 => "bios",
    Clone = 7 => "clone",
    MergeOf = 8 => "mergeof",
});

attribute_fields!(RomAttribute {
    Name = 0 => "name",
    Size = 1 => "size",
    Crc = 2 => "crc",
    Md5 = 3 => "md5",
    Sha1 = 4 => "sha1",
});
