use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        AssetRole, CatalogKey, CatalogRecordKind, CatalogRecordRef, CatalogScope, Crc32Digest,
        EvidenceScope, ExpectedEvidence, Md5Digest, PublishingSourceKey, RelationshipAssertionKey,
        RelationshipClaim, RelationshipEndpoint, RelationshipExplanation, RelationshipOrigin,
        RelationshipType, SnapshotKey,
    },
    reconciliation::{
        ExpectedAssetRequirement, ReconciliationStatus, RequirementSnapshot,
        compare_expected_evidence, reconcile_requirements,
    },
    resolution::EvidenceField,
};

#[test]
fn matching_strong_digest_is_compatible_but_preserves_weaker_conflicts() {
    let left = ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        sha1: Some([1; 20]),
        crc: Some(Crc32Digest([1; 4])),
        ..ExpectedEvidence::default()
    };
    let right = ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        sha1: Some([1; 20]),
        crc: Some(Crc32Digest([2; 4])),
        ..ExpectedEvidence::default()
    };

    let result = compare_expected_evidence(&left, &right);

    assert_eq!(result.status, ReconciliationStatus::Compatible);
    assert_eq!(result.agreements, [EvidenceField::Sha1]);
    assert_eq!(result.contradictions, [EvidenceField::Crc]);
}

#[test]
fn stronger_digest_conflict_blocks_matching_weaker_evidence() {
    let left = ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        sha1: Some([1; 20]),
        md5: Some(Md5Digest([1; 16])),
        crc: Some(Crc32Digest([1; 4])),
        size: Some(64),
        ..ExpectedEvidence::default()
    };
    let right = ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        sha1: Some([2; 20]),
        md5: Some(Md5Digest([1; 16])),
        crc: Some(Crc32Digest([1; 4])),
        size: Some(64),
        ..ExpectedEvidence::default()
    };

    let result = compare_expected_evidence(&left, &right);

    assert_eq!(result.status, ReconciliationStatus::Contradictory);
    assert_eq!(
        result.agreements,
        [EvidenceField::Md5, EvidenceField::Crc, EvidenceField::Size]
    );
    assert_eq!(result.contradictions, [EvidenceField::Sha1]);
}

#[test]
fn weak_partial_evidence_is_only_a_candidate_and_scope_mismatch_is_unknown() {
    let crc_only = ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        crc: Some(Crc32Digest([7; 4])),
        ..ExpectedEvidence::default()
    };
    let matching_crc = crc_only.clone();
    let weak = compare_expected_evidence(&crc_only, &matching_crc);
    assert_eq!(weak.status, ReconciliationStatus::Candidate);

    let incompatible_scope = ExpectedEvidence {
        scope: EvidenceScope::DiskData,
        crc: Some(Crc32Digest([7; 4])),
        ..ExpectedEvidence::default()
    };
    let scoped = compare_expected_evidence(&crc_only, &incompatible_scope);
    assert_eq!(scoped.status, ReconciliationStatus::Unknown);
    assert!(scoped.agreements.is_empty());
    assert!(scoped.contradictions.is_empty());

    let empty =
        compare_expected_evidence(&ExpectedEvidence::default(), &ExpectedEvidence::default());
    assert_eq!(empty.status, ReconciliationStatus::Unknown);
}

fn asset(
    snapshot: &SnapshotKey,
    set: &str,
    name: &str,
    crc: Option<[u8; 4]>,
    sha1: Option<[u8; 20]>,
) -> ExpectedAssetRequirement {
    ExpectedAssetRequirement {
        record: CatalogRecordRef::new(
            snapshot.clone(),
            CatalogRecordKind::AssetRequirement,
            format!("{set}/{name}#0"),
        ),
        owner: CatalogRecordRef::new(snapshot.clone(), CatalogRecordKind::Set, set),
        role: AssetRole::Rom,
        expected: ExpectedEvidence {
            scope: EvidenceScope::WholeAsset,
            provenance: mame_coalesce::domain::EvidenceProvenance::SourceDeclared,
            crc: crc.map(Crc32Digest),
            sha1,
            ..ExpectedEvidence::default()
        },
    }
}

