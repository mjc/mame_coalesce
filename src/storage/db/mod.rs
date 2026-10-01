mod queries;
mod setup;

use diesel::{SqliteConnection, r2d2::ConnectionManager};

pub use queries::*;
pub use setup::*;

type Connections = diesel::r2d2::Pool<ConnectionManager<SqliteConnection>>;

/// Connections and external temporary originals share the database's lifetime.
#[derive(Clone, Debug)]
pub struct Pool {
    connections: Connections,
    temporary_documents: Option<std::sync::Arc<tempfile::TempDir>>,
}

impl Pool {
    pub fn get(
        &self,
    ) -> Result<
        diesel::r2d2::PooledConnection<ConnectionManager<SqliteConnection>>,
        diesel::r2d2::PoolError,
    > {
        self.connections.get()
    }

    pub(crate) fn temporary_documents_path(&self) -> Option<&std::path::Path> {
        self.temporary_documents
            .as_ref()
            .map(|directory| directory.path())
    }
}
