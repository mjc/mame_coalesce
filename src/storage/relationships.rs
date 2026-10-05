use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};
use std::collections::HashMap;

use crate::domain::{
    CatalogContentId, CatalogRecordKind, CatalogRecordRef, CatalogSetId, ContentDigestAlgorithm,
    ContentIdentity, DocumentLocation, ExternalRecordRef, MergeMediaKind, ObservedContentIdentity,
    RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipEvidence,
    RelationshipExplanation, RelationshipOrigin, RelationshipReview, RelationshipReviewDecision,
    RelationshipReviewEvent, RelationshipRule, RelationshipSourceProvenance, RelationshipType,
    SnapshotKey,
};

use super::db::Pool;

mod evidence;
mod publication;
mod reviews;
mod targets;

/// Local registry owner; external relationship keys are interchange identities.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RelationshipId(i64);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RelationshipReviewId(i64);

#[derive(QueryableByName)]
struct ExplanationRow {
    #[diesel(sql_type = Nullable<Text>)]
    source_format: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    source_location_valid: bool,
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
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

const SUPPORT_SQL: &str = "SELECT supporting.assertion_key AS supported_assertion_key \
     FROM catalog_relationship_evidence AS evidence \
     JOIN catalog_relationships AS supporting \
       ON supporting.relationship_id=evidence.supporting_relationship_id \
     WHERE evidence.relationship_id=? ORDER BY evidence.list_order";

// Callers supply requested_reviews(review_id); publication and integrity use
// these same canonical predicates rather than a second owner validator.
const REVIEW_READINESS_SQL: &str = concat!(
    "WITH ",
    include_str!("db/relationship_review_readiness_scope.sql"),
    include_str!("db/relationship_readiness.sql"),
    "), ",
    include_str!("db/relationship_review_readiness.sql"),
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
    conn.immediate_transaction::<_, crate::Error, _>(|conn| insert_claim(conn, key, claim))
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
    id: RelationshipId,
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
        Ok(Self {
            id: RelationshipId(identity),
            key,
            claim,
        })
    }

