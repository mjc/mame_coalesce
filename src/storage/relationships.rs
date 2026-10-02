use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};
use std::collections::HashMap;

use crate::domain::{
    CatalogRecordKind, CatalogRecordRef, CatalogSetId, ContentDigestAlgorithm, ContentIdentity,
    DocumentLocation, ExternalRecordRef, MergeMediaKind, RelationshipAssertionKey,
    RelationshipClaim, RelationshipEndpoint, RelationshipEvidence, RelationshipExplanation,
    RelationshipOrigin, RelationshipReview, RelationshipReviewDecision, RelationshipReviewEvent,
    RelationshipSourceProvenance, RelationshipType, SnapshotKey,
};

use super::db::Pool;

mod evidence;
mod publication;

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
    #[diesel(sql_type = Nullable<BigInt>)]
    subject_set_id: Option<i64>,
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
    #[diesel(sql_type = Nullable<BigInt>)]
    target_set_id: Option<i64>,
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
struct SupportRow {
    #[diesel(sql_type = Text)]
    supported_assertion_key: String,
}

const SUPPORT_SQL: &str = "SELECT supported_assertion_key \
     FROM relationship_assertion_support WHERE assertion_key=? ORDER BY position";

#[derive(QueryableByName)]
struct OwnerValidationRow {
    #[diesel(sql_type = Bool)]
    is_valid: bool,
}

type SourceEndpointParts = (
    &'static str,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<CatalogSetId>,
);
type EndpointParts = (
    Option<String>,
    &'static str,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<CatalogSetId>,
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
    pub evidence: RelationshipEvidence,
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
    validate_endpoint_owner(conn, &claim.subject)?;
    validate_endpoint_owner(conn, &claim.target)?;
    let (subject_kind, subject_a, subject_b, subject_c, subject_set_id) =
        source_endpoint_parts(&claim.subject)?;
    let (target_kind, target_a, target_b, target_c, target_set_id) =
        source_endpoint_parts(&claim.target)?;
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
          source_column, subject_kind, subject_set_id, source_subject_a, source_subject_b, \
          source_subject_c, target_kind, target_set_id, source_target_a, source_target_b, \
          source_target_c) \
         VALUES (?, ?, 'source_assertion', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key.as_str())
    .bind::<Text, _>(claim.relation_type.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(field)
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.line))
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.column))
    .bind::<Text, _>(subject_kind)
    .bind::<Nullable<BigInt>, _>(subject_set_id.map(CatalogSetId::as_i64))
    .bind::<Nullable<Text>, _>(subject_a)
    .bind::<Nullable<Text>, _>(subject_b)
    .bind::<Nullable<BigInt>, _>(subject_c)
    .bind::<Text, _>(target_kind)
    .bind::<Nullable<BigInt>, _>(target_set_id.map(CatalogSetId::as_i64))
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
    Ok((
        record.kind.as_str(),
        Some(first),
        second,
        third,
        record.owner_set_id,
    ))
}

