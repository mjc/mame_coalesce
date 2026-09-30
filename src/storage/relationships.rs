use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Double, Nullable, Text},
};
use std::collections::HashMap;

use crate::domain::{
    CatalogRecordKind, CatalogRecordRef, ContentDigestAlgorithm, ContentIdentity, DocumentLocation,
    ExternalRecordRef, RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint,
    RelationshipExplanation, RelationshipOrigin, RelationshipReview, RelationshipReviewDecision,
    RelationshipReviewEvent, RelationshipSourceProvenance, RelationshipType, SnapshotKey,
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
    #[diesel(sql_type = Nullable<Text>)]
    generic_subject_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    generic_subject_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    generic_subject_c: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_subject_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_subject_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_subject_c: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    target_snapshot_key: Option<String>,
    #[diesel(sql_type = Text)]
    target_kind: String,
    #[diesel(sql_type = Nullable<Text>)]
    generic_target_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    generic_target_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    generic_target_c: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_target_a: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_target_b: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_target_c: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_snapshot_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    source_field: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_line: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_column: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    source_asset_merge_name: Option<String>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    source_asset_sha1: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Binary>)]
    source_asset_crc: Option<Vec<u8>>,
    #[diesel(sql_type = Nullable<BigInt>)]
    source_asset_size: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    rule_version: Option<String>,
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
struct EvidenceNodeRow {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = BigInt)]
    node_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    parent_node_id: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    object_key: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    array_index: Option<i64>,
    #[diesel(sql_type = Text)]
    value_type: String,
    #[diesel(sql_type = Nullable<Text>)]
    text_value: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    integer_value: Option<i64>,
    #[diesel(sql_type = Nullable<Text>)]
    unsigned_integer_value: Option<String>,
    #[diesel(sql_type = Nullable<Double>)]
    real_value: Option<f64>,
}

#[derive(QueryableByName)]
struct SupportRow {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = Text)]
    supported_assertion_key: String,
}

type SourceEndpointParts = (&'static str, Option<String>, Option<String>, Option<i64>);
type EndpointParts = (
    Option<String>,
    &'static str,
    Option<String>,
    Option<String>,
    Option<i64>,
);

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
    validate_claim(&claim)?;
    let key = RelationshipAssertionKey::fresh();
    insert_source_claim(conn, &key, &claim)?;
    Ok(key)
}

fn insert_source_claim(
    conn: &mut SqliteConnection,
    assertion_key: &RelationshipAssertionKey,
    claim: &RelationshipClaim,
) -> crate::Result<()> {
    let RelationshipOrigin::SourceAssertion {
        snapshot,
        field,
        location,
    } = &claim.origin
    else {
        return Err(crate::Error::InvalidPath(
            "typed source assertion requires source provenance".to_owned(),
        ));
    };
    let (subject_kind, subject_a, subject_b, subject_c) = source_endpoint_parts(&claim.subject)?;
    let (target_kind, target_a, target_b, target_c) = source_endpoint_parts(&claim.target)?;
    validate_source_evidence(
        claim,
        subject_kind,
        subject_a.as_deref(),
        target_kind,
        target_a.as_deref(),
        target_b.as_deref(),
    )?;
    sql_query(
        "INSERT INTO relationship_assertions \
         (assertion_key, relation_type, origin, source_snapshot_key, source_field, source_line, \
          source_column, subject_kind, source_subject_a, source_subject_b, source_subject_c, \
          target_kind, source_target_a, source_target_b, source_target_c) \
         VALUES (?, ?, 'source_assertion', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key.as_str())
    .bind::<Text, _>(claim.relation_type.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(field)
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.line))
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.column))
    .bind::<Text, _>(subject_kind)
    .bind::<Nullable<Text>, _>(subject_a)
    .bind::<Nullable<Text>, _>(subject_b)
    .bind::<Nullable<BigInt>, _>(subject_c)
    .bind::<Text, _>(target_kind)
    .bind::<Nullable<Text>, _>(target_a)
    .bind::<Nullable<Text>, _>(target_b)
    .bind::<Nullable<BigInt>, _>(target_c)
    .execute(conn)?;
    Ok(())
}