    fn publish(self, conn: &mut SqliteConnection) -> crate::Result<RelationshipAssertionKey> {
        let claim = self.claim;
        evidence::insert(conn, self.id, &claim.evidence)?;
        if let RelationshipOrigin::DerivedCandidate {
            supporting_assertions,
            ..
        } = &claim.origin
        {
            for (position, supported) in supporting_assertions.iter().enumerate() {
                let position = i64::try_from(position).map_err(|_| {
                    crate::Error::InvalidPath("relationship support position exceeds i64".into())
                })?;
                let supporting = require_published_relationship(conn, supported)?;
                sql_query("INSERT INTO catalog_relationship_evidence(relationship_id,list_order,supporting_relationship_id) VALUES(?,?,?)")
                    .bind::<BigInt,_>(self.id.0).bind::<BigInt,_>(position).bind::<BigInt,_>(supporting.0).execute(conn)?;
            }
        }
        evidence::publish(conn, self.id, &claim.evidence)?;
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

fn require_published_relationship(
    conn: &mut SqliteConnection,
    key: &RelationshipAssertionKey,
) -> crate::Result<RelationshipId> {
    publication::published_id(conn, key)?.ok_or_else(|| {
        crate::Error::InvalidPath(format!(
            "relationship assertion {} is missing or unpublished",
            key.as_str()
        ))
    })
}

#[derive(QueryableByName)]
struct RegisteredReview {
    #[diesel(sql_type = BigInt)]
    review_id: i64,
}

/// Review facts and any successor are invisible until this draft is consumed.
struct PendingReview(RelationshipReviewId);

impl PendingReview {
    fn insert(
        conn: &mut SqliteConnection,
        predecessor: RelationshipId,
        successor: Option<RelationshipId>,
        review: &RelationshipReview,
    ) -> crate::Result<Self> {
        let row = sql_query(
            "INSERT INTO catalog_relationship_reviews \
             (review_key, relationship_id, decision, note) VALUES (?, ?, ?, ?) \
             RETURNING review_id",
        )
        .bind::<Text, _>(uuid::Uuid::new_v4().simple().to_string())
        .bind::<BigInt, _>(predecessor.0)
        .bind::<Text, _>(review.decision.as_str())
        .bind::<Text, _>(&review.note)
        .get_result::<RegisteredReview>(conn)?;
        let id = RelationshipReviewId(row.review_id);
        if let Some(successor) = successor {
            sql_query("INSERT INTO replaced_catalog_relationships(review_id,replacement_relationship_id) VALUES(?,?)")
                .bind::<BigInt,_>(id.0).bind::<BigInt,_>(successor.0).execute(conn)?;
        }
        Ok(Self(id))
    }

    fn publish(self, conn: &mut SqliteConnection) -> crate::Result<()> {
        sql_query("INSERT INTO catalog_relationship_review_publications(review_id) VALUES(?)")
            .bind::<BigInt, _>(self.0.0)
            .execute(conn)?;
        Ok(())
    }
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
        let predecessor = require_published_relationship(conn, assertion_key)?;
        let successor = review
            .superseded_by
            .as_ref()
            .map(|key| require_published_relationship(conn, key))
            .transpose()?;
        PendingReview::insert(conn, predecessor, successor, review)?.publish(conn)
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
            "no_intro_pc_source_relationships",
        ].map(|view| format!("SELECT a.* FROM {view} a WHERE a.source_snapshot_key IN ((SELECT previous_key FROM requested_source_snapshots),(SELECT current_key FROM requested_source_snapshots)) AND a.assertion_key IN (SELECT assertion_key FROM scoped_source_identities) {source_filter}")).join(" UNION ALL ");
        Some(format!(
            "WITH requested_source_snapshots(previous_key,current_key) AS (VALUES (?,?)), \
             requested_decision_snapshots(snapshot_key) AS (VALUES (?),(?),(?),(?)), \
             requested_relationship_kinds(kind) AS (VALUES {kinds}), \
             scoped_source_identities AS MATERIALIZED ( \
               SELECT DISTINCT identity.assertion_key FROM requested_source_snapshots requested \
               CROSS JOIN catalog_relationships identity ON identity.snapshot_key IN (requested.previous_key,requested.current_key) \
               WHERE identity.origin='source'), \
             scoped_source_assertions AS NOT MATERIALIZED ({native_sources}), \
             {}, scoped_assertions AS MATERIALIZED ( \
               SELECT * FROM scoped_source_assertions UNION ALL SELECT * FROM scoped_generic_assertions)",
            include_str!("db/relationship_scope.sql")
        ))
    }

    fn query(self) -> String {
        let scoped = self.scoped_assertions_cte();
        let from = if scoped.is_some() {
            "scoped_assertions a"
        } else {
            "relationship_assertion_explanations a"
        };
        let cte = scoped.unwrap_or_default();
        let merge_name = "COALESCE(mame_rom_merge.merge_name,mame_disk_merge.merge_name,logiqx_merge.merge_name,cmp_merge.merge_name)";
        let occurrence_id =
            "COALESCE(native_source_occurrence.occurrence_id,source_occurrence.occurrence_id)";
        let size = "COALESCE(mame_rom.size,logiqx_rom.size,cmp_rom.size,pc_file.size,dat_rom.size)";
        let scope = "COALESCE(mame_rom.evidence_scope,mame_disk.evidence_scope,logiqx_rom.evidence_scope,logiqx_disk.evidence_scope,cmp_rom.evidence_scope,pc_file.evidence_scope,dat_rom.evidence_scope)";
        format!(
            "{cte} SELECT registry.relationship_id, a.assertion_key, a.relation_type, a.origin, \
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
                pi.format AS source_format, \
                (typeof(a.source_line)='integer' AND typeof(a.source_column)='integer' \
                 AND a.source_line>0 AND a.source_column>0) AS source_location_valid, \
                pi.parser_name, pi.parser_version, pi.rules_version \
         FROM {from} \
         CROSS JOIN catalog_relationships registry ON registry.assertion_key=a.assertion_key \
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
                    SELECT 1 FROM catalog_relationship_evidence_publications AS ready WHERE ready.relationship_id=registry.relationship_id)) \
         ORDER BY a.relation_type, a.source_snapshot_key, a.subject_kind, \
                  a.source_subject_a, a.source_subject_b, a.source_subject_c, \
                  a.generic_subject_a, a.generic_subject_b, a.generic_subject_c, \
                  a.target_kind, a.source_target_a, a.source_target_b, a.source_target_c, \
                  a.generic_target_a, a.generic_target_b, a.generic_target_c, a.assertion_key"
        )
    }
}

