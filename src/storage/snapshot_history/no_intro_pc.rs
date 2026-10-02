//! Semantic history from native synthetic P/C owners, never source reparsing.
use crate::domain::SnapshotKey;
use diesel::{
    OptionalExtension, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DocumentMetadata {
    header: Option<Vec<HeaderField>>,
    layout: Vec<RootChild>,
}

#[derive(Debug, PartialEq, Eq)]
enum HeaderField {
    Name(String),
    Description(String),
    Version(String),
}

#[derive(Debug, PartialEq, Eq)]
enum RootChild {
    Header,
    Game(String),
}

#[derive(QueryableByName)]
struct Presence {
    #[diesel(sql_type=BigInt)]
    present: i64,
}

#[derive(QueryableByName)]
struct FieldRow {
    #[diesel(sql_type=Text)]
    kind: String,
    #[diesel(sql_type=Text)]
    value: String,
}

pub(super) fn load_document(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Option<DocumentMetadata>> {
    let Some(document) = sql_query(
        "SELECT header_present AS present FROM no_intro_pc_documents WHERE snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Presence>(conn)
    .optional()?
    else {
        return Ok(None);
    };
    let header = if document.present == 1 {
        let rows=sql_query("SELECT 'name' AS kind,name_text AS value,source_order FROM no_intro_pc_header_names WHERE snapshot_key=? UNION ALL SELECT 'description',description_text,source_order FROM no_intro_pc_header_descriptions WHERE snapshot_key=? UNION ALL SELECT 'version',version_text,version_order FROM no_intro_pc_headers WHERE snapshot_key=? AND version_text IS NOT NULL ORDER BY source_order")
            .bind::<Text,_>(snapshot.as_str()).bind::<Text,_>(snapshot.as_str()).bind::<Text,_>(snapshot.as_str()).load::<FieldRow>(conn)?;
        Some(
            rows.into_iter()
                .map(|row| match row.kind.as_str() {
                    "name" => Ok(HeaderField::Name(row.value)),
                    "description" => Ok(HeaderField::Description(row.value)),
                    "version" => Ok(HeaderField::Version(row.value)),
                    _ => Err(crate::Error::DatabaseSchema(
                        "unknown native P/C header field".into(),
                    )),
                })
                .collect::<crate::Result<Vec<_>>>()?,
        )
    } else {
        None
    };
    let rows=sql_query("SELECT 'header' AS kind,'' AS value,source_order FROM no_intro_pc_headers WHERE snapshot_key=? UNION ALL SELECT 'game',sets.set_name,game.document_order FROM no_intro_pc_games AS game JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=? ORDER BY source_order")
        .bind::<Text,_>(snapshot.as_str()).bind::<Text,_>(snapshot.as_str()).load::<FieldRow>(conn)?;
    let layout = rows
        .into_iter()
        .map(|row| match row.kind.as_str() {
            "header" => Ok(RootChild::Header),
            "game" => Ok(RootChild::Game(row.value)),
            _ => Err(crate::Error::DatabaseSchema(
                "unknown native P/C root child".into(),
            )),
        })
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(Some(DocumentMetadata { header, layout }))
}

#[derive(QueryableByName)]
struct ChildRow {
    #[diesel(sql_type=BigInt)]
    set_id: i64,
    #[diesel(sql_type=Text)]
    kind: String,
    #[diesel(sql_type=Text)]
    value: String,
}

pub(super) fn load_game_layouts(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<BTreeMap<i64, Vec<serde_json::Value>>> {
    let rows=sql_query("WITH requested_sets AS (SELECT sets.set_id FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) WHERE groups.snapshot_key=?) SELECT game.set_id AS set_id,'description' AS kind,'' AS value,game.description_order AS source_order FROM requested_sets AS request JOIN no_intro_pc_games AS game ON game.set_id=request.set_id WHERE game.description IS NOT NULL UNION ALL SELECT occurrence.record_id,'rom',rom.name,rom.source_order FROM requested_sets AS request JOIN asset_occurrences AS occurrence ON occurrence.record_id=request.set_id JOIN no_intro_pc_file_claims AS rom USING(occurrence_id) ORDER BY 1,4")
        .bind::<Text,_>(snapshot.as_str()).load::<ChildRow>(conn)?;
    let mut layouts = BTreeMap::<i64, Vec<serde_json::Value>>::new();
    for row in rows {
        let child = match row.kind.as_str() {
            "description" => serde_json::json!({"description":true}),
            "rom" => serde_json::json!({"rom":row.value}),
            _ => {
                return Err(crate::Error::DatabaseSchema(
                    "unknown native P/C game child".into(),
                ));
            }
        };
        layouts.entry(row.set_id).or_default().push(child);
    }
    Ok(layouts)
}
