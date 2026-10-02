use camino::Utf8PathBuf;
use diesel::{Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, ExternalRecordRef, PublishingSourceKey, RelationshipAssertionKey,
        RelationshipClaim, RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin,
        RelationshipReview, RelationshipReviewDecision, RelationshipType, SnapshotKey,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MERGE_XML: &str = r#"<mame mameconfig="10">
  <machine name="parent"><description>Parent</description>
    <rom name="shared.bin" size="4" crc="12345678" sha1="1111111111111111111111111111111111111111"/>
    <disk name="shared.chd" sha1="2222222222222222222222222222222222222222"/>
  </machine>
  <machine name="child" romof="parent"><description>Child</description>
    <rom name="child.bin" merge="shared.bin" size="4" crc="12345678" sha1="1111111111111111111111111111111111111111"/>
    <disk name="child.chd" merge="shared.chd" sha1="2222222222222222222222222222222222222222"/>
    <rom name="no-merge.bin" size="1"/>
    <rom name="empty-merge.bin" merge=""/>
  </machine>
  <machine name="parentless"><description>Parentless</description>
    <rom name="orphan.bin" merge="missing.bin"/>
  </machine>
  <machine name="unresolved" romof="missing-parent"><description>Unresolved</description>
    <rom name="unresolved.bin" merge="missing.bin"/>
  </machine>
  <machine name="duplicate"><description>Duplicate one</description></machine>
  <machine name="duplicate"><description>Duplicate two</description></machine>
  <machine name="ambiguous" romof="duplicate"><description>Ambiguous</description>
    <rom name="ambiguous.bin" merge="same-name.bin"/>
  </machine>
</mame>"#;

fn request(path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("native-mame-merge-queries"),
        source_display_name: "Native MAME merge queries".into(),
        catalog_key: CatalogKey::new("native-mame-merge-queries"),
        catalog_display_name: "Native MAME merge queries".into(),
        scope: CatalogScope::Complete,
    }
}

fn import(database: &Database, path: &Utf8PathBuf, xml: &str) -> TestResult<SnapshotKey> {
    std::fs::write(path, xml)?;
    let report = app::import_catalog(database, &request(path.clone()))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report
        .snapshot_key
        .ok_or("published MAME snapshot missing")?)
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

fn table_count(connection: &mut SqliteConnection, table: &str) -> TestResult<i64> {
    Ok(sql_query(format!("SELECT count(*) AS count FROM {table}"))
        .get_result::<Count>(connection)?
        .count)
}

fn merge_explanations(
    database: &Database,
) -> TestResult<Vec<mame_coalesce::domain::RelationshipExplanation>> {
    Ok(app::explain_relationships(database)?
        .into_iter()
        .filter(|explanation| explanation.claim.relation_type == RelationshipType::SourceMerge)
        .collect())
}

fn record_review_and_support(
    database: &Database,
    native_key: &RelationshipAssertionKey,
) -> TestResult<RelationshipAssertionKey> {
    let candidate = app::record_relationship(
        database,
        &RelationshipClaim {
            relation_type: RelationshipType::CatalogCorrection,
            subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "native-mame-merge-test",
                "candidate",
            )),
            target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "native-mame-merge-test",
                "source-merge",
            )),
            origin: RelationshipOrigin::DerivedCandidate {
                rule_version: "native-mame-merge-support-v1".into(),
                supporting_assertions: vec![native_key.clone()],
            },
            evidence: RelationshipEvidence::Rationale {
                reason: "Review witness for the exact native MAME merge key".into(),
            },
        },
    )?;
    app::review_relationship(
        database,
        native_key,
        &RelationshipReview {
            decision: RelationshipReviewDecision::Accepted,
            note: "Reviewed the native merge declaration".into(),
            superseded_by: None,
        },
    )?;
    Ok(candidate)
}

