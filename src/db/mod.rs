mod queries;
mod setup;

use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_logger::LoggingConnection;

pub use queries::*;
pub use setup::*;
pub type DbPool = r2d2::Pool<ConnectionManager<LoggingConnection<SqliteConnection>>>;
