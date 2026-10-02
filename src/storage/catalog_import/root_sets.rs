use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};

use crate::domain::{CatalogSetId, SnapshotKey};

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
    let format = sql_query(
        "SELECT format FROM catalog_snapshots JOIN parser_interpretations USING (interpretation_key) \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Format>(conn)?;
    let kind = RootSetKind::from_format(&format.format)?;
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         SELECT ?,'root',0 WHERE NOT EXISTS (SELECT 1 FROM catalog_set_groups \
             WHERE snapshot_key=? AND kind='root')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .execute(conn)?;
    let group = sql_query(
        "SELECT set_group_id AS value FROM catalog_set_groups WHERE snapshot_key = ? AND kind = 'root'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Id>(conn)?;
    let id = sql_query(
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
