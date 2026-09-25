use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};

use crate::domain::{
    CatalogRecordKind, CatalogRecordRef, ContentDigestAlgorithm, ContentIdentity, DocumentLocation,
    RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipExplanation,
    RelationshipOrigin, RelationshipReview, RelationshipReviewDecision, RelationshipReviewEvent,
    RelationshipSourceProvenance, RelationshipType, SnapshotKey,
};

use super::db::Pool;

#[derive(QueryableByName)]
struct ExplanationRow {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = Text)]
    relation_type: String,
    #[diesel(sql_type = Text)]
    origin: String,
    #[diesel(sql_type = Nullable<Text>)]
    subject_snapshot_key: Option<String>,
    #[diesel(sql_type = Text)]
    subject_kind: String,
    #[diesel(sql_type = Text)]
    subject_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    target_snapshot_key: Option<String>,
    #[diesel(sql_type = Text)]
    target_kind: String,
    #[diesel(sql_type = Text)]
    target_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
    #[diesel(sql_type = Text)]
    evidence_json: String,
    #[diesel(sql_type = Nullable<Text>)]
    rule_version: Option<String>,
    #[diesel(sql_type = Text)]
    supporting_assertion_keys_json: String,
    #[diesel(sql_type = Nullable<Text>)]
    source_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    document_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    parser_name: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    parser_version: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    rules_version: Option<String>,
}

#[derive(QueryableByName)]
struct ReviewRow {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = Text)]
    decision: String,
    #[diesel(sql_type = Text)]
    note: String,
    #[diesel(sql_type = Nullable<Text>)]
    superseded_by_assertion_key: Option<String>,
    #[diesel(sql_type = Text)]
    created_at: String,
}

pub struct SourceRelationshipDraft {
    pub relation_type: RelationshipType,
    pub subject: CatalogRecordRef,
    pub target: CatalogRecordRef,
    pub source_field: String,
    pub source_location: Option<DocumentLocation>,
    pub evidence: serde_json::Value,
}

pub fn insert_source_assertion(
    conn: &mut SqliteConnection,
    draft: SourceRelationshipDraft,
) -> crate::Result<RelationshipAssertionKey> {
    let snapshot = draft.subject.snapshot.clone();
    let claim = RelationshipClaim {
        relation_type: draft.relation_type,
        subject: RelationshipEndpoint::CatalogRecord(draft.subject),
        target: RelationshipEndpoint::CatalogRecord(draft.target),
        origin: RelationshipOrigin::SourceAssertion {
            snapshot,
            field: draft.source_field,
            location: draft.source_location,
        },
        evidence: draft.evidence,
    };
    insert_claim(conn, &RelationshipAssertionKey::fresh(), &claim)
}

pub fn record_claim(
    pool: &Pool,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    validate_claim(claim)?;
    if matches!(&claim.origin, RelationshipOrigin::SourceAssertion { .. }) {
        return Err(crate::Error::InvalidPath(
            "source assertions can only be persisted by catalog import".to_owned(),
        ));
    }
    let key = RelationshipAssertionKey::fresh();
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        if let RelationshipOrigin::DerivedCandidate {
            supporting_assertions,
            ..
        } = &claim.origin
        {
            for supported in supporting_assertions {
                let found = sql_query(
                    "SELECT COUNT(*) AS found FROM relationship_assertions WHERE assertion_key = ?",
                )
                .bind::<Text, _>(supported.as_str())
                .get_result::<AssertionKeyRow>(conn)?;
                if found.found == 0 {
                    return Err(crate::Error::InvalidPath(format!(
                        "supporting relationship assertion {} does not exist",
                        supported.as_str()
                    )));
                }
            }
        }
        insert_claim(conn, &key, claim)
    })?;
    Ok(key)
}

#[derive(QueryableByName)]
struct AssertionKeyRow {
    #[diesel(sql_type = BigInt)]
    found: i64,
}

