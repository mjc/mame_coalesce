use std::{
    io,
    path::{Path, PathBuf},
};

use camino::Utf8PathBuf;
use diesel::{Connection, RunQueryDsl, sql_query, sql_types::BigInt};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportReport, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, ContentDigestAlgorithm, ContentIdentity, ExternalRecordRef,
        PublishingSourceKey, RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint,
        RelationshipEvidence, RelationshipOrigin, RelationshipReview, RelationshipReviewDecision,
        RelationshipType,
    },
};

#[derive(diesel::QueryableByName)]
struct RelationshipSchemaColumns {
    #[diesel(sql_type = BigInt)]
    found: i64,
}

fn request(path: Utf8PathBuf, source: &str, catalog: &str) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::Logiqx,
        source_key: PublishingSourceKey::new(source),
        source_display_name: format!("Publisher {source}"),
        catalog_key: CatalogKey::new(catalog),
        catalog_display_name: format!("Catalog {catalog}"),
        scope: CatalogScope::Unknown,
    }
}

fn utf8(path: PathBuf) -> Result<Utf8PathBuf, io::Error> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("non-UTF-8 path: {}", path.display()),
        )
    })
}

fn fixture(name: &str) -> Result<Utf8PathBuf, io::Error> {
    utf8(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/catalog")
            .join(name),
    )
}

fn import_conflicting_sources(
    database: &Database,
    directory: &std::path::Path,
) -> Result<(CatalogImportReport, CatalogImportReport), Box<dyn std::error::Error>> {
    let first = app::import_catalog(
        database,
        &request(
            fixture("logiqx/catalog-a-v1.dat")?,
            "publisher-one",
            "catalog-one",
        ),
    )?;
    let second_path = utf8(directory.join("conflicting.dat"))?;
    let original = std::fs::read_to_string(fixture("logiqx/catalog-a-v1.dat")?)?;
    std::fs::write(
        &second_path,
        original.replace("cloneof=\"parent\"", "cloneof=\"different-parent\""),
    )?;
    let second = app::import_catalog(
        database,
        &request(second_path, "publisher-two", "catalog-two"),
    )?;
    Ok((first, second))
}

fn record_candidate(
    database: &Database,
    supporting_assertion: RelationshipAssertionKey,
) -> Result<RelationshipAssertionKey, Box<dyn std::error::Error>> {
    let digest = ContentIdentity::new(
        ContentDigestAlgorithm::Sha1,
        "0123456789abcdef0123456789abcdef01234567",
    )?;
    Ok(app::record_relationship(
        database,
        &RelationshipClaim {
            relation_type: RelationshipType::ExactContentIdentity,
            subject: RelationshipEndpoint::ContentObject(digest.clone()),
            target: RelationshipEndpoint::ContentObject(digest),
            origin: RelationshipOrigin::DerivedCandidate {
                rule_version: "sha1-equality-v1".to_owned(),
                supporting_assertions: vec![supporting_assertion],
            },
            evidence: RelationshipEvidence::Rationale {
                reason: "SHA-1 digests are equal".to_owned(),
            },
        },
    )?)
}

fn review_candidate(
    database: &Database,
    candidate: &RelationshipAssertionKey,
) -> Result<(), Box<dyn std::error::Error>> {
    for (decision, note) in [
        (RelationshipReviewDecision::Accepted, "reviewed evidence"),
        (
            RelationshipReviewDecision::Withdrawn,
            "later evidence conflicts",
        ),
    ] {
        app::review_relationship(
            database,
            candidate,
            &RelationshipReview {
                decision,
                note: note.to_owned(),
                superseded_by: None,
            },
        )?;
    }
    Ok(())
}

