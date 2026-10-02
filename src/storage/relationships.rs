use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use std::collections::HashMap;

use crate::domain::{
    CatalogContentId, CatalogRecordKind, CatalogRecordRef, CatalogSetId, ContentDigestAlgorithm,
    ContentIdentity, DocumentLocation, ExternalRecordRef, MergeMediaKind, RelationshipAssertionKey,
    RelationshipClaim, RelationshipEndpoint, RelationshipEvidence, RelationshipExplanation,
    RelationshipOrigin, RelationshipReview, RelationshipReviewDecision, RelationshipReviewEvent,
    RelationshipRule, RelationshipSourceProvenance, RelationshipType, SnapshotKey,
};

use super::db::Pool;

mod evidence;
mod publication;
mod targets;

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
        insert_claim(conn, key, claim)
    })
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
        RelationshipOrigin::UserConclusion | RelationshipOrigin::DerivedCandidate { .. } => {}
    }
    Ok(())
}

#[derive(QueryableByName)]
struct RegisteredRelationship {
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
}

/// An owned decision draft with complete native targets; publication consumes it.
struct PendingRelationship<'a> {
    key: RelationshipAssertionKey,
    claim: &'a RelationshipClaim,
}

enum DecisionOrigin<'a> {
    Inferred(&'a RelationshipRule),
    Manual,
}

impl DecisionOrigin<'_> {
    const fn code(&self) -> &'static str {
        match self {
            Self::Inferred(_) => "derived",
            Self::Manual => "user",
        }
    }
}

impl<'a> PendingRelationship<'a> {
    fn insert(
        conn: &mut SqliteConnection,
        key: RelationshipAssertionKey,
        claim: &'a RelationshipClaim,
    ) -> crate::Result<Self> {
        let origin = match &claim.origin {
            RelationshipOrigin::DerivedCandidate { rule, .. } => DecisionOrigin::Inferred(rule),
            RelationshipOrigin::UserConclusion => DecisionOrigin::Manual,
            RelationshipOrigin::SourceAssertion { .. } => {
                return Err(crate::Error::InvalidPath(
                    "source declarations require their native owner".into(),
                ));
            }
        };
        let subject = targets::insert(conn, &claim.subject)?;
        let target = targets::insert(conn, &claim.target)?;
        let identity = sql_query("INSERT INTO catalog_relationships(assertion_key,origin) VALUES(?,?) RETURNING relationship_id")
            .bind::<Text,_>(key.as_str()).bind::<Text,_>(origin.code()).get_result::<RegisteredRelationship>(conn)?.relationship_id;
        match origin {
            DecisionOrigin::Inferred(rule) => {
                let rule = targets::insert_rule(conn, rule)?;
                sql_query("INSERT INTO inferred_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id,rule_id) VALUES(?,?,?,?,?)")
                    .bind::<BigInt,_>(identity).bind::<Text,_>(claim.relation_type.as_str())
                    .bind::<BigInt,_>(subject.database_value()).bind::<BigInt,_>(target.database_value())
                    .bind::<BigInt,_>(rule.database_value()).execute(conn)?;
            }
            DecisionOrigin::Manual => {
                sql_query("INSERT INTO manual_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id) VALUES(?,?,?,?)")
                    .bind::<BigInt,_>(identity).bind::<Text,_>(claim.relation_type.as_str())
                    .bind::<BigInt,_>(subject.database_value()).bind::<BigInt,_>(target.database_value()).execute(conn)?;
            }
        }
        Ok(Self { key, claim })
    }

    fn publish(self, conn: &mut SqliteConnection) -> crate::Result<RelationshipAssertionKey> {
        let claim = self.claim;
        evidence::insert(conn, self.key.as_str(), &claim.evidence)?;
        if let RelationshipOrigin::DerivedCandidate {
            supporting_assertions,
            ..
        } = &claim.origin
        {
            for (position, supported) in supporting_assertions.iter().enumerate() {
                let position = i64::try_from(position).map_err(|_| {
                    crate::Error::InvalidPath("relationship support position exceeds i64".into())
                })?;
                sql_query("INSERT INTO relationship_assertion_support(assertion_key,position,supported_assertion_key) VALUES(?,?,?)")
                    .bind::<Text,_>(self.key.as_str()).bind::<BigInt,_>(position).bind::<Text,_>(supported.as_str()).execute(conn)?;
            }
        }
        evidence::publish(conn, self.key.as_str(), &claim.evidence)?;
        Ok(self.key)
    }
}