#[test]
fn native_merge_explanations_preserve_media_occurrence_parent_and_scoped_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(&database, &path, MERGE_XML)?;
    std::fs::remove_file(&path)?;

    let explanations = merge_explanations(&database)?;
    assert_eq!(explanations.len(), 6);
    let child_rom = explanations
        .iter()
        .find(|row| {
            matches!(
                &row.claim.target,
                RelationshipEndpoint::MameMergeReference {
                    media_kind: mame_coalesce::domain::MameMergeKind::Rom,
                    merge_name,
                    ..
                } if merge_name == "shared.bin"
            )
        })
        .ok_or("native ROM merge explanation missing")?;
    assert_eq!(child_rom.source_field.as_deref(), Some("merge"));
    assert!(child_rom.source.is_some());
    assert!(child_rom.source_location.is_some());
    let occurrence_id = match &child_rom.claim.subject {
        RelationshipEndpoint::CatalogMediaEntry {
            snapshot: actual,
            occurrence_id,
        } if actual == &snapshot => occurrence_id.database_value(),
        _ => return Err("merge subject must be its source media occurrence".into()),
    };
    let machine_id = match &child_rom.claim.target {
        RelationshipEndpoint::MameMergeReference {
            snapshot: actual,
            machine_id,
            media_kind: mame_coalesce::domain::MameMergeKind::Rom,
            parent_name: Some(parent),
            merge_name,
        } if actual == &snapshot && parent == "parent" && merge_name == "shared.bin" => {
            machine_id.as_i64()
        }
        _ => return Err("merge target must preserve its same-edition machine context".into()),
    };
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let owner = sql_query(
        "SELECT count(*) AS count FROM asset_occurrences AS occurrence \
         JOIN catalog_sets AS catalog_set ON catalog_set.set_id=occurrence.record_id \
         JOIN catalog_set_groups AS group_row USING(set_group_id) \
         JOIN mame_machines AS machine ON machine.set_id=catalog_set.set_id \
         WHERE occurrence.occurrence_id=? AND group_row.snapshot_key=? AND machine.set_id=?",
    )
    .bind::<diesel::sql_types::BigInt, _>(occurrence_id)
    .bind::<diesel::sql_types::Text, _>(snapshot.as_str())
    .bind::<diesel::sql_types::BigInt, _>(machine_id)
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        owner.count, 1,
        "merge must point through its actual same-edition owner"
    );
    assert!(matches!(
        &child_rom.claim.evidence,
        RelationshipEvidence::Merge {
            declared_merge_name: Some(name), parent_set_name: Some(parent),
            expected_crc: Some(crc), size: Some(4), ..
        } if name == "shared.bin" && parent == "parent" && crc == "12345678"
    ));

    let child_disk = explanations
        .iter()
        .find(|row| {
            matches!(
                &row.claim.target,
                RelationshipEndpoint::MameMergeReference {
                    media_kind: mame_coalesce::domain::MameMergeKind::Disk,
                    merge_name,
                    ..
                } if merge_name == "shared.chd"
            )
        })
        .ok_or("native disk merge explanation missing")?;
    assert!(matches!(
        &child_disk.claim.target,
        RelationshipEndpoint::MameMergeReference {
            media_kind: mame_coalesce::domain::MameMergeKind::Disk,
            parent_name: Some(parent), ..
        } if parent == "parent"
    ));
    assert!(matches!(
        &child_disk.claim.evidence,
        RelationshipEvidence::Merge {
            declared_merge_name: Some(name), parent_set_name: Some(parent), ..
        } if name == "shared.chd" && parent == "parent"
    ));

    assert_unresolved_merge_literals(&explanations)?;
    assert_native_merge_storage(&mut connection)?;
    Ok(())
}

fn assert_unresolved_merge_literals(
    explanations: &[mame_coalesce::domain::RelationshipExplanation],
) -> TestResult {
    let parentless = explanations
        .iter()
        .find(|row| {
            matches!(
                &row.claim.target,
                RelationshipEndpoint::MameMergeReference { merge_name, parent_name: None, .. }
                    if merge_name == "missing.bin"
            )
        })
        .ok_or("parentless merge must remain an unresolved declaration")?;
    assert!(matches!(
        &parentless.claim.evidence,
        RelationshipEvidence::Merge {
            parent_set_name: None,
            ..
        }
    ));
    assert!(explanations.iter().any(|row| matches!(
        &row.claim.target,
        RelationshipEndpoint::MameMergeReference { merge_name, parent_name: Some(parent), .. }
            if merge_name == "missing.bin" && parent == "missing-parent"
    )));
    assert!(explanations.iter().any(|row| matches!(
        &row.claim.target,
        RelationshipEndpoint::MameMergeReference { merge_name, parent_name: Some(parent), .. }
            if merge_name == "same-name.bin" && parent == "duplicate"
    )));
    assert!(explanations.iter().any(|row| matches!(
        &row.claim.target,
        RelationshipEndpoint::MameMergeReference { merge_name, .. } if merge_name.is_empty()
    )));
    assert!(!explanations.iter().any(|row| matches!(
        &row.claim.target,
        RelationshipEndpoint::MameMergeReference { merge_name, .. } if merge_name == "no-merge.bin"
    )));

    Ok(())
}

