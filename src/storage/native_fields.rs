//! Declare a closed numeric field ledger without duplicating writer/reader codes.

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

pub(crate) use field_enum;