fn source_endpoint_parts(endpoint: &RelationshipEndpoint) -> crate::Result<SourceEndpointParts> {
    let RelationshipEndpoint::CatalogRecord(record) = endpoint else {
        return Err(crate::Error::InvalidPath(
            "source relationship endpoints must be catalog records".to_owned(),
        ));
    };
    let encoded = record.key.as_str();
    let (first, second, third) = match record.kind {
        CatalogRecordKind::Set => (encoded.to_owned(), None, None),
        CatalogRecordKind::SoftwareItem | CatalogRecordKind::AssetRequirement => {
            let parts: Vec<serde_json::Value> = serde_json::from_str(encoded)?;
            let expected = if record.kind == CatalogRecordKind::SoftwareItem {
                2
            } else {
                3
            };
            if parts.len() != expected {
                return Err(crate::Error::InvalidPath(
                    "source relationship endpoint has an invalid composite key".to_owned(),
                ));
            }
            let first = parts[0]
                .as_str()
                .ok_or_else(|| crate::Error::InvalidPath("invalid source endpoint key".into()))?
                .to_owned();
            let second = parts[1]
                .as_str()
                .ok_or_else(|| crate::Error::InvalidPath("invalid source endpoint key".into()))?
                .to_owned();
            let third = if expected == 3 {
                Some(parts[2].as_i64().ok_or_else(|| {
                    crate::Error::InvalidPath("invalid source endpoint order".into())
                })?)
            } else {
                None
            };
            (first, Some(second), third)
        }
    };
    Ok((record.kind.as_str(), Some(first), second, third))
}

fn validate_source_evidence(
    claim: &RelationshipClaim,
    subject_kind: &str,
    subject_a: Option<&str>,
    target_kind: &str,
    target_a: Option<&str>,
    target_b: Option<&str>,
) -> crate::Result<()> {
    let (field, evidence) = match &claim.origin {
        RelationshipOrigin::SourceAssertion { field, .. } => (field.as_str(), &claim.evidence),
        _ => unreachable!("source evidence is only validated for source claims"),
    };
    let matches = match field {
        "merge" => {
            subject_kind == "asset_requirement"
                && target_kind == "asset_requirement"
                && evidence
                    .get("declared_merge_name")
                    .and_then(serde_json::Value::as_str)
                    == target_b
        }
        "cloneof" if subject_kind == "software_item" => {
            evidence
                .get("list_name")
                .and_then(serde_json::Value::as_str)
                == subject_a
                && evidence
                    .get("target_item_name")
                    .and_then(serde_json::Value::as_str)
                    == target_b
        }
        "cloneof" | "parent_name" | "device_ref" | "romof" | "sampleof" => {
            evidence
                .get("target_name")
                .and_then(serde_json::Value::as_str)
                == target_a
        }
        _ => false,
    };
    if !matches {
        return Err(crate::Error::InvalidPath(format!(
            "source evidence does not match relationship field {field}"
        )));
    }
    if field == "merge" {
        for (name, length) in [("expected_sha1", 20), ("expected_crc", 4)] {
            if let Some(encoded) = evidence.get(name).and_then(serde_json::Value::as_str) {
                let valid = hex::decode(encoded).is_ok_and(|digest| digest.len() == length);
                if !valid {
                    return Err(crate::Error::InvalidPath(format!(
                        "source evidence {name} has an invalid digest"
                    )));
                }
            }
        }
    }
    Ok(())
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

fn endpoint_parts(endpoint: &RelationshipEndpoint) -> crate::Result<EndpointParts> {
    match endpoint {
        RelationshipEndpoint::CatalogRecord(record) => {
            let (kind, first, second, third) = source_endpoint_parts(endpoint)?;
            Ok((
                Some(record.snapshot.as_str().to_owned()),
                kind,
                first,
                second,
                third,
            ))
        }
        RelationshipEndpoint::ContentObject(identity) => Ok((
            None,
            "content_object",
            Some(identity.algorithm().as_str().to_owned()),
            Some(identity.digest().to_owned()),
            None,
        )),
        RelationshipEndpoint::ExternalRecord(record) => Ok((
            None,
            "external_record",
            Some(record.namespace.clone()),
            Some(record.key.as_str().to_owned()),
            None,
        )),
    }
}

fn insert_claim(
    conn: &mut SqliteConnection,
    assertion_key: &RelationshipAssertionKey,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    validate_claim(claim)?;
    let (subject_snapshot, subject_kind, subject_a, subject_b, subject_c) =
        endpoint_parts(&claim.subject)?;
    let (target_snapshot, target_kind, target_a, target_b, target_c) =
        endpoint_parts(&claim.target)?;
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
    sql_query(
        "INSERT INTO relationship_assertions \
         (assertion_key, relation_type, origin, generic_subject_snapshot_key, subject_kind, \
          generic_subject_a, generic_subject_b, generic_subject_c, generic_target_snapshot_key, \
          target_kind, generic_target_a, generic_target_b, generic_target_c, source_snapshot_key, \
          source_field, source_line, source_column, rule_version) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key.as_str())
    .bind::<Text, _>(claim.relation_type.as_str())
    .bind::<Text, _>(origin)
    .bind::<Nullable<Text>, _>(subject_snapshot)
    .bind::<Text, _>(subject_kind)
    .bind::<Nullable<Text>, _>(subject_a)
    .bind::<Nullable<Text>, _>(subject_b)
    .bind::<Nullable<BigInt>, _>(subject_c)
    .bind::<Nullable<Text>, _>(target_snapshot)
    .bind::<Text, _>(target_kind)
    .bind::<Nullable<Text>, _>(target_a)
    .bind::<Nullable<Text>, _>(target_b)
    .bind::<Nullable<BigInt>, _>(target_c)
    .bind::<Nullable<Text>, _>(source_snapshot)
    .bind::<Nullable<Text>, _>(source_field)
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.line))
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.column))
    .bind::<Nullable<Text>, _>(rule_version)
    .execute(conn)?;
    let mut next_node_id = 0;
    insert_evidence_node(
        conn,
        assertion_key.as_str(),
        None,
        None,
        None,
        &claim.evidence,
        &mut next_node_id,
    )?;
    for (position, supported_assertion_key) in supporting.iter().enumerate() {
        sql_query(
            "INSERT INTO relationship_assertion_support \
             (assertion_key, position, supported_assertion_key) VALUES (?, ?, ?)",
        )
        .bind::<Text, _>(assertion_key.as_str())
        .bind::<BigInt, _>(i64::try_from(position).map_err(|_| {
            crate::Error::InvalidPath("relationship support position exceeds i64".into())
        })?)
        .bind::<Text, _>(supported_assertion_key)
        .execute(conn)?;
    }
    Ok(assertion_key.clone())
}

