//! Explicit, evidence-backed reviews of catalog file identity conflicts.
//!
//! Reviews concern expected catalog content, never proof of observed ROM bytes.
//! Rejection is monotonic for the exact source assertion: an accept in another
//! review cannot reinstate it. Settling a conflict does not assign its incoming
//! entry a UUID or implicitly settle any other conflict.

use diesel::{
    QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use thiserror::Error;

use super::catalog_identity::OccurrenceId;
use crate::{database::Database, domain::CatalogContentId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConflictRef {
    pub incoming: OccurrenceId,
    pub candidate: CatalogContentId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Accept,
    Reject,
}

impl Disposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Reject => "reject",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceRole {
    Incoming,
    Candidate,
}

impl EvidenceRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Candidate => "candidate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WholeFileScope {
    WholeAsset,
    WholeFile,
}

impl WholeFileScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::WholeAsset => "whole_asset",
            Self::WholeFile => "whole_file",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeField {
    MameRom,
    LogiqxRom,
    ClrMameProRom,
    NoIntroPcFile,
    NoIntroDatRom,
    NoIntroDatabaseSourceFile,
    NoIntroDatabaseReleaseFile,
    /// First verification run of a native software ROM file declaration.
    SoftwareRomFile,
}

impl SizeField {
    const fn as_str(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom_size",
            Self::LogiqxRom => "logiqx_rom_size",
            Self::ClrMameProRom => "cmp_rom_size",
            Self::NoIntroPcFile => "no_intro_pc_file_size",
            Self::NoIntroDatRom => "no_intro_dat_rom_size",
            Self::NoIntroDatabaseSourceFile => "no_intro_database_source_file_size",
            Self::NoIntroDatabaseReleaseFile => "no_intro_database_release_file_size",
            Self::SoftwareRomFile => "software_rom_file_size",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewedEvidence {
    Hash {
        occurrence: OccurrenceId,
        digest_id: i64,
        scope: WholeFileScope,
        role: EvidenceRole,
    },
    Size {
        occurrence: OccurrenceId,
        field: SizeField,
        role: EvidenceRole,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvidenceDecision {
    pub conflict: ConflictRef,
    pub evidence: ReviewedEvidence,
    pub disposition: Disposition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    /// Settle only these source conflicts; this is not a permanent ban on a merge.
    KeepSeparate,
    /// Keep one current canonical UUID, redirecting the other named candidates.
    Merge {
        kept: CatalogContentId,
        old: Vec<CatalogContentId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMatchReview {
    pub rationale: String,
    pub conflicts: Vec<ConflictRef>,
    pub evidence: Vec<EvidenceDecision>,
    pub action: ReviewAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecisionId(i64);

impl DecisionId {
    #[must_use]
    pub const fn as_i64(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Error)]
pub enum ReviewError {
    #[error("review needs a nonempty rationale and at least one exact source conflict")]
    EmptyReview,
    #[error(
        "merge must name at least two distinct current canonical UUIDs connected by the reviewed source conflicts"
    )]
    InvalidMerge,
    #[error("unknown issued catalog file UUID {0}")]
    UnknownFile(CatalogContentId),
    #[error("catalog UUID redirect graph is corrupt")]
    CorruptRedirects,
}

/// Resolve an issued ID without changing either its source occurrences or its registry.
pub fn resolve_file_id(
    database: &Database,
    issued: CatalogContentId,
) -> crate::Result<CatalogContentId> {
    let mut connection = database.pool().get()?;
    super::catalog_content::resolve_issued_id(&mut connection, issued)
}

/// Publish one reviewed action atomically, or leave no decision/evidence/redirect rows.
pub fn record_review(database: &Database, review: &FileMatchReview) -> crate::Result<DecisionId> {
    let mut connection = database.pool().get()?;
    connection.immediate_transaction(|connection| record_in_transaction(connection, review))
}

#[derive(QueryableByName)]
struct AllocatedDecision {
    #[diesel(sql_type = BigInt)]
    decision_id: i64,
}

/// Only a draft can acquire evidence. Publication consumes it and returns the
/// public decision ID, which cannot be passed to the draft-writing functions.
struct DraftDecision {
    id: DecisionId,
}

impl DraftDecision {
    fn publish(self, connection: &mut SqliteConnection) -> crate::Result<DecisionId> {
        sql_query("INSERT INTO file_match_decision_publications(decision_id) VALUES (?)")
            .bind::<BigInt, _>(self.id.as_i64())
            .execute(connection)?;
        Ok(self.id)
    }
}

fn record_in_transaction(
    connection: &mut SqliteConnection,
    review: &FileMatchReview,
) -> crate::Result<DecisionId> {
    if review.rationale.trim().is_empty() || review.conflicts.is_empty() {
        return Err(ReviewError::EmptyReview.into());
    }
    let (kind, outcome, kept) = match &review.action {
        ReviewAction::KeepSeparate => ("keep_separate", "keep_separate", None),
        ReviewAction::Merge { kept, old } => {
            validate_merge(connection, review, *kept, old)?;
            ("merge", "merged", Some(kept.as_bytes().as_slice()))
        }
    };
    let allocated = sql_query("INSERT INTO file_match_decisions(decision,kept_content_uuid,rationale) VALUES (?,?,?) RETURNING decision_id")
        .bind::<Text,_>(kind).bind::<Nullable<Binary>,_>(kept).bind::<Text,_>(&review.rationale)
        .get_result::<AllocatedDecision>(connection)?;
    let decision = DraftDecision {
        id: DecisionId(allocated.decision_id),
    };
    for conflict in &review.conflicts {
        sql_query("INSERT INTO file_match_decision_conflicts(decision_id,occurrence_id,candidate_content_uuid,outcome) VALUES (?,?,?,?)")
            .bind::<BigInt,_>(decision.id.as_i64()).bind::<BigInt,_>(conflict.incoming.database_value())
            .bind::<Binary,_>(conflict.candidate.as_bytes().as_slice()).bind::<Text,_>(outcome).execute(connection)?;
    }
    for disposition in &review.evidence {
        insert_evidence(connection, &decision, *disposition)?;
    }
    if let ReviewAction::Merge { kept, old } = &review.action {
        for issued in old {
            sql_query("INSERT INTO merged_file_ids(old_content_uuid,kept_content_uuid,decision_id) VALUES (?,?,?)")
                .bind::<Binary,_>(issued.as_bytes().as_slice()).bind::<Binary,_>(kept.as_bytes().as_slice())
                .bind::<BigInt,_>(decision.id.as_i64()).execute(connection)?;
        }
    }
    decision.publish(connection)
}

fn validate_merge(
    connection: &mut SqliteConnection,
    review: &FileMatchReview,
    kept: CatalogContentId,
    old: &[CatalogContentId],
) -> crate::Result<()> {
    if old.is_empty()
        || old.contains(&kept)
        || super::catalog_content::resolve_issued_id(connection, kept)? != kept
    {
        return Err(ReviewError::InvalidMerge.into());
    }
    let mut distinct = std::collections::BTreeSet::new();
    distinct.insert(kept);
    for issued in old {
        if !distinct.insert(*issued)
            || super::catalog_content::resolve_issued_id(connection, *issued)? != *issued
        {
            return Err(ReviewError::InvalidMerge.into());
        }
        if !review.conflicts.iter().any(|conflict| {
            conflict.candidate == *issued
                && review
                    .conflicts
                    .iter()
                    .any(|other| other.incoming == conflict.incoming && other.candidate == kept)
        }) {
            return Err(ReviewError::InvalidMerge.into());
        }
    }
    if review
        .conflicts
        .iter()
        .any(|conflict| !distinct.contains(&conflict.candidate))
    {
        return Err(ReviewError::InvalidMerge.into());
    }
    Ok(())
}

fn insert_evidence(
    connection: &mut SqliteConnection,
    decision: &DraftDecision,
    review: EvidenceDecision,
) -> crate::Result<()> {
    let conflict = review.conflict;
    match review.evidence {
        ReviewedEvidence::Hash {
            occurrence,
            digest_id,
            scope,
            role,
        } => {
            sql_query("INSERT INTO file_match_hash_decisions(decision_id,occurrence_id,candidate_content_uuid,evidence_occurrence_id,digest_id,scope,role,disposition) VALUES (?,?,?,?,?,?,?,?)")
                .bind::<BigInt,_>(decision.id.as_i64()).bind::<BigInt,_>(conflict.incoming.database_value()).bind::<Binary,_>(conflict.candidate.as_bytes().as_slice())
                .bind::<BigInt,_>(occurrence.database_value()).bind::<BigInt,_>(digest_id).bind::<Text,_>(scope.as_str())
                .bind::<Text,_>(role.as_str()).bind::<Text,_>(review.disposition.as_str()).execute(connection)?;
        }
        ReviewedEvidence::Size {
            occurrence,
            field,
            role,
        } => {
            sql_query("INSERT INTO file_match_size_decisions(decision_id,occurrence_id,candidate_content_uuid,evidence_occurrence_id,size_field,role,disposition) VALUES (?,?,?,?,?,?,?)")
                .bind::<BigInt,_>(decision.id.as_i64()).bind::<BigInt,_>(conflict.incoming.database_value()).bind::<Binary,_>(conflict.candidate.as_bytes().as_slice())
                .bind::<BigInt,_>(occurrence.database_value()).bind::<Text,_>(field.as_str()).bind::<Text,_>(role.as_str())
                .bind::<Text,_>(review.disposition.as_str()).execute(connection)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    #[test]
    fn unknown_issued_uuid_returns_a_typed_error() -> crate::Result<()> {
        let database = Database::in_memory()?;
        let unknown = CatalogContentId::generate();
        assert!(matches!(resolve_file_id(&database, unknown),
            Err(crate::Error::FileMatchReview(ReviewError::UnknownFile(id))) if id == unknown));
        Ok(())
    }

    #[test]
    fn incomplete_review_does_not_create_a_draft_or_terminal_marker() -> crate::Result<()> {
        let database = Database::in_memory()?;
        let review = FileMatchReview {
            rationale: "No source conflict was named".into(),
            conflicts: Vec::new(),
            evidence: Vec::new(),
            action: ReviewAction::KeepSeparate,
        };
        assert!(matches!(
            record_review(&database, &review),
            Err(crate::Error::FileMatchReview(ReviewError::EmptyReview))
        ));
        let mut connection = database.pool().get()?;
        let rows = sql_query("SELECT (SELECT count(*) FROM file_match_decisions) + (SELECT count(*) FROM file_match_decision_publications) AS count")
            .get_result::<Count>(&mut connection)?;
        assert_eq!(rows.count, 0);
        Ok(())
    }
}
