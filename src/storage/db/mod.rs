mod queries;
mod setup;

use diesel::{SqliteConnection, r2d2::ConnectionManager};

pub use queries::*;
pub use setup::*;

pub type Pool = diesel::r2d2::Pool<ConnectionManager<SqliteConnection>>;
