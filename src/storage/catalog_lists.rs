//! Typed persistence for catalog identity and display metadata.

use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::{
    database::Database,
    domain::CatalogKey,
    storage::{
        catalog_ids::{CatalogId, PublisherId},
        db::Pool,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogMetadata {
    pub catalog_id: CatalogId,
    pub publisher_id: PublisherId,
    pub catalog_key: CatalogKey,
    pub display_name: String,
}

#[derive(QueryableByName)]
struct CatalogRow {
    #[diesel(sql_type = BigInt)]
    catalog_id: i64,
    #[diesel(sql_type = BigInt)]
    publisher_id: i64,
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    display_name: String,
}

impl CatalogRow {
    fn into_metadata(self) -> crate::Result<CatalogMetadata> {
        Ok(CatalogMetadata {
            catalog_id: positive_id(self.catalog_id)?,
            publisher_id: positive_publisher_id(self.publisher_id)?,
            catalog_key: CatalogKey::new(self.catalog_key),
            display_name: self.display_name,
        })
    }
}

pub struct CatalogRepository<'database> {
    pool: &'database Pool,
}

impl<'database> CatalogRepository<'database> {
    /// Create a repository backed by an already opened canonical database.
    #[must_use]
    pub fn new(database: &'database Database) -> Self {
        Self {
            pool: database.pool(),
        }
    }

    #[cfg(test)]
    fn from_pool(pool: &'database Pool) -> Self {
        Self { pool }
    }

    /// Register a stable catalog key under its publisher, updating its label if it exists.
    /// A catalog key is never reassigned to a different publisher.
    pub fn register(
        &self,
        key: &CatalogKey,
        publisher_id: PublisherId,
        display_name: &str,
    ) -> crate::Result<CatalogId> {
        let mut connection = self.pool.get()?;
        connection.immediate_transaction::<_, crate::Error, _>(|connection| {
            let existing = load_row(connection, key)?;
            match existing {
                Some(row) => {
                    let metadata = row.into_metadata()?;
                    if metadata.publisher_id != publisher_id {
                        return Err(crate::Error::CatalogIdentityConflict(
                            key.as_str().to_owned(),
                        ));
                    }
                    if metadata.display_name != display_name {
                        sql_query("UPDATE catalogs SET display_name = ? WHERE catalog_id = ?")
                            .bind::<Text, _>(display_name)
                            .bind::<BigInt, _>(metadata.catalog_id.as_i64())
                            .execute(connection)?;
                    }
                    Ok(metadata.catalog_id)
                }
                None => {
                    let row = sql_query(
                        "INSERT INTO catalogs (publisher_id, catalog_key, display_name) \
                         VALUES (?, ?, ?) RETURNING catalog_id, publisher_id, catalog_key, display_name",
                    )
                    .bind::<BigInt, _>(publisher_id.as_i64())
                    .bind::<Text, _>(key.as_str())
                    .bind::<Text, _>(display_name)
                    .get_result::<CatalogRow>(connection)?;
                    Ok(row.into_metadata()?.catalog_id)
                }
            }
        })
    }

    /// Load the typed identity and current display metadata for a catalog key.
    pub fn load(&self, key: &CatalogKey) -> crate::Result<Option<CatalogMetadata>> {
        let mut connection = self.pool.get()?;
        load_row(&mut connection, key)?
            .map(CatalogRow::into_metadata)
            .transpose()
    }
}

fn load_row(
    connection: &mut SqliteConnection,
    key: &CatalogKey,
) -> crate::Result<Option<CatalogRow>> {
    Ok(sql_query(
        "SELECT catalog_id, publisher_id, catalog_key, display_name \
         FROM catalogs WHERE catalog_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .get_result::<CatalogRow>(connection)
    .optional()?)
}

fn positive_id(value: i64) -> crate::Result<CatalogId> {
    CatalogId::try_from(value).map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
}

fn positive_publisher_id(value: i64) -> crate::Result<PublisherId> {
    PublisherId::try_from(value).map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
}

#[cfg(test)]
mod tests {
    use diesel::{QueryableByName, RunQueryDsl, sql_query, sql_types::Text};

    use crate::{
        domain::CatalogKey,
        storage::{
            catalog_ids::{CatalogId, PublisherId},
            db::{Pool, create_db_pool},
        },
    };

    use super::{CatalogMetadata, CatalogRepository};

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

    #[derive(QueryableByName)]
    struct InsertedPublisher {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        publisher_id: i64,
    }

    fn pool() -> crate::Result<Pool> {
        create_db_pool(":memory:")
    }

    fn publisher(pool: &Pool, key: &str, display_name: &str) -> crate::Result<PublisherId> {
        let row = sql_query(
            "INSERT INTO catalog_publishers (publisher_key, display_name) VALUES (?, ?) \
             RETURNING publisher_id",
        )
        .bind::<Text, _>(key)
        .bind::<Text, _>(display_name)
        .get_result::<InsertedPublisher>(&mut pool.get()?)?;
        PublisherId::try_from(row.publisher_id)
            .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))
    }

    fn assert_metadata(
        metadata: &CatalogMetadata,
        id: CatalogId,
        publisher_id: PublisherId,
        key: &CatalogKey,
        display_name: &str,
    ) {
        assert_eq!(metadata.catalog_id, id);
        assert_eq!(metadata.publisher_id, publisher_id);
        assert_eq!(&metadata.catalog_key, key);
        assert_eq!(metadata.display_name, display_name);
    }

    #[test]
    fn registers_and_loads_catalog_with_typed_identity_and_display_metadata() -> TestResult {
        let pool = pool()?;
        let publisher_id = publisher(&pool, "publisher-a", "Publisher A")?;
        let key = CatalogKey::new("stable-catalog-key");
        let repository = CatalogRepository::from_pool(&pool);

        let id = repository.register(&key, publisher_id, "Catalog title")?;
        let metadata = repository
            .load(&key)?
            .ok_or("registered catalog not found")?;

        assert_metadata(&metadata, id, publisher_id, &key, "Catalog title");
        Ok(())
    }

    #[test]
    fn catalog_key_cannot_be_reparented_to_another_publisher() -> TestResult {
        let pool = pool()?;
        let publisher_a = publisher(&pool, "publisher-a", "Publisher A")?;
        let publisher_b = publisher(&pool, "publisher-b", "Publisher B")?;
        let key = CatalogKey::new("stable-catalog-key");
        let repository = CatalogRepository::from_pool(&pool);
        let id = repository.register(&key, publisher_a, "Catalog title")?;

        assert!(
            repository
                .register(&key, publisher_b, "Changed title")
                .is_err()
        );

        let metadata = repository.load(&key)?.ok_or("catalog disappeared")?;
        assert_metadata(&metadata, id, publisher_a, &key, "Catalog title");
        Ok(())
    }

    #[test]
    fn catalog_display_name_can_change_without_changing_catalog_identity() -> TestResult {
        let pool = pool()?;
        let publisher_id = publisher(&pool, "publisher-a", "Publisher A")?;
        let key = CatalogKey::new("stable-catalog-key");
        let repository = CatalogRepository::from_pool(&pool);
        let original_id = repository.register(&key, publisher_id, "Old title")?;

        let updated_id = repository.register(&key, publisher_id, "Current title")?;
        let metadata = repository.load(&key)?.ok_or("catalog disappeared")?;

        assert_eq!(updated_id, original_id);
        assert_metadata(&metadata, original_id, publisher_id, &key, "Current title");
        Ok(())
    }

    #[test]
    fn loading_an_unknown_catalog_key_returns_none() -> TestResult {
        let pool = pool()?;
        let repository = CatalogRepository::from_pool(&pool);

        assert!(
            repository
                .load(&CatalogKey::new("missing-catalog"))?
                .is_none()
        );
        Ok(())
    }
}