fn snapshot_key(catalog: &CatalogKey, bytes: &[u8]) -> SnapshotKey {
    SnapshotKey::new(
        catalog,
        &mame_coalesce::domain::DocumentKey::from_bytes(bytes),
        &mame_coalesce::domain::ParserInterpretationKey::for_format(
            "logiqx",
            &CatalogScope::Complete,
        ),
    )
}

#[test]
fn shared_component_evidence_produces_asset_candidate_not_release_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let left_catalog = CatalogKey::new("publisher-one");
    let right_catalog = CatalogKey::new("publisher-two");
    let left_key = snapshot_key(&left_catalog, b"left document");
    let right_key = snapshot_key(&right_catalog, b"right document");
    let left = RequirementSnapshot {
        catalog: left_catalog,
        snapshot: left_key.clone(),
        requirements: vec![asset(
            &left_key,
            "release-one",
            "shared.bin",
            None,
            Some([8; 20]),
        )],
    };
    let right = RequirementSnapshot {
        catalog: right_catalog,
        snapshot: right_key.clone(),
        requirements: vec![asset(
            &right_key,
            "release-two",
            "renamed.bin",
            None,
            Some([8; 20]),
        )],
    };

    let report = reconcile_requirements(&left, &right, &[]);

    assert_eq!(report.outcomes.len(), 1);
    let result = &report.outcomes[0];
    assert_eq!(result.status, ReconciliationStatus::Compatible);
    assert_eq!(
        result
            .left
            .as_ref()
            .ok_or("left requirement missing")?
            .key
            .as_str(),
        "release-one/shared.bin#0"
    );
    assert_eq!(
        result
            .right
            .as_ref()
            .ok_or("right requirement missing")?
            .key
            .as_str(),
        "release-two/renamed.bin#0"
    );
    let candidate = result
        .relationship_candidate()
        .ok_or("compatible comparison should offer a reversible candidate")?;
    assert!(matches!(
        candidate.subject,
        RelationshipEndpoint::CatalogRecord(_)
    ));
    assert!(matches!(
        candidate.target,
        RelationshipEndpoint::CatalogRecord(_)
    ));
    assert!(matches!(
        candidate.origin,
        RelationshipOrigin::DerivedCandidate { .. }
    ));
    Ok(())
}

#[test]
fn owner_assertions_are_context_only_and_never_support_asset_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let left_catalog = CatalogKey::new("publisher-one");
    let right_catalog = CatalogKey::new("publisher-two");
    let left_key = snapshot_key(&left_catalog, b"left document");
    let right_key = snapshot_key(&right_catalog, b"right document");
    let left_requirement = asset(&left_key, "release-one", "shared.bin", None, Some([8; 20]));
    let right_requirement = asset(
        &right_key,
        "release-two",
        "renamed.bin",
        None,
        Some([8; 20]),
    );
    let parent = CatalogRecordRef::new(left_key.clone(), CatalogRecordKind::Set, "declared-parent");
    let source_assertion = RelationshipExplanation {
        assertion_key: RelationshipAssertionKey::new("source-parent-claim"),
        claim: RelationshipClaim {
            relation_type: RelationshipType::SourceParentClone,
            subject: RelationshipEndpoint::CatalogRecord(left_requirement.owner.clone()),
            target: RelationshipEndpoint::CatalogRecord(parent),
            origin: RelationshipOrigin::SourceAssertion {
                snapshot: left_key.clone(),
                field: "cloneof".to_owned(),
                location: None,
            },
            evidence: serde_json::json!({"source": "fixture"}),
        },
        source_field: Some("cloneof".to_owned()),
        source_location: None,
        source: None,
        latest_review: None,
        review_history: vec![],
    };
    let left = RequirementSnapshot {
        catalog: left_catalog,
        snapshot: left_key,
        requirements: vec![left_requirement],
    };
    let right = RequirementSnapshot {
        catalog: right_catalog,
        snapshot: right_key,
        requirements: vec![right_requirement],
    };

    let report = reconcile_requirements(&left, &right, &[source_assertion]);
    let outcome = &report.outcomes[0];
    assert_eq!(outcome.relationship_evidence.len(), 1);
    let candidate = outcome
        .relationship_candidate()
        .ok_or("strong digest candidate missing")?;
    assert!(matches!(
        candidate.origin,
        RelationshipOrigin::DerivedCandidate {
            supporting_assertions,
            ..
        } if supporting_assertions.is_empty()
    ));
    Ok(())
}