fn insert_claim(
    conn: &mut SqliteConnection,
    key: RelationshipAssertionKey,
    claim: &RelationshipClaim,
) -> crate::Result<RelationshipAssertionKey> {
    PendingRelationship::insert(conn, key, claim)?.publish(conn)
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
    fn scoped_assertions_cte(self) -> Option<String> {
        let (source_filter, kinds) = match self {
            Self::All => return None,
            Self::CatalogSets => (
                "AND (a.subject_kind IN ('catalog_set','software_item') OR a.target_kind IN ('catalog_set','software_item'))",
                "('catalog_set')",
            ),
            Self::Snapshots => (
                "",
                "('catalog_set'),('catalog_media_entry'),('no_intro_archive')",
            ),
        };
        let native_sources = [
            "mame_source_relationships", "logiqx_cmp_source_relationships",
            "software_dat_source_relationships", "no_intro_database_source_relationships",
        ].map(|view| format!("SELECT a.* FROM {view} a WHERE a.source_snapshot_key IN (SELECT snapshot_key FROM requested_source_snapshots) {source_filter}")).join(" UNION ALL ");
        Some(format!(
            "WITH requested_source_snapshots(snapshot_key) AS (VALUES (?),(?)), \
             requested_decision_snapshots(snapshot_key) AS (VALUES (?),(?),(?),(?)), \
             requested_relationship_kinds(kind) AS (VALUES {kinds}), \
             scoped_source_assertions AS NOT MATERIALIZED ({native_sources}), \
             {}, scoped_assertions AS MATERIALIZED ( \
               SELECT * FROM scoped_source_assertions UNION ALL SELECT * FROM scoped_generic_assertions)",
            include_str!("db/relationship_scope.sql")
        ))
    }

    fn query(self) -> String {
        let cte = self.scoped_assertions_cte().unwrap_or_default();
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
            let rule = if row.origin == "derived_candidate" {
                Some(targets::load_rule(conn, &row.assertion_key)?)
            } else {
                None
            };
            explanation(row, events, evidence, supports, rule)
        })
        .collect()
}

fn explanation(
    row: ExplanationRow,
    review_history: Vec<RelationshipReviewEvent>,
    generic_evidence: Option<RelationshipEvidence>,
    supporting_assertions: Vec<String>,
    rule: Option<RelationshipRule>,
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
    let origin = explanation_origin(&row, supporting_assertions, rule)?;
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
    rule: Option<RelationshipRule>,
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
            rule: rule.ok_or_else(|| {
                crate::Error::DatabaseSchema("candidate has no native rule revision".into())
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
    match kind {
        "catalog_media_entry" | "no_intro_archive" => {
            let snapshot = SnapshotKey::from_persisted(snapshot.ok_or_else(|| {
                crate::Error::DatabaseSchema("native target has no actual edition".into())
            })?);
            let id = third.ok_or_else(|| {
                crate::Error::DatabaseSchema("native target has no actual owner ID".into())
            })?;
            return if kind == "catalog_media_entry" {
                Ok(RelationshipEndpoint::CatalogMediaEntry {
                    snapshot,
                    occurrence_id: id.try_into()?,
                })
            } else {
                Ok(RelationshipEndpoint::NoIntroArchive {
                    snapshot,
                    archive_id: id.try_into()?,
                })
            };
        }
        "shared_file" => {
            return Ok(RelationshipEndpoint::SharedCatalogFile(
                first
                    .ok_or_else(|| {
                        crate::Error::DatabaseSchema("shared-file target has no UUID".into())
                    })?
                    .parse::<CatalogContentId>()?,
            ));
        }
        _ => {}
    }
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
            "inferred_relationship_from_target",
            "inferred_relationship_to_target",
            "manual_relationship_from_target",
            "manual_relationship_to_target",
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
                        || detail == "SCAN identity"
                        || detail == "SCAN target"
                }) || scoped_indexes.iter().any(|index| {
                    !details
                        .iter()
                        .any(|detail| detail.starts_with("SEARCH ") && detail.contains(index))
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
                    rule: crate::domain::RelationshipRule::new(
                        "support-order",
                        "v1",
                        "Ordered support witness",
                    )?,
                    supporting_assertions: declared.clone(),
                },
                ..base.clone()
            },
        )?;
        let other = record_claim(
            database.pool(),
            &RelationshipClaim {
                origin: RelationshipOrigin::DerivedCandidate {
                    rule: crate::domain::RelationshipRule::new(
                        "unrelated",
                        "v1",
                        "Unrelated support witness",
                    )?,
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
