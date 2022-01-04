mod queries;
mod setup;

use diesel::{r2d2::ConnectionManager, SqliteConnection};

pub use queries::*;
pub use setup::*;
pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
