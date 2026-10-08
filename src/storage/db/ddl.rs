/// Complete greenfield schema. There is no historical DDL or conversion path.
///
/// Catalog SQL is generated from the approved model, not maintained as a second
/// handwritten schema. Physical inventory remains independent of catalog IDs.
pub const SCHEMA: &str = concat!(
    "CREATE TABLE database_schema (singleton INTEGER PRIMARY KEY CHECK (singleton = 1), ",
    "schema_digest BLOB NOT NULL CHECK (typeof(schema_digest) = 'blob' AND length(schema_digest) = 32)) STRICT;\n",
    include_str!("inventory.sql"),
    "\n",
    include_str!("catalog.sql"),
);
