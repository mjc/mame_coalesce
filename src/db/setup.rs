use std::time::Duration;

use diesel::{
    connection::SimpleConnection,
    r2d2::{ConnectionManager, CustomizeConnection, Error},
    SqliteConnection,
};

use diesel_logger::LoggingConnection;

use super::SyncPool;
use crate::MameResult;

embed_migrations!("migrations");

#[derive(Debug)]
pub struct ConnectionOptions {
    pub enable_wal: bool,
    pub enable_foreign_keys: bool,
    pub busy_timeout: Option<Duration>,
}

impl CustomizeConnection<LoggingConnection<SqliteConnection>, Error> for ConnectionOptions {
    fn on_acquire(&self, conn: &mut LoggingConnection<SqliteConnection>) -> Result<(), Error> {
        (|| {
            if self.enable_wal {
                conn.batch_execute("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
            }
            if self.enable_foreign_keys {
                conn.batch_execute("PRAGMA foreign_keys = ON;")?;
            }
            if let Some(d) = self.busy_timeout {
                conn.batch_execute(&format!("PRAGMA busy_timeout = {};", d.as_millis()))?;
            }
            Ok(())
        })()
        .map_err(Error::QueryError)
    }
}

pub fn create_sync_pool(database_url: &str) -> MameResult<SyncPool> {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: SyncPool = r2d2::Pool::builder()
        .connection_customizer(Box::new(ConnectionOptions {
            enable_wal: true,
            enable_foreign_keys: true,
            busy_timeout: Some(Duration::from_secs(30)),
        }))
        .max_size(8)
        .build(manager)?;
    embedded_migrations::run(&pool.get()?)?;

    Ok(pool)
}
