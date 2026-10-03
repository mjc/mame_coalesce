//! Closed fields of the application's synthetic P/C grammar.

macro_rules! attribute_fields {
    ($name:ident { $($variant:ident = $code:literal => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(i64)]
        pub enum $name {
            $($variant = $code),+
        }

        impl $name {
            #[must_use]
            pub const fn code(self) -> i64 {
                self as i64
            }

            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub(crate) fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($wire => Some(Self::$variant)),+,
                    _ => None,
                }
            }

            pub(crate) const fn from_code(code: i64) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant)),+,
                    _ => None,
                }
            }
        }
    };
}

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