fn validate_claim(claim: &RelationshipClaim) -> crate::Result<()> {
    if !claim.evidence.is_object() {
        return Err(crate::Error::InvalidPath(
            "relationship evidence must be a JSON object".to_owned(),
        ));
    }
    match &claim.origin {
        RelationshipOrigin::SourceAssertion {
            snapshot,
            field,
            location,
        } => {
            if field.is_empty() {
                return Err(crate::Error::InvalidPath(
                    "source relationship field must not be empty".to_owned(),
                ));
            }
            for endpoint in [&claim.subject, &claim.target] {
                if let RelationshipEndpoint::CatalogRecord(record) = endpoint
                    && &record.snapshot != snapshot
                {
                    return Err(crate::Error::InvalidPath(
                        "source assertion endpoints must belong to its source snapshot".to_owned(),
                    ));
                }
            }
            if location.is_some_and(|location| location.line <= 0 || location.column <= 0) {
                return Err(crate::Error::InvalidPath(
                    "source relationship location must be positive".to_owned(),
                ));
            }
        }
        RelationshipOrigin::DerivedCandidate { rule_version, .. }
            if rule_version.trim().is_empty() =>
        {
            return Err(crate::Error::InvalidPath(
                "derived relationship candidate requires a rule version".to_owned(),
            ));
        }
        RelationshipOrigin::UserConclusion | RelationshipOrigin::DerivedCandidate { .. } => {}
    }
    Ok(())
}

fn endpoint_parts(
    endpoint: &RelationshipEndpoint,
) -> crate::Result<(Option<String>, &'static str, String)> {
    match endpoint {
        RelationshipEndpoint::CatalogRecord(record) => Ok((
            Some(record.snapshot.as_str().to_owned()),
            record.kind.as_str(),
            record.key.as_str().to_owned(),
        )),
        RelationshipEndpoint::ContentObject(identity) => Ok((
            None,
            "content_object",
            format!("{}:{}", identity.algorithm().as_str(), identity.digest()),
        )),
        RelationshipEndpoint::ExternalRecord(record) => {
            Ok((None, "external_record", serde_json::to_string(record)?))
        }
    }
}

fn insert_claim(
    conn: &mut SqliteConnection,
    assertion_key: &RelationshipAssertionKey,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    validate_claim(claim)?;
    let (subject_snapshot, subject_kind, subject_key) = endpoint_parts(&claim.subject)?;
    let (target_snapshot, target_kind, target_key) = endpoint_parts(&claim.target)?;
    let (origin, source_snapshot, source_field, location, rule_version, supporting) =
        match &claim.origin {
            RelationshipOrigin::SourceAssertion {
                snapshot,
                field,
                location,
            } => (
                "source_assertion",
                Some(snapshot.as_str().to_owned()),
                Some(field.clone()),
                *location,
                None,
                Vec::new(),
            ),
            RelationshipOrigin::DerivedCandidate {
                rule_version,
                supporting_assertions,
            } => (
                "derived_candidate",
                None,
                None,
                None,
                Some(rule_version.clone()),
                supporting_assertions
                    .iter()
                    .map(|key| key.as_str().to_owned())
                    .collect(),
            ),
            RelationshipOrigin::UserConclusion => {
                ("user_conclusion", None, None, None, None, Vec::new())
            }
        };
    let supporting_json = serde_json::to_string(&supporting)?;
    sql_query(
        "INSERT INTO relationship_assertions \
         (assertion_key, relation_type, origin, subject_snapshot_key, subject_kind, subject_key, \
          target_snapshot_key, target_kind, target_key, source_snapshot_key, source_field, \
          source_line, source_column, evidence_json, rule_version, supporting_assertion_keys_json) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key.as_str())
    .bind::<Text, _>(claim.relation_type.as_str())
    .bind::<Text, _>(origin)
    .bind::<Nullable<Text>, _>(subject_snapshot)
    .bind::<Text, _>(subject_kind)
    .bind::<Text, _>(subject_key)
    .bind::<Nullable<Text>, _>(target_snapshot)
    .bind::<Text, _>(target_kind)
    .bind::<Text, _>(target_key)
    .bind::<Nullable<Text>, _>(source_snapshot)
    .bind::<Nullable<Text>, _>(source_field)
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.line))
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.column))
    .bind::<Text, _>(serde_json::to_string(&claim.evidence)?)
    .bind::<Nullable<Text>, _>(rule_version)
    .bind::<Text, _>(supporting_json)
    .execute(conn)?;
    Ok(assertion_key.clone())
}

