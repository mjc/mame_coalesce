use std::time::Duration;

use deadpool_diesel::sqlite::{Manager, Pool, Runtime};
use diesel::{r2d2::ConnectionManager, SqliteConnection};

use diesel_logger::LoggingConnection;

use super::{AsyncPool, SyncPool};
use crate::MameResult;

embed_migrations!("migrations");

pub fn create_sync_pool(database_url: &str) -> MameResult<SyncPool> {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: SyncPool = r2d2::Pool::builder().max_size(8).build(manager)?;
    embedded_migrations::run(&pool.get()?)?;

    Ok(pool)
}

pub async fn create_async_pool() -> MameResult<AsyncPool> {
    let manager = Manager::new("coalesce.db", Runtime::Tokio1);
    let pool: AsyncPool = Pool::builder(manager)
        // TODO set wal mode in a post-create hook. Without this, the db is single threaded
        .max_size(1)
        .wait_timeout(Some(Duration::new(5, 0)))
        .runtime(Runtime::Tokio1)
        .build()?;
    let managed_conn = pool.get().await?;
    managed_conn
        .interact(|conn| embedded_migrations::run(conn).unwrap())
        .await?;
    Ok(pool)
}
