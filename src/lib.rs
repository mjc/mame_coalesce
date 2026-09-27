pub mod app;
mod build;
mod clrmamepro;
pub mod database;
pub mod disk;
mod document_input;
pub mod domain;
pub mod error;
pub mod hashes;
pub mod logiqx;
pub(crate) mod mame;
pub(crate) mod mame_softwarelist;
mod no_intro_pc_xml;
mod operations;
mod progress;
mod storage;

pub use domain::PublishingSource;
pub use error::Error;
pub use storage::documents::{AcquisitionMetadata, DocumentStore, RetainedDocument};
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub mod test_helpers {
    use crate::storage::db::{Pool, create_db_pool};

    /// Create an in-memory `SQLite` pool with migrations applied, suitable for unit tests.
    pub fn in_memory_pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }
}
