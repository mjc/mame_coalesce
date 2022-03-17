use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

use super::Pool;

embed_migrations!("migrations");

pub fn create_db_pool(database_url: &str) -> Pool {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: Pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");
    run_migrations(&pool);
    pool
}

fn run_migrations(pool: &Pool) {
    let connection = pool.clone().get().unwrap();
    embedded_migrations::run(&connection).expect("failed to migrate database");
}
