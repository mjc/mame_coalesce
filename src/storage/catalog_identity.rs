use diesel::{QueryableByName, sql_types::BigInt};

/// An internal source occurrence identity, distinct from its owning catalog set
/// and from the expected shared file UUID.
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
