pub mod app;
pub mod build;
pub mod clrmamepro;
pub mod database;
pub mod diagnostics;
pub mod disk;
mod document_input;
pub mod domain;
pub mod error;
pub mod hashes;
pub mod logiqx;
pub mod machine_dependencies;
pub mod mame;
pub mod mame_softwarelist;
pub mod mount;
pub mod no_intro_dat_xml;
pub mod no_intro_db_xml;
mod no_intro_pc_xml;
mod operations;
mod private_temp;
pub mod reconciliation;
pub mod resolution;
pub mod serving;
pub mod software_loading;
pub(crate) mod sources;
mod storage;
mod xml_reader;

pub use domain::PublishingSource;
pub use error::Error;
pub use no_intro_dat_xml::NoIntroDatMode;
pub use storage::backup::{
    BackupOutcome, IntegrityReport, RestoreOutcome, RestorePolicy, check_integrity, create_backup,
    restore_backup,
};
pub use storage::catalog_clrmamepro;
pub use storage::catalog_editions;
pub use storage::catalog_files;
pub use storage::catalog_hashes;
pub use storage::catalog_ids;
pub use storage::catalog_lists;
pub use storage::catalog_logiqx;
pub use storage::catalog_machines;
pub use storage::catalog_messages;
pub use storage::catalog_no_intro_dat;
pub use storage::catalog_no_intro_database;
pub use storage::catalog_requirements;
pub use storage::catalog_selection;
pub use storage::catalog_software;
pub use storage::documents::{
    AcquisitionMetadata, DocumentStore, RetainedDocument, TransportHeader,
};
pub use storage::file_match_reviews;
pub use storage::import_diagnostics;
pub use storage::reading_rules;
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
pub mod test_helpers {
    use crate::storage::db::{Pool, create_db_pool};

    /// Create an in-memory `SQLite` pool from the current schema for unit tests.
    pub fn in_memory_pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }
}
