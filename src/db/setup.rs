use deadpool_diesel::sqlite::{Manager, Pool, Runtime};
use diesel::{r2d2::ConnectionManager, SqliteConnection};

use diesel_logger::LoggingConnection;

use super::{AsyncPool, SyncPool};
use crate::MameResult;

embed_migrations!("migrations");

pub fn create_sync_pool(database_url: &str) -> MameResult<SyncPool> {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: SyncPool = r2d2::Pool::builder().build(manager)?;
    embedded_migrations::run(&pool.get()?)?;

    Ok(pool)
}

pub async fn create_async_pool(
) -> deadpool_diesel::Pool<deadpool_diesel::Manager<diesel::SqliteConnection>> {
    let manager = Manager::new("coalesce.db", Runtime::Tokio1);
    let pool: AsyncPool = Pool::builder(manager).max_size(8).build().unwrap();
    let managed_conn = pool.get().await.unwrap();
    managed_conn
        .interact(|conn| {
            embedded_migrations::run(conn).unwrap();
        })
        .await
        .unwrap();
    pool
}
