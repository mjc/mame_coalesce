pub mod app;
mod build;
pub mod database;
pub mod domain;
pub mod error;
pub mod hashes;
pub mod logiqx;
mod operations;
mod progress;
mod storage;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub mod test_helpers {
    use crate::storage::db::{Pool, create_db_pool};

    /// Create an in-memory `SQLite` pool with migrations applied, suitable for unit tests.
    pub fn in_memory_pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }
}
