use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    AssetRole, CatalogKey, CatalogRecordRef, ExpectedEvidence, OccurrenceId,
    RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipEvidence,
    RelationshipExplanation, RelationshipOrigin, RelationshipReviewDecision, RelationshipType,
    SnapshotKey,
};
use crate::resolution::{EvidenceField, compare_expected_fields};
use serde::{Deserialize, Serialize};

const MAX_EXPANDED_MATCH_PAIRS: usize = 4096;
type RequirementPair = (usize, usize);
type CompressedAmbiguity = BTreeMap<RequirementPair, u8>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReconciliationStatus {
    Compatible,
    Candidate,
    Contradictory,
    Ambiguous,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedEvidenceReconciliation {
    pub status: ReconciliationStatus,
    pub agreements: Vec<crate::resolution::EvidenceField>,
    pub contradictions: Vec<crate::resolution::EvidenceField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedAssetRequirement {
    pub record: CatalogRecordRef,
    pub owner: CatalogRecordRef,
    /// Actual root-media owner when loaded from a catalog registry.
    pub media_entry_id: Option<OccurrenceId>,
    pub role: AssetRole,
    pub expected: ExpectedEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementSnapshot {
    pub catalog: CatalogKey,
    pub snapshot: SnapshotKey,
    pub requirements: Vec<ExpectedAssetRequirement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementReconciliation {
    pub left: Option<CatalogRecordRef>,
    pub right: Option<CatalogRecordRef>,
    pub left_expected: Option<ExpectedEvidence>,
    pub right_expected: Option<ExpectedEvidence>,
    pub status: ReconciliationStatus,
    pub evidence: ExpectedEvidenceReconciliation,
    pub relationship_evidence: Vec<RelationshipExplanation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogReconciliation {
    pub left_catalog: CatalogKey,
    pub right_catalog: CatalogKey,
    pub left_snapshot: SnapshotKey,
    pub right_snapshot: SnapshotKey,
    pub outcomes: Vec<RequirementReconciliation>,
}

#[must_use]
pub fn compare_expected_evidence(
    left: &ExpectedEvidence,
    right: &ExpectedEvidence,
) -> ExpectedEvidenceReconciliation {
    let mut result = ExpectedEvidenceReconciliation {
        status: ReconciliationStatus::Unknown,
        agreements: Vec::new(),
        contradictions: Vec::new(),
    };
    if left.scope == crate::domain::EvidenceScope::Unknown || left.scope != right.scope {
        return result;
    }
    let comparison = compare_expected_fields(left, right);
    result.agreements = comparison.agreements;
    result.contradictions = comparison.contradictions;

    if (left.scope == crate::domain::EvidenceScope::WholeAsset
        && result.contradictions.contains(&EvidenceField::Size))
        || result.contradictions.contains(&EvidenceField::Sha1)
    {
        result.status = ReconciliationStatus::Contradictory;
    } else if result.agreements.contains(&EvidenceField::Sha1) {
        result.status = ReconciliationStatus::Compatible;
    } else if result.contradictions.contains(&EvidenceField::Md5) {
        result.status = ReconciliationStatus::Contradictory;
    } else if result.agreements.contains(&EvidenceField::Md5) {
        result.status = ReconciliationStatus::Candidate;
    } else if result.contradictions.contains(&EvidenceField::Crc)
        || result.contradictions.contains(&EvidenceField::Size)
    {
        result.status = ReconciliationStatus::Contradictory;
    } else if result.agreements.contains(&EvidenceField::Crc) {
        result.status = ReconciliationStatus::Candidate;
    }
    result
}

/// Reconcile requirement facts without matching names, merging sets, or consulting inventory.
#[must_use]
pub fn reconcile_requirements(
    left: &RequirementSnapshot,
    right: &RequirementSnapshot,
    relationships: &[RelationshipExplanation],
) -> CatalogReconciliation {
    let native_records = left
        .requirements
        .iter()
        .chain(&right.requirements)
        .filter_map(|requirement| {
            requirement
                .media_entry_id
                .map(|id| ((&requirement.record.snapshot, id), &requirement.record))
        })
        .collect::<BTreeMap<_, _>>();
    let mut relationship_index = BTreeMap::<CatalogRecordRef, Vec<&RelationshipExplanation>>::new();
    for explanation in relationships {
        for endpoint in [&explanation.claim.subject, &explanation.claim.target] {
            let record = match endpoint {
                RelationshipEndpoint::CatalogRecord(record) => Some(record),
                RelationshipEndpoint::CatalogMediaEntry {
                    snapshot,
                    occurrence_id,
                } => native_records.get(&(snapshot, *occurrence_id)).copied(),
                _ => None,
            };
            if let Some(record) = record {
                relationship_index
                    .entry(record.clone())
                    .or_default()
                    .push(explanation);
            }
        }
    }
    let mut left_requirements = left.requirements.iter().collect::<Vec<_>>();
    let mut right_requirements = right.requirements.iter().collect::<Vec<_>>();
    left_requirements.sort_by(|a, b| a.record.cmp(&b.record));
    right_requirements.sort_by(|a, b| a.record.cmp(&b.record));

    let (pairs, compressed_ambiguity) = candidate_pairs(&left_requirements, &right_requirements);

    let mut outcomes = pairs
        .iter()
        .map(|(left_index, right_index)| {
            let mut outcome = pair_outcome(
                left_requirements[*left_index],
                right_requirements[*right_index],
                &relationship_index,
            );
            if compressed_ambiguity
                .get(&(*left_index, *right_index))
                .is_some_and(|strength| {
                    outcome.evidence.contradictions.is_empty()
                        && evidence_strength(&outcome.evidence) <= *strength
                })
            {
                outcome.status = ReconciliationStatus::Ambiguous;
            }
            outcome
        })
        .collect::<Vec<_>>();
    mark_ambiguous(&mut outcomes);

    let paired_left = pairs
        .iter()
        .map(|(index, _)| *index)
        .collect::<BTreeSet<_>>();
    let paired_right = pairs
        .iter()
        .map(|(_, index)| *index)
        .collect::<BTreeSet<_>>();
    for (index, requirement) in left_requirements.iter().enumerate() {
        if !paired_left.contains(&index) {
            outcomes.push(unmatched(requirement, true, &relationship_index));
        }
    }
    for (index, requirement) in right_requirements.iter().enumerate() {
        if !paired_right.contains(&index) {
            outcomes.push(unmatched(requirement, false, &relationship_index));
        }
    }
    outcomes.sort_by(|a, b| a.left.cmp(&b.left).then_with(|| a.right.cmp(&b.right)));

    CatalogReconciliation {
        left_catalog: left.catalog.clone(),
        right_catalog: right.catalog.clone(),
        left_snapshot: left.snapshot.clone(),
        right_snapshot: right.snapshot.clone(),
        outcomes,
    }
}

fn candidate_pairs(
    left_requirements: &[&ExpectedAssetRequirement],
    right_requirements: &[&ExpectedAssetRequirement],
) -> (BTreeSet<RequirementPair>, CompressedAmbiguity) {
    let mut left_index = BTreeMap::<EvidenceIndexKey, Vec<usize>>::new();
    for (index, requirement) in left_requirements.iter().enumerate() {
        for key in index_keys(requirement) {
            left_index.entry(key).or_default().push(index);
        }
    }
    let mut right_index = BTreeMap::<EvidenceIndexKey, Vec<usize>>::new();
    for (index, requirement) in right_requirements.iter().enumerate() {
        for key in index_keys(requirement) {
            right_index.entry(key).or_default().push(index);
        }
    }

    let mut pairs = BTreeSet::new();
    let mut compressed_ambiguity = CompressedAmbiguity::new();
    for (key, left_matches) in left_index {
        if let Some(right_matches) = right_index.get(&key) {
            let bucket_pairs = left_matches.len().saturating_mul(right_matches.len());
            if bucket_pairs > MAX_EXPANDED_MATCH_PAIRS {
                let strength = fingerprint_strength(&key.fingerprint);
                if let Some(first_left) = left_matches.first() {
                    for right_index in right_matches {
                        add_compressed_pair(
                            &mut pairs,
                            &mut compressed_ambiguity,
                            (*first_left, *right_index),
                            strength,
                        );
                    }
                }
                if let Some(first_right) = right_matches.first() {
                    for left_index in left_matches.iter().skip(1) {
                        add_compressed_pair(
                            &mut pairs,
                            &mut compressed_ambiguity,
                            (*left_index, *first_right),
                            strength,
                        );
                    }
                }
            } else {
                pairs.extend(left_matches.iter().flat_map(|left_index| {
                    right_matches
                        .iter()
                        .map(move |right_index| (*left_index, *right_index))
                }));
            }
        }
    }
    (pairs, compressed_ambiguity)
}

fn add_compressed_pair(
    pairs: &mut BTreeSet<RequirementPair>,
    compressed_ambiguity: &mut CompressedAmbiguity,
    pair: RequirementPair,
    strength: u8,
) {
    pairs.insert(pair);
    compressed_ambiguity
        .entry(pair)
        .and_modify(|current| *current = (*current).max(strength))
        .or_insert(strength);
}

impl RequirementReconciliation {
    /// Turn a possible match into a reversible relationship candidate for the MAMEC-37 store.
    #[must_use]
    pub fn relationship_candidate(&self) -> Option<RelationshipClaim> {
        if !matches!(
            self.status,
            ReconciliationStatus::Compatible | ReconciliationStatus::Candidate
        ) {
            return None;
        }
        let (Some(left), Some(right)) = (&self.left, &self.right) else {
            return None;
        };
        let (Some(left_expected), Some(right_expected)) =
            (&self.left_expected, &self.right_expected)
        else {
            return None;
        };
        if left_expected.scope != crate::domain::EvidenceScope::WholeAsset
            || right_expected.scope != crate::domain::EvidenceScope::WholeAsset
            || !self.evidence.contradictions.is_empty()
        {
            return None;
        }
        let supporting_assertions = self
            .relationship_evidence
            .iter()
            .filter(|item| {
                matches!(
                    item.claim.origin,
                    RelationshipOrigin::SourceAssertion { .. }
                ) && item.claim.relation_type == RelationshipType::ExactContentIdentity
                    && endpoints_connect_pair(&item.claim.subject, &item.claim.target, left, right)
                    && item.latest_review.as_ref().is_none_or(|review| {
                        !matches!(
                            review.decision,
                            RelationshipReviewDecision::Rejected
                                | RelationshipReviewDecision::Withdrawn
                                | RelationshipReviewDecision::Superseded
                        )
                    })
            })
            .map(|item| item.assertion_key.clone())
            .collect::<Vec<RelationshipAssertionKey>>();
        Some(RelationshipClaim {
            relation_type: RelationshipType::ExactContentIdentity,
            subject: RelationshipEndpoint::CatalogRecord(left.clone()),
            target: RelationshipEndpoint::CatalogRecord(right.clone()),
            origin: RelationshipOrigin::DerivedCandidate {
                rule_version: "catalog-evidence-reconciliation-v1".to_owned(),
                supporting_assertions,
            },
            evidence: RelationshipEvidence::CatalogComparison {
                status: self.status,
                agreements: self.evidence.agreements.clone(),
                contradictions: self.evidence.contradictions.clone(),
            },
        })
    }
}

fn endpoints_connect_pair(
    subject: &RelationshipEndpoint,
    target: &RelationshipEndpoint,
    left: &CatalogRecordRef,
    right: &CatalogRecordRef,
) -> bool {
    matches!(
        (subject, target),
        (RelationshipEndpoint::CatalogRecord(a), RelationshipEndpoint::CatalogRecord(b))
            if (a == left && b == right) || (a == right && b == left)
    )
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EvidenceIndexKey {
    role: AssetRole,
    scope: crate::domain::EvidenceScope,
    fingerprint: EvidenceFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EvidenceFingerprint {
    Sha1([u8; 20]),
    Md5([u8; 16]),
    Crc([u8; 4]),
}

fn index_keys(requirement: &ExpectedAssetRequirement) -> Vec<EvidenceIndexKey> {
    let expected = &requirement.expected;
    if expected.scope == crate::domain::EvidenceScope::Unknown
        || requirement.role == AssetRole::Other
    {
        return Vec::new();
    }
    let mut keys = Vec::new();
    if let Some(digest) = expected.sha1 {
        keys.push(EvidenceFingerprint::Sha1(digest).into_key(requirement));
    }
    if let Some(digest) = expected.md5 {
        keys.push(EvidenceFingerprint::Md5(digest.0).into_key(requirement));
    }
    if let Some(digest) = expected.crc {
        keys.push(EvidenceFingerprint::Crc(digest.0).into_key(requirement));
    }
    keys
}

impl EvidenceFingerprint {
    const fn into_key(self, requirement: &ExpectedAssetRequirement) -> EvidenceIndexKey {
        EvidenceIndexKey {
            role: requirement.role,
            scope: requirement.expected.scope,
            fingerprint: self,
        }
    }
}

fn pair_outcome(
    left: &ExpectedAssetRequirement,
    right: &ExpectedAssetRequirement,
    relationships: &BTreeMap<CatalogRecordRef, Vec<&RelationshipExplanation>>,
) -> RequirementReconciliation {
    let evidence = compare_expected_evidence(&left.expected, &right.expected);
    let status = if left.role == right.role {
        evidence.status
    } else {
        ReconciliationStatus::Unknown
    };
    RequirementReconciliation {
        left: Some(left.record.clone()),
        right: Some(right.record.clone()),
        left_expected: Some(left.expected.clone()),
        right_expected: Some(right.expected.clone()),
        status,
        evidence,
        relationship_evidence: related_assertions(
            relationships,
            &[&left.record, &left.owner, &right.record, &right.owner],
        ),
    }
}

fn unmatched(
    requirement: &ExpectedAssetRequirement,
    is_left: bool,
    relationships: &BTreeMap<CatalogRecordRef, Vec<&RelationshipExplanation>>,
) -> RequirementReconciliation {
    RequirementReconciliation {
        left: is_left.then(|| requirement.record.clone()),
        right: (!is_left).then(|| requirement.record.clone()),
        left_expected: is_left.then(|| requirement.expected.clone()),
        right_expected: (!is_left).then(|| requirement.expected.clone()),
        status: ReconciliationStatus::Unknown,
        evidence: ExpectedEvidenceReconciliation {
            status: ReconciliationStatus::Unknown,
            agreements: Vec::new(),
            contradictions: Vec::new(),
        },
        relationship_evidence: related_assertions(
            relationships,
            &[&requirement.record, &requirement.owner],
        ),
    }
}

fn related_assertions(
    relationships: &BTreeMap<CatalogRecordRef, Vec<&RelationshipExplanation>>,
    records: &[&CatalogRecordRef],
) -> Vec<RelationshipExplanation> {
    let mut related = Vec::new();
    for record in records {
        if let Some(assertions) = relationships.get(*record) {
            related.extend(assertions.iter().copied());
        }
        if record.owner_set_id.is_some() {
            let mut logical_selector = (*record).clone();
            logical_selector.owner_set_id = None;
            if let Some(assertions) = relationships.get(&logical_selector) {
                related.extend(assertions.iter().copied());
            }
        }
    }
    related.sort_by(|a, b| a.assertion_key.cmp(&b.assertion_key));
    related.dedup_by(|a, b| a.assertion_key == b.assertion_key);
    related.into_iter().cloned().collect()
}

fn mark_ambiguous(outcomes: &mut [RequirementReconciliation]) {
    let mut left_counts = BTreeMap::<CatalogRecordRef, (u8, usize)>::new();
    let mut right_counts = BTreeMap::<CatalogRecordRef, (u8, usize)>::new();
    for outcome in outcomes.iter().filter(|outcome| {
        matches!(
            outcome.status,
            ReconciliationStatus::Compatible | ReconciliationStatus::Candidate
        ) && outcome.evidence.contradictions.is_empty()
    }) {
        let strength = evidence_strength(&outcome.evidence);
        for (record, counts) in [
            (&outcome.left, &mut left_counts),
            (&outcome.right, &mut right_counts),
        ] {
            if let Some(record) = record {
                let entry = counts.entry(record.clone()).or_default();
                if strength > entry.0 {
                    *entry = (strength, 1);
                } else if strength == entry.0 {
                    entry.1 += 1;
                }
            }
        }
    }
    for outcome in outcomes.iter_mut().filter(|outcome| {
        matches!(
            outcome.status,
            ReconciliationStatus::Compatible | ReconciliationStatus::Candidate
        ) && outcome.evidence.contradictions.is_empty()
    }) {
        let strength = evidence_strength(&outcome.evidence);
        let left_ambiguous = outcome.left.as_ref().is_some_and(|record| {
            left_counts
                .get(record)
                .is_some_and(|(best, count)| *best == strength && *count > 1)
        });
        let right_ambiguous = outcome.right.as_ref().is_some_and(|record| {
            right_counts
                .get(record)
                .is_some_and(|(best, count)| *best == strength && *count > 1)
        });
        if left_ambiguous || right_ambiguous {
            outcome.status = ReconciliationStatus::Ambiguous;
        }
    }
}

fn evidence_strength(evidence: &ExpectedEvidenceReconciliation) -> u8 {
    if evidence.agreements.contains(&EvidenceField::Sha1) {
        3
    } else if evidence.agreements.contains(&EvidenceField::Md5) {
        2
    } else {
        u8::from(evidence.agreements.contains(&EvidenceField::Crc))
    }
}

const fn fingerprint_strength(fingerprint: &EvidenceFingerprint) -> u8 {
    match fingerprint {
        EvidenceFingerprint::Sha1(_) => 3,
        EvidenceFingerprint::Md5(_) => 2,
        EvidenceFingerprint::Crc(_) => 1,
    }
}
