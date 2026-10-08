//! Stable publisher keys with separately editable display names.
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::Text,
};

use crate::{
    domain::{PublishingSource, PublishingSourceKey},
    storage::catalog_ids::PublisherId,
};

#[derive(QueryableByName)]
struct PublisherRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    publisher_id: i64,
    #[diesel(sql_type = Text)]
    display_name: String,
}

fn row_id(row: PublisherRow) -> crate::Result<(PublisherId, String)> {
    let id = PublisherId::try_from(row.publisher_id)
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    Ok((id, row.display_name))
}

fn lookup(
    conn: &mut SqliteConnection,
    key: &PublishingSourceKey,
) -> crate::Result<Option<(PublisherId, String)>> {
    let row = sql_query(
        "SELECT publisher_id, display_name FROM catalog_publishers WHERE publisher_key = ?",
    )
    .bind::<Text, _>(key.as_str())
    .get_result::<PublisherRow>(conn)
    .optional()?;
    row.map(row_id).transpose()
}

pub(super) fn lookup_id(
    conn: &mut SqliteConnection,
    key: &PublishingSourceKey,
) -> crate::Result<Option<PublisherId>> {
    Ok(lookup(conn, key)?.map(|(id, _)| id))
}

fn insert(
    conn: &mut SqliteConnection,
    key: &PublishingSourceKey,
    name: &str,
) -> crate::Result<PublisherId> {
    let row = sql_query(
        "INSERT INTO catalog_publishers (publisher_key, display_name, locator) \
         VALUES (?, ?, NULL) RETURNING publisher_id, display_name",
    )
    .bind::<Text, _>(key.as_str())
    .bind::<Text, _>(name)
    .get_result::<PublisherRow>(conn)?;
    row_id(row).map(|(id, _)| id)
}

/// Called inside the import's immediate transaction; conflicting publishers fail.
pub(super) fn ensure(
    conn: &mut SqliteConnection,
    key: &PublishingSourceKey,
    name: &str,
) -> crate::Result<()> {
    match lookup(conn, key)? {
        Some((_, existing)) if existing != name => Err(crate::Error::SourceIdentityConflict(
            key.as_str().to_owned(),
        )),
        Some(_) => Ok(()),
        None => insert(conn, key, name).map(|_| ()),
    }
}

/// An explicit publisher registration may edit its label, never replace its key.
pub(super) fn register(
    conn: &mut SqliteConnection,
    source: &PublishingSource,
) -> crate::Result<()> {
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        if lookup(conn, source.key())?.is_some() {
            sql_query("UPDATE catalog_publishers SET display_name = ? WHERE publisher_key = ?")
                .bind::<Text, _>(source.display_name())
                .bind::<Text, _>(source.key().as_str())
                .execute(conn)?;
            Ok(())
        } else {
            insert(conn, source.key(), source.display_name()).map(|_| ())
        }
    })
}
