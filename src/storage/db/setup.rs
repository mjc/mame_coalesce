use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    r2d2::ConnectionManager,
    sql_query,
    sql_types::{Binary, Text},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::Pool;

/// Complete DDL for a new database. It is never an upgrade script.
const SCHEMA: &str = concat!(
    include_str!("coverage.sql"),
    "\n",
    include_str!("schema.sql"),
    "\n",
    include_str!("logiqx.sql")
);

#[derive(Debug)]
struct EnableForeignKeys;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for EnableForeignKeys
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000")
            .map_err(diesel::r2d2::Error::QueryError)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, QueryableByName)]
struct SchemaObject {
    #[diesel(sql_type = Text)]
    object_type: String,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    table_name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

#[derive(QueryableByName)]
struct SchemaDigest {
    #[diesel(sql_type = Binary)]
    schema_digest: Vec<u8>,
}

enum DatabaseState {
    Empty,
    Initialized,
}

fn schema_objects(conn: &mut SqliteConnection) -> crate::Result<BTreeSet<SchemaObject>> {
    Ok(sql_query(
        "SELECT type AS object_type, name, tbl_name AS table_name, sql \
         FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT GLOB 'sqlite_*' \
         ORDER BY type, name, tbl_name",
    )
    .load::<SchemaObject>(conn)?
    .into_iter()
    .collect())
}

fn database_state(conn: &mut SqliteConnection) -> crate::Result<DatabaseState> {
    let objects = schema_objects(conn)?;
    if objects.is_empty() {
        return Ok(DatabaseState::Empty);
    }
    if objects
        .iter()
        .any(|object| object.object_type == "table" && object.name == "database_schema")
    {
        Ok(DatabaseState::Initialized)
    } else {
        Err(crate::Error::DatabaseSchema(
            "this database is not a current catalog database; use a new empty cache path"
                .to_owned(),
        ))
    }
}

/// Validate the exact current schema without writing or attempting an upgrade.
pub fn validate_database_schema(conn: &mut SqliteConnection) -> crate::Result<()> {
    let stored = sql_query("SELECT schema_digest FROM database_schema WHERE singleton = 1")
        .get_result::<SchemaDigest>(conn)
        .map_err(|error| {
            crate::Error::DatabaseSchema(format!("missing schema identity: {error}"))
        })?;
    if stored.schema_digest.as_slice() != Sha256::digest(SCHEMA.as_bytes()).as_slice() {
        return Err(crate::Error::DatabaseSchema(
            "this database uses a different schema; create a new database and reimport catalogs"
                .to_owned(),
        ));
    }
    let mut reference = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    reference.batch_execute(SCHEMA)?;
    if schema_objects(conn)? != schema_objects(&mut reference)? {
        return Err(crate::Error::DatabaseSchema(
            "database schema differs from the authoritative DDL; it will not be repaired automatically".to_owned(),
        ));
    }
    Ok(())
}

/// Create an empty database atomically, or validate an already-current one.
/// Existing databases are never converted, upgraded, erased or repaired.
pub fn initialize_database(conn: &mut SqliteConnection) -> crate::Result<()> {
    conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000")?;
    conn.immediate_transaction(|conn| match database_state(conn)? {
        DatabaseState::Empty => {
            conn.batch_execute(SCHEMA)?;
            sql_query("INSERT INTO database_schema(singleton, schema_digest) VALUES (1, ?)")
                .bind::<Binary, _>(Sha256::digest(SCHEMA.as_bytes()).as_slice())
                .execute(conn)?;
            Ok(())
        }
        DatabaseState::Initialized => validate_database_schema(conn),
    })
}

pub fn create_db_pool(database_url: &str) -> crate::Result<Pool> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let mut builder = Pool::builder().connection_customizer(Box::new(EnableForeignKeys));
    // A bare in-memory SQLite database belongs to one connection, not to the pool.
    if database_url == ":memory:" {
        builder = builder.max_size(1).idle_timeout(None).max_lifetime(None);
    }
    let pool = builder.build(manager)?;
    {
        let mut connection = pool.get()?;
        initialize_database(&mut connection)?;
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_pool_keeps_one_non_expiring_database() -> crate::Result<()> {
        let pool = create_db_pool(":memory:")?;
        assert_eq!(pool.max_size(), 1);
        assert_eq!(pool.max_lifetime(), None);
        assert_eq!(pool.idle_timeout(), None);
        {
            let mut conn = pool.get()?;
            conn.batch_execute("CREATE TABLE checkout_witness(value TEXT); INSERT INTO checkout_witness VALUES ('kept')")?;
        }
        let mut conn = pool.get()?;
        assert_eq!(
            schema_objects(&mut conn)?
                .iter()
                .filter(|object| object.name == "checkout_witness")
                .count(),
            1
        );
        Ok(())
    }
}