pub fn review_claim(
    pool: &Pool,
    assertion_key: &RelationshipAssertionKey,
    review: &RelationshipReview,
) -> crate::Result<()> {
    if review.note.trim().is_empty() {
        return Err(crate::Error::InvalidPath(
            "relationship review note must not be empty".to_owned(),
        ));
    }
    if (review.decision == RelationshipReviewDecision::Superseded) != review.superseded_by.is_some()
    {
        return Err(crate::Error::InvalidPath(
            "superseded reviews must identify the replacement assertion".to_owned(),
        ));
    }
    if review
        .superseded_by
        .as_ref()
        .is_some_and(|key| key == assertion_key)
    {
        return Err(crate::Error::InvalidPath(
            "an assertion cannot supersede itself".to_owned(),
        ));
    }
    let mut conn = pool.get()?;
    conn.immediate_transaction::<_, crate::Error, _>(|conn| {
        sql_query(
            "INSERT INTO relationship_reviews \
             (review_key, assertion_key, decision, note, superseded_by_assertion_key) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind::<Text, _>(uuid::Uuid::new_v4().to_string())
        .bind::<Text, _>(assertion_key.as_str())
        .bind::<Text, _>(review.decision.as_str())
        .bind::<Text, _>(&review.note)
        .bind::<Nullable<Text>, _>(
            review
                .superseded_by
                .as_ref()
                .map(|key| key.as_str().to_owned()),
        )
        .execute(conn)?;
        Ok(())
    })
}

pub fn explain_all(pool: &Pool) -> crate::Result<Vec<RelationshipExplanation>> {
    let mut conn = pool.get()?;
    let rows = sql_query(
        "SELECT a.assertion_key, a.relation_type, a.origin, \
                a.subject_snapshot_key, a.subject_kind, a.subject_key, \
                a.target_snapshot_key, a.target_kind, a.target_key, \
                a.source_snapshot_key, a.source_field, a.source_line, a.source_column, \
                a.evidence_json, a.rule_version, a.supporting_assertion_keys_json, \
                ps.source_key, ps.display_name AS source_name, \
                s.document_key, s.declared_version, pi.parser_name, pi.parser_version, pi.rules_version \
         FROM relationship_assertions a \
         LEFT JOIN catalog_snapshots s ON s.snapshot_key = a.source_snapshot_key \
         LEFT JOIN catalogs c ON c.catalog_key = s.catalog_key \
         LEFT JOIN publishing_sources ps ON ps.source_key = c.source_key \
         LEFT JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, a.subject_key, \
                  a.target_kind, a.target_key, a.assertion_key",
    )
    .load::<ExplanationRow>(&mut conn)?;
    let reviews = sql_query(
        "SELECT assertion_key, decision, note, superseded_by_assertion_key, created_at \
         FROM relationship_reviews ORDER BY review_id",
    )
    .load::<ReviewRow>(&mut conn)?;
    let mut history = std::collections::BTreeMap::<String, Vec<RelationshipReviewEvent>>::new();
    for review in reviews {
        history
            .entry(review.assertion_key)
            .or_default()
            .push(RelationshipReviewEvent {
                review: RelationshipReview {
                    decision: parse_review_decision(&review.decision)?,
                    note: review.note,
                    superseded_by: review
                        .superseded_by_assertion_key
                        .map(RelationshipAssertionKey::new),
                },
                created_at: review.created_at,
            });
    }
    rows.into_iter()
        .map(|row| {
            let events = history.remove(&row.assertion_key).unwrap_or_default();
            explanation(row, events)
        })
        .collect()
}

fn explanation(
    row: ExplanationRow,
    review_history: Vec<RelationshipReviewEvent>,
) -> crate::Result<RelationshipExplanation> {
    let relation_type = parse_relation_type(&row.relation_type)?;
    let origin = match row.origin.as_str() {
        "source_assertion" => RelationshipOrigin::SourceAssertion {
            snapshot: SnapshotKey::from_persisted(row.source_snapshot_key.clone().ok_or_else(
                || crate::Error::InvalidPath("source assertion has no snapshot".to_owned()),
            )?),
            field: row.source_field.clone().ok_or_else(|| {
                crate::Error::InvalidPath("source assertion has no source field".to_owned())
            })?,
            location: row
                .source_line
                .zip(row.source_column)
                .map(|(line, column)| DocumentLocation { line, column }),
        },
        "derived_candidate" => {
            let supporting_assertions: Vec<String> =
                serde_json::from_str(&row.supporting_assertion_keys_json)?;
            RelationshipOrigin::DerivedCandidate {
                rule_version: row.rule_version.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("candidate has no rule version".to_owned())
                })?,
                supporting_assertions: supporting_assertions
                    .into_iter()
                    .map(RelationshipAssertionKey::new)
                    .collect(),
            }
        }
        "user_conclusion" => RelationshipOrigin::UserConclusion,
        origin => {
            return Err(crate::Error::InvalidPath(format!(
                "unknown relationship origin {origin}"
            )));
        }
    };
    let latest_review = review_history.last().map(|event| event.review.clone());
    let source = row
        .source_key
        .map(|source_key| RelationshipSourceProvenance {
            source_key,
            source_name: row.source_name.unwrap_or_default(),
            document_key: row.document_key.unwrap_or_default(),
            declared_version: row.declared_version,
            parser_name: row.parser_name,
            parser_version: row.parser_version,
            rules_version: row.rules_version,
        });
    Ok(RelationshipExplanation {
        assertion_key: RelationshipAssertionKey::new(row.assertion_key),
        claim: RelationshipClaim {
            relation_type,
            subject: endpoint(
                row.subject_snapshot_key,
                &row.subject_kind,
                &row.subject_key,
            )?,
            target: endpoint(row.target_snapshot_key, &row.target_kind, &row.target_key)?,
            origin,
            evidence: serde_json::from_str(&row.evidence_json)?,
        },
        source_field: row.source_field,
        source_location: row
            .source_line
            .zip(row.source_column)
            .map(|(line, column)| DocumentLocation { line, column }),
        source,
        latest_review,
        review_history,
    })
}

