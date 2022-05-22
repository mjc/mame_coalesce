mod queries;
mod setup;

use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

pub use queries::*;
pub use setup::*;

use crate::options::Cli;

pub type Pool = r2d2::Pool<ConnectionManager<LoggingConnection<SqliteConnection>>>;

pub fn get_pool(cli: &Cli) -> Pool {
    let pool = match create_db_pool(cli.database_path()) {
        Ok(pool) => pool,
        Err(err) => panic!("Couldn't create db pool: {err:?}"),
    };
    pool
}
