use std::fs::{self, File, OpenOptions};

use camino::{Utf8Path, Utf8PathBuf};
use fs4::fs_std::FileExt;

/// Handle to the application's persistent catalog and scan cache.
pub struct Database {
    pool: crate::storage::db::Pool,
    _cache_lock: Option<File>,
}

#[derive(Clone, Copy)]
pub(crate) enum CacheLockMode {
    Shared,
    Exclusive,
}

pub(crate) fn lock_cache_file(
    cache_path: &Utf8Path,
    mode: CacheLockMode,
) -> crate::Result<(Utf8PathBuf, File)> {
    let parent = cache_path
        .parent()
        .filter(|parent| !parent.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = cache_path
        .file_name()
        .ok_or_else(|| crate::Error::InvalidPath(cache_path.to_string()))?;
    let path_exists = match fs::symlink_metadata(cache_path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    let canonical_path = if path_exists {
        cache_path.as_std_path().canonicalize()?
    } else {
        parent.canonicalize()?.join(file_name)
    };
    let canonical_path = Utf8PathBuf::from_path_buf(canonical_path)
        .map_err(|path| crate::Error::InvalidPath(path.display().to_string()))?;
    if canonical_path.exists() {
        reject_multiple_hard_links(&canonical_path)?;
    }
    let lock_path = format!("{}.lock", canonical_path.as_str());
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    match mode {
        CacheLockMode::Shared => FileExt::try_lock_shared(&lock_file).map_err(|error| {
            crate::Error::CacheBackup(format!(
                "cache is being maintained by another process: {error}"
            ))
        })?,
        CacheLockMode::Exclusive => FileExt::try_lock_exclusive(&lock_file).map_err(|error| {
            crate::Error::CacheBackup(format!("cache is open by another process: {error}"))
        })?,
    }
    Ok((canonical_path, lock_file))
}

pub(crate) fn reject_multiple_hard_links(path: &Utf8Path) -> crate::Result<()> {
    let metadata = fs::metadata(path)?;
    #[cfg(unix)]
    let link_count = {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink()
    };
    #[cfg(windows)]
    let link_count = {
        use std::os::windows::fs::MetadataExt;
        metadata.number_of_links()
    };
    #[cfg(not(any(unix, windows)))]
    let link_count = 1;
    if link_count > 1 {
        return Err(crate::Error::CacheBackup(format!(
            "refusing cache file with multiple hard links: {path}"
        )));
    }
    Ok(())
}

impl Database {
    /// Open a current database or create an empty one directly from the schema.
    pub fn open(cache_path: &Utf8PathBuf) -> crate::Result<Self> {
        let (canonical_path, cache_lock) = lock_cache_file(cache_path, CacheLockMode::Shared)?;
        crate::storage::db::create_db_pool(canonical_path.as_str()).map(|pool| Self {
            pool,
            _cache_lock: Some(cache_lock),
        })
    }

    pub(crate) const fn pool(&self) -> &crate::storage::db::Pool {
        &self.pool
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> crate::Result<Self> {
        crate::storage::db::create_db_pool(":memory:").map(|pool| Self {
            pool,
            _cache_lock: None,
        })
    }
}
