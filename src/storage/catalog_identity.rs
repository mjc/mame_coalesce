use diesel::{QueryableByName, sql_types::BigInt};

/// Internal identities are deliberately distinct from stable document keys and
/// from publisher names. They may only be obtained from the database that owns them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordId(i64);

impl RecordId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OccurrenceId(i64);

impl OccurrenceId {
    pub(crate) const fn from_database(value: i64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn database_value(self) -> i64 {
        self.0
    }
}

#[derive(QueryableByName)]
pub struct AllocatedOccurrence {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
}
