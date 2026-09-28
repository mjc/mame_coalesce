//! Pure, deterministic matching of catalog requirements to source observations.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::{DatRom, EvidenceProvenance, EvidenceScope, SourceFile, SourceRoot};

pub use crate::domain::MatchingPolicy;

/// Per-category cap for evidence assessments and duplicate copies in one resolution result.
pub const MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStrength {
    Sha1,
    Md5,
    CrcAndSize,
}

impl MatchStrength {
    const fn rank(self) -> u8 {
        match self {
            Self::Sha1 => 3,
            Self::Md5 => 2,
            Self::CrcAndSize => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceField {
    Sha1,
    Md5,
    Crc,
    Size,
}

impl EvidenceField {
    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AvailableEvidence(u8);

impl AvailableEvidence {
    const fn insert(&mut self, field: EvidenceField) {
        self.0 |= field.bit();
    }

    const fn contains(self, field: EvidenceField) -> bool {
        self.0 & field.bit() != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingReason {
    NoExpectedContentEvidence,
    UnsupportedExpectedScope,
    NoComparableObservedEvidence,
    InsufficientEvidence,
    NoMatchingSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssessment {
    pub source: SourceFile,
    pub strength: Option<MatchStrength>,
    pub agreements: Vec<EvidenceField>,
    pub conflicts: Vec<EvidenceField>,
    pub comparable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Matched {
        selected: Box<SourceFile>,
        strength: MatchStrength,
        equivalent_copies: Vec<SourceFile>,
        assessments: Vec<SourceAssessment>,
        /// Matching duplicate sources omitted by the global detail budget.
        #[serde(default)]
        omitted_equivalent_copies: usize,
        /// Per-source evidence records omitted by the global detail budget.
        #[serde(default)]
        omitted_assessments: usize,
    },
    Ambiguous {
        candidates: Vec<SourceAssessment>,
        /// Candidate records omitted by the global detail budget.
        #[serde(default)]
        omitted_candidates: usize,
    },
    Conflicting {
        candidates: Vec<SourceAssessment>,
        /// Candidate records omitted by the global detail budget.
        #[serde(default)]
        omitted_candidates: usize,
    },
    Missing {
        reason: MissingReason,
        assessments: Vec<SourceAssessment>,
        /// Per-source evidence records omitted by the global detail budget.
        #[serde(default)]
        omitted_assessments: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementResolution {
    pub requirement: DatRom,
    pub status: ResolutionStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SourceIndex(usize);

#[derive(Clone)]
struct CandidateSources<'a> {
    postings: [&'a [SourceIndex]; 3],
}

impl Iterator for CandidateSources<'_> {
    type Item = SourceIndex;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self
            .postings
            .iter()
            .filter_map(|posting| posting.first())
            .min()
            .copied()?;
        for posting in &mut self.postings {
            if posting.first() == Some(&next) {
                *posting = &posting[1..];
            }
        }
        Some(next)
    }
}

struct EvidenceIndex<'a> {
    sha1: HashMap<&'a [u8], Vec<SourceIndex>>,
    md5: HashMap<&'a [u8], Vec<SourceIndex>>,
    crc: HashMap<&'a [u8], Vec<SourceIndex>>,
    size: HashMap<u64, Vec<SourceIndex>>,
    available: AvailableEvidence,
}

impl<'a> EvidenceIndex<'a> {
    fn new(sources: &[&'a SourceFile]) -> Self {
        let mut index = Self {
            sha1: HashMap::new(),
            md5: HashMap::new(),
            crc: HashMap::new(),
            size: HashMap::new(),
            available: AvailableEvidence::default(),
        };

        for (source_index, source) in sources.iter().enumerate() {
            if !has_comparable_evidence(source) {
                continue;
            }
            let observed = &source.observed;
            if let Some(sha1) = observed.sha1.as_ref() {
                index
                    .sha1
                    .entry(sha1.as_slice())
                    .or_default()
                    .push(SourceIndex(source_index));
                index.available.insert(EvidenceField::Sha1);
            }
            if let Some(md5) = observed.md5.as_ref() {
                index
                    .md5
                    .entry(md5.0.as_slice())
                    .or_default()
                    .push(SourceIndex(source_index));
                index.available.insert(EvidenceField::Md5);
            }
            if let Some(crc) = observed.crc.as_ref() {
                index
                    .crc
                    .entry(crc.0.as_slice())
                    .or_default()
                    .push(SourceIndex(source_index));
                index.available.insert(EvidenceField::Crc);
            }
            if observed.size.is_some() {
                index.available.insert(EvidenceField::Size);
                if let Some(size) = observed.size {
                    index
                        .size
                        .entry(size)
                        .or_default()
                        .push(SourceIndex(source_index));
                }
            }
        }
        index
    }

    fn candidates(&self, expected: &crate::domain::ExpectedEvidence) -> CandidateSources<'_> {
        CandidateSources {
            postings: [
                expected
                    .sha1
                    .as_ref()
                    .and_then(|sha1| self.sha1.get(sha1.as_slice()))
                    .map_or(&[], Vec::as_slice),
                expected
                    .md5
                    .as_ref()
                    .and_then(|md5| self.md5.get(md5.0.as_slice()))
                    .map_or(&[], Vec::as_slice),
                expected
                    .crc
                    .as_ref()
                    .and_then(|crc| self.crc.get(crc.0.as_slice()))
                    .map_or(&[], Vec::as_slice),
            ],
        }
    }

    const fn has_comparable_field(&self, expected: &crate::domain::ExpectedEvidence) -> bool {
        expected.sha1.is_some() && self.available.contains(EvidenceField::Sha1)
            || expected.md5.is_some() && self.available.contains(EvidenceField::Md5)
            || expected.crc.is_some() && self.available.contains(EvidenceField::Crc)
            || expected.size.is_some() && self.available.contains(EvidenceField::Size)
    }
}

const fn has_comparable_evidence(source: &SourceFile) -> bool {
    matches!(source.observed.scope, EvidenceScope::WholeAsset)
        && matches!(
            source.observed.provenance,
            EvidenceProvenance::Computed | EvidenceProvenance::SourceDeclared
        )
}

fn has_uncontradicted_size_match(
    expected: &crate::domain::ExpectedEvidence,
    sources: &[&SourceFile],
    evidence_index: &EvidenceIndex<'_>,
) -> bool {
    let Some(size) = expected.size else {
        return false;
    };
    evidence_index.size.get(&size).is_some_and(|candidates| {
        candidates.iter().any(|&SourceIndex(index)| {
            let observed = &sources[index].observed;
            expected
                .sha1
                .as_ref()
                .zip(observed.sha1.as_ref())
                .is_none_or(|(expected, observed)| expected == observed)
                && expected
                    .md5
                    .as_ref()
                    .zip(observed.md5.as_ref())
                    .is_none_or(|(expected, observed)| expected == observed)
                && expected
                    .crc
                    .as_ref()
                    .zip(observed.crc.as_ref())
                    .is_none_or(|(expected, observed)| expected == observed)
        })
    })
}

/// Resolve one catalog's requirements against one already-selected source root.
///
/// The result owns the requirement, candidates and selected source so callers may retain it
/// independently of database rows or scanner buffers. No presentation or exit policy is applied.
#[must_use]
pub fn resolve(
    dat_roms: &[DatRom],
    source_files: &[SourceFile],
    catalog_name: &str,
    source_root: &SourceRoot,
    policy: MatchingPolicy,
) -> Vec<RequirementResolution> {
    let mut requirements = dat_roms
        .iter()
        .filter(|rom| rom.catalog_name() == catalog_name)
        .collect::<Vec<_>>();
    requirements.sort();

    let mut sources = source_files
        .iter()
        .filter(|source| source_in_root(source, source_root))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| source_order(left, right));

    match policy {
        MatchingPolicy::EvidenceAware => {
            let evidence_index = EvidenceIndex::new(&sources);
            let mut assessment_budget = MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND;
            let mut copy_budget = MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND;
            requirements
                .into_iter()
                .map(|requirement| {
                    let status = resolve_one(
                        requirement,
                        &sources,
                        &evidence_index,
                        &mut assessment_budget,
                        &mut copy_budget,
                    );
                    RequirementResolution {
                        requirement: requirement.clone(),
                        status,
                    }
                })
                .collect()
        }
        MatchingPolicy::Sha1Compatibility => {
            // Compatibility matching is SHA-1-only, but source scope is still authoritative.
            let mut sha1_sources: HashMap<&[u8], Vec<&SourceFile>> = HashMap::new();
            for source in &sources {
                if source.observed.scope == EvidenceScope::WholeAsset
                    && let Some(sha1) = source.observed.sha1.as_ref()
                {
                    sha1_sources
                        .entry(sha1.as_slice())
                        .or_default()
                        .push(*source);
                }
            }
            let mut copy_budget = MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND;
            requirements
                .into_iter()
                .map(|requirement| {
                    let status =
                        resolve_sha1_compatibility(requirement, &sha1_sources, &mut copy_budget);
                    RequirementResolution {
                        requirement: requirement.clone(),
                        status,
                    }
                })
                .collect()
        }
    }
}

fn retain_count(total: usize, budget: &mut usize) -> (usize, usize) {
    let retained = total.min(*budget);
    *budget -= retained;
    (retained, total - retained)
}

#[derive(Clone, Copy)]
struct ResolutionContext<'a, 'sources> {
    expected: &'a crate::domain::ExpectedEvidence,
    sources: &'a [&'sources SourceFile],
    evidence_index: &'a EvidenceIndex<'sources>,
}

#[derive(Clone, Copy)]
struct BestMatch {
    selected_index: usize,
    strength: MatchStrength,
    strongest_count: usize,
    candidate_count: usize,
}

fn resolve_one(
    requirement: &DatRom,
    sources: &[&SourceFile],
    evidence_index: &EvidenceIndex<'_>,
    assessment_budget: &mut usize,
    copy_budget: &mut usize,
) -> ResolutionStatus {
    let expected = &requirement.expected;
    if expected.scope != EvidenceScope::WholeAsset {
        return ResolutionStatus::Missing {
            reason: MissingReason::UnsupportedExpectedScope,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    }
    let has_expected_digest =
        expected.sha1.is_some() || expected.md5.is_some() || expected.crc.is_some();
    if !has_expected_digest {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoExpectedContentEvidence,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    }

    let mut candidate_count = 0;
    let mut conflicting_count = 0;
    let mut digest_agrees = false;
    let mut strongest_count = 0;
    let mut best: Option<(usize, MatchStrength)> = None;
    for SourceIndex(index) in evidence_index.candidates(expected) {
        candidate_count += 1;
        let facts = assess_evidence(expected, sources[index]);
        conflicting_count += usize::from(facts.is_conflicting());
        digest_agrees |= facts.agreements.0
            & (EvidenceField::Sha1.bit() | EvidenceField::Md5.bit() | EvidenceField::Crc.bit())
            != 0;
        if let Some(strength) = facts.strength {
            match best {
                Some((_, current)) if current.rank() > strength.rank() => {}
                Some((_, current)) if current == strength => strongest_count += 1,
                _ => {
                    best = Some((index, strength));
                    strongest_count = 1;
                }
            }
        }
    }

    let context = ResolutionContext {
        expected,
        sources,
        evidence_index,
    };
    let Some((selected_index, strength)) = best else {
        return resolve_without_match(
            context,
            candidate_count,
            conflicting_count,
            digest_agrees,
            assessment_budget,
        );
    };

    resolve_best_match(
        context,
        BestMatch {
            selected_index,
            strength,
            strongest_count,
            candidate_count,
        },
        assessment_budget,
        copy_budget,
    )
}

fn resolve_without_match(
    context: ResolutionContext<'_, '_>,
    candidate_count: usize,
    conflicting_count: usize,
    digest_agrees: bool,
    assessment_budget: &mut usize,
) -> ResolutionStatus {
    let ResolutionContext {
        expected,
        sources,
        evidence_index,
    } = context;
    if conflicting_count != 0 {
        let (retained, omitted_candidates) = retain_count(conflicting_count, assessment_budget);
        let candidates = evidence_index
            .candidates(expected)
            .filter(|source_index| {
                let SourceIndex(index) = *source_index;
                assess_evidence(expected, sources[index]).is_conflicting()
            })
            .map(|SourceIndex(index)| {
                assess_evidence(expected, sources[index]).into_assessment(sources[index])
            })
            .take(retained)
            .collect();
        return ResolutionStatus::Conflicting {
            candidates,
            omitted_candidates,
        };
    }

    let size_agrees = has_uncontradicted_size_match(expected, sources, evidence_index);
    let reason = if candidate_count == 0 {
        if sources.is_empty() {
            MissingReason::NoMatchingSource
        } else if size_agrees {
            MissingReason::InsufficientEvidence
        } else if evidence_index.has_comparable_field(expected) {
            MissingReason::NoMatchingSource
        } else {
            MissingReason::NoComparableObservedEvidence
        }
    } else if digest_agrees {
        MissingReason::InsufficientEvidence
    } else if !evidence_index.has_comparable_field(expected) {
        MissingReason::NoComparableObservedEvidence
    } else {
        MissingReason::NoMatchingSource
    };
    let (retained, omitted_assessments) = retain_count(candidate_count, assessment_budget);
    let assessments = evidence_index
        .candidates(expected)
        .take(retained)
        .map(|SourceIndex(index)| {
            assess_evidence(expected, sources[index]).into_assessment(sources[index])
        })
        .collect();
    ResolutionStatus::Missing {
        reason,
        assessments,
        omitted_assessments,
    }
}

fn resolve_best_match(
    context: ResolutionContext<'_, '_>,
    best: BestMatch,
    assessment_budget: &mut usize,
    copy_budget: &mut usize,
) -> ResolutionStatus {
    let ResolutionContext {
        expected,
        sources,
        evidence_index,
    } = context;
    let BestMatch {
        selected_index,
        strength,
        strongest_count,
        candidate_count,
    } = best;
    let strongest = evidence_index
        .candidates(expected)
        .filter_map(|SourceIndex(index)| {
            (assess_evidence(expected, sources[index]).strength == Some(strength))
                .then_some(sources[index])
        });
    if strongest_count > 1 && !tied_candidates_are_equivalent(strength, strongest) {
        let (retained, omitted_candidates) = retain_count(strongest_count, assessment_budget);
        let candidates = evidence_index
            .candidates(expected)
            .filter(|source_index| {
                let SourceIndex(index) = *source_index;
                assess_evidence(expected, sources[index]).strength == Some(strength)
            })
            .map(|SourceIndex(index)| {
                assess_evidence(expected, sources[index]).into_assessment(sources[index])
            })
            .take(retained)
            .collect();
        return ResolutionStatus::Ambiguous {
            candidates,
            omitted_candidates,
        };
    }

    let (retained_assessments, omitted_assessments) =
        retain_count(candidate_count, assessment_budget);
    let assessments = evidence_index
        .candidates(expected)
        .take(retained_assessments)
        .map(|SourceIndex(index)| {
            assess_evidence(expected, sources[index]).into_assessment(sources[index])
        })
        .collect();
    let (retained_copies, omitted_equivalent_copies) = retain_count(strongest_count, copy_budget);
    let equivalent_copies = evidence_index
        .candidates(expected)
        .filter(|source_index| {
            let SourceIndex(index) = *source_index;
            assess_evidence(expected, sources[index]).strength == Some(strength)
        })
        .map(|SourceIndex(index)| sources[index].clone())
        .take(retained_copies)
        .collect();
    ResolutionStatus::Matched {
        selected: Box::new(sources[selected_index].clone()),
        strength,
        equivalent_copies,
        assessments,
        omitted_equivalent_copies,
        omitted_assessments,
    }
}

fn tied_candidates_are_equivalent<'a>(
    strength: MatchStrength,
    candidates: impl Iterator<Item = &'a SourceFile> + Clone,
) -> bool {
    let consistent = tied_candidates_have_consistent_evidence(candidates.clone());
    match strength {
        MatchStrength::Sha1 => consistent,
        MatchStrength::Md5 | MatchStrength::CrcAndSize => {
            consistent && tied_candidates_share_sha1(candidates)
        }
    }
}

fn tied_candidates_have_consistent_evidence<'a>(
    candidates: impl Iterator<Item = &'a SourceFile> + Clone,
) -> bool {
    evidence_is_consistent(
        candidates
            .clone()
            .map(|candidate| candidate.observed.size.as_ref()),
    ) && evidence_is_consistent(
        candidates
            .clone()
            .map(|candidate| candidate.observed.crc.as_ref()),
    ) && evidence_is_consistent(
        candidates
            .clone()
            .map(|candidate| candidate.observed.md5.as_ref()),
    ) && evidence_is_consistent(
        candidates
            .clone()
            .map(|candidate| candidate.observed.sha1.as_ref()),
    ) && evidence_is_consistent(candidates.map(|candidate| Some(&candidate.observed.xxh3)))
}

fn evidence_is_consistent<'a, T: PartialEq + 'a>(
    values: impl IntoIterator<Item = Option<&'a T>>,
) -> bool {
    let mut established = None;
    for value in values.into_iter().flatten() {
        if let Some(established) = established {
            if established != value {
                return false;
            }
        } else {
            established = Some(value);
        }
    }
    true
}

fn tied_candidates_share_sha1<'a>(candidates: impl Iterator<Item = &'a SourceFile>) -> bool {
    let mut sha1s = candidates.map(|candidate| candidate.observed.sha1.as_ref());
    matches!(sha1s.next(), Some(Some(first)) if sha1s.all(|sha1| sha1 == Some(first)))
}

fn resolve_sha1_compatibility(
    requirement: &DatRom,
    sha1_sources: &HashMap<&[u8], Vec<&SourceFile>>,
    copy_budget: &mut usize,
) -> ResolutionStatus {
    if requirement.expected.scope != EvidenceScope::WholeAsset {
        return ResolutionStatus::Missing {
            reason: MissingReason::UnsupportedExpectedScope,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    }
    let Some(expected_sha1) = requirement.sha1() else {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoExpectedContentEvidence,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    };
    let Some(copies) = sha1_sources.get(expected_sha1.as_slice()) else {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoMatchingSource,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    };
    let Some(selected) = copies.first() else {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoMatchingSource,
            assessments: Vec::new(),
            omitted_assessments: 0,
        };
    };
    let (retained, omitted_equivalent_copies) = retain_count(copies.len(), copy_budget);
    ResolutionStatus::Matched {
        selected: Box::new((**selected).clone()),
        strength: MatchStrength::Sha1,
        equivalent_copies: copies
            .iter()
            .take(retained)
            .map(|source| (**source).clone())
            .collect(),
        assessments: Vec::new(),
        omitted_equivalent_copies,
        omitted_assessments: 0,
    }
}

struct AssessmentFacts {
    strength: Option<MatchStrength>,
    agreements: AvailableEvidence,
    conflicts: AvailableEvidence,
    comparable: bool,
}

impl AssessmentFacts {
    const fn is_conflicting(&self) -> bool {
        self.conflicts.0 != 0
            && self.agreements.0
                & (EvidenceField::Sha1.bit() | EvidenceField::Md5.bit() | EvidenceField::Crc.bit())
                != 0
    }

    fn into_assessment(self, source: &SourceFile) -> SourceAssessment {
        const FIELDS: [EvidenceField; 4] = [
            EvidenceField::Sha1,
            EvidenceField::Md5,
            EvidenceField::Crc,
            EvidenceField::Size,
        ];
        SourceAssessment {
            source: source.clone(),
            strength: self.strength,
            agreements: FIELDS
                .into_iter()
                .filter(|field| self.agreements.contains(*field))
                .collect(),
            conflicts: FIELDS
                .into_iter()
                .filter(|field| self.conflicts.contains(*field))
                .collect(),
            comparable: self.comparable,
        }
    }
}

fn assess_evidence(
    expected: &crate::domain::ExpectedEvidence,
    source: &SourceFile,
) -> AssessmentFacts {
    let observed = &source.observed;
    let supported = observed.scope == EvidenceScope::WholeAsset
        && matches!(
            observed.provenance,
            EvidenceProvenance::Computed | EvidenceProvenance::SourceDeclared
        );
    if !supported {
        return AssessmentFacts {
            strength: None,
            agreements: AvailableEvidence::default(),
            conflicts: AvailableEvidence::default(),
            comparable: false,
        };
    }

    let mut conflicts = AvailableEvidence::default();
    let sha1_equal = compare(
        expected.sha1,
        observed.sha1,
        EvidenceField::Sha1,
        &mut conflicts,
    );
    let md5_equal = compare(
        expected.md5,
        observed.md5,
        EvidenceField::Md5,
        &mut conflicts,
    );
    let crc_equal = compare(
        expected.crc,
        observed.crc,
        EvidenceField::Crc,
        &mut conflicts,
    );
    let size_equal = compare(
        expected.size,
        observed.size,
        EvidenceField::Size,
        &mut conflicts,
    );
    let comparable = conflicts.0 != 0
        || expected.sha1.is_some() && observed.sha1.is_some()
        || expected.md5.is_some() && observed.md5.is_some()
        || expected.crc.is_some() && observed.crc.is_some()
        || expected.size.is_some() && observed.size.is_some();
    let mut agreements = AvailableEvidence::default();
    if sha1_equal {
        agreements.insert(EvidenceField::Sha1);
    }
    if md5_equal {
        agreements.insert(EvidenceField::Md5);
    }
    if crc_equal {
        agreements.insert(EvidenceField::Crc);
    }
    if size_equal {
        agreements.insert(EvidenceField::Size);
    }

    let strength = if sha1_equal {
        Some(MatchStrength::Sha1)
    } else if md5_equal {
        Some(MatchStrength::Md5)
    } else if crc_equal && size_equal {
        Some(MatchStrength::CrcAndSize)
    } else {
        None
    };

    // A contradictory stronger digest vetoes fallback to weaker evidence. Once stronger
    // evidence matches, retain lower-priority contradictions as explanation, not vetoes.
    let strength = match strength {
        Some(MatchStrength::Sha1) => Some(MatchStrength::Sha1),
        Some(MatchStrength::Md5) if !conflicts.contains(EvidenceField::Sha1) => {
            Some(MatchStrength::Md5)
        }
        Some(MatchStrength::CrcAndSize)
            if !conflicts.contains(EvidenceField::Sha1)
                && !conflicts.contains(EvidenceField::Md5) =>
        {
            Some(MatchStrength::CrcAndSize)
        }
        Some(_) | None => None,
    };

    AssessmentFacts {
        strength,
        agreements,
        conflicts,
        comparable,
    }
}

fn compare<T: PartialEq>(
    expected: Option<T>,
    observed: Option<T>,
    field: EvidenceField,
    conflicts: &mut AvailableEvidence,
) -> bool {
    match (expected, observed) {
        (Some(expected), Some(observed)) if expected == observed => true,
        (Some(_), Some(_)) => {
            conflicts.insert(field);
            false
        }
        _ => false,
    }
}

fn source_order(left: &SourceFile, right: &SourceFile) -> std::cmp::Ordering {
    left.location
        .priority()
        .cmp(&right.location.priority())
        .then_with(|| left.location.path().cmp(right.location.path()))
        .then_with(|| {
            left.location
                .member_name()
                .cmp(&right.location.member_name())
        })
        .then_with(|| left.location.cmp(&right.location))
        .then_with(|| left.source_root.cmp(&right.source_root))
        .then_with(|| left.observed.cmp(&right.observed))
        .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        .then_with(|| left.scan_run.cmp(&right.scan_run))
        .then_with(|| left.scan_provenance.cmp(&right.scan_provenance))
}

fn source_in_root(source: &SourceFile, source_root: &SourceRoot) -> bool {
    let source_root = Utf8Path::new(source_root.as_str());
    Utf8Path::new(source.location.path()).starts_with(source_root)
        || Utf8Path::new(source.source_root.as_str()) == source_root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AssetRole, CatalogKey, ExpectedEvidence, ObservedContent, RequirementKey, SetKey,
        SetMetadata, SourceLocation,
    };
    use crate::hashes::Sha1Digest;

    fn requirement(mut expected: ExpectedEvidence) -> DatRom {
        if expected.scope == EvidenceScope::Unknown {
            expected.scope = EvidenceScope::WholeAsset;
        }
        DatRom {
            catalog_name: "catalog".to_owned(),
            key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "set"), "game.rom"),
            parent_name: None,
            set_metadata: SetMetadata::default(),
            role: AssetRole::Rom,
            component_order: Some(0),
            expected,
        }
    }

