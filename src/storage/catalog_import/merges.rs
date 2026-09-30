use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};

use crate::{
    domain::{
        CatalogRecordKind, CatalogRecordRef, DocumentLocation, RelationshipType, SnapshotKey,
    },
    logiqx::RecordLocation,
    storage::relationships::{SourceRelationshipDraft, insert_source_assertion},
};

const MERGE_BATCH_SIZE: i64 = 256;

#[derive(Clone, Copy)]
pub(super) struct MergeDeclaration<'a> {
    pub(super) set_name: &'a str,
    pub(super) component_order: i64,
    pub(super) asset_name: &'a str,
    pub(super) merged_name: &'a str,
    pub(super) parent: &'a str,
    pub(super) role: &'a str,
    pub(super) sha1: Option<&'a [u8]>,
    pub(super) crc: Option<&'a [u8]>,
    pub(super) size: Option<u64>,
    pub(super) location: RecordLocation,
}

#[derive(QueryableByName)]
struct MergeAssetRow {
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = Text)]
    asset_name: String,
    #[diesel(sql_type = Text)]
    merged_name: String,
    #[diesel(sql_type = Text)]
    parent: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

pub(super) fn persist_merge_relationship(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    declaration: MergeDeclaration<'_>,
) -> crate::Result<()> {
    let parent_components = sql_query(
        "SELECT asset.component_order FROM asset_requirement_rows AS asset \
         JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ? AND sets.set_name = ? \
           AND asset.asset_name = ? AND asset.role = ? \
         ORDER BY asset.component_order LIMIT 2",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(declaration.parent)
    .bind::<Text, _>(declaration.merged_name)
    .bind::<Text, _>(declaration.role)
    .load::<super::ComponentOrderRow>(conn)?;
    let [parent_component] = parent_components.as_slice() else {
        return Ok(());
    };

    insert_source_assertion(
        conn,
        SourceRelationshipDraft {
            relation_type: RelationshipType::ExactContentIdentity,
            subject: CatalogRecordRef::new(
                snapshot.clone(),
                CatalogRecordKind::AssetRequirement,
                super::super::catalog_reconciliation::record_key(&(
                    declaration.set_name,
                    declaration.asset_name,
                    declaration.component_order,
                ))?,
            ),
            target: CatalogRecordRef::new(
                snapshot.clone(),
                CatalogRecordKind::AssetRequirement,
                super::super::catalog_reconciliation::record_key(&(
                    declaration.parent,
                    declaration.merged_name,
                    parent_component.component_order,
                ))?,
            ),
            source_field: "merge".to_owned(),
            source_location: Some(DocumentLocation {
                line: declaration.location.line,
                column: declaration.location.column,
            }),
            evidence: serde_json::json!({
                "declared_merge_name": declaration.merged_name,
                "parent_set_name": declaration.parent,
                "expected_sha1": declaration.sha1.map(hex::encode),
                "expected_crc": declaration.crc.map(hex::encode),
                "size": declaration.size,
            }),
        },
    )?;
    Ok(())
}

pub(super) fn persist_snapshot_merges(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<()> {
    let mut cursor = (String::new(), -1_i64);
    loop {
        let rows = sql_query(
            "SELECT s.set_name, a.component_order, a.asset_name, a.merge_name AS merged_name, \
                    COALESCE(mf_romof.target_name, json_extract(s.metadata_json, '$.romof'), s.parent_name) AS parent, \
                    a.role, \
                    (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion \
                     JOIN digest_values AS digest USING (digest_id) \
                     WHERE assertion.set_id = a.set_id \
                       AND assertion.component_order = a.component_order \
                       AND assertion.scope = a.evidence_scope \
                       AND digest.algorithm = 'sha1') AS sha1, \
                    (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion \
                     JOIN digest_values AS digest USING (digest_id) \
                     WHERE assertion.set_id = a.set_id \
                       AND assertion.component_order = a.component_order \
                       AND assertion.scope = a.evidence_scope \
                       AND digest.algorithm = 'crc32') AS crc, \
                    a.size, a.source_line, a.source_column \
             FROM asset_requirement_rows AS a \
             JOIN snapshot_sets AS s USING (set_id) \
             LEFT JOIN mame_machine_dependencies AS mf_romof \
               ON mf_romof.set_id = s.set_id \
              AND mf_romof.dependency_kind = 'romof' \
             WHERE s.snapshot_key = ? AND a.merge_name IS NOT NULL \
               AND COALESCE(mf_romof.target_name, json_extract(s.metadata_json, '$.romof'), s.parent_name) IS NOT NULL \
               AND (s.set_name, a.component_order) > (?, ?) \
             ORDER BY s.set_name, a.component_order LIMIT ?",
        )
        .bind::<Text, _>(snapshot.as_str())
        .bind::<Text, _>(&cursor.0)
        .bind::<BigInt, _>(cursor.1)
        .bind::<BigInt, _>(MERGE_BATCH_SIZE)
        .load::<MergeAssetRow>(conn)?;
        let Some(last) = rows.last() else {
            break;
        };

        for row in &rows {
            let size =
                row.size.map(u64::try_from).transpose().map_err(|_| {
                    crate::Error::InvalidPath("negative asset size in snapshot".into())
                })?;
            persist_merge_relationship(
                conn,
                snapshot,
                MergeDeclaration {
                    set_name: &row.set_name,
                    component_order: row.component_order,
                    asset_name: &row.asset_name,
                    merged_name: &row.merged_name,
                    parent: &row.parent,
                    role: &row.role,
                    sha1: row.sha1.as_deref(),
                    crc: row.crc.as_deref(),
                    size,
                    location: RecordLocation {
                        line: row.source_line,
                        column: row.source_column,
                    },
                },
            )?;
        }

        cursor = (last.set_name.clone(), last.component_order);
    }
    Ok(())
}
