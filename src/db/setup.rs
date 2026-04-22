use diesel::{SqliteConnection, r2d2::ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

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