pub fn explain_all(pool: &Pool) -> crate::Result<Vec<RelationshipExplanation>> {
    let mut conn = pool.get()?;
    let rows = sql_query(ExplanationScope::All.query()).load::<ExplanationRow>(&mut conn)?;
    let reviews = reviews::load(&mut conn, &rows)?;
    build_explanations(&mut conn, rows, reviews)
}

pub fn explain_catalog_sets_for_snapshots(
    conn: &mut SqliteConnection,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<Vec<RelationshipExplanation>> {
    if catalog_set_relationships_are_empty(conn, previous, current)? {
        return Ok(Vec::new());
    }
    let rows = sql_query(ExplanationScope::CatalogSets.query())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .load::<ExplanationRow>(conn)?;
    let reviews = reviews::load(conn, &rows)?;
    build_explanations(conn, rows, reviews)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeRelationshipOwners {
    CatalogSets,
    NoIntroArchives,
}

impl NativeRelationshipOwners {
    fn for_snapshot(conn: &mut SqliteConnection, snapshot: &SnapshotKey) -> crate::Result<Self> {
        #[derive(QueryableByName)]
        struct FormatRow {
            #[diesel(sql_type = Text)]
            format: String,
        }

        let row = sql_query(
            "SELECT interpretation.format FROM catalog_snapshots AS snapshot \
             JOIN parser_interpretations AS interpretation USING(interpretation_key) \
             WHERE snapshot.snapshot_key=?",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<FormatRow>(conn)?;
        match row.format.as_str() {
            "no-intro-database-xml-compatible" | "no-intro-database-xml-nul-compatible" => {
                Ok(Self::NoIntroArchives)
            }
            "mame-listxml"
            | "logiqx"
            | "clrmamepro-dat"
            | "mame-softwarelist-xml"
            | "no-intro-pc-xml"
            | "no-intro-dat-v3-strict"
            | "no-intro-dat-v3-compatible"
            | "no-intro-dat-v4-strict"
            | "no-intro-dat-v4-compatible" => Ok(Self::CatalogSets),
            _ => Err(crate::Error::DatabaseSchema(format!(
                "unsupported history parser format: {}",
                row.format
            ))),
        }
    }
}

fn catalog_set_relationships_are_empty(
    conn: &mut SqliteConnection,
    previous: &SnapshotKey,
    current: &SnapshotKey,
) -> crate::Result<bool> {
    #[derive(QueryableByName)]
    struct DecisionPresence {
        #[diesel(sql_type = Bool)]
        present: bool,
    }

    let previous_owners = NativeRelationshipOwners::for_snapshot(conn, previous)?;
    let current_owners = NativeRelationshipOwners::for_snapshot(conn, current)?;
    if previous_owners != NativeRelationshipOwners::NoIntroArchives
        || current_owners != NativeRelationshipOwners::NoIntroArchives
    {
        return Ok(false);
    }

    // Export source assertions belong to archives, not root catalog sets. Root
    // owners can still acquire manual or inferred decisions at either endpoint.
    // Reuse the full reader's canonical owner scope before avoiding its costly
    // native-source expansion; any scoped decision requires the full reader.
    let query = format!(
        "WITH requested_decision_snapshots(snapshot_key) AS (VALUES (?),(?),(?),(?)), \
         requested_relationship_kinds(kind) AS (VALUES ('catalog_set')), \
         {} SELECT EXISTS(SELECT 1 FROM scoped_decision_ids) AS present",
        include_str!("db/relationship_scope.sql")
    );
    let decisions = sql_query(query)
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .bind::<Text, _>(previous.as_str())
        .bind::<Text, _>(current.as_str())
        .get_result::<DecisionPresence>(conn)?;
    Ok(!decisions.present)
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
    let reviews = reviews::load(conn, &rows)?;
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
            .bind::<BigInt, _>(row.relationship_id)
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
                Some(evidence::load(conn, RelationshipId(row.relationship_id))?)
            };
            let supports = support_by_assertion
                .remove(&row.assertion_key)
                .unwrap_or_default();
            let rule = if row.origin == "derived_candidate" {
                Some(targets::load_rule(
                    conn,
                    RelationshipId(row.relationship_id),
                )?)
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
    if row.origin == "source_assertion"
        && row.source_format.as_deref() == Some("mame-listxml")
        && !row.source_location_valid
    {
        return Err(crate::Error::XmlValidation(
            "MAME relationship has no valid attribute QName witness".into(),
        ));
    }
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
        Some("archive_clone" | "archive_mergeof" | "clone" | "mergeof") => {
            Ok(RelationshipEvidence::ArchiveReference {
                declared_archive_reference: target_name.clone(),
                source_field: row.source_field.clone().ok_or_else(|| {
                    crate::Error::InvalidPath("archive source evidence has no field".into())
                })?,
            })
        }
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
        "content_object" | "observed_content" => {
            let algorithm = match first.as_str() {
                "crc32" => ContentDigestAlgorithm::Crc32,
                "md5" => ContentDigestAlgorithm::Md5,
                "sha1" => ContentDigestAlgorithm::Sha1,
                "sha256" => ContentDigestAlgorithm::Sha256,
                _ => return Err(crate::Error::InvalidPath("unknown digest algorithm".into())),
            };
            let digest = second
                .ok_or_else(|| crate::Error::InvalidPath("content object has no digest".into()))?;
            if kind == "observed_content" {
                Ok(RelationshipEndpoint::ObservedContent(
                    ObservedContentIdentity::new(algorithm, digest)?,
                ))
            } else {
                Ok(RelationshipEndpoint::ContentObject(ContentIdentity::new(
                    algorithm, digest,
                )?))
            }
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
    use crate::app::{CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus};
    use crate::domain::{CatalogKey, CatalogScope, PublishingSourceKey};
    use std::fmt::Write as _;

    #[derive(QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = Text)]
        detail: String,
    }

    fn small_history_sql_catalogs(
        database: &crate::database::Database,
    ) -> crate::Result<Vec<SnapshotKey>> {
        let directory = tempfile::tempdir()?;
        let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("catalog.dat"))
            .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
        std::fs::write(
            &path,
            "<datafile><game name='parent'><description>Parent</description></game><game name='child' cloneof='parent'><description>Child</description></game></datafile>",
        )?;
        ["left", "right", "unrelated"]
            .into_iter()
            .map(|name| {
                let report = crate::app::import_catalog(
                    database,
                    &CatalogImportRequest {
                        document_path: path.clone(),
                        format: CatalogDocumentFormat::Logiqx(
                            crate::logiqx::LogiqxMode::ObservedCompatible,
                        ),
                        source_key: PublishingSourceKey::new(name),
                        source_display_name: name.into(),
                        catalog_key: CatalogKey::new(name),
                        catalog_display_name: name.into(),
                        scope: CatalogScope::Complete,
                    },
                )?;
                assert_eq!(report.status, CatalogImportStatus::Succeeded);
                report
                    .snapshot_key
                    .ok_or_else(|| crate::Error::InvalidPath("missing fixture snapshot".into()))
            })
            .collect()
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "Test instrumentation and fixture invariants must fail loudly"
    )]
    fn history_sql_unreviewed_native_assertions_skip_review_readiness() -> crate::Result<()> {
        use diesel::{Connection as _, connection::InstrumentationEvent};
        use std::sync::{Arc, Mutex};

        let database = crate::database::Database::in_memory()?;
        let snapshots = small_history_sql_catalogs(&database)?;
        let queries = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured = Arc::clone(&queries);
        let mut conn = database.pool().get()?;
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                captured
                    .lock()
                    .expect("query capture lock")
                    .push(query.to_string());
            }
        });
        for scope in [ExplanationScope::CatalogSets, ExplanationScope::Snapshots] {
            let explanations = match scope {
                ExplanationScope::CatalogSets => {
                    explain_catalog_sets_for_snapshots(&mut conn, &snapshots[0], &snapshots[1])?
                }
                ExplanationScope::Snapshots => {
                    explain_for_snapshots(&mut conn, &snapshots[0], &snapshots[1])?
                }
                ExplanationScope::All => unreachable!("scoped fixture cases"),
            };
            assert_eq!(explanations.len(), 2);
            assert!(explanations.iter().all(|row| row.review_history.is_empty()));
        }
        drop(conn);
        assert_eq!(explain_all(database.pool())?.len(), 3);
        assert!(
            queries
                .lock()
                .expect("query capture lock")
                .iter()
                .all(|query| !query.contains("review_readiness AS")),
            "unreviewed native assertions must not compile the review-readiness pipeline"
        );
        let unrelated =
            explain_for_snapshots(&mut *database.pool().get()?, &snapshots[2], &snapshots[2])?
                .into_iter()
                .next()
                .expect("unrelated source assertion");
        review_claim(
            database.pool(),
            &unrelated.assertion_key,
            &RelationshipReview {
                decision: RelationshipReviewDecision::Accepted,
                note: "unrelated published review".into(),
                superseded_by: None,
            },
        )?;
        queries.lock().expect("query capture lock").clear();
        let scoped =
            explain_for_snapshots(&mut *database.pool().get()?, &snapshots[0], &snapshots[1])?;
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|row| row.review_history.is_empty()));
        assert!(
            queries
                .lock()
                .expect("query capture lock")
                .iter()
                .all(|query| !query.contains("review_readiness AS")),
            "unrelated published reviews must not trigger selected-owner review compilation"
        );
        let all = explain_all(database.pool())?;
        assert_eq!(all.len(), 3);
        assert_eq!(
            all.iter()
                .map(|row| row.review_history.len())
                .sum::<usize>(),
            1
        );
        Ok(())
    }

    fn import_review_plan_catalogs(
        database: &crate::database::Database,
    ) -> crate::Result<Vec<SnapshotKey>> {
        let path = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/catalog/logiqx/catalog-a-v1.dat");
        let mut snapshots = Vec::new();
        for name in ["left", "right", "unrelated"] {
            let report = crate::app::import_catalog(
                database,
                &CatalogImportRequest {
                    document_path: path.clone(),
                    format: CatalogDocumentFormat::Logiqx(
                        crate::logiqx::LogiqxMode::ObservedCompatible,
                    ),
                    source_key: PublishingSourceKey::new(name),
                    source_display_name: name.into(),
                    catalog_key: CatalogKey::new(name),
                    catalog_display_name: name.into(),
                    scope: CatalogScope::Complete,
                },
            )?;
            assert_eq!(report.status, CatalogImportStatus::Succeeded);
            snapshots.push(
                report
                    .snapshot_key
                    .ok_or_else(|| crate::Error::InvalidPath("missing fixture snapshot".into()))?,
            );
        }
        let unrelated_directory = tempfile::tempdir()?;
        let unrelated_path =
            camino::Utf8PathBuf::from_path_buf(unrelated_directory.path().join("unrelated.dat"))
                .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
        let mut games = String::new();
        for index in 0..512 {
            write!(games, "<game name='unrelated-{index}' cloneof='unknown-{index}'><description>Unrelated</description></game>")
                .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
        }
        std::fs::write(&unrelated_path, format!("<datafile>{games}</datafile>"))?;
        let report = crate::app::import_catalog(
            database,
            &CatalogImportRequest {
                document_path: unrelated_path,
                format: CatalogDocumentFormat::Logiqx(
                    crate::logiqx::LogiqxMode::ObservedCompatible,
                ),
                source_key: PublishingSourceKey::new("populated-unrelated"),
                source_display_name: "Populated unrelated".into(),
                catalog_key: CatalogKey::new("populated-unrelated"),
                catalog_display_name: "Populated unrelated".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        Ok(snapshots)
    }

    fn populate_review_plan_relationships(
        conn: &mut SqliteConnection,
    ) -> crate::Result<(Vec<RelationshipId>, PendingReview)> {
        let base = RelationshipClaim {
            relation_type: RelationshipType::CatalogCorrection,
            subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("plans", "left")),
            target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("plans", "right")),
            origin: RelationshipOrigin::UserConclusion,
            evidence: RelationshipEvidence::Rationale {
                reason: "populated query witness".into(),
            },
        };
        let identities = conn.immediate_transaction::<_, crate::Error, _>(|conn| {
            let mut identities = Vec::new();
            for index in 0..128 {
                let key = insert_claim(conn, RelationshipAssertionKey::fresh(), &base)?;
                let id = require_published_relationship(conn, &key)?;
                let accepted = RelationshipReview {
                    decision: RelationshipReviewDecision::Accepted,
                    note: "published plan witness".into(),
                    superseded_by: None,
                };
                for _ in 0..3 {
                    PendingReview::insert(conn, id, None, &accepted)?.publish(conn)?;
                }
                if let Some(previous) = identities.last().copied() {
                    let superseded = RelationshipReview {
                        decision: RelationshipReviewDecision::Superseded,
                        note: "historical edge plan witness".into(),
                        superseded_by: None,
                    };
                    PendingReview::insert(conn, id, Some(previous), &superseded)?.publish(conn)?;
                    PendingReview::insert(conn, id, None, &accepted)?.publish(conn)?;
                }
                let withdrawn = RelationshipReview {
                    decision: RelationshipReviewDecision::Withdrawn,
                    note: "unsealed plan witness".into(),
                    superseded_by: None,
                };
                let _draft = PendingReview::insert(conn, id, None, &withdrawn)?;
                if index < 32 {
                    let candidate = RelationshipClaim {
                        origin: RelationshipOrigin::DerivedCandidate {
                            rule: RelationshipRule::new(
                                "populated-support",
                                "v1",
                                "Populated support witness",
                            )?,
                            supporting_assertions: vec![key.clone(), key],
                        },
                        ..base.clone()
                    };
                    insert_claim(conn, RelationshipAssertionKey::fresh(), &candidate)?;
                }
                identities.push(id);
            }
            Ok(identities)
        })?;
        let superseded = RelationshipReview {
            decision: RelationshipReviewDecision::Superseded,
            note: "bounded active chain witness".into(),
            superseded_by: None,
        };
        for pair in identities[..4].windows(2) {
            PendingReview::insert(conn, pair[0], Some(pair[1]), &superseded)?.publish(conn)?;
        }
        let cycle_draft =
            PendingReview::insert(conn, identities[3], Some(identities[0]), &superseded)?;
        Ok((identities, cycle_draft))
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Keep owner-plan and chronological batch witnesses on the same populated fixture"
    )]
    fn populated_scoped_review_and_support_plans_stay_owner_bounded() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let snapshots = import_review_plan_catalogs(&database)?;
        let mut conn = database.pool().get()?;
        let (identities, cycle_draft) = populate_review_plan_relationships(&mut conn)?;
        sql_query("ANALYZE").execute(&mut conn)?;
        let cycle_sql = concat!(
            "WITH RECURSIVE requested_reviews(review_id) AS (VALUES (?)), ",
            include_str!("db/relationship_review_active_edges.sql"),
            "SELECT * FROM cycle_results"
        );
        let cycle_details = sql_query(format!("EXPLAIN QUERY PLAN {cycle_sql}"))
            .bind::<BigInt, _>(cycle_draft.0.0)
            .load::<PlanRow>(&mut conn)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        assert!(
            cycle_details.iter().any(|detail| detail.starts_with(
                "SEARCH candidate USING COVERING INDEX catalog_relationship_reviews_owner_latest"
            )) && !cycle_details.iter().any(|detail| [
                "SCAN candidate",
                "SCAN latest",
                "SCAN replacement",
                "SCAN catalog_relationship_reviews"
            ]
            .iter()
            .any(|prefix| detail.starts_with(prefix))),
            "cycle traversal must seek latest published reviews per visited owner: {cycle_details:?}"
        );
        let support = sql_query(format!("EXPLAIN QUERY PLAN {SUPPORT_SQL}"))
            .bind::<BigInt, _>(identities[0].0)
            .load::<PlanRow>(&mut conn)?;
        assert!(
            support
                .iter()
                .any(|row| row.detail.starts_with("SEARCH evidence USING PRIMARY KEY")),
            "support owner seek missing"
        );
        for (query, id, expected_seek) in [
            (
                reviews::candidates_query(1),
                identities[0].0,
                "SEARCH review USING COVERING INDEX catalog_relationship_reviews_owner_latest",
            ),
            (
                reviews::readiness_query(1),
                cycle_draft.0.0,
                "SEARCH review USING INTEGER PRIMARY KEY",
            ),
        ] {
            let details = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
                .bind::<BigInt, _>(id)
                .load::<PlanRow>(&mut conn)?
                .into_iter()
                .map(|row| row.detail)
                .collect::<Vec<_>>();
            let scans = details.iter().any(|detail| {
                [
                    "SCAN review",
                    "SCAN identity",
                    "SCAN registry",
                    "SCAN inferred",
                    "SCAN manual",
                    "SCAN owner",
                    "SCAN replacement",
                    "SCAN predecessor_identity",
                    "SCAN successor_identity",
                ]
                .iter()
                .any(|prefix| detail.starts_with(prefix))
            });
            assert!(
                !scans,
                "scoped review hydration scans unrelated owners: {details:?}"
            );
            assert!(
                details
                    .iter()
                    .any(|detail| detail.starts_with(expected_seek)),
                "review history must start from indexed selected owners: {details:?}"
            );
        }
        let scoped = explain_for_snapshots(&mut conn, &snapshots[0], &snapshots[1])?;
        assert!(scoped.iter().all(|row| row.review_history.is_empty()));
        drop(conn);
        let all = explain_all(database.pool())?;
        assert_eq!(
            all.iter()
                .map(|row| row.review_history.len())
                .sum::<usize>(),
            641,
            "all published review events must survive both owner and review-ID batches; drafts stay hidden"
        );
        assert_eq!(
            all.iter()
                .filter(|row| row
                    .latest_review
                    .as_ref()
                    .is_some_and(|review| review.note == "bounded active chain witness"))
                .count(),
            3,
            "late replacement reviews must remain the latest events across review batches"
        );
        Ok(())
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

    fn import_pc_plan_catalogs(
        database: &crate::database::Database,
    ) -> crate::Result<Vec<SnapshotKey>> {
        let directory = tempfile::tempdir()?;
        let path = camino::Utf8PathBuf::from_path_buf(directory.path().join("pc-plans.xml"))
            .map_err(|_| crate::Error::InvalidPath("non-UTF-8 test path".into()))?;
        std::fs::write(
            &path,
            "<datafile><game name='clone' clone='0007'/><game name='merge' clone='P' mergeof='0008'/></datafile>",
        )?;
        let mut snapshots = Vec::new();
        for index in 0..34 {
            let name = format!("pc-plan-{index}");
            let report = crate::app::import_catalog(
                database,
                &CatalogImportRequest {
                    document_path: path.clone(),
                    format: CatalogDocumentFormat::NoIntroPcXml,
                    source_key: PublishingSourceKey::new(&name),
                    source_display_name: name.clone(),
                    catalog_key: CatalogKey::new(&name),
                    catalog_display_name: name,
                    scope: CatalogScope::Complete,
                },
            )?;
            assert_eq!(report.status, CatalogImportStatus::Succeeded);
            snapshots.push(
                report
                    .snapshot_key
                    .ok_or_else(|| crate::Error::InvalidPath("missing P/C plan snapshot".into()))?,
            );
        }
        Ok(snapshots)
    }

    #[test]
    fn populated_pc_scoped_explanations_keep_native_sources_and_indexed_owners() -> crate::Result<()>
    {
        let database = crate::database::Database::in_memory()?;
        let snapshots = import_pc_plan_catalogs(&database)?;
        let mut connection = database.pool().get()?;
        sql_query("ANALYZE").execute(&mut connection)?;
        let requested = &snapshots[..2];
        for scope in [ExplanationScope::CatalogSets, ExplanationScope::Snapshots] {
            let mut query = sql_query(scope.query()).into_boxed::<diesel::sqlite::Sqlite>();
            for snapshot in requested.iter().cycle().take(6) {
                query = query.bind::<Text, _>(snapshot.as_str());
            }
            let rows = query.load::<ExplanationRow>(&mut connection)?;
            assert_eq!(
                rows.len(),
                4,
                "both production scopes must retain P/C sources"
            );
            assert!(rows.iter().all(|row| {
                row.subject_snapshot_key
                    .as_deref()
                    .is_some_and(|key| requested.iter().any(|snapshot| snapshot.as_str() == key))
                    && matches!(row.source_field.as_deref(), Some("clone" | "mergeof"))
            }));

            let mut same_edition = sql_query(scope.query()).into_boxed::<diesel::sqlite::Sqlite>();
            for snapshot in std::iter::repeat_n(&snapshots[0], 6) {
                same_edition = same_edition.bind::<Text, _>(snapshot.as_str());
            }
            assert_eq!(
                same_edition.load::<ExplanationRow>(&mut connection)?.len(),
                2,
                "requesting the same edition twice must not duplicate its source declarations"
            );

            let mut query = sql_query(format!("EXPLAIN QUERY PLAN {}", scope.query()))
                .into_boxed::<diesel::sqlite::Sqlite>();
            for snapshot in requested.iter().cycle().take(6) {
                query = query.bind::<Text, _>(snapshot.as_str());
            }
            let details = query
                .load::<PlanRow>(&mut connection)?
                .into_iter()
                .map(|row| row.detail)
                .collect::<Vec<_>>();
            for table in ["no_intro_pc_clone_links", "no_intro_pc_merge_links"] {
                assert!(
                    details.iter().any(|line| {
                        line.starts_with("SEARCH link ")
                            && line.contains(table)
                            && line.contains("relationship_id=?")
                    }),
                    "P/C declarations must be sought by issued ID: {table}: {details:?}"
                );
            }
            for alias in ["registry", "reported", "link", "native", "sets", "groups"] {
                assert!(
                    !details.iter().any(|line| line == &format!("SCAN {alias}")
                        || line.starts_with(&format!("SCAN {alias} "))),
                    "production explanations scanned unrelated native owners: {alias}: {details:?}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn production_support_query_seeks_only_selected_relationship_ids() -> crate::Result<()> {
        let database = crate::database::Database::in_memory()?;
        let mut conn = database.pool().get()?;
        let plan = sql_query(format!("EXPLAIN QUERY PLAN {SUPPORT_SQL}"))
            .bind::<BigInt, _>(17)
            .load::<PlanRow>(&mut conn)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        assert!(
            plan.iter()
                .any(|row| row.starts_with("SEARCH evidence USING PRIMARY KEY"))
                && !plan.iter().any(|row| row.starts_with("SCAN evidence")),
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
        let selected_id = require_published_relationship(&mut conn, &selected)?;
        let actual = sql_query(SUPPORT_SQL)
            .bind::<BigInt, _>(selected_id.0)
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