fn validate_source_evidence(
    claim: &RelationshipClaim,
    subject_kind: &str,
    subject_a: Option<&str>,
    target_kind: &str,
    target_a: Option<&str>,
    target_b: Option<&str>,
) -> crate::Result<()> {
    let RelationshipOrigin::SourceAssertion { field, .. } = &claim.origin else {
        return Err(crate::Error::InvalidPath(
            "source evidence requires source provenance".into(),
        ));
    };
    let matches = match (field.as_str(), &claim.evidence) {
        (
            "merge",
            RelationshipEvidence::Merge {
                declared_merge_name,
                expected_sha1,
                expected_crc,
                ..
            },
        ) => {
            for (encoded, length) in [(expected_sha1, 20), (expected_crc, 4)] {
                if encoded.as_deref().is_some_and(|value| {
                    !hex::decode(value).is_ok_and(|bytes| bytes.len() == length)
                }) {
                    return Err(crate::Error::InvalidPath(
                        "source merge evidence has an invalid digest".into(),
                    ));
                }
            }
            subject_kind == "asset_requirement"
                && target_kind == "asset_requirement"
                && declared_merge_name.as_deref() == target_b
        }
        (
            "cloneof",
            RelationshipEvidence::SoftwareClone {
                list_name,
                target_item_name,
            },
        ) if subject_kind == "software_item" => {
            Some(list_name.as_str()) == subject_a && Some(target_item_name.as_str()) == target_b
        }
        (
            "cloneof" | "parent_name" | "device_ref",
            RelationshipEvidence::SourceReference { target_name },
        ) => Some(target_name.as_str()) == target_a,
        (
            "romof" | "sampleof" | "device_ref",
            RelationshipEvidence::SourceFieldReference {
                source_field,
                target_name,
            },
        ) => source_field == field && Some(target_name.as_str()) == target_a,
        _ => false,
    };
    if !matches {
        return Err(crate::Error::InvalidPath(format!(
            "source evidence does not match relationship field {field}"
        )));
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
                if !publication::is_published(conn, supported)? {
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

fn validate_claim(claim: &RelationshipClaim) -> crate::Result<()> {
    match (&claim.origin, &claim.evidence) {
        (
            RelationshipOrigin::SourceAssertion { .. },
            RelationshipEvidence::Rationale { .. } | RelationshipEvidence::CatalogComparison { .. },
        ) => {
            return Err(crate::Error::InvalidPath(
                "source evidence must come from native catalog facts".into(),
            ));
        }
        (_, RelationshipEvidence::Rationale { reason }) if reason.trim().is_empty() => {
            return Err(crate::Error::InvalidPath(
                "relationship rationale must not be empty".into(),
            ));
        }
        (
            RelationshipOrigin::DerivedCandidate { .. },
            RelationshipEvidence::CatalogComparison { .. },
        )
        | (_, RelationshipEvidence::Rationale { .. })
        | (RelationshipOrigin::SourceAssertion { .. }, _) => {}
        _ => {
            return Err(crate::Error::InvalidPath(
                "evidence subtype does not match relationship origin".into(),
            ));
        }
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

fn validate_endpoint_owner(
    conn: &mut SqliteConnection,
    endpoint: &RelationshipEndpoint,
) -> crate::Result<()> {
    let RelationshipEndpoint::CatalogRecord(record) = endpoint else {
        return Ok(());
    };
    let Some(owner_set_id) = record.owner_set_id else {
        return Ok(());
    };
    if owner_set_id.as_i64() <= 0 {
        return Err(crate::Error::InvalidPath(
            "catalog set owner ID must be positive".to_owned(),
        ));
    }
    let (_, first, second, _, _) = source_endpoint_parts(endpoint)?;
    let first = first.ok_or_else(|| {
        crate::Error::InvalidPath("catalog record endpoint has no first key part".into())
    })?;
    let (set_name, software_list_name) = match record.kind {
        CatalogRecordKind::Set | CatalogRecordKind::AssetRequirement => (first, None),
        CatalogRecordKind::SoftwareItem => {
            let item_name = second.ok_or_else(|| {
                crate::Error::InvalidPath("software item endpoint has no item key".into())
            })?;
            (item_name, Some(first))
        }
    };
    let row = sql_query(
        "SELECT EXISTS ( \
             SELECT 1 FROM catalog_sets AS owner \
             JOIN catalog_set_groups AS groups USING (set_group_id) \
             LEFT JOIN software_lists AS software_list ON software_list.namespace_id = groups.set_group_id \
             WHERE owner.set_id = ? AND groups.snapshot_key = ? \
               AND ( \
                 (? IN ('catalog_set', 'asset_requirement') AND groups.kind = 'root' \
                  AND owner.set_name = ?) \
                 OR (? = 'software_item' AND groups.kind = 'software_list' \
                     AND software_list.name = ? AND owner.set_name = ?) \
               ) \
         ) AS is_valid",
    )
    .bind::<BigInt, _>(owner_set_id.as_i64())
    .bind::<Text, _>(record.snapshot.as_str())
    .bind::<Text, _>(record.kind.as_str())
    .bind::<Text, _>(&set_name)
    .bind::<Text, _>(record.kind.as_str())
    .bind::<Nullable<Text>, _>(software_list_name.as_deref())
    .bind::<Text, _>(&set_name)
    .get_result::<OwnerValidationRow>(conn)?;
    if !row.is_valid {
        return Err(crate::Error::InvalidPath(format!(
            "catalog set owner {} does not match endpoint {:?} in snapshot {}",
            owner_set_id.as_i64(),
            record.kind,
            record.snapshot.as_str()
        )));
    }
    Ok(())
}

fn endpoint_parts(endpoint: &RelationshipEndpoint) -> crate::Result<EndpointParts> {
    match endpoint {
        RelationshipEndpoint::CatalogRecord(record) => {
            let (kind, first, second, third, owner_set_id) = source_endpoint_parts(endpoint)?;
            Ok((
                Some(record.snapshot.as_str().to_owned()),
                kind,
                first,
                second,
                third,
                owner_set_id,
            ))
        }
        RelationshipEndpoint::ContentObject(identity) => Ok((
            None,
            "content_object",
            Some(identity.algorithm().as_str().to_owned()),
            Some(identity.digest().to_owned()),
            None,
            None,
        )),
        RelationshipEndpoint::ExternalRecord(record) => Ok((
            None,
            "external_record",
            Some(record.namespace.clone()),
            Some(record.key.as_str().to_owned()),
            None,
            None,
        )),
        RelationshipEndpoint::NoIntroArchive { .. }
        | RelationshipEndpoint::NoIntroArchiveReference { .. }
        | RelationshipEndpoint::NoIntroDatIdReference { .. }
        | RelationshipEndpoint::CatalogMediaEntry { .. }
        | RelationshipEndpoint::CatalogMergeReference { .. } => Err(crate::Error::InvalidPath(
            "native endpoints require a native source assertion".into(),
        )),
    }
}

fn insert_claim(
    conn: &mut SqliteConnection,
    assertion_key: &RelationshipAssertionKey,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    validate_claim(claim)?;
    validate_endpoint_owner(conn, &claim.subject)?;
    validate_endpoint_owner(conn, &claim.target)?;
    let (subject_snapshot, subject_kind, subject_a, subject_b, subject_c, subject_set_id) =
        endpoint_parts(&claim.subject)?;
    let (target_snapshot, target_kind, target_a, target_b, target_c, target_set_id) =
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
          subject_set_id, generic_subject_a, generic_subject_b, generic_subject_c, \
          generic_target_snapshot_key, target_kind, target_set_id, generic_target_a, \
          generic_target_b, generic_target_c, source_snapshot_key, source_field, source_line, \
          source_column, rule_version) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind::<Text, _>(assertion_key.as_str())
    .bind::<Text, _>(claim.relation_type.as_str())
    .bind::<Text, _>(origin)
    .bind::<Nullable<Text>, _>(subject_snapshot)
    .bind::<Text, _>(subject_kind)
    .bind::<Nullable<BigInt>, _>(subject_set_id.map(CatalogSetId::as_i64))
    .bind::<Nullable<Text>, _>(subject_a)
    .bind::<Nullable<Text>, _>(subject_b)
    .bind::<Nullable<BigInt>, _>(subject_c)
    .bind::<Nullable<Text>, _>(target_snapshot)
    .bind::<Text, _>(target_kind)
    .bind::<Nullable<BigInt>, _>(target_set_id.map(CatalogSetId::as_i64))
    .bind::<Nullable<Text>, _>(target_a)
    .bind::<Nullable<Text>, _>(target_b)
    .bind::<Nullable<BigInt>, _>(target_c)
    .bind::<Nullable<Text>, _>(source_snapshot)
    .bind::<Nullable<Text>, _>(source_field)
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.line))
    .bind::<Nullable<BigInt>, _>(location.map(|value| value.column))
    .bind::<Nullable<Text>, _>(rule_version)
    .execute(conn)?;
    evidence::insert(conn, assertion_key.as_str(), &claim.evidence)?;
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
    evidence::publish(conn, assertion_key.as_str(), &claim.evidence)?;
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

#[derive(Clone, Copy)]
enum ExplanationScope {
    All,
    CatalogSets,
    Snapshots,
}

impl ExplanationScope {
    const fn scoped_assertions_cte(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::CatalogSets => Some(
                "WITH scoped_source_assertions AS NOT MATERIALIZED ( \
                   SELECT a.* FROM relationship_assertion_explanations a \
                   WHERE a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?) \
                     AND (a.subject_kind IN ('catalog_set', 'software_item') \
                          OR a.target_kind IN ('catalog_set', 'software_item')) \
                 ), scoped_generic_assertions AS NOT MATERIALIZED ( \
                   SELECT a.* FROM stored_relationship_assertion_explanations a \
                   WHERE a.origin != 'source_assertion' \
                     AND a.subject_kind IN ('catalog_set', 'software_item') \
                     AND a.generic_subject_snapshot_key IN (?, ?) \
                   UNION \
                   SELECT a.* FROM stored_relationship_assertion_explanations a \
                   WHERE a.origin != 'source_assertion' \
                     AND a.target_kind IN ('catalog_set', 'software_item') \
                     AND a.generic_target_snapshot_key IN (?, ?) \
                 ), scoped_assertions AS MATERIALIZED ( \
                   SELECT * FROM scoped_source_assertions \
                   UNION ALL \
                   SELECT * FROM scoped_generic_assertions \
                 )",
            ),
            Self::Snapshots => Some(
                "WITH scoped_source_assertions AS NOT MATERIALIZED ( \
                   SELECT a.* FROM relationship_assertion_explanations a \
                   WHERE a.origin = 'source_assertion' AND a.source_snapshot_key IN (?, ?) \
                 ), scoped_generic_assertions AS NOT MATERIALIZED ( \
                   SELECT a.* FROM stored_relationship_assertion_explanations a \
                   WHERE a.origin != 'source_assertion' \
                     AND a.generic_subject_snapshot_key IN (?, ?) \
                   UNION \
                   SELECT a.* FROM stored_relationship_assertion_explanations a \
                   WHERE a.origin != 'source_assertion' \
                     AND a.generic_target_snapshot_key IN (?, ?) \
                 ), scoped_assertions AS MATERIALIZED ( \
                   SELECT * FROM scoped_source_assertions \
                   UNION ALL \
                   SELECT * FROM scoped_generic_assertions \
                 )",
            ),
        }
    }

    fn query(self) -> String {
        let cte = self.scoped_assertions_cte().unwrap_or("");
        let from = if self.scoped_assertions_cte().is_some() {
            "scoped_assertions a"
        } else {
            "relationship_assertion_explanations a"
        };
        let merge_name = "COALESCE(mame_rom_merge.merge_name,mame_disk_merge.merge_name,logiqx_merge.merge_name,cmp_merge.merge_name,pc_file.merge_name)";
        let occurrence_id =
            "COALESCE(native_source_occurrence.occurrence_id,source_occurrence.occurrence_id)";
        let size = "COALESCE(mame_rom.size,logiqx_rom.size,cmp_rom.size,pc_file.size,dat_rom.size)";
        let scope = "COALESCE(mame_rom.evidence_scope,mame_disk.evidence_scope,logiqx_rom.evidence_scope,logiqx_disk.evidence_scope,cmp_rom.evidence_scope,pc_file.evidence_scope,dat_rom.evidence_scope)";
        format!(
            "{cte} SELECT a.assertion_key, a.relation_type, a.origin, \
                a.subject_snapshot_key, a.subject_kind, a.subject_set_id, \
                a.generic_subject_a, a.generic_subject_b, \
                a.generic_subject_c, a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                a.target_snapshot_key, a.target_kind, a.target_set_id, \
                a.generic_target_a, a.generic_target_b, \
                a.generic_target_c, a.source_target_a, a.source_target_b, a.source_target_c, \
                a.source_snapshot_key, a.source_field, a.source_line, a.source_column, \
                {merge_name} AS source_asset_merge_name, \
                (SELECT assertion.digest FROM usable_occurrence_digest_assertions AS assertion \
                 WHERE assertion.occurrence_id = {occurrence_id} \
                   AND assertion.provenance = 'source_declared' AND assertion.scope = {scope} \
                   AND assertion.algorithm = 'sha1') AS source_asset_sha1, \
                (SELECT assertion.digest FROM usable_occurrence_digest_assertions AS assertion \
                 WHERE assertion.occurrence_id = {occurrence_id} \
                   AND assertion.provenance = 'source_declared' AND assertion.scope = {scope} \
                   AND assertion.algorithm = 'crc32') AS source_asset_crc, \
                {size} AS source_asset_size, a.rule_version, \
                ps.source_key, ps.display_name AS source_name, \
                s.document_key, \
                (SELECT source_version.declared_version FROM catalog_snapshot_versions source_version \
                 WHERE source_version.snapshot_key = a.source_snapshot_key) AS declared_version, \
                pi.parser_name, pi.parser_version, pi.rules_version \
         FROM {from} \
         LEFT JOIN catalog_set_groups source_asset_group \
           ON a.origin = 'source_assertion' AND a.subject_kind = 'asset_requirement' \
          AND source_asset_group.snapshot_key = a.source_snapshot_key \
          AND source_asset_group.kind = 'root' \
         LEFT JOIN catalog_sets source_asset_set \
           ON source_asset_set.set_group_id = source_asset_group.set_group_id \
          AND source_asset_set.set_name = a.source_subject_a \
          AND (a.subject_set_id IS NULL OR source_asset_set.set_id = a.subject_set_id) \
         LEFT JOIN asset_occurrences source_occurrence \
           ON source_occurrence.record_id = source_asset_set.set_id \
          AND source_occurrence.occurrence_order = a.source_subject_c \
         LEFT JOIN asset_occurrences native_source_occurrence \
           ON a.origin='source_assertion' AND a.subject_kind='catalog_media_entry' \
          AND native_source_occurrence.occurrence_id=a.source_subject_c \
          AND native_source_occurrence.record_id=a.subject_set_id \
         LEFT JOIN mame_rom_claims mame_rom ON mame_rom.occurrence_id={occurrence_id} \
         LEFT JOIN mame_disk_claims mame_disk ON mame_disk.occurrence_id={occurrence_id} \
         LEFT JOIN mame_rom_merges mame_rom_merge ON mame_rom_merge.occurrence_id={occurrence_id} \
         LEFT JOIN mame_disk_merges mame_disk_merge ON mame_disk_merge.occurrence_id={occurrence_id} \
         LEFT JOIN logiqx_rom_claims logiqx_rom ON logiqx_rom.occurrence_id={occurrence_id} \
         LEFT JOIN logiqx_disk_claims logiqx_disk ON logiqx_disk.occurrence_id={occurrence_id} \
         LEFT JOIN logiqx_file_merges logiqx_merge ON logiqx_merge.occurrence_id={occurrence_id} \
         LEFT JOIN cmp_rom_claims cmp_rom ON cmp_rom.occurrence_id={occurrence_id} \
         LEFT JOIN clrmamepro_rom_merges cmp_merge ON cmp_merge.occurrence_id={occurrence_id} \
         LEFT JOIN no_intro_pc_file_claims pc_file ON pc_file.occurrence_id={occurrence_id} \
         LEFT JOIN no_intro_dat_rom_claims dat_rom ON dat_rom.occurrence_id={occurrence_id} \
         LEFT JOIN catalog_snapshots s ON s.snapshot_key = a.source_snapshot_key \
         LEFT JOIN catalogs c ON c.catalog_key = s.catalog_key \
         LEFT JOIN publishing_sources ps ON ps.source_key = c.source_key \
         LEFT JOIN parser_interpretations pi ON pi.interpretation_key = s.interpretation_key \
         WHERE (a.origin='source_assertion' AND EXISTS ( \
                    SELECT 1 FROM snapshot_publications AS ready WHERE ready.snapshot_key=a.source_snapshot_key)) \
            OR (a.origin<>'source_assertion' AND EXISTS ( \
                    SELECT 1 FROM relationship_evidence_publications AS ready WHERE ready.assertion_key=a.assertion_key)) \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, \
                  a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                  a.generic_subject_a, a.generic_subject_b, a.generic_subject_c, \
                  a.target_kind, a.source_target_a, a.source_target_b, a.source_target_c, \
                  a.generic_target_a, a.generic_target_b, a.generic_target_c, a.assertion_key"
        )
    }

    fn review_query(self) -> String {
        self.scoped_assertions_cte().map_or_else(
            || {
                "SELECT assertion_key, decision, note, superseded_by_assertion_key, created_at \
                 FROM relationship_reviews ORDER BY review_id"
                    .to_owned()
            },
            |cte| {
                format!(
                    "{cte} SELECT r.assertion_key, r.decision, r.note, \
                        r.superseded_by_assertion_key, r.created_at \
                 FROM scoped_assertions scoped \
                 JOIN relationship_reviews r USING (assertion_key) \
                 ORDER BY r.review_id"
                )
            },
        )
    }
}

pub fn explain_all(pool: &Pool) -> crate::Result<Vec<RelationshipExplanation>> {
    let mut conn = pool.get()?;
    let rows = sql_query(ExplanationScope::All.query()).load::<ExplanationRow>(&mut conn)?;
    let reviews = sql_query(ExplanationScope::All.review_query()).load::<ReviewRow>(&mut conn)?;
    build_explanations(&mut conn, rows, reviews)
}

pub fn explain_catalog_sets_for_snapshots(
    conn: &mut SqliteConnection,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<Vec<RelationshipExplanation>> {
    let rows = sql_query(ExplanationScope::CatalogSets.query())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .load::<ExplanationRow>(conn)?;
    let reviews = sql_query(ExplanationScope::CatalogSets.review_query())
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
    let rows = sql_query(ExplanationScope::Snapshots.query())
        .bind::<Text, _>(left.as_str())
        .bind::<Text, _>(right.as_str())
        .bind::<Text, _>(left.as_str())
        .bind::<Text, _>(right.as_str())
        .bind::<Text, _>(left.as_str())
        .bind::<Text, _>(right.as_str())
        .load::<ExplanationRow>(conn)?;
    let reviews = sql_query(ExplanationScope::Snapshots.review_query())
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
    let mut support_by_assertion = HashMap::<String, Vec<String>>::new();
    // Only derived candidates may own support edges; source-only catalogs need
    // no per-declaration support lookup at all.
    for row in rows.iter().filter(|row| row.origin == "derived_candidate") {
        if support_by_assertion.contains_key(&row.assertion_key) {
            continue;
        }
        let supports = sql_query(SUPPORT_SQL)
            .bind::<Text, _>(&row.assertion_key)
            .load::<SupportRow>(conn)?
            .into_iter()
            .map(|support| support.supported_assertion_key)
            .collect();
        support_by_assertion.insert(row.assertion_key.clone(), supports);
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
                Some(evidence::load(conn, &row.assertion_key)?)
            };
            let supports = support_by_assertion
                .remove(&row.assertion_key)
                .unwrap_or_default();
            explanation(row, events, evidence, supports)
        })
        .collect()
}

fn explanation(
    row: ExplanationRow,
    review_history: Vec<RelationshipReviewEvent>,
    generic_evidence: Option<RelationshipEvidence>,
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
    let origin = explanation_origin(&row, supporting_assertions)?;
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
                    row.subject_set_id,
                    row.source_subject_a,
                    row.source_subject_b.as_deref(),
                    row.source_subject_c,
                )?
            } else {
                typed_endpoint(
                    row.subject_snapshot_key,
                    &row.subject_kind,
                    row.subject_set_id,
                    row.generic_subject_a,
                    row.generic_subject_b,
                    row.generic_subject_c,
                )?
            },
            target: if is_source {
                source_endpoint(
                    row.target_snapshot_key,
                    &row.target_kind,
                    row.target_set_id,
                    row.source_target_a,
                    row.source_target_b.as_deref(),
                    row.source_target_c,
                )?
            } else {
                typed_endpoint(
                    row.target_snapshot_key,
                    &row.target_kind,
                    row.target_set_id,
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

fn explanation_origin(
    row: &ExplanationRow,
    supporting_assertions: Vec<String>,
) -> crate::Result<RelationshipOrigin> {
    match row.origin.as_str() {
        "source_assertion" => Ok(RelationshipOrigin::SourceAssertion {
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
        }),
        "derived_candidate" => Ok(RelationshipOrigin::DerivedCandidate {
            rule_version: row.rule_version.clone().ok_or_else(|| {
                crate::Error::InvalidPath("candidate has no rule version".to_owned())
            })?,
            supporting_assertions: supporting_assertions
                .into_iter()
                .map(RelationshipAssertionKey::new)
                .collect(),
        }),
        "user_conclusion" => Ok(RelationshipOrigin::UserConclusion),
        origin => Err(crate::Error::InvalidPath(format!(
            "unknown relationship origin {origin}"
        ))),
    }
}

fn source_evidence_value(row: &ExplanationRow) -> crate::Result<RelationshipEvidence> {
    if row.source_field.as_deref() == Some("merge") {
        return Ok(RelationshipEvidence::Merge {
            declared_merge_name: row.source_asset_merge_name.clone(),
            parent_set_name: row.source_target_a.clone(),
            expected_sha1: row.source_asset_sha1.as_deref().map(hex::encode),
            expected_crc: row.source_asset_crc.as_deref().map(hex::encode),
            size: row.source_asset_size,
        });
    }
    let target_name = row
        .source_target_a
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidPath("source evidence has no target name".into()))?;
    match row.source_field.as_deref() {
        Some("archive_clone" | "archive_mergeof") => Ok(RelationshipEvidence::ArchiveReference {
            declared_archive_reference: target_name.clone(),
            source_field: row.source_field.clone().ok_or_else(|| {
                crate::Error::InvalidPath("archive source evidence has no field".into())
            })?,
        }),
        Some("cloneof") if row.subject_kind == "software_item" => {
            Ok(RelationshipEvidence::SoftwareClone {
                list_name: row.source_subject_a.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("software clone has no list".into())
                })?,
                target_item_name: row.source_target_b.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("software clone has no target".into())
                })?,
            })
        }
        Some("romof" | "sampleof" | "cloneofid") => {
            Ok(RelationshipEvidence::SourceFieldReference {
                source_field: row.source_field.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("source evidence has no field".into())
                })?,
                target_name: target_name.clone(),
            })
        }
        Some("cloneof" | "parent_name" | "device_ref") => {
            Ok(RelationshipEvidence::SourceReference {
                target_name: target_name.clone(),
            })
        }
        other => Err(crate::Error::InvalidPath(format!(
            "unsupported typed source evidence for field {other:?}"
        ))),
    }
}

