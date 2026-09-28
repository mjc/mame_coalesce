use camino::Utf8PathBuf;

/// Handle to the application's persistent catalog and scan cache.
pub struct Database {
    pool: crate::storage::db::Pool,
}

impl Database {
    /// Open or create a cache database at `cache_path`, applying pending migrations.
    pub fn open(cache_path: &Utf8PathBuf) -> crate::Result<Self> {
        if let Some(parent) = cache_path.parent()
            && !parent.as_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        crate::storage::db::create_db_pool(cache_path.as_str()).map(|pool| Self { pool })
    }

    pub(crate) const fn pool(&self) -> &crate::storage::db::Pool {
        &self.pool
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> crate::Result<Self> {
        crate::storage::db::create_db_pool(":memory:").map(|pool| Self { pool })
    }
}
