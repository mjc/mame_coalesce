use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    r2d2::ConnectionManager,
    sql_query,
    sql_types::{Binary, Bool, Text},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use super::{Pool, ddl::SCHEMA};

#[derive(Debug)]
struct EnableForeignKeys;

#[derive(QueryableByName)]
struct TempConfiguration {
    #[diesel(sql_type = Bool)]
    forced_memory: bool,
}

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for EnableForeignKeys
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        // This hook runs once when a connection is established, not on pool
        // checkout: changing temp_store later would delete its request tables.
        conn.batch_execute(
            "PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000; PRAGMA temp_store = FILE",
        )
        .map_err(diesel::r2d2::Error::QueryError)?;
        let configuration =
            sql_query("SELECT sqlite_compileoption_used('TEMP_STORE=3') AS forced_memory")
                .get_result::<TempConfiguration>(conn)
                .map_err(diesel::r2d2::Error::QueryError)?;
        if configuration.forced_memory {
            return Err(diesel::r2d2::Error::QueryError(
                diesel::result::Error::QueryBuilderError(
                    "catalog imports require SQLite with file-backed temporary storage".into(),
                ),
            ));
        }
        Ok(())
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
    super::super::catalog_content::registry_id(conn)?;
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
    let mut builder =
        super::Connections::builder().connection_customizer(Box::new(EnableForeignKeys));
    // A bare in-memory SQLite database belongs to one connection, not to the pool.
    if database_url == ":memory:" {
        builder = builder.max_size(1).idle_timeout(None).max_lifetime(None);
    }
    let pool = Pool {
        connections: builder.build(manager)?,
        temporary_documents: if database_url == ":memory:" {
            Some(std::sync::Arc::new(
                tempfile::Builder::new()
                    .prefix("mame-coalesce-documents-")
                    .tempdir()?,
            ))
        } else {
            None
        },
    };
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
        #[derive(QueryableByName)]
        struct TempState {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            temp_store: i64,
            #[diesel(sql_type = Text)]
            value: String,
        }
        let pool = create_db_pool(":memory:")?;
        assert_eq!(pool.connections.max_size(), 1);
        assert_eq!(pool.connections.max_lifetime(), None);
        assert_eq!(pool.connections.idle_timeout(), None);
        {
            let mut conn = pool.get()?;
            conn.batch_execute("CREATE TABLE checkout_witness(value TEXT); INSERT INTO checkout_witness VALUES ('kept')")?;
            conn.batch_execute("CREATE TEMP TABLE temporary_checkout_witness(value TEXT); INSERT INTO temporary_checkout_witness VALUES ('kept')")?;
        }
        let mut conn = pool.get()?;
        let temporary = sql_query("SELECT temp_store,value FROM pragma_temp_store CROSS JOIN temp.temporary_checkout_witness")
            .get_result::<TempState>(&mut conn)?;
        assert_eq!(
            (temporary.temp_store, temporary.value.as_str()),
            (1, "kept")
        );
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