fn insert_evidence_node(
    conn: &mut SqliteConnection,
    assertion_key: &str,
    parent_node_id: Option<i64>,
    object_key: Option<&str>,
    array_index: Option<i64>,
    value: &serde_json::Value,
    next_node_id: &mut i64,
) -> crate::Result<()> {
    let node_id = *next_node_id;
    *next_node_id += 1;
    let (value_type, text_value, integer_value, unsigned_integer_value, real_value) = match value {
        serde_json::Value::Object(_) => ("object", None, None, None, None),
        serde_json::Value::Array(_) => ("array", None, None, None, None),
        serde_json::Value::String(value) => ("string", Some(value.clone()), None, None, None),
        serde_json::Value::Number(value) if value.as_i64().is_some() => {
            ("integer", None, value.as_i64(), None, None)
        }
        serde_json::Value::Number(value) if value.as_u64().is_some() => (
            "unsigned_integer",
            None,
            None,
            value.as_u64().map(|number| number.to_string()),
            None,
        ),
        serde_json::Value::Number(value) => (
            "real",
            None,
            None,
            None,
            Some(value.as_f64().ok_or_else(|| {
                crate::Error::InvalidPath(
                    "relationship evidence number is not representable".into(),
                )
            })?),
        ),
        serde_json::Value::Bool(true) => ("true", None, None, None, None),
        serde_json::Value::Bool(false) => ("false", None, None, None, None),
        serde_json::Value::Null => ("null", None, None, None, None),
    };
    sql_query(
        "INSERT INTO relationship_assertion_evidence \
         (assertion_key, node_id, parent_node_id, object_key, array_index, value_type, \
          text_value, integer_value, unsigned_integer_value, real_value) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key)
    .bind::<BigInt, _>(node_id)
    .bind::<Nullable<BigInt>, _>(parent_node_id)
    .bind::<Nullable<Text>, _>(object_key)
    .bind::<Nullable<BigInt>, _>(array_index)
    .bind::<Text, _>(value_type)
    .bind::<Nullable<Text>, _>(text_value)
    .bind::<Nullable<BigInt>, _>(integer_value)
    .bind::<Nullable<Text>, _>(unsigned_integer_value)
    .bind::<Nullable<Double>, _>(real_value)
    .execute(conn)?;
    if let Some(object) = value.as_object() {
        for (key, child) in object {
            insert_evidence_node(
                conn,
                assertion_key,
                Some(node_id),
                Some(key),
                None,
                child,
                next_node_id,
            )?;
        }
    } else if let Some(array) = value.as_array() {
        for (index, child) in array.iter().enumerate() {
            insert_evidence_node(
                conn,
                assertion_key,
                Some(node_id),
                None,
                Some(i64::try_from(index).map_err(|_| {
                    crate::Error::InvalidPath("relationship evidence array exceeds i64".into())
                })?),
                child,
                next_node_id,
            )?;
        }
    }
    Ok(())
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
                a.subject_snapshot_key, a.subject_kind, a.generic_subject_a, a.generic_subject_b, \
                a.generic_subject_c, a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                a.target_snapshot_key, a.target_kind, a.generic_target_a, a.generic_target_b, \
                a.generic_target_c, a.source_target_a, a.source_target_b, a.source_target_c, \
                a.source_snapshot_key, a.source_field, a.source_line, a.source_column, \
                source_asset.merge_name AS source_asset_merge_name, \
                source_asset.sha1 AS source_asset_sha1, source_asset.crc AS source_asset_crc, \
                source_asset.size AS source_asset_size, a.rule_version, \
                ps.source_key, ps.display_name AS source_name, \
                s.document_key, s.declared_version, pi.parser_name, pi.parser_version, pi.rules_version \
         FROM relationship_assertions a \
         LEFT JOIN asset_requirements source_asset \
           ON a.origin = 'source_assertion' AND a.subject_kind = 'asset_requirement' \
          AND source_asset.snapshot_key = a.source_snapshot_key \
          AND source_asset.set_name = a.source_subject_a \
          AND source_asset.component_order = a.source_subject_c \
         LEFT JOIN catalog_snapshots s ON s.snapshot_key = a.source_snapshot_key \
         LEFT JOIN catalogs c ON c.catalog_key = s.catalog_key \
         LEFT JOIN publishing_sources ps ON ps.source_key = c.source_key \
         LEFT JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, \
                  a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                  a.generic_subject_a, a.generic_subject_b, a.generic_subject_c, \
                  a.target_kind, a.source_target_a, a.source_target_b, a.source_target_c, \
                  a.generic_target_a, a.generic_target_b, a.generic_target_c, a.assertion_key",
    )
    .load::<ExplanationRow>(&mut conn)?;
    let reviews = sql_query(
        "SELECT assertion_key, decision, note, superseded_by_assertion_key, created_at \
         FROM relationship_reviews ORDER BY review_id",
    )
    .load::<ReviewRow>(&mut conn)?;
    build_explanations(&mut conn, rows, reviews)
}