fn endpoint(
    snapshot: Option<String>,
    kind: &str,
    key: &str,
) -> crate::Result<RelationshipEndpoint> {
    match kind {
        "catalog_set" | "asset_requirement" | "software_item" => {
            let kind = match kind {
                "catalog_set" => CatalogRecordKind::Set,
                "asset_requirement" => CatalogRecordKind::AssetRequirement,
                _ => CatalogRecordKind::SoftwareItem,
            };
            Ok(RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
                SnapshotKey::from_persisted(snapshot.ok_or_else(|| {
                    crate::Error::InvalidPath(
                        "catalog relationship endpoint has no snapshot".into(),
                    )
                })?),
                kind,
                key,
            )))
        }
        "content_object" => {
            let (algorithm, digest) = key.split_once(':').ok_or_else(|| {
                crate::Error::InvalidPath("content identity is missing its digest algorithm".into())
            })?;
            let algorithm = match algorithm {
                "crc32" => ContentDigestAlgorithm::Crc32,
                "md5" => ContentDigestAlgorithm::Md5,
                "sha1" => ContentDigestAlgorithm::Sha1,
                "sha256" => ContentDigestAlgorithm::Sha256,
                _ => return Err(crate::Error::InvalidPath("unknown digest algorithm".into())),
            };
            Ok(RelationshipEndpoint::ContentObject(ContentIdentity::new(
                algorithm, digest,
            )?))
        }
        "external_record" => Ok(RelationshipEndpoint::ExternalRecord(serde_json::from_str(
            key,
        )?)),
        other => Err(crate::Error::InvalidPath(format!(
            "unknown relationship endpoint kind {other}"
        ))),
    }
}

fn parse_relation_type(value: &str) -> crate::Result<RelationshipType> {
    match value {
        "exact_content_identity" => Ok(RelationshipType::ExactContentIdentity),
        "revision_of" => Ok(RelationshipType::RevisionOf),
        "dump_of_intended_release" => Ok(RelationshipType::DumpOfIntendedRelease),
        "alternate_representation_of" => Ok(RelationshipType::AlternateRepresentationOf),
        "source_parent_clone" => Ok(RelationshipType::SourceParentClone),
        "runtime_dependency" => Ok(RelationshipType::RuntimeDependency),
        "catalog_correction" => Ok(RelationshipType::CatalogCorrection),
        "catalog_continuity" => Ok(RelationshipType::CatalogContinuity),
        other => Err(crate::Error::InvalidPath(format!(
            "unknown relationship type {other}"
        ))),
    }
}

fn parse_review_decision(value: &str) -> crate::Result<RelationshipReviewDecision> {
    match value {
        "accepted" => Ok(RelationshipReviewDecision::Accepted),
        "rejected" => Ok(RelationshipReviewDecision::Rejected),
        "withdrawn" => Ok(RelationshipReviewDecision::Withdrawn),
        "superseded" => Ok(RelationshipReviewDecision::Superseded),
        other => Err(crate::Error::InvalidPath(format!(
            "unknown relationship review decision {other}"
        ))),
    }
}
