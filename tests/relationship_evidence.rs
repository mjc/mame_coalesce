use camino::Utf8PathBuf;
use diesel::{Connection, RunQueryDsl, SqliteConnection, connection::SimpleConnection, sql_query};
use mame_coalesce::{
    app,
    database::Database,
    domain::{
        ExternalRecordRef, RelationshipClaim, RelationshipEndpoint, RelationshipEvidence,
        RelationshipOrigin, RelationshipReview, RelationshipReviewDecision, RelationshipType,
    },
    reconciliation::ReconciliationStatus,
    resolution::EvidenceField,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Catalog {
    _directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
    connection: SqliteConnection,
}

fn catalog() -> TestResult<Catalog> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&path)?;
    let connection = SqliteConnection::establish(path.as_str())?;
    Ok(Catalog {
        _directory: directory,
        path,
        database,
        connection,
    })
}

fn claim(evidence: RelationshipEvidence, origin: RelationshipOrigin) -> RelationshipClaim {
    RelationshipClaim {
        relation_type: RelationshipType::CatalogContinuity,
        subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("publisher", "left")),
        target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("publisher", "right")),
        origin,
        evidence,
    }
}

#[allow(clippy::expect_used)]
fn derived() -> RelationshipOrigin {
    RelationshipOrigin::DerivedCandidate {
        rule: mame_coalesce::domain::RelationshipRule::new(
            "fixture",
            "v1",
            "Fixture relationship rule",
        )
        .expect("valid declared fixture rule"),
        supporting_assertions: Vec::new(),
    }
}

fn rationale(reason: &str) -> RelationshipEvidence {
    RelationshipEvidence::Rationale {
        reason: reason.into(),
    }
}

fn draft(conn: &mut SqliteConnection, key: &str) -> TestResult {
    for side in ["left", "right"] {
        sql_query("INSERT INTO catalog_relationship_targets(kind) VALUES('external_record')")
            .execute(conn)?;
        sql_query("INSERT INTO external_catalog_targets(target_id,namespace,declared_key) VALUES(last_insert_rowid(),'draft-fixture',?)")
            .bind::<diesel::sql_types::Text,_>(format!("{key}/{side}")).execute(conn)?;
    }
    conn.batch_execute("INSERT INTO catalog_relationship_rules(rule_key,revision,description) SELECT 'fixture','v1','Fixture relationship rule' WHERE NOT EXISTS(SELECT 1 FROM catalog_relationship_rules WHERE rule_key='fixture' AND revision='v1')")?;
    sql_query("INSERT INTO catalog_relationships(assertion_key,origin) VALUES(?,'derived')")
        .bind::<diesel::sql_types::Text, _>(key)
        .execute(conn)?;
    sql_query("INSERT INTO inferred_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id,rule_id) SELECT identity.relationship_id,'catalog_continuity',source.target_id,target.target_id,rule.rule_id FROM catalog_relationships AS identity JOIN external_catalog_targets AS source ON source.namespace='draft-fixture' AND source.declared_key=? JOIN external_catalog_targets AS target ON target.namespace='draft-fixture' AND target.declared_key=? JOIN catalog_relationship_rules AS rule ON rule.rule_key='fixture' AND rule.revision='v1' WHERE identity.assertion_key=?")
        .bind::<diesel::sql_types::Text,_>(format!("{key}/left"))
        .bind::<diesel::sql_types::Text,_>(format!("{key}/right"))
        .bind::<diesel::sql_types::Text,_>(key).execute(conn)?;
    Ok(())
}

