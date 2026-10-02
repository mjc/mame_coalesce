use diesel::{QueryableByName, sql_types::BigInt};

pub use crate::domain::OccurrenceId;

#[derive(QueryableByName)]
pub struct AllocatedOccurrence {
    #[diesel(sql_type = BigInt)]
    pub occurrence_id: i64,
}