fn assert_native_merge_storage(connection: &mut SqliteConnection) -> TestResult {
    assert_eq!(table_count(connection, "mame_rom_merges")?, 5);
    assert_eq!(table_count(connection, "mame_disk_merges")?, 1);
    assert_eq!(table_count(connection, "relationship_assertions")?, 0);
    let issued = sql_query(
        "SELECT count(*) AS count FROM catalog_relationships AS identity \
         JOIN reported_catalog_relationships AS reported USING(relationship_id) \
         WHERE identity.origin='source' \
           AND reported.source_reference_kind IN ('mame_rom_merge','mame_disk_merge')",
    )
    .get_result::<Count>(connection)?;
    assert_eq!(
        issued.count, 6,
        "each native merge owns one issued source key"
    );
    let scopes = sql_query(
        "SELECT (SELECT evidence_scope FROM mame_rom_claims WHERE occurrence_id=(SELECT occurrence_id FROM mame_rom_merges WHERE merge_name='shared.bin')) AS rom_scope, \
                (SELECT evidence_scope FROM mame_disk_claims WHERE occurrence_id=(SELECT occurrence_id FROM mame_disk_merges WHERE merge_name='shared.chd')) AS disk_scope",
    )
    .get_result::<ScopeRow>(connection)?;
    assert_eq!(scopes.rom_scope, "whole_asset");
    assert_eq!(scopes.disk_scope, "chd_header_sha1");
    Ok(())
}

#[derive(QueryableByName)]
struct ScopeRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    rom_scope: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    disk_scope: String,
}

#[test]
fn exact_native_merge_keys_support_review_reimport_backup_and_eof_rollback() -> TestResult {
    use mame_coalesce::{check_integrity, create_backup, restore_backup};

    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(&database, &path, MERGE_XML)?;
    let native_key = merge_explanations(&database)?
        .into_iter()
        .find(|row| {
            matches!(
                &row.claim.target,
                RelationshipEndpoint::MameMergeReference { merge_name, .. }
                    if merge_name == "shared.bin"
            )
        })
        .ok_or("native ROM merge assertion missing")?
        .assertion_key;
    let candidate_key = record_review_and_support(&database, &native_key)?;
    assert_eq!(import(&database, &path, MERGE_XML)?, snapshot);

    let explanations = app::explain_relationships(&database)?;
    let reviewed = explanations
        .iter()
        .find(|row| row.assertion_key == native_key)
        .ok_or("exact native merge key disappeared after reimport")?;
    assert_eq!(reviewed.review_history.len(), 1);
    assert_eq!(
        reviewed
            .latest_review
            .as_ref()
            .map(|review| review.decision),
        Some(RelationshipReviewDecision::Accepted)
    );
    assert!(explanations.iter().any(|row| {
        row.assertion_key == candidate_key
            && matches!(
                &row.claim.origin,
                RelationshipOrigin::DerivedCandidate { supporting_assertions, .. }
                    if supporting_assertions == std::slice::from_ref(&native_key)
            )
    }));

    std::fs::write(
        &path,
        "<mame mameconfig='10'><machine name='late'><description>Late</description><rom name='rolled-back.bin' size='1' merge='lost' /><disk name='rolled-back.chd' merge='lost-chd' /></machine>",
    )?;
    let failed = app::import_catalog(&database, &request(path))?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(merge_explanations(&database)?.len(), 6);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    assert_eq!(table_count(&mut connection, "mame_rom_merges")?, 5);
    assert_eq!(table_count(&mut connection, "mame_disk_merges")?, 1);

    drop(database);
    let backup = Utf8PathBuf::try_from(directory.path().join("catalog.backup"))?;
    let restored_path = Utf8PathBuf::try_from(directory.path().join("restored.sqlite"))?;
    create_backup(&database_path, &backup)?;
    restore_backup(&backup, &restored_path, RestorePolicy::CreateNew)?;
    let restored = Database::open(&restored_path)?;
    assert_eq!(app::explain_relationships(&restored)?, explanations);
    assert_eq!(merge_explanations(&restored)?.len(), 6);
    drop(restored);
    assert!(check_integrity(&restored_path)?.is_clean());
    Ok(())
}
