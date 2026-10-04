//! One transaction owns publication, cursor, document and complete selected sets.

use super::{
    ClrMameProCursor, ClrMameProPage, ClrMameProPageLimit, ClrMameProQueryError,
    ClrMameProSnapshot, document, queries,
    sets::{self, SetRow},
};
use crate::{
    database::Database,
    domain::{
        CatalogKey, CatalogRegistryId, DocumentKey, ParserInterpretationKey, PublishingSourceKey,
        SnapshotKey,
    },
    storage::catalog_content::registry_id,
};
use diesel::{
    Connection, OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

type QueryResult<T> = Result<T, ClrMameProQueryError>;

#[derive(QueryableByName)]
struct SnapshotRow {
    #[diesel(sql_type=Text)]
    snapshot_key: String,
    #[diesel(sql_type=Text)]
    source_key: String,
    #[diesel(sql_type=Text)]
    source_name: String,
    #[diesel(sql_type=Text)]
    catalog_key: String,
    #[diesel(sql_type=Text)]
    catalog_name: String,
    #[diesel(sql_type=Text)]
    document_key: String,
    #[diesel(sql_type=Text)]
    interpretation_key: String,
    #[diesel(sql_type=Text)]
    format: String,
    #[diesel(sql_type=Text)]
    rules_version: String,
    #[diesel(sql_type=Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

#[derive(QueryableByName)]
struct GroupRow {
    #[diesel(sql_type=BigInt)]
    set_group_id: i64,
    #[diesel(sql_type=Text)]
    kind: String,
    #[diesel(sql_type=BigInt)]
    list_order: i64,
    #[diesel(sql_type=BigInt)]
    valid: i64,
}

pub(super) fn sets_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&ClrMameProCursor>,
    limit: ClrMameProPageLimit,
) -> QueryResult<ClrMameProPage> {
    let mut connection = database.pool().get()?;
    connection.transaction(|connection| read_page(connection, snapshot, cursor, limit))
}

fn read_page(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    cursor: Option<&ClrMameProCursor>,
    limit: ClrMameProPageLimit,
) -> QueryResult<ClrMameProPage> {
    let registry = registry_id(connection)?;
    validate_cursor(cursor, registry, snapshot)?;
    let snapshot_row = load_snapshot(connection, snapshot, registry)?;
    let document = document::load(connection, snapshot.as_str())?;
    let group = load_group(connection, snapshot)?;
    let header_order = document.header.as_ref().map(|header| header.source_order);
    let anchor_order = load_anchor(connection, group, cursor, header_order)?;
    let rows = load_rows(connection, group, cursor, limit)?;
    validate_orders(&rows, cursor, anchor_order, header_order)?;
    if cursor.is_none() && rows.is_empty() {
        return Err(ClrMameProQueryError::InvalidMetadata(group));
    }
    let has_more = rows.len() > limit.0;
    let rows = rows.into_iter().take(limit.0).collect::<Vec<_>>();
    let sets = sets::hydrate(connection, snapshot.as_str(), rows)?;
    let next_cursor = if has_more {
        sets.last().map(|set| ClrMameProCursor {
            registry_id: registry,
            snapshot_key: snapshot.clone(),
            owner_id: set.id,
            list_order: set.list_order,
        })
    } else {
        None
    };
    Ok(ClrMameProPage {
        snapshot: snapshot_row,
        document,
        sets,
        next_cursor,
    })
}

fn validate_cursor(
    cursor: Option<&ClrMameProCursor>,
    registry: CatalogRegistryId,
    snapshot: &SnapshotKey,
) -> QueryResult<()> {
    if let Some(cursor) = cursor {
        if cursor.registry_id != registry {
            return Err(ClrMameProQueryError::CursorRegistryMismatch);
        }
        if &cursor.snapshot_key != snapshot {
            return Err(ClrMameProQueryError::CursorSnapshotMismatch);
        }
    }
    Ok(())
}

fn load_snapshot(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    registry: CatalogRegistryId,
) -> QueryResult<ClrMameProSnapshot> {
    let row = sql_query(queries::SNAPSHOT)
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotRow>(connection)
        .optional()?
        .ok_or_else(|| ClrMameProQueryError::NotPublished(snapshot.clone()))?;
    if row.valid != 1
        || row.snapshot_key != snapshot.as_str()
        || row.format != "clrmamepro-dat"
        || row.rules_version != "clrmamepro-declared-text-compat-v1"
    {
        return Err(ClrMameProQueryError::NotPublished(snapshot.clone()));
    }
    Ok(ClrMameProSnapshot {
        registry_id: registry,
        snapshot_key: snapshot.clone(),
        source_key: PublishingSourceKey::new(row.source_key),
        source_name: row.source_name,
        catalog_key: CatalogKey::new(row.catalog_key),
        catalog_name: row.catalog_name,
        document_key: row.document_key.parse::<DocumentKey>()?,
        interpretation_key: ParserInterpretationKey::from_persisted(row.interpretation_key),
        format: row.format,
        declared_version: row.declared_version,
    })
}

fn load_group(connection: &mut SqliteConnection, snapshot: &SnapshotKey) -> QueryResult<i64> {
    let rows = sql_query(queries::GROUPS)
        .bind::<Text, _>(snapshot.as_str())
        .load::<GroupRow>(connection)?;
    let [group] = rows.as_slice() else {
        return Err(ClrMameProQueryError::MismatchedOwner(0));
    };
    if group.valid != 1 || group.kind != "root" || group.list_order != 0 || group.set_group_id <= 0
    {
        return Err(ClrMameProQueryError::MismatchedOwner(group.set_group_id));
    }
    Ok(group.set_group_id)
}

fn load_rows(
    connection: &mut SqliteConnection,
    group: i64,
    cursor: Option<&ClrMameProCursor>,
    limit: ClrMameProPageLimit,
) -> QueryResult<Vec<SetRow>> {
    let bound = i64::try_from(limit.0)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or(ClrMameProQueryError::InvalidPageLimit(limit.0))?;
    let mut query = sql_query(queries::sets(cursor.is_some()))
        .into_boxed::<diesel::sqlite::Sqlite>()
        .bind::<BigInt, _>(group);
    if let Some(cursor) = cursor {
        query = query
            .bind::<BigInt, _>(cursor.list_order)
            .bind::<BigInt, _>(cursor.owner_id.as_i64());
    }
    Ok(query.bind::<BigInt, _>(bound).load::<SetRow>(connection)?)
}

fn load_anchor(
    connection: &mut SqliteConnection,
    group: i64,
    cursor: Option<&ClrMameProCursor>,
    header_order: Option<usize>,
) -> QueryResult<Option<usize>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let anchor = sql_query(queries::anchor())
        .bind::<BigInt, _>(group)
        .bind::<BigInt, _>(cursor.owner_id.as_i64())
        .bind::<BigInt, _>(cursor.list_order)
        .get_result::<SetRow>(connection)
        .optional()?
        .ok_or(ClrMameProQueryError::MismatchedOwner(
            cursor.owner_id.as_i64(),
        ))?;
    let order = sets::validate_row(&anchor)?;
    if header_order == Some(order) {
        return Err(ClrMameProQueryError::InvalidPositions(anchor.set_id));
    }
    Ok(Some(order))
}

fn validate_orders(
    rows: &[SetRow],
    cursor: Option<&ClrMameProCursor>,
    anchor_order: Option<usize>,
    header_order: Option<usize>,
) -> QueryResult<()> {
    let mut expected = cursor
        .map_or(Some(0), |cursor| cursor.list_order.checked_add(1))
        .ok_or(ClrMameProQueryError::InvalidMetadata(0))?;
    let mut previous = anchor_order;
    for row in rows {
        let order = sets::validate_row(row)?;
        if row.list_order != expected
            || previous.is_some_and(|prior| order <= prior)
            || header_order == Some(order)
        {
            return Err(ClrMameProQueryError::InvalidMetadata(row.set_id));
        }
        previous = Some(order);
        expected = expected
            .checked_add(1)
            .ok_or(ClrMameProQueryError::InvalidMetadata(row.set_id))?;
    }
    Ok(())
}
