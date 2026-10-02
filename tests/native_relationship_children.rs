use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app,
    database::Database,
    domain::{
        ExternalRecordRef, RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint,
        RelationshipEvidence, RelationshipOrigin, RelationshipReview, RelationshipReviewDecision,
        RelationshipRule, RelationshipType,
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

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        Ok(Self {
            database: Database::open(&path)?,
            connection: SqliteConnection::establish(path.as_str())?,
            _directory: directory,
            path,
        })
    }

    fn claim(&self, name: &str) -> TestResult<RelationshipAssertionKey> {
        Ok(app::record_relationship(
            &self.database,
            &RelationshipClaim {
                relation_type: RelationshipType::CatalogContinuity,
                subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                    "publisher",
                    name,
                )),
                target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                    "publisher",
                    "destination",
                )),
                origin: RelationshipOrigin::UserConclusion,
                evidence: RelationshipEvidence::Rationale {
                    reason: format!("Reviewed {name}"),
                },
            },
        )?)
    }

    fn review(
        &self,
        predecessor: &RelationshipAssertionKey,
        replacement: Option<&RelationshipAssertionKey>,
        decision: RelationshipReviewDecision,
    ) -> TestResult {
        Ok(app::review_relationship(
            &self.database,
            predecessor,
            &RelationshipReview {
                decision,
                note: format!("Reviewed {decision:?}"),
                superseded_by: replacement.cloned(),
            },
        )?)
    }

    fn id(&mut self, key: &RelationshipAssertionKey) -> TestResult<i64> {
        Ok(sql_query(
            "SELECT relationship_id AS value FROM catalog_relationships WHERE assertion_key=?",
        )
        .bind::<Text, _>(key.as_str())
        .get_result::<Integer>(&mut self.connection)?
        .value)
    }

    fn draft_review(&mut self, key: &str, owner: i64, decision: &str) -> TestResult<i64> {
        Ok(sql_query("INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES(?,?,?,'raw review fixture') RETURNING review_id AS value")
            .bind::<Text,_>(key).bind::<BigInt,_>(owner).bind::<Text,_>(decision)
            .get_result::<Integer>(&mut self.connection)?.value)
    }

    fn replacement(&mut self, review: i64, successor: i64) -> diesel::QueryResult<usize> {
        sql_query("INSERT INTO replaced_catalog_relationships VALUES(?,?)")
            .bind::<BigInt, _>(review)
            .bind::<BigInt, _>(successor)
            .execute(&mut self.connection)
    }

    fn seal(&mut self, review: i64) -> diesel::QueryResult<usize> {
        sql_query("INSERT INTO catalog_relationship_review_publications VALUES(?)")
            .bind::<BigInt, _>(review)
            .execute(&mut self.connection)
    }
}

