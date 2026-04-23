use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XML parse error: {0}")]
    Xml(#[from] serde_xml_rs::Error),

    #[error("Diesel error: {0}")]
    Diesel(#[from] diesel::result::Error),

    #[error("Database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Archive error: {0}")]
    Archive(#[from] compress_tools::Error),

    #[error("Mmap error: {0}")]
    Mmap(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Thread pool error: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}
