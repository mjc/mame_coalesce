use camino::Utf8PathBuf;

pub struct Database {
    pool: crate::storage::db::Pool,
}

impl Database {
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
}
