use camino::{Utf8Path, Utf8PathBuf};

use crate::storage::db::{self, Pool};

pub mod scan;

/// List scan-eligible files below `path`, excluding database files and sidecars.
pub fn list_source_paths(path: &Utf8Path, pool: &Pool) -> crate::Result<Vec<Utf8PathBuf>> {
    let source_root = path.canonicalize_utf8()?;
    let excluded_paths = db::database_file_paths(pool)?;
    scan::walk_for_files(&source_root, &excluded_paths)
}