pub fn explain_catalog_sets_for_snapshots(
    conn: &mut SqliteConnection,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<Vec<RelationshipExplanation>> {
    let rows = sql_query(
        "SELECT a.assertion_key, a.relation_type, a.origin, \
                a.subject_snapshot_key, a.subject_kind, a.generic_subject_a, a.generic_subject_b, \
                a.generic_subject_c, a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                a.target_snapshot_key, a.target_kind, a.generic_target_a, a.generic_target_b, \
                a.generic_target_c, a.source_target_a, a.source_target_b, a.source_target_c, \
                a.source_snapshot_key, a.source_field, a.source_line, a.source_column, \
                source_asset.merge_name AS source_asset_merge_name, \
                source_asset.sha1 AS source_asset_sha1, source_asset.crc AS source_asset_crc, \
                source_asset.size AS source_asset_size, a.rule_version, \
                ps.source_key, ps.display_name AS source_name, \
                s.document_key, s.declared_version, pi.parser_name, pi.parser_version, pi.rules_version \
         FROM relationship_assertions a \
         LEFT JOIN asset_requirements source_asset \
           ON a.origin = 'source_assertion' AND a.subject_kind = 'asset_requirement' \
          AND source_asset.snapshot_key = a.source_snapshot_key \
          AND source_asset.set_name = a.source_subject_a \
          AND source_asset.component_order = a.source_subject_c \
         LEFT JOIN catalog_snapshots s ON s.snapshot_key = a.source_snapshot_key \
         LEFT JOIN catalogs c ON c.catalog_key = s.catalog_key \
         LEFT JOIN publishing_sources ps ON ps.source_key = c.source_key \
         LEFT JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         WHERE (a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?) \
                AND (a.subject_kind = 'catalog_set' OR a.target_kind = 'catalog_set')) \
            OR (a.origin != 'source_assertion' AND ( \
                (a.subject_kind = 'catalog_set' AND a.generic_subject_snapshot_key IN (?, ?)) \
                OR (a.target_kind = 'catalog_set' AND a.generic_target_snapshot_key IN (?, ?)))) \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, \
                  a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                  a.generic_subject_a, a.generic_subject_b, a.generic_subject_c, \
                  a.target_kind, a.source_target_a, a.source_target_b, a.source_target_c, \
                  a.generic_target_a, a.generic_target_b, a.generic_target_c, a.assertion_key",
    )
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .load::<ExplanationRow>(conn)?;
    let reviews = sql_query(
        "SELECT r.assertion_key, r.decision, r.note, r.superseded_by_assertion_key, r.created_at \
         FROM relationship_reviews r \
         JOIN relationship_assertions a ON a.assertion_key = r.assertion_key \
         WHERE (a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?) \
                AND (a.subject_kind = 'catalog_set' OR a.target_kind = 'catalog_set')) \
            OR (a.origin != 'source_assertion' AND ( \
                (a.subject_kind = 'catalog_set' AND a.generic_subject_snapshot_key IN (?, ?)) \
                OR (a.target_kind = 'catalog_set' AND a.generic_target_snapshot_key IN (?, ?)))) \
         ORDER BY r.review_id",
    )
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .bind::<Text, _>(previous.as_str())
    .bind::<Text, _>(current.as_str())
    .load::<ReviewRow>(conn)?;
    build_explanations(conn, rows, reviews)
}

pub fn explain_for_snapshots(
    conn: &mut SqliteConnection,
    left: &SnapshotKey,
    right: &SnapshotKey,
) -> crate::Result<Vec<RelationshipExplanation>> {
    let rows = sql_query(
        "SELECT a.assertion_key, a.relation_type, a.origin, \
                a.subject_snapshot_key, a.subject_kind, a.generic_subject_a, a.generic_subject_b, \
                a.generic_subject_c, a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                a.target_snapshot_key, a.target_kind, a.generic_target_a, a.generic_target_b, \
                a.generic_target_c, a.source_target_a, a.source_target_b, a.source_target_c, \
                a.source_snapshot_key, a.source_field, a.source_line, a.source_column, \
                source_asset.merge_name AS source_asset_merge_name, \
                source_asset.sha1 AS source_asset_sha1, source_asset.crc AS source_asset_crc, \
                source_asset.size AS source_asset_size, a.rule_version, \
                ps.source_key, ps.display_name AS source_name, \
                s.document_key, s.declared_version, pi.parser_name, pi.parser_version, pi.rules_version \
         FROM relationship_assertions a \
         LEFT JOIN asset_requirements source_asset \
           ON a.origin = 'source_assertion' AND a.subject_kind = 'asset_requirement' \
          AND source_asset.snapshot_key = a.source_snapshot_key \
          AND source_asset.set_name = a.source_subject_a \
          AND source_asset.component_order = a.source_subject_c \
         LEFT JOIN catalog_snapshots s ON s.snapshot_key = a.source_snapshot_key \
         LEFT JOIN catalogs c ON c.catalog_key = s.catalog_key \
         LEFT JOIN publishing_sources ps ON ps.source_key = c.source_key \
         LEFT JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         WHERE (a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?)) \
            OR (a.origin != 'source_assertion' AND ( \
                a.generic_subject_snapshot_key IN (?, ?) OR a.generic_target_snapshot_key IN (?, ?))) \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, \
                  a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                  a.generic_subject_a, a.generic_subject_b, a.generic_subject_c, \
                  a.target_kind, a.source_target_a, a.source_target_b, a.source_target_c, \
                  a.generic_target_a, a.generic_target_b, a.generic_target_c, a.assertion_key",
    )
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .load::<ExplanationRow>(conn)?;
    let reviews = sql_query(
        "SELECT r.assertion_key, r.decision, r.note, r.superseded_by_assertion_key, r.created_at \
         FROM relationship_reviews r \
         JOIN relationship_assertions a ON a.assertion_key = r.assertion_key \
         WHERE (a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?)) \
            OR (a.origin != 'source_assertion' AND ( \
                a.generic_subject_snapshot_key IN (?, ?) OR a.generic_target_snapshot_key IN (?, ?))) \
         ORDER BY r.review_id",
    )
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .bind::<Text, _>(left.as_str())
    .bind::<Text, _>(right.as_str())
    .load::<ReviewRow>(conn)?;
    build_explanations(conn, rows, reviews)
}

fn build_explanations(
    conn: &mut SqliteConnection,
    rows: Vec<ExplanationRow>,
    reviews: Vec<ReviewRow>,
) -> crate::Result<Vec<RelationshipExplanation>> {
    let evidence_rows = sql_query(
        "SELECT assertion_key, node_id, parent_node_id, object_key, array_index, value_type, \
                text_value, integer_value, unsigned_integer_value, real_value \
         FROM relationship_assertion_evidence ORDER BY assertion_key, node_id",
    )
    .load::<EvidenceNodeRow>(conn)?;
    let support_rows = sql_query(
        "SELECT assertion_key, supported_assertion_key \
         FROM relationship_assertion_support ORDER BY assertion_key, position",
    )
    .load::<SupportRow>(conn)?;
    let mut evidence_by_assertion = HashMap::<String, Vec<EvidenceNodeRow>>::new();
    for node in evidence_rows {
        evidence_by_assertion
            .entry(node.assertion_key.clone())
            .or_default()
            .push(node);
    }
    let mut support_by_assertion = HashMap::<String, Vec<String>>::new();
    for support in support_rows {
        support_by_assertion
            .entry(support.assertion_key)
            .or_default()
            .push(support.supported_assertion_key);
    }
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
            let evidence = if row.origin == "source_assertion" {
                None
            } else {
                Some(rebuild_evidence(
                    evidence_by_assertion
                        .remove(&row.assertion_key)
                        .ok_or_else(|| {
                            crate::Error::InvalidPath(
                                "relationship assertion has no typed evidence".into(),
                            )
                        })?,
                )?)
            };
            let supports = support_by_assertion
                .remove(&row.assertion_key)
                .unwrap_or_default();
            explanation(row, events, evidence, supports)
        })
        .collect()
}