#[derive(QueryableByName)]
struct Integer {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct Column {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    declared_type: String,
}

fn assert_integer_owner_schema(connection: &mut SqliteConnection) -> TestResult {
    for (table, expected) in [
        (
            "catalog_relationship_rationales",
            vec!["relationship_id", "reason"],
        ),
        (
            "catalog_relationship_comparisons",
            vec!["relationship_id", "status"],
        ),
        (
            "catalog_relationship_comparison_fields",
            vec!["relationship_id", "disposition", "list_order", "field"],
        ),
        (
            "catalog_relationship_evidence_publications",
            vec!["relationship_id", "evidence_kind"],
        ),
        (
            "catalog_relationship_evidence",
            vec![
                "relationship_id",
                "list_order",
                "supporting_relationship_id",
            ],
        ),
        (
            "catalog_relationship_reviews",
            vec![
                "review_id",
                "review_key",
                "relationship_id",
                "decision",
                "note",
                "reviewed_at",
            ],
        ),
        (
            "replaced_catalog_relationships",
            vec!["review_id", "replacement_relationship_id"],
        ),
        (
            "catalog_relationship_review_publications",
            vec!["review_id"],
        ),
    ] {
        let columns =
            sql_query("SELECT name,type AS declared_type FROM pragma_table_info(?) ORDER BY cid")
                .bind::<Text, _>(table)
                .load::<Column>(connection)?;
        assert_eq!(
            columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{table}"
        );
        for column in columns.iter().filter(|column| column.name.ends_with("_id")) {
            assert_eq!(column.declared_type, "INTEGER", "{table}.{}", column.name);
        }
    }
    Ok(())
}

#[test]
fn relationship_children_store_integer_owners_without_repeated_external_keys() -> TestResult {
    let mut catalog = Catalog::new()?;
    let predecessor = catalog.claim("predecessor")?;
    let candidate = RelationshipClaim {
        relation_type: RelationshipType::CatalogContinuity,
        subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "publisher",
            "candidate",
        )),
        target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
            "publisher",
            "destination",
        )),
        origin: RelationshipOrigin::DerivedCandidate {
            rule: RelationshipRule::new("catalog-comparison", "v1", "Compare source claims")?,
            supporting_assertions: vec![predecessor.clone(), predecessor.clone()],
        },
        evidence: RelationshipEvidence::CatalogComparison {
            status: ReconciliationStatus::Candidate,
            agreements: vec![EvidenceField::Crc, EvidenceField::Sha1],
            contradictions: vec![EvidenceField::Size],
        },
    };
    let successor = app::record_relationship(&catalog.database, &candidate)?;
    catalog.review(
        &predecessor,
        Some(&successor),
        RelationshipReviewDecision::Superseded,
    )?;
    assert_integer_owner_schema(&mut catalog.connection)?;
    let expected = app::explain_relationships(&catalog.database)?;
    assert_eq!(expected.len(), 2);
    assert_eq!(
        expected
            .iter()
            .find(|row| row.assertion_key == successor)
            .ok_or("missing candidate")?
            .claim,
        candidate
    );
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    let backup = catalog.path.with_file_name("backup.sqlite");
    let restored = catalog.path.with_file_name("restored.sqlite");
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, mame_coalesce::RestorePolicy::CreateNew)?;
    assert_eq!(
        app::explain_relationships(&Database::open(&restored)?)?,
        expected
    );
    Ok(())
}

