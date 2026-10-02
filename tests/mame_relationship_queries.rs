use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{MameFilePayload, occurrences_for_ids},
    catalog_machines::{MachineDependency, MachinePageLimit, machines_for_snapshot},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, PublishingSourceKey, RelationshipAssertionKey, RelationshipClaim,
        RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin, RelationshipReview,
        RelationshipReviewDecision, RelationshipType, SnapshotKey,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const EDITION_ONE: &str = r#"<mame build="edition-one" mameconfig="10">
  <machine name="parent"><description>Parent</description><rom name="base.bin" size="1"/></machine>
  <machine name="game" cloneof="parent" romof="missing-bios" sampleof="missing-samples">
    <description>Game</description>
    <rom name="shared.bin" size="2" merge="parent-shared.bin"/>
    <device_ref name="sound" tag=":sound"/>
    <disk name="shared.chd" merge="parent-shared.chd"/>
    <device_ref name="sound" tag=":sound:secondary"/>
    <rom name="local.bin" size="3"/>
  </machine>
  <machine name="orphan" cloneof="missing-parent"><description>Orphan</description></machine>
</mame>"#;

const EDITION_TWO: &str = r#"<mame build="edition-two" mameconfig="10">
  <machine name="parent"><description>Parent, revised</description><rom name="base.bin" size="1"/></machine>
  <machine name="game" cloneof="missing-parent" romof="missing-bios" sampleof="missing-samples">
    <description>Game</description>
    <rom name="shared.bin" size="2" merge="parent-shared.bin"/>
    <device_ref name="sound" tag=":sound"/>
    <disk name="shared.chd" merge="parent-shared.chd"/>
    <device_ref name="sound" tag=":sound:secondary"/>
    <rom name="local.bin" size="3"/>
  </machine>
</mame>"#;

fn request(path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("mame-relationship-editions"),
        source_display_name: "MAME relationship editions".to_owned(),
        catalog_key: CatalogKey::new("mame-relationship-editions"),
        catalog_display_name: "MAME relationship editions".to_owned(),
        scope: CatalogScope::Complete,
    }
}

fn import(database: &Database, path: &Utf8PathBuf, xml: &str) -> TestResult<SnapshotKey> {
    std::fs::write(path, xml)?;
    let report = app::import_catalog(database, &request(path.clone()))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("published snapshot missing")?)
}

