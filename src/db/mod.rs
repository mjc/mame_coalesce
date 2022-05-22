mod queries;
mod setup;

use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

pub use queries::*;
pub use setup::*;

pub type SyncPool = r2d2::Pool<ConnectionManager<LoggingConnection<SqliteConnection>>>;
pub type SyncPooledConnection =
    r2d2::PooledConnection<ConnectionManager<LoggingConnection<SqliteConnection>>>;

pub type AsyncPool = deadpool::managed::Pool<deadpool_diesel::Manager<diesel::SqliteConnection>>;
