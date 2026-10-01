use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use serde::Serialize;

use crate::{
    domain::{
        AssetRole, CatalogKey, CatalogRecordKind, CatalogRecordRef, CatalogSetId, Crc32Digest,
        EvidenceProvenance, EvidenceScope, ExpectedEvidence, Md5Digest, SnapshotKey,
    },
    reconciliation::{
        CatalogReconciliation, ExpectedAssetRequirement, RequirementSnapshot,
        reconcile_requirements,
    },
};

use super::db::Pool;

#[derive(QueryableByName)]
struct SnapshotIdentityRow {
    #[diesel(sql_type = Text)]
    catalog_key: String,
}

#[derive(QueryableByName)]
struct RequirementRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = Text)]
    asset_name: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    md5: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = Nullable<Text>)]
    merge_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    serial: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
}

pub(super) struct SnapshotRequirementLoad {
    pub(super) catalog: CatalogKey,
    pub(super) root_requirements: Vec<NativeRootRequirement>,
}

pub(super) struct NativeRootRequirement {
    pub(super) set_id: CatalogSetId,
    pub(super) set_name: String,
    pub(super) asset_name: String,
    pub(super) component_order: i64,
    pub(super) role: AssetRole,
    pub(super) expected: ExpectedEvidence,
}

#[derive(QueryableByName)]
struct SoftwareRequirementRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    list_name: String,
    #[diesel(sql_type = Text)]
    item_name: String,
    #[diesel(sql_type = Text)]
    part_name: String,
    #[diesel(sql_type = Text)]
    area_kind: String,
    #[diesel(sql_type = Text)]
    area_name: String,
    #[diesel(sql_type = BigInt)]
    area_order: i64,
    #[diesel(sql_type = BigInt)]
    component_order: i64,
    #[diesel(sql_type = Text)]
    component_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    component_name: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = Nullable<Binary>)]
    crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    evidence_provenance: String,
    #[diesel(sql_type = Nullable<Text>)]
    dump_status: Option<String>,
}

pub fn reconcile(
    pool: &Pool,
    left_key: &SnapshotKey,
    right_key: &SnapshotKey,
) -> crate::Result<CatalogReconciliation> {
    if left_key == right_key {
        return Err(crate::Error::InvalidPath(
            "catalog reconciliation requires two distinct snapshots".to_owned(),
        ));
    }
    let mut conn = pool.get()?;
    let (left, right, relationships) = conn.transaction::<_, crate::Error, _>(|conn| {
        Ok((
            reconciliation_snapshot(conn, left_key)?,
            reconciliation_snapshot(conn, right_key)?,
            super::relationships::explain_for_snapshots(conn, left_key, right_key)?,
        ))
    })?;
    Ok(reconcile_requirements(&left, &right, &relationships))
}