#[test]
fn duplicate_weak_signatures_are_ambiguous_and_record_order_is_irrelevant() {
    let left_catalog = CatalogKey::new("publisher-one");
    let right_catalog = CatalogKey::new("publisher-two");
    let left_key = snapshot_key(&left_catalog, b"left");
    let right_key = snapshot_key(&right_catalog, b"right");
    let left = RequirementSnapshot {
        catalog: left_catalog,
        snapshot: left_key.clone(),
        requirements: vec![
            asset(&left_key, "one", "one.bin", Some([3; 4]), None),
            asset(&left_key, "two", "two.bin", Some([3; 4]), None),
        ],
    };
    let mut right = RequirementSnapshot {
        catalog: right_catalog,
        snapshot: right_key.clone(),
        requirements: vec![
            asset(&right_key, "three", "three.bin", Some([3; 4]), None),
            asset(&right_key, "four", "four.bin", Some([3; 4]), None),
        ],
    };

    let expected = reconcile_requirements(&left, &right, &[]);
    right.requirements.reverse();
    let permuted = reconcile_requirements(&left, &right, &[]);

    assert_eq!(expected, permuted);
    assert_eq!(expected.outcomes.len(), 4);
    assert!(
        expected
            .outcomes
            .iter()
            .all(|result| result.status == ReconciliationStatus::Ambiguous)
    );
    assert!(
        expected
            .outcomes
            .iter()
            .all(|result| result.relationship_candidate().is_none())
    );
}

#[test]
fn published_snapshots_reconcile_without_any_local_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path =
        camino::Utf8PathBuf::from_path_buf(directory.path().join("catalogs.sqlite"))
            .map_err(|path| format!("non-UTF-8 path: {}", path.display()))?;
    let database = Database::open(&database_path)?;
    let document = |name: &str, set: &str, sha1: &str| {
        format!(
            "<datafile><header><name>{name}</name></header><game name=\"{set}\"><rom name=\"renamed.bin\" size=\"32\" crc=\"12345678\" sha1=\"{sha1}\"/></game></datafile>"
        )
    };
    let utf8_path = |path| {
        camino::Utf8PathBuf::from_path_buf(path)
            .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))
    };
    let left_path = utf8_path(directory.path().join("left.dat"))?;
    let right_path = utf8_path(directory.path().join("right.dat"))?;
    std::fs::write(
        &left_path,
        document(
            "left publisher",
            "left-release",
            "1111111111111111111111111111111111111111",
        ),
    )?;
    std::fs::write(
        &right_path,
        document(
            "right publisher",
            "right-release",
            "1111111111111111111111111111111111111111",
        ),
    )?;
    let import = |path, publisher: &str, catalog: &str| CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::Logiqx,
        source_key: PublishingSourceKey::new(publisher),
        source_display_name: publisher.to_owned(),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: catalog.to_owned(),
        scope: CatalogScope::Complete,
    };
    let left = app::import_catalog(
        &database,
        &import(left_path, "left-publisher", "left-catalog"),
    )?;
    let right = app::import_catalog(
        &database,
        &import(right_path, "right-publisher", "right-catalog"),
    )?;
    let report = app::reconcile_catalog_snapshots(
        &database,
        left.snapshot_key
            .as_ref()
            .ok_or("left import produced no snapshot")?,
        right
            .snapshot_key
            .as_ref()
            .ok_or("right import produced no snapshot")?,
    )?;

    assert_eq!(report.outcomes.len(), 1);
    let outcome = &report.outcomes[0];
    assert_eq!(outcome.status, ReconciliationStatus::Compatible);
    let candidate = outcome
        .relationship_candidate()
        .ok_or("candidate missing")?;
    assert!(matches!(
        candidate.subject,
        RelationshipEndpoint::CatalogRecord(_)
    ));
    assert!(matches!(
        candidate.target,
        RelationshipEndpoint::CatalogRecord(_)
    ));
    assert!(matches!(
        candidate.origin,
        RelationshipOrigin::DerivedCandidate { .. }
    ));
    let recorded = app::record_relationship(&database, &candidate)?;
    assert!(
        app::explain_relationships(&database)?
            .iter()
            .any(|item| item.assertion_key == recorded)
    );
    Ok(())
}