#[test]
fn explanations_preserve_conflicts_candidates_and_reversible_reviews()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::open(&utf8(directory.path().join("relationships.sqlite"))?)?;
    let (first, second) = import_conflicting_sources(&database, directory.path())?;

    let before = app::explain_relationships(&database)?;
    let source_claims = before
        .iter()
        .filter(|item| item.claim.relation_type == RelationshipType::SourceParentClone)
        .collect::<Vec<_>>();
    assert_eq!(source_claims.len(), 2);
    assert_ne!(source_claims[0].claim.target, source_claims[1].claim.target);
    assert!(
        source_claims
            .iter()
            .all(|item| item.latest_review.is_none())
    );
    assert!(source_claims.iter().all(|item| item.source.is_some()));
    assert_eq!(before, app::explain_relationships(&database)?);
    assert!(app::record_relationship(&database, &source_claims[0].claim).is_err());

    let candidate = record_candidate(&database, source_claims[0].assertion_key.clone())?;

    let candidate_explanation = app::explain_relationships(&database)?
        .into_iter()
        .find(|item| item.assertion_key == candidate)
        .ok_or_else(|| io::Error::other("recorded candidate is not explainable"))?;
    assert!(candidate_explanation.latest_review.is_none());
    assert!(matches!(
        &candidate_explanation.claim.origin,
        RelationshipOrigin::DerivedCandidate {
            rule_version,
            supporting_assertions
        } if rule_version == "sha1-equality-v1" && supporting_assertions.len() == 1
    ));
    assert_eq!(
        candidate_explanation.claim.evidence,
        RelationshipEvidence::Rationale {
            reason: "SHA-1 digests are equal".to_owned(),
        }
    );

    review_candidate(&database, &candidate)?;
    app::review_relationship(
        &database,
        &source_claims[0].assertion_key,
        &RelationshipReview {
            decision: RelationshipReviewDecision::Superseded,
            note: "the second source snapshot supersedes this claim".to_owned(),
            superseded_by: Some(source_claims[1].assertion_key.clone()),
        },
    )?;

    app::import_catalog(
        &database,
        &request(
            fixture("logiqx/catalog-a-v1.dat")?,
            "publisher-one",
            "catalog-one",
        ),
    )?;
    let after = app::explain_relationships(&database)?;
    let reviewed = after
        .iter()
        .find(|item| item.assertion_key == candidate)
        .ok_or_else(|| io::Error::other("reviewed candidate is not explainable"))?;
    assert_eq!(
        reviewed
            .latest_review
            .as_ref()
            .map(|review| review.decision),
        Some(RelationshipReviewDecision::Withdrawn)
    );
    assert_eq!(reviewed.review_history.len(), 2);
    let reviewed_source = after
        .iter()
        .find(|item| item.assertion_key == source_claims[0].assertion_key)
        .ok_or_else(|| io::Error::other("reviewed source claim is not explainable"))?;
    assert_eq!(
        reviewed_source
            .latest_review
            .as_ref()
            .and_then(|review| review.superseded_by.as_ref()),
        Some(&source_claims[1].assertion_key)
    );
    assert_eq!(
        after
            .iter()
            .filter(|item| item.claim.relation_type == RelationshipType::SourceParentClone)
            .count(),
        2,
        "reviewing a candidate must not overwrite contradictory source claims"
    );
    assert_ne!(first.snapshot_key, second.snapshot_key);
    Ok(())
}

#[test]
fn generic_endpoint_components_round_trip_without_encoded_keys()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = utf8(directory.path().join("typed-endpoints.sqlite"))?;
    let database = Database::open(&database_path)?;
    let claim = RelationshipClaim {
        relation_type: RelationshipType::CatalogCorrection,
        subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "registry",
            "[\"part-a\",\"part-b\"]",
        )),
        target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "registry",
            "{\"not\":\"a persisted composite key\"}",
        )),
        origin: RelationshipOrigin::UserConclusion,
        evidence: RelationshipEvidence::Rationale {
            reason: "manual reconciliation".to_owned(),
        },
    };
    let key = app::record_relationship(&database, &claim)?;
    let explanation = app::explain_relationships(&database)?
        .into_iter()
        .find(|item| item.assertion_key == key)
        .ok_or_else(|| io::Error::other("typed endpoint claim is not explainable"))?;
    assert_eq!(explanation.claim, claim);

    let mut connection = diesel::SqliteConnection::establish(database_path.as_str())?;
    let legacy_columns = sql_query(
        "SELECT COUNT(*) AS found FROM ( \
             SELECT name FROM pragma_table_info('relationship_assertions') \
             UNION ALL SELECT name FROM pragma_table_info('relationship_rationales') \
             UNION ALL SELECT name FROM pragma_table_info('relationship_comparisons') \
             UNION ALL SELECT name FROM pragma_table_info('relationship_comparison_fields') \
             UNION ALL SELECT name FROM pragma_table_info('relationship_evidence_publications') \
             UNION ALL SELECT name FROM pragma_table_info('relationship_assertion_support') \
         ) WHERE lower(name) LIKE '%json%' \
             OR name IN ('subject_key', 'target_key', 'generic_subject_key', 'generic_target_key')",
    )
    .get_result::<RelationshipSchemaColumns>(&mut connection)?;
    assert_eq!(legacy_columns.found, 0);
    Ok(())
}