#[test]
fn published_replacement_reviews_reject_active_cycles_atomically() -> TestResult {
    let catalog = Catalog::new()?;
    let first = catalog.claim("first")?;
    let second = catalog.claim("second")?;
    catalog.review(
        &first,
        Some(&second),
        RelationshipReviewDecision::Superseded,
    )?;
    let before = app::explain_relationships(&catalog.database)?;
    assert!(
        catalog
            .review(
                &second,
                Some(&first),
                RelationshipReviewDecision::Superseded
            )
            .is_err()
    );
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn withdrawn_supersession_remains_in_history_but_allows_a_reverse_correction() -> TestResult {
    let catalog = Catalog::new()?;
    let first = catalog.claim("first")?;
    let second = catalog.claim("second")?;
    catalog.review(
        &first,
        Some(&second),
        RelationshipReviewDecision::Superseded,
    )?;
    catalog.review(&first, None, RelationshipReviewDecision::Withdrawn)?;
    catalog.review(
        &second,
        Some(&first),
        RelationshipReviewDecision::Superseded,
    )?;
    let explanations = app::explain_relationships(&catalog.database)?;
    let original = explanations
        .iter()
        .find(|row| row.assertion_key == first)
        .ok_or("missing predecessor")?;
    assert_eq!(original.review_history.len(), 2);
    assert_eq!(
        original.review_history[0].review.superseded_by,
        Some(second.clone())
    );
    assert_eq!(
        original
            .latest_review
            .as_ref()
            .map(|review| review.decision),
        Some(RelationshipReviewDecision::Withdrawn)
    );
    let before = explanations;
    assert!(
        catalog
            .review(
                &first,
                Some(&second),
                RelationshipReviewDecision::Superseded
            )
            .is_err()
    );
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    Ok(())
}

#[test]
fn unsealed_reviews_are_invisible_but_integrity_and_backup_report_them() -> TestResult {
    let mut catalog = Catalog::new()?;
    let predecessor = catalog.claim("predecessor")?;
    let successor = catalog.claim("successor")?;
    catalog.review(&predecessor, None, RelationshipReviewDecision::Accepted)?;
    let before = app::explain_relationships(&catalog.database)?;
    let predecessor_id = catalog.id(&predecessor)?;
    let successor_id = catalog.id(&successor)?;
    let draft = catalog.draft_review("unfinished-review", predecessor_id, "superseded")?;
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    assert!(
        catalog.seal(draft).is_err(),
        "superseded review requires its successor"
    );
    catalog.replacement(draft, successor_id)?;
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    drop(catalog.database);
    let report = mame_coalesce::check_integrity(&catalog.path)?;
    assert!(
        report
            .durable_issues
            .iter()
            .any(|issue| issue.contains("unfinished-review") && issue.contains("publication")),
        "{report:?}"
    );
    assert!(
        mame_coalesce::create_backup(
            &catalog.path,
            &catalog.path.with_file_name("unfinished.backup")
        )
        .is_err()
    );
    sql_query("INSERT INTO catalog_relationship_review_publications VALUES(?)")
        .bind::<BigInt, _>(draft)
        .execute(&mut catalog.connection)?;
    let database = Database::open(&catalog.path)?;
    let after = app::explain_relationships(&database)?;
    let reviewed = after
        .iter()
        .find(|row| row.assertion_key == predecessor)
        .ok_or("missing predecessor")?;
    assert_eq!(reviewed.review_history.len(), 2);
    assert_eq!(
        reviewed
            .latest_review
            .as_ref()
            .and_then(|review| review.superseded_by.as_ref()),
        Some(&successor)
    );
    drop(database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn publishing_older_reviews_preserves_history_without_activating_historical_cycles() -> TestResult {
    let mut catalog = Catalog::new()?;
    let first = catalog.claim("first")?;
    let second = catalog.claim("second")?;
    let first_id = catalog.id(&first)?;
    let second_id = catalog.id(&second)?;
    let old = catalog.draft_review("old-draft", first_id, "superseded")?;
    catalog.replacement(old, second_id)?;
    catalog.review(&first, None, RelationshipReviewDecision::Withdrawn)?;
    catalog.review(
        &second,
        Some(&first),
        RelationshipReviewDecision::Superseded,
    )?;
    catalog.seal(old)?;
    let explanations = app::explain_relationships(&catalog.database)?;
    let original = explanations
        .iter()
        .find(|row| row.assertion_key == first)
        .ok_or("missing first")?;
    assert_eq!(original.review_history.len(), 2);
    assert_eq!(
        original.review_history[0].review.superseded_by,
        Some(second.clone())
    );
    assert_eq!(
        original
            .latest_review
            .as_ref()
            .map(|review| review.decision),
        Some(RelationshipReviewDecision::Withdrawn)
    );
    assert!(
        catalog
            .review(
                &first,
                Some(&second),
                RelationshipReviewDecision::Superseded
            )
            .is_err()
    );
    assert_eq!(app::explain_relationships(&catalog.database)?, explanations);
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn latest_published_review_is_selected_before_decision_and_ignores_newer_drafts() -> TestResult {
    let mut catalog = Catalog::new()?;
    let first = catalog.claim("first")?;
    let second = catalog.claim("second")?;
    let third = catalog.claim("third")?;
    catalog.review(
        &first,
        Some(&second),
        RelationshipReviewDecision::Superseded,
    )?;
    catalog.review(
        &second,
        Some(&third),
        RelationshipReviewDecision::Superseded,
    )?;
    let second_id = catalog.id(&second)?;
    let draft = catalog.draft_review("unsealed-withdrawal", second_id, "withdrawn")?;
    let before = app::explain_relationships(&catalog.database)?;
    assert!(
        catalog
            .review(&third, Some(&first), RelationshipReviewDecision::Superseded)
            .is_err()
    );
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    catalog.seal(draft)?;
    catalog.review(&third, Some(&first), RelationshipReviewDecision::Superseded)?;
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

fn assert_rejected(connection: &mut SqliteConnection, sql: &str) -> TestResult {
    let error = connection
        .batch_execute(sql)
        .err()
        .ok_or_else(|| format!("write bypassed guard: {sql}"))?;
    let message = error.to_string();
    assert!(
        !message.contains("no such") && !message.contains("syntax error"),
        "test did not exercise a valid write: {sql}: {message}"
    );
    Ok(())
}

#[test]
fn review_owners_edges_and_seals_reject_orphans_and_replacements_without_sqlite_enforcement()
-> TestResult {
    let mut catalog = Catalog::new()?;
    let first = catalog.claim("first")?;
    let second = catalog.claim("second")?;
    let first_id = catalog.id(&first)?;
    let second_id = catalog.id(&second)?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    for sql in [
        "INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES('orphan',-999999,'accepted','otherwise valid')".to_owned(),
        format!("INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES('invalid-decision',{first_id},'unknown','otherwise valid')"),
        "INSERT INTO replaced_catalog_relationships VALUES(-999999,1)".to_owned(),
        "INSERT INTO catalog_relationship_review_publications VALUES(-999999)".to_owned(),
    ] {
        assert_rejected(&mut catalog.connection, &sql)?;
    }
    let review = catalog.draft_review("issued-review", first_id, "superseded")?;
    assert!(catalog.replacement(review, -999_999).is_err());
    assert!(catalog.replacement(review, first_id).is_err());
    assert!(catalog.seal(review).is_err());
    assert_rejected(
        &mut catalog.connection,
        &format!(
            "INSERT OR REPLACE INTO catalog_relationship_reviews(review_id,review_key,relationship_id,decision,note) VALUES({},'issued-review',{second_id},'accepted','fresh primary key')",
            review + 100
        ),
    )?;
    catalog.replacement(review, second_id)?;
    catalog.seal(review)?;
    let before = app::explain_relationships(&catalog.database)?;
    for sql in [
        format!(
            "INSERT OR REPLACE INTO catalog_relationship_reviews SELECT * FROM catalog_relationship_reviews WHERE review_id={review}"
        ),
        format!("UPDATE catalog_relationship_reviews SET note='changed' WHERE review_id={review}"),
        format!("DELETE FROM catalog_relationship_reviews WHERE review_id={review}"),
        format!(
            "INSERT OR REPLACE INTO replaced_catalog_relationships VALUES({review},{first_id})"
        ),
        format!(
            "UPDATE replaced_catalog_relationships SET replacement_relationship_id={first_id} WHERE review_id={review}"
        ),
        format!("DELETE FROM replaced_catalog_relationships WHERE review_id={review}"),
        format!("INSERT OR REPLACE INTO catalog_relationship_review_publications VALUES({review})"),
        format!(
            "UPDATE catalog_relationship_review_publications SET review_id=-999999 WHERE review_id={review}"
        ),
        format!("DELETE FROM catalog_relationship_review_publications WHERE review_id={review}"),
    ] {
        assert_rejected(&mut catalog.connection, &sql)?;
    }
    let accepted = catalog.draft_review("accepted-review", second_id, "accepted")?;
    assert!(catalog.replacement(accepted, first_id).is_err());
    catalog.seal(accepted)?;
    assert!(catalog.replacement(accepted, first_id).is_err());
    let after = app::explain_relationships(&catalog.database)?;
    let original = after
        .iter()
        .find(|row| row.assertion_key == first)
        .ok_or("missing first")?;
    let previous = before
        .iter()
        .find(|row| row.assertion_key == first)
        .ok_or("missing first")?;
    assert_eq!(original, previous);
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn blank_unicode_review_notes_are_rejected_and_nonblank_literals_are_preserved() -> TestResult {
    let mut catalog = Catalog::new()?;
    let key = catalog.claim("owner")?;
    let owner = catalog.id(&key)?;
    for character in [
        '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}',
        '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
        '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}',
        '\u{3000}',
    ] {
        let blank = character.to_string().repeat(2);
        assert!(
            app::review_relationship(
                &catalog.database,
                &key,
                &RelationshipReview {
                    decision: RelationshipReviewDecision::Accepted,
                    note: blank.clone(),
                    superseded_by: None,
                }
            )
            .is_err()
        );
        assert!(sql_query("INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES(?,?,'accepted',?)")
            .bind::<Text,_>(format!("blank-{}", u32::from(character))).bind::<BigInt,_>(owner).bind::<Text,_>(&blank)
            .execute(&mut catalog.connection).is_err(), "SQL accepted whitespace U+{:04X}", u32::from(character));
    }
    for note in [" \0 ", "\u{200b}", "\u{feff}", "\u{3000}keep\u{85}"] {
        app::review_relationship(
            &catalog.database,
            &key,
            &RelationshipReview {
                decision: RelationshipReviewDecision::Accepted,
                note: note.into(),
                superseded_by: None,
            },
        )?;
    }
    let explanation = app::explain_relationships(&catalog.database)?
        .into_iter()
        .find(|row| row.assertion_key == key)
        .ok_or("missing owner")?;
    assert_eq!(
        explanation
            .review_history
            .iter()
            .map(|event| event.review.note.as_str())
            .collect::<Vec<_>>(),
        [" \0 ", "\u{200b}", "\u{feff}", "\u{3000}keep\u{85}"]
    );
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn child_foreign_keys_never_allocate_an_owner_when_the_requested_id_is_null() -> TestResult {
    for table in [
        "catalog_relationship_rationales",
        "catalog_relationship_comparisons",
        "catalog_relationship_evidence_publications",
        "replaced_catalog_relationships",
        "catalog_relationship_review_publications",
    ] {
        let mut catalog = Catalog::new()?;
        let first = catalog.claim("first")?;
        let second = catalog.claim("second")?;
        let first_id = catalog.id(&first)?;
        let second_id = catalog.id(&second)?;
        catalog.review(&first, None, RelationshipReviewDecision::Accepted)?;
        catalog
            .connection
            .batch_execute("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=OFF")?;
        catalog.connection.batch_execute(&format!(
            "INSERT INTO catalog_relationship_rules(rule_key,revision,description) VALUES('null-owner','v1','NULL owner witness'); \
             INSERT INTO catalog_relationships(relationship_id,assertion_key,origin) VALUES(-1,'negative-owner','derived'),(3,'unsealed-positive-owner','derived'); \
             INSERT INTO inferred_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id,rule_id) \
             SELECT registry.relationship_id,native.relation_type,native.from_target_id,native.to_target_id,rule.rule_id \
             FROM catalog_relationships registry CROSS JOIN manual_catalog_relationships native \
             CROSS JOIN catalog_relationship_rules rule \
             WHERE registry.relationship_id IN(-1,3) AND native.relationship_id={first_id} AND rule.rule_key='null-owner'"
        ))?;
        let write = match table {
            "catalog_relationship_rationales" => {
                "INSERT INTO catalog_relationship_rationales VALUES(NULL,'otherwise valid')"
                    .to_owned()
            }
            "catalog_relationship_comparisons" => {
                "INSERT INTO catalog_relationship_comparisons VALUES(NULL,'candidate')".to_owned()
            }
            "catalog_relationship_evidence_publications" => {
                catalog.connection.batch_execute(
                    "INSERT INTO catalog_relationship_rationales VALUES(-1,'negative owner only')",
                )?;
                "INSERT INTO catalog_relationship_evidence_publications VALUES(NULL,'rationale')"
                    .to_owned()
            }
            "replaced_catalog_relationships" => {
                catalog.connection.batch_execute(&format!("INSERT INTO catalog_relationship_reviews(review_id,review_key,relationship_id,decision,note) VALUES(-1,'negative-review',{first_id},'superseded','negative review only')"))?;
                format!("INSERT INTO replaced_catalog_relationships VALUES(NULL,{second_id})")
            }
            "catalog_relationship_review_publications" => {
                catalog.connection.batch_execute(&format!("INSERT INTO catalog_relationship_reviews(review_id,review_key,relationship_id,decision,note) VALUES(-1,'negative-review',{first_id},'accepted','negative review only'),(2,'unsealed-positive-review',{second_id},'accepted','positive review only')"))?;
                "INSERT INTO catalog_relationship_review_publications VALUES(NULL)".to_owned()
            }
            _ => return Err("unhandled child table fixture".into()),
        };
        assert_rejected(&mut catalog.connection, &write)?;
        let omitted = match table {
            "catalog_relationship_rationales" => "INSERT INTO catalog_relationship_rationales(reason) VALUES('otherwise valid')".to_owned(),
            "catalog_relationship_comparisons" => "INSERT INTO catalog_relationship_comparisons(status) VALUES('candidate')".to_owned(),
            "catalog_relationship_evidence_publications" => "INSERT INTO catalog_relationship_evidence_publications(evidence_kind) VALUES('rationale')".to_owned(),
            "replaced_catalog_relationships" => format!("INSERT INTO replaced_catalog_relationships(replacement_relationship_id) VALUES({second_id})"),
            "catalog_relationship_review_publications" => "INSERT INTO catalog_relationship_review_publications DEFAULT VALUES".to_owned(),
            _ => return Err("unhandled omitted-owner fixture".into()),
        };
        assert_rejected(&mut catalog.connection, &omitted)?;
    }
    Ok(())
}