fn rebuild_evidence(rows: Vec<EvidenceNodeRow>) -> crate::Result<serde_json::Value> {
    let mut nodes = HashMap::<i64, EvidenceNodeRow>::new();
    let mut children = HashMap::<i64, Vec<i64>>::new();
    let mut root = None;
    for row in rows {
        if let Some(parent) = row.parent_node_id {
            children.entry(parent).or_default().push(row.node_id);
        } else {
            root = Some(row.node_id);
        }
        nodes.insert(row.node_id, row);
    }
    let root =
        root.ok_or_else(|| crate::Error::InvalidPath("relationship evidence has no root".into()))?;
    let value = evidence_value(root, &nodes, &children)?;
    if !value.is_object() {
        return Err(crate::Error::InvalidPath(
            "relationship evidence root is not an object".into(),
        ));
    }
    Ok(value)
}

fn evidence_value(
    node_id: i64,
    nodes: &HashMap<i64, EvidenceNodeRow>,
    children: &HashMap<i64, Vec<i64>>,
) -> crate::Result<serde_json::Value> {
    let node = nodes
        .get(&node_id)
        .ok_or_else(|| crate::Error::InvalidPath("missing relationship evidence node".into()))?;
    let child_ids = children.get(&node_id).cloned().unwrap_or_default();
    match node.value_type.as_str() {
        "object" => {
            let mut object = serde_json::Map::new();
            for child_id in child_ids {
                let child = nodes.get(&child_id).ok_or_else(|| {
                    crate::Error::InvalidPath("missing relationship evidence child".into())
                })?;
                let key = child.object_key.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("object evidence child has no key".into())
                })?;
                object.insert(key, evidence_value(child_id, nodes, children)?);
            }
            Ok(serde_json::Value::Object(object))
        }
        "array" => {
            let mut indexed = child_ids
                .into_iter()
                .map(|id| {
                    let index = nodes
                        .get(&id)
                        .and_then(|child| child.array_index)
                        .ok_or_else(|| {
                            crate::Error::InvalidPath("array evidence child has no position".into())
                        })?;
                    Ok((index, id))
                })
                .collect::<crate::Result<Vec<_>>>()?;
            indexed.sort_by_key(|(index, _)| *index);
            indexed
                .into_iter()
                .map(|(_, id)| evidence_value(id, nodes, children))
                .collect::<crate::Result<Vec<_>>>()
                .map(serde_json::Value::Array)
        }
        "string" => Ok(serde_json::Value::String(
            node.text_value
                .clone()
                .ok_or_else(|| crate::Error::InvalidPath("string evidence has no value".into()))?,
        )),
        "integer" => {
            let number = node
                .integer_value
                .ok_or_else(|| crate::Error::InvalidPath("integer evidence has no value".into()))?;
            Ok(serde_json::Value::Number(serde_json::Number::from(number)))
        }
        "unsigned_integer" => {
            let number = node
                .unsigned_integer_value
                .as_deref()
                .ok_or_else(|| {
                    crate::Error::InvalidPath("unsigned integer evidence has no value".into())
                })?
                .parse::<serde_json::Number>()
                .map_err(|error| {
                    crate::Error::InvalidPath(format!("invalid unsigned integer evidence: {error}"))
                })?;
            Ok(serde_json::Value::Number(number))
        }
        "real" => serde_json::Number::from_f64(
            node.real_value
                .ok_or_else(|| crate::Error::InvalidPath("real evidence has no value".into()))?,
        )
        .map(serde_json::Value::Number)
        .ok_or_else(|| crate::Error::InvalidPath("invalid real evidence".into())),
        "true" => Ok(serde_json::Value::Bool(true)),
        "false" => Ok(serde_json::Value::Bool(false)),
        "null" => Ok(serde_json::Value::Null),
        other => Err(crate::Error::InvalidPath(format!(
            "unknown relationship evidence type {other}"
        ))),
    }
}

