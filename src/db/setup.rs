use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

use crate::MameResult;

use super::Pool;

embed_migrations!("migrations");

pub fn create_db_pool(database_url: &str) -> MameResult<Pool> {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: Pool = r2d2::Pool::builder().build(manager)?;
    run_migrations(&pool)?;
    Ok(pool)
}

fn run_migrations(pool: &Pool) -> MameResult<bool> {
    let connection = pool.clone().get()?;
    embedded_migrations::run(&connection)?;
    Ok(true)
}
