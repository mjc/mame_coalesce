//! Stable publisher keys with separately editable display names.
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::Text,
};

use crate::domain::{PublishingSource, PublishingSourceKey};

#[derive(QueryableByName)]
struct DisplayName {
    #[diesel(sql_type=Text)]
    display_name: String,
}

fn lookup(conn: &mut SqliteConnection, key: &PublishingSourceKey) -> crate::Result<Option<String>> {
    Ok(
        sql_query("SELECT display_name FROM publishing_sources WHERE source_key=?")
            .bind::<Text, _>(key.as_str())
            .get_result::<DisplayName>(conn)
            .optional()?
            .map(|row| row.display_name),
    )
}

fn insert(conn: &mut SqliteConnection, key: &PublishingSourceKey, name: &str) -> crate::Result<()> {
    sql_query("INSERT INTO publishing_sources(source_key,display_name) VALUES(?,?)")
        .bind::<Text, _>(key.as_str())
        .bind::<Text, _>(name)
        .execute(conn)?;
    Ok(())
}

/// Called inside the import's immediate transaction; conflicting publishers fail.
pub(super) fn ensure(
    conn: &mut SqliteConnection,
    key: &PublishingSourceKey,
    name: &str,
) -> crate::Result<()> {
    match lookup(conn, key)? {
        Some(existing) if existing != name => Err(crate::Error::SourceIdentityConflict(
            key.as_str().to_owned(),
        )),
        Some(_) => Ok(()),
        None => insert(conn, key, name),
    }
}

/// An explicit publisher registration may edit its label, never replace its key.
pub(super) fn register(
    conn: &mut SqliteConnection,
    source: &PublishingSource,
) -> crate::Result<()> {
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        if lookup(conn, source.key())?.is_some() {
            sql_query("UPDATE publishing_sources SET display_name=? WHERE source_key=?")
                .bind::<Text, _>(source.display_name())
                .bind::<Text, _>(source.key().as_str())
                .execute(conn)?;
            Ok(())
        } else {
            insert(conn, source.key(), source.display_name())
        }
    })
}
