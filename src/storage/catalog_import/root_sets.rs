use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    sql_types::{BigInt, Text},
    sqlite::Sqlite,
};

use crate::{
    domain::{CatalogSetId, SnapshotKey},
    storage::cached_sql::cached_sql,
};

use super::SnapshotSet;

#[derive(QueryableByName)]
struct Format {
    #[diesel(sql_type = Text)]
    format: String,
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

enum RootSetKind {
    MameMachine,
    LogiqxGame,
    ClrMameProSet,
    NoIntroPcGame,
}

impl RootSetKind {
    fn from_format(format: &str) -> crate::Result<Self> {
        match format {
            "mame-listxml" => Ok(Self::MameMachine),
            "logiqx" => Ok(Self::LogiqxGame),
            "clrmamepro-dat" => Ok(Self::ClrMameProSet),
            "no-intro-pc-xml" => Ok(Self::NoIntroPcGame),
            _ => Err(crate::Error::DatabaseSchema(format!(
                "format {format} cannot own a root catalog set"
            ))),
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::MameMachine => "mame_machine",
            Self::LogiqxGame => "logiqx_game",
            Self::ClrMameProSet => "cmp_set",
            Self::NoIntroPcGame => "no_intro_pc_game",
        }
    }
}

pub(super) fn insert(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    set: &SnapshotSet,
) -> crate::Result<CatalogSetId> {
    let format = cached_sql(
        "SELECT format FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key) \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Format>(conn)?;
    let kind = RootSetKind::from_format(&format.format)?;
    cached_sql(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         SELECT ?,'root',0 WHERE NOT EXISTS (SELECT 1 FROM catalog_set_groups \
             WHERE snapshot_key=? AND kind='root')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .execute(conn)?;
    let group = cached_sql(
        "SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key = ? AND kind = 'root'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Id>(conn)?;
    let id = cached_sql(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         VALUES (?, ?, (SELECT COALESCE(MAX(list_order),-1)+1 FROM catalog_sets WHERE set_group_id = ?), ?, ?, ?) \
         RETURNING set_id AS value",
    )
    .bind::<BigInt, _>(group.value)
    .bind::<Text, _>(kind.code())
    .bind::<BigInt, _>(group.value)
    .bind::<Text, _>(&set.name)
    .bind::<BigInt, _>(set.location.line)
    .bind::<BigInt, _>(set.location.column)
    .get_result::<Id>(conn)?;
    Ok(CatalogSetId::from_database(id.value))
}

/// Allocate a page of native root owners; RETURNING order is not significant.
pub(super) fn insert_many(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    sets: &[SnapshotSet],
) -> crate::Result<Vec<CatalogSetId>> {
    #[derive(QueryableByName)]
    struct Group {
        #[diesel(sql_type = BigInt)]
        group_id: i64,
        #[diesel(sql_type = BigInt)]
        next_order: i64,
    }
    if sets.is_empty() {
        return Ok(Vec::new());
    }
    let format = cached_sql(
        "SELECT format FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key) WHERE snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Format>(conn)?;
    let kind = RootSetKind::from_format(&format.format)?;
    cached_sql("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) SELECT ?,'root',0 WHERE NOT EXISTS (SELECT 1 FROM catalog_set_groups WHERE snapshot_key=? AND kind='root')")
        .bind::<Text, _>(snapshot.as_str())
        .bind::<Text, _>(snapshot.as_str())
        .execute(conn)?;
    let group = cached_sql("SELECT set_group_id AS group_id, (SELECT COALESCE(MAX(list_order),-1)+1 FROM catalog_sets WHERE set_group_id=groups.set_group_id) AS next_order FROM catalog_set_groups AS groups WHERE snapshot_key=? AND kind='root'")
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<Group>(conn)?;
    let mut result = Vec::with_capacity(sets.len());
    for page in sets.chunks(64) {
        let first_order = group
            .next_order
            .checked_add(super::checked_order(result.len(), "root sets")?)
            .ok_or_else(|| crate::Error::DatabaseSchema("catalog set order overflow".into()))?;
        let mut rows = SetInsert {
            group: group.group_id,
            kind: kind.code(),
            first_order,
            sets: page,
        }
        .load::<(i64, i64)>(conn)?;
        rows.sort_unstable_by_key(|row| row.1);
        if rows.len() != page.len()
            || rows.iter().enumerate().any(|(offset, row)| {
                i64::try_from(offset)
                    .ok()
                    .and_then(|offset| first_order.checked_add(offset))
                    != Some(row.1)
            })
        {
            return Err(crate::Error::DatabaseSchema(
                "bulk root owner correspondence is incomplete".into(),
            ));
        }
        result.extend(
            rows.into_iter()
                .map(|row| CatalogSetId::from_database(row.0)),
        );
    }
    Ok(result)
}

struct SetInsert<'a> {
    group: i64,
    kind: &'static str,
    first_order: i64,
    sets: &'a [SnapshotSet],
}

impl QueryId for SetInsert<'_> {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl Query for SetInsert<'_> {
    type SqlType = (BigInt, BigInt);
}

impl RunQueryDsl<SqliteConnection> for SetInsert<'_> {}

impl QueryFragment<Sqlite> for SetInsert<'_> {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> diesel::QueryResult<()> {
        pass.push_sql("INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES ");
        for (index, set) in self.sets.iter().enumerate() {
            if index > 0 {
                pass.push_sql(",");
            }
            i64::try_from(index)
                .ok()
                .and_then(|index| self.first_order.checked_add(index))
                .ok_or_else(|| {
                    diesel::result::Error::QueryBuilderError("catalog set order overflow".into())
                })?;
            pass.push_sql("(");
            pass.push_bind_param::<BigInt, _>(&self.group)?;
            pass.push_sql(",");
            pass.push_bind_param::<Text, _>(&self.kind)?;
            pass.push_sql(",");
            pass.push_bind_param::<BigInt, _>(&self.first_order)?;
            // Only the fixed page offset is SQL; the changing order stays bound.
            pass.push_sql("+");
            pass.push_sql(&index.to_string());
            pass.push_sql(",");
            pass.push_bind_param::<Text, _>(&set.name)?;
            pass.push_sql(",");
            pass.push_bind_param::<BigInt, _>(&set.location.line)?;
            pass.push_sql(",");
            pass.push_bind_param::<BigInt, _>(&set.location.column)?;
            pass.push_sql(")");
        }
        pass.push_sql(" RETURNING set_id,list_order");
        Ok(())
    }
}
