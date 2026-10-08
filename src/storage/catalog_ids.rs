//! Checked, domain-specific identifiers for the catalog SQLite schema.

use thiserror::Error;

/// The raw SQLite integer was not a valid positive catalog identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CatalogIdError {
    /// Catalog row identifiers must be strictly positive.
    #[error("{id_type} must be positive, got {value}")]
    NonPositive { id_type: &'static str, value: i64 },
}

impl From<CatalogIdError> for crate::Error {
    fn from(error: CatalogIdError) -> Self {
        Self::DatabaseSchema(error.to_string())
    }
}

macro_rules! catalog_id {
    ($name:ident, $id_type:literal) => {
        #[doc = concat!("A checked positive SQLite identifier for `", $id_type, "` rows.")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i64);

        impl $name {
            /// Return the positive SQLite integer represented by this identifier.
            #[must_use]
            pub const fn as_i64(self) -> i64 {
                self.0
            }
        }

        impl TryFrom<i64> for $name {
            type Error = CatalogIdError;

            fn try_from(value: i64) -> Result<Self, Self::Error> {
                if value > 0 {
                    Ok(Self(value))
                } else {
                    Err(CatalogIdError::NonPositive {
                        id_type: $id_type,
                        value,
                    })
                }
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

catalog_id!(PublisherId, "publisher_id");
catalog_id!(CatalogId, "catalog_id");
catalog_id!(SourceFileId, "source_file_id");
catalog_id!(FetchAttemptId, "fetch_attempt_id");
catalog_id!(FileReceiptId, "file_receipt_id");
catalog_id!(ReadingRulesId, "reading_rules_id");
catalog_id!(CoverageId, "coverage_id");
catalog_id!(EditionId, "edition_id");
catalog_id!(ImportId, "import_id");
catalog_id!(HashId, "hash_id");
catalog_id!(SourceElementId, "source_element_id");
catalog_id!(SetGroupId, "set_group_id");
catalog_id!(SetId, "set_id");
catalog_id!(MediaEntryId, "media_entry_id");
catalog_id!(ImportMessageId, "message_id");

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_positive_sqlite_id<T>()
    where
        T: Copy + Debug + Eq + Hash + Ord + TryFrom<i64> + Into<i64>,
        <T as TryFrom<i64>>::Error: Debug,
    {
        let one = T::try_from(1).expect("one is a positive SQLite ID");
        let two = T::try_from(2).expect("two is a positive SQLite ID");

        let raw: i64 = one.into();
        assert_eq!(raw, 1);
        assert!(one < two);
        assert!(T::try_from(0).is_err());
        assert!(T::try_from(-1).is_err());
        let max: i64 = T::try_from(i64::MAX).unwrap().into();
        assert_eq!(max, i64::MAX);
    }

    #[test]
    fn every_catalog_key_is_a_checked_positive_sqlite_integer() {
        assert_positive_sqlite_id::<PublisherId>();
        assert_positive_sqlite_id::<CatalogId>();
        assert_positive_sqlite_id::<SourceFileId>();
        assert_positive_sqlite_id::<FetchAttemptId>();
        assert_positive_sqlite_id::<FileReceiptId>();
        assert_positive_sqlite_id::<ReadingRulesId>();
        assert_positive_sqlite_id::<CoverageId>();
        assert_positive_sqlite_id::<EditionId>();
        assert_positive_sqlite_id::<ImportId>();
        assert_positive_sqlite_id::<HashId>();
        assert_positive_sqlite_id::<SourceElementId>();
        assert_positive_sqlite_id::<SetGroupId>();
        assert_positive_sqlite_id::<SetId>();
        assert_positive_sqlite_id::<MediaEntryId>();
        assert_positive_sqlite_id::<ImportMessageId>();
    }

    #[test]
    fn equal_raw_values_remain_distinct_catalog_id_domains() {
        let publisher = PublisherId::try_from(7).unwrap();
        let catalog = CatalogId::try_from(7).unwrap();

        assert_eq!(publisher.as_i64(), catalog.as_i64());
    }
}