pub(super) fn snapshot_requirements(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<SnapshotRequirementLoad> {
    let identity = sql_query(
        "SELECT snapshot.catalog_key FROM catalog_snapshots AS snapshot \
         JOIN snapshot_publications AS publication USING (snapshot_key) \
         WHERE snapshot.snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SnapshotIdentityRow>(conn)
    .map_err(|error| match error {
        diesel::result::Error::NotFound => crate::Error::InvalidPath(format!(
            "catalog snapshot {} does not exist or has not been published",
            snapshot.as_str()
        )),
        error => error.into(),
    })?;
    let rows = sql_query(
        "SELECT sets.set_id, sets.set_name, rows.component_order, rows.asset_name, rows.role, rows.size, \
         (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = rows.set_id \
            AND assertion.component_order = rows.component_order \
            AND assertion.scope = rows.evidence_scope AND digest.algorithm = 'crc32') AS crc, \
         (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = rows.set_id \
            AND assertion.component_order = rows.component_order \
            AND assertion.scope = rows.evidence_scope AND digest.algorithm = 'md5') AS md5, \
         (SELECT digest.digest FROM asset_requirement_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.set_id = rows.set_id \
            AND assertion.component_order = rows.component_order \
            AND assertion.scope = rows.evidence_scope AND digest.algorithm = 'sha1') AS sha1, \
         rows.evidence_scope, rows.evidence_provenance, rows.merge_name, rows.dump_status, \
         rows.serial, rows.date \
         FROM asset_requirement_rows AS rows JOIN snapshot_sets AS sets USING (set_id) \
         WHERE sets.snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<RequirementRow>(conn)?;
    let root_requirements = rows
        .into_iter()
        .map(native_requirement)
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(SnapshotRequirementLoad {
        catalog: CatalogKey::new(identity.catalog_key),
        root_requirements,
    })
}

fn reconciliation_snapshot(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<RequirementSnapshot> {
    let loaded = snapshot_requirements(conn, snapshot)?;
    let mut requirements = loaded
        .root_requirements
        .into_iter()
        .map(|row| {
            Ok(ExpectedAssetRequirement {
                record: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::AssetRequirement,
                    record_key(&(&row.set_name, &row.asset_name, row.component_order))?,
                )
                .with_owner(row.set_id),
                owner: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::Set,
                    row.set_name.clone(),
                )
                .with_owner(row.set_id),
                role: row.role,
                expected: row.expected,
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;
    requirements.extend(software_requirements(conn, snapshot)?);
    Ok(RequirementSnapshot {
        catalog: loaded.catalog,
        snapshot: snapshot.clone(),
        requirements,
    })
}

fn native_requirement(row: RequirementRow) -> crate::Result<NativeRootRequirement> {
    let expected = ExpectedEvidence {
        scope: parse_scope(&row.evidence_scope),
        provenance: parse_provenance(&row.evidence_provenance),
        size: row
            .size
            .map(|size| {
                u64::try_from(size).map_err(|_| {
                    crate::Error::InvalidPath("negative catalog asset size".to_owned())
                })
            })
            .transpose()?,
        crc: digest(row.crc, "CRC")?.map(Crc32Digest),
        md5: digest(row.md5, "MD5")?.map(Md5Digest),
        sha1: digest(row.sha1, "SHA-1")?,
        merge: row.merge_name,
        dump_status: row.dump_status,
        serial: row.serial,
        date: row.date,
    };
    Ok(NativeRootRequirement {
        set_id: CatalogSetId::from_database(row.set_id),
        set_name: row.set_name,
        asset_name: row.asset_name,
        component_order: row.component_order,
        role: parse_role(&row.role),
        expected,
    })
}

fn software_requirements(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Vec<ExpectedAssetRequirement>> {
    let rows = sql_query(
        "SELECT occurrence.record_id AS set_id, component.list_name, component.item_name, component.part_name, \
         component.area_kind, component.area_name, component.area_order, \
         component.component_order, component.component_kind, component.component_name, \
         component.size, \
         (SELECT digest.digest FROM occurrence_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.occurrence_id = component.occurrence_id \
            AND assertion.scope = component.evidence_scope AND digest.algorithm = 'crc32') AS crc, \
         (SELECT digest.digest FROM occurrence_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.occurrence_id = component.occurrence_id \
            AND assertion.scope = component.evidence_scope AND digest.algorithm = 'sha1') AS sha1, \
         component.evidence_scope, \
         (SELECT CASE COUNT(DISTINCT assertion.provenance) \
                     WHEN 0 THEN 'source_declared' \
                     WHEN 1 THEN MIN(assertion.provenance) \
                     ELSE 'unknown' END \
                   FROM occurrence_digest_assertions AS assertion \
                   JOIN digest_values AS digest USING (digest_id) \
                   WHERE assertion.occurrence_id = component.occurrence_id \
                     AND assertion.scope = component.evidence_scope \
                     AND digest.algorithm IN ('crc32', 'sha1')) AS evidence_provenance, \
         component.dump_status \
         FROM software_components AS component \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) WHERE component.snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<SoftwareRequirementRow>(conn)?;
    rows.into_iter()
        .map(|row| {
            Ok(ExpectedAssetRequirement {
                record: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::AssetRequirement,
                    record_key(&(
                        &row.list_name,
                        &row.item_name,
                        &row.part_name,
                        &row.area_kind,
                        &row.area_name,
                        row.area_order,
                        &row.component_name,
                        row.component_order,
                    ))?,
                )
                .with_owner(CatalogSetId::from_database(row.set_id)),
                owner: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::SoftwareItem,
                    record_key(&(&row.list_name, &row.item_name))?,
                )
                .with_owner(CatalogSetId::from_database(row.set_id)),
                role: parse_role(&row.component_kind),
                expected: ExpectedEvidence {
                    scope: parse_scope(&row.evidence_scope),
                    provenance: parse_provenance(&row.evidence_provenance),
                    size: row
                        .size
                        .map(|size| {
                            u64::try_from(size).map_err(|_| {
                                crate::Error::InvalidPath(
                                    "negative software component size".to_owned(),
                                )
                            })
                        })
                        .transpose()?,
                    crc: digest(row.crc, "CRC")?.map(Crc32Digest),
                    sha1: digest(row.sha1, "SHA-1")?,
                    dump_status: row.dump_status,
                    ..ExpectedEvidence::default()
                },
            })
        })
        .collect()
}

pub(super) fn record_key(value: &impl Serialize) -> crate::Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn digest<const N: usize>(value: Option<Vec<u8>>, name: &str) -> crate::Result<Option<[u8; N]>> {
    value
        .map(|bytes| {
            bytes.try_into().map_err(|bytes: Vec<u8>| {
                crate::Error::InvalidHash(format!(
                    "stored {name} digest has {} bytes; expected {N}",
                    bytes.len()
                ))
            })
        })
        .transpose()
}

fn parse_role(value: &str) -> AssetRole {
    match value {
        "rom" => AssetRole::Rom,
        "disk" => AssetRole::Disk,
        _ => AssetRole::Other,
    }
}

fn parse_scope(value: &str) -> EvidenceScope {
    match value {
        "whole_asset" => EvidenceScope::WholeAsset,
        "disk_data" => EvidenceScope::DiskData,
        "chd_header_sha1" => EvidenceScope::ChdHeaderSha1,
        "track" => EvidenceScope::Track,
        _ => EvidenceScope::Unknown,
    }
}

fn parse_provenance(value: &str) -> EvidenceProvenance {
    match value {
        "source_declared" => EvidenceProvenance::SourceDeclared,
        "computed" => EvidenceProvenance::Computed,
        _ => EvidenceProvenance::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use diesel::{Connection, RunQueryDsl, SqliteConnection, sql_query};

    use crate::domain::SnapshotKey;

    use super::{record_key, snapshot_requirements};

    #[test]
    fn composite_record_keys_preserve_field_boundaries_and_optional_names() -> crate::Result<()> {
        let split_asset = record_key(&("a/b", "c", 0_i64))?;
        let split_set = record_key(&("a", "b/c", 0_i64))?;
        assert_ne!(split_asset, split_set);

        let split_software_item = record_key(&("list/item", "a"))?;
        let split_software_list = record_key(&("list", "item/a"))?;
        assert_ne!(split_software_item, split_software_list);

        let named_component = record_key(&(
            "list",
            "item",
            "part",
            "rom",
            "area",
            Some("<unnamed>"),
            0_i64,
        ))?;
        let unnamed_component =
            record_key(&("list", "item", "part", "rom", "area", None::<&str>, 0_i64))?;
        assert_ne!(named_component, unnamed_component);

        let first_area_component = record_key(&(
            "list", "item", "part", "rom", "program", 0_i64, "same.bin", 0_i64,
        ))?;
        let second_area_component = record_key(&(
            "list", "item", "part", "rom", "program", 1_i64, "same.bin", 0_i64,
        ))?;
        assert_ne!(first_area_component, second_area_component);
        Ok(())
    }

    #[test]
    fn identity_only_snapshots_are_not_reconciliation_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        sql_query(
            "CREATE TABLE catalog_snapshots (snapshot_key TEXT PRIMARY KEY, catalog_key TEXT NOT NULL)",
        )
        .execute(&mut conn)?;
        sql_query("CREATE TABLE snapshot_publications (snapshot_key TEXT PRIMARY KEY)")
            .execute(&mut conn)?;
        sql_query(
            "INSERT INTO catalog_snapshots (snapshot_key, catalog_key) VALUES ('identity-only', 'catalog')",
        )
        .execute(&mut conn)?;

        let result = snapshot_requirements(
            &mut conn,
            &SnapshotKey::from_persisted("identity-only".to_owned()),
        );

        assert!(
            matches!(result, Err(crate::Error::InvalidPath(message)) if message.contains("not been published"))
        );
        Ok(())
    }
}
