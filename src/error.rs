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
    Archive(#[from] r7z::R7zError),

    #[error("RAR error: {0}")]
    Rar(#[from] unrar::error::UnrarError),

    #[error("Mmap error: {0}")]
    Mmap(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid hash: {0}")]
    InvalidHash(String),

    #[error("document exceeds the configured {limit}-byte input limit")]
    DocumentTooLarge { limit: usize },

    #[error("XML entity declarations are not allowed")]
    XmlEntityNotAllowed,

    #[error("XML validation failed: {0}")]
    XmlValidation(String),

    #[error("catalog parse failed: {message}")]
    CatalogParse {
        message: String,
        record_kind: Option<String>,
        record_name: Option<String>,
        line: Option<i64>,
        column: Option<i64>,
    },

    #[error("different source bytes produced an existing document digest")]
    DocumentDigestCollision,

    #[error("acquired document bytes do not match the source-provided digest")]
    DocumentDigestMismatch,

    #[error("document {0} does not have retained payload bytes")]
    DocumentUnavailable(String),

    #[error("catalog identity {0} conflicts with previously persisted metadata")]
    CatalogIdentityConflict(String),

    #[error("source identity {0} conflicts with previously persisted metadata")]
    SourceIdentityConflict(String),

    #[error("transport metadata serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported serialized plan version: {0}")]
    UnsupportedPlanVersion(u32),

    #[error("{0}")]
    PlanValidation(#[from] crate::build::validation::PlanValidation),

    #[error("ROM size cannot be stored in SQLite: {0}")]
    InvalidRomSize(u64),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Thread pool error: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}
