//! Shared safeguards for source-to-native-query corpus verifiers.

use std::{error::Error, fmt::Debug};

use camino::Utf8Path;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::database::Database;

pub type VerifyResult<T = ()> = Result<T, Box<dyn Error>>;

/// Refuse missing, empty and schema-less SQLite files before normal catalog opening.
///
/// Normal opening still locks and validates the database; this is not a
/// filesystem-strict read-only connection. No catalog import is performed.
pub fn open_existing_catalog(path: &Utf8Path) -> VerifyResult<Database> {
    if !path.is_file() {
        return Err(format!("catalog database is not an existing file: {path}").into());
    }
    if std::fs::metadata(path)?.len() == 0 {
        return Err(format!("catalog database is an empty file: {path}").into());
    }
    require_catalog_schema(path)?;
    Ok(Database::open(path)?)
}

#[derive(QueryableByName)]
struct CatalogSchemaMarker {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn require_catalog_schema(path: &Utf8Path) -> VerifyResult {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    sql_query("PRAGMA query_only = ON").execute(&mut connection)?;
    let marker = sql_query(
        "SELECT COUNT(*) AS count FROM sqlite_schema \
         WHERE type='table' AND name='database_schema'",
    )
    .get_result::<CatalogSchemaMarker>(&mut connection)?;
    if marker.count != 1 {
        return Err(format!("not an initialized catalog database: {path}").into());
    }
    Ok(())
}

pub fn equal<T: PartialEq + Debug + ?Sized>(field: &str, source: &T, catalog: &T) -> VerifyResult {
    if source == catalog {
        Ok(())
    } else {
        Err(format!("{field} mismatch: source={source:?}, catalog={catalog:?}").into())
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};

    use super::{VerifyResult, equal, open_existing_catalog};

    #[test]
    fn absent_and_empty_database_files_are_not_initialized() -> VerifyResult {
        let directory = tempfile::tempdir()?;
        let missing = Utf8PathBuf::try_from(directory.path().join("missing.sqlite"))?;
        assert!(open_existing_catalog(&missing).is_err());
        assert!(!missing.exists());
        assert!(!missing.with_extension("sqlite.lock").exists());

        let empty = Utf8PathBuf::try_from(directory.path().join("empty.sqlite"))?;
        std::fs::write(&empty, [])?;
        assert!(open_existing_catalog(&empty).is_err());
        assert!(std::fs::read(&empty)?.is_empty());
        assert!(!empty.with_extension("sqlite.lock").exists());
        Ok(())
    }

    #[test]
    fn schema_less_sqlite_is_rejected_without_mutation() -> VerifyResult {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("empty.sqlite"))?;
        let mut connection = SqliteConnection::establish(path.as_str())?;
        connection.batch_execute("PRAGMA user_version = 1")?;
        drop(connection);
        let before = std::fs::read(&path)?;
        assert!(!before.is_empty());
        assert!(open_existing_catalog(&path).is_err());
        assert_eq!(std::fs::read(&path)?, before);
        assert!(!path.with_extension("sqlite.lock").exists());
        Ok(())
    }

    #[test]
    fn non_sqlite_bytes_are_rejected_without_mutation() -> VerifyResult {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("not.sqlite"))?;
        let before = b"not a SQLite database\0arbitrary bytes";
        std::fs::write(&path, before)?;
        assert!(open_existing_catalog(&path).is_err());
        assert_eq!(std::fs::read(&path)?, before);
        assert!(!path.with_extension("sqlite.lock").exists());
        Ok(())
    }

    #[test]
    fn mismatches_name_the_exact_field_and_both_values() -> VerifyResult {
        equal("header.name", "same", "same")?;
        let error = equal("game[65].name", "source", "catalog")
            .err()
            .ok_or("different names unexpectedly compare equal")?
            .to_string();
        assert!(error.contains("game[65].name"));
        assert!(error.contains("source=\"source\""));
        assert!(error.contains("catalog=\"catalog\""));
        Ok(())
    }
}