fn explanation(
    row: ExplanationRow,
    review_history: Vec<RelationshipReviewEvent>,
    generic_evidence: Option<serde_json::Value>,
    supporting_assertions: Vec<String>,
) -> crate::Result<RelationshipExplanation> {
    let relation_type = parse_relation_type(&row.relation_type)?;
    let is_source = row.origin == "source_assertion";
    let evidence = if is_source {
        source_evidence_value(&row)?
    } else {
        generic_evidence.ok_or_else(|| {
            crate::Error::InvalidPath("relationship assertion has no evidence".into())
        })?
    };
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
        "derived_candidate" => RelationshipOrigin::DerivedCandidate {
            rule_version: row.rule_version.clone().ok_or_else(|| {
                crate::Error::InvalidPath("candidate has no rule version".to_owned())
            })?,
            supporting_assertions: supporting_assertions
                .into_iter()
                .map(RelationshipAssertionKey::new)
                .collect(),
        },
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
            subject: if is_source {
                source_endpoint(
                    row.subject_snapshot_key,
                    &row.subject_kind,
                    row.source_subject_a,
                    row.source_subject_b.as_deref(),
                    row.source_subject_c,
                )?
            } else {
                typed_endpoint(
                    row.subject_snapshot_key,
                    &row.subject_kind,
                    row.generic_subject_a,
                    row.generic_subject_b,
                    row.generic_subject_c,
                )?
            },
            target: if is_source {
                source_endpoint(
                    row.target_snapshot_key,
                    &row.target_kind,
                    row.source_target_a,
                    row.source_target_b.as_deref(),
                    row.source_target_c,
                )?
            } else {
                typed_endpoint(
                    row.target_snapshot_key,
                    &row.target_kind,
                    row.generic_target_a,
                    row.generic_target_b,
                    row.generic_target_c,
                )?
            },
            origin,
            evidence,
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

fn source_evidence_value(row: &ExplanationRow) -> crate::Result<serde_json::Value> {
    let target_name = row
        .source_target_a
        .as_deref()
        .ok_or_else(|| crate::Error::InvalidPath("source evidence has no target name".into()))?;
    match row.source_field.as_deref() {
        Some("merge") => Ok(serde_json::json!({
            "declared_merge_name": row.source_asset_merge_name,
            "parent_set_name": target_name,
            "expected_sha1": row.source_asset_sha1.as_deref().map(hex::encode),
            "expected_crc": row.source_asset_crc.as_deref().map(hex::encode),
            "size": row.source_asset_size,
        })),
        Some("cloneof") if row.subject_kind == "software_item" => Ok(serde_json::json!({
            "list_name": row.source_subject_a,
            "target_item_name": row.source_target_b,
        })),
        Some("romof" | "sampleof") => Ok(serde_json::json!({
            "source_field": row.source_field.as_deref(),
            "target_name": target_name,
        })),
        Some("cloneof" | "parent_name" | "device_ref") => {
            Ok(serde_json::json!({"target_name": target_name}))
        }
        other => Err(crate::Error::InvalidPath(format!(
            "unsupported typed source evidence for field {other:?}"
        ))),
    }
}

fn source_endpoint(
    snapshot: Option<String>,
    kind: &str,
    first: Option<String>,
    second: Option<&str>,
    third: Option<i64>,
) -> crate::Result<RelationshipEndpoint> {
    let snapshot = SnapshotKey::from_persisted(snapshot.ok_or_else(|| {
        crate::Error::InvalidPath("source relationship endpoint has no snapshot".into())
    })?);
    let first = first.ok_or_else(|| {
        crate::Error::InvalidPath("source relationship endpoint has no first key part".into())
    })?;
    let (kind, key) = match kind {
        "catalog_set" => (CatalogRecordKind::Set, first),
        "software_item" => (
            CatalogRecordKind::SoftwareItem,
            serde_json::to_string(&(
                first.as_str(),
                second.ok_or_else(|| {
                    crate::Error::InvalidPath("software endpoint has no item key".into())
                })?,
            ))?,
        ),
        "asset_requirement" => (
            CatalogRecordKind::AssetRequirement,
            serde_json::to_string(&(
                first.as_str(),
                second.ok_or_else(|| {
                    crate::Error::InvalidPath("asset endpoint has no asset key".into())
                })?,
                third.ok_or_else(|| {
                    crate::Error::InvalidPath("asset endpoint has no component order".into())
                })?,
            ))?,
        ),
        other => {
            return Err(crate::Error::InvalidPath(format!(
                "unknown source relationship endpoint kind {other}"
            )));
        }
    };
    Ok(RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
        snapshot, kind, key,
    )))
}

