use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use serde::Serialize;

use crate::{
    domain::{
        AssetRole, CatalogKey, CatalogRecordKind, CatalogRecordRef, Crc32Digest,
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

#[derive(QueryableByName)]
struct SoftwareRequirementRow {
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
    let (left, right) = {
        let mut conn = pool.get()?;
        (
            snapshot_requirements(&mut conn, left_key)?,
            snapshot_requirements(&mut conn, right_key)?,
        )
    };
    let relationships = super::relationships::explain_all(pool)?;
    Ok(reconcile_requirements(&left, &right, &relationships))
}

fn snapshot_requirements(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<RequirementSnapshot> {
    let identity = sql_query("SELECT catalog_key FROM catalog_snapshots WHERE snapshot_key = ?")
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<SnapshotIdentityRow>(conn)
        .map_err(|error| match error {
            diesel::result::Error::NotFound => crate::Error::InvalidPath(format!(
                "catalog snapshot {} does not exist",
                snapshot.as_str()
            )),
            error => error.into(),
        })?;
    let mut requirements = sql_query(
        "SELECT set_name, component_order, asset_name, role, size, crc, md5, sha1, \
         evidence_scope, evidence_provenance, merge_name, dump_status, serial, date \
         FROM asset_requirements WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<RequirementRow>(conn)?
    .into_iter()
    .map(|row| {
        Ok(ExpectedAssetRequirement {
            record: CatalogRecordRef::new(
                snapshot.clone(),
                CatalogRecordKind::AssetRequirement,
                record_key(&(&row.set_name, &row.asset_name, row.component_order))?,
            ),
            owner: CatalogRecordRef::new(
                snapshot.clone(),
                CatalogRecordKind::Set,
                row.set_name.clone(),
            ),
            role: parse_role(&row.role),
            expected: ExpectedEvidence {
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
            },
        })
    })
    .collect::<crate::Result<Vec<_>>>()?;
    requirements.extend(software_requirements(conn, snapshot)?);
    Ok(RequirementSnapshot {
        catalog: CatalogKey::new(identity.catalog_key),
        snapshot: snapshot.clone(),
        requirements,
    })
}

fn software_requirements(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> crate::Result<Vec<ExpectedAssetRequirement>> {
    let rows = sql_query(
        "SELECT list_name, item_name, part_name, area_kind, area_name, component_order, \
         component_kind, component_name, size, crc, sha1, dump_status \
         FROM software_components WHERE snapshot_key = ?",
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
                        &row.component_name,
                        row.component_order,
                    ))?,
                ),
                owner: CatalogRecordRef::new(
                    snapshot.clone(),
                    CatalogRecordKind::SoftwareItem,
                    record_key(&(&row.list_name, &row.item_name))?,
                ),
                role: parse_role(&row.component_kind),
                expected: ExpectedEvidence {
                    scope: if row.component_kind == "disk" {
                        EvidenceScope::DiskData
                    } else {
                        EvidenceScope::WholeAsset
                    },
                    provenance: EvidenceProvenance::SourceDeclared,
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
        "track" => EvidenceScope::Track,
        _ => EvidenceScope::Unknown,
    }
}

fn parse_provenance(value: &str) -> EvidenceProvenance {
    match value {
        "source_declared" => EvidenceProvenance::SourceDeclared,
        "computed" => EvidenceProvenance::Computed,
        "legacy_cache" => EvidenceProvenance::LegacyCache,
        _ => EvidenceProvenance::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::record_key;

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
        Ok(())
    }
}