#[test]
fn content_identity_deserialization_enforces_digest_shape() {
    let invalid = r#"{"algorithm":"sha1","digest":"not-a-sha1"}"#;
    assert!(serde_json::from_str::<ContentIdentity>(invalid).is_err());
    assert!(ContentIdentity::new(ContentDigestAlgorithm::Sha1, "a".repeat(40)).is_ok());
}

#[test]
fn parent_claims_keep_their_adapter_specific_source_provenance()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database = Database::open(&utf8(
        directory.path().join("adapter-relationships.sqlite"),
    )?)?;

    let mut clrmamepro = request(
        fixture("clrmamepro/sample.dat")?,
        "clrmamepro-source",
        "clrmamepro-catalog",
    );
    clrmamepro.format = CatalogDocumentFormat::ClrMamePro;
    app::import_catalog(&database, &clrmamepro)?;

    let mut software_list = request(
        fixture("mame/software-list.xml")?,
        "software-list-source",
        "software-list-catalog",
    );
    software_list.format = CatalogDocumentFormat::MameSoftwareListXml;
    app::import_catalog(&database, &software_list)?;

    let machine_path = utf8(directory.path().join("machine.xml"))?;
    std::fs::write(
        &machine_path,
        br#"<mame><machine name="mame_clone" cloneof="mame_parent"><description>MAME clone</description></machine></mame>"#,
    )?;
    let mut machine = request(machine_path, "machine-source", "machine-catalog");
    machine.format = CatalogDocumentFormat::MameListXml;
    app::import_catalog(&database, &machine)?;

    let claims = app::explain_relationships(&database)?
        .into_iter()
        .filter(|claim| claim.claim.relation_type == RelationshipType::SourceParentClone)
        .collect::<Vec<_>>();
    assert_eq!(claims.len(), 3);
    assert!(claims.iter().all(|claim| claim.source.is_some()));
    assert!(claims.iter().all(|claim| claim.source_location.is_some()));
    assert_eq!(
        claims
            .iter()
            .map(|claim| claim.source_field.as_deref().unwrap_or_default())
            .collect::<std::collections::BTreeSet<_>>(),
        std::iter::once("cloneof").collect()
    );
    assert!(claims.iter().any(|claim| {
        matches!(
            &claim.claim.subject,
            RelationshipEndpoint::CatalogRecord(record)
                if record.key.as_str() == "clone_set"
        )
    }));
    assert!(claims.iter().any(|claim| {
        matches!(
            &claim.claim.target,
            RelationshipEndpoint::CatalogRecord(record)
                if record.key.as_str() == r#"["demo_cart","demo_original"]"#
        )
    }));
    assert!(claims.iter().any(|claim| {
        matches!(
            &claim.claim.subject,
            RelationshipEndpoint::CatalogRecord(record)
                if record.key.as_str() == r#"["demo_cart","demo_game"]"#
        )
    }));
    assert!(claims.iter().any(|claim| {
        matches!(
            &claim.claim.subject,
            RelationshipEndpoint::CatalogRecord(record)
                if record.key.as_str() == "mame_clone"
        )
    }));
    Ok(())
}
