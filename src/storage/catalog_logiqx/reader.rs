use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::{
    database::Database,
    domain::{CatalogRegistryId, SnapshotKey},
    logiqx::RecordLocation,
};

use super::{
    LogiqxCursor, LogiqxPage, LogiqxPageLimit, LogiqxQueryError, LogiqxSnapshot, LogiqxYesNo,
};

pub(super) type QueryResult<T> = Result<T, LogiqxQueryError>;

macro_rules! row {
    ($name:ident { $($field:ident: $rust_ty:ty => $sql_ty:ty),* $(,)? }) => {
        #[derive(diesel::QueryableByName)]
        pub(super) struct $name {
            $(#[diesel(sql_type = $sql_ty)] pub(super) $field: $rust_ty,)*
        }
    };
}
pub(super) use row;

row!(SnapshotRow {
    snapshot_key: String => Text,
    source_key: String => Text,
    source_name: String => Text,
    catalog_key: String => Text,
    catalog_name: String => Text,
    document_key: String => Text,
    interpretation_key: String => Text,
    rules_version: Option<String> => Nullable<Text>,
    rules_valid: i64 => BigInt,
    format: String => Text,
    declared_version: Option<String> => Nullable<Text>,
    published: i64 => BigInt,
});

row!(GroupRow {
    set_group_id: i64 => BigInt,
    kind: String => Text,
    list_order: i64 => BigInt,
});

pub(super) fn logiqx_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&LogiqxCursor>,
    limit: LogiqxPageLimit,
) -> QueryResult<LogiqxPage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| {
        let generation = crate::storage::catalog_content::registry_id(connection)?;
        validate_cursor(cursor, generation, snapshot)?;
        let provenance = snapshot_row(connection, snapshot, generation)?;
        let document = super::document::load(connection, snapshot)?;
        let groups = sql_query(super::queries::GROUPS)
            .bind::<Text, _>(snapshot.as_str())
            .load::<GroupRow>(connection)?;
        if groups.len() > 1
            || groups.iter().any(|group| {
                group.kind != "root" || group.list_order != 0 || group.set_group_id <= 0
            })
        {
            return Err(LogiqxQueryError::MismatchedOwner(
                groups.first().map_or(0, |group| group.set_group_id),
            ));
        }
        let bound = i64::try_from(limit.0)
            .map_err(|_| LogiqxQueryError::PageLimitOverflow)?
            .checked_add(1)
            .ok_or(LogiqxQueryError::PageLimitOverflow)?;
        let mut rows = match groups.first() {
            Some(group) => super::games::load(connection, group.set_group_id, cursor, bound)?,
            None if cursor.is_some() => return Err(LogiqxQueryError::MismatchedOwner(0)),
            None => Vec::new(),
        };
        let has_more = rows.len() > limit.0;
        rows.truncate(limit.0);
        let games = super::games::hydrate(connection, snapshot, rows)?;
        let next_cursor = if has_more {
            games.last().map(|game| LogiqxCursor {
                generation,
                snapshot: snapshot.clone(),
                order: game.list_order,
                owner_id: game.id,
            })
        } else {
            None
        };
        Ok(LogiqxPage {
            snapshot: provenance,
            document,
            games,
            next_cursor,
        })
    })
}

fn validate_cursor(
    cursor: Option<&LogiqxCursor>,
    generation: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.generation != generation {
            return Err(LogiqxQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot != snapshot {
            return Err(LogiqxQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn snapshot_row(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    generation: CatalogRegistryId,
) -> QueryResult<LogiqxSnapshot> {
    let row = sql_query(super::queries::SNAPSHOT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotRow>(connection)
        .map_err(|error| match error {
            diesel::result::Error::NotFound => {
                LogiqxQueryError::NotPublishedLogiqx(snapshot.clone())
            }
            other => LogiqxQueryError::Database(other),
        })?;
    if row.published != 1
        || row.format != "logiqx"
        || row.snapshot_key != snapshot.as_str()
        || row.rules_valid != 1
    {
        return Err(LogiqxQueryError::NotPublishedLogiqx(snapshot.clone()));
    }
    Ok(LogiqxSnapshot {
        registry_id: generation,
        snapshot_key: snapshot.clone(),
        source_key: crate::domain::PublishingSourceKey::new(row.source_key),
        source_name: row.source_name,
        catalog_key: crate::domain::CatalogKey::new(row.catalog_key),
        catalog_name: row.catalog_name,
        document_key: row.document_key.parse()?,
        interpretation_key: crate::domain::ParserInterpretationKey::from_persisted(
            row.interpretation_key,
        ),
        rules_version: row.rules_version,
        format: row.format,
        declared_version: row.declared_version,
    })
}

pub(super) fn batch<T: QueryableByName<diesel::sqlite::Sqlite> + 'static>(
    connection: &mut SqliteConnection,
    sql: &str,
    ids: &[i64],
) -> QueryResult<Vec<T>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut query = sql_query(sql).into_boxed::<diesel::sqlite::Sqlite>();
    for id in ids {
        query = query.bind::<BigInt, _>(*id);
    }
    Ok(query.load(connection)?)
}

pub(super) const fn location(line: i64, column: i64, owner: i64) -> QueryResult<RecordLocation> {
    if line <= 0 || column <= 0 {
        return Err(LogiqxQueryError::InvalidPositions(owner));
    }
    Ok(RecordLocation { line, column })
}

pub(super) fn stored_bool(value: i64, field: &'static str, owner: i64) -> QueryResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid(field, owner, value.to_string())),
    }
}

pub(super) fn yes_no(value: &str, field: &'static str, owner: i64) -> QueryResult<LogiqxYesNo> {
    match value {
        "yes" => Ok(LogiqxYesNo::Yes),
        "no" => Ok(LogiqxYesNo::No),
        _ => Err(invalid(field, owner, value.to_owned())),
    }
}

pub(super) fn default_presence(
    value: &str,
    present: i64,
    default: &str,
    field: &'static str,
    owner: i64,
) -> QueryResult<bool> {
    let present = stored_bool(present, field, owner)?;
    if !present && value != default {
        return Err(invalid(field, owner, value.to_owned()));
    }
    Ok(present)
}

pub(super) const fn invalid(field: &'static str, owner: i64, value: String) -> LogiqxQueryError {
    LogiqxQueryError::InvalidStoredValue {
        field,
        owner,
        value,
    }
}