    fn source(path: &str, observed: ObservedContent) -> SourceFile {
        SourceFile {
            source_root: SourceRoot::new("/roms"),
            location: SourceLocation::BareFile {
                path: path.to_owned(),
            },
            observed,
            fingerprint: None,
            scan_run: None,
            scan_provenance: None,
        }
    }

    fn computed(
        size: Option<u64>,
        crc: Option<[u8; 4]>,
        md5: Option<[u8; 16]>,
        sha1: Option<Sha1Digest>,
    ) -> ObservedContent {
        ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size,
            crc: crc.map(crate::domain::Crc32Digest),
            md5: md5.map(crate::domain::Md5Digest),
            sha1,
            xxh3: [0; 8],
        }
    }

    fn result(status: &ResolutionStatus) -> &ResolutionStatus {
        status
    }

    #[test]
    fn compatibility_policy_keeps_sha1_authoritative_over_conflicting_metadata() {
        let rom = requirement(ExpectedEvidence {
            size: Some(99),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(
                Some(1),
                Some([0, 0, 0, 9]),
                None,
                Some(crate::hashes::sha1_bytes(b"rom")),
            ),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert!(matches!(
            result(&resolutions[0].status),
            ResolutionStatus::Matched {
                strength: MatchStrength::Sha1,
                ..
            }
        ));
    }

    #[test]
    fn compatibility_policy_rejects_non_whole_asset_evidence() {
        let sha1 = crate::hashes::sha1_bytes(b"rom");
        let mut rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        rom.expected.scope = EvidenceScope::Unknown;
        let inventory = [source(
            "/roms/disk.chd",
            computed(None, None, None, Some(sha1)),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::UnsupportedExpectedScope,
                ..
            }
        ));
    }

    #[test]
    fn compatibility_policy_ignores_non_whole_asset_observations() {
        let sha1 = crate::hashes::sha1_bytes(b"rom");
        let rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        let mut disk_observation = computed(None, None, None, Some(sha1));
        disk_observation.scope = EvidenceScope::DiskData;
        let mut unknown_observation = computed(None, None, None, Some(sha1));
        unknown_observation.scope = EvidenceScope::Unknown;
        let inventory = [
            source("/roms/disk.chd", disk_observation),
            source("/roms/unknown.bin", unknown_observation),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoMatchingSource,
                ..
            }
        ));
    }

    #[test]
    fn unrelated_digest_conflicts_do_not_turn_a_miss_into_a_conflict() {
        let rom = requirement(ExpectedEvidence {
            sha1: Some(crate::hashes::sha1_bytes(b"expected")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/unrelated.rom",
            computed(
                None,
                None,
                None,
                Some(crate::hashes::sha1_bytes(b"unrelated")),
            ),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoMatchingSource,
                ..
            }
        ));
    }

    #[test]
    fn evidence_aware_policy_uses_md5_only_when_observed_sha1_does_not_contradict() {
        let md5 = [3; 16];
        let rom = requirement(ExpectedEvidence {
            md5: Some(crate::domain::Md5Digest(md5)),
            sha1: Some(crate::hashes::sha1_bytes(b"expected")),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(
                    None,
                    None,
                    Some(md5),
                    Some(crate::hashes::sha1_bytes(b"wrong")),
                ),
            ),
            source("/roms/b.rom", computed(None, None, Some(md5), None)),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            result(&resolutions[0].status),
            ResolutionStatus::Matched {
                selected,
                strength: MatchStrength::Md5,
                ..
            } if selected.location.path() == "/roms/b.rom"
        ));
    }

    #[test]
    fn matching_stronger_evidence_retains_lower_priority_conflict_for_explanation() {
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            md5: Some(crate::domain::Md5Digest([1; 16])),
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(None, None, Some([2; 16]), Some(sha1)),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                strength: MatchStrength::Sha1,
                assessments,
                ..
            } if assessments[0].conflicts == [EvidenceField::Md5]
        ));
    }

    #[test]
    fn duplicate_strong_matches_are_equivalent_copies_with_stable_selection() {
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source("/roms/z.rom", computed(None, None, None, Some(sha1))),
            source("/roms/a.rom", computed(None, None, None, Some(sha1))),
            source(
                "/roms/unrelated.rom",
                computed(
                    None,
                    None,
                    None,
                    Some(crate::hashes::sha1_bytes(b"unrelated")),
                ),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                selected,
                equivalent_copies,
                assessments,
                ..
            } if selected.location.path() == "/roms/a.rom"
                && equivalent_copies.len() == 2
                && assessments.len() == 2
        ));
    }

    #[test]
    fn crc_and_size_matches_are_ambiguous_not_content_identity() {
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, None),
            ),
            source(
                "/roms/b.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, None),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
    }

    #[test]
    fn crc_and_size_tie_with_matching_sha1_preserves_same_content_copies() {
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/b.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, Some(sha1)),
            ),
            source(
                "/roms/a.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, Some(sha1)),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                selected,
                strength: MatchStrength::CrcAndSize,
                equivalent_copies,
                ..
            } if selected.location.path() == "/roms/a.rom"
                && equivalent_copies.len() == 2
        ));
    }

    #[test]
    fn large_crc_and_size_tie_with_shared_sha1_resolves_linearly() {
        const COPY_COUNT: usize = 10_000;
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = (0..COPY_COUNT)
            .map(|index| {
                source(
                    &format!("/roms/{index:05}.rom"),
                    computed(Some(3), Some([0, 0, 0, 7]), None, Some(sha1)),
                )
            })
            .collect::<Vec<_>>();

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                equivalent_copies,
                omitted_equivalent_copies,
                ..
            } if equivalent_copies.len() == MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND
                && *omitted_equivalent_copies
                    == COPY_COUNT - MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND
        ));
    }

    #[test]
    fn strongest_tie_after_retained_detail_cap_still_makes_result_ambiguous() {
        let sha1_a = crate::hashes::sha1_bytes(b"first content");
        let sha1_b = crate::hashes::sha1_bytes(b"late different content");
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = (0..=MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND)
            .map(|index| {
                let sha1 = if index == MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND {
                    sha1_b
                } else {
                    sha1_a
                };
                source(
                    &format!("/roms/{index:03}.rom"),
                    computed(Some(3), Some([0, 0, 0, 7]), None, Some(sha1)),
                )
            })
            .collect::<Vec<_>>();

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous {
                candidates,
                omitted_candidates: 1,
            } if candidates.len() == MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND
        ));
    }

    #[test]
    fn conflicting_candidate_after_retained_detail_cap_is_not_missed() {
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let mut inventory = (0..MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND)
            .map(|index| {
                source(
                    &format!("/roms/{index:03}.rom"),
                    computed(None, Some([0, 0, 0, 7]), None, None),
                )
            })
            .collect::<Vec<_>>();
        inventory.push(source(
            &format!("/roms/{MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND:03}.rom"),
            computed(Some(4), Some([0, 0, 0, 7]), None, None),
        ));

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Conflicting {
                candidates,
                omitted_candidates: 0,
            } if candidates.len() == 1
                && candidates[0].source.location.path().ends_with("/256.rom")
        ));
    }

    #[test]
    fn md5_tie_with_different_sha1_is_ambiguous() {
        let md5 = crate::domain::Md5Digest([7; 16]);
        let rom = requirement(ExpectedEvidence {
            md5: Some(md5),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(
                    None,
                    None,
                    Some(md5.0),
                    Some(crate::hashes::sha1_bytes(b"content a")),
                ),
            ),
            source(
                "/roms/b.rom",
                computed(
                    None,
                    None,
                    Some(md5.0),
                    Some(crate::hashes::sha1_bytes(b"content b")),
                ),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous { candidates, .. }
                if candidates.len() == 2
                    && candidates.iter().all(|candidate| candidate.strength == Some(MatchStrength::Md5))
        ));
    }

    #[test]
    fn md5_tie_without_complete_sha1_evidence_is_ambiguous() {
        let md5 = crate::domain::Md5Digest([8; 16]);
        let rom = requirement(ExpectedEvidence {
            md5: Some(md5),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(
                    None,
                    None,
                    Some(md5.0),
                    Some(crate::hashes::sha1_bytes(b"content")),
                ),
            ),
            source("/roms/b.rom", computed(None, None, Some(md5.0), None)),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
    }

    #[test]
    fn md5_tie_with_matching_sha1_preserves_same_content_copies() {
        let md5 = crate::domain::Md5Digest([9; 16]);
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            md5: Some(md5),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/b.rom",
                computed(Some(10), None, Some(md5.0), Some(sha1)),
            ),
            source(
                "/roms/a.rom",
                computed(Some(10), None, Some(md5.0), Some(sha1)),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                selected,
                strength: MatchStrength::Md5,
                equivalent_copies,
                ..
            } if selected.location.path() == "/roms/a.rom"
                && equivalent_copies.len() == 2
        ));
    }

    #[test]
    fn md5_tie_with_matching_sha1_but_different_xxh3_is_ambiguous() {
        let md5 = crate::domain::Md5Digest([11; 16]);
        let sha1 = crate::hashes::sha1_bytes(b"same reported evidence");
        let rom = requirement(ExpectedEvidence {
            md5: Some(md5),
            ..ExpectedEvidence::default()
        });
        let mut first = source("/roms/a.rom", computed(None, None, Some(md5.0), Some(sha1)));
        first.observed.xxh3 = [1; 8];
        let mut second = source("/roms/b.rom", computed(None, None, Some(md5.0), Some(sha1)));
        second.observed.xxh3 = [2; 8];

        let resolutions = resolve(
            &[rom],
            &[first, second],
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
    }

    #[test]
    fn md5_tie_with_matching_sha1_but_different_sizes_is_ambiguous() {
        let md5 = crate::domain::Md5Digest([10; 16]);
        let sha1 = crate::hashes::sha1_bytes(b"content");
        let rom = requirement(ExpectedEvidence {
            md5: Some(md5),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(Some(10), None, Some(md5.0), Some(sha1)),
            ),
            source(
                "/roms/b.rom",
                computed(Some(11), None, Some(md5.0), Some(sha1)),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
    }

    #[test]
    fn crc_without_size_is_insufficient_evidence_to_select_a_source() {
        let rom = requirement(ExpectedEvidence {
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(Some(3), Some([0, 0, 0, 7]), None, None),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::InsufficientEvidence,
                ..
            }
        ));
    }

    #[test]
    fn size_only_agreement_is_insufficient_not_a_content_match() {
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            sha1: Some(crate::hashes::sha1_bytes(b"expected")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source("/roms/a.rom", computed(Some(3), None, None, None))];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::InsufficientEvidence,
                ..
            }
        ));
    }

    #[test]
    fn matching_size_does_not_override_a_conflicting_digest() {
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            sha1: Some(crate::hashes::sha1_bytes(b"expected")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(
                Some(3),
                None,
                None,
                Some(crate::hashes::sha1_bytes(b"different")),
            ),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoMatchingSource,
                ..
            }
        ));
    }

    #[test]
    fn no_expected_or_observed_evidence_is_reported_without_guessing() {
        let empty_requirement = requirement(ExpectedEvidence::default());
        let hash_only = requirement(ExpectedEvidence {
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source("/roms/a.rom", computed(None, None, None, None))];

        let empty_result = resolve(
            &[empty_requirement],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );
        let hash_result = resolve(
            &[hash_only],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            empty_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoExpectedContentEvidence,
                ..
            }
        ));
        assert!(matches!(
            hash_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoComparableObservedEvidence,
                ..
            }
        ));

        let mut legacy_observation =
            computed(None, None, None, Some(crate::hashes::sha1_bytes(b"rom")));
        legacy_observation.provenance = EvidenceProvenance::Unknown;
        let legacy_result = resolve(
            &[requirement(ExpectedEvidence {
                sha1: Some(crate::hashes::sha1_bytes(b"rom")),
                ..ExpectedEvidence::default()
            })],
            &[source("/roms/legacy.rom", legacy_observation)],
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );
        assert!(matches!(
            legacy_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoComparableObservedEvidence,
                ..
            }
        ));
    }

    #[test]
    fn evidence_aware_policy_rejects_unsupported_expected_scope() {
        let mut disk_requirement = requirement(ExpectedEvidence {
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        disk_requirement.expected.scope = EvidenceScope::DiskData;
        let inventory = [source(
            "/roms/disk.chd",
            computed(None, None, None, Some(crate::hashes::sha1_bytes(b"rom"))),
        )];

        let resolutions = resolve(
            &[disk_requirement],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::UnsupportedExpectedScope,
                ..
            }
        ));
    }

    #[test]
    fn resolver_scopes_catalog_and_root_and_orders_candidates() {
        let sha1 = crate::hashes::sha1_bytes(b"rom");
        let mut rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        rom.catalog_name = "catalog-a".to_owned();
        let other_catalog = requirement(rom.expected.clone());
        let mut outside_source =
            source("/roms-other/a.rom", computed(None, None, None, Some(sha1)));
        outside_source.source_root = SourceRoot::new("/roms-other");
        let inventory = [
            outside_source,
            source("/roms/z.rom", computed(None, None, None, Some(sha1))),
            source("/roms/a.rom", computed(None, None, None, Some(sha1))),
        ];

        let resolutions = resolve(
            &[other_catalog.clone(), rom.clone()],
            &inventory,
            "catalog-a",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched { selected, .. }
                if selected.location.path() == "/roms/a.rom"
        ));
        let reordered = resolve(
            &[rom, other_catalog],
            &inventory.iter().rev().cloned().collect::<Vec<_>>(),
            "catalog-a",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );
        assert_eq!(resolutions, reordered);
    }

    #[test]
    fn resolver_preserves_sources_with_matching_stored_root() {
        let mut stored_root_source = source(
            "/legacy/location/rom.rom",
            computed(None, None, None, Some(crate::hashes::sha1_bytes(b"rom"))),
        );
        stored_root_source.source_root = SourceRoot::new("/roms");

        assert!(source_in_root(
            &stored_root_source,
            &SourceRoot::new("/roms")
        ));
        assert!(!source_in_root(
            &stored_root_source,
            &SourceRoot::new("/other")
        ));
    }

    #[test]
    fn resolver_bounds_retained_evidence_details_across_all_requirements() {
        let sha1 = crate::hashes::sha1_bytes(b"rom");
        let requirements = (0..300)
            .map(|_| {
                requirement(ExpectedEvidence {
                    sha1: Some(sha1),
                    ..ExpectedEvidence::default()
                })
            })
            .collect::<Vec<_>>();
        let inventory = (0..300)
            .map(|index| {
                source(
                    &format!("/roms/{index:03}.rom"),
                    computed(None, None, None, Some(sha1)),
                )
            })
            .collect::<Vec<_>>();

        let resolutions = resolve(
            &requirements,
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );
        let (retained_assessments, omitted_assessments) = resolutions
            .iter()
            .map(|resolution| match &resolution.status {
                ResolutionStatus::Matched {
                    assessments,
                    omitted_assessments,
                    ..
                } => (assessments.len(), *omitted_assessments),
                _ => (0, 0),
            })
            .fold(
                (0, 0),
                |(retained, omitted), (next_retained, next_omitted)| {
                    (retained + next_retained, omitted + next_omitted)
                },
            );

        assert_eq!(retained_assessments, MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND);
        assert_eq!(
            omitted_assessments,
            300 * 300 - MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND
        );
        let retained_copies = resolutions
            .iter()
            .map(|resolution| match &resolution.status {
                ResolutionStatus::Matched {
                    equivalent_copies, ..
                } => equivalent_copies.len(),
                _ => 0,
            })
            .sum::<usize>();
        assert_eq!(retained_copies, MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND);
        let omitted_copies = resolutions
            .iter()
            .map(|resolution| match &resolution.status {
                ResolutionStatus::Matched {
                    omitted_equivalent_copies,
                    ..
                } => *omitted_equivalent_copies,
                _ => 0,
            })
            .sum::<usize>();
        assert_eq!(
            omitted_copies,
            300 * 300 - MAX_RETAINED_EVIDENCE_DETAILS_PER_KIND
        );
    }
}