#[test]
fn software_list_requirements_keep_item_relationships_as_context()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path =
        camino::Utf8PathBuf::from_path_buf(directory.path().join("software.sqlite"))
            .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))?;
    let database = Database::open(&database_path)?;
    let document = camino::Utf8PathBuf::from(format!(
        "{}/fixtures/catalog/mame/software-list.xml",
        env!("CARGO_MANIFEST_DIR")
    ));
    let import = |catalog: &str| CatalogImportRequest {
        document_path: document.clone(),
        format: CatalogDocumentFormat::MameSoftwareListXml,
        source_key: PublishingSourceKey::new(format!("{catalog}-publisher")),
        source_display_name: format!("Publisher {catalog}"),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: format!("Catalog {catalog}"),
        scope: CatalogScope::Complete,
    };
    let left = app::import_catalog(&database, &import("software-left"))?;
    let right = app::import_catalog(&database, &import("software-right"))?;
    let report = app::reconcile_catalog_snapshots(
        &database,
        left.snapshot_key
            .as_ref()
            .ok_or("left software import produced no snapshot")?,
        right
            .snapshot_key
            .as_ref()
            .ok_or("right software import produced no snapshot")?,
    )?;

    let explained_match = report.outcomes.iter().find(|outcome| {
        outcome
            .left_expected
            .as_ref()
            .is_some_and(|expected| expected.sha1.is_some())
            && outcome.relationship_evidence.iter().any(|explanation| {
                explanation.claim.relation_type == RelationshipType::SourceParentClone
            })
    });
    let outcome = explained_match.ok_or("software component match lost its parent assertion")?;
    let candidate = outcome
        .relationship_candidate()
        .ok_or("software component identity candidate missing")?;
    assert!(matches!(
        candidate.origin,
        RelationshipOrigin::DerivedCandidate {
            supporting_assertions,
            ..
        } if supporting_assertions.is_empty()
    ));
    Ok(())
}

#[test]
fn merge_assertion_targets_the_unique_asset_requirement_record()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path =
        camino::Utf8PathBuf::from_path_buf(directory.path().join("merge.sqlite"))
            .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))?;
    let database = Database::open(&database_path)?;
    let document = camino::Utf8PathBuf::from_path_buf(directory.path().join("machines.xml"))
        .map_err(|path| std::io::Error::other(format!("non-UTF-8 path: {}", path.display())))?;
    std::fs::write(
        &document,
        r#"<mame><machine name="parent"><disk name="parent_disk" sha1="1123456789abcdef0123456789abcdef01234567" /></machine><machine name="clone" cloneof="parent"><disk name="clone_disk" sha1="1123456789abcdef0123456789abcdef01234567" merge="parent_disk" /></machine></mame>"#,
    )?;
    let import = |catalog: &str| CatalogImportRequest {
        document_path: document.clone(),
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new(format!("{catalog}-publisher")),
        source_display_name: format!("Publisher {catalog}"),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: format!("Catalog {catalog}"),
        scope: CatalogScope::Complete,
    };
    let left = app::import_catalog(&database, &import("machine-left"))?;
    let right = app::import_catalog(&database, &import("machine-right"))?;
    let report = app::reconcile_catalog_snapshots(
        &database,
        left.snapshot_key
            .as_ref()
            .ok_or("left machine import produced no snapshot")?,
        right
            .snapshot_key
            .as_ref()
            .ok_or("right machine import produced no snapshot")?,
    )?;
    let parent_match = report.outcomes.iter().find(|outcome| {
        outcome
            .left
            .as_ref()
            .is_some_and(|record| record.key.as_str() == "parent/parent_disk#0")
    });
    let outcome = parent_match.ok_or("parent asset requirement pair missing")?;
    assert!(outcome.relationship_evidence.iter().any(|explanation| {
        explanation.claim.relation_type == RelationshipType::ExactContentIdentity
            && explanation.source_field.as_deref() == Some("merge")
    }));
    Ok(())
}