#[test]
fn typed_machine_query_preserves_relationship_variants_tags_merges_and_source_order() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import(&database, &document, EDITION_ONE)?;

    std::fs::remove_file(&document)?;
    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(10)?)?;
    let game = page
        .machines
        .iter()
        .find(|machine| machine.name == "game")
        .ok_or("game machine missing")?;
    assert_eq!(game.list_order, 1);
    assert_eq!(
        game.assets
            .iter()
            .map(|asset| asset.source_order)
            .collect::<Vec<_>>(),
        [1, 3, 5]
    );
    assert_eq!(
        game.assets
            .iter()
            .map(|asset| asset.kind)
            .collect::<Vec<_>>(),
        [
            mame_coalesce::catalog_machines::MachineAssetKind::Rom,
            mame_coalesce::catalog_machines::MachineAssetKind::Disk,
            mame_coalesce::catalog_machines::MachineAssetKind::Rom,
        ]
    );
    let dependencies = &game.dependencies;
    assert!(dependencies.iter().any(|dependency| matches!(
        dependency,
        MachineDependency::CloneOf { target_name, .. } if target_name == "parent"
    )));
    assert!(dependencies.iter().any(|dependency| matches!(
        dependency,
        MachineDependency::RomOf { target_name, .. } if target_name == "missing-bios"
    )));
    assert!(dependencies.iter().any(|dependency| matches!(
        dependency,
        MachineDependency::SampleOf { target_name, .. } if target_name == "missing-samples"
    )));
    let orphan = page
        .machines
        .iter()
        .find(|machine| machine.name == "orphan")
        .ok_or("orphan machine missing")?;
    assert!(orphan.dependencies.iter().any(|dependency| matches!(
        dependency,
        MachineDependency::CloneOf { target_name, .. } if target_name == "missing-parent"
    )));
    let device_references = dependencies
        .iter()
        .filter_map(|dependency| match dependency {
            MachineDependency::DeviceReference(reference) => Some(reference),
            MachineDependency::CloneOf { .. }
            | MachineDependency::RomOf { .. }
            | MachineDependency::SampleOf { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(device_references.len(), 2);
    assert_eq!(
        device_references
            .iter()
            .map(|reference| (
                reference.name.as_str(),
                reference.tag.as_str(),
                reference.source_order
            ))
            .collect::<Vec<_>>(),
        [("sound", ":sound", 2), ("sound", ":sound:secondary", 4)]
    );

    let ids = game
        .assets
        .iter()
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&database, &ids)?;
    let media = files
        .iter()
        .filter_map(|file| file.mame_file.as_ref())
        .collect::<Vec<_>>();
    assert_eq!(media.len(), 3);
    assert!(matches!(
        media[0],
        MameFilePayload::Rom(rom) if rom.merge_name.as_deref() == Some("parent-shared.bin")
    ));
    assert!(matches!(
        media[1],
        MameFilePayload::Disk(disk) if disk.merge_name.as_deref() == Some("parent-shared.chd")
    ));
    assert!(matches!(
        media[2],
        MameFilePayload::Rom(rom) if rom.merge_name.is_none()
    ));
    Ok(())
}

fn record_native_support_and_review(
    database: &Database,
    subject: &RelationshipEndpoint,
    native_key: &RelationshipAssertionKey,
    review: &RelationshipReview,
) -> TestResult<RelationshipAssertionKey> {
    let candidate = app::record_relationship(
        database,
        &RelationshipClaim {
            relation_type: RelationshipType::CatalogCorrection,
            subject: subject.clone(),
            target: subject.clone(),
            origin: RelationshipOrigin::DerivedCandidate {
                rule: mame_coalesce::domain::RelationshipRule::new(
                    "native-mame-support-witness",
                    "v1",
                    "Native MAME support witness",
                )?,
                supporting_assertions: vec![native_key.clone()],
            },
            evidence: RelationshipEvidence::Rationale {
                reason: "Native source relationship support witness".into(),
            },
        },
    )?;
    app::review_relationship(database, native_key, review)?;
    Ok(candidate)
}

fn assert_clone_relationship_history(
    database: &Database,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> TestResult {
    let diff = app::diff_catalog_snapshots(database, previous, current)?;
    let game_history = diff
        .records
        .iter()
        .find(|record| record.set_name == "game")
        .ok_or("game snapshot history missing")?;
    assert!(
        game_history
            .relationship_evidence
            .iter()
            .any(|explanation| {
                explanation.claim.relation_type == RelationshipType::SourceParentClone
            })
    );
    Ok(())
}

#[test]
fn source_clone_explanations_and_snapshot_history_survive_reimport_and_backup() -> TestResult {
    use mame_coalesce::{RestorePolicy, check_integrity, create_backup, restore_backup};

    let directory = tempfile::tempdir()?;
    let document = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let previous = import(&database, &document, EDITION_ONE)?;
    let current = import(&database, &document, EDITION_TWO)?;
    assert_ne!(previous, current);

    let history =
        app::catalog_snapshot_history(&database, &CatalogKey::new("mame-relationship-editions"))?;
    assert_eq!(history.len(), 2);
    assert!(history.iter().any(|entry| entry.snapshot == previous));
    assert!(history.iter().any(|entry| entry.snapshot == current));

    let explanations = app::explain_relationships(&database)?;
    let current_clone = explanations
        .iter()
        .find(|explanation| {
            explanation.claim.relation_type == RelationshipType::SourceParentClone
                && matches!(
                    &explanation.claim.subject,
                    RelationshipEndpoint::CatalogRecord(record)
                        if record.snapshot == current && record.key.as_str() == "game"
                )
        })
        .ok_or("current source clone assertion missing")?;
    assert_eq!(current_clone.source_field.as_deref(), Some("cloneof"));
    assert!(current_clone.source.is_some());
    assert!(current_clone.source_location.is_some());
    assert!(matches!(
        &current_clone.claim.target,
        RelationshipEndpoint::CatalogRecord(record) if record.key.as_str() == "missing-parent"
    ));

    let native_key = current_clone.assertion_key.clone();
    let review = RelationshipReview {
        decision: RelationshipReviewDecision::Accepted,
        note: "Native source relationship review witness".into(),
        superseded_by: None,
    };
    let candidate = record_native_support_and_review(
        &database,
        &current_clone.claim.subject,
        &native_key,
        &review,
    )?;

    assert_clone_relationship_history(&database, &previous, &current)?;

    std::fs::remove_file(&document)?;
    let previous_page =
        machines_for_snapshot(&database, &previous, None, MachinePageLimit::new(10)?)?;
    assert!(
        previous_page
            .machines
            .iter()
            .any(|machine| machine.name == "game")
    );
    std::fs::write(&document, EDITION_TWO)?;
    assert_eq!(import(&database, &document, EDITION_TWO)?, current);

    let explanations = app::explain_relationships(&database)?;
    let reviewed_native = explanations
        .iter()
        .find(|explanation| explanation.assertion_key == native_key)
        .ok_or("reviewed native source key missing after reimport")?;
    assert_eq!(reviewed_native.latest_review.as_ref(), Some(&review));
    assert_eq!(reviewed_native.review_history.len(), 1);
    let supported_candidate = explanations
        .iter()
        .find(|explanation| explanation.assertion_key == candidate)
        .ok_or("native-supported candidate missing after reimport")?;
    assert!(matches!(
        &supported_candidate.claim.origin,
        RelationshipOrigin::DerivedCandidate { supporting_assertions, .. }
            if supporting_assertions.as_slice() == std::slice::from_ref(&native_key)
    ));

    drop(database);
    let backup = Utf8PathBuf::try_from(directory.path().join("catalog.backup"))?;
    let restored_path = Utf8PathBuf::try_from(directory.path().join("restored.sqlite"))?;
    create_backup(&database_path, &backup)?;
    restore_backup(&backup, &restored_path, RestorePolicy::CreateNew)?;
    let restored = Database::open(&restored_path)?;
    assert_eq!(app::explain_relationships(&restored)?, explanations);
    assert_eq!(
        machines_for_snapshot(&restored, &current, None, MachinePageLimit::new(10)?)?,
        machines_for_snapshot(
            &Database::open(&database_path)?,
            &current,
            None,
            MachinePageLimit::new(10)?
        )?
    );
    drop(restored);
    assert!(check_integrity(&restored_path)?.is_clean());
    Ok(())
}