fn source_endpoint(
    snapshot: Option<String>,
    kind: &str,
    owner_set_id: Option<i64>,
    first: Option<String>,
    second: Option<&str>,
    third: Option<i64>,
) -> crate::Result<RelationshipEndpoint> {
    let snapshot = SnapshotKey::from_persisted(snapshot.ok_or_else(|| {
        crate::Error::InvalidPath("source relationship endpoint has no snapshot".into())
    })?);
    if kind == "catalog_media_entry" {
        let id = third.ok_or_else(|| {
            crate::Error::InvalidPath("media endpoint has no native owner".into())
        })?;
        return Ok(RelationshipEndpoint::CatalogMediaEntry {
            snapshot,
            occurrence_id: id.try_into()?,
        });
    }
    if matches!(
        kind,
        "catalog_rom_merge_reference" | "catalog_disk_merge_reference"
    ) {
        let set_id = owner_set_id.ok_or_else(|| {
            crate::Error::InvalidPath("merge reference has no declaring set".into())
        })?;
        return Ok(RelationshipEndpoint::CatalogMergeReference {
            snapshot,
            set_id: CatalogSetId::from_database(set_id),
            media_kind: if kind == "catalog_rom_merge_reference" {
                MergeMediaKind::Rom
            } else {
                MergeMediaKind::Disk
            },
            parent_name: first,
            merge_name: second
                .ok_or_else(|| {
                    crate::Error::InvalidPath("merge reference has no declared literal".into())
                })?
                .to_owned(),
        });
    }
    if kind == "no_intro_archive" {
        let id = third.ok_or_else(|| {
            crate::Error::InvalidPath("archive endpoint has no native owner".into())
        })?;
        return Ok(RelationshipEndpoint::NoIntroArchive {
            snapshot,
            archive_id: id.try_into()?,
        });
    }
    if kind == "no_intro_archive_reference" {
        let literal = first.ok_or_else(|| {
            crate::Error::InvalidPath("archive reference has no native declaration".into())
        })?;
        return Ok(RelationshipEndpoint::NoIntroArchiveReference { snapshot, literal });
    }
    if kind == "no_intro_dat_id_reference" {
        return Ok(RelationshipEndpoint::NoIntroDatIdReference {
            snapshot,
            declaring_set: owner_set_id
                .ok_or_else(|| {
                    crate::Error::InvalidPath("DAT ID reference has no declaring set".into())
                })?
                .try_into()?,
            declared_id: first.ok_or_else(|| {
                crate::Error::InvalidPath("DAT ID reference has no publisher ID".into())
            })?,
        });
    }
    let first = first.ok_or_else(|| {
        crate::Error::InvalidPath("source relationship endpoint has no first key part".into())
    })?;
    let record_kind = match kind {
        "catalog_set" => CatalogRecordKind::Set,
        "software_item" => CatalogRecordKind::SoftwareItem,
        "asset_requirement" => CatalogRecordKind::AssetRequirement,
        other => {
            return Err(crate::Error::InvalidPath(format!(
                "unknown source relationship endpoint kind {other}"
            )));
        }
    };
    catalog_record_endpoint(snapshot, record_kind, owner_set_id, first, second, third)
}

