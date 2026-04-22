#![deny(elided_lifetimes_in_paths, clippy::all)]
#![warn(
    clippy::all,
    clippy::nursery,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::str_to_string,
    clippy::unneeded_field_pattern,
    clippy::unwrap_used
)]

#[macro_use]
extern crate diesel;

use log::warn;

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
    use crate::db::{create_db_pool, Pool};

    /// Create an in-memory SQLite pool with migrations applied, suitable for unit tests.
    pub fn in_memory_pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }
}
