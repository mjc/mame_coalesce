use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

embed_migrations!("migrations");

pub fn create_db_pool(database_url: &str) -> super::DbPool {
    let manager = ConnectionManager::<LoggingConnection<SqliteConnection>>::new(database_url);
    let pool: super::DbPool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool.");
    run_migrations(&pool);
    pool
}

fn run_migrations(pool: &super::DbPool) {
    let connection = pool.clone().get().unwrap();
    embedded_migrations::run(&connection).expect("failed to migrate database");
}