fn catalog_record_endpoint(
    snapshot: SnapshotKey,
    kind: CatalogRecordKind,
    owner_set_id: Option<i64>,
    first: String,
    second: Option<&str>,
    third: Option<i64>,
) -> crate::Result<RelationshipEndpoint> {
    let key = match kind {
        CatalogRecordKind::Set => first,
        CatalogRecordKind::SoftwareItem => serde_json::to_string(&(
            first.as_str(),
            second.ok_or_else(|| {
                crate::Error::InvalidPath("software endpoint has no item key".into())
            })?,
        ))?,
        CatalogRecordKind::AssetRequirement => serde_json::to_string(&(
            first.as_str(),
            second.ok_or_else(|| {
                crate::Error::InvalidPath("asset endpoint has no asset key".into())
            })?,
            third.ok_or_else(|| {
                crate::Error::InvalidPath("asset endpoint has no component order".into())
            })?,
        ))?,
    };
    let record = CatalogRecordRef::new(snapshot, kind, key);
    let record = match owner_set_id {
        Some(id) => record.with_owner(CatalogSetId::from_database(id)),
        None => record,
    };
    Ok(RelationshipEndpoint::CatalogRecord(record))
}

fn typed_endpoint(
    snapshot: Option<String>,
    kind: &str,
    owner_set_id: Option<i64>,
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
            catalog_record_endpoint(
                snapshot,
                record_kind,
                owner_set_id,
                first,
                second.as_deref(),
                third,
            )
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
        "source_merge" => Ok(RelationshipType::SourceMerge),
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

#[cfg(test)]
mod query_plan_tests {
    use super::*;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    #[test]
    fn scoped_explanations_do_not_scan_unrelated_snapshot_versions() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let mut conn = database.pool().get()?;
        let mut scoped_plans = Vec::new();
        for scope in [ExplanationScope::CatalogSets, ExplanationScope::Snapshots] {
            let plan = sql_query(format!("EXPLAIN QUERY PLAN {}", scope.query()))
                .bind::<Text, _>("previous")
                .bind::<Text, _>("current")
                .bind::<Text, _>("previous")
                .bind::<Text, _>("current")
                .bind::<Text, _>("previous")
                .bind::<Text, _>("current")
                .load::<PlanRow>(&mut conn)?;
            let details = plan.into_iter().map(|row| row.detail).collect::<Vec<_>>();
            let name = match scope {
                ExplanationScope::CatalogSets => "catalog sets",
                ExplanationScope::Snapshots => "snapshots",
                ExplanationScope::All => unreachable!(),
            };
            scoped_plans.push((name, details));
        }
        let scoped_indexes = [
            "relationship_assertions_source_snapshot_index",
            "relationship_assertions_subject_snapshot_kind_index",
            "relationship_assertions_target_snapshot_kind_index",
        ];
        let violations = scoped_plans
            .into_iter()
            .filter(|(_, details)| {
                details.iter().any(|detail| {
                    detail == "SCAN snapshot"
                        || detail.contains("MATERIALIZE catalog_snapshot_versions")
                        || detail.contains("MATERIALIZE snapshot_sets")
                        || detail == "SCAN groups"
                        || detail.contains("MATERIALIZE asset_requirement_rows")
                        || detail.starts_with("SCAN payload")
                        || detail == "MATERIALIZE scoped_source_assertions"
                        || detail == "MATERIALIZE scoped_generic_assertions"
                        || detail.starts_with("SCAN relationship_assertions")
                        || detail.starts_with("SCAN position")
                }) || scoped_indexes.iter().any(|index| {
                    !details.iter().any(|detail| {
                        detail.starts_with("SEARCH relationship_assertions USING INDEX ")
                            && detail.contains(index)
                    })
                }) || !details
                    .iter()
                    .any(|detail| detail.starts_with("SEARCH position USING PRIMARY KEY"))
                    || !details
                        .iter()
                        .any(|detail| detail.starts_with("SEARCH source_asset_set"))
                    || details
                        .iter()
                        .any(|detail| detail.starts_with("SCAN native_source_occurrence"))
                    || !details.iter().any(|detail| {
                        detail.starts_with("SEARCH native_source_occurrence USING")
                            && (detail.contains("PRIMARY KEY")
                                || (detail.contains("occurrence_id=?")
                                    && detail.contains("record_id=?")))
                    })
                    || [
                        "mame_rom",
                        "mame_disk",
                        "mame_rom_merge",
                        "mame_disk_merge",
                        "logiqx_rom",
                        "logiqx_disk",
                        "cmp_rom",
                        "pc_file",
                        "dat_rom",
                    ]
                    .iter()
                    .any(|alias| {
                        details
                            .iter()
                            .any(|detail| detail.starts_with(&format!("SCAN {alias}")))
                            || !details.iter().any(|detail| {
                                detail.starts_with(&format!("SEARCH {alias} USING"))
                                    && detail.contains("PRIMARY KEY")
                            })
                    })
                    || details
                        .iter()
                        .filter(|detail| *detail == "MATERIALIZE scoped_assertions")
                        .count()
                        != 1
            })
            .map(|(name, details)| format!("{name}: {details:?}"))
            .collect::<Vec<_>>();
        assert!(
            violations.is_empty(),
            "scoped queries must seek source, generic, and native assertions and their asset projections: {violations:?}"
        );
        Ok(())
    }

    #[test]
    fn production_support_query_seeks_only_selected_assertion_keys() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let mut conn = database.pool().get()?;
        let plan = sql_query(format!("EXPLAIN QUERY PLAN {SUPPORT_SQL}"))
            .bind::<Text, _>("selected-assertion")
            .load::<PlanRow>(&mut conn)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        assert!(
            plan.iter()
                .any(|row| row
                    .starts_with("SEARCH relationship_assertion_support USING PRIMARY KEY"))
                && !plan
                    .iter()
                    .any(|row| row.starts_with("SCAN relationship_assertion_support")),
            "support hydration must seek only the requested assertion: {plan:?}"
        );
        Ok(())
    }

    #[test]
    fn selected_supports_keep_declared_order_without_unrelated_edges() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let subject = RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("test", "left"));
        let target = RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("test", "right"));
        let evidence = RelationshipEvidence::Rationale {
            reason: "support ordering witness".into(),
        };
        let base = RelationshipClaim {
            relation_type: RelationshipType::CatalogCorrection,
            subject,
            target,
            origin: RelationshipOrigin::UserConclusion,
            evidence,
        };
        let first = record_claim(database.pool(), &base)?;
        let second = record_claim(database.pool(), &base)?;
        let unrelated = record_claim(database.pool(), &base)?;
        let declared = vec![second, first];
        let selected = record_claim(
            database.pool(),
            &RelationshipClaim {
                origin: RelationshipOrigin::DerivedCandidate {
                    rule_version: "support-order-v1".into(),
                    supporting_assertions: declared.clone(),
                },
                ..base.clone()
            },
        )?;
        let other = record_claim(
            database.pool(),
            &RelationshipClaim {
                origin: RelationshipOrigin::DerivedCandidate {
                    rule_version: "unrelated-v1".into(),
                    supporting_assertions: vec![unrelated],
                },
                ..base
            },
        )?;
        assert_ne!(selected, other);
        let mut conn = database.pool().get()?;
        let actual = sql_query(SUPPORT_SQL)
            .bind::<Text, _>(selected.as_str())
            .load::<SupportRow>(&mut conn)?
            .into_iter()
            .map(|row| RelationshipAssertionKey::new(row.supported_assertion_key))
            .collect::<Vec<_>>();
        assert_eq!(actual, declared);
        drop(conn);
        let explanations = explain_all(database.pool())?;
        let selected_explanation = explanations
            .iter()
            .find(|row| row.assertion_key == selected)
            .ok_or_else(|| crate::Error::InvalidPath("selected support witness missing".into()))?;
        assert!(matches!(&selected_explanation.claim.origin,
            RelationshipOrigin::DerivedCandidate { supporting_assertions, .. } if *supporting_assertions == declared));
        Ok(())
    }
}
