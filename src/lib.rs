pub mod db;
pub mod error;
pub mod hashes;
pub mod logger;
pub mod logiqx;
pub mod models;
pub mod operations;
pub mod progress;
pub mod schema;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub mod test_helpers {
    use crate::db::{Pool, create_db_pool};

    /// Create an in-memory `SQLite` pool with migrations applied, suitable for unit tests.
    pub fn in_memory_pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }
}