#[test]
fn closed_evidence_types_reject_arbitrary_payloads_and_wrong_origins() -> TestResult {
    let catalog = catalog()?;
    let original = claim(
        rationale("é😀: reviewed publisher correspondence"),
        RelationshipOrigin::UserConclusion,
    );
    let key = app::record_relationship(&catalog.database, &original)?;
    let explanation = app::explain_relationships(&catalog.database)?
        .into_iter()
        .find(|row| row.assertion_key == key)
        .ok_or("missing published rationale")?;
    assert_eq!(explanation.claim, original);
    assert!(
        serde_json::from_str::<RelationshipEvidence>(
            r#"{"kind":"rationale","reason":"ok","details":{"anything":[1,null]}}"#
        )
        .is_err()
    );
    assert!(
        serde_json::from_str::<RelationshipEvidence>(r#"{"kind":"object","value":{"anything":1}}"#)
            .is_err()
    );
    for evidence in [
        rationale(" "),
        RelationshipEvidence::SourceReference {
            target_name: "parent".into(),
        },
        RelationshipEvidence::CatalogComparison {
            status: ReconciliationStatus::Compatible,
            agreements: vec![EvidenceField::Sha1],
            contradictions: Vec::new(),
        },
    ] {
        assert!(
            app::record_relationship(
                &catalog.database,
                &claim(evidence, RelationshipOrigin::UserConclusion)
            )
            .is_err()
        );
    }
    assert_eq!(app::explain_relationships(&catalog.database)?.len(), 1);
    Ok(())
}

#[test]
fn typed_comparison_decisions_preserve_assessment_order_without_expected_value_copies() -> TestResult
{
    let mut catalog = catalog()?;
    for status in [
        ReconciliationStatus::Compatible,
        ReconciliationStatus::Candidate,
        ReconciliationStatus::Contradictory,
        ReconciliationStatus::Ambiguous,
        ReconciliationStatus::Unknown,
    ] {
        let original = claim(
            RelationshipEvidence::CatalogComparison {
                status,
                agreements: vec![EvidenceField::Crc, EvidenceField::Sha1, EvidenceField::Md5],
                contradictions: vec![EvidenceField::Size],
            },
            derived(),
        );
        let key = app::record_relationship(&catalog.database, &original)?;
        let actual = app::explain_relationships(&catalog.database)?
            .into_iter()
            .find(|row| row.assertion_key == key)
            .ok_or("missing typed comparison")?;
        assert_eq!(actual.claim, original);
    }
    for agreements in [
        vec![EvidenceField::Size],
        vec![EvidenceField::Sha1, EvidenceField::Sha1],
    ] {
        assert!(
            app::record_relationship(
                &catalog.database,
                &claim(
                    RelationshipEvidence::CatalogComparison {
                        status: ReconciliationStatus::Compatible,
                        agreements,
                        contradictions: vec![EvidenceField::Size],
                    },
                    derived()
                )
            )
            .is_err()
        );
    }
    assert_eq!(
        app::explain_relationships(&catalog.database)?.len(),
        5,
        "invalid comparisons must roll back"
    );
    let names = sql_query(
        "SELECT name AS value FROM pragma_table_info('relationship_comparisons') ORDER BY cid",
    )
    .load::<TextValue>(&mut catalog.connection)?
    .into_iter()
    .map(|row| row.value)
    .collect::<Vec<_>>();
    assert_eq!(names, ["assertion_key", "status"]);
    assert_eq!(
        sql_query(
            "SELECT count(*) AS value FROM relationship_assertions WHERE origin='derived_candidate'"
        )
        .get_result::<Count>(&mut catalog.connection)?
        .value,
        5
    );
    Ok(())
}

#[derive(diesel::QueryableByName)]
struct TextValue {
    #[diesel(sql_type = diesel::sql_types::Text)]
    value: String,
}

#[derive(diesel::QueryableByName)]
struct Count {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    value: i64,
}

#[test]
fn incomplete_relationships_cannot_be_explained_reviewed_supported_or_backed_up() -> TestResult {
    let mut catalog = catalog()?;
    draft(&mut catalog.connection, "draft")?;
    assert!(app::explain_relationships(&catalog.database)?.is_empty());
    let review = RelationshipReview {
        decision: RelationshipReviewDecision::Accepted,
        note: "review".into(),
        superseded_by: None,
    };
    assert!(
        app::review_relationship(
            &catalog.database,
            &mame_coalesce::domain::RelationshipAssertionKey::new("draft"),
            &review
        )
        .is_err()
    );
    let supporting = RelationshipOrigin::DerivedCandidate {
        rule: mame_coalesce::domain::RelationshipRule::new(
            "fixture",
            "v1",
            "Fixture relationship rule",
        )?,
        supporting_assertions: vec![mame_coalesce::domain::RelationshipAssertionKey::new(
            "draft",
        )],
    };
    assert!(
        app::record_relationship(
            &catalog.database,
            &claim(rationale("supported"), supporting)
        )
        .is_err()
    );
    drop(catalog.database);
    let integrity = mame_coalesce::check_integrity(&catalog.path)?;
    assert!(
        integrity
            .durable_issues
            .iter()
            .any(|issue| issue.contains("draft") && issue.contains("publication"))
    );
    let backup_path = catalog.path.with_file_name("incomplete.backup");
    assert!(mame_coalesce::create_backup(&catalog.path, &backup_path).is_err());
    catalog.connection.batch_execute("INSERT INTO relationship_rationales VALUES('draft','complete rationale'); INSERT INTO relationship_evidence_publications VALUES('draft','rationale')")?;
    let database = Database::open(&catalog.path)?;
    assert_eq!(app::explain_relationships(&database)?.len(), 1);
    drop(database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn installed_guards_reject_orphans_replacements_late_children_and_bad_seals_without_sqlite_enforcement()
-> TestResult {
    let mut catalog = catalog()?;
    let rationale_key =
        app::record_relationship(&catalog.database, &claim(rationale("immutable"), derived()))?;
    let comparison_key = app::record_relationship(
        &catalog.database,
        &claim(
            RelationshipEvidence::CatalogComparison {
                status: ReconciliationStatus::Compatible,
                agreements: vec![EvidenceField::Sha1],
                contradictions: Vec::new(),
            },
            derived(),
        ),
    )?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let r = rationale_key.as_str();
    let c = comparison_key.as_str();
    let attempted = [
        "INSERT INTO relationship_rationales VALUES('missing','orphan')".to_owned(),
        "INSERT INTO relationship_comparisons VALUES('missing','compatible')".to_owned(),
        "INSERT INTO relationship_comparison_fields VALUES('missing','agreement',0,'sha1')"
            .to_owned(),
        "INSERT INTO relationship_evidence_publications VALUES('missing','rationale')".to_owned(),
        format!("INSERT OR REPLACE INTO relationship_rationales VALUES('{r}','replacement')"),
        format!("INSERT OR REPLACE INTO relationship_comparisons VALUES('{c}','unknown')"),
        format!(
            "INSERT OR REPLACE INTO relationship_comparison_fields VALUES('{c}','agreement',0,'md5')"
        ),
        format!(
            "INSERT OR REPLACE INTO relationship_evidence_publications VALUES('{r}','rationale')"
        ),
        format!("INSERT INTO relationship_comparisons VALUES('{r}','unknown')"),
        format!("INSERT INTO relationship_rationales VALUES('{c}','dual subtype')"),
        format!(
            "INSERT INTO relationship_comparison_fields VALUES('{c}','contradiction',0,'size')"
        ),
        format!("INSERT INTO relationship_assertion_support VALUES('{r}',0,'{c}')"),
        format!("UPDATE relationship_rationales SET reason='changed' WHERE assertion_key='{r}'"),
        format!("DELETE FROM relationship_comparisons WHERE assertion_key='{c}'"),
        format!("DELETE FROM relationship_comparison_fields WHERE assertion_key='{c}'"),
        format!("DELETE FROM relationship_evidence_publications WHERE assertion_key='{r}'"),
    ];
    for sql in attempted {
        assert!(catalog.connection.batch_execute(&sql).is_err(), "{sql}");
    }
    draft(&mut catalog.connection, "replacement-control")?;
    let replacement = format!(
        "INSERT OR REPLACE INTO inferred_catalog_relationships SELECT native.* FROM inferred_catalog_relationships AS native JOIN catalog_relationships AS identity USING(relationship_id) WHERE identity.assertion_key='{r}'"
    );
    let error = catalog
        .connection
        .batch_execute(&replacement)
        .err()
        .ok_or("valid replacement must hit the immutable trigger")?;
    assert!(
        error
            .to_string()
            .contains("inferred relationship requires an unused identity"),
        "wrong rejection: {error}"
    );
    draft(&mut catalog.connection, "fractional-support")?;
    assert!(
        catalog
            .connection
            .batch_execute(&format!(
                "INSERT INTO relationship_assertion_support VALUES('fractional-support',0.5,'{r}')"
            ))
            .is_err(),
        "support positions must be integers"
    );
    draft(&mut catalog.connection, "gap")?;
    catalog.connection.batch_execute("INSERT INTO relationship_comparisons VALUES('gap','candidate'); INSERT INTO relationship_comparison_fields VALUES('gap','agreement',1,'sha1')")?;
    for sql in [
        "INSERT INTO relationship_evidence_publications VALUES('gap','catalog_comparison')",
        "INSERT INTO relationship_evidence_publications VALUES('gap','rationale')",
        "INSERT INTO relationship_rationales VALUES('gap','dual subtype')",
        "INSERT OR REPLACE INTO relationship_comparison_fields VALUES('gap','agreement',1,'md5')",
        "INSERT INTO relationship_comparison_fields VALUES('gap','contradiction',0,'sha1')",
        "INSERT INTO relationship_comparison_fields VALUES('gap','agreement',0.5,'crc')",
        "INSERT INTO relationship_comparison_fields VALUES('gap','agreement',-1,'size')",
        "INSERT INTO relationship_comparison_fields VALUES('gap','agreement',0,'unknown')",
    ] {
        assert!(catalog.connection.batch_execute(sql).is_err(), "{sql}");
    }
    assert_eq!(app::explain_relationships(&catalog.database)?.len(), 2);
    Ok(())
}

#[test]
fn paired_backup_preserves_typed_evidence_support_and_supersession() -> TestResult {
    let catalog = catalog()?;
    let first = app::record_relationship(
        &catalog.database,
        &claim(rationale("publisher correction"), derived()),
    )?;
    let second = app::record_relationship(
        &catalog.database,
        &claim(
            RelationshipEvidence::CatalogComparison {
                status: ReconciliationStatus::Candidate,
                agreements: vec![EvidenceField::Crc, EvidenceField::Size],
                contradictions: Vec::new(),
            },
            RelationshipOrigin::DerivedCandidate {
                rule: mame_coalesce::domain::RelationshipRule::new(
                    "fixture",
                    "v2",
                    "Fixture relationship rule",
                )?,
                supporting_assertions: vec![first.clone()],
            },
        ),
    )?;
    app::review_relationship(
        &catalog.database,
        &first,
        &RelationshipReview {
            decision: RelationshipReviewDecision::Superseded,
            note: "more specific comparison".into(),
            superseded_by: Some(second),
        },
    )?;
    let expected = app::explain_relationships(&catalog.database)?;
    drop(catalog.database);
    let backup = catalog.path.with_file_name("catalog.backup");
    let restored = catalog.path.with_file_name("restored.sqlite");
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, mame_coalesce::RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    assert_eq!(app::explain_relationships(&restored_database)?, expected);
    drop(restored_database);
    assert!(mame_coalesce::check_integrity(&restored)?.is_clean());
    Ok(())
}

#[test]
fn source_evidence_stays_with_native_owners_and_cannot_acquire_decision_copies() -> TestResult {
    let mut catalog = catalog()?;
    let document = catalog.path.with_file_name("source.xml");
    std::fs::write(
        &document,
        "<datafile><game name='child' cloneof='parent'><rom name='file.bin' size='1'/></game><game name='parent'/></datafile>",
    )?;
    let report = app::import_catalog(
        &catalog.database,
        &app::CatalogImportRequest {
            document_path: document,
            format: app::CatalogDocumentFormat::Logiqx,
            source_key: mame_coalesce::domain::PublishingSourceKey::new("fixture"),
            source_display_name: "Fixture".into(),
            catalog_key: mame_coalesce::domain::CatalogKey::new("fixture"),
            catalog_display_name: "Fixture".into(),
            scope: mame_coalesce::domain::CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let explanations = app::explain_relationships(&catalog.database)?;
    assert_eq!(explanations.len(), 1);
    assert_eq!(
        explanations[0].claim.evidence,
        RelationshipEvidence::SourceReference {
            target_name: "parent".into()
        }
    );
    let key = explanations[0].assertion_key.as_str();
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    for sql in [
        format!("INSERT INTO relationship_rationales VALUES('{key}','copied source')"),
        format!("INSERT INTO relationship_comparisons VALUES('{key}','compatible')"),
        format!("INSERT INTO relationship_evidence_publications VALUES('{key}','rationale')"),
    ] {
        assert!(catalog.connection.batch_execute(&sql).is_err(), "{sql}");
    }
    for table in [
        "relationship_rationales",
        "relationship_comparisons",
        "relationship_comparison_fields",
        "relationship_evidence_publications",
    ] {
        assert_eq!(
            sql_query(format!("SELECT count(*) AS value FROM {table}"))
                .get_result::<Count>(&mut catalog.connection)?
                .value,
            0,
            "{table}"
        );
    }
    assert_eq!(app::explain_relationships(&catalog.database)?, explanations);
    Ok(())
}
