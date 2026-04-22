use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use super::Pool;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn create_db_pool(database_url: &str) -> crate::Result<Pool> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder().build(manager)?;
    {
        let mut conn = pool.get()?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| crate::Error::Migration(e.to_string()))?;
    }
    Ok(pool)
}