fn typed_endpoint(
    snapshot: Option<String>,
    kind: &str,
    first: Option<String>,
    second: Option<String>,
    third: Option<i64>,
) -> crate::Result<RelationshipEndpoint> {
    let first = first.ok_or_else(|| {
        crate::Error::InvalidPath("relationship endpoint has no first component".into())
    })?;
    match kind {
        "catalog_set" | "software_item" | "asset_requirement" => {
            let snapshot = SnapshotKey::from_persisted(snapshot.ok_or_else(|| {
                crate::Error::InvalidPath("catalog relationship endpoint has no snapshot".into())
            })?);
            let record_kind = match kind {
                "catalog_set" => CatalogRecordKind::Set,
                "software_item" => CatalogRecordKind::SoftwareItem,
                _ => CatalogRecordKind::AssetRequirement,
            };
            let key = match record_kind {
                CatalogRecordKind::Set => first,
                CatalogRecordKind::SoftwareItem => serde_json::to_string(&(
                    first.as_str(),
                    second.ok_or_else(|| {
                        crate::Error::InvalidPath(
                            "software endpoint has no second component".into(),
                        )
                    })?,
                ))?,
                CatalogRecordKind::AssetRequirement => serde_json::to_string(&(
                    first.as_str(),
                    second.ok_or_else(|| {
                        crate::Error::InvalidPath("asset endpoint has no second component".into())
                    })?,
                    third.ok_or_else(|| {
                        crate::Error::InvalidPath("asset endpoint has no order".into())
                    })?,
                ))?,
            };
            Ok(RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
                snapshot,
                record_kind,
                key,
            )))
        }
        "content_object" => {
            let algorithm = match first.as_str() {
                "crc32" => ContentDigestAlgorithm::Crc32,
                "md5" => ContentDigestAlgorithm::Md5,
                "sha1" => ContentDigestAlgorithm::Sha1,
                "sha256" => ContentDigestAlgorithm::Sha256,
                _ => return Err(crate::Error::InvalidPath("unknown digest algorithm".into())),
            };
            let digest = second
                .ok_or_else(|| crate::Error::InvalidPath("content object has no digest".into()))?;
            Ok(RelationshipEndpoint::ContentObject(ContentIdentity::new(
                algorithm, digest,
            )?))
        }
        "external_record" => Ok(RelationshipEndpoint::ExternalRecord(
            ExternalRecordRef::new(
                first,
                second.ok_or_else(|| {
                    crate::Error::InvalidPath("external record has no key".into())
                })?,
            ),
        )),
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
